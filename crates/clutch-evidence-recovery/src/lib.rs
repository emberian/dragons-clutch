// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Pure runtime projection for evidence-only recovery.
//!
//! [`clutch_product_series`] remains the sole owner of reusable recovery-policy
//! semantics and its checked relative-to-absolute schedule compiler. This
//! crate accepts the compiler's [`CompiledScheduleV1`] together with its typed
//! policy and market identities, and consumes the authoritative
//! [`SeriesFundingQuoteV1`] rather than restating progress prices. It owns only
//! mutable runtime phase, progress, and lamport conservation. An adapter must
//! prove that each supplied schedule is the exact lowering of the authenticated
//! Market and policy.
//!
//! [`RecoveryClock`] and [`EvidenceDecision`] are also adapter boundaries. This
//! crate does not parse a Clock sysvar, map Unix time to source buckets,
//! authenticate source evidence, or compute a payout.

mod external;

pub use external::{
    ExternalRecoveryAdmissionV1, ExternalRecoveryFundingV1, ExternalRecoveryStateV1,
    ExternalRecoveryTransitionPlanV1, ExternalRecoveryWorkAuthorizationV1,
    EXTERNAL_RECOVERY_STATE_V1_BYTES,
};

pub use clutch_product_series::{
    AbsoluteRecoveryAttemptV1, CompiledScheduleV1, ComponentDebitV1, EvidenceOnlyRecoveryPolicyId,
    MarketInstanceId, MarketInstanceV2Id, RecoveryAttemptFundingV1, SeriesFundingQuoteId,
    SeriesFundingQuoteV1, MAX_RECOVERY_ATTEMPTS,
};

/// Fixed width of every non-artifact semantic identity.
pub const IDENTITY_BYTES: usize = 32;
/// Exact canonical width of one persisted recovery state.
pub const RECOVERY_STATE_V2_BYTES: usize = 1_016;

const RECOVERY_STATE_V2_MAGIC: [u8; 8] = *b"DCRECST2";
const RECOVERY_STATE_V2_SCHEMA: u16 = 2;

/// Recovery transition result.
pub type Result<T> = core::result::Result<T, RecoveryError>;

/// Opaque identity authenticated by an adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct Identity([u8; IDENTITY_BYTES]);

impl Identity {
    /// Reserved padding identity.
    pub const ZERO: Self = Self([0; IDENTITY_BYTES]);

    /// Construct an opaque identity from fixed bytes.
    pub const fn from_bytes(bytes: [u8; IDENTITY_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return the fixed identity bytes.
    pub const fn bytes(self) -> [u8; IDENTITY_BYTES] {
        self.0
    }

    /// Whether this is reserved padding.
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

/// Adapter-authenticated numeric Clock projection.
///
/// `current_bucket` must be derived exactly from the authenticated source grid
/// and canonical Clock. The kernel checks monotonicity and immutable bucket
/// windows but cannot verify that mapping itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct RecoveryClock {
    /// Monotone Clock slot.
    pub slot: u64,
    /// Exact signed Clock Unix timestamp.
    pub unix_timestamp: i64,
    /// Exact current canonical source-grid bucket.
    pub current_bucket: u64,
}

/// Immutable identities and rent principal admitted with a reserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct RecoveryAdmission {
    /// Typed FundingQuote identity authenticated through the Series attachment.
    pub series_funding_quote_id: SeriesFundingQuoteId,
    /// Canonical logical recovery-instance and reserve identity.
    pub state_id: Identity,
    /// Adapter-authenticated nonzero state generation.
    pub generation: u64,
    /// Owner of unused work principal after evidence success.
    pub work_funder: Identity,
    /// Owner of the reserve's rent principal.
    pub rent_payer: Identity,
    /// Neutral disposition role; the adapter must bind it to the canonical sink.
    pub neutral_sink: Identity,
}

/// Exact funding deltas observed during admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct FundingObservation {
    /// Reserve balance before exact payer transfers; entirely donation.
    pub reserve_balance_before: u64,
    /// Reserve balance after both exact payer transfers.
    pub reserve_balance_after: u64,
    /// Exact work-funder debit.
    pub work_funder_debit_lamports: u64,
    /// Exact rent-payer debit.
    pub rent_payer_debit_lamports: u64,
}

/// Opaque identity for evidence accepted by an external adapter.
///
/// Construction checks only nonzeroness and makes no authenticity, freshness,
/// source, Terms, or payout claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct EvidenceDecision(Identity);

impl EvidenceDecision {
    /// Bind a nonzero adapter-supplied evidence-decision identity.
    pub fn from_adapter(identity: Identity) -> Result<Self> {
        require_live(identity)?;
        Ok(Self(identity))
    }

    /// Return the opaque decision identity.
    pub const fn identity(self) -> Identity {
        self.0
    }
}

/// Mutable recovery phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RecoveryPhase {
    /// Before primary evidence maturity.
    Active = 0,
    /// Finite compiled recovery windows remain.
    DegradedRecoverable = 1,
    /// Every compiled window closed and the work reserve was neutralized.
    RecoveryDormant = 2,
    /// An adapter accepted one evidence decision.
    Resolved = 3,
}

/// Terminal disposition of the finite reserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReserveDisposition {
    /// Principal remains live.
    Open = 0,
    /// Evidence success refunded unused work principal.
    Success = 1,
    /// Final schedule expiry neutralized unused work principal.
    Dormancy = 2,
}

/// Deterministic refusal from the pure runtime projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryError {
    /// A persisted recovery state was not the one exact canonical width.
    WrongLength,
    /// A persisted recovery state used another discriminator.
    BadMagic,
    /// A persisted recovery state used another schema version.
    BadVersion,
    /// Reserved persisted bytes were nonzero.
    NonCanonicalReserved,
    /// A persisted enum discriminant was unknown.
    InvalidEnum,
    /// A required identity is zero.
    ZeroIdentity,
    /// A generation is zero.
    ZeroGeneration,
    /// The compiled schedule is malformed as a runtime projection.
    InvalidScheduleProjection,
    /// The funding projection is malformed.
    InvalidFundingProjection,
    /// A policy, schedule, or funding identity/count does not match.
    ProjectionMismatch,
    /// Inactive fixed-array padding is nonzero.
    NonCanonicalPadding,
    /// Admission occurred at or after primary evidence maturity.
    AdmissionAfterPrimaryMaturity,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// A payer aliases the neutral sink.
    InterestedNeutralSink,
    /// The recovery reserve aliases one of its transfer recipients.
    StateRecipientAlias,
    /// Exact individual funding deltas do not match.
    FundingDeltaMismatch,
    /// Physical reserve lamports are one or more short.
    ReserveBalanceShortfall,
    /// The transition targeted the wrong phase.
    WrongPhase,
    /// Slot, Unix timestamp, or canonical bucket moved backwards.
    ClockMovedBackwards,
    /// The first compiled recovery window has not opened.
    RecoveryNotOpen,
    /// No current compiled attempt window is eligible.
    AttemptNotOpen,
    /// Accepted progress was zero, replayed, or moved backwards.
    NonmonotoneProgress,
    /// Accepted progress exceeds the FundingQuote cap for this attempt.
    ProgressLimitExceeded,
    /// The work reserve cannot cover an exact accepted-progress payment.
    WorkPrincipalShortfall,
    /// A plan no longer matches the complete current state.
    StalePlan,
    /// The exact post-transfer reserve balance differs from the plan.
    PostBalanceMismatch,
    /// A successor semantic state attempted to duplicate liveness custody.
    ExternalCustodyMismatch,
    /// A scheduled liveness debit could not cover the exact semantic reward.
    InvalidScheduledCeiling,
    /// The independently funded liveness call budget was exhausted.
    ExternalCallBudgetExhausted,
    /// New exposure is closed by phase or the current authenticated bucket.
    ExposureClosed,
    /// A private reachable-state or conservation invariant failed.
    InvariantViolation,
}

/// Closed economic-market identity generation admitted by the recovery core.
///
/// The variants remain typed rather than treating two equal 32-byte digests
/// from different market-preimage domains as interchangeable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryMarketIdentity {
    /// Legacy V1 market-preimage identity.
    Legacy(MarketInstanceId),
    /// Full-width V2 market-preimage identity used by `SeriesPlanV5`.
    Successor(MarketInstanceV2Id),
}

impl RecoveryMarketIdentity {
    fn validate(self) -> Result<()> {
        match self {
            Self::Legacy(id) => id.validate(),
            Self::Successor(id) => id.validate(),
        }
        .map_err(|_| RecoveryError::ZeroIdentity)
    }
}

/// One semantic transfer compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Transfer {
    /// Recipient role, or zero exactly when amount is zero.
    pub recipient: Identity,
    /// Exact lamports.
    pub lamports: u64,
}

impl Transfer {
    const NONE: Self = Self {
        recipient: Identity::ZERO,
        lamports: 0,
    };

    fn new(recipient: Identity, lamports: u64) -> Self {
        if lamports == 0 {
            Self::NONE
        } else {
            Self {
                recipient,
                lamports,
            }
        }
    }

    fn validate(self) -> Result<()> {
        if (self.lamports == 0) != self.recipient.is_zero() {
            Err(RecoveryError::InvariantViolation)
        } else {
            Ok(())
        }
    }
}

/// Exact transfer compartments for one transition.
///
/// Recipient roles may alias. An adapter must aggregate all compartments by
/// authenticated destination and verify each final recipient delta exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct TransferPlan {
    /// Exact reward for newly accepted progress.
    pub accepted_progress_reward: Transfer,
    /// Unused work principal returned after success.
    pub work_funder_refund: Transfer,
    /// Rent principal returned after reserve disposition.
    pub rent_payer_refund: Transfer,
    /// Donations and, on dormancy, unused work principal.
    pub neutral_sink_transfer: Transfer,
}

impl TransferPlan {
    const NONE: Self = Self {
        accepted_progress_reward: Transfer::NONE,
        work_funder_refund: Transfer::NONE,
        rent_payer_refund: Transfer::NONE,
        neutral_sink_transfer: Transfer::NONE,
    };

    /// Checked total debit from the reserve across all compartments.
    pub fn total_lamports(self) -> Result<u64> {
        self.validate()?;
        checked_add(
            checked_add(
                self.accepted_progress_reward.lamports,
                self.work_funder_refund.lamports,
            )?,
            checked_add(
                self.rent_payer_refund.lamports,
                self.neutral_sink_transfer.lamports,
            )?,
        )
    }

    fn validate(self) -> Result<()> {
        self.accepted_progress_reward.validate()?;
        self.work_funder_refund.validate()?;
        self.rent_payer_refund.validate()?;
        self.neutral_sink_transfer.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
struct ActiveWork {
    work_id: Identity,
    reward_recipient: Identity,
    attempt_index: u8,
}

impl ActiveWork {
    const EMPTY: Self = Self {
        work_id: Identity::ZERO,
        reward_recipient: Identity::ZERO,
        attempt_index: 0,
    };

    fn is_empty(self) -> bool {
        self == Self::EMPTY
    }
}

/// Exact accounting snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct RecoveryLedger {
    /// Initial FundingQuote work principal.
    pub work_initial: u64,
    /// Work principal still live.
    pub work_remaining: u64,
    /// Cumulative accepted-progress rewards.
    pub accepted_progress_paid: u64,
    /// Unused work principal refunded on success.
    pub success_refunded: u64,
    /// Unused work principal neutralized after final close.
    pub dormancy_neutralized: u64,
    /// Initial rent principal.
    pub rent_initial: u64,
    /// Rent principal still live.
    pub rent_remaining: u64,
    /// Rent principal returned to its payer.
    pub rent_refunded: u64,
    /// Cumulative unsolicited donations observed.
    pub donations_received: u128,
    /// Donations still live.
    pub donations_remaining: u128,
    /// Donations transferred to the neutral sink.
    pub donations_neutralized: u128,
}

/// Complete fixed-memory runtime state.
///
/// This is not a persisted account codec or stable Rust memory ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct RecoveryState {
    market_identity: RecoveryMarketIdentity,
    schedule: CompiledScheduleV1,
    series_funding_quote_id: SeriesFundingQuoteId,
    funding_quote: SeriesFundingQuoteV1,
    state_id: Identity,
    generation: u64,
    work_funder: Identity,
    rent_payer: Identity,
    neutral_sink: Identity,
    resolution_evidence_id: Identity,
    phase: RecoveryPhase,
    reserve_disposition: ReserveDisposition,
    transition_nonce: u64,
    last_clock: RecoveryClock,
    next_attempt_index: u8,
    accepted_progress_units: [u64; MAX_RECOVERY_ATTEMPTS],
    active_work: ActiveWork,
    work_initial: u64,
    work_remaining: u64,
    accepted_progress_paid: u64,
    success_refunded: u64,
    dormancy_neutralized: u64,
    rent_initial: u64,
    rent_remaining: u64,
    rent_refunded: u64,
    donations_received: u128,
    donations_remaining: u128,
    donations_neutralized: u128,
}

impl RecoveryState {
    /// Admit exact product-compiler output and its authoritative FundingQuote.
    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        market_instance_id: MarketInstanceId,
        recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
        schedule: CompiledScheduleV1,
        funding_quote: SeriesFundingQuoteV1,
        admission: RecoveryAdmission,
        creation_clock: RecoveryClock,
        observation: FundingObservation,
    ) -> Result<Self> {
        Self::admit_market(
            RecoveryMarketIdentity::Legacy(market_instance_id),
            recovery_policy_id,
            schedule,
            funding_quote,
            admission,
            creation_clock,
            observation,
        )
    }

    /// Admit a full-width V2 occurrence compiled from one `SeriesPlanV5`
    /// ordinal without converting it to the legacy market-identity domain.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_v2(
        market_instance_id: MarketInstanceV2Id,
        recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
        schedule: CompiledScheduleV1,
        funding_quote: SeriesFundingQuoteV1,
        admission: RecoveryAdmission,
        creation_clock: RecoveryClock,
        observation: FundingObservation,
    ) -> Result<Self> {
        Self::admit_market(
            RecoveryMarketIdentity::Successor(market_instance_id),
            recovery_policy_id,
            schedule,
            funding_quote,
            admission,
            creation_clock,
            observation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn admit_market(
        market_identity: RecoveryMarketIdentity,
        recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
        schedule: CompiledScheduleV1,
        funding_quote: SeriesFundingQuoteV1,
        admission: RecoveryAdmission,
        creation_clock: RecoveryClock,
        observation: FundingObservation,
    ) -> Result<Self> {
        market_identity.validate()?;
        recovery_policy_id
            .validate()
            .map_err(|_| RecoveryError::ZeroIdentity)?;
        admission
            .series_funding_quote_id
            .validate()
            .map_err(|_| RecoveryError::ZeroIdentity)?;
        validate_schedule(&schedule)?;
        validate_funding_quote_projection(&funding_quote, recovery_policy_id, &schedule)?;
        let funding_quote_id = funding_quote.id().map_err(map_funding_quote_error)?;
        if funding_quote_id != admission.series_funding_quote_id {
            return Err(RecoveryError::ProjectionMismatch);
        }
        let work_principal = funding_quote
            .recovery_work_principal_lamports()
            .map_err(map_funding_quote_error)?;
        let rent_principal = funding_quote.recovery_rent_principal_lamports;
        if creation_clock.current_bucket >= schedule.primary_maturity_bucket_exclusive {
            return Err(RecoveryError::AdmissionAfterPrimaryMaturity);
        }
        require_live(admission.state_id)?;
        require_live(admission.work_funder)?;
        require_live(admission.rent_payer)?;
        require_live(admission.neutral_sink)?;
        if admission.generation == 0 {
            return Err(RecoveryError::ZeroGeneration);
        }
        if admission.work_funder == admission.neutral_sink
            || admission.rent_payer == admission.neutral_sink
        {
            return Err(RecoveryError::InterestedNeutralSink);
        }
        if admission.state_id == admission.work_funder
            || admission.state_id == admission.rent_payer
            || admission.state_id == admission.neutral_sink
        {
            return Err(RecoveryError::StateRecipientAlias);
        }
        if observation.work_funder_debit_lamports != work_principal
            || observation.rent_payer_debit_lamports != rent_principal
        {
            return Err(RecoveryError::FundingDeltaMismatch);
        }
        let payer_total = checked_add(
            observation.work_funder_debit_lamports,
            observation.rent_payer_debit_lamports,
        )?;
        let expected_after = checked_add(observation.reserve_balance_before, payer_total)?;
        if expected_after != observation.reserve_balance_after {
            return Err(RecoveryError::FundingDeltaMismatch);
        }
        let state = Self {
            market_identity,
            schedule,
            series_funding_quote_id: admission.series_funding_quote_id,
            funding_quote,
            state_id: admission.state_id,
            generation: admission.generation,
            work_funder: admission.work_funder,
            rent_payer: admission.rent_payer,
            neutral_sink: admission.neutral_sink,
            resolution_evidence_id: Identity::ZERO,
            phase: RecoveryPhase::Active,
            reserve_disposition: ReserveDisposition::Open,
            transition_nonce: 0,
            last_clock: creation_clock,
            next_attempt_index: 0,
            accepted_progress_units: [0; MAX_RECOVERY_ATTEMPTS],
            active_work: ActiveWork::EMPTY,
            work_initial: work_principal,
            work_remaining: work_principal,
            accepted_progress_paid: 0,
            success_refunded: 0,
            dormancy_neutralized: 0,
            rent_initial: rent_principal,
            rent_remaining: rent_principal,
            rent_refunded: 0,
            donations_received: u128::from(observation.reserve_balance_before),
            donations_remaining: u128::from(observation.reserve_balance_before),
            donations_neutralized: 0,
        };
        state.check()?;
        if state.required_open_balance()? != observation.reserve_balance_after {
            return Err(RecoveryError::InvariantViolation);
        }
        Ok(state)
    }

    /// Typed economic Market identity whose compiler output is bound.
    pub const fn market_identity(&self) -> RecoveryMarketIdentity {
        self.market_identity
    }

    /// Legacy Market identity, absent for a successor occurrence.
    pub const fn market_instance_id(&self) -> Option<MarketInstanceId> {
        match self.market_identity {
            RecoveryMarketIdentity::Legacy(id) => Some(id),
            RecoveryMarketIdentity::Successor(_) => None,
        }
    }

    /// Full-width V2 Market identity, absent for a legacy occurrence.
    pub const fn market_instance_v2_id(&self) -> Option<MarketInstanceV2Id> {
        match self.market_identity {
            RecoveryMarketIdentity::Legacy(_) => None,
            RecoveryMarketIdentity::Successor(id) => Some(id),
        }
    }

    /// Exact recovery-reserve logical identity.
    pub const fn state_id(&self) -> Identity {
        self.state_id
    }

    /// Exact work-principal owner recorded at admission.
    pub const fn work_funder(&self) -> Identity {
        self.work_funder
    }

    /// Exact rent-principal owner recorded at admission.
    pub const fn rent_payer(&self) -> Identity {
        self.rent_payer
    }

    /// Neutral sink role recorded at admission.
    pub const fn neutral_sink(&self) -> Identity {
        self.neutral_sink
    }

    /// Sole reusable recovery-policy identity.
    pub const fn recovery_policy_id(&self) -> EvidenceOnlyRecoveryPolicyId {
        self.funding_quote.evidence_only_recovery_policy_id
    }

    /// Separate operational FundingQuote identity.
    pub const fn series_funding_quote_id(&self) -> SeriesFundingQuoteId {
        self.series_funding_quote_id
    }

    /// Exact absolute source/recovery schedule admitted at creation.
    pub const fn schedule(&self) -> CompiledScheduleV1 {
        self.schedule
    }

    /// Encode the complete canonical recovery state for a persisted adapter.
    ///
    /// The codec remains owned here so an account adapter cannot create a
    /// parallel mutable budget or phase truth.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.check()?;
        let mut writer = StateWriter::new(output)?;
        writer.bytes(&RECOVERY_STATE_V2_MAGIC)?;
        writer.u16(RECOVERY_STATE_V2_SCHEMA)?;
        writer.u8(match self.market_identity {
            RecoveryMarketIdentity::Legacy(_) => 1,
            RecoveryMarketIdentity::Successor(_) => 2,
        })?;
        writer.u8(self.phase as u8)?;
        writer.u8(self.reserve_disposition as u8)?;
        writer.u8(self.next_attempt_index)?;
        writer.reserved(2)?;
        writer.bytes(&match self.market_identity {
            RecoveryMarketIdentity::Legacy(id) => id.bytes(),
            RecoveryMarketIdentity::Successor(id) => id.bytes(),
        })?;
        writer.u64(self.schedule.start_bucket)?;
        writer.u64(self.schedule.end_bucket_exclusive)?;
        writer.u64(self.schedule.primary_maturity_bucket_exclusive)?;
        writer.u8(self.schedule.recovery_attempt_count)?;
        writer.reserved(7)?;
        for attempt in self.schedule.recovery_attempts {
            writer.u64(attempt.repair_generation)?;
            writer.u64(attempt.opens_at_bucket)?;
            writer.u64(attempt.closes_at_bucket)?;
        }
        writer.bytes(&self.series_funding_quote_id.bytes())?;
        let mut quote = [0; clutch_product_series::SERIES_FUNDING_QUOTE_BYTES];
        clutch_product_series::FixedCodec::encode_into(&self.funding_quote, &mut quote)
            .map_err(map_funding_quote_error)?;
        writer.bytes(&quote)?;
        for identity in [
            self.state_id,
            self.work_funder,
            self.rent_payer,
            self.neutral_sink,
            self.resolution_evidence_id,
        ] {
            writer.bytes(&identity.bytes())?;
        }
        writer.u64(self.generation)?;
        writer.u64(self.transition_nonce)?;
        writer.u64(self.last_clock.slot)?;
        writer.i64(self.last_clock.unix_timestamp)?;
        writer.u64(self.last_clock.current_bucket)?;
        for progress in self.accepted_progress_units {
            writer.u64(progress)?;
        }
        writer.bytes(&self.active_work.work_id.bytes())?;
        writer.bytes(&self.active_work.reward_recipient.bytes())?;
        writer.u8(self.active_work.attempt_index)?;
        writer.reserved(7)?;
        for value in [
            self.work_initial,
            self.work_remaining,
            self.accepted_progress_paid,
            self.success_refunded,
            self.dormancy_neutralized,
            self.rent_initial,
            self.rent_remaining,
            self.rent_refunded,
        ] {
            writer.u64(value)?;
        }
        writer.u128(self.donations_received)?;
        writer.u128(self.donations_remaining)?;
        writer.u128(self.donations_neutralized)?;
        writer.finish()
    }

    /// Decode and fully validate one complete canonical persisted state.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = StateReader::new(input)?;
        if reader.bytes::<8>()? != RECOVERY_STATE_V2_MAGIC {
            return Err(RecoveryError::BadMagic);
        }
        if reader.u16()? != RECOVERY_STATE_V2_SCHEMA {
            return Err(RecoveryError::BadVersion);
        }
        let market_kind = reader.u8()?;
        let phase = decode_phase(reader.u8()?)?;
        let reserve_disposition = decode_disposition(reader.u8()?)?;
        let next_attempt_index = reader.u8()?;
        reader.reserved(2)?;
        let market_bytes = reader.bytes::<IDENTITY_BYTES>()?;
        let market_identity = match market_kind {
            1 => RecoveryMarketIdentity::Legacy(MarketInstanceId::from_bytes(market_bytes)),
            2 => RecoveryMarketIdentity::Successor(MarketInstanceV2Id::from_bytes(market_bytes)),
            _ => return Err(RecoveryError::InvalidEnum),
        };
        let start_bucket = reader.u64()?;
        let end_bucket_exclusive = reader.u64()?;
        let primary_maturity_bucket_exclusive = reader.u64()?;
        let recovery_attempt_count = reader.u8()?;
        reader.reserved(7)?;
        let mut recovery_attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        let mut attempt_index = 0_usize;
        while attempt_index < MAX_RECOVERY_ATTEMPTS {
            recovery_attempts[attempt_index] = AbsoluteRecoveryAttemptV1 {
                repair_generation: reader.u64()?,
                opens_at_bucket: reader.u64()?,
                closes_at_bucket: reader.u64()?,
            };
            attempt_index += 1;
        }
        let schedule = CompiledScheduleV1 {
            start_bucket,
            end_bucket_exclusive,
            primary_maturity_bucket_exclusive,
            recovery_attempt_count,
            recovery_attempts,
        };
        let series_funding_quote_id = SeriesFundingQuoteId::from_bytes(reader.bytes()?);
        let quote = reader.bytes::<{ clutch_product_series::SERIES_FUNDING_QUOTE_BYTES }>()?;
        let funding_quote =
            <SeriesFundingQuoteV1 as clutch_product_series::FixedCodec>::decode(&quote)
                .map_err(map_funding_quote_error)?;
        let state_id = Identity::from_bytes(reader.bytes()?);
        let work_funder = Identity::from_bytes(reader.bytes()?);
        let rent_payer = Identity::from_bytes(reader.bytes()?);
        let neutral_sink = Identity::from_bytes(reader.bytes()?);
        let resolution_evidence_id = Identity::from_bytes(reader.bytes()?);
        let generation = reader.u64()?;
        let transition_nonce = reader.u64()?;
        let last_clock = RecoveryClock {
            slot: reader.u64()?,
            unix_timestamp: reader.i64()?,
            current_bucket: reader.u64()?,
        };
        let mut accepted_progress_units = [0; MAX_RECOVERY_ATTEMPTS];
        let mut progress_index = 0_usize;
        while progress_index < MAX_RECOVERY_ATTEMPTS {
            accepted_progress_units[progress_index] = reader.u64()?;
            progress_index += 1;
        }
        let active_work = ActiveWork {
            work_id: Identity::from_bytes(reader.bytes()?),
            reward_recipient: Identity::from_bytes(reader.bytes()?),
            attempt_index: reader.u8()?,
        };
        reader.reserved(7)?;
        let state = Self {
            market_identity,
            schedule,
            series_funding_quote_id,
            funding_quote,
            state_id,
            generation,
            work_funder,
            rent_payer,
            neutral_sink,
            resolution_evidence_id,
            phase,
            reserve_disposition,
            transition_nonce,
            last_clock,
            next_attempt_index,
            accepted_progress_units,
            active_work,
            work_initial: reader.u64()?,
            work_remaining: reader.u64()?,
            accepted_progress_paid: reader.u64()?,
            success_refunded: reader.u64()?,
            dormancy_neutralized: reader.u64()?,
            rent_initial: reader.u64()?,
            rent_remaining: reader.u64()?,
            rent_refunded: reader.u64()?,
            donations_received: reader.u128()?,
            donations_remaining: reader.u128()?,
            donations_neutralized: reader.u128()?,
        };
        reader.finish()?;
        state.check()?;
        Ok(state)
    }

    /// Adapter-authenticated generation binding.
    ///
    /// Freshness and nonreuse remain adapter/tombstone obligations.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Current recovery phase.
    pub const fn phase(&self) -> RecoveryPhase {
        self.phase
    }

    /// Reserve disposition.
    pub const fn reserve_disposition(&self) -> ReserveDisposition {
        self.reserve_disposition
    }

    /// Monotone plan nonce.
    pub const fn transition_nonce(&self) -> u64 {
        self.transition_nonce
    }

    /// Last admitted numeric Clock projection.
    pub const fn last_clock(&self) -> RecoveryClock {
        self.last_clock
    }

    /// First compiled attempt not yet closed by authenticated bucket time.
    pub const fn next_attempt_index(&self) -> u8 {
        self.next_attempt_index
    }

    /// Current compiled source-generation/open/close attempt, if one remains.
    pub fn current_attempt(&self) -> Result<Option<AbsoluteRecoveryAttemptV1>> {
        self.check()?;
        let index = usize::from(self.next_attempt_index);
        if index < usize::from(self.schedule.recovery_attempt_count) {
            Ok(Some(self.schedule.recovery_attempts[index]))
        } else {
            Ok(None)
        }
    }

    /// Current accepted-progress total for one attempt index.
    pub fn accepted_progress_units(&self, attempt_index: u8) -> Result<u64> {
        self.check()?;
        let index = usize::from(attempt_index);
        if index >= usize::from(self.schedule.recovery_attempt_count) {
            return Err(RecoveryError::ProjectionMismatch);
        }
        Ok(self.accepted_progress_units[index])
    }

    /// Exact FundingQuote row for one active compiled attempt.
    pub fn attempt_funding(&self, attempt_index: u8) -> Result<RecoveryAttemptFundingV1> {
        self.check()?;
        let index = usize::from(attempt_index);
        if index >= usize::from(self.schedule.recovery_attempt_count) {
            return Err(RecoveryError::ProjectionMismatch);
        }
        Ok(self.funding_quote.recovery_attempt_funding[index])
    }

    /// Active Work identity, if any.
    pub fn active_work_id(&self) -> Option<Identity> {
        if self.active_work.is_empty() {
            None
        } else {
            Some(self.active_work.work_id)
        }
    }

    /// Opaque evidence-decision identity, if resolved.
    pub fn resolution_evidence_id(&self) -> Option<Identity> {
        if self.resolution_evidence_id.is_zero() {
            None
        } else {
            Some(self.resolution_evidence_id)
        }
    }

    /// Fixed accounting snapshot.
    pub const fn ledger(&self) -> RecoveryLedger {
        RecoveryLedger {
            work_initial: self.work_initial,
            work_remaining: self.work_remaining,
            accepted_progress_paid: self.accepted_progress_paid,
            success_refunded: self.success_refunded,
            dormancy_neutralized: self.dormancy_neutralized,
            rent_initial: self.rent_initial,
            rent_remaining: self.rent_remaining,
            rent_refunded: self.rent_refunded,
            donations_received: self.donations_received,
            donations_remaining: self.donations_remaining,
            donations_neutralized: self.donations_neutralized,
        }
    }

    /// Refuse new exposure using phase and the current authenticated bucket.
    ///
    /// This closes the crank-lag gap: `Active` alone is never sufficient once
    /// the first compiled recovery window has opened.
    pub fn check_new_exposure(&self, clock: RecoveryClock) -> Result<()> {
        self.check()?;
        self.validate_next_clock(clock)?;
        if self.phase != RecoveryPhase::Active
            || clock.current_bucket >= self.schedule.primary_maturity_bucket_exclusive
        {
            return Err(RecoveryError::ExposureClosed);
        }
        Ok(())
    }

    /// Validate all private reachable-state and conservation invariants.
    pub fn check(&self) -> Result<()> {
        self.market_identity
            .validate()
            .map_err(|_| RecoveryError::InvariantViolation)?;
        self.recovery_policy_id()
            .validate()
            .map_err(|_| RecoveryError::InvariantViolation)?;
        validate_schedule(&self.schedule)?;
        validate_funding_quote_projection(
            &self.funding_quote,
            self.recovery_policy_id(),
            &self.schedule,
        )?;
        self.series_funding_quote_id
            .validate()
            .map_err(|_| RecoveryError::InvariantViolation)?;
        if self.funding_quote.id().map_err(map_funding_quote_error)? != self.series_funding_quote_id
        {
            return Err(RecoveryError::ProjectionMismatch);
        }
        let quoted_work = self
            .funding_quote
            .recovery_work_principal_lamports()
            .map_err(map_funding_quote_error)?;
        require_live(self.state_id)?;
        require_live(self.work_funder)?;
        require_live(self.rent_payer)?;
        require_live(self.neutral_sink)?;
        if self.generation == 0
            || self.work_funder == self.neutral_sink
            || self.rent_payer == self.neutral_sink
            || self.state_id == self.work_funder
            || self.state_id == self.rent_payer
            || self.state_id == self.neutral_sink
            || self.work_initial != quoted_work
            || self.rent_initial != self.funding_quote.recovery_rent_principal_lamports
        {
            return Err(RecoveryError::InvariantViolation);
        }
        let work_sum = checked_add(
            checked_add(self.work_remaining, self.accepted_progress_paid)?,
            checked_add(self.success_refunded, self.dormancy_neutralized)?,
        )?;
        if work_sum != self.work_initial
            || checked_add(self.rent_remaining, self.rent_refunded)? != self.rent_initial
            || checked_add_u128(self.donations_remaining, self.donations_neutralized)?
                != self.donations_received
        {
            return Err(RecoveryError::InvariantViolation);
        }
        let count = usize::from(self.schedule.recovery_attempt_count);
        let mut expected_paid = 0_u64;
        let mut index = 0_usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let progress = self.accepted_progress_units[index];
            if index < count {
                let terms = self.funding_quote.recovery_attempt_funding[index];
                if progress > terms.max_progress_units {
                    return Err(RecoveryError::InvariantViolation);
                }
                expected_paid = checked_add(
                    expected_paid,
                    progress
                        .checked_mul(terms.lamports_per_progress_unit)
                        .ok_or(RecoveryError::ArithmeticOverflow)?,
                )?;
            } else if progress != 0 {
                return Err(RecoveryError::InvariantViolation);
            }
            index += 1;
        }
        if expected_paid != self.accepted_progress_paid {
            return Err(RecoveryError::InvariantViolation);
        }
        let cursor = usize::from(self.next_attempt_index);
        if cursor > count {
            return Err(RecoveryError::InvariantViolation);
        }
        let cursor_attempt = if cursor < count {
            Some(self.schedule.recovery_attempts[cursor])
        } else {
            None
        };
        let mut expired = 0_usize;
        while expired < cursor {
            if self.last_clock.current_bucket
                < self.schedule.recovery_attempts[expired].closes_at_bucket
            {
                return Err(RecoveryError::InvariantViolation);
            }
            expired += 1;
        }
        if !self.active_work.is_empty() {
            require_live(self.active_work.work_id)?;
            require_live(self.active_work.reward_recipient)?;
            if self.phase != RecoveryPhase::DegradedRecoverable
                || usize::from(self.active_work.attempt_index) != cursor
                || self.active_work.reward_recipient == self.state_id
                || self.active_work.reward_recipient == self.neutral_sink
            {
                return Err(RecoveryError::InvariantViolation);
            }
            let attempt = cursor_attempt.ok_or(RecoveryError::InvariantViolation)?;
            if self.accepted_progress_units[cursor] == 0 {
                return Err(RecoveryError::InvariantViolation);
            }
            if self.last_clock.current_bucket < attempt.opens_at_bucket
                || self.last_clock.current_bucket >= attempt.closes_at_bucket
            {
                return Err(RecoveryError::InvariantViolation);
            }
        }

        match self.phase {
            RecoveryPhase::Active => {
                if self.reserve_disposition != ReserveDisposition::Open
                    || cursor != 0
                    || self.last_clock.current_bucket
                        >= self.schedule.primary_maturity_bucket_exclusive
                    || !self.active_work.is_empty()
                    || self.accepted_progress_paid != 0
                    || self.success_refunded != 0
                    || self.dormancy_neutralized != 0
                    || self.rent_refunded != 0
                    || self.donations_neutralized != 0
                    || !self.resolution_evidence_id.is_zero()
                {
                    return Err(RecoveryError::InvariantViolation);
                }
            }
            RecoveryPhase::DegradedRecoverable => {
                let attempt = cursor_attempt.ok_or(RecoveryError::InvariantViolation)?;
                if self.reserve_disposition != ReserveDisposition::Open
                    || self.last_clock.current_bucket
                        < self.schedule.primary_maturity_bucket_exclusive
                    || self.last_clock.current_bucket >= attempt.closes_at_bucket
                    || self.success_refunded != 0
                    || self.dormancy_neutralized != 0
                    || self.rent_refunded != 0
                    || self.donations_neutralized != 0
                    || !self.resolution_evidence_id.is_zero()
                {
                    return Err(RecoveryError::InvariantViolation);
                }
            }
            RecoveryPhase::RecoveryDormant => {
                if self.reserve_disposition != ReserveDisposition::Dormancy
                    || cursor != count
                    || !self.active_work.is_empty()
                    || self.work_remaining != 0
                    || self.success_refunded != 0
                    || self.rent_remaining != 0
                    || self.donations_remaining != 0
                    || !self.resolution_evidence_id.is_zero()
                {
                    return Err(RecoveryError::InvariantViolation);
                }
            }
            RecoveryPhase::Resolved => {
                if self.reserve_disposition == ReserveDisposition::Open
                    || !self.active_work.is_empty()
                    || self.work_remaining != 0
                    || self.rent_remaining != 0
                    || self.donations_remaining != 0
                    || self.resolution_evidence_id.is_zero()
                {
                    return Err(RecoveryError::InvariantViolation);
                }
                match self.reserve_disposition {
                    ReserveDisposition::Success => {
                        let attempt = cursor_attempt.ok_or(RecoveryError::InvariantViolation)?;
                        if self.dormancy_neutralized != 0
                            || self.last_clock.current_bucket >= attempt.closes_at_bucket
                        {
                            return Err(RecoveryError::InvariantViolation);
                        }
                    }
                    ReserveDisposition::Dormancy => {
                        if self.success_refunded != 0 || cursor != count {
                            return Err(RecoveryError::InvariantViolation);
                        }
                    }
                    ReserveDisposition::Open => return Err(RecoveryError::InvariantViolation),
                }
            }
        }
        Ok(())
    }

    /// Plan permissionless degradation at primary evidence maturity.
    ///
    /// If the call arrives after every finite window closed, the same plan
    /// immediately applies the dormancy disposition; crank delay cannot change
    /// residue ownership.
    pub fn plan_enter_degraded(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
    ) -> Result<TransitionPlan> {
        self.check()?;
        if self.phase != RecoveryPhase::Active {
            return Err(RecoveryError::WrongPhase);
        }
        self.validate_next_clock(clock)?;
        if clock.current_bucket < self.schedule.primary_maturity_bucket_exclusive {
            return Err(RecoveryError::RecoveryNotOpen);
        }
        let mut next = self.observe_open_balance(actual_reserve_balance)?;
        next.phase = RecoveryPhase::DegradedRecoverable;
        next.last_clock = clock;
        next.advance_expired_attempts()?;
        let transfers = if usize::from(next.next_attempt_index)
            == usize::from(next.schedule.recovery_attempt_count)
        {
            next.apply_dormancy_disposition()?
        } else {
            TransferPlan::NONE
        };
        self.make_plan(next, actual_reserve_balance, transfers)
    }

    /// Advance the immutable compiled schedule using authenticated bucket time.
    ///
    /// At the final exclusive close bucket this atomically neutralizes unused
    /// work and donations and returns rent principal.
    pub fn plan_advance_schedule(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
    ) -> Result<TransitionPlan> {
        self.check()?;
        if self.phase != RecoveryPhase::DegradedRecoverable {
            return Err(RecoveryError::WrongPhase);
        }
        self.validate_next_clock(clock)?;
        let mut next = self.observe_open_balance(actual_reserve_balance)?;
        next.last_clock = clock;
        next.advance_expired_attempts()?;
        let transfers = if usize::from(next.next_attempt_index)
            == usize::from(next.schedule.recovery_attempt_count)
        {
            next.apply_dormancy_disposition()?
        } else {
            TransferPlan::NONE
        };
        self.make_plan(next, actual_reserve_balance, transfers)
    }

    /// Plan exact payment for a strictly advancing accepted-progress cursor.
    ///
    /// The call both establishes/replaces the sole active Work and advances
    /// progress atomically. Therefore a zero-progress caller cannot squat the
    /// active-work slot, and any different Work with newly accepted progress
    /// can replace a stalled Work.
    pub fn plan_accept_work_progress(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        work_id: Identity,
        reward_recipient: Identity,
        accepted_progress_total: u64,
    ) -> Result<TransitionPlan> {
        self.check()?;
        if self.phase != RecoveryPhase::DegradedRecoverable {
            return Err(RecoveryError::WrongPhase);
        }
        self.validate_next_clock(clock)?;
        require_live(work_id)?;
        require_live(reward_recipient)?;
        self.validate_reward_recipient(reward_recipient)?;
        let mut next = self.observe_open_balance(actual_reserve_balance)?;
        next.last_clock = clock;
        next.advance_expired_attempts()?;
        let index = usize::from(next.next_attempt_index);
        let count = usize::from(next.schedule.recovery_attempt_count);
        if index >= count {
            return Err(RecoveryError::AttemptNotOpen);
        }
        let attempt = next.schedule.recovery_attempts[index];
        if clock.current_bucket < attempt.opens_at_bucket
            || clock.current_bucket >= attempt.closes_at_bucket
        {
            return Err(RecoveryError::AttemptNotOpen);
        }
        let prior = next.accepted_progress_units[index];
        if accepted_progress_total <= prior {
            return Err(RecoveryError::NonmonotoneProgress);
        }
        let terms = next.funding_quote.recovery_attempt_funding[index];
        if accepted_progress_total > terms.max_progress_units {
            return Err(RecoveryError::ProgressLimitExceeded);
        }
        let delta = accepted_progress_total - prior;
        let reward = delta
            .checked_mul(terms.lamports_per_progress_unit)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        if next.work_remaining < reward {
            return Err(RecoveryError::WorkPrincipalShortfall);
        }
        next.work_remaining -= reward;
        next.accepted_progress_paid = checked_add(next.accepted_progress_paid, reward)?;
        next.accepted_progress_units[index] = accepted_progress_total;
        next.active_work = ActiveWork {
            work_id,
            reward_recipient,
            attempt_index: next.next_attempt_index,
        };
        let transfers = TransferPlan {
            accepted_progress_reward: Transfer::new(reward_recipient, reward),
            ..TransferPlan::NONE
        };
        self.make_plan(next, actual_reserve_balance, transfers)
    }

    /// Plan caller-funded resolution after external evidence validation.
    ///
    /// Before final schedule expiry, unused work is a success refund. At or
    /// after the final exclusive close, this first applies dormancy ownership
    /// and then records the evidence decision. After dormancy, any hostile
    /// prefund is classified only as a new donation and neutralized; it never
    /// recreates work or rent principal.
    pub fn plan_resolve_caller_funded(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        evidence: EvidenceDecision,
    ) -> Result<TransitionPlan> {
        self.check()?;
        self.validate_next_clock(clock)?;
        if self.phase == RecoveryPhase::Resolved {
            return Err(RecoveryError::WrongPhase);
        }
        let mut next;
        let transfers;
        match self.phase {
            RecoveryPhase::Active => {
                next = self.observe_open_balance(actual_reserve_balance)?;
                next.last_clock = clock;
                if clock.current_bucket >= next.schedule.primary_maturity_bucket_exclusive {
                    next.phase = RecoveryPhase::DegradedRecoverable;
                    next.advance_expired_attempts()?;
                    if usize::from(next.next_attempt_index)
                        == usize::from(next.schedule.recovery_attempt_count)
                    {
                        transfers = next.apply_dormancy_disposition()?;
                    } else {
                        transfers = next.apply_success_disposition()?;
                    }
                } else {
                    transfers = next.apply_success_disposition()?;
                }
            }
            RecoveryPhase::DegradedRecoverable => {
                next = self.observe_open_balance(actual_reserve_balance)?;
                next.last_clock = clock;
                next.advance_expired_attempts()?;
                if usize::from(next.next_attempt_index)
                    == usize::from(next.schedule.recovery_attempt_count)
                {
                    transfers = next.apply_dormancy_disposition()?;
                } else {
                    transfers = next.apply_success_disposition()?;
                }
            }
            RecoveryPhase::RecoveryDormant => {
                next = *self;
                next.last_clock = clock;
                next.donations_received =
                    checked_add_u128(next.donations_received, u128::from(actual_reserve_balance))?;
                next.donations_neutralized = checked_add_u128(
                    next.donations_neutralized,
                    u128::from(actual_reserve_balance),
                )?;
                transfers = TransferPlan {
                    neutral_sink_transfer: Transfer::new(next.neutral_sink, actual_reserve_balance),
                    ..TransferPlan::NONE
                };
            }
            RecoveryPhase::Resolved => return Err(RecoveryError::WrongPhase),
        }
        next.phase = RecoveryPhase::Resolved;
        next.resolution_evidence_id = evidence.identity();
        self.make_plan(next, actual_reserve_balance, transfers)
    }

    /// Plan one final paid accepted-progress advance and evidence resolution.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_resolve_paid_progress(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        work_id: Identity,
        reward_recipient: Identity,
        accepted_progress_total: u64,
        evidence: EvidenceDecision,
    ) -> Result<TransitionPlan> {
        self.check()?;
        if self.phase != RecoveryPhase::DegradedRecoverable {
            return Err(RecoveryError::WrongPhase);
        }
        self.validate_next_clock(clock)?;
        require_live(work_id)?;
        require_live(reward_recipient)?;
        self.validate_reward_recipient(reward_recipient)?;
        let mut next = self.observe_open_balance(actual_reserve_balance)?;
        next.last_clock = clock;
        next.advance_expired_attempts()?;
        let index = usize::from(next.next_attempt_index);
        let count = usize::from(next.schedule.recovery_attempt_count);
        if index >= count {
            return Err(RecoveryError::AttemptNotOpen);
        }
        let attempt = next.schedule.recovery_attempts[index];
        if clock.current_bucket < attempt.opens_at_bucket
            || clock.current_bucket >= attempt.closes_at_bucket
        {
            return Err(RecoveryError::AttemptNotOpen);
        }
        let prior = next.accepted_progress_units[index];
        if accepted_progress_total <= prior {
            return Err(RecoveryError::NonmonotoneProgress);
        }
        let terms = next.funding_quote.recovery_attempt_funding[index];
        if accepted_progress_total > terms.max_progress_units {
            return Err(RecoveryError::ProgressLimitExceeded);
        }
        let reward = (accepted_progress_total - prior)
            .checked_mul(terms.lamports_per_progress_unit)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        if next.work_remaining < reward {
            return Err(RecoveryError::WorkPrincipalShortfall);
        }
        next.work_remaining -= reward;
        next.accepted_progress_paid = checked_add(next.accepted_progress_paid, reward)?;
        next.accepted_progress_units[index] = accepted_progress_total;
        next.active_work = ActiveWork {
            work_id,
            reward_recipient,
            attempt_index: next.next_attempt_index,
        };
        let reward_transfer = Transfer::new(reward_recipient, reward);
        next.phase = RecoveryPhase::Resolved;
        next.resolution_evidence_id = evidence.identity();
        let transfers = next.apply_success_disposition_with_reward(reward_transfer)?;
        self.make_plan(next, actual_reserve_balance, transfers)
    }

    fn validate_next_clock(&self, clock: RecoveryClock) -> Result<()> {
        if clock.slot < self.last_clock.slot
            || clock.unix_timestamp < self.last_clock.unix_timestamp
            || clock.current_bucket < self.last_clock.current_bucket
        {
            return Err(RecoveryError::ClockMovedBackwards);
        }
        Ok(())
    }

    fn validate_reward_recipient(&self, reward_recipient: Identity) -> Result<()> {
        if reward_recipient == self.state_id {
            return Err(RecoveryError::StateRecipientAlias);
        }
        if reward_recipient == self.neutral_sink {
            return Err(RecoveryError::InterestedNeutralSink);
        }
        Ok(())
    }

    fn advance_expired_attempts(&mut self) -> Result<()> {
        let count = usize::from(self.schedule.recovery_attempt_count);
        let mut index = usize::from(self.next_attempt_index);
        while index < count
            && self.last_clock.current_bucket
                >= self.schedule.recovery_attempts[index].closes_at_bucket
        {
            self.active_work = ActiveWork::EMPTY;
            index += 1;
        }
        self.next_attempt_index =
            u8::try_from(index).map_err(|_| RecoveryError::ArithmeticOverflow)?;
        Ok(())
    }

    fn observe_open_balance(&self, actual_balance: u64) -> Result<Self> {
        if self.reserve_disposition != ReserveDisposition::Open {
            return Err(RecoveryError::InvariantViolation);
        }
        let base = checked_add(self.work_remaining, self.rent_remaining)?;
        let donations = u64::try_from(self.donations_remaining)
            .map_err(|_| RecoveryError::InvariantViolation)?;
        let accounted = checked_add(base, donations)?;
        if actual_balance < accounted {
            return Err(RecoveryError::ReserveBalanceShortfall);
        }
        let newly_donated = actual_balance - accounted;
        let mut next = *self;
        next.donations_received =
            checked_add_u128(next.donations_received, u128::from(newly_donated))?;
        next.donations_remaining =
            checked_add_u128(next.donations_remaining, u128::from(newly_donated))?;
        Ok(next)
    }

    fn required_open_balance(&self) -> Result<u64> {
        let donations = u64::try_from(self.donations_remaining)
            .map_err(|_| RecoveryError::InvariantViolation)?;
        checked_add(
            checked_add(self.work_remaining, self.rent_remaining)?,
            donations,
        )
    }

    fn apply_success_disposition(&mut self) -> Result<TransferPlan> {
        self.apply_success_disposition_with_reward(Transfer::NONE)
    }

    fn apply_success_disposition_with_reward(&mut self, reward: Transfer) -> Result<TransferPlan> {
        let work_refund = self.work_remaining;
        self.work_remaining = 0;
        self.success_refunded = checked_add(self.success_refunded, work_refund)?;
        let rent_refund = self.rent_remaining;
        self.rent_remaining = 0;
        self.rent_refunded = checked_add(self.rent_refunded, rent_refund)?;
        let donation = u64::try_from(self.donations_remaining)
            .map_err(|_| RecoveryError::InvariantViolation)?;
        self.donations_remaining = 0;
        self.donations_neutralized =
            checked_add_u128(self.donations_neutralized, u128::from(donation))?;
        self.active_work = ActiveWork::EMPTY;
        self.reserve_disposition = ReserveDisposition::Success;
        Ok(TransferPlan {
            accepted_progress_reward: reward,
            work_funder_refund: Transfer::new(self.work_funder, work_refund),
            rent_payer_refund: Transfer::new(self.rent_payer, rent_refund),
            neutral_sink_transfer: Transfer::new(self.neutral_sink, donation),
        })
    }

    fn apply_dormancy_disposition(&mut self) -> Result<TransferPlan> {
        let unused_work = self.work_remaining;
        self.work_remaining = 0;
        self.dormancy_neutralized = checked_add(self.dormancy_neutralized, unused_work)?;
        let rent_refund = self.rent_remaining;
        self.rent_remaining = 0;
        self.rent_refunded = checked_add(self.rent_refunded, rent_refund)?;
        let donations = u64::try_from(self.donations_remaining)
            .map_err(|_| RecoveryError::InvariantViolation)?;
        self.donations_remaining = 0;
        self.donations_neutralized =
            checked_add_u128(self.donations_neutralized, u128::from(donations))?;
        self.active_work = ActiveWork::EMPTY;
        self.next_attempt_index = self.schedule.recovery_attempt_count;
        self.phase = RecoveryPhase::RecoveryDormant;
        self.reserve_disposition = ReserveDisposition::Dormancy;
        Ok(TransferPlan {
            rent_payer_refund: Transfer::new(self.rent_payer, rent_refund),
            neutral_sink_transfer: Transfer::new(
                self.neutral_sink,
                checked_add(unused_work, donations)?,
            ),
            ..TransferPlan::NONE
        })
    }

    fn make_plan(
        &self,
        mut next: Self,
        expected_pre_balance: u64,
        transfers: TransferPlan,
    ) -> Result<TransitionPlan> {
        transfers.validate()?;
        next.transition_nonce = self
            .transition_nonce
            .checked_add(1)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        next.check()?;
        let expected_post_balance = expected_pre_balance
            .checked_sub(transfers.total_lamports()?)
            .ok_or(RecoveryError::ReserveBalanceShortfall)?;
        let canonical_post = match next.reserve_disposition {
            ReserveDisposition::Open => next.required_open_balance()?,
            ReserveDisposition::Success | ReserveDisposition::Dormancy => 0,
        };
        if expected_post_balance != canonical_post {
            return Err(RecoveryError::InvariantViolation);
        }
        Ok(TransitionPlan {
            before: *self,
            after: next,
            expected_pre_balance,
            expected_post_balance,
            transfers,
        })
    }
}

/// Transactional state-and-transfer plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionPlan {
    before: RecoveryState,
    after: RecoveryState,
    expected_pre_balance: u64,
    expected_post_balance: u64,
    transfers: TransferPlan,
}

impl TransitionPlan {
    /// Reserve balance used to produce this plan.
    pub const fn expected_pre_balance(&self) -> u64 {
        self.expected_pre_balance
    }

    /// Exact required post-transfer reserve balance.
    pub const fn expected_post_balance(&self) -> u64 {
        self.expected_post_balance
    }

    /// Exact transfer compartments.
    pub const fn transfers(&self) -> TransferPlan {
        self.transfers
    }

    /// Resulting phase after commit.
    pub const fn resulting_phase(&self) -> RecoveryPhase {
        self.after.phase
    }
}

impl RecoveryState {
    /// Commit a current plan after exact post-balance verification.
    ///
    /// Every refusal occurs before mutation.
    pub fn commit_plan(&mut self, plan: TransitionPlan, actual_post_balance: u64) -> Result<()> {
        self.check()?;
        if *self != plan.before {
            return Err(RecoveryError::StalePlan);
        }
        if actual_post_balance != plan.expected_post_balance {
            return Err(RecoveryError::PostBalanceMismatch);
        }
        plan.transfers.validate()?;
        plan.after.check()?;
        *self = plan.after;
        Ok(())
    }
}

fn validate_schedule(schedule: &CompiledScheduleV1) -> Result<()> {
    schedule.validate().map_err(|error| match error {
        clutch_product_series::Error::NonCanonicalPadding => RecoveryError::NonCanonicalPadding,
        clutch_product_series::Error::ArithmeticOverflow => RecoveryError::ArithmeticOverflow,
        _ => RecoveryError::InvalidScheduleProjection,
    })
}

fn validate_funding_quote_projection(
    funding_quote: &SeriesFundingQuoteV1,
    recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    schedule: &CompiledScheduleV1,
) -> Result<()> {
    funding_quote.validate().map_err(map_funding_quote_error)?;
    if funding_quote.evidence_only_recovery_policy_id != recovery_policy_id
        || funding_quote.recovery_attempt_count != schedule.recovery_attempt_count
    {
        return Err(RecoveryError::ProjectionMismatch);
    }
    Ok(())
}

fn map_funding_quote_error(error: clutch_product_series::Error) -> RecoveryError {
    match error {
        clutch_product_series::Error::ZeroIdentity => RecoveryError::ZeroIdentity,
        clutch_product_series::Error::NonCanonicalPadding => RecoveryError::NonCanonicalPadding,
        clutch_product_series::Error::ArithmeticOverflow => RecoveryError::ArithmeticOverflow,
        _ => RecoveryError::InvalidFundingProjection,
    }
}

fn checked_add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right)
        .ok_or(RecoveryError::ArithmeticOverflow)
}

fn checked_add_u128(left: u128, right: u128) -> Result<u128> {
    left.checked_add(right)
        .ok_or(RecoveryError::ArithmeticOverflow)
}

fn require_live(identity: Identity) -> Result<()> {
    if identity.is_zero() {
        Err(RecoveryError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn decode_phase(value: u8) -> Result<RecoveryPhase> {
    match value {
        0 => Ok(RecoveryPhase::Active),
        1 => Ok(RecoveryPhase::DegradedRecoverable),
        2 => Ok(RecoveryPhase::RecoveryDormant),
        3 => Ok(RecoveryPhase::Resolved),
        _ => Err(RecoveryError::InvalidEnum),
    }
}

fn decode_disposition(value: u8) -> Result<ReserveDisposition> {
    match value {
        0 => Ok(ReserveDisposition::Open),
        1 => Ok(ReserveDisposition::Success),
        2 => Ok(ReserveDisposition::Dormancy),
        _ => Err(RecoveryError::InvalidEnum),
    }
}

struct StateWriter<'a> {
    output: &'a mut [u8],
    at: usize,
}

impl<'a> StateWriter<'a> {
    fn new(output: &'a mut [u8]) -> Result<Self> {
        if output.len() != RECOVERY_STATE_V2_BYTES {
            return Err(RecoveryError::WrongLength);
        }
        output.fill(0);
        Ok(Self { output, at: 0 })
    }

    fn bytes(&mut self, value: &[u8]) -> Result<()> {
        let end = self
            .at
            .checked_add(value.len())
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let target = self
            .output
            .get_mut(self.at..end)
            .ok_or(RecoveryError::WrongLength)?;
        target.copy_from_slice(value);
        self.at = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<()> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn i64(&mut self, value: i64) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn reserved(&mut self, count: usize) -> Result<()> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        if end > self.output.len() {
            return Err(RecoveryError::WrongLength);
        }
        self.at = end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.at == RECOVERY_STATE_V2_BYTES {
            Ok(())
        } else {
            Err(RecoveryError::WrongLength)
        }
    }
}

struct StateReader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> StateReader<'a> {
    fn new(input: &'a [u8]) -> Result<Self> {
        if input.len() != RECOVERY_STATE_V2_BYTES {
            return Err(RecoveryError::WrongLength);
        }
        Ok(Self { input, at: 0 })
    }

    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self
            .at
            .checked_add(N)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let source = self
            .input
            .get(self.at..end)
            .ok_or(RecoveryError::WrongLength)?;
        let mut value = [0; N];
        value.copy_from_slice(source);
        self.at = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes()?))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes()?))
    }

    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.bytes()?))
    }

    fn u128(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(self.bytes()?))
    }

    fn reserved(&mut self, count: usize) -> Result<()> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let source = self
            .input
            .get(self.at..end)
            .ok_or(RecoveryError::WrongLength)?;
        if source.iter().any(|byte| *byte != 0) {
            return Err(RecoveryError::NonCanonicalReserved);
        }
        self.at = end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.at == RECOVERY_STATE_V2_BYTES {
            Ok(())
        } else {
            Err(RecoveryError::WrongLength)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: Identity = Identity::from_bytes([1; IDENTITY_BYTES]);
    const FUNDER: Identity = Identity::from_bytes([2; IDENTITY_BYTES]);
    const RENT: Identity = Identity::from_bytes([3; IDENTITY_BYTES]);
    const SINK: Identity = Identity::from_bytes([4; IDENTITY_BYTES]);
    const WORK_A: Identity = Identity::from_bytes([5; IDENTITY_BYTES]);
    const WORK_B: Identity = Identity::from_bytes([6; IDENTITY_BYTES]);
    const KEEPER_A: Identity = Identity::from_bytes([7; IDENTITY_BYTES]);
    const KEEPER_B: Identity = Identity::from_bytes([8; IDENTITY_BYTES]);
    const EVIDENCE: Identity = Identity::from_bytes([9; IDENTITY_BYTES]);

    fn market_id() -> MarketInstanceId {
        MarketInstanceId::from_bytes([10; IDENTITY_BYTES])
    }

    fn policy_id() -> EvidenceOnlyRecoveryPolicyId {
        EvidenceOnlyRecoveryPolicyId::from_bytes([11; IDENTITY_BYTES])
    }

    fn clock(slot: u64, unix_timestamp: i64, current_bucket: u64) -> RecoveryClock {
        RecoveryClock {
            slot,
            unix_timestamp,
            current_bucket,
        }
    }

    fn schedule() -> CompiledScheduleV1 {
        let mut attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = AbsoluteRecoveryAttemptV1 {
            repair_generation: 10,
            opens_at_bucket: 106,
            closes_at_bucket: 108,
        };
        attempts[1] = AbsoluteRecoveryAttemptV1 {
            repair_generation: 11,
            opens_at_bucket: 109,
            closes_at_bucket: 111,
        };
        CompiledScheduleV1 {
            start_bucket: 100,
            end_bucket_exclusive: 105,
            primary_maturity_bucket_exclusive: 106,
            recovery_attempt_count: 2,
            recovery_attempts: attempts,
        }
    }

    fn funding() -> SeriesFundingQuoteV1 {
        let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = RecoveryAttemptFundingV1 {
            max_progress_units: 3,
            lamports_per_progress_unit: 5,
        };
        attempts[1] = RecoveryAttemptFundingV1 {
            max_progress_units: 2,
            lamports_per_progress_unit: 7,
        };
        SeriesFundingQuoteV1 {
            evidence_only_recovery_policy_id: policy_id(),
            market_core: ComponentDebitV1::ZERO,
            recovery_reserve: ComponentDebitV1 {
                lamports: 40,
                collateral_atoms: 0,
            },
            source_work: ComponentDebitV1::ZERO,
            liquidity_facility: ComponentDebitV1::ZERO,
            wrapper_set: ComponentDebitV1::ZERO,
            recovery_attempt_count: 2,
            recovery_attempt_funding: attempts,
            recovery_rent_principal_lamports: 11,
        }
    }

    fn admission() -> RecoveryAdmission {
        RecoveryAdmission {
            series_funding_quote_id: funding().id().unwrap(),
            state_id: STATE,
            generation: 3,
            work_funder: FUNDER,
            rent_payer: RENT,
            neutral_sink: SINK,
        }
    }

    fn new_state(donation: u64) -> (RecoveryState, u64) {
        let balance = donation + 40;
        let state = RecoveryState::admit(
            market_id(),
            policy_id(),
            schedule(),
            funding(),
            admission(),
            clock(1, 1, 100),
            FundingObservation {
                reserve_balance_before: donation,
                reserve_balance_after: balance,
                work_funder_debit_lamports: 29,
                rent_payer_debit_lamports: 11,
            },
        )
        .unwrap();
        (state, balance)
    }

    fn evidence() -> EvidenceDecision {
        EvidenceDecision::from_adapter(EVIDENCE).unwrap()
    }

    fn commit(state: &mut RecoveryState, plan: TransitionPlan) -> u64 {
        let post = plan.expected_post_balance();
        state.commit_plan(plan, post).unwrap();
        post
    }

    fn degrade(state: &mut RecoveryState, balance: u64) -> u64 {
        let plan = state
            .plan_enter_degraded(clock(10, 10, 106), balance)
            .unwrap();
        commit(state, plan)
    }

    #[test]
    fn projections_bind_policy_count_padding_and_exact_quote_total() {
        validate_funding_quote_projection(&funding(), policy_id(), &schedule()).unwrap();
        let mut wrong_policy = funding();
        wrong_policy.evidence_only_recovery_policy_id =
            EvidenceOnlyRecoveryPolicyId::from_bytes([99; IDENTITY_BYTES]);
        assert_eq!(
            validate_funding_quote_projection(&wrong_policy, policy_id(), &schedule()),
            Err(RecoveryError::ProjectionMismatch)
        );
        let mut padded = funding();
        padded.recovery_attempt_funding[2].max_progress_units = 1;
        assert_eq!(
            validate_funding_quote_projection(&padded, policy_id(), &schedule()),
            Err(RecoveryError::NonCanonicalPadding)
        );
        let mut wrong_total = funding();
        wrong_total.recovery_reserve.lamports = 39;
        assert_eq!(
            validate_funding_quote_projection(&wrong_total, policy_id(), &schedule()),
            Err(RecoveryError::InvalidFundingProjection)
        );
        let mut collateral = funding();
        collateral.recovery_reserve.collateral_atoms = 1;
        assert_eq!(
            validate_funding_quote_projection(&collateral, policy_id(), &schedule()),
            Err(RecoveryError::InvalidFundingProjection)
        );
    }

    #[test]
    fn equal_successor_repair_generations_refuse() {
        let mut equal = schedule();
        equal.recovery_attempts[1].repair_generation = equal.recovery_attempts[0].repair_generation;
        assert_eq!(
            validate_schedule(&equal),
            Err(RecoveryError::InvalidScheduleProjection)
        );
    }

    #[test]
    fn funding_projection_overflow_refuses() {
        let mut overflowing = funding();
        overflowing.recovery_attempt_funding[0].max_progress_units = u64::MAX;
        assert_eq!(
            validate_funding_quote_projection(&overflowing, policy_id(), &schedule()),
            Err(RecoveryError::ArithmeticOverflow)
        );
    }

    #[test]
    fn same_expected_quote_id_refuses_different_valid_rates() {
        let canonical = funding();
        let canonical_id = canonical.id().unwrap();
        let mut different_rates = canonical;
        different_rates.recovery_attempt_funding[0] = RecoveryAttemptFundingV1 {
            max_progress_units: 1,
            lamports_per_progress_unit: 15,
        };
        different_rates.validate().unwrap();
        assert_ne!(different_rates.id().unwrap(), canonical_id);
        let mut expected_canonical = admission();
        expected_canonical.series_funding_quote_id = canonical_id;
        assert_eq!(
            RecoveryState::admit(
                market_id(),
                policy_id(),
                schedule(),
                different_rates,
                expected_canonical,
                clock(1, 1, 100),
                FundingObservation {
                    reserve_balance_before: 0,
                    reserve_balance_after: 40,
                    work_funder_debit_lamports: 29,
                    rent_payer_debit_lamports: 11,
                },
            ),
            Err(RecoveryError::ProjectionMismatch)
        );

        let (mut admitted, _) = new_state(0);
        admitted.funding_quote = different_rates;
        assert_eq!(admitted.check(), Err(RecoveryError::ProjectionMismatch));
    }

    #[test]
    fn admission_keeps_prefund_donation_and_checks_each_payer() {
        let (state, balance) = new_state(13);
        assert_eq!(balance, 53);
        assert_eq!(state.ledger().donations_received, 13);
        assert_eq!(state.ledger().work_initial, 29);
        let result = RecoveryState::admit(
            market_id(),
            policy_id(),
            schedule(),
            funding(),
            admission(),
            clock(1, 1, 100),
            FundingObservation {
                reserve_balance_before: 13,
                reserve_balance_after: 53,
                work_funder_debit_lamports: 28,
                rent_payer_debit_lamports: 12,
            },
        );
        assert_eq!(result, Err(RecoveryError::FundingDeltaMismatch));

        let mut wrong_component = funding();
        wrong_component.recovery_reserve.lamports = 41;
        assert_eq!(
            RecoveryState::admit(
                market_id(),
                policy_id(),
                schedule(),
                wrong_component,
                admission(),
                clock(1, 1, 100),
                FundingObservation {
                    reserve_balance_before: 0,
                    reserve_balance_after: 40,
                    work_funder_debit_lamports: 29,
                    rent_payer_debit_lamports: 11,
                },
            ),
            Err(RecoveryError::InvalidFundingProjection)
        );
    }

    #[test]
    fn admission_refuses_every_reserve_recipient_alias() {
        let mut aliased = admission();
        aliased.state_id = FUNDER;
        assert_eq!(
            RecoveryState::admit(
                market_id(),
                policy_id(),
                schedule(),
                funding(),
                aliased,
                clock(1, 1, 100),
                FundingObservation {
                    reserve_balance_before: 0,
                    reserve_balance_after: 40,
                    work_funder_debit_lamports: 29,
                    rent_payer_debit_lamports: 11,
                },
            ),
            Err(RecoveryError::StateRecipientAlias)
        );
        aliased = admission();
        aliased.state_id = RENT;
        assert_eq!(
            RecoveryState::admit(
                market_id(),
                policy_id(),
                schedule(),
                funding(),
                aliased,
                clock(1, 1, 100),
                FundingObservation {
                    reserve_balance_before: 0,
                    reserve_balance_after: 40,
                    work_funder_debit_lamports: 29,
                    rent_payer_debit_lamports: 11,
                },
            ),
            Err(RecoveryError::StateRecipientAlias)
        );
        aliased = admission();
        aliased.state_id = SINK;
        assert_eq!(
            RecoveryState::admit(
                market_id(),
                policy_id(),
                schedule(),
                funding(),
                aliased,
                clock(1, 1, 100),
                FundingObservation {
                    reserve_balance_before: 0,
                    reserve_balance_after: 40,
                    work_funder_debit_lamports: 29,
                    rent_payer_debit_lamports: 11,
                },
            ),
            Err(RecoveryError::StateRecipientAlias)
        );
    }

    #[test]
    fn exposure_gate_closes_at_maturity_even_before_phase_crank() {
        let (state, _) = new_state(0);
        state.check_new_exposure(clock(2, 2, 105)).unwrap();
        assert_eq!(
            state.check_new_exposure(clock(3, 3, 106)),
            Err(RecoveryError::ExposureClosed)
        );
    }

    #[test]
    fn exposure_closes_at_maturity_even_when_first_repair_opens_later() {
        let mut delayed = schedule();
        delayed.recovery_attempts[0].opens_at_bucket = 107;
        let state = RecoveryState::admit(
            market_id(),
            policy_id(),
            delayed,
            funding(),
            admission(),
            clock(1, 1, 100),
            FundingObservation {
                reserve_balance_before: 0,
                reserve_balance_after: 40,
                work_funder_debit_lamports: 29,
                rent_payer_debit_lamports: 11,
            },
        )
        .unwrap();
        assert_eq!(
            state.check_new_exposure(clock(2, 2, 106)),
            Err(RecoveryError::ExposureClosed)
        );
        let plan = state.plan_enter_degraded(clock(2, 2, 106), 40).unwrap();
        assert_eq!(plan.resulting_phase(), RecoveryPhase::DegradedRecoverable);
    }

    #[test]
    fn degradation_boundary_is_inclusive_and_late_crank_cannot_refund() {
        let (mut state, balance) = new_state(3);
        assert_eq!(
            state.plan_enter_degraded(clock(10, 10, 105), balance),
            Err(RecoveryError::RecoveryNotOpen)
        );
        let plan = state
            .plan_enter_degraded(clock(11, 11, 111), balance)
            .unwrap();
        assert_eq!(plan.resulting_phase(), RecoveryPhase::RecoveryDormant);
        assert_eq!(plan.transfers().work_funder_refund.lamports, 0);
        assert_eq!(plan.transfers().neutral_sink_transfer.lamports, 32);
        commit(&mut state, plan);
        assert_eq!(state.ledger().dormancy_neutralized, 29);
    }

    #[test]
    fn clock_slot_time_and_bucket_are_all_monotone() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);
        assert_eq!(
            state.plan_advance_schedule(clock(9, 11, 106), balance),
            Err(RecoveryError::ClockMovedBackwards)
        );
        assert_eq!(
            state.plan_advance_schedule(clock(11, 9, 106), balance),
            Err(RecoveryError::ClockMovedBackwards)
        );
        assert_eq!(
            state.plan_advance_schedule(clock(11, 11, 105), balance),
            Err(RecoveryError::ClockMovedBackwards)
        );
        assert_eq!(
            state.plan_advance_schedule(clock(11, -1, 106), balance),
            Err(RecoveryError::ClockMovedBackwards)
        );
    }

    #[test]
    fn signed_clock_timestamps_have_no_arbitrary_epoch_restriction() {
        let state = RecoveryState::admit(
            market_id(),
            policy_id(),
            schedule(),
            funding(),
            admission(),
            clock(1, -10, 100),
            FundingObservation {
                reserve_balance_before: 0,
                reserve_balance_after: 40,
                work_funder_debit_lamports: 29,
                rent_payer_debit_lamports: 11,
            },
        )
        .unwrap();
        state.check_new_exposure(clock(2, -9, 101)).unwrap();
    }

    #[test]
    fn zero_progress_cannot_create_or_squat_work() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);
        assert_eq!(state.active_work_id(), None);
        assert_eq!(
            state.plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, KEEPER_A, 0,),
            Err(RecoveryError::NonmonotoneProgress)
        );
        assert_eq!(state.active_work_id(), None);
    }

    #[test]
    fn progress_refuses_reserve_and_neutral_sink_recipients() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);
        let before = state;
        assert_eq!(
            state.plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, STATE, 1),
            Err(RecoveryError::StateRecipientAlias)
        );
        assert_eq!(state, before);
        assert_eq!(
            state.plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, SINK, 1),
            Err(RecoveryError::InterestedNeutralSink)
        );
        assert_eq!(
            state.plan_resolve_paid_progress(
                clock(11, 11, 106),
                balance,
                WORK_A,
                STATE,
                1,
                evidence(),
            ),
            Err(RecoveryError::StateRecipientAlias)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn newly_accepted_progress_can_replace_a_stalled_work() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);
        let plan = state
            .plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, KEEPER_A, 1)
            .unwrap();
        assert_eq!(plan.transfers().accepted_progress_reward.lamports, 5);
        let balance = commit(&mut state, plan);
        assert_eq!(state.active_work_id(), Some(WORK_A));
        let plan = state
            .plan_accept_work_progress(clock(12, 12, 107), balance, WORK_B, KEEPER_B, 2)
            .unwrap();
        assert_eq!(plan.transfers().accepted_progress_reward.lamports, 5);
        commit(&mut state, plan);
        assert_eq!(state.active_work_id(), Some(WORK_B));
    }

    #[test]
    fn hostile_full_window_trace_cannot_squat_or_redirect_unused_principal() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);

        assert_eq!(
            state.plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, KEEPER_A, 0,),
            Err(RecoveryError::NonmonotoneProgress)
        );
        let plan = state
            .plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, KEEPER_A, 1)
            .unwrap();
        let balance = commit(&mut state, plan);
        assert_eq!(
            state.plan_accept_work_progress(clock(11, 11, 106), balance, WORK_B, KEEPER_B, 1,),
            Err(RecoveryError::NonmonotoneProgress)
        );
        let plan = state
            .plan_accept_work_progress(clock(12, 12, 107), balance, WORK_B, KEEPER_B, 2)
            .unwrap();
        let balance = commit(&mut state, plan);

        let plan = state
            .plan_advance_schedule(clock(13, 13, 108), balance)
            .unwrap();
        let balance = commit(&mut state, plan);
        assert_eq!(state.active_work_id(), None);
        assert_eq!(
            state.plan_accept_work_progress(clock(14, 14, 109), balance, WORK_A, KEEPER_A, 0,),
            Err(RecoveryError::NonmonotoneProgress)
        );
        let plan = state
            .plan_accept_work_progress(clock(14, 14, 109), balance, WORK_A, KEEPER_A, 1)
            .unwrap();
        let balance = commit(&mut state, plan);
        let plan = state
            .plan_accept_work_progress(clock(15, 15, 110), balance, WORK_B, KEEPER_B, 2)
            .unwrap();
        let balance = commit(&mut state, plan);

        let plan = state
            .plan_advance_schedule(clock(16, 16, 111), balance)
            .unwrap();
        assert_eq!(plan.resulting_phase(), RecoveryPhase::RecoveryDormant);
        assert_eq!(plan.transfers().accepted_progress_reward.lamports, 0);
        assert_eq!(plan.transfers().work_funder_refund.lamports, 0);
        assert_eq!(plan.transfers().neutral_sink_transfer.lamports, 5);
        commit(&mut state, plan);
        assert_eq!(state.ledger().accepted_progress_paid, 24);
        assert_eq!(state.ledger().dormancy_neutralized, 5);
    }

    #[test]
    fn replay_over_cap_and_one_lamport_short_refuse_without_mutation() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);
        let before = state;
        assert_eq!(
            state.plan_accept_work_progress(clock(11, 11, 106), balance - 1, WORK_A, KEEPER_A, 1,),
            Err(RecoveryError::ReserveBalanceShortfall)
        );
        assert_eq!(state, before);
        let plan = state
            .plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, KEEPER_A, 1)
            .unwrap();
        let balance = commit(&mut state, plan);
        assert_eq!(
            state.plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, KEEPER_A, 1,),
            Err(RecoveryError::NonmonotoneProgress)
        );
        assert_eq!(
            state.plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, KEEPER_A, 4,),
            Err(RecoveryError::ProgressLimitExceeded)
        );
    }

    #[test]
    fn exclusive_close_advances_to_gap_then_next_attempt() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);
        let plan = state
            .plan_advance_schedule(clock(12, 12, 108), balance)
            .unwrap();
        let balance = commit(&mut state, plan);
        assert_eq!(state.next_attempt_index(), 1);
        assert_eq!(
            state.plan_accept_work_progress(clock(12, 12, 108), balance, WORK_A, KEEPER_A, 1,),
            Err(RecoveryError::AttemptNotOpen)
        );
        state
            .plan_accept_work_progress(clock(13, 13, 109), balance, WORK_A, KEEPER_A, 1)
            .unwrap();
    }

    #[test]
    fn final_close_owns_residue_independent_of_crank_order() {
        let (mut state, balance) = new_state(2);
        let balance = degrade(&mut state, balance);
        let success = state
            .plan_resolve_caller_funded(clock(20, 20, 110), balance, evidence())
            .unwrap();
        assert_eq!(success.transfers().work_funder_refund.lamports, 29);

        let at_close = state
            .plan_resolve_caller_funded(clock(21, 21, 111), balance, evidence())
            .unwrap();
        assert_eq!(at_close.resulting_phase(), RecoveryPhase::Resolved);
        assert_eq!(at_close.transfers().work_funder_refund.lamports, 0);
        assert_eq!(at_close.transfers().neutral_sink_transfer.lamports, 31);
        commit(&mut state, at_close);
        assert_eq!(state.reserve_disposition(), ReserveDisposition::Dormancy);
    }

    #[test]
    fn late_donation_cannot_grief_caller_funded_resolution() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);
        let plan = state
            .plan_advance_schedule(clock(20, 20, 111), balance)
            .unwrap();
        commit(&mut state, plan);
        let before = state.ledger();
        let plan = state
            .plan_resolve_caller_funded(clock(100, 100, 200), 1, evidence())
            .unwrap();
        assert_eq!(plan.transfers().neutral_sink_transfer.lamports, 1);
        commit(&mut state, plan);
        assert_eq!(state.phase(), RecoveryPhase::Resolved);
        assert_eq!(
            state.ledger().donations_received,
            before.donations_received + 1
        );
        assert_eq!(
            state.ledger().donations_neutralized,
            before.donations_neutralized + 1
        );
    }

    #[test]
    fn maximum_prior_donation_cannot_overflow_and_regrief_late_resolution() {
        let prior_donation = u64::MAX - 40;
        let (mut state, balance) = new_state(prior_donation);
        assert_eq!(balance, u64::MAX);
        let plan = state
            .plan_enter_degraded(clock(20, 20, 111), balance)
            .unwrap();
        commit(&mut state, plan);
        let before = state.ledger();
        let late = 100_u64;
        let plan = state
            .plan_resolve_caller_funded(clock(100, 100, 200), late, evidence())
            .unwrap();
        assert_eq!(plan.transfers().neutral_sink_transfer.lamports, late);
        commit(&mut state, plan);
        assert_eq!(
            state.ledger().donations_received,
            before.donations_received + u128::from(late)
        );
    }

    #[test]
    fn later_open_reserve_donation_never_becomes_success_refund() {
        let (mut state, balance) = new_state(2);
        let donated_balance = balance + 9;
        let plan = state
            .plan_enter_degraded(clock(10, 10, 106), donated_balance)
            .unwrap();
        let balance = commit(&mut state, plan);
        let plan = state
            .plan_resolve_caller_funded(clock(11, 11, 106), balance, evidence())
            .unwrap();
        assert_eq!(plan.transfers().work_funder_refund.lamports, 29);
        assert_eq!(plan.transfers().rent_payer_refund.lamports, 11);
        assert_eq!(plan.transfers().neutral_sink_transfer.lamports, 11);
    }

    #[test]
    fn paid_resolution_pays_only_new_progress_then_refunds() {
        let (mut state, balance) = new_state(0);
        let balance = degrade(&mut state, balance);
        let plan = state
            .plan_accept_work_progress(clock(11, 11, 106), balance, WORK_A, KEEPER_A, 1)
            .unwrap();
        let balance = commit(&mut state, plan);
        let plan = state
            .plan_resolve_paid_progress(
                clock(12, 12, 107),
                balance,
                WORK_B,
                KEEPER_B,
                3,
                evidence(),
            )
            .unwrap();
        assert_eq!(plan.transfers().accepted_progress_reward.lamports, 10);
        assert_eq!(plan.transfers().work_funder_refund.lamports, 14);
        commit(&mut state, plan);
        assert_eq!(state.ledger().accepted_progress_paid, 15);
        assert_eq!(state.ledger().success_refunded, 14);
    }

    #[test]
    fn stale_plan_and_wrong_post_balance_refuse_atomically() {
        let (mut state, balance) = new_state(0);
        let plan = state
            .plan_enter_degraded(clock(10, 10, 106), balance)
            .unwrap();
        let replay = plan;
        commit(&mut state, plan);
        let before = state;
        assert_eq!(
            state.commit_plan(replay, balance),
            Err(RecoveryError::StalePlan)
        );
        assert_eq!(state, before);

        let plan = state
            .plan_resolve_caller_funded(clock(11, 11, 106), balance, evidence())
            .unwrap();
        let before = state;
        assert_eq!(
            state.commit_plan(plan, 1),
            Err(RecoveryError::PostBalanceMismatch)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn transition_nonce_overflow_refuses_before_mutation() {
        let (mut state, balance) = new_state(0);
        state.transition_nonce = u64::MAX;
        let before = state;
        assert_eq!(
            state.plan_resolve_caller_funded(clock(2, 2, 101), balance, evidence()),
            Err(RecoveryError::ArithmeticOverflow)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn hostile_private_state_validation_rejects_unreachable_active_payment() {
        let (mut state, _) = new_state(0);
        state.work_remaining -= 5;
        state.accepted_progress_paid += 5;
        assert_eq!(state.check(), Err(RecoveryError::InvariantViolation));
    }

    #[test]
    fn terminal_plan_refuses_corrupt_reserve_recipient_alias() {
        let (mut state, balance) = new_state(0);
        state.work_funder = STATE;
        let before = state;
        assert_eq!(
            state.plan_resolve_caller_funded(clock(2, 2, 101), balance, evidence()),
            Err(RecoveryError::InvariantViolation)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn hostile_private_state_rejects_degradation_before_maturity() {
        let (mut state, _) = new_state(0);
        state.phase = RecoveryPhase::DegradedRecoverable;
        assert_eq!(state.check(), Err(RecoveryError::InvariantViolation));
    }

    #[test]
    fn fallible_indexed_views_refuse_corrupt_counts_without_panicking() {
        let (mut state, _) = new_state(0);
        state.schedule.recovery_attempt_count = u8::MAX;
        state.next_attempt_index = u8::MAX;
        assert_eq!(
            state.current_attempt(),
            Err(RecoveryError::InvalidScheduleProjection)
        );
        assert_eq!(
            state.accepted_progress_units(u8::MAX),
            Err(RecoveryError::InvalidScheduleProjection)
        );
    }

    #[test]
    fn recipient_aliases_remain_separate_exact_compartments() {
        let mut same_payer = admission();
        same_payer.rent_payer = FUNDER;
        let mut state = RecoveryState::admit(
            market_id(),
            policy_id(),
            schedule(),
            funding(),
            same_payer,
            clock(1, 1, 100),
            FundingObservation {
                reserve_balance_before: 0,
                reserve_balance_after: 40,
                work_funder_debit_lamports: 29,
                rent_payer_debit_lamports: 11,
            },
        )
        .unwrap();
        let plan = state
            .plan_resolve_caller_funded(clock(2, 2, 101), 40, evidence())
            .unwrap();
        assert_eq!(plan.transfers().work_funder_refund.recipient, FUNDER);
        assert_eq!(plan.transfers().rent_payer_refund.recipient, FUNDER);
        assert_eq!(plan.transfers().total_lamports().unwrap(), 40);
        commit(&mut state, plan);
    }
}
