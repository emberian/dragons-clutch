//! Fixed hostile-byte layout for resumable occupation-resolution work.
//!
//! This module is intentionally self-contained until its instruction and
//! account dispatch is integrated.  It owns no archive bytes, observations,
//! proofs, caller-supplied masses, payout vectors, or terminal resolution
//! authority.  A live Fold names only the already-bound work and sealed
//! archive plus an optimistic cursor and bounded count; the program must read
//! those cursor-indexed records directly from the immutable program-owned
//! archive account.
//!
//! **STOP:** the numeric tags in this isolated module are proposed allocations,
//! not live ABI. The layout export and instruction router must recheck their
//! then-current registries before making any of them reachable.

/// Width of every identity and complete commitment.
pub const HASH_BYTES: usize = 32;
/// Maximum active outcome count and mass-vector width.
pub const MAX_OUTCOMES: usize = 16;
/// Exact current sealed SourceArchive V1 capacity.
pub const SOURCE_ARCHIVE_MAX_RECORDS_V1: u8 = 32;
/// Maximum records one Fold instruction may process.
pub const MAX_FOLD_RECORDS_V1: u8 = 4;
/// Exact canonical native BasisSpec V1 artifact width.
pub const BASIS_SPEC_BYTES_V1: usize = 304;
/// ResolutionWork account discriminator proposed after existing account tag 21.
pub const RESOLUTION_WORK_ACCOUNT_TAG: u8 = 22;
/// First ResolutionWork account schema.
pub const RESOLUTION_WORK_ACCOUNT_VERSION: u8 = 1;
/// Exact encoded length of one active [`ResolutionWorkAccountV1`].
pub const RESOLUTION_WORK_ACCOUNT_BYTES: usize = 1_296;
/// Existing common instruction envelope version.
pub const RESOLUTION_WORK_INTENT_VERSION: u8 = 3;
/// Begin instruction discriminator proposed after existing instruction tag 31.
pub const BEGIN_RESOLUTION_WORK_TAG: u8 = 32;
/// Fold instruction discriminator.
pub const FOLD_RESOLUTION_WORK_TAG: u8 = 33;
/// Finalize instruction discriminator.
pub const FINALIZE_RESOLUTION_WORK_TAG: u8 = 34;
/// Abort instruction discriminator.
pub const ABORT_RESOLUTION_WORK_TAG: u8 = 35;
/// Exact Begin instruction-data length.
pub const BEGIN_RESOLUTION_WORK_BYTES: usize = 2 + HASH_BYTES + 1 + 8 + 8 + HASH_BYTES;
/// Exact Fold instruction-data length.
pub const FOLD_RESOLUTION_WORK_BYTES: usize = 2 + (3 * HASH_BYTES) + 8 + 1;
/// Exact Finalize instruction-data length.
pub const FINALIZE_RESOLUTION_WORK_BYTES: usize = 2 + HASH_BYTES + 8 + HASH_BYTES;
/// Exact Abort instruction-data length.
pub const ABORT_RESOLUTION_WORK_BYTES: usize = 2 + HASH_BYTES + 8 + HASH_BYTES;

/// Only persisted Work status. Terminal transitions close the account.
pub const WORK_STATUS_ACTIVE: u8 = 1;
/// Exact-only componentwise averaging.
pub const FINALIZATION_EXACT_ONLY: u8 = 1;
/// Largest-remainder averaging with lowest-index exact ties.
pub const FINALIZATION_LARGEST_REMAINDER_V1: u8 = 2;
/// Frozen native B-spline evaluator semantic version.
pub const BASIS_EVALUATOR_VERSION_V1: u16 = 1;
/// Frozen occupation-summary semantic version.
pub const OCCUPATION_SUMMARY_VERSION_V1: u16 = 1;
/// Sole output Resolution layout version.
pub const OCCUPATION_RESOLUTION_VERSION_V4: u16 = 4;
/// Frozen cost-schedule semantic version.
pub const RESOLUTION_WORK_COST_VERSION_V1: u16 = 1;

const BASIS_MAGIC_V1: [u8; 8] = *b"DCBASV01";
const BASIS_SCHEMA_VERSION_V1: u16 = 1;
const BASIS_SEMANTIC_NATIVE_BSPLINE: u8 = 1;
const UNIFORM_SPACING_NONE: u8 = u8::MAX;
const BASIS_EDGE_CLAMP: u8 = 1;
const BASIS_EDGE_REFUSE: u8 = 2;

/// Result of a ResolutionWork codec operation.
pub type Result<T> = core::result::Result<T, ResolutionWorkCodecError>;

/// Deterministic refusal from hostile ResolutionWork bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionWorkCodecError {
    /// Input was shorter than the exact wire shape.
    Truncated,
    /// Input carried bytes beyond the exact wire shape.
    TrailingBytes,
    /// Output cannot hold the exact wire shape.
    OutputTooSmall,
    /// Account or instruction discriminator is wrong.
    WrongTag,
    /// Account, intent, or semantic version is unsupported.
    WrongVersion,
    /// A required identity or commitment is all zero.
    ZeroIdentity,
    /// An enum byte is not registered.
    InvalidEnum,
    /// A count, span, or vector shape is impossible.
    InvalidCount,
    /// A slot or half-open bucket window is impossible.
    InvalidWindow,
    /// Two redundant semantic bindings disagree.
    MismatchedBinding,
    /// Reserved or inactive fixed-width padding is nonzero.
    NonCanonicalPadding,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// The prepaid deposit cannot cover the frozen worst-case lifecycle.
    Underfunded,
}

/// Complete frozen cost and reward schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionWorkCostScheduleV1 {
    /// Cost semantic version.
    pub version: u16,
    /// Exact account bytes on which the release rent quote was measured.
    pub work_state_bytes: u32,
    /// Rent locked until atomic close.
    pub rent_reserve: u64,
    /// Minimum admitted lifetime from Begin through last Fold slot.
    pub minimum_lifetime_slots: u64,
    /// One-time non-reward Begin charge.
    pub begin_charge: u64,
    /// Non-reward charge for each Fold call.
    pub fold_base_charge: u64,
    /// Non-reward charge for each folded record.
    pub fold_per_record_charge: u64,
    /// Worker reward for each Fold call.
    pub fold_base_reward: u64,
    /// Worker reward for each folded record.
    pub fold_per_record_reward: u64,
    /// Non-reward successful Finalize charge.
    pub finalize_charge: u64,
    /// Successful finalizer reward.
    pub finalize_reward: u64,
    /// Non-reward permitted Abort charge.
    pub abort_charge: u64,
    /// Permitted aborter reward.
    pub abort_reward: u64,
}

impl ResolutionWorkCostScheduleV1 {
    /// Validate the exact schedule version and live account size.
    pub fn validate(&self) -> Result<()> {
        if self.version != RESOLUTION_WORK_COST_VERSION_V1 {
            return Err(ResolutionWorkCodecError::WrongVersion);
        }
        if self.work_state_bytes as usize != RESOLUTION_WORK_ACCOUNT_BYTES
            || self.rent_reserve == 0
            || self.minimum_lifetime_slots == 0
        {
            return Err(ResolutionWorkCodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Worst-case deposit when every archive record is folded alone.
    pub fn minimum_deposit(&self, record_count: u8) -> Result<u64> {
        self.validate()?;
        if record_count == 0 || record_count > SOURCE_ARCHIVE_MAX_RECORDS_V1 {
            return Err(ResolutionWorkCodecError::InvalidCount);
        }
        let singleton = checked_sum4(
            self.fold_base_charge,
            self.fold_per_record_charge,
            self.fold_base_reward,
            self.fold_per_record_reward,
        )?;
        let folds = singleton
            .checked_mul(u64::from(record_count))
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        let finalize = self
            .finalize_charge
            .checked_add(self.finalize_reward)
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        let abort = self
            .abort_charge
            .checked_add(self.abort_reward)
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        checked_sum4(
            self.rent_reserve,
            self.begin_charge,
            folds,
            core::cmp::max(finalize, abort),
        )
    }
}

/// Active prepaid accounting. Terminal refunds are absent because Work closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionWorkFundingV1 {
    /// Original payer deposit, including rent.
    pub deposited: u64,
    /// Rent still locked in the active Work account.
    pub rent_locked: u64,
    /// Work budget remaining for charges, rewards, and eventual refund.
    pub prepaid_remaining: u64,
    /// Cumulative non-reward charges already paid.
    pub charges_paid: u64,
    /// Cumulative caller rewards already paid.
    pub rewards_paid: u64,
}

/// Sole persisted, active owner of one resumable occupation accumulator.
///
/// The basis artifact and digest are both frozen. The layout validates the
/// canonical artifact envelope and its redundant outcome/denominator fields;
/// the live adapter must additionally run the authoritative BasisSpec decoder
/// and recompute `basis_spec_digest`. It must similarly recompute every other
/// stored digest from the authenticated immutable accounts at Begin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionWorkAccountV1 {
    /// Canonical Work identity/PDA.
    pub work_id: [u8; HASH_BYTES],
    /// Original payer and sole refund recipient.
    pub payer: [u8; HASH_BYTES],
    /// Segregated prepaid reserve/vault identity.
    pub prepaid_reserve: [u8; HASH_BYTES],
    /// Payer-selected replay-separating nonce.
    pub work_nonce: [u8; HASH_BYTES],
    /// Market identity.
    pub market: [u8; HASH_BYTES],
    /// Complete immutable Terms digest.
    pub terms_digest: [u8; HASH_BYTES],
    /// Sole canonical Resolution V4 target.
    pub resolution_target: [u8; HASH_BYTES],
    /// Exact expected owner of Work and sealed archive.
    pub program_owner: [u8; HASH_BYTES],
    /// Exact immutable sealed SourceArchive PDA.
    pub archive_account: [u8; HASH_BYTES],
    /// Digest of the complete canonical basis artifact.
    pub basis_spec_digest: [u8; HASH_BYTES],
    /// Digest of the authenticated immutable SourceSpec.
    pub source_spec_digest: [u8; HASH_BYTES],
    /// Full stored commitment of the sealed SourceArchive.
    pub archive_commitment: [u8; HASH_BYTES],
    /// Canonical archive-domain digest.
    pub archive_domain_digest: [u8; HASH_BYTES],
    /// Equal-duration grid identity.
    pub grid_identity: [u8; HASH_BYTES],
    /// Exact canonical BasisSpec V1 artifact bytes.
    pub basis_spec_artifact: [u8; BASIS_SPEC_BYTES_V1],
    /// Frozen archive repair generation.
    pub archive_generation: u64,
    /// Frozen equal bucket duration in seconds.
    pub bucket_duration: u64,
    /// Inclusive first archive bucket.
    pub start_bucket: u64,
    /// Exclusive final archive bucket.
    pub end_bucket_exclusive: u64,
    /// Slot at which Begin committed this state.
    pub opened_slot: u64,
    /// Last slot in which a new Fold may execute, inclusive.
    pub expires_slot: u64,
    /// Slot of Begin or the most recent successful Fold.
    pub last_progress_slot: u64,
    /// Exact next archive bucket to process.
    pub next_bucket: u64,
    /// Number of successful non-empty Fold calls.
    pub fold_count: u64,
    /// Last progress slot exactly when complete; zero while incomplete.
    pub completion_slot: u64,
    /// Accepted plus explicit-gap buckets accumulated so far.
    pub sample_count: u64,
    /// Accepted buckets accumulated so far.
    pub coverage_count: u64,
    /// Common native payout denominator.
    pub denominator: u64,
    /// Program-owned exact accumulator masses with canonical inactive padding.
    pub masses: [u128; MAX_OUTCOMES],
    /// Complete frozen charge/reward/rent schedule.
    pub costs: ResolutionWorkCostScheduleV1,
    /// Digest of the complete canonical cost schedule.
    pub cost_schedule_digest: [u8; HASH_BYTES],
    /// Exact active prepaid ledger.
    pub funding: ResolutionWorkFundingV1,
    /// Exactly [`WORK_STATUS_ACTIVE`]; terminal transitions close this account.
    pub status: u8,
    /// Exact-only or largest-remainder averaging.
    pub finalization_mode: u8,
    /// Active mass prefix length.
    pub outcome_count: u8,
    /// Total sealed archive record count.
    pub archive_record_count: u8,
    /// Frozen point-evaluator version.
    pub basis_evaluator_version: u16,
    /// Frozen summary version.
    pub occupation_summary_version: u16,
    /// Sole target Resolution layout version.
    pub resolution_version: u16,
    /// Work PDA bump.
    pub stored_bump: u8,
    /// Prepaid reserve PDA bump.
    pub reserve_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
    /// Reserved canonical zero bytes.
    pub reserved: [u8; 3],
}

impl ResolutionWorkAccountV1 {
    /// Validate all self-contained identity, geometry, mass, slot, and ledger invariants.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            &self.work_id,
            &self.payer,
            &self.prepaid_reserve,
            &self.work_nonce,
            &self.market,
            &self.terms_digest,
            &self.resolution_target,
            &self.program_owner,
            &self.archive_account,
            &self.basis_spec_digest,
            &self.source_spec_digest,
            &self.archive_commitment,
            &self.archive_domain_digest,
            &self.grid_identity,
            &self.cost_schedule_digest,
        ] {
            require_identity(identity)?;
        }
        if self.status != WORK_STATUS_ACTIVE {
            return Err(ResolutionWorkCodecError::InvalidEnum);
        }
        if !matches!(
            self.finalization_mode,
            FINALIZATION_EXACT_ONLY | FINALIZATION_LARGEST_REMAINDER_V1
        ) {
            return Err(ResolutionWorkCodecError::InvalidEnum);
        }
        if self.basis_evaluator_version != BASIS_EVALUATOR_VERSION_V1
            || self.occupation_summary_version != OCCUPATION_SUMMARY_VERSION_V1
            || self.resolution_version != OCCUPATION_RESOLUTION_VERSION_V4
        {
            return Err(ResolutionWorkCodecError::WrongVersion);
        }
        if self.flags != 0 || self.reserved != [0; 3] {
            return Err(ResolutionWorkCodecError::NonCanonicalPadding);
        }
        self.validate_window_and_progress()?;
        validate_basis_artifact(
            &self.basis_spec_artifact,
            self.outcome_count,
            self.denominator,
        )?;
        self.validate_masses()?;
        self.validate_costs_and_funding()
    }

    fn validate_window_and_progress(&self) -> Result<()> {
        if self.bucket_duration == 0
            || self.start_bucket >= self.end_bucket_exclusive
            || self.archive_record_count == 0
            || self.archive_record_count > SOURCE_ARCHIVE_MAX_RECORDS_V1
        {
            return Err(ResolutionWorkCodecError::InvalidWindow);
        }
        let span = self
            .end_bucket_exclusive
            .checked_sub(self.start_bucket)
            .ok_or(ResolutionWorkCodecError::InvalidWindow)?;
        if span != u64::from(self.archive_record_count)
            || self.next_bucket < self.start_bucket
            || self.next_bucket > self.end_bucket_exclusive
        {
            return Err(ResolutionWorkCodecError::MismatchedBinding);
        }
        let expected_samples = self
            .next_bucket
            .checked_sub(self.start_bucket)
            .ok_or(ResolutionWorkCodecError::InvalidCount)?;
        if self.sample_count != expected_samples || self.coverage_count > self.sample_count {
            return Err(ResolutionWorkCodecError::InvalidCount);
        }
        if self.last_progress_slot < self.opened_slot || self.last_progress_slot > self.expires_slot
        {
            return Err(ResolutionWorkCodecError::InvalidWindow);
        }
        let lifetime = self
            .expires_slot
            .checked_sub(self.opened_slot)
            .ok_or(ResolutionWorkCodecError::InvalidWindow)?;
        if lifetime < self.costs.minimum_lifetime_slots {
            return Err(ResolutionWorkCodecError::InvalidWindow);
        }
        if self.sample_count == 0 {
            if self.fold_count != 0
                || self.last_progress_slot != self.opened_slot
                || self.completion_slot != 0
            {
                return Err(ResolutionWorkCodecError::MismatchedBinding);
            }
        } else if self.fold_count == 0 || self.fold_count > self.sample_count {
            return Err(ResolutionWorkCodecError::InvalidCount);
        }
        if self.next_bucket == self.end_bucket_exclusive {
            if self.completion_slot != self.last_progress_slot {
                return Err(ResolutionWorkCodecError::MismatchedBinding);
            }
        } else if self.completion_slot != 0 {
            return Err(ResolutionWorkCodecError::MismatchedBinding);
        }
        Ok(())
    }

    fn validate_masses(&self) -> Result<()> {
        if !(2..=MAX_OUTCOMES as u8).contains(&self.outcome_count) || self.denominator == 0 {
            return Err(ResolutionWorkCodecError::InvalidCount);
        }
        let mut total = 0_u128;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            if index < usize::from(self.outcome_count) {
                total = total
                    .checked_add(self.masses[index])
                    .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
            } else if self.masses[index] != 0 {
                return Err(ResolutionWorkCodecError::NonCanonicalPadding);
            }
            index += 1;
        }
        let expected = u128::from(self.denominator)
            .checked_mul(u128::from(self.coverage_count))
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        if total != expected {
            return Err(ResolutionWorkCodecError::MismatchedBinding);
        }
        Ok(())
    }

    fn validate_costs_and_funding(&self) -> Result<()> {
        self.costs.validate()?;
        if self.funding.rent_locked != self.costs.rent_reserve
            || self.funding.deposited < self.costs.minimum_deposit(self.archive_record_count)?
        {
            return Err(ResolutionWorkCodecError::Underfunded);
        }
        let expected_charges = self
            .costs
            .fold_base_charge
            .checked_mul(self.fold_count)
            .and_then(|value| {
                self.costs
                    .fold_per_record_charge
                    .checked_mul(self.sample_count)
                    .and_then(|records| value.checked_add(records))
            })
            .and_then(|value| value.checked_add(self.costs.begin_charge))
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        let expected_rewards = self
            .costs
            .fold_base_reward
            .checked_mul(self.fold_count)
            .and_then(|value| {
                self.costs
                    .fold_per_record_reward
                    .checked_mul(self.sample_count)
                    .and_then(|records| value.checked_add(records))
            })
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        if self.funding.charges_paid != expected_charges
            || self.funding.rewards_paid != expected_rewards
        {
            return Err(ResolutionWorkCodecError::MismatchedBinding);
        }
        let accounted = self
            .funding
            .rent_locked
            .checked_add(self.funding.prepaid_remaining)
            .and_then(|value| value.checked_add(self.funding.charges_paid))
            .and_then(|value| value.checked_add(self.funding.rewards_paid))
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        if accounted != self.funding.deposited {
            return Err(ResolutionWorkCodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exactly [`RESOLUTION_WORK_ACCOUNT_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < RESOLUTION_WORK_ACCOUNT_BYTES {
            return Err(ResolutionWorkCodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        writer.u8(RESOLUTION_WORK_ACCOUNT_TAG)?;
        writer.u8(RESOLUTION_WORK_ACCOUNT_VERSION)?;
        writer.hash(&self.work_id)?;
        writer.hash(&self.payer)?;
        writer.hash(&self.prepaid_reserve)?;
        writer.hash(&self.work_nonce)?;
        writer.hash(&self.market)?;
        writer.hash(&self.terms_digest)?;
        writer.hash(&self.resolution_target)?;
        writer.hash(&self.program_owner)?;
        writer.hash(&self.archive_account)?;
        writer.hash(&self.basis_spec_digest)?;
        writer.hash(&self.source_spec_digest)?;
        writer.hash(&self.archive_commitment)?;
        writer.hash(&self.archive_domain_digest)?;
        writer.hash(&self.grid_identity)?;
        writer.bytes(&self.basis_spec_artifact)?;
        writer.u64(self.archive_generation)?;
        writer.u64(self.bucket_duration)?;
        writer.u64(self.start_bucket)?;
        writer.u64(self.end_bucket_exclusive)?;
        writer.u64(self.opened_slot)?;
        writer.u64(self.expires_slot)?;
        writer.u64(self.last_progress_slot)?;
        writer.u64(self.next_bucket)?;
        writer.u64(self.fold_count)?;
        writer.u64(self.completion_slot)?;
        writer.u64(self.sample_count)?;
        writer.u64(self.coverage_count)?;
        writer.u64(self.denominator)?;
        for mass in self.masses {
            writer.u128(mass)?;
        }
        encode_costs(&mut writer, &self.costs)?;
        writer.hash(&self.cost_schedule_digest)?;
        encode_funding(&mut writer, &self.funding)?;
        writer.u8(self.status)?;
        writer.u8(self.finalization_mode)?;
        writer.u8(self.outcome_count)?;
        writer.u8(self.archive_record_count)?;
        writer.u16(self.basis_evaluator_version)?;
        writer.u16(self.occupation_summary_version)?;
        writer.u16(self.resolution_version)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.reserve_bump)?;
        writer.u8(self.flags)?;
        writer.bytes(&self.reserved)?;
        if writer.at != RESOLUTION_WORK_ACCOUNT_BYTES {
            return Err(ResolutionWorkCodecError::MismatchedBinding);
        }
        Ok(writer.at)
    }

    /// Decode exactly one active ResolutionWork account.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            RESOLUTION_WORK_ACCOUNT_TAG,
            RESOLUTION_WORK_ACCOUNT_VERSION,
            RESOLUTION_WORK_ACCOUNT_BYTES,
        )?;
        let mut basis_spec_artifact = [0; BASIS_SPEC_BYTES_V1];
        let work_id = reader.hash()?;
        let payer = reader.hash()?;
        let prepaid_reserve = reader.hash()?;
        let work_nonce = reader.hash()?;
        let market = reader.hash()?;
        let terms_digest = reader.hash()?;
        let resolution_target = reader.hash()?;
        let program_owner = reader.hash()?;
        let archive_account = reader.hash()?;
        let basis_spec_digest = reader.hash()?;
        let source_spec_digest = reader.hash()?;
        let archive_commitment = reader.hash()?;
        let archive_domain_digest = reader.hash()?;
        let grid_identity = reader.hash()?;
        reader.copy_into(&mut basis_spec_artifact)?;
        let archive_generation = reader.u64()?;
        let bucket_duration = reader.u64()?;
        let start_bucket = reader.u64()?;
        let end_bucket_exclusive = reader.u64()?;
        let opened_slot = reader.u64()?;
        let expires_slot = reader.u64()?;
        let last_progress_slot = reader.u64()?;
        let next_bucket = reader.u64()?;
        let fold_count = reader.u64()?;
        let completion_slot = reader.u64()?;
        let sample_count = reader.u64()?;
        let coverage_count = reader.u64()?;
        let denominator = reader.u64()?;
        let mut masses = [0; MAX_OUTCOMES];
        for mass in &mut masses {
            *mass = reader.u128()?;
        }
        let costs = decode_costs(&mut reader)?;
        let cost_schedule_digest = reader.hash()?;
        let funding = decode_funding(&mut reader)?;
        let value = Self {
            work_id,
            payer,
            prepaid_reserve,
            work_nonce,
            market,
            terms_digest,
            resolution_target,
            program_owner,
            archive_account,
            basis_spec_digest,
            source_spec_digest,
            archive_commitment,
            archive_domain_digest,
            grid_identity,
            basis_spec_artifact,
            archive_generation,
            bucket_duration,
            start_bucket,
            end_bucket_exclusive,
            opened_slot,
            expires_slot,
            last_progress_slot,
            next_bucket,
            fold_count,
            completion_slot,
            sample_count,
            coverage_count,
            denominator,
            masses,
            costs,
            cost_schedule_digest,
            funding,
            status: reader.u8()?,
            finalization_mode: reader.u8()?,
            outcome_count: reader.u8()?,
            archive_record_count: reader.u8()?,
            basis_evaluator_version: reader.u16()?,
            occupation_summary_version: reader.u16()?,
            resolution_version: reader.u16()?,
            stored_bump: reader.u8()?,
            reserve_bump: reader.u8()?,
            flags: reader.u8()?,
            reserved: reader.array3()?,
        };
        reader.done()?;
        value.validate()?;
        Ok(value)
    }
}

/// Begin instruction body. All other bindings come from authenticated accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginResolutionWorkV1 {
    /// Unique payer-selected work nonce.
    pub work_nonce: [u8; HASH_BYTES],
    /// Exact final averaging rule.
    pub finalization_mode: u8,
    /// Inclusive final Fold slot.
    pub expires_slot: u64,
    /// Exact deposit the payer authorizes and the adapter must transfer.
    pub declared_deposit: u64,
    /// Digest of the release-pinned schedule supplied through program policy.
    pub cost_schedule_digest: [u8; HASH_BYTES],
}

impl BeginResolutionWorkV1 {
    /// Encode exact Begin instruction data.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        require_identity(&self.work_nonce)?;
        require_identity(&self.cost_schedule_digest)?;
        if !matches!(
            self.finalization_mode,
            FINALIZATION_EXACT_ONLY | FINALIZATION_LARGEST_REMAINDER_V1
        ) {
            return Err(ResolutionWorkCodecError::InvalidEnum);
        }
        if self.declared_deposit == 0 {
            return Err(ResolutionWorkCodecError::Underfunded);
        }
        let mut writer =
            intent_writer(out, BEGIN_RESOLUTION_WORK_TAG, BEGIN_RESOLUTION_WORK_BYTES)?;
        writer.hash(&self.work_nonce)?;
        writer.u8(self.finalization_mode)?;
        writer.u64(self.expires_slot)?;
        writer.u64(self.declared_deposit)?;
        writer.hash(&self.cost_schedule_digest)?;
        Ok(writer.at)
    }

    /// Decode exact Begin instruction data.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            BEGIN_RESOLUTION_WORK_TAG,
            RESOLUTION_WORK_INTENT_VERSION,
            BEGIN_RESOLUTION_WORK_BYTES,
        )?;
        let value = Self {
            work_nonce: reader.hash()?,
            finalization_mode: reader.u8()?,
            expires_slot: reader.u64()?,
            declared_deposit: reader.u64()?,
            cost_schedule_digest: reader.hash()?,
        };
        reader.done()?;
        let mut canonical = [0; BEGIN_RESOLUTION_WORK_BYTES];
        value.encode(&mut canonical)?;
        Ok(value)
    }
}

/// Fold instruction body. It carries no record bytes, proof, point, mass, or vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldResolutionWorkV1 {
    /// Expected active Work identity.
    pub work_id: [u8; HASH_BYTES],
    /// Exact sealed SourceArchive account bound at Begin.
    pub archive_account: [u8; HASH_BYTES],
    /// Full stored sealed-archive commitment bound at Begin.
    pub archive_commitment: [u8; HASH_BYTES],
    /// Exact optimistic next cursor.
    pub expected_cursor: u64,
    /// Nonzero bounded contiguous record count.
    pub record_count: u8,
}

impl FoldResolutionWorkV1 {
    /// Encode exact Fold instruction data.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        require_identity(&self.work_id)?;
        require_identity(&self.archive_account)?;
        require_identity(&self.archive_commitment)?;
        if self.record_count == 0 || self.record_count > MAX_FOLD_RECORDS_V1 {
            return Err(ResolutionWorkCodecError::InvalidCount);
        }
        let mut writer = intent_writer(out, FOLD_RESOLUTION_WORK_TAG, FOLD_RESOLUTION_WORK_BYTES)?;
        writer.hash(&self.work_id)?;
        writer.hash(&self.archive_account)?;
        writer.hash(&self.archive_commitment)?;
        writer.u64(self.expected_cursor)?;
        writer.u8(self.record_count)?;
        Ok(writer.at)
    }

    /// Decode exact Fold instruction data.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(
            input,
            FOLD_RESOLUTION_WORK_TAG,
            RESOLUTION_WORK_INTENT_VERSION,
            FOLD_RESOLUTION_WORK_BYTES,
        )?;
        let value = Self {
            work_id: reader.hash()?,
            archive_account: reader.hash()?,
            archive_commitment: reader.hash()?,
            expected_cursor: reader.u64()?,
            record_count: reader.u8()?,
        };
        reader.done()?;
        let mut canonical = [0; FOLD_RESOLUTION_WORK_BYTES];
        value.encode(&mut canonical)?;
        Ok(value)
    }
}

/// Finalize instruction body. The payout vector is computed internally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizeResolutionWorkV1 {
    /// Expected active Work identity.
    pub work_id: [u8; HASH_BYTES],
    /// Exact optimistic end cursor.
    pub expected_cursor: u64,
    /// Full stored sealed-archive commitment bound at Begin.
    pub expected_archive_commitment: [u8; HASH_BYTES],
}

impl FinalizeResolutionWorkV1 {
    /// Encode exact Finalize instruction data.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        encode_terminal(
            out,
            FINALIZE_RESOLUTION_WORK_TAG,
            self.work_id,
            self.expected_cursor,
            self.expected_archive_commitment,
        )
    }

    /// Decode exact Finalize instruction data.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let (work_id, expected_cursor, expected_archive_commitment) = decode_terminal(
            input,
            FINALIZE_RESOLUTION_WORK_TAG,
            FINALIZE_RESOLUTION_WORK_BYTES,
        )?;
        Ok(Self {
            work_id,
            expected_cursor,
            expected_archive_commitment,
        })
    }
}

/// Abort instruction body. Abort cannot write or select a Resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbortResolutionWorkV1 {
    /// Expected active Work identity.
    pub work_id: [u8; HASH_BYTES],
    /// Exact optimistic current cursor.
    pub expected_cursor: u64,
    /// Full stored sealed-archive commitment bound at Begin.
    pub expected_archive_commitment: [u8; HASH_BYTES],
}

impl AbortResolutionWorkV1 {
    /// Encode exact Abort instruction data.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        encode_terminal(
            out,
            ABORT_RESOLUTION_WORK_TAG,
            self.work_id,
            self.expected_cursor,
            self.expected_archive_commitment,
        )
    }

    /// Decode exact Abort instruction data.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let (work_id, expected_cursor, expected_archive_commitment) = decode_terminal(
            input,
            ABORT_RESOLUTION_WORK_TAG,
            ABORT_RESOLUTION_WORK_BYTES,
        )?;
        Ok(Self {
            work_id,
            expected_cursor,
            expected_archive_commitment,
        })
    }
}

fn validate_basis_artifact(
    bytes: &[u8; BASIS_SPEC_BYTES_V1],
    outcome_count: u8,
    denominator: u64,
) -> Result<()> {
    if bytes[..8] != BASIS_MAGIC_V1
        || read_u16_at(bytes, 8) != BASIS_SCHEMA_VERSION_V1
        || read_u16_at(bytes, 10) != BASIS_EVALUATOR_VERSION_V1
        || bytes[12] != BASIS_SEMANTIC_NATIVE_BSPLINE
    {
        return Err(ResolutionWorkCodecError::WrongVersion);
    }
    if bytes[13] != outcome_count || read_u64_at(bytes, 24) != denominator {
        return Err(ResolutionWorkCodecError::MismatchedBinding);
    }
    if bytes[18..24].iter().any(|byte| *byte != 0) {
        return Err(ResolutionWorkCodecError::NonCanonicalPadding);
    }
    let degree = bytes[14];
    let knot_count = usize::from(bytes[15]);
    if degree > 3 || !(2..=MAX_OUTCOMES as u8).contains(&outcome_count) || denominator == 0 {
        return Err(ResolutionWorkCodecError::InvalidCount);
    }
    let expected_knots = match degree {
        0 => usize::from(outcome_count) - 1,
        1 => usize::from(outcome_count),
        _ => usize::from(outcome_count) + 1 - usize::from(degree),
    };
    if knot_count != expected_knots || knot_count == 0 || knot_count > MAX_OUTCOMES {
        return Err(ResolutionWorkCodecError::InvalidCount);
    }
    if degree >= 1 && knot_count < 2 {
        return Err(ResolutionWorkCodecError::InvalidCount);
    }
    let uniform = bytes[16];
    if (degree >= 2 && uniform == UNIFORM_SPACING_NONE)
        || uniform >= 128 && uniform != UNIFORM_SPACING_NONE
    {
        return Err(ResolutionWorkCodecError::InvalidEnum);
    }
    if !matches!(bytes[17], BASIS_EDGE_CLAMP | BASIS_EDGE_REFUSE) {
        return Err(ResolutionWorkCodecError::InvalidEnum);
    }
    if read_u128_at(bytes, 32) == 0 {
        return Err(ResolutionWorkCodecError::InvalidWindow);
    }
    let mut previous = 0_u128;
    let mut index = 0_usize;
    while index < MAX_OUTCOMES {
        let knot = read_u128_at(bytes, 48 + (index * 16));
        if index < knot_count {
            if index == 0 {
                if degree == 0 && knot == 0 {
                    return Err(ResolutionWorkCodecError::InvalidWindow);
                }
            } else {
                if knot <= previous {
                    return Err(ResolutionWorkCodecError::InvalidWindow);
                }
                if uniform != UNIFORM_SPACING_NONE && knot - previous != (1_u128 << uniform) {
                    return Err(ResolutionWorkCodecError::MismatchedBinding);
                }
            }
            previous = knot;
        } else if knot != 0 {
            return Err(ResolutionWorkCodecError::NonCanonicalPadding);
        }
        index += 1;
    }
    Ok(())
}

fn encode_costs(writer: &mut Writer<'_>, value: &ResolutionWorkCostScheduleV1) -> Result<()> {
    writer.u16(value.version)?;
    writer.u32(value.work_state_bytes)?;
    writer.u64(value.rent_reserve)?;
    writer.u64(value.minimum_lifetime_slots)?;
    writer.u64(value.begin_charge)?;
    writer.u64(value.fold_base_charge)?;
    writer.u64(value.fold_per_record_charge)?;
    writer.u64(value.fold_base_reward)?;
    writer.u64(value.fold_per_record_reward)?;
    writer.u64(value.finalize_charge)?;
    writer.u64(value.finalize_reward)?;
    writer.u64(value.abort_charge)?;
    writer.u64(value.abort_reward)
}

fn decode_costs(reader: &mut Reader<'_>) -> Result<ResolutionWorkCostScheduleV1> {
    Ok(ResolutionWorkCostScheduleV1 {
        version: reader.u16()?,
        work_state_bytes: reader.u32()?,
        rent_reserve: reader.u64()?,
        minimum_lifetime_slots: reader.u64()?,
        begin_charge: reader.u64()?,
        fold_base_charge: reader.u64()?,
        fold_per_record_charge: reader.u64()?,
        fold_base_reward: reader.u64()?,
        fold_per_record_reward: reader.u64()?,
        finalize_charge: reader.u64()?,
        finalize_reward: reader.u64()?,
        abort_charge: reader.u64()?,
        abort_reward: reader.u64()?,
    })
}

fn encode_funding(writer: &mut Writer<'_>, value: &ResolutionWorkFundingV1) -> Result<()> {
    writer.u64(value.deposited)?;
    writer.u64(value.rent_locked)?;
    writer.u64(value.prepaid_remaining)?;
    writer.u64(value.charges_paid)?;
    writer.u64(value.rewards_paid)
}

fn decode_funding(reader: &mut Reader<'_>) -> Result<ResolutionWorkFundingV1> {
    Ok(ResolutionWorkFundingV1 {
        deposited: reader.u64()?,
        rent_locked: reader.u64()?,
        prepaid_remaining: reader.u64()?,
        charges_paid: reader.u64()?,
        rewards_paid: reader.u64()?,
    })
}

fn intent_writer<'a>(out: &'a mut [u8], tag: u8, exact: usize) -> Result<Writer<'a>> {
    if out.len() < exact {
        return Err(ResolutionWorkCodecError::OutputTooSmall);
    }
    let mut writer = Writer::new(out);
    writer.u8(tag)?;
    writer.u8(RESOLUTION_WORK_INTENT_VERSION)?;
    Ok(writer)
}

fn encode_terminal(
    out: &mut [u8],
    tag: u8,
    work_id: [u8; HASH_BYTES],
    cursor: u64,
    commitment: [u8; HASH_BYTES],
) -> Result<usize> {
    require_identity(&work_id)?;
    require_identity(&commitment)?;
    let exact = if tag == FINALIZE_RESOLUTION_WORK_TAG {
        FINALIZE_RESOLUTION_WORK_BYTES
    } else {
        ABORT_RESOLUTION_WORK_BYTES
    };
    let mut writer = intent_writer(out, tag, exact)?;
    writer.hash(&work_id)?;
    writer.u64(cursor)?;
    writer.hash(&commitment)?;
    Ok(writer.at)
}

fn decode_terminal(
    input: &[u8],
    tag: u8,
    exact: usize,
) -> Result<([u8; HASH_BYTES], u64, [u8; HASH_BYTES])> {
    let mut reader = Reader::new(input, tag, RESOLUTION_WORK_INTENT_VERSION, exact)?;
    let work_id = reader.hash()?;
    let cursor = reader.u64()?;
    let commitment = reader.hash()?;
    reader.done()?;
    require_identity(&work_id)?;
    require_identity(&commitment)?;
    Ok((work_id, cursor, commitment))
}

fn require_identity(value: &[u8; HASH_BYTES]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(ResolutionWorkCodecError::ZeroIdentity);
    }
    Ok(())
}

fn checked_sum4(a: u64, b: u64, c: u64, d: u64) -> Result<u64> {
    a.checked_add(b)
        .and_then(|value| value.checked_add(c))
        .and_then(|value| value.checked_add(d))
        .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)
}

fn read_u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u64_at(bytes: &[u8], at: usize) -> u64 {
    let mut value = [0; 8];
    value.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(value)
}

fn read_u128_at(bytes: &[u8], at: usize) -> u128 {
    let mut value = [0; 16];
    value.copy_from_slice(&bytes[at..at + 16]);
    u128::from_le_bytes(value)
}

#[derive(Debug)]
struct Writer<'a> {
    out: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    const fn new(out: &'a mut [u8]) -> Self {
        Self { out, at: 0 }
    }

    fn bytes(&mut self, value: &[u8]) -> Result<()> {
        let end = self
            .at
            .checked_add(value.len())
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        let target = self
            .out
            .get_mut(self.at..end)
            .ok_or(ResolutionWorkCodecError::OutputTooSmall)?;
        target.copy_from_slice(value);
        self.at = end;
        Ok(())
    }

    fn hash(&mut self, value: &[u8; HASH_BYTES]) -> Result<()> {
        self.bytes(value)
    }

    fn u8(&mut self, value: u8) -> Result<()> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
}

#[derive(Debug)]
struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8], tag: u8, version: u8, exact: usize) -> Result<Self> {
        if input.len() < exact {
            return Err(ResolutionWorkCodecError::Truncated);
        }
        if input.len() > exact {
            return Err(ResolutionWorkCodecError::TrailingBytes);
        }
        if input[0] != tag {
            return Err(ResolutionWorkCodecError::WrongTag);
        }
        if input[1] != version {
            return Err(ResolutionWorkCodecError::WrongVersion);
        }
        Ok(Self { input, at: 2 })
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(len)
            .ok_or(ResolutionWorkCodecError::ArithmeticOverflow)?;
        let value = self
            .input
            .get(self.at..end)
            .ok_or(ResolutionWorkCodecError::Truncated)?;
        self.at = end;
        Ok(value)
    }

    fn copy_into(&mut self, out: &mut [u8]) -> Result<()> {
        out.copy_from_slice(self.take(out.len())?);
        Ok(())
    }

    fn hash(&mut self) -> Result<[u8; HASH_BYTES]> {
        let mut value = [0; HASH_BYTES];
        self.copy_into(&mut value)?;
        Ok(value)
    }

    fn array3(&mut self) -> Result<[u8; 3]> {
        let mut value = [0; 3];
        self.copy_into(&mut value)?;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        let mut value = [0; 2];
        self.copy_into(&mut value)?;
        Ok(u16::from_le_bytes(value))
    }

    fn u32(&mut self) -> Result<u32> {
        let mut value = [0; 4];
        self.copy_into(&mut value)?;
        Ok(u32::from_le_bytes(value))
    }

    fn u64(&mut self) -> Result<u64> {
        let mut value = [0; 8];
        self.copy_into(&mut value)?;
        Ok(u64::from_le_bytes(value))
    }

    fn u128(&mut self) -> Result<u128> {
        let mut value = [0; 16];
        self.copy_into(&mut value)?;
        Ok(u128::from_le_bytes(value))
    }

    fn done(&self) -> Result<()> {
        if self.at != self.input.len() {
            return Err(ResolutionWorkCodecError::TrailingBytes);
        }
        Ok(())
    }
}

const _: () = assert!(RESOLUTION_WORK_ACCOUNT_BYTES == 1_296);
const _: () = assert!(BEGIN_RESOLUTION_WORK_BYTES == 83);
const _: () = assert!(FOLD_RESOLUTION_WORK_BYTES == 107);
const _: () = assert!(FINALIZE_RESOLUTION_WORK_BYTES == 74);
const _: () = assert!(ABORT_RESOLUTION_WORK_BYTES == 74);
