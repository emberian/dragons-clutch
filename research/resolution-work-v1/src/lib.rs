#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Safe, fixed-width model of resumable native occupation resolution.
//!
//! This crate is an isolated executable model. It is not a live instruction
//! ABI, account codec, compute-unit measurement, archive adapter, or release
//! claim. It makes the proposed state transition precise while keeping all
//! inputs fixed-width and allocation-free.

use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS, MAX_OUTCOMES};
use clutch_bspline_accumulator::{
    BasisDomain, FinalizationMode, SequentialSummaryBuilder, Summary, BASIS_EVALUATOR_VERSION,
    OCCUPATION_SUMMARY_VERSION,
};
use sha2::{Digest, Sha256};

/// Width of every modeled authenticated identity.
pub const ID_BYTES: usize = 32;
/// Resolution-work semantic version implemented by this model.
pub const RESOLUTION_WORK_VERSION: u16 = 1;
/// Sealed-archive receipt version implemented by this model.
pub const ARCHIVE_RECEIPT_VERSION: u16 = 1;
/// Canonical occupation resolution layout version emitted by finalization.
pub const RESOLUTION_V4_VERSION: u16 = 4;
/// Maximum number of records accepted by one fold transition.
pub const MAX_FOLD_RECORDS: usize = 4;
/// Exact current SourceArchive V1 record capacity.
pub const SOURCE_ARCHIVE_MAX_RECORDS_V1: usize = 32;
/// Exact current SourceArchive V1 account length.
pub const SOURCE_ARCHIVE_ACCOUNT_V1_BYTES: u32 = 2_560;
/// Exact revision-one native basis artifact width mirrored from the host codec.
pub const BASIS_SPEC_BYTES_V1: usize = 304;

/// A fixed-width external identity or canonical digest.
pub type Id = [u8; ID_BYTES];

const ZERO_ID: Id = [0; ID_BYTES];
const BASIS_MAGIC_V1: [u8; 8] = *b"DCBASV01";
const BASIS_SCHEMA_VERSION_V1: u16 = 1;
const SEMANTIC_NATIVE_BSPLINE: u8 = 1;
const BASIS_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/basis-spec/v1";

/// Result alias for resolution-work transitions.
pub type Result<T> = core::result::Result<T, Error>;

/// Deterministic refusal from a modeled resolution-work transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A required identity or commitment is all zero.
    InvalidIdentity,
    /// A work, archive, evaluator, summary, or resolution version is unsupported.
    UnsupportedVersion,
    /// A half-open archive or market window is empty or inconsistent.
    InvalidWindow,
    /// A complete canonical digest does not match the supplied object.
    DigestMismatch,
    /// An immutable market, Terms, basis, source, archive, or grid binding differs.
    BindingMismatch,
    /// A basis specification is invalid or cannot form an accumulator domain.
    InvalidBasis,
    /// An archive account or receipt has invalid length, state, count, or padding.
    InvalidArchive,
    /// A fold has zero records or exceeds the fixed call bound.
    InvalidChunk,
    /// The fold does not begin at the exact next unprocessed bucket.
    WrongCursor,
    /// A requested range extends past the frozen archive end.
    WrongRecordOrder,
    /// A checked count, cost, mass, cursor, or funding calculation overflowed.
    ArithmeticOverflow,
    /// The prepaid deposit cannot cover the complete worst-case lifecycle quote.
    Underfunded,
    /// Finalization was attempted before the exact frozen end cursor.
    NotAtEnd,
    /// A slot moved before the opening/progress slot or violated the frozen lifetime.
    InvalidSlot,
    /// A new fold was attempted strictly after the frozen work expiry.
    Expired,
    /// The accumulated archive has no accepted coverage.
    NoCoverage,
    /// One or more authenticated archive records are explicit gaps.
    IncompleteCoverage,
    /// Exact-only finalization encountered an inexact average.
    InexactAverage,
    /// The basis evaluator refused one authenticated accepted point.
    PointRefused,
    /// Abort is not allowed after partial progress or after successful finalizability.
    AbortForbidden,
    /// The work state was already finalized or aborted.
    AlreadyTerminal,
    /// A private accounting or accumulator invariant was violated.
    InvariantViolation,
}

/// Why one canonical archive bucket contributes no accepted observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MissingReasonV1 {
    /// The source had no canonical record for the bucket.
    Absent = 1,
    /// The canonical source record failed its frozen confidence rule.
    ConfidenceRefused = 2,
    /// The source adapter explicitly refused the record.
    SourceRefused = 3,
    /// The canonical record was present but malformed under the frozen version.
    Malformed = 4,
}

/// Canonical semantic content of one sealed archive bucket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveObservationV1 {
    /// An authenticated coordinate admitted by the frozen source policy.
    Accepted(u128),
    /// An explicit authenticated gap with a named reason.
    Missing(MissingReasonV1),
}

/// One canonical archive record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveRecordV1 {
    /// Absolute canonical bucket index.
    pub bucket: u64,
    /// Accepted point or explicit gap.
    pub observation: ArchiveObservationV1,
}

impl ArchiveRecordV1 {
    const EMPTY: Self = Self {
        bucket: 0,
        observation: ArchiveObservationV1::Accepted(0),
    };
}

/// Immutable runtime metadata and semantic domain used to initialize an archive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveAccountConfigV1 {
    /// Canonical archive PDA.
    pub account_key: Id,
    /// Expected Dragon's Clutch program owner.
    pub owner: Id,
    /// Runtime account data length.
    pub data_len: u32,
    /// Whether the data-bearing account is executable.
    pub executable: bool,
    /// Receipt/account semantic version.
    pub receipt_version: u16,
    /// Digest of the authenticated immutable source specification.
    pub source_spec_digest: Id,
    /// Digest of the source, grid, and record-codec domain.
    pub archive_domain_digest: Id,
    /// Opaque canonical equal-duration grid identity.
    pub grid_identity: Id,
    /// Nonzero equal bucket duration in grid units.
    pub bucket_duration: u64,
    /// Exact append-only archive generation.
    pub archive_generation: u64,
    /// Inclusive first canonical bucket.
    pub start_bucket: u64,
    /// Exclusive final canonical bucket.
    pub end_bucket_exclusive: u64,
}

/// Append-then-seal, fixed-capacity model of the current program-owned archive.
///
/// All fields are private. `append` is the only record mutator and refuses
/// after `seal`; there is no unseal, record-replacement, or post-seal mutation
/// method. This type-state-by-invariant models the live program obligation that
/// no instruction may mutate a sealed SourceArchive account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveAccountV1 {
    config: ArchiveAccountConfigV1,
    record_count: u8,
    records: [ArchiveRecordV1; SOURCE_ARCHIVE_MAX_RECORDS_V1],
    sealed: bool,
    stored_commitment: Id,
}

impl ArchiveAccountV1 {
    /// Create one empty unsealed archive snapshot with immutable metadata.
    pub fn new(config: ArchiveAccountConfigV1) -> Self {
        Self {
            config,
            record_count: 0,
            records: [ArchiveRecordV1::EMPTY; SOURCE_ARCHIVE_MAX_RECORDS_V1],
            sealed: false,
            stored_commitment: ZERO_ID,
        }
    }

    /// Append the exact next bucket before sealing.
    pub fn append(&mut self, observation: ArchiveObservationV1) -> Result<()> {
        if self.sealed {
            return Err(Error::InvalidArchive);
        }
        let index = usize::from(self.record_count);
        if index >= SOURCE_ARCHIVE_MAX_RECORDS_V1 {
            return Err(Error::InvalidArchive);
        }
        let bucket = self
            .config
            .start_bucket
            .checked_add(u64::from(self.record_count))
            .ok_or(Error::ArithmeticOverflow)?;
        if bucket >= self.config.end_bucket_exclusive {
            return Err(Error::InvalidArchive);
        }
        self.records[index] = ArchiveRecordV1 {
            bucket,
            observation,
        };
        self.record_count = self
            .record_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Seal a complete archive and store its full canonical commitment.
    pub fn seal(&mut self) -> Result<ArchiveReceiptV1> {
        if self.sealed {
            return Err(Error::InvalidArchive);
        }
        validate_archive_config(&self.config)?;
        let span = self
            .config
            .end_bucket_exclusive
            .checked_sub(self.config.start_bucket)
            .ok_or(Error::InvalidWindow)?;
        if span != u64::from(self.record_count) {
            return Err(Error::InvalidArchive);
        }
        validate_archive_padding(self)?;
        let mut next = self.clone();
        next.stored_commitment = archive_account_commitment(&next);
        check_id(next.stored_commitment)?;
        next.sealed = true;
        let receipt = next.receipt()?;
        *self = next;
        Ok(receipt)
    }

    /// Return the exact sealed receipt; unsealed or malformed state refuses.
    pub fn receipt(&self) -> Result<ArchiveReceiptV1> {
        self.validate_full()?;
        Ok(ArchiveReceiptV1 {
            receipt_version: self.config.receipt_version,
            archive_account: self.config.account_key,
            archive_owner: self.config.owner,
            source_spec_digest: self.config.source_spec_digest,
            archive_domain_digest: self.config.archive_domain_digest,
            grid_identity: self.config.grid_identity,
            bucket_duration: self.config.bucket_duration,
            archive_generation: self.config.archive_generation,
            start_bucket: self.config.start_bucket,
            end_bucket_exclusive: self.config.end_bucket_exclusive,
            record_count: u64::from(self.record_count),
            archive_digest: self.stored_commitment,
        })
    }

    /// Immutable runtime account key.
    pub const fn account_key(&self) -> Id {
        self.config.account_key
    }

    /// Immutable runtime owner.
    pub const fn owner(&self) -> Id {
        self.config.owner
    }

    /// Declared exact runtime data length.
    pub const fn data_len(&self) -> u32 {
        self.config.data_len
    }

    /// Whether this data-bearing runtime account is executable.
    pub const fn executable(&self) -> bool {
        self.config.executable
    }

    /// Whether the terminal seal transition occurred.
    pub const fn is_sealed(&self) -> bool {
        self.sealed
    }

    /// Full stored commitment frozen by sealing.
    pub const fn stored_commitment(&self) -> Id {
        self.stored_commitment
    }

    fn validate_full(&self) -> Result<()> {
        validate_archive_config(&self.config)?;
        if !self.sealed || self.stored_commitment == ZERO_ID {
            return Err(Error::InvalidArchive);
        }
        let span = self
            .config
            .end_bucket_exclusive
            .checked_sub(self.config.start_bucket)
            .ok_or(Error::InvalidWindow)?;
        if span != u64::from(self.record_count) {
            return Err(Error::InvalidArchive);
        }
        validate_archive_padding(self)?;
        if archive_account_commitment(self) != self.stored_commitment {
            return Err(Error::DigestMismatch);
        }
        Ok(())
    }

    fn recheck_frozen(&self, receipt: &ArchiveReceiptV1) -> Result<()> {
        if self.config.account_key != receipt.archive_account
            || self.config.owner != receipt.archive_owner
            || self.config.data_len != SOURCE_ARCHIVE_ACCOUNT_V1_BYTES
            || self.config.executable
            || !self.sealed
            || self.config.receipt_version != receipt.receipt_version
            || self.config.source_spec_digest != receipt.source_spec_digest
            || self.config.archive_domain_digest != receipt.archive_domain_digest
            || self.config.grid_identity != receipt.grid_identity
            || self.config.bucket_duration != receipt.bucket_duration
            || self.config.archive_generation != receipt.archive_generation
            || self.config.start_bucket != receipt.start_bucket
            || self.config.end_bucket_exclusive != receipt.end_bucket_exclusive
            || u64::from(self.record_count) != receipt.record_count
            || self.stored_commitment != receipt.archive_digest
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    fn record_at_bucket(&self, bucket: u64) -> Result<ArchiveRecordV1> {
        let offset = bucket
            .checked_sub(self.config.start_bucket)
            .ok_or(Error::WrongRecordOrder)?;
        if offset >= u64::from(self.record_count) {
            return Err(Error::WrongRecordOrder);
        }
        let index = usize::try_from(offset).map_err(|_| Error::ArithmeticOverflow)?;
        Ok(self.records[index])
    }
}

/// Caller-chosen work bounds containing no archive record bytes or proofs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldRequestV1 {
    /// Exact work identity expected by the caller.
    pub work_id: Id,
    /// Exact canonical archive account expected by the caller.
    pub archive_account: Id,
    /// Full sealed archive commitment expected by the caller.
    pub archive_digest: Id,
    /// Exact cursor expected by the caller.
    pub expected_cursor: u64,
    /// Number of next account-owned records in `1..=MAX_FOLD_RECORDS`.
    pub record_count: u8,
}

/// Authenticated description of one sealed canonical archive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveReceiptV1 {
    /// Receipt semantic version.
    pub receipt_version: u16,
    /// Exact canonical archive account/PDA.
    pub archive_account: Id,
    /// Exact expected program owner.
    pub archive_owner: Id,
    /// Digest of the authenticated immutable source specification.
    pub source_spec_digest: Id,
    /// Digest of the source, bucket-grid, and record-codec domain.
    pub archive_domain_digest: Id,
    /// Opaque canonical equal-duration grid identity.
    pub grid_identity: Id,
    /// Nonzero equal bucket duration in grid units.
    pub bucket_duration: u64,
    /// Exact append-only archive generation.
    pub archive_generation: u64,
    /// Inclusive first canonical bucket.
    pub start_bucket: u64,
    /// Exclusive final canonical bucket.
    pub end_bucket_exclusive: u64,
    /// Exact number of canonical records, including explicit gaps.
    pub record_count: u64,
    /// Full commitment stored by the sealed program-owned archive account.
    pub archive_digest: Id,
}

impl ArchiveReceiptV1 {
    /// Validate version, identities, exact current cap, and span/count equality.
    pub fn validate(&self) -> Result<()> {
        if self.receipt_version != ARCHIVE_RECEIPT_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        check_id(self.archive_account)?;
        check_id(self.archive_owner)?;
        check_id(self.source_spec_digest)?;
        check_id(self.archive_domain_digest)?;
        check_id(self.grid_identity)?;
        check_id(self.archive_digest)?;
        if self.bucket_duration == 0 || self.start_bucket >= self.end_bucket_exclusive {
            return Err(Error::InvalidWindow);
        }
        let span = self
            .end_bucket_exclusive
            .checked_sub(self.start_bucket)
            .ok_or(Error::InvalidWindow)?;
        if span != self.record_count
            || self.record_count == 0
            || self.record_count > SOURCE_ARCHIVE_MAX_RECORDS_V1 as u64
        {
            return Err(Error::InvalidArchive);
        }
        Ok(())
    }
}

/// Immutable market-side authority frozen into one work state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketBindingV4 {
    /// Market identity.
    pub market: Id,
    /// Complete immutable Terms digest.
    pub terms_digest: Id,
    /// Unique resolution account or target identity.
    pub resolution_target: Id,
    /// Exact Dragon's Clutch program owner of the archive account.
    pub program_owner: Id,
    /// Exact canonical sealed archive account/PDA.
    pub archive_account: Id,
    /// Canonical digest of the complete native basis specification.
    pub basis_spec_digest: Id,
    /// Exact authenticated source specification digest.
    pub source_spec_digest: Id,
    /// Exact sealed archive digest selected for resolution.
    pub archive_digest: Id,
    /// Exact archive-domain digest.
    pub archive_domain_digest: Id,
    /// Exact archive generation.
    pub archive_generation: u64,
    /// Frozen inclusive occupation-window start bucket.
    pub start_bucket: u64,
    /// Frozen exclusive occupation-window end bucket.
    pub end_bucket_exclusive: u64,
    /// Frozen native basis evaluator version.
    pub basis_evaluator_version: u16,
    /// Frozen occupation-summary version.
    pub occupation_summary_version: u16,
    /// Frozen resolution output version, exactly V4 here.
    pub resolution_version: u16,
}

impl MarketBindingV4 {
    /// Validate nonzero identities, window, and all semantic versions.
    pub fn validate(&self) -> Result<()> {
        check_id(self.market)?;
        check_id(self.terms_digest)?;
        check_id(self.resolution_target)?;
        check_id(self.program_owner)?;
        check_id(self.archive_account)?;
        check_id(self.basis_spec_digest)?;
        check_id(self.source_spec_digest)?;
        check_id(self.archive_digest)?;
        check_id(self.archive_domain_digest)?;
        if self.start_bucket >= self.end_bucket_exclusive {
            return Err(Error::InvalidWindow);
        }
        if self.basis_evaluator_version != BASIS_EVALUATOR_VERSION
            || self.occupation_summary_version != OCCUPATION_SUMMARY_VERSION
            || self.resolution_version != RESOLUTION_V4_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        Ok(())
    }
}

/// Abstract, externally measured cost and rent parameters frozen at `begin`.
///
/// Values are exact within this model but are deliberately not described as
/// Solana lamports, byte sizes, account counts, or compute units. A live
/// adapter would need to replace each placeholder with measured and admitted
/// release evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CostScheduleV1 {
    /// Cost-model semantic version.
    pub version: u16,
    /// Externally measured model-state bytes placeholder.
    pub work_state_bytes: u32,
    /// Locked rent reserve for the modeled work state.
    pub rent_reserve: u64,
    /// Minimum admitted work lifetime in slots.
    pub minimum_lifetime_slots: u64,
    /// One-time non-reward begin charge.
    pub begin_charge: u64,
    /// Non-reward charge per fold call.
    pub fold_base_charge: u64,
    /// Non-reward charge per authenticated record.
    pub fold_per_record_charge: u64,
    /// Worker reward per fold call.
    pub fold_base_reward: u64,
    /// Worker reward per authenticated record.
    pub fold_per_record_reward: u64,
    /// Non-reward successful-finalize charge.
    pub finalize_charge: u64,
    /// Successful-finalizer reward.
    pub finalize_reward: u64,
    /// Non-reward permitted-abort charge.
    pub abort_charge: u64,
    /// Permitted-aborter reward.
    pub abort_reward: u64,
}

impl CostScheduleV1 {
    /// Validate the version and nonzero state/rent placeholders.
    pub fn validate(&self) -> Result<()> {
        if self.version != RESOLUTION_WORK_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if self.work_state_bytes == 0 || self.rent_reserve == 0 || self.minimum_lifetime_slots == 0
        {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    /// Canonical digest frozen into the work identity.
    pub fn digest(&self) -> Result<Id> {
        self.validate()?;
        let mut hasher = tagged_hasher(b"DC_RESOLUTION_COST_SCHEDULE_V1");
        hash_u16(&mut hasher, self.version);
        hash_u32(&mut hasher, self.work_state_bytes);
        hash_u64(&mut hasher, self.rent_reserve);
        hash_u64(&mut hasher, self.minimum_lifetime_slots);
        hash_u64(&mut hasher, self.begin_charge);
        hash_u64(&mut hasher, self.fold_base_charge);
        hash_u64(&mut hasher, self.fold_per_record_charge);
        hash_u64(&mut hasher, self.fold_base_reward);
        hash_u64(&mut hasher, self.fold_per_record_reward);
        hash_u64(&mut hasher, self.finalize_charge);
        hash_u64(&mut hasher, self.finalize_reward);
        hash_u64(&mut hasher, self.abort_charge);
        hash_u64(&mut hasher, self.abort_reward);
        Ok(finish_hash(hasher))
    }

    /// Complete worst-case deposit, assuming every record is folded alone.
    ///
    /// The terminal reserve covers the more expensive of successful finalize
    /// and permitted abort. This makes a started valid work item economically
    /// completable without future deposits.
    pub fn minimum_deposit(&self, record_count: u64) -> Result<u64> {
        self.validate()?;
        if record_count == 0 {
            return Err(Error::InvalidWindow);
        }
        let per_singleton_fold = checked_sum4(
            self.fold_base_charge,
            self.fold_per_record_charge,
            self.fold_base_reward,
            self.fold_per_record_reward,
        )?;
        let fold_reserve = per_singleton_fold
            .checked_mul(record_count)
            .ok_or(Error::ArithmeticOverflow)?;
        let finalize_terminal = self
            .finalize_charge
            .checked_add(self.finalize_reward)
            .ok_or(Error::ArithmeticOverflow)?;
        let abort_terminal = self
            .abort_charge
            .checked_add(self.abort_reward)
            .ok_or(Error::ArithmeticOverflow)?;
        checked_sum4(
            self.rent_reserve,
            self.begin_charge,
            fold_reserve,
            core::cmp::max(finalize_terminal, abort_terminal),
        )
    }

    fn fold_cost(&self, record_count: u64) -> Result<(u64, u64)> {
        let charge = self
            .fold_per_record_charge
            .checked_mul(record_count)
            .and_then(|value| value.checked_add(self.fold_base_charge))
            .ok_or(Error::ArithmeticOverflow)?;
        let reward = self
            .fold_per_record_reward
            .checked_mul(record_count)
            .and_then(|value| value.checked_add(self.fold_base_reward))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok((charge, reward))
    }
}

/// Immutable input to `ResolutionWorkV1::begin`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginRequestV1 {
    /// Market-side identities and frozen interval.
    pub market: MarketBindingV4,
    /// Complete hostile native basis specification.
    pub basis_spec: BasisSpec,
    /// Authenticated sealed-archive receipt.
    pub archive: ArchiveReceiptV1,
    /// Exact average/final residual rule.
    pub finalization_mode: FinalizationMode,
    /// Frozen abstract cost and rent schedule.
    pub costs: CostScheduleV1,
    /// Total prepaid deposit, including locked rent.
    pub deposit: u64,
    /// Nonzero payer identity and sole terminal refund recipient.
    pub payer: Id,
    /// Nonzero segregated prepaid reserve/vault identity.
    pub prepaid_reserve: Id,
    /// Nonzero nonce making the work instance unique for the resolution target.
    pub work_nonce: Id,
    /// Slot at which Begin executes.
    pub current_slot: u64,
    /// Last slot in which a new Fold may execute, inclusive.
    pub expires_slot: u64,
}

/// Exact prepaid accounting retained by the work state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingLedgerV1 {
    deposited: u64,
    rent_locked: u64,
    prepaid_remaining: u64,
    charges_paid: u64,
    rewards_paid: u64,
    refund_paid: u64,
}

impl FundingLedgerV1 {
    /// Original total deposit.
    pub const fn deposited(&self) -> u64 {
        self.deposited
    }

    /// Rent reserve still locked in the open work account.
    pub const fn rent_locked(&self) -> u64 {
        self.rent_locked
    }

    /// Work budget not yet charged, rewarded, or refunded.
    pub const fn prepaid_remaining(&self) -> u64 {
        self.prepaid_remaining
    }

    /// Cumulative non-reward charges paid from the original deposit.
    pub const fn charges_paid(&self) -> u64 {
        self.charges_paid
    }

    /// Cumulative caller rewards paid from the original deposit.
    pub const fn rewards_paid(&self) -> u64 {
        self.rewards_paid
    }

    /// Terminal refund, including released rent, or zero while active.
    pub const fn refund_paid(&self) -> u64 {
        self.refund_paid
    }

    fn debit(&mut self, charge: u64, reward: u64) -> Result<()> {
        let outflow = charge
            .checked_add(reward)
            .ok_or(Error::ArithmeticOverflow)?;
        self.prepaid_remaining = self
            .prepaid_remaining
            .checked_sub(outflow)
            .ok_or(Error::Underfunded)?;
        self.charges_paid = self
            .charges_paid
            .checked_add(charge)
            .ok_or(Error::ArithmeticOverflow)?;
        self.rewards_paid = self
            .rewards_paid
            .checked_add(reward)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    fn close_and_refund(&mut self) -> Result<u64> {
        let refund = self
            .prepaid_remaining
            .checked_add(self.rent_locked)
            .ok_or(Error::ArithmeticOverflow)?;
        self.prepaid_remaining = 0;
        self.rent_locked = 0;
        self.refund_paid = refund;
        Ok(refund)
    }

    fn validate(&self, terminal: bool) -> Result<()> {
        let total = self
            .rent_locked
            .checked_add(self.prepaid_remaining)
            .and_then(|value| value.checked_add(self.charges_paid))
            .and_then(|value| value.checked_add(self.rewards_paid))
            .and_then(|value| value.checked_add(self.refund_paid))
            .ok_or(Error::ArithmeticOverflow)?;
        if total != self.deposited {
            return Err(Error::InvariantViolation);
        }
        if terminal {
            if self.rent_locked != 0 || self.prepaid_remaining != 0 {
                return Err(Error::InvariantViolation);
            }
        } else if self.rent_locked == 0 || self.refund_paid != 0 {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }
}

/// Current lifecycle phase of one work state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkStatusV1 {
    /// More folds or a terminal transition may be performed.
    Active,
    /// One canonical resolution was written and all funds were closed out.
    Finalized,
    /// A narrowly permitted abort closed the state without a payout vector.
    Aborted,
}

/// Canonical fixed-width payout vector embedded by resolution V4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoutVectorV1 {
    /// Number of active native Eggs.
    pub active_len: u8,
    /// Exact common payout denominator.
    pub denominator: u64,
    /// Active weights followed by canonical zero padding.
    pub weights: [u64; MAX_OUTCOMES],
}

/// One canonical occupation resolution produced exactly once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionV4 {
    /// Resolution layout version, exactly four.
    pub version: u16,
    /// Work instance authorizing this output.
    pub work_id: Id,
    /// Exact market identity.
    pub market: Id,
    /// Complete immutable Terms digest.
    pub terms_digest: Id,
    /// Unique resolution target identity.
    pub resolution_target: Id,
    /// Complete native basis digest.
    pub basis_spec_digest: Id,
    /// Exact source specification digest.
    pub source_spec_digest: Id,
    /// Exact sealed archive digest.
    pub archive_digest: Id,
    /// Inclusive first accumulated bucket.
    pub start_bucket: u64,
    /// Exclusive last accumulated bucket.
    pub end_bucket_exclusive: u64,
    /// Count of authenticated accepted buckets.
    pub coverage_count: u64,
    /// Count of authenticated explicit gaps.
    pub gap_count: u64,
    /// Frozen final averaging rule.
    pub finalization_mode: FinalizationMode,
    /// Canonical native payout vector.
    pub payout: PayoutVectorV1,
    /// Digest of every preceding canonical resolution field.
    pub resolution_commitment: Id,
}

/// Successful fold transfer plan and new cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldReceiptV1 {
    /// Work identity.
    pub work_id: Id,
    /// Reward recipient named by the successful caller.
    pub worker: Id,
    /// Segregated reserve from which charge and reward were debited.
    pub prepaid_reserve: Id,
    /// Inclusive first processed bucket.
    pub start_bucket: u64,
    /// Exclusive last processed bucket.
    pub end_bucket_exclusive: u64,
    /// Non-reward charge paid from prepaid funds.
    pub charge_paid: u64,
    /// Worker reward paid from prepaid funds.
    pub reward_paid: u64,
}

/// Successful finalization transfer plan and canonical output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizeReceiptV1 {
    /// Canonical resolution written exactly once.
    pub resolution: ResolutionV4,
    /// Reward recipient named by the successful caller.
    pub finalizer: Id,
    /// Segregated reserve from which charge, reward, and refund were debited.
    pub prepaid_reserve: Id,
    /// Non-reward charge paid from prepaid funds.
    pub charge_paid: u64,
    /// Finalizer reward paid from prepaid funds.
    pub reward_paid: u64,
    /// Exact payer receiving the terminal refund.
    pub payer: Id,
    /// Remaining work funds plus released rent returned to the payer.
    pub payer_refund: u64,
}

/// Why a permitted abort was terminally safe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortReasonV1 {
    /// No fold had yet consumed external work.
    Unstarted,
    /// The complete authenticated span contained no accepted buckets.
    CompleteNoCoverage,
    /// The complete authenticated span contained explicit gaps.
    CompleteWithGaps,
    /// Exact-only averaging was impossible after complete authenticated work.
    CompleteInexactAverage,
    /// The work remained incomplete strictly after its frozen Fold expiry.
    ExpiredIncomplete,
}

/// Successful abort transfer plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbortReceiptV1 {
    /// Work identity.
    pub work_id: Id,
    /// Exact permitted reason.
    pub reason: AbortReasonV1,
    /// Reward recipient named by the successful caller.
    pub aborter: Id,
    /// Segregated reserve from which charge, reward, and refund were debited.
    pub prepaid_reserve: Id,
    /// Non-reward charge paid from prepaid funds.
    pub charge_paid: u64,
    /// Aborter reward paid from prepaid funds.
    pub reward_paid: u64,
    /// Exact payer receiving the terminal refund.
    pub payer: Id,
    /// Remaining work funds plus released rent returned to the payer.
    pub payer_refund: u64,
}

/// Private resumable state for one exact sealed archive and resolution target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionWorkV1 {
    work_id: Id,
    payer: Id,
    prepaid_reserve: Id,
    work_nonce: Id,
    market: MarketBindingV4,
    basis_spec: BasisSpec,
    domain: BasisDomain,
    archive: ArchiveReceiptV1,
    mode: FinalizationMode,
    costs: CostScheduleV1,
    cost_schedule_digest: Id,
    next_bucket: u64,
    fold_count: u64,
    opened_slot: u64,
    expires_slot: u64,
    last_progress_slot: u64,
    completed_slot: Option<u64>,
    accumulator: SequentialSummaryBuilder,
    funding: FundingLedgerV1,
    status: WorkStatusV1,
    resolution: Option<ResolutionV4>,
}

impl ResolutionWorkV1 {
    /// Validate every immutable input once and create a fully prepaid work state.
    pub fn begin(request: BeginRequestV1, archive_account: &ArchiveAccountV1) -> Result<Self> {
        request.market.validate()?;
        request.archive.validate()?;
        archive_account.validate_full()?;
        archive_account.recheck_frozen(&request.archive)?;
        request.costs.validate()?;
        check_id(request.payer)?;
        check_id(request.prepaid_reserve)?;
        check_id(request.work_nonce)?;
        let lifetime = request
            .expires_slot
            .checked_sub(request.current_slot)
            .ok_or(Error::InvalidSlot)?;
        if lifetime < request.costs.minimum_lifetime_slots {
            return Err(Error::InvalidSlot);
        }
        request
            .basis_spec
            .validate()
            .map_err(|_| Error::InvalidBasis)?;

        let computed_basis_digest = basis_spec_digest(&request.basis_spec);
        if computed_basis_digest != request.market.basis_spec_digest
            || request.market.source_spec_digest != request.archive.source_spec_digest
            || request.market.program_owner != request.archive.archive_owner
            || request.market.archive_account != request.archive.archive_account
            || request.market.archive_digest != request.archive.archive_digest
            || request.market.archive_domain_digest != request.archive.archive_domain_digest
            || request.market.archive_generation != request.archive.archive_generation
            || request.market.start_bucket != request.archive.start_bucket
            || request.market.end_bucket_exclusive != request.archive.end_bucket_exclusive
        {
            return Err(Error::BindingMismatch);
        }

        let span = request.archive.record_count;
        let minimum = request.costs.minimum_deposit(span)?;
        if request.deposit < minimum {
            return Err(Error::Underfunded);
        }
        let domain = BasisDomain::new(
            computed_basis_digest,
            request.archive.grid_identity,
            request.archive.bucket_duration,
            request.basis_spec,
        )
        .map_err(|_| Error::InvalidBasis)?;
        let accumulator = SequentialSummaryBuilder::new(domain).map_err(map_accumulator_error)?;
        let cost_schedule_digest = request.costs.digest()?;
        let work_id = work_identity(
            &request.market,
            request.archive.archive_digest,
            cost_schedule_digest,
            request.finalization_mode,
            request.payer,
            request.prepaid_reserve,
            request.work_nonce,
            request.current_slot,
            request.expires_slot,
        );
        let prepaid_after_rent = request
            .deposit
            .checked_sub(request.costs.rent_reserve)
            .and_then(|value| value.checked_sub(request.costs.begin_charge))
            .ok_or(Error::Underfunded)?;
        let funding = FundingLedgerV1 {
            deposited: request.deposit,
            rent_locked: request.costs.rent_reserve,
            prepaid_remaining: prepaid_after_rent,
            charges_paid: request.costs.begin_charge,
            rewards_paid: 0,
            refund_paid: 0,
        };
        let work = Self {
            work_id,
            payer: request.payer,
            prepaid_reserve: request.prepaid_reserve,
            work_nonce: request.work_nonce,
            market: request.market,
            basis_spec: request.basis_spec,
            domain,
            archive: request.archive,
            mode: request.finalization_mode,
            costs: request.costs,
            cost_schedule_digest,
            next_bucket: request.archive.start_bucket,
            fold_count: 0,
            opened_slot: request.current_slot,
            expires_slot: request.expires_slot,
            last_progress_slot: request.current_slot,
            completed_slot: None,
            accumulator,
            funding,
            status: WorkStatusV1::Active,
            resolution: None,
        };
        work.validate()?;
        Ok(work)
    }

    /// Recheck and atomically fold next records read only from the sealed account.
    pub fn fold(
        &mut self,
        request: FoldRequestV1,
        archive_account: &ArchiveAccountV1,
        worker: Id,
        current_slot: u64,
    ) -> Result<FoldReceiptV1> {
        self.require_active()?;
        check_id(worker)?;
        self.validate()?;
        if current_slot < self.last_progress_slot {
            return Err(Error::InvalidSlot);
        }
        if current_slot > self.expires_slot {
            return Err(Error::Expired);
        }
        validate_fold_request(self, request)?;
        archive_account.recheck_frozen(&self.archive)?;

        let count = usize::from(request.record_count);
        let mut next_accumulator = self.accumulator.clone();
        let mut offset = 0_usize;
        while offset < count {
            let expected_bucket = self
                .next_bucket
                .checked_add(u64::try_from(offset).map_err(|_| Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            if expected_bucket >= self.archive.end_bucket_exclusive {
                return Err(Error::WrongRecordOrder);
            }
            let record = archive_account.record_at_bucket(expected_bucket)?;
            if record.bucket != expected_bucket {
                return Err(Error::InvariantViolation);
            }
            match record.observation {
                ArchiveObservationV1::Accepted(point) => next_accumulator
                    .append_accepted(expected_bucket, point)
                    .map_err(map_accumulator_error)?,
                ArchiveObservationV1::Missing(_) => next_accumulator
                    .append_missing(expected_bucket)
                    .map_err(map_accumulator_error)?,
            }
            offset += 1;
        }

        let end = self
            .next_bucket
            .checked_add(u64::from(request.record_count))
            .ok_or(Error::ArithmeticOverflow)?;
        if end > self.archive.end_bucket_exclusive {
            return Err(Error::WrongRecordOrder);
        }
        let (charge, reward) = self.costs.fold_cost(u64::from(request.record_count))?;
        let mut next_funding = self.funding;
        next_funding.debit(charge, reward)?;
        let next_fold_count = self
            .fold_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;

        let receipt = FoldReceiptV1 {
            work_id: self.work_id,
            worker,
            prepaid_reserve: self.prepaid_reserve,
            start_bucket: self.next_bucket,
            end_bucket_exclusive: end,
            charge_paid: charge,
            reward_paid: reward,
        };
        let mut next = self.clone();
        next.accumulator = next_accumulator;
        next.funding = next_funding;
        next.next_bucket = end;
        next.fold_count = next_fold_count;
        next.last_progress_slot = current_slot;
        if end == self.archive.end_bucket_exclusive {
            next.completed_slot = Some(current_slot);
        }
        next.validate()?;
        *self = next;
        Ok(receipt)
    }

    /// At the exact end, write one canonical V4 vector and close all work funds.
    pub fn finalize(&mut self, finalizer: Id, current_slot: u64) -> Result<FinalizeReceiptV1> {
        self.require_active()?;
        check_id(finalizer)?;
        self.validate()?;
        if current_slot < self.last_progress_slot {
            return Err(Error::InvalidSlot);
        }
        if self.next_bucket != self.archive.end_bucket_exclusive {
            return Err(Error::NotAtEnd);
        }
        if self.completed_slot.is_none() {
            return Err(Error::InvariantViolation);
        }
        let summary = self.accumulator.clone().finish();
        let weights = summary.finalize(self.mode).map_err(map_accumulator_error)?;
        let payout = PayoutVectorV1 {
            active_len: weights.active_len(),
            denominator: weights.denominator(),
            weights: weights.weights(),
        };
        let mut resolution = ResolutionV4 {
            version: RESOLUTION_V4_VERSION,
            work_id: self.work_id,
            market: self.market.market,
            terms_digest: self.market.terms_digest,
            resolution_target: self.market.resolution_target,
            basis_spec_digest: self.market.basis_spec_digest,
            source_spec_digest: self.market.source_spec_digest,
            archive_digest: self.market.archive_digest,
            start_bucket: self.market.start_bucket,
            end_bucket_exclusive: self.market.end_bucket_exclusive,
            coverage_count: summary.coverage_count(),
            gap_count: summary.gap_count(),
            finalization_mode: self.mode,
            payout,
            resolution_commitment: ZERO_ID,
        };
        resolution.resolution_commitment = resolution_digest(&resolution);

        let mut next_funding = self.funding;
        next_funding.debit(self.costs.finalize_charge, self.costs.finalize_reward)?;
        let refund = next_funding.close_and_refund()?;
        next_funding.validate(true)?;

        let receipt = FinalizeReceiptV1 {
            resolution,
            finalizer,
            prepaid_reserve: self.prepaid_reserve,
            charge_paid: self.costs.finalize_charge,
            reward_paid: self.costs.finalize_reward,
            payer: self.payer,
            payer_refund: refund,
        };
        let mut next = self.clone();
        next.funding = next_funding;
        next.status = WorkStatusV1::Finalized;
        next.resolution = Some(resolution);
        next.validate()?;
        *self = next;
        Ok(receipt)
    }

    /// Close an unstarted item, or a completely folded deterministic refusal.
    ///
    /// Partial-progress abort is deliberately forbidden: once a worker has
    /// advanced the cursor, worst-case prepayment makes completion the only
    /// ordinary path. At the exact end, abort is permitted only when the
    /// frozen finalization semantics deterministically refuse a payout.
    pub fn abort(&mut self, aborter: Id, current_slot: u64) -> Result<AbortReceiptV1> {
        self.require_active()?;
        check_id(aborter)?;
        self.validate()?;
        if current_slot < self.last_progress_slot {
            return Err(Error::InvalidSlot);
        }
        let summary = self.accumulator.clone().finish();
        let reason = if self.next_bucket == self.archive.start_bucket {
            if current_slot <= self.expires_slot && aborter != self.payer {
                return Err(Error::AbortForbidden);
            }
            AbortReasonV1::Unstarted
        } else if self.next_bucket != self.archive.end_bucket_exclusive {
            if current_slot <= self.expires_slot {
                return Err(Error::AbortForbidden);
            }
            AbortReasonV1::ExpiredIncomplete
        } else {
            match summary.finalize(self.mode) {
                Err(clutch_bspline_accumulator::Error::NoCoverage) => {
                    AbortReasonV1::CompleteNoCoverage
                }
                Err(clutch_bspline_accumulator::Error::IncompleteCoverage) => {
                    AbortReasonV1::CompleteWithGaps
                }
                Err(clutch_bspline_accumulator::Error::InexactAverage) => {
                    AbortReasonV1::CompleteInexactAverage
                }
                _ => return Err(Error::AbortForbidden),
            }
        };

        let mut next_funding = self.funding;
        next_funding.debit(self.costs.abort_charge, self.costs.abort_reward)?;
        let refund = next_funding.close_and_refund()?;
        next_funding.validate(true)?;
        let receipt = AbortReceiptV1 {
            work_id: self.work_id,
            reason,
            aborter,
            prepaid_reserve: self.prepaid_reserve,
            charge_paid: self.costs.abort_charge,
            reward_paid: self.costs.abort_reward,
            payer: self.payer,
            payer_refund: refund,
        };
        let mut next = self.clone();
        next.funding = next_funding;
        next.status = WorkStatusV1::Aborted;
        next.validate()?;
        *self = next;
        Ok(receipt)
    }

    /// Unique identity of this fully bound work instance.
    pub const fn work_id(&self) -> Id {
        self.work_id
    }

    /// Exact next unprocessed bucket.
    pub const fn next_bucket(&self) -> u64 {
        self.next_bucket
    }

    /// Number of successful fold transitions.
    pub const fn fold_count(&self) -> u64 {
        self.fold_count
    }

    /// Payer and sole terminal refund recipient.
    pub const fn payer(&self) -> Id {
        self.payer
    }

    /// Segregated reserve/vault identity funding all work transitions.
    pub const fn prepaid_reserve(&self) -> Id {
        self.prepaid_reserve
    }

    /// Last slot in which a new Fold is admissible.
    pub const fn expires_slot(&self) -> u64 {
        self.expires_slot
    }

    /// Slot at which the exact end cursor was reached, if complete.
    pub const fn completed_slot(&self) -> Option<u64> {
        self.completed_slot
    }

    /// Current active or terminal phase.
    pub const fn status(&self) -> WorkStatusV1 {
        self.status
    }

    /// Exact prepaid accounting ledger.
    pub const fn funding(&self) -> FundingLedgerV1 {
        self.funding
    }

    /// Canonical summary accumulated so far.
    pub fn summary(&self) -> Summary {
        self.accumulator.clone().finish()
    }

    /// Canonical output after successful finalization, otherwise `None`.
    pub const fn resolution(&self) -> Option<ResolutionV4> {
        self.resolution
    }

    /// Frozen cost-schedule digest.
    pub const fn cost_schedule_digest(&self) -> Id {
        self.cost_schedule_digest
    }

    fn require_active(&self) -> Result<()> {
        if self.status != WorkStatusV1::Active {
            return Err(Error::AlreadyTerminal);
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        self.market.validate()?;
        self.archive.validate()?;
        self.costs.validate()?;
        check_id(self.work_id)?;
        check_id(self.payer)?;
        check_id(self.prepaid_reserve)?;
        check_id(self.work_nonce)?;
        let lifetime = self
            .expires_slot
            .checked_sub(self.opened_slot)
            .ok_or(Error::InvariantViolation)?;
        if lifetime < self.costs.minimum_lifetime_slots
            || self.last_progress_slot < self.opened_slot
            || self.last_progress_slot > self.expires_slot
        {
            return Err(Error::InvariantViolation);
        }
        let expected_work_id = work_identity(
            &self.market,
            self.archive.archive_digest,
            self.cost_schedule_digest,
            self.mode,
            self.payer,
            self.prepaid_reserve,
            self.work_nonce,
            self.opened_slot,
            self.expires_slot,
        );
        if self.work_id != expected_work_id
            || self.cost_schedule_digest != self.costs.digest()?
            || self.market.program_owner != self.archive.archive_owner
            || self.market.archive_account != self.archive.archive_account
            || self.market.archive_digest != self.archive.archive_digest
            || self.market.source_spec_digest != self.archive.source_spec_digest
            || self.market.archive_domain_digest != self.archive.archive_domain_digest
            || self.market.archive_generation != self.archive.archive_generation
            || self.market.start_bucket != self.archive.start_bucket
            || self.market.end_bucket_exclusive != self.archive.end_bucket_exclusive
            || self.domain.spec() != self.basis_spec
            || self.domain.spec_digest() != self.market.basis_spec_digest
            || self.domain.grid_identity() != self.archive.grid_identity
            || self.domain.bucket_duration() != self.archive.bucket_duration
        {
            return Err(Error::InvariantViolation);
        }
        if self.next_bucket < self.archive.start_bucket
            || self.next_bucket > self.archive.end_bucket_exclusive
        {
            return Err(Error::InvariantViolation);
        }
        let summary = self.accumulator.clone().finish();
        summary.validate().map_err(map_accumulator_error)?;
        let expected_count = self
            .next_bucket
            .checked_sub(self.archive.start_bucket)
            .ok_or(Error::InvariantViolation)?;
        if summary.sample_count() != expected_count {
            return Err(Error::InvariantViolation);
        }
        if self.next_bucket == self.archive.end_bucket_exclusive {
            if self.completed_slot != Some(self.last_progress_slot) {
                return Err(Error::InvariantViolation);
            }
        } else if self.completed_slot.is_some() {
            return Err(Error::InvariantViolation);
        }
        match self.status {
            WorkStatusV1::Active => {
                if self.resolution.is_some() {
                    return Err(Error::InvariantViolation);
                }
                self.funding.validate(false)
            }
            WorkStatusV1::Finalized => {
                validate_resolution(self, self.resolution.ok_or(Error::InvariantViolation)?)?;
                self.funding.validate(true)
            }
            WorkStatusV1::Aborted => {
                if self.resolution.is_some() {
                    return Err(Error::InvariantViolation);
                }
                self.funding.validate(true)
            }
        }
    }
}

/// Encode the exact existing revision-one native basis artifact projection.
///
/// A caller must separately validate the hostile [`BasisSpec`] before treating
/// these bytes as canonical. `begin` does so before using their digest. The
/// layout deliberately matches the existing 304-byte host compiler artifact;
/// this model does not define a second basis identity.
pub fn encode_basis_spec_v1(spec: &BasisSpec) -> [u8; BASIS_SPEC_BYTES_V1] {
    let mut out = [0_u8; BASIS_SPEC_BYTES_V1];
    out[0..8].copy_from_slice(&BASIS_MAGIC_V1);
    out[8..10].copy_from_slice(&BASIS_SCHEMA_VERSION_V1.to_le_bytes());
    out[10..12].copy_from_slice(&BASIS_EVALUATOR_VERSION.to_le_bytes());
    out[12] = SEMANTIC_NATIVE_BSPLINE;
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

/// Compute the existing domain-separated revision-one basis artifact digest.
pub fn basis_spec_digest(spec: &BasisSpec) -> Id {
    let mut hasher = Sha256::new();
    hasher.update(BASIS_DIGEST_DOMAIN_V1);
    hasher.update(encode_basis_spec_v1(spec));
    finish_hash(hasher)
}

fn validate_fold_request(work: &ResolutionWorkV1, request: FoldRequestV1) -> Result<()> {
    if request.work_id != work.work_id
        || request.archive_account != work.archive.archive_account
        || request.archive_digest != work.archive.archive_digest
    {
        return Err(Error::BindingMismatch);
    }
    if request.expected_cursor != work.next_bucket {
        return Err(Error::WrongCursor);
    }
    let count = usize::from(request.record_count);
    if count == 0 || count > MAX_FOLD_RECORDS {
        return Err(Error::InvalidChunk);
    }
    let end = request
        .expected_cursor
        .checked_add(u64::from(request.record_count))
        .ok_or(Error::ArithmeticOverflow)?;
    if end > work.archive.end_bucket_exclusive {
        return Err(Error::WrongRecordOrder);
    }
    Ok(())
}

fn validate_archive_config(config: &ArchiveAccountConfigV1) -> Result<()> {
    check_id(config.account_key)?;
    check_id(config.owner)?;
    check_id(config.source_spec_digest)?;
    check_id(config.archive_domain_digest)?;
    check_id(config.grid_identity)?;
    if config.receipt_version != ARCHIVE_RECEIPT_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    if config.data_len != SOURCE_ARCHIVE_ACCOUNT_V1_BYTES || config.executable {
        return Err(Error::InvalidArchive);
    }
    if config.bucket_duration == 0 || config.start_bucket >= config.end_bucket_exclusive {
        return Err(Error::InvalidWindow);
    }
    let span = config
        .end_bucket_exclusive
        .checked_sub(config.start_bucket)
        .ok_or(Error::InvalidWindow)?;
    if span == 0 || span > SOURCE_ARCHIVE_MAX_RECORDS_V1 as u64 {
        return Err(Error::InvalidArchive);
    }
    Ok(())
}

fn validate_archive_padding(archive: &ArchiveAccountV1) -> Result<()> {
    let active = usize::from(archive.record_count);
    let mut index = 0_usize;
    while index < SOURCE_ARCHIVE_MAX_RECORDS_V1 {
        if index < active {
            let expected = archive
                .config
                .start_bucket
                .checked_add(u64::try_from(index).map_err(|_| Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            if archive.records[index].bucket != expected {
                return Err(Error::InvalidArchive);
            }
        } else if archive.records[index] != ArchiveRecordV1::EMPTY {
            return Err(Error::InvalidArchive);
        }
        index += 1;
    }
    Ok(())
}

fn archive_account_commitment(archive: &ArchiveAccountV1) -> Id {
    let mut hasher = tagged_hasher(b"DC_SOURCE_ARCHIVE_FULL_COMMITMENT_MODEL_V1");
    hash_u16(&mut hasher, archive.config.receipt_version);
    hasher.update(archive.config.account_key);
    hasher.update(archive.config.owner);
    hash_u32(&mut hasher, archive.config.data_len);
    hasher.update([u8::from(archive.config.executable)]);
    hasher.update(archive.config.source_spec_digest);
    hasher.update(archive.config.archive_domain_digest);
    hasher.update(archive.config.grid_identity);
    hash_u64(&mut hasher, archive.config.bucket_duration);
    hash_u64(&mut hasher, archive.config.archive_generation);
    hash_u64(&mut hasher, archive.config.start_bucket);
    hash_u64(&mut hasher, archive.config.end_bucket_exclusive);
    hasher.update([archive.record_count]);
    hasher.update([1]); // terminal sealed state
    for record in archive.records {
        hash_u64(&mut hasher, record.bucket);
        match record.observation {
            ArchiveObservationV1::Accepted(point) => {
                hasher.update([0]);
                hash_u128(&mut hasher, point);
            }
            ArchiveObservationV1::Missing(reason) => {
                hasher.update([1, reason as u8]);
                hash_u128(&mut hasher, 0);
            }
        }
    }
    finish_hash(hasher)
}

#[allow(clippy::too_many_arguments)]
fn work_identity(
    market: &MarketBindingV4,
    archive_digest: Id,
    cost_digest: Id,
    mode: FinalizationMode,
    payer: Id,
    prepaid_reserve: Id,
    nonce: Id,
    opened_slot: u64,
    expires_slot: u64,
) -> Id {
    let mut hasher = tagged_hasher(b"DC_RESOLUTION_WORK_ID_V1");
    hash_u16(&mut hasher, RESOLUTION_WORK_VERSION);
    hasher.update(market.market);
    hasher.update(market.terms_digest);
    hasher.update(market.resolution_target);
    hasher.update(market.program_owner);
    hasher.update(market.archive_account);
    hasher.update(market.basis_spec_digest);
    hasher.update(market.source_spec_digest);
    hasher.update(archive_digest);
    hasher.update(market.archive_domain_digest);
    hash_u64(&mut hasher, market.archive_generation);
    hash_u64(&mut hasher, market.start_bucket);
    hash_u64(&mut hasher, market.end_bucket_exclusive);
    hash_u16(&mut hasher, market.basis_evaluator_version);
    hash_u16(&mut hasher, market.occupation_summary_version);
    hash_u16(&mut hasher, market.resolution_version);
    hasher.update([finalization_mode_tag(mode)]);
    hasher.update(cost_digest);
    hasher.update(payer);
    hasher.update(prepaid_reserve);
    hasher.update(nonce);
    hash_u64(&mut hasher, opened_slot);
    hash_u64(&mut hasher, expires_slot);
    finish_hash(hasher)
}

fn resolution_digest(resolution: &ResolutionV4) -> Id {
    let mut hasher = tagged_hasher(b"DC_NATIVE_RESOLUTION_V4");
    hash_u16(&mut hasher, resolution.version);
    hasher.update(resolution.work_id);
    hasher.update(resolution.market);
    hasher.update(resolution.terms_digest);
    hasher.update(resolution.resolution_target);
    hasher.update(resolution.basis_spec_digest);
    hasher.update(resolution.source_spec_digest);
    hasher.update(resolution.archive_digest);
    hash_u64(&mut hasher, resolution.start_bucket);
    hash_u64(&mut hasher, resolution.end_bucket_exclusive);
    hash_u64(&mut hasher, resolution.coverage_count);
    hash_u64(&mut hasher, resolution.gap_count);
    hasher.update([finalization_mode_tag(resolution.finalization_mode)]);
    hasher.update([resolution.payout.active_len]);
    hash_u64(&mut hasher, resolution.payout.denominator);
    let mut index = 0_usize;
    while index < MAX_OUTCOMES {
        hash_u64(&mut hasher, resolution.payout.weights[index]);
        index += 1;
    }
    finish_hash(hasher)
}

fn validate_resolution(work: &ResolutionWorkV1, resolution: ResolutionV4) -> Result<()> {
    let summary = work.accumulator.clone().finish();
    let expected = summary.finalize(work.mode).map_err(map_accumulator_error)?;
    if resolution.version != RESOLUTION_V4_VERSION
        || resolution.work_id != work.work_id
        || resolution.market != work.market.market
        || resolution.terms_digest != work.market.terms_digest
        || resolution.resolution_target != work.market.resolution_target
        || resolution.basis_spec_digest != work.market.basis_spec_digest
        || resolution.source_spec_digest != work.market.source_spec_digest
        || resolution.archive_digest != work.archive.archive_digest
        || resolution.start_bucket != work.archive.start_bucket
        || resolution.end_bucket_exclusive != work.archive.end_bucket_exclusive
        || resolution.coverage_count != summary.coverage_count()
        || resolution.gap_count != summary.gap_count()
        || resolution.finalization_mode != work.mode
        || resolution.payout.active_len != expected.active_len()
        || resolution.payout.denominator != expected.denominator()
        || resolution.payout.weights != expected.weights()
        || resolution.resolution_commitment == ZERO_ID
        || resolution.resolution_commitment != resolution_digest(&resolution)
    {
        return Err(Error::InvariantViolation);
    }
    Ok(())
}

fn finalization_mode_tag(mode: FinalizationMode) -> u8 {
    match mode {
        FinalizationMode::ExactOnly => 0,
        FinalizationMode::LargestRemainderV1 => 1,
    }
}

fn map_accumulator_error(error: clutch_bspline_accumulator::Error) -> Error {
    match error {
        clutch_bspline_accumulator::Error::NoCoverage => Error::NoCoverage,
        clutch_bspline_accumulator::Error::IncompleteCoverage => Error::IncompleteCoverage,
        clutch_bspline_accumulator::Error::InexactAverage => Error::InexactAverage,
        clutch_bspline_accumulator::Error::Basis(_) => Error::PointRefused,
        clutch_bspline_accumulator::Error::ArithmeticOverflow
        | clutch_bspline_accumulator::Error::BucketOverflow => Error::ArithmeticOverflow,
        _ => Error::InvariantViolation,
    }
}

fn check_id(identity: Id) -> Result<()> {
    if identity == ZERO_ID {
        return Err(Error::InvalidIdentity);
    }
    Ok(())
}

fn checked_sum4(a: u64, b: u64, c: u64, d: u64) -> Result<u64> {
    a.checked_add(b)
        .and_then(|value| value.checked_add(c))
        .and_then(|value| value.checked_add(d))
        .ok_or(Error::ArithmeticOverflow)
}

fn tagged_hasher(tag: &[u8]) -> Sha256 {
    let mut hasher = Sha256::new();
    hasher.update(tag);
    hasher
}

fn finish_hash(hasher: Sha256) -> Id {
    hasher.finalize().into()
}

fn hash_u16(hasher: &mut Sha256, value: u16) {
    hasher.update(value.to_be_bytes());
}

fn hash_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_be_bytes());
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn hash_u128(hasher: &mut Sha256, value: u128) {
    hasher.update(value.to_be_bytes());
}
