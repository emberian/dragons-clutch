//! PROPOSED semantic kernel for resumable occupation resolution.
//!
//! This file is deliberately unreachable until the shared layout export,
//! router, account-plane refusal codes, and PDA registry are integrated.  It
//! contains the allocation-free state transitions that the eventual
//! `AccountInfo` handlers stage before any write or lamport movement.  In
//! particular, Fold accepts a lifetime-bound verified archive view and never
//! accepts record bytes, proofs, points, masses, or vectors from instruction
//! data.
//!
//! **STOP:** the temporary path import below is an isolation device, not a
//! second layout owner. Integration must replace it with the public
//! `clutch_solana_layout::resolution_work` module after the shared registry
//! rechecks the proposed tags.

#[path = "../../../../solana-layout/src/resolution_work.rs"]
#[allow(dead_code)] // isolated import includes instruction codecs routed only after integration
mod isolated_layout;

use crate::native_window::{
    self, NativeWindowError, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
    STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
};
use crate::source_archive::{
    SourceArchiveError, VerifiedSealedArchiveViewV1, SOURCE_ARCHIVE_MAX_RECORDS_V1,
};
use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS};
use clutch_bspline_accumulator::{
    Error as AccumulatorError, FinalizationMode, SequentialSummaryBuilder, Summary,
};
use clutch_solana_layout::{
    occupation_resolution::{
        OccupationResolutionAccount, RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION,
    },
    CodecError, Hash32, PayoutVectorBytes, TermsAccount, PAYOUT_INDEX_UNRESOLVED,
};
use isolated_layout::{
    ResolutionWorkAccountV1, ResolutionWorkCodecError, ResolutionWorkCostScheduleV1,
    ResolutionWorkFundingV1, ABORT_RESOLUTION_WORK_BYTES, BASIS_EVALUATOR_VERSION_V1,
    BASIS_SPEC_BYTES_V1, FINALIZATION_EXACT_ONLY, FINALIZATION_LARGEST_REMAINDER_V1,
    MAX_FOLD_RECORDS_V1, OCCUPATION_RESOLUTION_VERSION_V4, OCCUPATION_SUMMARY_VERSION_V1,
    RESOLUTION_WORK_ACCOUNT_BYTES, WORK_STATUS_ACTIVE,
};

/// Proposed deterministic Work PDA seed.
pub const RESOLUTION_WORK_SEED_V1: &[u8] = b"resolution-work-v1";
/// Proposed deterministic system-owned prepaid-reserve PDA seed.
pub const RESOLUTION_WORK_RESERVE_SEED_V1: &[u8] = b"resolution-reserve-v1";

const BASIS_MAGIC_V1: [u8; 8] = *b"DCBASV01";
const BASIS_SCHEMA_VERSION_V1: u16 = 1;
const BASIS_SEMANTIC_NATIVE_BSPLINE: u8 = 1;
const BASIS_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/basis-spec/v1";
const COST_DIGEST_DOMAIN_V1: &[u8] = b"DC_RESOLUTION_COST_SCHEDULE_V1";
const WORK_ID_DOMAIN_V1: &[u8] = b"DC_RESOLUTION_WORK_ID_V1";

/// Result of one proposed resolution-work semantic transition.
pub type Result<T> = core::result::Result<T, ResolutionWorkError>;

/// Typed refusal before the account plane assigns stable numeric projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionWorkError {
    /// The isolated Work account or instruction layout refused state.
    Codec(ResolutionWorkCodecError),
    /// Existing canonical Resolution/Terms vector layout refused state.
    OutputCodec(CodecError),
    /// Immutable Terms cannot form the registered occupation domain.
    Terms(NativeWindowError),
    /// The exact accumulator refused restored or appended state.
    Accumulator(AccumulatorError),
    /// The already-verified archive refused a bounded indexed read.
    Archive(SourceArchiveError),
    /// A market, Terms, basis, source, archive, grid, or version binding differs.
    BindingMismatch,
    /// Fold did not begin at the one exact next cursor.
    WrongCursor,
    /// Fold count was zero, over the call bound, or past the frozen end.
    InvalidChunk,
    /// A new Fold arrived after the inclusive expiry slot.
    Expired,
    /// A slot moved backward relative to the last successful transition.
    InvalidSlot,
    /// Finalize arrived before the exact end cursor.
    NotAtEnd,
    /// The prepaid reserve cannot cover a quoted transition.
    Underfunded,
    /// Abort is unsafe for the current progress/finalizability state.
    AbortForbidden,
    /// Checked cursor, cost, count, or refund arithmetic overflowed.
    ArithmeticOverflow,
}

impl From<ResolutionWorkCodecError> for ResolutionWorkError {
    fn from(error: ResolutionWorkCodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<CodecError> for ResolutionWorkError {
    fn from(error: CodecError) -> Self {
        Self::OutputCodec(error)
    }
}

impl From<NativeWindowError> for ResolutionWorkError {
    fn from(error: NativeWindowError) -> Self {
        Self::Terms(error)
    }
}

impl From<AccumulatorError> for ResolutionWorkError {
    fn from(error: AccumulatorError) -> Self {
        Self::Accumulator(error)
    }
}

impl From<SourceArchiveError> for ResolutionWorkError {
    fn from(error: SourceArchiveError) -> Self {
        Self::Archive(error)
    }
}

/// Already-authenticated immutable bindings supplied by the future Begin account plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginBindingsV1 {
    /// Derived Work identity/PDA.
    pub work_id: [u8; 32],
    /// Authenticated payer and sole close/refund destination.
    pub payer: [u8; 32],
    /// Derived prepaid reserve PDA.
    pub prepaid_reserve: [u8; 32],
    /// Nonzero payer-selected nonce included in the Work commitment.
    pub work_nonce: [u8; 32],
    /// Active market identity.
    pub market: [u8; 32],
    /// Canonical unresolved v4 Resolution target.
    pub resolution_target: [u8; 32],
    /// Exact Dragon's Clutch program id and archive owner.
    pub program_owner: [u8; 32],
    /// Digest recomputed from the canonical basis artifact.
    pub basis_spec_digest: [u8; 32],
    /// Digest recomputed from the canonical cost schedule.
    pub cost_schedule_digest: [u8; 32],
    /// Exact rent/charge/reward schedule selected by program policy.
    pub costs: ResolutionWorkCostScheduleV1,
    /// Actual total payer funding, including the exact Work rent reserve.
    pub deposited: u64,
    /// Begin slot.
    pub opened_slot: u64,
    /// Inclusive final Fold slot.
    pub expires_slot: u64,
    /// Exact final averaging rule.
    pub finalization_mode: u8,
    /// Canonical Work PDA bump.
    pub work_bump: u8,
    /// Canonical prepaid reserve PDA bump.
    pub reserve_bump: u8,
}

/// Optimistic Fold guards decoded from the fixed 107-byte payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldGuardsV1 {
    /// Expected Work identity.
    pub work_id: [u8; 32],
    /// Exact archive PDA.
    pub archive_account: [u8; 32],
    /// Full sealed archive commitment.
    pub archive_commitment: [u8; 32],
    /// Exact next cursor.
    pub expected_cursor: u64,
    /// Nonzero bounded contiguous record count.
    pub record_count: u8,
}

/// Successful staged Fold accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldTransitionV1 {
    /// Complete post-Fold Work image.
    pub next: ResolutionWorkAccountV1,
    /// Non-reward charge debited only from prepaid budget.
    pub charge: u64,
    /// Worker reward debited only from prepaid budget.
    pub reward: u64,
}

/// Successful staged Finalize result before atomic account writes/transfers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizeTransitionV1 {
    /// Sole canonical persisted payout authority.
    pub resolution: OccupationResolutionAccount,
    /// Non-reward charge debited only from prepaid budget.
    pub charge: u64,
    /// Finalizer reward debited only from prepaid budget.
    pub reward: u64,
    /// Unused prepaid budget plus released Work rent returned to the payer.
    pub payer_refund: u64,
}

/// Narrow reason a Work item may close without writing Resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortReasonV1 {
    /// Payer cancels before the first Fold.
    Unstarted,
    /// Incomplete Work is permissionlessly reaped strictly after expiry.
    ExpiredIncomplete,
    /// Complete archive has no accepted observation.
    CompleteNoCoverage,
    /// Complete archive retains one or more authenticated gaps.
    CompleteWithGaps,
    /// Complete exact-only average is not representable.
    CompleteInexactAverage,
}

/// Successful staged Abort result before atomic close/transfers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbortTransitionV1 {
    /// Exact safe abort reason.
    pub reason: AbortReasonV1,
    /// Non-reward charge debited only from prepaid budget.
    pub charge: u64,
    /// Aborter reward debited only from prepaid budget.
    pub reward: u64,
    /// Unused prepaid budget plus released Work rent returned to the payer.
    pub payer_refund: u64,
}

/// Construct the canonical active Work image after the account plane has
/// authenticated Market, unresolved Resolution, Terms, SourceSpec, and the
/// exact sealed archive account.
#[inline(never)]
pub fn begin_state(
    bindings: BeginBindingsV1,
    terms: &TermsAccount,
    archive: VerifiedSealedArchiveViewV1<'_>,
) -> Result<ResolutionWorkAccountV1> {
    let domain = native_window::occupation_domain(terms)?;
    let receipt = archive.receipt();
    let span = receipt
        .end_bucket_exclusive()
        .checked_sub(receipt.start_bucket())
        .ok_or(ResolutionWorkError::BindingMismatch)?;
    let record_count = u8::try_from(span).map_err(|_| ResolutionWorkError::InvalidChunk)?;
    if record_count == 0 || usize::from(record_count) > SOURCE_ARCHIVE_MAX_RECORDS_V1 {
        return Err(ResolutionWorkError::InvalidChunk);
    }
    if receipt.feed() != terms.feed
        || receipt.start_bucket() != terms.expected_start_bucket
        || receipt.end_bucket_exclusive() != terms.expected_end_bucket_exclusive
        || receipt.repair_generation() != terms.repair_generation
    {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    let expected_finalization = finalization_from_terms(terms)?;
    if bindings.finalization_mode != expected_finalization {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    let basis_spec_artifact = encode_basis_artifact(domain.spec());
    if bindings.basis_spec_digest != basis_artifact_digest(&basis_spec_artifact)
        || bindings.cost_schedule_digest != cost_schedule_digest(bindings.costs)
        || bindings.work_id
            != work_identity(
                &bindings,
                terms,
                receipt.archive_key(),
                receipt.page_commitment().bytes(),
                receipt.window().bytes(),
                receipt.repair_generation(),
                receipt.start_bucket(),
                receipt.end_bucket_exclusive(),
            )
    {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    let prepaid_remaining = bindings
        .deposited
        .checked_sub(bindings.costs.rent_reserve)
        .and_then(|value| value.checked_sub(bindings.costs.begin_charge))
        .ok_or(ResolutionWorkError::Underfunded)?;
    let value = ResolutionWorkAccountV1 {
        work_id: bindings.work_id,
        payer: bindings.payer,
        prepaid_reserve: bindings.prepaid_reserve,
        work_nonce: bindings.work_nonce,
        market: bindings.market,
        terms_digest: terms.terms.bytes(),
        resolution_target: bindings.resolution_target,
        program_owner: bindings.program_owner,
        archive_account: receipt.archive_key(),
        basis_spec_digest: bindings.basis_spec_digest,
        source_spec_digest: terms.feed.bytes(),
        archive_commitment: receipt.page_commitment().bytes(),
        archive_domain_digest: receipt.window().bytes(),
        grid_identity: native_window::canonical_grid_identity_v1(
            terms.grid_family_id,
            terms.grid_version,
            terms.bucket_seconds,
        ),
        basis_spec_artifact,
        archive_generation: receipt.repair_generation(),
        bucket_duration: terms.bucket_seconds,
        start_bucket: receipt.start_bucket(),
        end_bucket_exclusive: receipt.end_bucket_exclusive(),
        opened_slot: bindings.opened_slot,
        expires_slot: bindings.expires_slot,
        last_progress_slot: bindings.opened_slot,
        next_bucket: receipt.start_bucket(),
        fold_count: 0,
        completion_slot: 0,
        sample_count: 0,
        coverage_count: 0,
        denominator: domain.spec().denominator,
        masses: [0; 16],
        costs: bindings.costs,
        cost_schedule_digest: bindings.cost_schedule_digest,
        funding: ResolutionWorkFundingV1 {
            deposited: bindings.deposited,
            rent_locked: bindings.costs.rent_reserve,
            prepaid_remaining,
            charges_paid: bindings.costs.begin_charge,
            rewards_paid: 0,
        },
        status: WORK_STATUS_ACTIVE,
        finalization_mode: bindings.finalization_mode,
        outcome_count: domain.spec().outcome_count,
        archive_record_count: record_count,
        basis_evaluator_version: BASIS_EVALUATOR_VERSION_V1,
        occupation_summary_version: OCCUPATION_SUMMARY_VERSION_V1,
        resolution_version: OCCUPATION_RESOLUTION_VERSION_V4,
        stored_bump: bindings.work_bump,
        reserve_bump: bindings.reserve_bump,
        flags: 0,
        reserved: [0; 3],
    };
    value.validate()?;
    Ok(value)
}

/// Recheck immutable Terms/archive bindings and stage one exact contiguous Fold.
#[inline(never)]
pub fn fold_state(
    work: ResolutionWorkAccountV1,
    guards: FoldGuardsV1,
    terms: &TermsAccount,
    archive: VerifiedSealedArchiveViewV1<'_>,
    current_slot: u64,
) -> Result<FoldTransitionV1> {
    work.validate()?;
    validate_static_bindings(&work, terms, archive)?;
    if guards.work_id != work.work_id
        || guards.archive_account != work.archive_account
        || guards.archive_commitment != work.archive_commitment
    {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    if guards.expected_cursor != work.next_bucket {
        return Err(ResolutionWorkError::WrongCursor);
    }
    if guards.record_count == 0 || guards.record_count > MAX_FOLD_RECORDS_V1 {
        return Err(ResolutionWorkError::InvalidChunk);
    }
    if current_slot < work.last_progress_slot {
        return Err(ResolutionWorkError::InvalidSlot);
    }
    if current_slot > work.expires_slot {
        return Err(ResolutionWorkError::Expired);
    }
    let chunk_end = work
        .next_bucket
        .checked_add(u64::from(guards.record_count))
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    if chunk_end > work.end_bucket_exclusive {
        return Err(ResolutionWorkError::InvalidChunk);
    }

    let domain = native_window::occupation_domain(terms)?;
    let restored = Summary::from_canonical_parts(
        domain,
        if work.sample_count == 0 {
            0
        } else {
            work.start_bucket
        },
        if work.sample_count == 0 {
            0
        } else {
            work.next_bucket
        },
        work.sample_count,
        work.coverage_count,
        work.masses,
    )?;
    let mut accumulator = SequentialSummaryBuilder::resume(restored)?;
    let receipt = archive.receipt();
    let mut offset = 0_u64;
    while offset < u64::from(guards.record_count) {
        let bucket = work
            .next_bucket
            .checked_add(offset)
            .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
        let archive_index = bucket
            .checked_sub(work.start_bucket)
            .ok_or(ResolutionWorkError::WrongCursor)?;
        let observation = archive.archived_observation(
            usize::try_from(archive_index).map_err(|_| ResolutionWorkError::InvalidChunk)?,
        )?;
        if observation.bucket != bucket || observation.low != observation.high {
            return Err(ResolutionWorkError::BindingMismatch);
        }
        accumulator.append_accepted(bucket, observation.low)?;
        offset += 1;
    }
    if receipt.page_commitment().bytes() != work.archive_commitment {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    let summary = accumulator.finish();
    let (charge, reward) = fold_quote(work.costs, guards.record_count)?;
    let mut next = work;
    debit(&mut next.funding, charge, reward)?;
    next.next_bucket = chunk_end;
    next.fold_count = next
        .fold_count
        .checked_add(1)
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    next.last_progress_slot = current_slot;
    next.sample_count = summary.sample_count();
    next.coverage_count = summary.coverage_count();
    next.masses = summary.masses();
    if chunk_end == next.end_bucket_exclusive {
        next.completion_slot = current_slot;
    }
    next.validate()?;
    Ok(FoldTransitionV1 {
        next,
        charge,
        reward,
    })
}

/// Stage the sole canonical Resolution V4 image and terminal accounting.
///
/// The account plane must validate this result before mutating anything, then
/// atomically update Market/kernel/supply exactly as monolithic v4 Resolve,
/// write this Resolution once, pay from reserve, and close Work/reserve. Any
/// late failure must leave every prestate byte and lamport unchanged.
#[inline(never)]
pub fn finalize_state(
    work: ResolutionWorkAccountV1,
    terms: &TermsAccount,
    archive: VerifiedSealedArchiveViewV1<'_>,
    expected_cursor: u64,
    expected_archive_commitment: [u8; 32],
    current_slot: u64,
    resolution_bump: u8,
) -> Result<FinalizeTransitionV1> {
    work.validate()?;
    validate_static_bindings(&work, terms, archive)?;
    if expected_cursor != work.next_bucket || expected_archive_commitment != work.archive_commitment
    {
        return Err(ResolutionWorkError::WrongCursor);
    }
    if current_slot < work.last_progress_slot {
        return Err(ResolutionWorkError::InvalidSlot);
    }
    if work.next_bucket != work.end_bucket_exclusive {
        return Err(ResolutionWorkError::NotAtEnd);
    }
    let domain = native_window::occupation_domain(terms)?;
    let summary = Summary::from_canonical_parts(
        domain,
        work.start_bucket,
        work.end_bucket_exclusive,
        work.sample_count,
        work.coverage_count,
        work.masses,
    )?;
    let finalized = summary.finalize(accumulator_mode(work.finalization_mode)?)?;
    let vector = PayoutVectorBytes {
        denominator: finalized.denominator(),
        weights: finalized.weights(),
    };
    vector.validate_active(finalized.active_len(), finalized.denominator())?;
    let receipt = archive.receipt();
    let resolution = OccupationResolutionAccount {
        market: Hash32::from_bytes(work.market),
        terms: Hash32::from_bytes(work.terms_digest),
        feed: Hash32::from_bytes(work.source_spec_digest),
        window: Hash32::from_bytes(work.archive_domain_digest),
        feed_cursor: receipt.sealed_feed_cursor(),
        sealed_end_bucket_exclusive: work.end_bucket_exclusive,
        repair_generation: work.archive_generation,
        resolved_slot: current_slot,
        mode: RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        outcome_count: work.outcome_count,
        resolved_value: 0,
        vector,
        archive_commitment: Hash32::from_bytes(work.archive_commitment),
        statistic: terms.statistic_id,
        finalization: work.finalization_mode,
        basis_evaluator_version: work.basis_evaluator_version,
        occupation_summary_version: work.occupation_summary_version,
        sample_count: work.sample_count,
        coverage_count: work.coverage_count,
        gap_count: work
            .sample_count
            .checked_sub(work.coverage_count)
            .ok_or(ResolutionWorkError::ArithmeticOverflow)?,
        stored_bump: resolution_bump,
        flags: 0,
        reserved: 0,
    };
    resolution.validate()?;
    let (charge, reward, payer_refund) = terminal_quote(
        work.funding,
        work.costs.finalize_charge,
        work.costs.finalize_reward,
    )?;
    Ok(FinalizeTransitionV1 {
        resolution,
        charge,
        reward,
        payer_refund,
    })
}

/// Stage a narrowly permitted terminal close that cannot write Resolution.
#[inline(never)]
pub fn abort_state(
    work: ResolutionWorkAccountV1,
    terms: &TermsAccount,
    current_slot: u64,
    caller_is_payer: bool,
) -> Result<AbortTransitionV1> {
    work.validate()?;
    validate_work_identity(&work, terms)?;
    if current_slot < work.last_progress_slot {
        return Err(ResolutionWorkError::InvalidSlot);
    }
    let reason = if work.sample_count == 0 {
        if !caller_is_payer {
            return Err(ResolutionWorkError::AbortForbidden);
        }
        AbortReasonV1::Unstarted
    } else if work.next_bucket != work.end_bucket_exclusive {
        if current_slot <= work.expires_slot {
            return Err(ResolutionWorkError::AbortForbidden);
        }
        AbortReasonV1::ExpiredIncomplete
    } else {
        let domain = native_window::occupation_domain(terms)?;
        if domain.spec_digest() != work.terms_digest
            || encode_basis_artifact(domain.spec()) != work.basis_spec_artifact
        {
            return Err(ResolutionWorkError::BindingMismatch);
        }
        let summary = Summary::from_canonical_parts(
            domain,
            work.start_bucket,
            work.end_bucket_exclusive,
            work.sample_count,
            work.coverage_count,
            work.masses,
        )?;
        if summary.coverage_count() == 0 {
            AbortReasonV1::CompleteNoCoverage
        } else if summary.gap_count() != 0 {
            AbortReasonV1::CompleteWithGaps
        } else {
            match summary.finalize(accumulator_mode(work.finalization_mode)?) {
                Err(AccumulatorError::InexactAverage) => AbortReasonV1::CompleteInexactAverage,
                Err(error) => return Err(error.into()),
                Ok(_) => return Err(ResolutionWorkError::AbortForbidden),
            }
        }
    };
    let (charge, reward, payer_refund) = terminal_quote(
        work.funding,
        work.costs.abort_charge,
        work.costs.abort_reward,
    )?;
    Ok(AbortTransitionV1 {
        reason,
        charge,
        reward,
        payer_refund,
    })
}

fn validate_static_bindings(
    work: &ResolutionWorkAccountV1,
    terms: &TermsAccount,
    archive: VerifiedSealedArchiveViewV1<'_>,
) -> Result<()> {
    let domain = native_window::occupation_domain(terms)?;
    let receipt = archive.receipt();
    validate_work_identity(work, terms)?;
    if work.terms_digest != terms.terms.bytes()
        || work.source_spec_digest != terms.feed.bytes()
        || work.basis_spec_artifact != encode_basis_artifact(domain.spec())
        || work.basis_spec_digest != basis_artifact_digest(&work.basis_spec_artifact)
        || work.cost_schedule_digest != cost_schedule_digest(work.costs)
        || work.grid_identity
            != native_window::canonical_grid_identity_v1(
                terms.grid_family_id,
                terms.grid_version,
                terms.bucket_seconds,
            )
        || work.bucket_duration != terms.bucket_seconds
        || work.outcome_count != terms.outcome_count
        || work.denominator != terms.payouts[0].denominator
        || work.finalization_mode != finalization_from_terms(terms)?
        || work.archive_account != receipt.archive_key()
        || work.archive_commitment != receipt.page_commitment().bytes()
        || work.archive_domain_digest != receipt.window().bytes()
        || work.archive_generation != receipt.repair_generation()
        || work.start_bucket != receipt.start_bucket()
        || work.end_bucket_exclusive != receipt.end_bucket_exclusive()
        || work.status != WORK_STATUS_ACTIVE
    {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    Ok(())
}

fn validate_work_identity(work: &ResolutionWorkAccountV1, terms: &TermsAccount) -> Result<()> {
    let domain = native_window::occupation_domain(terms)?;
    let bindings = BeginBindingsV1 {
        work_id: work.work_id,
        payer: work.payer,
        prepaid_reserve: work.prepaid_reserve,
        work_nonce: work.work_nonce,
        market: work.market,
        resolution_target: work.resolution_target,
        program_owner: work.program_owner,
        basis_spec_digest: work.basis_spec_digest,
        cost_schedule_digest: work.cost_schedule_digest,
        costs: work.costs,
        deposited: work.funding.deposited,
        opened_slot: work.opened_slot,
        expires_slot: work.expires_slot,
        finalization_mode: work.finalization_mode,
        work_bump: work.stored_bump,
        reserve_bump: work.reserve_bump,
    };
    if work.terms_digest != terms.terms.bytes()
        || work.source_spec_digest != terms.feed.bytes()
        || work.basis_spec_artifact != encode_basis_artifact(domain.spec())
        || work.basis_spec_digest != basis_artifact_digest(&work.basis_spec_artifact)
        || work.cost_schedule_digest != cost_schedule_digest(work.costs)
        || work.work_id
            != work_identity(
                &bindings,
                terms,
                work.archive_account,
                work.archive_commitment,
                work.archive_domain_digest,
                work.archive_generation,
                work.start_bucket,
                work.end_bucket_exclusive,
            )
    {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    Ok(())
}

fn finalization_from_terms(terms: &TermsAccount) -> Result<u8> {
    match terms.statistic_id {
        STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06 => Ok(FINALIZATION_EXACT_ONLY),
        STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07 => {
            Ok(FINALIZATION_LARGEST_REMAINDER_V1)
        }
        _ => Err(ResolutionWorkError::BindingMismatch),
    }
}

fn accumulator_mode(finalization: u8) -> Result<FinalizationMode> {
    match finalization {
        FINALIZATION_EXACT_ONLY => Ok(FinalizationMode::ExactOnly),
        FINALIZATION_LARGEST_REMAINDER_V1 => Ok(FinalizationMode::LargestRemainderV1),
        _ => Err(ResolutionWorkError::BindingMismatch),
    }
}

fn fold_quote(costs: ResolutionWorkCostScheduleV1, records: u8) -> Result<(u64, u64)> {
    let charge = costs
        .fold_per_record_charge
        .checked_mul(u64::from(records))
        .and_then(|value| value.checked_add(costs.fold_base_charge))
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    let reward = costs
        .fold_per_record_reward
        .checked_mul(u64::from(records))
        .and_then(|value| value.checked_add(costs.fold_base_reward))
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    Ok((charge, reward))
}

fn debit(funding: &mut ResolutionWorkFundingV1, charge: u64, reward: u64) -> Result<()> {
    let outflow = charge
        .checked_add(reward)
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    funding.prepaid_remaining = funding
        .prepaid_remaining
        .checked_sub(outflow)
        .ok_or(ResolutionWorkError::Underfunded)?;
    funding.charges_paid = funding
        .charges_paid
        .checked_add(charge)
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    funding.rewards_paid = funding
        .rewards_paid
        .checked_add(reward)
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    Ok(())
}

fn terminal_quote(
    mut funding: ResolutionWorkFundingV1,
    charge: u64,
    reward: u64,
) -> Result<(u64, u64, u64)> {
    debit(&mut funding, charge, reward)?;
    let refund = funding
        .prepaid_remaining
        .checked_add(funding.rent_locked)
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    Ok((charge, reward, refund))
}

fn encode_basis_artifact(spec: BasisSpec) -> [u8; BASIS_SPEC_BYTES_V1] {
    let mut out = [0; BASIS_SPEC_BYTES_V1];
    out[..8].copy_from_slice(&BASIS_MAGIC_V1);
    out[8..10].copy_from_slice(&BASIS_SCHEMA_VERSION_V1.to_le_bytes());
    out[10..12].copy_from_slice(&BASIS_EVALUATOR_VERSION_V1.to_le_bytes());
    out[12] = BASIS_SEMANTIC_NATIVE_BSPLINE;
    out[13] = spec.outcome_count;
    out[14] = spec.degree;
    out[15] = spec.knot_count;
    out[16] = spec.uniform_log2_spacing;
    out[17] = match spec.edge_policy {
        EdgePolicy::Clamp => 1,
        EdgePolicy::Refuse => 2,
    };
    out[24..32].copy_from_slice(&spec.denominator.to_le_bytes());
    out[32..48].copy_from_slice(&spec.domain_max.to_le_bytes());
    let mut index = 0_usize;
    while index < MAX_KNOTS {
        let start = 48 + (index * 16);
        out[start..start + 16].copy_from_slice(&spec.knots[index].to_le_bytes());
        index += 1;
    }
    out
}

fn basis_artifact_digest(bytes: &[u8; BASIS_SPEC_BYTES_V1]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[BASIS_DIGEST_DOMAIN_V1, bytes]).to_bytes()
}

fn cost_schedule_digest(costs: ResolutionWorkCostScheduleV1) -> [u8; 32] {
    let mut bytes = [0_u8; 2 + 4 + (11 * 8)];
    let mut at = 0_usize;
    append_bytes(&mut bytes, &mut at, &costs.version.to_be_bytes());
    append_bytes(&mut bytes, &mut at, &costs.work_state_bytes.to_be_bytes());
    for value in [
        costs.rent_reserve,
        costs.minimum_lifetime_slots,
        costs.begin_charge,
        costs.fold_base_charge,
        costs.fold_per_record_charge,
        costs.fold_base_reward,
        costs.fold_per_record_reward,
        costs.finalize_charge,
        costs.finalize_reward,
        costs.abort_charge,
        costs.abort_reward,
    ] {
        append_bytes(&mut bytes, &mut at, &value.to_be_bytes());
    }
    solana_sha256_hasher::hashv(&[COST_DIGEST_DOMAIN_V1, &bytes]).to_bytes()
}

#[allow(clippy::too_many_arguments)]
fn work_identity(
    bindings: &BeginBindingsV1,
    terms: &TermsAccount,
    archive_account: [u8; 32],
    archive_commitment: [u8; 32],
    archive_domain: [u8; 32],
    archive_generation: u64,
    start_bucket: u64,
    end_bucket_exclusive: u64,
) -> [u8; 32] {
    let version = 1_u16.to_be_bytes();
    let archive_generation = archive_generation.to_be_bytes();
    let start_bucket = start_bucket.to_be_bytes();
    let end_bucket_exclusive = end_bucket_exclusive.to_be_bytes();
    let evaluator = BASIS_EVALUATOR_VERSION_V1.to_be_bytes();
    let summary = OCCUPATION_SUMMARY_VERSION_V1.to_be_bytes();
    let resolution = OCCUPATION_RESOLUTION_VERSION_V4.to_be_bytes();
    let mode = [bindings.finalization_mode];
    let opened = bindings.opened_slot.to_be_bytes();
    let expires = bindings.expires_slot.to_be_bytes();
    solana_sha256_hasher::hashv(&[
        WORK_ID_DOMAIN_V1,
        &version,
        &bindings.market,
        &terms.terms.bytes(),
        &bindings.resolution_target,
        &bindings.program_owner,
        &archive_account,
        &bindings.basis_spec_digest,
        &terms.feed.bytes(),
        &archive_commitment,
        &archive_domain,
        &archive_generation,
        &start_bucket,
        &end_bucket_exclusive,
        &evaluator,
        &summary,
        &resolution,
        &mode,
        &bindings.cost_schedule_digest,
        &bindings.payer,
        &bindings.prepaid_reserve,
        &bindings.work_nonce,
        &opened,
        &expires,
    ])
    .to_bytes()
}

fn append_bytes<const N: usize>(out: &mut [u8; N], at: &mut usize, value: &[u8]) {
    let end = *at + value.len();
    out[*at..end].copy_from_slice(value);
    *at = end;
}

const _: () = assert!(RESOLUTION_WORK_ACCOUNT_BYTES == 1_296);
const _: () = assert!(ABORT_RESOLUTION_WORK_BYTES == 74);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{
        ParsedPriceV1, PriceParserV1, SourceAccountView, SourceError, SourceSpecFieldsV1,
        SourceSpecV1, TrustedClockV1, ORIENTATION_QUOTE_PER_BASE,
        SELECTION_FINALIZED_BUCKET_RECORD,
    };
    use crate::source_archive::{
        append_authenticated, initialize_archive, initialize_source_spec_account, seal_archive,
        verify_recorded_sealed_archive_view, verify_source_spec_account, ArchiveAccountViewV1,
        ArchivePredecessorV1, CoveragePolicy, DeploymentAuthenticatorV1, FeedIdentity, Grid,
        RuntimeAccountViewV1, SourceSpecAccountViewV1, VerifiedSourceSpecAccountV1, WindowDomain,
        SOURCE_ARCHIVE_ACCOUNT_V1_BYTES, SOURCE_SPEC_ACCOUNT_V1_BYTES,
    };
    use clutch_solana_layout::{MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED};

    const ADAPTER: [u8; 32] = [0xa1; 32];
    const PROVIDER_PROGRAM: [u8; 32] = [0xb2; 32];
    const PROVIDER_LOADER: [u8; 32] = [0xb3; 32];
    const DEPLOYMENT: [u8; 32] = [0xb4; 32];
    const DEPLOYMENT_OWNER: [u8; 32] = [0xb5; 32];
    const SOURCE_ACCOUNT: [u8; 32] = [0xc3; 32];
    const VERIFIER: [u8; 32] = [0xd4; 32];
    const CLUTCH_PROGRAM: [u8; 32] = [0xe5; 32];
    const SOURCE_SPEC_KEY: [u8; 32] = [0xe6; 32];
    const ARCHIVE_KEY: [u8; 32] = [0xf6; 32];
    const DEPLOYMENT_GENERATION: u64 = 9;
    const RECORD_BYTES: usize = 77;

    struct MockDeployment;

    impl DeploymentAuthenticatorV1 for MockDeployment {
        const VERIFIER_ID: [u8; 32] = VERIFIER;
        const VERIFIER_VERSION: u32 = 2;
        const PROVIDER_PROGRAM: [u8; 32] = PROVIDER_PROGRAM;
        const PROVIDER_PROGRAM_OWNER: [u8; 32] = PROVIDER_LOADER;
        const DEPLOYMENT_ACCOUNT: [u8; 32] = DEPLOYMENT;
        const DEPLOYMENT_OWNER: [u8; 32] = DEPLOYMENT_OWNER;

        fn deployment_generation(
            provider_program_data: &[u8],
            deployment_account_data: &[u8],
        ) -> core::result::Result<u64, SourceArchiveError> {
            if provider_program_data != b"mock-provider-program-v1"
                || deployment_account_data.len() != 16
                || &deployment_account_data[..8] != b"MOCKDEP1"
            {
                return Err(SourceArchiveError::DeploymentAdapterRefused);
            }
            let mut generation = [0; 8];
            generation.copy_from_slice(&deployment_account_data[8..]);
            Ok(u64::from_le_bytes(generation))
        }
    }

    struct MockParser;

    impl PriceParserV1 for MockParser {
        const SOURCE_ADAPTER_ID: [u8; 32] = ADAPTER;
        const SOURCE_ADAPTER_VERSION: u32 = 7;
        const PARSER_ID: u16 = 11;
        const PARSER_VERSION: u16 = 3;

        fn parse(
            account: SourceAccountView<'_>,
        ) -> core::result::Result<ParsedPriceV1, SourceError> {
            let bytes = account.data();
            if bytes.len() != RECORD_BYTES || &bytes[..4] != b"SRC1" {
                return Err(SourceError::ParserRefused);
            }
            Ok(ParsedPriceV1 {
                deployment_generation: u64_at(bytes, 4),
                source_sequence: u64_at(bytes, 12),
                publish_slot: u64_at(bytes, 20),
                publish_time: u64_at(bytes, 28),
                canonical_bucket: u64_at(bytes, 36),
                finalized_bucket: bytes[76] == 1,
                price_atoms: u128_at(bytes, 44),
                confidence_atoms: u128_at(bytes, 60),
            })
        }
    }

    fn hash(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn source_spec() -> SourceSpecV1 {
        SourceSpecV1::new(SourceSpecFieldsV1 {
            source_adapter_id: Hash32::from_bytes(ADAPTER),
            source_adapter_version: 7,
            parser_id: 11,
            parser_version: 3,
            source_program: PROVIDER_PROGRAM,
            source_account: SOURCE_ACCOUNT,
            deployment_generation: DEPLOYMENT_GENERATION,
            base_asset_id: hash(1),
            quote_asset_id: hash(2),
            orientation: ORIENTATION_QUOTE_PER_BASE,
            normalized_decimals: 6,
            grid_family_id: 5,
            grid_version: 2,
            bucket_seconds: 60,
            max_staleness_slots: 20,
            max_staleness_seconds: 120,
            max_future_seconds: 2,
            max_confidence_atoms: 10,
            max_confidence_bps: 10_000,
            confidence_multiplier: 1,
            selection_rule: SELECTION_FINALIZED_BUCKET_RECORD,
        })
        .unwrap()
    }

    fn window(spec: SourceSpecV1) -> WindowDomain {
        let feed = FeedIdentity::new(ADAPTER, spec.feed_id().bytes(), 7, 1).unwrap();
        WindowDomain::new(
            feed,
            Grid::new(5, 2, 60).unwrap(),
            100,
            104,
            105,
            6,
            CoveragePolicy::COMPLETE_REQUIRED,
        )
        .unwrap()
    }

    fn terms(spec: SourceSpecV1) -> TermsAccount {
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut anchor = [0; MAX_OUTCOMES];
        anchor[0] = 7;
        payouts[0] = PayoutVectorBytes {
            denominator: 7,
            weights: anchor,
        };
        let mut knots = [0; MAX_KNOTS];
        knots[..3].copy_from_slice(&[0, 8, 16]);
        let mut value = TermsAccount {
            terms: Hash32::ZERO,
            realm: hash(0x11),
            profile: hash(0x12),
            feed: spec.feed_id(),
            price_grid: hash(0x13),
            outcome_count: 4,
            payout_count: 1,
            payouts,
            grid_family_id: 5,
            grid_version: 2,
            bucket_seconds: 60,
            expected_start_bucket: 100,
            expected_end_bucket_exclusive: 104,
            maturity_horizon_buckets: 5,
            coverage_policy_id: u32::from(CoveragePolicy::COMPLETE_REQUIRED.id()),
            repair_policy_id: 1,
            failure_policy_id: 1,
            statistic_id: STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
            ambiguity_policy_id: 1,
            edge_policy_id: 1,
            basis_degree: 2,
            knot_count: 3,
            uniform_log2_spacing: 3,
            failure_payout_index: 0,
            coverage_policy_parameter: 0,
            repair_generation: 6,
            source_version: 7,
            evaluator_version: 1,
            source_adapter_id: Hash32::from_bytes(ADAPTER),
            payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
            knots,
            collateral_cap: 1_000,
            stored_bump: 9,
            flags: 0,
        };
        value.terms = value.recomputed_terms_digest().unwrap();
        value.validate().unwrap();
        value
    }

    fn verified_spec(
        spec_account: &[u8; SOURCE_SPEC_ACCOUNT_V1_BYTES],
    ) -> VerifiedSourceSpecAccountV1 {
        verify_source_spec_account(
            CLUTCH_PROGRAM,
            SOURCE_SPEC_KEY,
            SourceSpecAccountViewV1::new(SOURCE_SPEC_KEY, CLUTCH_PROGRAM, false, spec_account),
        )
        .unwrap()
    }

    fn complete_archive(
        prices: [u128; 4],
    ) -> (
        SourceSpecV1,
        [u8; SOURCE_SPEC_ACCOUNT_V1_BYTES],
        [u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES],
        WindowDomain,
    ) {
        let spec = source_spec();
        let window = window(spec);
        let mut spec_account = [0; SOURCE_SPEC_ACCOUNT_V1_BYTES];
        initialize_source_spec_account(&mut spec_account, spec, 254).unwrap();
        let mut archive = [0; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
        initialize_archive::<MockDeployment>(
            &mut archive,
            verified_spec(&spec_account),
            window,
            ArchivePredecessorV1::GENESIS,
            253,
        )
        .unwrap();
        let deployment = deployment_bytes();
        for (index, price) in prices.into_iter().enumerate() {
            let bucket = 100 + index as u64;
            let record = record(bucket, 1 + index as u64, 1_000 + index as u64, price);
            append_authenticated::<MockParser, MockDeployment>(
                &mut archive,
                verified_spec(&spec_account),
                window,
                TrustedClockV1 {
                    slot: 1_005 + index as u64,
                    unix_seconds: bucket * 60 + 1,
                },
                RuntimeAccountViewV1::new(
                    PROVIDER_PROGRAM,
                    PROVIDER_LOADER,
                    true,
                    b"mock-provider-program-v1",
                ),
                RuntimeAccountViewV1::new(DEPLOYMENT, DEPLOYMENT_OWNER, false, &deployment),
                SourceAccountView::new(SOURCE_ACCOUNT, PROVIDER_PROGRAM, false, &record),
            )
            .unwrap();
        }
        seal_archive::<MockDeployment>(&mut archive, verified_spec(&spec_account), window, 105)
            .unwrap();
        (spec, spec_account, archive, window)
    }

    fn verified_archive<'a>(
        spec_account: &[u8; SOURCE_SPEC_ACCOUNT_V1_BYTES],
        archive: &'a [u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES],
        window: WindowDomain,
        key: [u8; 32],
    ) -> VerifiedSealedArchiveViewV1<'a> {
        verify_recorded_sealed_archive_view(
            CLUTCH_PROGRAM,
            key,
            ArchiveAccountViewV1::new(key, CLUTCH_PROGRAM, false, archive),
            verified_spec(spec_account),
            window,
        )
        .unwrap()
    }

    fn costs() -> ResolutionWorkCostScheduleV1 {
        ResolutionWorkCostScheduleV1 {
            version: 1,
            work_state_bytes: RESOLUTION_WORK_ACCOUNT_BYTES as u32,
            rent_reserve: 100,
            minimum_lifetime_slots: 10,
            begin_charge: 0,
            fold_base_charge: 0,
            fold_per_record_charge: 0,
            fold_base_reward: 1,
            fold_per_record_reward: 1,
            finalize_charge: 0,
            finalize_reward: 1,
            abort_charge: 0,
            abort_reward: 1,
        }
    }

    fn bindings(terms: &TermsAccount, receipt: VerifiedSealedArchiveViewV1<'_>) -> BeginBindingsV1 {
        let artifact =
            encode_basis_artifact(native_window::occupation_domain(terms).unwrap().spec());
        let mut value = BeginBindingsV1 {
            work_id: [0; 32],
            payer: [2; 32],
            prepaid_reserve: [3; 32],
            work_nonce: [4; 32],
            market: [5; 32],
            resolution_target: [6; 32],
            program_owner: CLUTCH_PROGRAM,
            basis_spec_digest: basis_artifact_digest(&artifact),
            cost_schedule_digest: cost_schedule_digest(costs()),
            costs: costs(),
            deposited: 200,
            opened_slot: 10,
            expires_slot: 20,
            finalization_mode: FINALIZATION_LARGEST_REMAINDER_V1,
            work_bump: 200,
            reserve_bump: 201,
        };
        let archive_receipt = receipt.receipt();
        value.work_id = work_identity(
            &value,
            terms,
            archive_receipt.archive_key(),
            archive_receipt.page_commitment().bytes(),
            archive_receipt.window().bytes(),
            archive_receipt.repair_generation(),
            archive_receipt.start_bucket(),
            archive_receipt.end_bucket_exclusive(),
        );
        value
    }

    #[test]
    fn every_chunk_partition_matches_monolithic_v4_and_late_finalize() {
        let (spec, spec_account, archive, window) = complete_archive([1, 4, 7, 12]);
        let terms = terms(spec);
        let archive_view = verified_archive(&spec_account, &archive, window, ARCHIVE_KEY);
        let begin = bindings(&terms, archive_view);
        for chunks in [[1_u8, 1, 1, 1], [2, 2, 0, 0], [4, 0, 0, 0]] {
            let mut work = begin_state(begin, &terms, archive_view).unwrap();
            let mut now = 11;
            for count in chunks {
                if count == 0 {
                    continue;
                }
                let guard = FoldGuardsV1 {
                    work_id: work.work_id,
                    archive_account: work.archive_account,
                    archive_commitment: work.archive_commitment,
                    expected_cursor: work.next_bucket,
                    record_count: count,
                };
                work = fold_state(work, guard, &terms, archive_view, now)
                    .unwrap()
                    .next;
                now += 1;
            }
            let terminal = finalize_state(
                work,
                &terms,
                archive_view,
                104,
                work.archive_commitment,
                99,
                7,
            )
            .unwrap();
            let monolithic =
                native_window::preflight_verified_archive(&terms, archive_view).unwrap();
            assert_eq!(terminal.resolution.vector, monolithic.vector());
            assert_eq!(terminal.resolution.sample_count, monolithic.sample_count());
            assert_eq!(
                terminal.resolution.coverage_count,
                monolithic.coverage_count()
            );
            assert_eq!(
                terminal.resolution.archive_commitment,
                monolithic.archive_commitment()
            );
            assert_eq!(
                terminal.payer_refund + terminal.reward + work.funding.rewards_paid,
                begin.deposited
            );
            assert_eq!(
                abort_state(work, &terms, 99, false),
                Err(ResolutionWorkError::AbortForbidden)
            );
        }
    }

    #[test]
    fn wrong_cursor_archive_replay_expiry_and_underfunding_are_atomic() {
        let (spec, spec_account, archive, window) = complete_archive([1, 4, 7, 12]);
        let terms = terms(spec);
        let archive_view = verified_archive(&spec_account, &archive, window, ARCHIVE_KEY);
        let begin = bindings(&terms, archive_view);
        let work = begin_state(begin, &terms, archive_view).unwrap();
        let mut wrong = FoldGuardsV1 {
            work_id: work.work_id,
            archive_account: work.archive_account,
            archive_commitment: work.archive_commitment,
            expected_cursor: 101,
            record_count: 1,
        };
        assert_eq!(
            fold_state(work, wrong, &terms, archive_view, 11),
            Err(ResolutionWorkError::WrongCursor)
        );
        wrong.expected_cursor = 100;
        let first = fold_state(work, wrong, &terms, archive_view, 11)
            .unwrap()
            .next;
        assert_eq!(
            fold_state(first, wrong, &terms, archive_view, 12),
            Err(ResolutionWorkError::WrongCursor)
        );
        let current = FoldGuardsV1 {
            expected_cursor: first.next_bucket,
            ..wrong
        };
        assert_eq!(
            fold_state(first, current, &terms, archive_view, 21),
            Err(ResolutionWorkError::Expired)
        );
        assert_eq!(
            abort_state(first, &terms, 20, false),
            Err(ResolutionWorkError::AbortForbidden)
        );
        assert_eq!(
            abort_state(first, &terms, 21, false).unwrap().reason,
            AbortReasonV1::ExpiredIncomplete
        );

        let (_, alternate_spec_account, alternate_archive, alternate_window) =
            complete_archive([2, 5, 8, 13]);
        let alternate_view = verified_archive(
            &alternate_spec_account,
            &alternate_archive,
            alternate_window,
            ARCHIVE_KEY,
        );
        assert_eq!(
            fold_state(first, current, &terms, alternate_view, 12),
            Err(ResolutionWorkError::BindingMismatch)
        );

        let mut underfunded = begin;
        underfunded.deposited = 100;
        assert_eq!(
            begin_state(underfunded, &terms, archive_view),
            Err(ResolutionWorkError::Codec(
                ResolutionWorkCodecError::Underfunded
            ))
        );
        let unstarted = begin_state(begin, &terms, archive_view).unwrap();
        let mut mutable_basis_identity = unstarted;
        mutable_basis_identity.basis_spec_digest[0] ^= 1;
        assert_eq!(
            abort_state(mutable_basis_identity, &terms, 10, true),
            Err(ResolutionWorkError::BindingMismatch)
        );
        assert_eq!(
            abort_state(unstarted, &terms, 10, false),
            Err(ResolutionWorkError::AbortForbidden)
        );
        assert_eq!(
            abort_state(unstarted, &terms, 10, true).unwrap().reason,
            AbortReasonV1::Unstarted
        );
    }

    fn deployment_bytes() -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(b"MOCKDEP1");
        bytes[8..].copy_from_slice(&DEPLOYMENT_GENERATION.to_le_bytes());
        bytes
    }

    fn record(bucket: u64, sequence: u64, publish_slot: u64, price: u128) -> [u8; RECORD_BYTES] {
        let mut bytes = [0; RECORD_BYTES];
        bytes[..4].copy_from_slice(b"SRC1");
        bytes[4..12].copy_from_slice(&DEPLOYMENT_GENERATION.to_le_bytes());
        bytes[12..20].copy_from_slice(&sequence.to_le_bytes());
        bytes[20..28].copy_from_slice(&publish_slot.to_le_bytes());
        bytes[28..36].copy_from_slice(&(bucket * 60).to_le_bytes());
        bytes[36..44].copy_from_slice(&bucket.to_le_bytes());
        bytes[44..60].copy_from_slice(&price.to_le_bytes());
        bytes[60..76].copy_from_slice(&0_u128.to_le_bytes());
        bytes[76] = 1;
        bytes
    }

    fn u64_at(bytes: &[u8], offset: usize) -> u64 {
        let mut value = [0; 8];
        value.copy_from_slice(&bytes[offset..offset + 8]);
        u64::from_le_bytes(value)
    }

    fn u128_at(bytes: &[u8], offset: usize) -> u128 {
        let mut value = [0; 16];
        value.copy_from_slice(&bytes[offset..offset + 16]);
        u128::from_le_bytes(value)
    }
}
