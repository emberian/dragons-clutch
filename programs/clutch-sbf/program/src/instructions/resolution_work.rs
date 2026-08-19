//! Resumable occupation resolution.
//!
//! The allocation-free state transitions in this module stage complete images
//! before the account plane writes bytes or moves lamports. In
//! particular, Fold accepts a lifetime-bound verified archive view and never
//! accepts record bytes, proofs, points, masses, or vectors from instruction
//! data.
//!
use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::native_window::{
    self, NativeWindowError, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
    STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
};
use crate::source_archive::{
    ArchiveAccountViewV1, SourceArchiveError, SourceSpecAccountViewV1, VerifiedSealedArchiveViewV1,
    WindowDomain, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES, SOURCE_ARCHIVE_MAX_RECORDS_V1,
    SOURCE_SPEC_ACCOUNT_V1_BYTES,
};
use crate::{seeds, source_archive as archive};
use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS};
use clutch_bspline_accumulator::{
    BasisDomain, Error as AccumulatorError, FinalizationMode, SequentialSummaryBuilder, Summary,
};
use clutch_solana_layout::resolution_work::{
    AbortResolutionWorkV1, BeginResolutionWorkV1, FinalizeResolutionWorkV1, FoldResolutionWorkV1,
    ResolutionWorkAccountV1, ResolutionWorkCodecError, ResolutionWorkCostScheduleV1,
    ResolutionWorkFundingV1, ABORT_RESOLUTION_WORK_BYTES, BASIS_EVALUATOR_VERSION_V1,
    BASIS_SPEC_BYTES_V1, FINALIZATION_EXACT_ONLY, FINALIZATION_LARGEST_REMAINDER_V1,
    MAX_FOLD_RECORDS_V1, OCCUPATION_RESOLUTION_VERSION_V4, OCCUPATION_SUMMARY_VERSION_V1,
    RESOLUTION_WORK_ACCOUNT_BYTES, RESOLUTION_WORK_COST_VERSION_V1,
    RESOLUTION_WORK_MAX_LIFETIME_SLOTS_V1, WORK_STATUS_ACTIVE,
};
use clutch_solana_layout::{
    account_len,
    occupation_resolution::{
        OccupationResolutionAccount, OCCUPATION_RESOLUTION_LEN,
        RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION,
    },
    CodecError, Hash32, Intent, PayoutVectorBytes, TermsAccount, PAYOUT_INDEX_UNRESOLVED,
};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_sdk_ids::incinerator;

use super::artifact::read_clock_slot;
use super::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, SYSTEM_PROGRAM_ID,
};

const BASIS_MAGIC_V1: [u8; 8] = *b"DCBASV01";
const BASIS_SCHEMA_VERSION_V1: u16 = 1;
const BASIS_SEMANTIC_NATIVE_BSPLINE: u8 = 1;
const BASIS_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/basis-spec/v1";
const COST_DIGEST_DOMAIN_V1: &[u8] = b"DC_RESOLUTION_COST_SCHEDULE_V1";
const WORK_ID_DOMAIN_V1: &[u8] = b"DC_RESOLUTION_WORK_ID_V1";

/// Canonical protocol-neutral destination. Lamports credited here are burned
/// by the runtime; no program instruction can recover or redirect them.
pub const RESOLUTION_WORK_NEUTRAL_SINK_V1: Pubkey = incinerator::ID;

/// Minimum number of slots in which an active Work may accept new folds.
pub const RESOLUTION_WORK_MIN_LIFETIME_SLOTS_V1: u64 = 8;
/// V1 network-policy base fee cap (two signatures at 5,000 lamports each).
pub const RESOLUTION_WORK_BASE_FEE_CAP_V1: u64 = 10_000;
/// V1 priority-price cap in micro-lamports per requested compute unit.
pub const RESOLUTION_WORK_MICROLAMPORTS_PER_CU_CAP_V1: u64 = 1_000_000;
/// V1 keeper surplus above the capped external transaction quote.
pub const RESOLUTION_WORK_KEEPER_TIP_V1: u64 = 100_000;
/// Selected Begin/Fold compute-unit envelope.
pub const RESOLUTION_WORK_FOLD_CU_LIMIT_V1: u32 = 1_050_000;
/// Selected Finalize compute-unit envelope.
pub const RESOLUTION_WORK_FINALIZE_CU_LIMIT_V1: u32 = 1_400_000;
/// Selected Abort compute-unit envelope.
pub const RESOLUTION_WORK_ABORT_CU_LIMIT_V1: u32 = 750_000;
/// Frozen V1 reward for one successful Fold call. The measured Fold(1..=4)
/// rows all select a 1,050,000-CU policy envelope; the reward adds the frozen
/// 10,000-lamport base-fee cap and 100,000-lamport keeper tip.
pub const RESOLUTION_WORK_FOLD_BASE_REWARD_V1: u64 = 1_160_000;
/// V1 pays per admitted Fold call, not per record within that bounded call.
pub const RESOLUTION_WORK_FOLD_RECORD_REWARD_V1: u64 = 0;
/// Frozen V1 finalizer reward at the maximum admitted 1,400,000-CU envelope,
/// plus the base-fee cap and keeper tip. The measured Finalize must separately
/// remain at or below the 1,120,000-CU 25%-headroom threshold.
pub const RESOLUTION_WORK_FINALIZE_REWARD_V1: u64 = 1_510_000;
/// Frozen V1 expired/permitted Abort reward: 750,000-CU selected envelope plus
/// the base-fee cap and keeper tip.
pub const RESOLUTION_WORK_ABORT_REWARD_V1: u64 = 860_000;

/// Begin roles: payer, Market, Terms, Resolution, SourceSpec, SourceArchive,
/// Work, Reserve, System program, Rent, Clock.
pub const BEGIN_ACCOUNT_COUNT: usize = 11;
/// Fold roles: worker, Market, Terms, SourceSpec, SourceArchive, Work, Reserve,
/// Clock.
pub const FOLD_ACCOUNT_COUNT: usize = 8;
/// Abort roles: caller, frozen payer, Market, Terms, Work, Reserve, canonical
/// incinerator, Clock.
pub const ABORT_ACCOUNT_COUNT: usize = 8;
/// Finalize fixed roles before the dynamic outcome-mint suffix.
pub const FINALIZE_FIXED_ACCOUNT_COUNT: usize = 15;

const IX_ACTOR: usize = 0;
const IX_BEGIN_MARKET: usize = 1;
const IX_BEGIN_TERMS: usize = 2;
const IX_BEGIN_RESOLUTION: usize = 3;
const IX_BEGIN_SOURCE_SPEC: usize = 4;
const IX_BEGIN_SOURCE_ARCHIVE: usize = 5;
const IX_BEGIN_WORK: usize = 6;
const IX_BEGIN_RESERVE: usize = 7;
const IX_BEGIN_SYSTEM: usize = 8;
const IX_BEGIN_RENT: usize = 9;
const IX_BEGIN_CLOCK: usize = 10;

const IX_FOLD_MARKET: usize = 1;
const IX_FOLD_TERMS: usize = 2;
const IX_FOLD_SOURCE_SPEC: usize = 3;
const IX_FOLD_SOURCE_ARCHIVE: usize = 4;
const IX_FOLD_WORK: usize = 5;
const IX_FOLD_RESERVE: usize = 6;
const IX_FOLD_CLOCK: usize = 7;

const IX_ABORT_PAYER: usize = 1;
const IX_ABORT_MARKET: usize = 2;
const IX_ABORT_TERMS: usize = 3;
const IX_ABORT_WORK: usize = 4;
const IX_ABORT_RESERVE: usize = 5;
const IX_ABORT_INCINERATOR: usize = 6;
const IX_ABORT_CLOCK: usize = 7;

// Finalize keeps the monolithic v4 prefix at 0..10 and its outcome mints at
// 10..10+n. Payer/Work/Reserve/Incinerator/Clock follow that dynamic prefix.
const FINALIZE_MONOLITHIC_PREFIX: usize = 10;

const BEGIN_STATE_ROLES: [StateRole; 5] = [
    StateRole::read_only(IX_BEGIN_MARKET, account_len::MARKET),
    StateRole::read_only(IX_BEGIN_TERMS, account_len::TERMS),
    StateRole::read_only(IX_BEGIN_RESOLUTION, OCCUPATION_RESOLUTION_LEN),
    StateRole::read_only(IX_BEGIN_SOURCE_SPEC, SOURCE_SPEC_ACCOUNT_V1_BYTES),
    StateRole::read_only(IX_BEGIN_SOURCE_ARCHIVE, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES),
];

const FOLD_STATE_ROLES: [StateRole; 6] = [
    StateRole::read_only(IX_FOLD_MARKET, account_len::MARKET),
    StateRole::read_only(IX_FOLD_TERMS, account_len::TERMS),
    StateRole::read_only(IX_FOLD_SOURCE_SPEC, SOURCE_SPEC_ACCOUNT_V1_BYTES),
    StateRole::read_only(IX_FOLD_SOURCE_ARCHIVE, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES),
    StateRole::writable(IX_FOLD_WORK, RESOLUTION_WORK_ACCOUNT_BYTES),
    StateRole::writable(IX_FOLD_RESERVE, 0),
];

const ABORT_STATE_ROLES: [StateRole; 4] = [
    StateRole::read_only(IX_ABORT_MARKET, account_len::MARKET),
    StateRole::read_only(IX_ABORT_TERMS, account_len::TERMS),
    StateRole::writable(IX_ABORT_WORK, RESOLUTION_WORK_ACCOUNT_BYTES),
    StateRole::writable(IX_ABORT_RESERVE, 0),
];

/// Result of one resolution-work semantic transition.
pub type Result<T> = core::result::Result<T, ResolutionWorkError>;

/// Typed refusal before the account plane assigns stable numeric projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionWorkError {
    /// The Work account or instruction layout refused state.
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

/// Already-authenticated immutable bindings supplied by the Begin account plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginBindingsV1 {
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
    /// Combined Work/Reserve balance observed before the exact payer transfers.
    pub initial_donation_lamports: u64,
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

/// Stack-bounded semantic projection of one fully validated immutable Terms.
///
/// The live account plane constructs this in a frame that owns the full
/// 1,656-byte Terms decode, then returns only the fields needed beside a
/// 1,296-byte Work value. The persisted basis artifact is reconstructed from
/// the validated domain and independently hashed at every transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkTermsV1 {
    terms_digest: [u8; 32],
    feed: [u8; 32],
    domain: BasisDomain,
    window: WindowDomain,
    finalization_mode: u8,
    statistic: u16,
    terms_bump: u8,
}

/// Optimistic Fold guards decoded from the fixed 107-byte payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldGuardsV1 {
    /// Expected Work identity.
    pub work_commitment: [u8; 32],
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
    /// Another canonical path already resolved the same Market; Work can no
    /// longer influence payout authority and is safe to reap.
    MarketAlreadyResolved,
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

/// Derive the one live V1 schedule from authenticated runtime rent and
/// compile-time economic constants.
///
/// Every protocol charge is zero because V1 has no authenticated fee sink.
/// Rewards are positive, bounded, and can leave only the program-owned Reserve.
pub fn release_cost_schedule_v1(rent: &RentParameters) -> Outcome<ResolutionWorkCostScheduleV1> {
    let work_rent = rent.minimum_balance(RESOLUTION_WORK_ACCOUNT_BYTES)?;
    let reserve_rent = rent.minimum_balance(0)?;
    let rent_reserve = work_rent
        .checked_add(reserve_rent)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let schedule = ResolutionWorkCostScheduleV1 {
        version: RESOLUTION_WORK_COST_VERSION_V1,
        work_state_bytes: RESOLUTION_WORK_ACCOUNT_BYTES as u32,
        rent_reserve,
        minimum_lifetime_slots: RESOLUTION_WORK_MIN_LIFETIME_SLOTS_V1,
        begin_charge: 0,
        fold_base_charge: 0,
        fold_per_record_charge: 0,
        fold_base_reward: RESOLUTION_WORK_FOLD_BASE_REWARD_V1,
        fold_per_record_reward: RESOLUTION_WORK_FOLD_RECORD_REWARD_V1,
        finalize_charge: 0,
        finalize_reward: RESOLUTION_WORK_FINALIZE_REWARD_V1,
        abort_charge: 0,
        abort_reward: RESOLUTION_WORK_ABORT_REWARD_V1,
    };
    schedule
        .validate()
        .map_err(ResolutionWorkError::from)
        .map_err(Refusal::from)?;
    Ok(schedule)
}

/// Digest the exact live schedule for construction clients and Begin checks.
pub fn release_cost_schedule_digest_v1(costs: ResolutionWorkCostScheduleV1) -> [u8; 32] {
    cost_schedule_digest(costs)
}

#[inline(never)]
fn project_work_terms(terms: &TermsAccount) -> Result<WorkTermsV1> {
    let domain = native_window::occupation_domain(terms)?;
    let window = native_window::occupation_window(terms)?;
    Ok(WorkTermsV1 {
        terms_digest: terms.terms.bytes(),
        feed: terms.feed.bytes(),
        domain,
        window,
        finalization_mode: finalization_from_terms(terms)?,
        statistic: terms.statistic_id,
        terms_bump: terms.stored_bump,
    })
}

/// Full Terms authentication isolated from every frame that owns Work.
#[inline(never)]
fn authenticate_work_terms(
    program_id: &Pubkey,
    account: &AccountInfo,
    market: accounts::MarketFacts,
) -> Outcome<WorkTermsV1> {
    let data = account.data.borrow();
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_into(&data, &mut terms)?;
    require(
        market.terms == terms.terms
            && market.realm == terms.realm
            && market.profile == terms.profile
            && market.feed == terms.feed
            && market.outcome_count == terms.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let terms_pda = seeds::terms_pda(program_id, &terms.realm.bytes(), &terms.terms.bytes());
    expect_pda(account.key, terms_pda, Some(terms.stored_bump))?;
    project_work_terms(&terms).map_err(Into::into)
}

/// Route one already-decoded ResolutionWork action.
#[inline(never)]
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    require(request.sequence == 0, ClutchError::Replay)?;
    match request.action {
        Action::Layout(Intent::BeginResolutionWork(intent)) => begin(program_id, accounts, intent),
        Action::Layout(Intent::FoldResolutionWork(intent)) => fold(program_id, accounts, intent),
        Action::Layout(Intent::FinalizeResolutionWork(intent)) => {
            finalize(program_id, accounts, intent)
        }
        Action::Layout(Intent::AbortResolutionWork(intent)) => abort(program_id, accounts, intent),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

#[inline(never)]
fn begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: BeginResolutionWorkV1,
) -> Outcome<()> {
    require_count(accounts, BEGIN_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require(accounts[IX_ACTOR].is_writable, ClutchError::NotWritable)?;
    require(
        !accounts[IX_ACTOR].executable,
        ClutchError::ExecutableAccount,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &BEGIN_STATE_ROLES)?;
    require_system_program(&accounts[IX_BEGIN_SYSTEM])?;
    let rent = read_rent(&accounts[IX_BEGIN_RENT])?;
    let slot = read_clock_slot(&accounts[IX_BEGIN_CLOCK])?;

    let market = accounts::read_market(&accounts[IX_BEGIN_MARKET].data.borrow())?;
    require(market.lifecycle == 0, ClutchError::NotActive)?;
    let market_bytes = market.market.bytes();
    let market_pda = seeds::market_pda(program_id, &market.realm.bytes(), &market_bytes);
    expect_pda(
        accounts[IX_BEGIN_MARKET].key,
        market_pda,
        Some(market.stored_bump),
    )?;
    let resolution_pda = seeds::resolution_pda(program_id, &market_bytes);
    expect_pda(accounts[IX_BEGIN_RESOLUTION].key, resolution_pda, None)?;

    let work_pda = seeds::resolution_work_pda(program_id, &market_bytes);
    let reserve_pda =
        seeds::resolution_reserve_pda(program_id, &market_bytes, &work_pda.0.to_bytes());
    expect_pda(accounts[IX_BEGIN_WORK].key, work_pda, None)?;
    expect_pda(accounts[IX_BEGIN_RESERVE].key, reserve_pda, None)?;
    require_creatable(&accounts[IX_BEGIN_WORK])?;
    require_reserve_available(program_id, &accounts[IX_BEGIN_RESERVE])?;
    let initial_donation_lamports = accounts[IX_BEGIN_WORK]
        .lamports()
        .checked_add(accounts[IX_BEGIN_RESERVE].lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    let costs = release_cost_schedule_v1(&rent)?;
    let cost_digest = cost_schedule_digest(costs);
    require(
        intent.cost_schedule_digest == cost_digest,
        ClutchError::MismatchedState,
    )?;
    let work_rent = rent.minimum_balance(RESOLUTION_WORK_ACCOUNT_BYTES)?;
    let reserve_rent = rent.minimum_balance(0)?;
    require(
        costs.rent_reserve
            == work_rent
                .checked_add(reserve_rent)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    create_fully_funded_work(
        program_id,
        &accounts[IX_BEGIN_WORK],
        &accounts[IX_ACTOR],
        &accounts[IX_BEGIN_SYSTEM],
        work_rent,
        &[seeds::SEED_RESOLUTION_WORK, &market_bytes, &[work_pda.1]],
    )?;
    let reserve_funding = intent
        .declared_deposit
        .checked_sub(work_rent)
        .ok_or(ResolutionWorkError::Underfunded)?;
    require(
        reserve_funding >= reserve_rent,
        ClutchError::AccountCreationFailed,
    )?;
    create_zero_data_reserve(
        program_id,
        &accounts[IX_ACTOR],
        &accounts[IX_BEGIN_RESERVE],
        &accounts[IX_BEGIN_SYSTEM],
        reserve_funding,
        &[
            seeds::SEED_RESOLUTION_RESERVE,
            &market_bytes,
            &work_pda.0.to_bytes(),
            &[reserve_pda.1],
        ],
    )?;
    initialize_begin_work(
        program_id,
        accounts,
        intent,
        market,
        costs,
        cost_digest,
        slot,
        resolution_pda,
        work_pda,
        reserve_pda,
        initial_donation_lamports,
    )
}

/// Perform the large immutable-Terms/archive derivation only after the CPI
/// phase has released its compact locals. Any refusal rolls the entire Solana
/// transaction, including both just-created PDA accounts, back atomically.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn initialize_begin_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: BeginResolutionWorkV1,
    market: accounts::MarketFacts,
    costs: ResolutionWorkCostScheduleV1,
    cost_digest: [u8; 32],
    slot: u64,
    resolution_pda: (Pubkey, u8),
    work_pda: (Pubkey, u8),
    reserve_pda: (Pubkey, u8),
    initial_donation_lamports: u64,
) -> Outcome<()> {
    let terms = authenticate_work_terms(program_id, &accounts[IX_BEGIN_TERMS], market)?;
    validate_begin_resolution(
        &accounts[IX_BEGIN_RESOLUTION],
        market,
        terms,
        resolution_pda.1,
    )?;

    let expected_window_id = archive::canonical_window_id(terms.window);
    let source_spec_pda = seeds::source_spec_pda(program_id, &terms.feed);
    let source_archive_pda =
        seeds::source_archive_pda(program_id, &terms.feed, &expected_window_id.bytes());
    expect_pda(accounts[IX_BEGIN_SOURCE_SPEC].key, source_spec_pda, None)?;
    expect_pda(
        accounts[IX_BEGIN_SOURCE_ARCHIVE].key,
        source_archive_pda,
        None,
    )?;
    let source_spec_data = accounts[IX_BEGIN_SOURCE_SPEC].data.borrow();
    let verified_spec = archive::verify_source_spec_account(
        program_id.to_bytes(),
        source_spec_pda.0.to_bytes(),
        SourceSpecAccountViewV1::new(
            accounts[IX_BEGIN_SOURCE_SPEC].key.to_bytes(),
            accounts[IX_BEGIN_SOURCE_SPEC].owner.to_bytes(),
            accounts[IX_BEGIN_SOURCE_SPEC].executable,
            &source_spec_data,
        ),
    )
    .map_err(ResolutionWorkError::from)?;
    require(
        verified_spec.stored_bump() == source_spec_pda.1,
        ClutchError::WrongBump,
    )?;
    let source_archive_data = accounts[IX_BEGIN_SOURCE_ARCHIVE].data.borrow();
    let verified_archive = archive::verify_recorded_sealed_archive_view(
        program_id.to_bytes(),
        source_archive_pda.0.to_bytes(),
        ArchiveAccountViewV1::new(
            accounts[IX_BEGIN_SOURCE_ARCHIVE].key.to_bytes(),
            accounts[IX_BEGIN_SOURCE_ARCHIVE].owner.to_bytes(),
            accounts[IX_BEGIN_SOURCE_ARCHIVE].executable,
            &source_archive_data,
        ),
        verified_spec,
        terms.window,
    )
    .map_err(ResolutionWorkError::from)?;
    write_begin_work(
        &accounts[IX_BEGIN_WORK],
        BeginBindingsV1 {
            payer: accounts[IX_ACTOR].key.to_bytes(),
            prepaid_reserve: reserve_pda.0.to_bytes(),
            work_nonce: intent.work_nonce,
            market: market.market.bytes(),
            resolution_target: resolution_pda.0.to_bytes(),
            program_owner: program_id.to_bytes(),
            basis_spec_digest: basis_artifact_digest(&encode_basis_artifact(terms.domain.spec())),
            cost_schedule_digest: cost_digest,
            costs,
            deposited: intent.declared_deposit,
            initial_donation_lamports,
            opened_slot: slot,
            expires_slot: intent.expires_slot,
            finalization_mode: intent.finalization_mode,
            work_bump: work_pda.1,
            reserve_bump: reserve_pda.1,
        },
        terms,
        verified_archive,
    )?;
    drop(source_archive_data);
    drop(source_spec_data);
    Ok(())
}

#[inline(never)]
fn validate_begin_resolution(
    account: &AccountInfo,
    market: accounts::MarketFacts,
    terms: WorkTermsV1,
    expected_bump: u8,
) -> Outcome<()> {
    let data = account.data.borrow();
    let resolution = OccupationResolutionAccount::decode(&data)?;
    require(
        resolution.market == market.market
            && resolution.terms.bytes() == terms.terms_digest
            && resolution.feed.bytes() == terms.feed
            && !resolution.is_resolved(),
        ClutchError::NotActive,
    )?;
    require(
        resolution.stored_bump == expected_bump,
        ClutchError::WrongBump,
    )
}

#[inline(never)]
fn write_begin_work(
    account: &AccountInfo,
    bindings: BeginBindingsV1,
    terms: WorkTermsV1,
    archive: VerifiedSealedArchiveViewV1<'_>,
) -> Outcome<()> {
    let work = begin_state_projected(bindings, terms, archive)?;
    work.encode(
        &mut account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    Ok(())
}

#[inline(never)]
fn create_fully_funded_work<'a>(
    program_id: &Pubkey,
    target: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_principal: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    let before = target.lamports();
    let expected = before
        .checked_add(rent_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(rent_principal),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(RESOLUTION_WORK_ACCOUNT_BYTES),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == expected
            && target.owner == program_id
            && target.data_len() == RESOLUTION_WORK_ACCOUNT_BYTES,
        ClutchError::AccountCreationFailed,
    )
}

fn require_reserve_available(program_id: &Pubkey, reserve: &AccountInfo) -> Outcome<()> {
    require(
        reserve.data_len() == 0
            && (*reserve.owner == SYSTEM_PROGRAM_ID || reserve.owner == program_id),
        ClutchError::AlreadyInitialized,
    )
}

#[inline(never)]
fn create_zero_data_reserve<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    reserve: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_reserve_available(program_id, reserve)?;
    let before = reserve.lamports();
    let expected = before
        .checked_add(lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(lamports),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*reserve.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), reserve.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        reserve.lamports() == expected,
        ClutchError::AccountCreationFailed,
    )?;
    if *reserve.owner == SYSTEM_PROGRAM_ID {
        let assign = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &assign_data(program_id),
            vec![AccountMeta::new(*reserve.key, true)],
        );
        invoke_signed(
            &assign,
            &[reserve.clone(), system_program.clone()],
            &[signer_seeds],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    }
    require(
        reserve.owner == program_id && reserve.data_len() == 0 && reserve.lamports() == expected,
        ClutchError::AccountCreationFailed,
    )
}

fn validate_release_cost_shape(costs: ResolutionWorkCostScheduleV1) -> Outcome<()> {
    require(
        costs.version == RESOLUTION_WORK_COST_VERSION_V1
            && costs.work_state_bytes as usize == RESOLUTION_WORK_ACCOUNT_BYTES
            && costs.rent_reserve > 0
            && costs.minimum_lifetime_slots == RESOLUTION_WORK_MIN_LIFETIME_SLOTS_V1
            && costs.begin_charge == 0
            && costs.fold_base_charge == 0
            && costs.fold_per_record_charge == 0
            && costs.fold_base_reward == RESOLUTION_WORK_FOLD_BASE_REWARD_V1
            && costs.fold_per_record_reward == RESOLUTION_WORK_FOLD_RECORD_REWARD_V1
            && costs.finalize_charge == 0
            && costs.finalize_reward == RESOLUTION_WORK_FINALIZE_REWARD_V1
            && costs.abort_charge == 0
            && costs.abort_reward == RESOLUTION_WORK_ABORT_REWARD_V1,
        ClutchError::MismatchedState,
    )
}

fn reconcile_active_funding(
    work_account: &AccountInfo,
    reserve: &AccountInfo,
    funding: &mut ResolutionWorkFundingV1,
) -> Outcome<()> {
    let actual = work_account
        .lamports()
        .checked_add(reserve.lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let required = funding
        .rent_locked
        .checked_add(funding.prepaid_remaining)
        .and_then(|value| value.checked_add(funding.charges_paid))
        .and_then(|value| value.checked_add(funding.donation_lamports))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let newly_observed = actual
        .checked_sub(required)
        .ok_or(ResolutionWorkError::Underfunded)?;
    funding.donation_lamports = funding
        .donation_lamports
        .checked_add(newly_observed)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn pay_reward(reserve: &AccountInfo, recipient: &AccountInfo, reward: u64) -> Outcome<()> {
    if reward == 0 {
        return Ok(());
    }
    require(reserve.key != recipient.key, ClutchError::AccountAlias)?;
    require(recipient.is_writable, ClutchError::NotWritable)?;
    require(!recipient.executable, ClutchError::ExecutableAccount)?;
    let reserve_after = reserve
        .lamports()
        .checked_sub(reward)
        .ok_or(ResolutionWorkError::Underfunded)?;
    let recipient_after = recipient
        .lamports()
        .checked_add(reward)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    {
        let mut destination = recipient
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **destination = recipient_after;
    }
    {
        let mut source = reserve
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **source = reserve_after;
    }
    Ok(())
}

#[inline(never)]
fn fold(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: FoldResolutionWorkV1,
) -> Outcome<()> {
    require_count(accounts, FOLD_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require(accounts[IX_ACTOR].is_writable, ClutchError::NotWritable)?;
    require(
        !accounts[IX_ACTOR].executable,
        ClutchError::ExecutableAccount,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &FOLD_STATE_ROLES)?;
    let slot = read_clock_slot(&accounts[IX_FOLD_CLOCK])?;

    let market = accounts::read_market(&accounts[IX_FOLD_MARKET].data.borrow())?;
    require(market.lifecycle == 0, ClutchError::NotActive)?;
    let market_bytes = market.market.bytes();
    let market_pda = seeds::market_pda(program_id, &market.realm.bytes(), &market_bytes);
    expect_pda(
        accounts[IX_FOLD_MARKET].key,
        market_pda,
        Some(market.stored_bump),
    )?;
    let terms = authenticate_work_terms(program_id, &accounts[IX_FOLD_TERMS], market)?;

    let expected_window_id = archive::canonical_window_id(terms.window);
    let source_spec_pda = seeds::source_spec_pda(program_id, &terms.feed);
    let source_archive_pda =
        seeds::source_archive_pda(program_id, &terms.feed, &expected_window_id.bytes());
    expect_pda(accounts[IX_FOLD_SOURCE_SPEC].key, source_spec_pda, None)?;
    expect_pda(
        accounts[IX_FOLD_SOURCE_ARCHIVE].key,
        source_archive_pda,
        None,
    )?;
    let source_spec_data = accounts[IX_FOLD_SOURCE_SPEC].data.borrow();
    let verified_spec = archive::verify_source_spec_account(
        program_id.to_bytes(),
        source_spec_pda.0.to_bytes(),
        SourceSpecAccountViewV1::new(
            accounts[IX_FOLD_SOURCE_SPEC].key.to_bytes(),
            accounts[IX_FOLD_SOURCE_SPEC].owner.to_bytes(),
            accounts[IX_FOLD_SOURCE_SPEC].executable,
            &source_spec_data,
        ),
    )
    .map_err(ResolutionWorkError::from)?;
    require(
        verified_spec.stored_bump() == source_spec_pda.1,
        ClutchError::WrongBump,
    )?;
    let source_archive_data = accounts[IX_FOLD_SOURCE_ARCHIVE].data.borrow();
    let verified_archive = archive::verify_recorded_sealed_archive_view(
        program_id.to_bytes(),
        source_archive_pda.0.to_bytes(),
        ArchiveAccountViewV1::new(
            accounts[IX_FOLD_SOURCE_ARCHIVE].key.to_bytes(),
            accounts[IX_FOLD_SOURCE_ARCHIVE].owner.to_bytes(),
            accounts[IX_FOLD_SOURCE_ARCHIVE].executable,
            &source_archive_data,
        ),
        verified_spec,
        terms.window,
    )
    .map_err(ResolutionWorkError::from)?;
    commit_fold(
        program_id,
        accounts,
        intent,
        market_bytes,
        terms,
        verified_archive,
        slot,
    )?;
    drop(source_archive_data);
    drop(source_spec_data);
    Ok(())
}

#[inline(never)]
fn commit_fold(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: FoldResolutionWorkV1,
    market: [u8; 32],
    terms: WorkTermsV1,
    archive: VerifiedSealedArchiveViewV1<'_>,
    slot: u64,
) -> Outcome<()> {
    let work_data = accounts[IX_FOLD_WORK].data.borrow();
    let mut work = ResolutionWorkAccountV1::decode(&work_data)?;
    validate_release_cost_shape(work.costs)?;
    let work_pda = seeds::resolution_work_pda(program_id, &market);
    let reserve_pda = seeds::resolution_reserve_pda(program_id, &market, &work_pda.0.to_bytes());
    expect_pda(accounts[IX_FOLD_WORK].key, work_pda, Some(work.stored_bump))?;
    expect_pda(
        accounts[IX_FOLD_RESERVE].key,
        reserve_pda,
        Some(work.reserve_bump),
    )?;
    require(
        work.market == market
            && work.program_owner == program_id.to_bytes()
            && work.prepaid_reserve == reserve_pda.0.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    reconcile_active_funding(
        &accounts[IX_FOLD_WORK],
        &accounts[IX_FOLD_RESERVE],
        &mut work.funding,
    )?;
    let (charge, reward) = fold_state_in_place(
        &mut work,
        FoldGuardsV1 {
            work_commitment: intent.work_commitment,
            archive_account: intent.archive_account,
            archive_commitment: intent.archive_commitment,
            expected_cursor: intent.expected_cursor,
            record_count: intent.record_count,
        },
        terms,
        archive,
        slot,
    )?;
    require(charge == 0, ClutchError::MismatchedState)?;
    drop(work_data);
    let funding = work.funding;
    work.encode(
        &mut accounts[IX_FOLD_WORK]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    pay_reward(&accounts[IX_FOLD_RESERVE], &accounts[IX_ACTOR], reward)?;
    let mut verified_funding = funding;
    reconcile_active_funding(
        &accounts[IX_FOLD_WORK],
        &accounts[IX_FOLD_RESERVE],
        &mut verified_funding,
    )?;
    require(verified_funding == funding, ClutchError::MismatchedState)
}

fn require_distinct_except(
    accounts: &[AccountInfo],
    allowed_left: usize,
    allowed_right: usize,
) -> Outcome<()> {
    let mut left = 0_usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if !((left == allowed_left && right == allowed_right)
                || (left == allowed_right && right == allowed_left))
            {
                require(
                    accounts[left].key != accounts[right].key,
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_neutral_sink(account: &AccountInfo) -> Outcome<()> {
    require(
        account.key == &RESOLUTION_WORK_NEUTRAL_SINK_V1,
        ClutchError::WrongPda,
    )?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerminalLamports {
    payer_credit: u64,
    actor_credit: u64,
    /// Unsolicited surplus retained forever in the canonical zero-data Reserve.
    reserve_retained: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FinalizePrepared {
    resolution: OccupationResolutionAccount,
    transfers: TerminalLamports,
    market: accounts::MarketFacts,
    terms_bump: u8,
    resolution_bump: u8,
}

fn terminal_lamports(
    work_account: &AccountInfo,
    reserve: &AccountInfo,
    payer: &AccountInfo,
    actor: &AccountInfo,
    payer_refund: u64,
    actor_reward: u64,
    reserve_retained: u64,
) -> Outcome<TerminalLamports> {
    let actual = work_account
        .lamports()
        .checked_add(reserve.lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let quoted = payer_refund
        .checked_add(actor_reward)
        .and_then(|value| value.checked_add(reserve_retained))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(actual == quoted, ClutchError::MismatchedState)?;
    let payer_credit = payer_refund;
    if payer.key == actor.key {
        payer
            .lamports()
            .checked_add(payer_credit)
            .and_then(|value| value.checked_add(actor_reward))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    } else {
        payer
            .lamports()
            .checked_add(payer_credit)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        actor
            .lamports()
            .checked_add(actor_reward)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    Ok(TerminalLamports {
        payer_credit,
        actor_credit: actor_reward,
        reserve_retained,
    })
}

fn close_work_and_reserve(
    work_account: &AccountInfo,
    reserve: &AccountInfo,
    neutral_sink: &AccountInfo,
    payer: &AccountInfo,
    actor: &AccountInfo,
    transfer: TerminalLamports,
) -> Outcome<()> {
    require_neutral_sink(neutral_sink)?;
    if payer.key == actor.key {
        let credit = transfer
            .payer_credit
            .checked_add(transfer.actor_credit)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let after = payer
            .lamports()
            .checked_add(credit)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let mut destination = payer
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **destination = after;
    } else {
        let payer_after = payer
            .lamports()
            .checked_add(transfer.payer_credit)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let actor_after = actor
            .lamports()
            .checked_add(transfer.actor_credit)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        {
            let mut destination = payer
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            **destination = payer_after;
        }
        {
            let mut destination = actor
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            **destination = actor_after;
        }
    }
    {
        let after = neutral_sink
            .lamports()
            .checked_add(transfer.reserve_retained)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let mut sink_lamports = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **sink_lamports = after;
    }
    {
        let mut work_lamports = work_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **work_lamports = 0;
    }
    {
        let mut reserve_lamports = reserve
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **reserve_lamports = 0;
    }
    work_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .fill(0);
    Ok(())
}

#[inline(never)]
fn abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: AbortResolutionWorkV1,
) -> Outcome<()> {
    require_count(accounts, ABORT_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require(accounts[IX_ACTOR].is_writable, ClutchError::NotWritable)?;
    require(
        !accounts[IX_ACTOR].executable,
        ClutchError::ExecutableAccount,
    )?;
    require(
        accounts[IX_ABORT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        !accounts[IX_ABORT_PAYER].executable,
        ClutchError::ExecutableAccount,
    )?;
    require_distinct_except(accounts, IX_ACTOR, IX_ABORT_PAYER)?;
    require_neutral_sink(&accounts[IX_ABORT_INCINERATOR])?;
    accounts::validate_state_roles(program_id, accounts, &ABORT_STATE_ROLES)?;
    let slot = read_clock_slot(&accounts[IX_ABORT_CLOCK])?;

    let market = accounts::read_market(&accounts[IX_ABORT_MARKET].data.borrow())?;
    let market_bytes = market.market.bytes();
    let market_pda = seeds::market_pda(program_id, &market.realm.bytes(), &market_bytes);
    expect_pda(
        accounts[IX_ABORT_MARKET].key,
        market_pda,
        Some(market.stored_bump),
    )?;
    let terms = authenticate_work_terms(program_id, &accounts[IX_ABORT_TERMS], market)?;

    let transfers = prepare_abort(
        program_id,
        accounts,
        intent,
        market_bytes,
        market.lifecycle == 1,
        terms,
        slot,
    )?;
    close_work_and_reserve(
        &accounts[IX_ABORT_WORK],
        &accounts[IX_ABORT_RESERVE],
        &accounts[IX_ABORT_INCINERATOR],
        &accounts[IX_ABORT_PAYER],
        &accounts[IX_ACTOR],
        transfers,
    )
}

#[inline(never)]
fn prepare_abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: AbortResolutionWorkV1,
    market: [u8; 32],
    market_is_resolved: bool,
    terms: WorkTermsV1,
    slot: u64,
) -> Outcome<TerminalLamports> {
    let work_data = accounts[IX_ABORT_WORK].data.borrow();
    let mut work = ResolutionWorkAccountV1::decode(&work_data)?;
    validate_release_cost_shape(work.costs)?;
    let work_pda = seeds::resolution_work_pda(program_id, &market);
    let reserve_pda = seeds::resolution_reserve_pda(program_id, &market, &work_pda.0.to_bytes());
    expect_pda(
        accounts[IX_ABORT_WORK].key,
        work_pda,
        Some(work.stored_bump),
    )?;
    expect_pda(
        accounts[IX_ABORT_RESERVE].key,
        reserve_pda,
        Some(work.reserve_bump),
    )?;
    require(
        work.market == market
            && work.program_owner == program_id.to_bytes()
            && work.prepaid_reserve == reserve_pda.0.to_bytes()
            && work.payer == accounts[IX_ABORT_PAYER].key.to_bytes()
            && intent.work_commitment == work.work_commitment
            && intent.expected_cursor == work.next_bucket
            && intent.expected_archive_commitment == work.archive_commitment,
        ClutchError::MismatchedState,
    )?;
    reconcile_active_funding(
        &accounts[IX_ABORT_WORK],
        &accounts[IX_ABORT_RESERVE],
        &mut work.funding,
    )?;
    let transition = abort_state_projected(
        &work,
        terms,
        slot,
        accounts[IX_ACTOR].key == accounts[IX_ABORT_PAYER].key,
        market_is_resolved,
    )?;
    require(transition.charge == 0, ClutchError::MismatchedState)?;
    terminal_lamports(
        &accounts[IX_ABORT_WORK],
        &accounts[IX_ABORT_RESERVE],
        &accounts[IX_ABORT_PAYER],
        &accounts[IX_ACTOR],
        transition.payer_refund,
        transition.reward,
        work.funding
            .donation_lamports
            .checked_add(work.funding.charges_paid)
            .and_then(|value| value.checked_add(transition.charge))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    )
}

#[inline(never)]
fn finalize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: FinalizeResolutionWorkV1,
) -> Outcome<()> {
    require(
        accounts.len() >= FINALIZE_FIXED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_ACTOR])?;
    require(accounts[IX_ACTOR].is_writable, ClutchError::NotWritable)?;
    require(
        !accounts[IX_ACTOR].executable,
        ClutchError::ExecutableAccount,
    )?;

    // The monolithic prefix fixes Market at one and Terms at five.
    let market = accounts::read_market(&accounts[1].data.borrow())?;
    let outcome_count = usize::from(market.outcome_count);
    let extras = FINALIZE_MONOLITHIC_PREFIX
        .checked_add(outcome_count)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(accounts.len() == extras + 5, ClutchError::AccountCount)?;
    let payer_index = extras;
    let work_index = extras + 1;
    let reserve_index = extras + 2;
    let incinerator_index = extras + 3;
    let clock_index = extras + 4;
    require(accounts[payer_index].is_writable, ClutchError::NotWritable)?;
    require(
        !accounts[payer_index].executable,
        ClutchError::ExecutableAccount,
    )?;
    require_distinct_except(accounts, IX_ACTOR, payer_index)?;
    require_neutral_sink(&accounts[incinerator_index])?;
    let slot = read_clock_slot(&accounts[clock_index])?;

    accounts::validate_state_roles(
        program_id,
        accounts,
        &[
            StateRole::writable(work_index, RESOLUTION_WORK_ACCOUNT_BYTES),
            StateRole::writable(reserve_index, 0),
        ],
    )?;
    let prepared = prepare_finalize_evidence(
        program_id,
        accounts,
        intent,
        market,
        payer_index,
        work_index,
        reserve_index,
        slot,
    )?;
    super::observe_resolve::apply_resumable_occupation_candidate(
        program_id,
        &accounts[..extras],
        prepared.resolution,
        prepared.market,
        prepared.terms_bump,
        prepared.resolution_bump,
    )?;
    close_work_and_reserve(
        &accounts[work_index],
        &accounts[reserve_index],
        &accounts[incinerator_index],
        &accounts[payer_index],
        &accounts[IX_ACTOR],
        prepared.transfers,
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn prepare_finalize_evidence(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: FinalizeResolutionWorkV1,
    market: accounts::MarketFacts,
    payer_index: usize,
    work_index: usize,
    reserve_index: usize,
    slot: u64,
) -> Outcome<FinalizePrepared> {
    let terms = authenticate_work_terms(program_id, &accounts[5], market)?;
    let expected_window_id = archive::canonical_window_id(terms.window);
    let source_spec_pda = seeds::source_spec_pda(program_id, &terms.feed);
    let source_archive_pda =
        seeds::source_archive_pda(program_id, &terms.feed, &expected_window_id.bytes());
    expect_pda(accounts[8].key, source_spec_pda, None)?;
    expect_pda(accounts[9].key, source_archive_pda, None)?;
    let source_spec_data = accounts[8].data.borrow();
    let verified_spec = archive::verify_source_spec_account(
        program_id.to_bytes(),
        source_spec_pda.0.to_bytes(),
        SourceSpecAccountViewV1::new(
            accounts[8].key.to_bytes(),
            accounts[8].owner.to_bytes(),
            accounts[8].executable,
            &source_spec_data,
        ),
    )
    .map_err(ResolutionWorkError::from)?;
    require(
        verified_spec.stored_bump() == source_spec_pda.1,
        ClutchError::WrongBump,
    )?;
    let source_archive_data = accounts[9].data.borrow();
    let verified_archive = archive::verify_recorded_sealed_archive_view(
        program_id.to_bytes(),
        source_archive_pda.0.to_bytes(),
        ArchiveAccountViewV1::new(
            accounts[9].key.to_bytes(),
            accounts[9].owner.to_bytes(),
            accounts[9].executable,
            &source_archive_data,
        ),
        verified_spec,
        terms.window,
    )
    .map_err(ResolutionWorkError::from)?;
    prepare_finalize_work(
        program_id,
        accounts,
        intent,
        market,
        terms,
        verified_archive,
        payer_index,
        work_index,
        reserve_index,
        slot,
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn prepare_finalize_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent: FinalizeResolutionWorkV1,
    market: accounts::MarketFacts,
    terms: WorkTermsV1,
    archive: VerifiedSealedArchiveViewV1<'_>,
    payer_index: usize,
    work_index: usize,
    reserve_index: usize,
    slot: u64,
) -> Outcome<FinalizePrepared> {
    let work_data = accounts[work_index].data.borrow();
    let mut work = ResolutionWorkAccountV1::decode(&work_data)?;
    validate_release_cost_shape(work.costs)?;
    let market_bytes = market.market.bytes();
    let work_pda = seeds::resolution_work_pda(program_id, &market_bytes);
    let reserve_pda =
        seeds::resolution_reserve_pda(program_id, &market_bytes, &work_pda.0.to_bytes());
    expect_pda(accounts[work_index].key, work_pda, Some(work.stored_bump))?;
    expect_pda(
        accounts[reserve_index].key,
        reserve_pda,
        Some(work.reserve_bump),
    )?;
    require(
        work.market == market_bytes
            && work.program_owner == program_id.to_bytes()
            && work.prepaid_reserve == reserve_pda.0.to_bytes()
            && work.payer == accounts[payer_index].key.to_bytes()
            && work.resolution_target == accounts[6].key.to_bytes()
            && work.archive_account == accounts[9].key.to_bytes()
            && intent.work_commitment == work.work_commitment,
        ClutchError::MismatchedState,
    )?;
    reconcile_active_funding(
        &accounts[work_index],
        &accounts[reserve_index],
        &mut work.funding,
    )?;

    let resolution_pda = seeds::resolution_pda(program_id, &market_bytes);
    expect_pda(accounts[6].key, resolution_pda, None)?;
    let transition = finalize_state_projected(
        &work,
        terms,
        archive,
        intent.expected_cursor,
        intent.expected_archive_commitment,
        slot,
        resolution_pda.1,
    )?;
    require(transition.charge == 0, ClutchError::MismatchedState)?;
    let transfers = terminal_lamports(
        &accounts[work_index],
        &accounts[reserve_index],
        &accounts[payer_index],
        &accounts[IX_ACTOR],
        transition.payer_refund,
        transition.reward,
        work.funding
            .donation_lamports
            .checked_add(work.funding.charges_paid)
            .and_then(|value| value.checked_add(transition.charge))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    )?;
    Ok(FinalizePrepared {
        resolution: transition.resolution,
        transfers,
        market,
        terms_bump: terms.terms_bump,
        resolution_bump: resolution_pda.1,
    })
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
    begin_state_projected(bindings, project_work_terms(terms)?, archive)
}

#[inline(never)]
fn begin_state_projected(
    bindings: BeginBindingsV1,
    terms: WorkTermsV1,
    archive: VerifiedSealedArchiveViewV1<'_>,
) -> Result<ResolutionWorkAccountV1> {
    let lifetime = bindings
        .expires_slot
        .checked_sub(bindings.opened_slot)
        .ok_or(ResolutionWorkError::InvalidSlot)?;
    if !(RESOLUTION_WORK_MIN_LIFETIME_SLOTS_V1..=RESOLUTION_WORK_MAX_LIFETIME_SLOTS_V1)
        .contains(&lifetime)
    {
        return Err(ResolutionWorkError::InvalidSlot);
    }
    let receipt = archive.receipt();
    let span = receipt
        .end_bucket_exclusive()
        .checked_sub(receipt.start_bucket())
        .ok_or(ResolutionWorkError::BindingMismatch)?;
    let record_count = u8::try_from(span).map_err(|_| ResolutionWorkError::InvalidChunk)?;
    if record_count == 0 || usize::from(record_count) > SOURCE_ARCHIVE_MAX_RECORDS_V1 {
        return Err(ResolutionWorkError::InvalidChunk);
    }
    if receipt.feed().bytes() != terms.feed
        || receipt.start_bucket() != terms.window.start_bucket()
        || receipt.end_bucket_exclusive() != terms.window.end_bucket_exclusive()
        || receipt.repair_generation() != terms.window.generation()
    {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    if bindings.finalization_mode != terms.finalization_mode {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    let basis_spec_artifact = encode_basis_artifact(terms.domain.spec());
    if bindings.basis_spec_digest != basis_artifact_digest(&basis_spec_artifact)
        || bindings.cost_schedule_digest != cost_schedule_digest(bindings.costs)
    {
        return Err(ResolutionWorkError::BindingMismatch);
    }
    let work_commitment = compute_work_commitment(
        &bindings,
        terms.terms_digest,
        terms.feed,
        receipt.archive_key(),
        receipt.page_commitment().bytes(),
        receipt.window().bytes(),
        receipt.repair_generation(),
        receipt.start_bucket(),
        receipt.end_bucket_exclusive(),
    );
    let prepaid_remaining = bindings
        .deposited
        .checked_sub(bindings.costs.rent_reserve)
        .and_then(|value| value.checked_sub(bindings.costs.begin_charge))
        .ok_or(ResolutionWorkError::Underfunded)?;
    let value = ResolutionWorkAccountV1 {
        work_commitment,
        payer: bindings.payer,
        prepaid_reserve: bindings.prepaid_reserve,
        work_nonce: bindings.work_nonce,
        market: bindings.market,
        terms_digest: terms.terms_digest,
        resolution_target: bindings.resolution_target,
        program_owner: bindings.program_owner,
        archive_account: receipt.archive_key(),
        basis_spec_digest: bindings.basis_spec_digest,
        source_spec_digest: terms.feed,
        archive_commitment: receipt.page_commitment().bytes(),
        archive_domain_digest: receipt.window().bytes(),
        grid_identity: terms.domain.grid_identity(),
        basis_spec_artifact,
        archive_generation: receipt.repair_generation(),
        bucket_duration: terms.domain.bucket_duration(),
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
        denominator: terms.domain.spec().denominator,
        masses: [0; 16],
        costs: bindings.costs,
        cost_schedule_digest: bindings.cost_schedule_digest,
        funding: ResolutionWorkFundingV1 {
            donation_lamports: bindings.initial_donation_lamports,
            rent_locked: bindings.costs.rent_reserve,
            prepaid_remaining,
            charges_paid: bindings.costs.begin_charge,
            rewards_paid: 0,
        },
        status: WORK_STATUS_ACTIVE,
        finalization_mode: bindings.finalization_mode,
        outcome_count: terms.domain.spec().outcome_count,
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
    mut work: ResolutionWorkAccountV1,
    guards: FoldGuardsV1,
    terms: &TermsAccount,
    archive: VerifiedSealedArchiveViewV1<'_>,
    current_slot: u64,
) -> Result<FoldTransitionV1> {
    let projected = project_work_terms(terms)?;
    let (charge, reward) =
        fold_state_in_place(&mut work, guards, projected, archive, current_slot)?;
    Ok(FoldTransitionV1 {
        next: work,
        charge,
        reward,
    })
}

/// Stack-bounded live Fold seam. The caller owns exactly one decoded Work and
/// may encode it back into the same account after this function succeeds.
#[inline(never)]
fn fold_state_in_place(
    work: &mut ResolutionWorkAccountV1,
    guards: FoldGuardsV1,
    terms: WorkTermsV1,
    archive: VerifiedSealedArchiveViewV1<'_>,
    current_slot: u64,
) -> Result<(u64, u64)> {
    work.validate()?;
    validate_static_bindings_projected(work, terms, archive)?;
    if guards.work_commitment != work.work_commitment
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

    let restored = Summary::from_canonical_parts(
        terms.domain,
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
    debit(&mut work.funding, charge, reward)?;
    work.next_bucket = chunk_end;
    work.fold_count = work
        .fold_count
        .checked_add(1)
        .ok_or(ResolutionWorkError::ArithmeticOverflow)?;
    work.last_progress_slot = current_slot;
    work.sample_count = summary.sample_count();
    work.coverage_count = summary.coverage_count();
    work.masses = summary.masses();
    if chunk_end == work.end_bucket_exclusive {
        work.completion_slot = current_slot;
    }
    work.validate()?;
    Ok((charge, reward))
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
    finalize_state_projected(
        &work,
        project_work_terms(terms)?,
        archive,
        expected_cursor,
        expected_archive_commitment,
        current_slot,
        resolution_bump,
    )
}

#[inline(never)]
fn finalize_state_projected(
    work: &ResolutionWorkAccountV1,
    terms: WorkTermsV1,
    archive: VerifiedSealedArchiveViewV1<'_>,
    expected_cursor: u64,
    expected_archive_commitment: [u8; 32],
    current_slot: u64,
    resolution_bump: u8,
) -> Result<FinalizeTransitionV1> {
    work.validate()?;
    validate_static_bindings_projected(work, terms, archive)?;
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
    let summary = Summary::from_canonical_parts(
        terms.domain,
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
        // Monolithic v4 defines this logical evidence slot as zero. Work may
        // take many bank slots, but Finalize must write byte-identical evidence.
        resolved_slot: 0,
        mode: RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        outcome_count: work.outcome_count,
        resolved_value: 0,
        vector,
        archive_commitment: Hash32::from_bytes(work.archive_commitment),
        statistic: terms.statistic,
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
    market_is_resolved: bool,
) -> Result<AbortTransitionV1> {
    abort_state_projected(
        &work,
        project_work_terms(terms)?,
        current_slot,
        caller_is_payer,
        market_is_resolved,
    )
}

#[inline(never)]
fn abort_state_projected(
    work: &ResolutionWorkAccountV1,
    terms: WorkTermsV1,
    current_slot: u64,
    caller_is_payer: bool,
    market_is_resolved: bool,
) -> Result<AbortTransitionV1> {
    work.validate()?;
    validate_work_commitment_projected(work, terms)?;
    if current_slot < work.last_progress_slot {
        return Err(ResolutionWorkError::InvalidSlot);
    }
    let reason = if market_is_resolved {
        AbortReasonV1::MarketAlreadyResolved
    } else if work.next_bucket == work.end_bucket_exclusive {
        if terms.domain.spec_digest() != work.terms_digest
            || encode_basis_artifact(terms.domain.spec()) != work.basis_spec_artifact
        {
            return Err(ResolutionWorkError::BindingMismatch);
        }
        let summary = Summary::from_canonical_parts(
            terms.domain,
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
    } else if current_slot > work.expires_slot {
        // Any incomplete state, including zero progress, is permissionlessly
        // reapable after the bounded V1 TTL. Complete valid work stays finalizable.
        AbortReasonV1::ExpiredIncomplete
    } else if work.sample_count == 0 {
        if !caller_is_payer {
            return Err(ResolutionWorkError::AbortForbidden);
        }
        AbortReasonV1::Unstarted
    } else {
        return Err(ResolutionWorkError::AbortForbidden);
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

fn validate_static_bindings_projected(
    work: &ResolutionWorkAccountV1,
    terms: WorkTermsV1,
    archive: VerifiedSealedArchiveViewV1<'_>,
) -> Result<()> {
    let receipt = archive.receipt();
    validate_work_commitment_projected(work, terms)?;
    if work.terms_digest != terms.terms_digest
        || work.source_spec_digest != terms.feed
        || work.basis_spec_artifact != encode_basis_artifact(terms.domain.spec())
        || work.basis_spec_digest != basis_artifact_digest(&work.basis_spec_artifact)
        || work.cost_schedule_digest != cost_schedule_digest(work.costs)
        || work.grid_identity != terms.domain.grid_identity()
        || work.bucket_duration != terms.domain.bucket_duration()
        || work.outcome_count != terms.domain.spec().outcome_count
        || work.denominator != terms.domain.spec().denominator
        || work.finalization_mode != terms.finalization_mode
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

fn validate_work_commitment_projected(
    work: &ResolutionWorkAccountV1,
    terms: WorkTermsV1,
) -> Result<()> {
    let bindings = BeginBindingsV1 {
        payer: work.payer,
        prepaid_reserve: work.prepaid_reserve,
        work_nonce: work.work_nonce,
        market: work.market,
        resolution_target: work.resolution_target,
        program_owner: work.program_owner,
        basis_spec_digest: work.basis_spec_digest,
        cost_schedule_digest: work.cost_schedule_digest,
        costs: work.costs,
        deposited: payer_deposit(work.funding)?,
        initial_donation_lamports: work.funding.donation_lamports,
        opened_slot: work.opened_slot,
        expires_slot: work.expires_slot,
        finalization_mode: work.finalization_mode,
        work_bump: work.stored_bump,
        reserve_bump: work.reserve_bump,
    };
    if work.terms_digest != terms.terms_digest
        || work.source_spec_digest != terms.feed
        || work.basis_spec_artifact != encode_basis_artifact(terms.domain.spec())
        || work.basis_spec_digest != basis_artifact_digest(&work.basis_spec_artifact)
        || work.cost_schedule_digest != cost_schedule_digest(work.costs)
        || work.work_commitment
            != compute_work_commitment(
                &bindings,
                terms.terms_digest,
                terms.feed,
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

fn payer_deposit(funding: ResolutionWorkFundingV1) -> Result<u64> {
    funding
        .rent_locked
        .checked_add(funding.prepaid_remaining)
        .and_then(|value| value.checked_add(funding.charges_paid))
        .and_then(|value| value.checked_add(funding.rewards_paid))
        .ok_or(ResolutionWorkError::ArithmeticOverflow)
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
fn compute_work_commitment(
    bindings: &BeginBindingsV1,
    terms_digest: [u8; 32],
    source_spec_digest: [u8; 32],
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
    let deposited = bindings.deposited.to_be_bytes();
    solana_sha256_hasher::hashv(&[
        WORK_ID_DOMAIN_V1,
        &version,
        &bindings.market,
        &terms_digest,
        &bindings.resolution_target,
        &bindings.program_owner,
        &archive_account,
        &bindings.basis_spec_digest,
        &source_spec_digest,
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
        &deposited,
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

    fn bindings(terms: &TermsAccount) -> BeginBindingsV1 {
        let artifact =
            encode_basis_artifact(native_window::occupation_domain(terms).unwrap().spec());
        BeginBindingsV1 {
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
            initial_donation_lamports: 0,
            opened_slot: 10,
            expires_slot: 20,
            finalization_mode: FINALIZATION_LARGEST_REMAINDER_V1,
            work_bump: 200,
            reserve_bump: 201,
        }
    }

    #[test]
    fn every_chunk_partition_matches_monolithic_v4_and_late_finalize() {
        let (spec, spec_account, archive, window) = complete_archive([1, 4, 7, 12]);
        let terms = terms(spec);
        let archive_view = verified_archive(&spec_account, &archive, window, ARCHIVE_KEY);
        let begin = bindings(&terms);
        for chunks in [[1_u8, 1, 1, 1], [2, 2, 0, 0], [4, 0, 0, 0]] {
            let mut work = begin_state(begin, &terms, archive_view).unwrap();
            let mut now = 11;
            for count in chunks {
                if count == 0 {
                    continue;
                }
                let guard = FoldGuardsV1 {
                    work_commitment: work.work_commitment,
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
                abort_state(work, &terms, 99, false, false),
                Err(ResolutionWorkError::AbortForbidden)
            );
        }
    }

    #[test]
    fn wrong_cursor_archive_replay_expiry_and_underfunding_are_atomic() {
        let (spec, spec_account, archive, window) = complete_archive([1, 4, 7, 12]);
        let terms = terms(spec);
        let archive_view = verified_archive(&spec_account, &archive, window, ARCHIVE_KEY);
        let begin = bindings(&terms);
        let work = begin_state(begin, &terms, archive_view).unwrap();
        let mut wrong = FoldGuardsV1 {
            work_commitment: work.work_commitment,
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
            abort_state(first, &terms, 20, false, false),
            Err(ResolutionWorkError::AbortForbidden)
        );
        assert_eq!(
            abort_state(first, &terms, 21, false, false).unwrap().reason,
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
        let mut overlong = begin;
        overlong.expires_slot = overlong.opened_slot + RESOLUTION_WORK_MAX_LIFETIME_SLOTS_V1 + 1;
        assert_eq!(
            begin_state(overlong, &terms, archive_view),
            Err(ResolutionWorkError::InvalidSlot)
        );
        let unstarted = begin_state(begin, &terms, archive_view).unwrap();
        let mut mutable_basis_identity = unstarted;
        mutable_basis_identity.basis_spec_digest[0] ^= 1;
        assert_eq!(
            abort_state(mutable_basis_identity, &terms, 10, true, false),
            Err(ResolutionWorkError::BindingMismatch)
        );
        assert_eq!(
            abort_state(unstarted, &terms, 10, false, false),
            Err(ResolutionWorkError::AbortForbidden)
        );
        assert_eq!(
            abort_state(unstarted, &terms, 10, true, false)
                .unwrap()
                .reason,
            AbortReasonV1::Unstarted
        );
        assert_eq!(
            abort_state(unstarted, &terms, 21, false, false)
                .unwrap()
                .reason,
            AbortReasonV1::ExpiredIncomplete
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
