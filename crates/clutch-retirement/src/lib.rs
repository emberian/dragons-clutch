// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Fixed-layout codecs and pure transitions for counted retirement.
//!
//! This crate owns only ADR-0007's new semantic facts. Existing live account
//! bodies remain owned by `clutch-solana-layout`; an adapter appends the exact
//! tails exported here and must authenticate the base body, program owner,
//! length, PDA, and generation before calling a transition. Nothing in this
//! crate can enumerate Solana accounts or authorize a deployment.
//!
//! Every public type named `Adapter*ProjectionV1` is deliberately a forgeable
//! pure-data carrier. Its public fields let tests exercise hostile adapter
//! output, but confer no owner, PDA, codec, executable, or account-byte
//! authority. A live handler must produce each projection from authenticated
//! runtime facts before invoking this crate. Only private-field `Validated*`
//! values represent a capability minted by complete pure validation.
//! `AuthenticatedEpochChildV1` is a frozen historical type alias whose name is
//! preserved for source compatibility; it is also a forgeable projection and
//! does not contradict that rule.

mod codec;
mod position_v3;
mod transition;

pub use codec::{
    ChildGenerationV1, DeletableRentOwnerV1, EpochChildCountsV1, EpochRetirementTailV1,
    GeneralEpochTombstoneV1, Identity32V1, MarketEpochCursorV1, PositionRetirementTailV1,
    PositionTombstoneV1, RentSplitV2, ReservationCountTailV1, ReservationRetirementTailV2,
};
pub use position_v3::{
    project_dealer_position_v3, project_general_position_v3, project_series_position_v3,
    project_structured_claim_position_v3, AdapterPositionMarketBindingV3,
    AdapterPositionPurposeBindingV3, DealerPositionProjectionV3, GeneralPositionProjectionV3,
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, PositionTerminalProjectionV3,
    PositionTombstoneLifecycleV3, PositionTombstoneV3, PositionTombstoneV3Fields, PositionV3Fields,
    PositionV3PdaSeeds, PositionV3Sha256Backend, SeriesPositionProjectionV3,
    StructuredClaimPositionProjectionV3, POSITION_TOMBSTONE_V3_SEMANTIC_DOMAIN,
    POSITION_V3_PDA_PREFIX, POSITION_V3_SEMANTIC_DOMAIN,
};
pub use transition::{
    admit_deletable_rent, admit_initial_rent_split, admit_reopen_rent_split, close_epoch,
    close_epoch_child, close_epoch_child_v2, close_general_reservation_archive, close_position,
    close_registered_candidate, close_registered_candidate_v2, create_epoch_child,
    create_epoch_child_v2, create_registered_candidate_after_validation,
    create_registered_candidate_after_validation_v2, entitle_reservation, entitle_reservation_v2,
    open_general_epoch, open_general_epoch_root, plan_direct_reservation_close,
    plan_epoch_retirement, plan_epoch_root_retirement, plan_general_reservation_close,
    plan_position_replay_retirement, plan_position_retirement, register_direct_reservation,
    register_direct_reservation_v2, register_general_reservation, register_general_reservation_v2,
    reopen_position, reopen_position_with_replay, terminate_reservation, terminate_reservation_v2,
    update_registered_candidate_status_after_validation,
    update_registered_candidate_status_after_validation_v2, AdapterDirectEpochProjectionV1,
    AdapterEpochAccountProjectionV1, AdapterMarketAccountProjectionV1,
    AdapterNeutralSinkBindingProjectionV1, AdapterPositionAccountProjectionV1,
    AdapterReplayAbsenceProjectionV1, AdapterReplayAccountProjectionV1, AuthenticatedEpochChildV1,
    ChildSlotV1, CoalescedPayerDebitsV1, CoalescedRecipientCreditsV1,
    CountedEpochChildProjectionV2, CountedEpochChildSlotV2, CountedReservationV1,
    CountedReservationV2, DeletableAccountClosePlanV1, DeletableRentAdmissionPlanV1,
    DeletableRentDispositionV1, DirectEpochLifecyclePhaseV1, DirectReservationClosePlanV1,
    DirectReservationCloseRequestV1, DirectReservationRegistrationAccountsV1,
    EpochBudgetRootSiblingV1, EpochChildProjectionV1, EpochLifecycleStateV5, EpochRootAccountsV1,
    EpochRootRetirementPlanV1, EpochRootRetirementRequestV1, EpochWindowRootSiblingV1,
    GeneralEpochLifecycleProjectionV2, GeneralReservationClosePlanV1,
    GeneralReservationCloseRequestV1, GeneralReservationRegistrationAccountsV1, LiveEpochV5,
    LiveGeneralEpochProjectionV2, LivePositionV2, LiveReplaySuccessorV1,
    OpenGeneralEpochRootPlanV1, OpenGeneralEpochRootRequestV1, PayerDebitV1,
    PositionEconomicStateV1, PositionLifecycleStateV2, PositionReplayAccountsV1,
    PositionReplayReopenAccountsV1, PositionReplayReopenPlanV1, PositionReplayReopenRequestV1,
    PositionReplayRetirementPlanV1, PositionReplayRetirementRequestV1, RecipientBalanceBookV1,
    RecipientBalanceV1, RecipientCreditV1, RentDispositionV2, RentSplitAdmissionPlanV2,
    ReplayLifecycleStateV1, RetirementCommitPlanV2, ValidatedAdmissionLedgerRetiredV1,
    MAX_RETIREMENT_RECIPIENTS,
};

/// Number of bytes in every persisted identity.
pub const IDENTITY_BYTES: usize = 32;
/// Fixed active outcome width inherited from the current Position layout.
pub const MAX_OUTCOMES: usize = 16;

/// Derive the canonical nonzero generation from a checked Epoch index.
pub const fn canonical_epoch_generation(epoch_index: u64) -> Result<u64, RetirementErrorV2> {
    match epoch_index.checked_add(1) {
        Some(generation) => Ok(generation),
        None => Err(RetirementErrorV2::EpochIndexExhausted),
    }
}

/// Existing Position account discriminator.
pub const POSITION_ACCOUNT_TAG: u8 = 6;
/// Counted Position schema.
pub const POSITION_ACCOUNT_VERSION_V2: u8 = 2;
/// Full-width, purpose-neutral Position successor schema.
///
/// The central registry reserves this coordinate with every runtime route
/// disabled until its account adapter is integrated.
pub const POSITION_ACCOUNT_VERSION_V3: u8 = 3;
/// Existing Epoch account discriminator.
pub const EPOCH_ACCOUNT_TAG: u8 = 11;
/// Counted general Epoch schema.
///
/// Versions 3 and 4 are already occupied by direct-Epoch families under the
/// same tag, so the first noncolliding counted general version is 5.
pub const EPOCH_ACCOUNT_VERSION_V5: u8 = 5;
/// Existing Market account discriminator.
pub const MARKET_ACCOUNT_TAG: u8 = 3;
/// Monotone-cursor Market schema.
pub const MARKET_ACCOUNT_VERSION_V2: u8 = 2;
/// Existing general reservation discriminator.
pub const RESERVATION_ACCOUNT_TAG: u8 = 19;
/// Counted general reservation schema.
pub const RESERVATION_ACCOUNT_VERSION_V5: u8 = 5;
/// Existing direct Reservation V2 schema under the shared discriminator.
pub const DIRECT_RESERVATION_ACCOUNT_VERSION_V2: u8 = 2;
/// Counted direct Reservation schema.
///
/// Version 3 was a historical general-Reservation wire schema, version 4 is
/// current general, and version 5 is the counted general successor. Direct
/// promotion therefore uses the next never-allocated version, 6.
pub const DIRECT_RESERVATION_ACCOUNT_VERSION_V6: u8 = 6;
/// Deletable counted general Reservation successor schema.
///
/// This is a codec-local provisional version. It is intentionally not routed
/// by the live SBF program before central registry allocation.
pub const RESERVATION_ACCOUNT_VERSION_V7: u8 = 7;
/// Deletable counted direct Reservation successor schema.
///
/// This is a codec-local provisional version. It is intentionally not routed
/// by the live SBF program before central registry allocation.
pub const DIRECT_RESERVATION_ACCOUNT_VERSION_V8: u8 = 8;

/// Codec-local Position tombstone discriminator.
///
/// The authoritative central registry reserves this coordinate as disabled.
/// It has no live codec or SBF route merely because this local codec exists.
pub const POSITION_TOMBSTONE_TAG: u8 = 0x75;
/// First Position tombstone schema.
pub const POSITION_TOMBSTONE_VERSION_V1: u8 = 1;
/// Full-identity Position V3 permanent tombstone schema.
pub const POSITION_TOMBSTONE_VERSION_V3: u8 = 3;
/// Codec-local general Epoch tombstone discriminator.
///
/// The authoritative central registry reserves this coordinate as disabled.
/// It has no live codec or SBF route merely because this local codec exists.
pub const GENERAL_EPOCH_TOMBSTONE_TAG: u8 = 0x76;
/// First general Epoch tombstone schema.
pub const GENERAL_EPOCH_TOMBSTONE_VERSION_V1: u8 = 1;

/// Existing Position V1 bytes, owned by `clutch-solana-layout`.
pub const POSITION_V1_BYTES: usize = 220;
/// Existing Epoch V2 bytes, owned by `clutch-solana-layout`.
pub const EPOCH_V2_BYTES: usize = 329;
/// Existing Market V1 bytes, owned by `clutch-solana-layout`.
pub const MARKET_V1_BYTES: usize = 726;
/// Existing general Reservation V4 bytes, owned by `clutch-solana-layout`.
pub const RESERVATION_V4_BYTES: usize = 618;
/// Existing direct Reservation V2 bytes, owned by `clutch-solana-layout`.
pub const DIRECT_RESERVATION_V2_BYTES: usize = 618;
/// Current reference Replay body width, owned by `clutch-solana-reference`.
pub const REFERENCE_REPLAY_V1_BYTES: usize = 84;

/// Exact rent split tail width.
pub const RENT_SPLIT_V2_BYTES: usize = 56;
/// Exact counted Position extension width.
pub const POSITION_RETIREMENT_TAIL_V1_BYTES: usize = 60;
/// Exact frozen nine-class epoch-child counter width.
pub const EPOCH_CHILD_COUNTS_V1_BYTES: usize = 36;
/// Exact counted Epoch extension width.
pub const EPOCH_RETIREMENT_TAIL_V1_BYTES: usize = 100;
/// Exact monotone Market cursor width.
pub const MARKET_EPOCH_CURSOR_V1_BYTES: usize = 8;
/// Exact reservation counter-marker extension width.
pub const RESERVATION_COUNT_TAIL_V1_BYTES: usize = 9;
/// Exact embedded funding owner width for a fully deleted account.
pub const DELETABLE_RENT_OWNER_V1_BYTES: usize = 48;
/// Projected closeable Replay width before central tag/version allocation.
///
/// This is size arithmetic only. It does not allocate a wire schema or route.
pub const PROJECTED_REPLAY_SUCCESSOR_BYTES: usize =
    REFERENCE_REPLAY_V1_BYTES + DELETABLE_RENT_OWNER_V1_BYTES;
/// Exact Reservation count-plus-funding extension width.
pub const RESERVATION_RETIREMENT_TAIL_V2_BYTES: usize =
    RESERVATION_COUNT_TAIL_V1_BYTES + DELETABLE_RENT_OWNER_V1_BYTES;
/// Exact common epoch-generation extension width for other child schemas.
pub const CHILD_GENERATION_V1_BYTES: usize = 8;

/// Exact full Position V2 width after composition.
pub const POSITION_V2_BYTES: usize = POSITION_V1_BYTES + POSITION_RETIREMENT_TAIL_V1_BYTES;
/// Exact full-width global Position V3 body.
pub const POSITION_V3_BYTES: usize = 480;
/// Exact full general Epoch V5 width after composition.
pub const EPOCH_V5_BYTES: usize = EPOCH_V2_BYTES + EPOCH_RETIREMENT_TAIL_V1_BYTES;
/// Exact full Market V2 width after composition.
pub const MARKET_V2_BYTES: usize = MARKET_V1_BYTES + MARKET_EPOCH_CURSOR_V1_BYTES;
/// Exact full general Reservation V5 width after composition.
pub const RESERVATION_V5_BYTES: usize = RESERVATION_V4_BYTES + RESERVATION_COUNT_TAIL_V1_BYTES;
/// Exact full direct Reservation V6 width after composition.
pub const DIRECT_RESERVATION_V6_BYTES: usize =
    DIRECT_RESERVATION_V2_BYTES + RESERVATION_COUNT_TAIL_V1_BYTES;
/// Exact full deletable general Reservation V7 width after composition.
pub const RESERVATION_V7_BYTES: usize = RESERVATION_V4_BYTES + RESERVATION_RETIREMENT_TAIL_V2_BYTES;
/// Exact full deletable direct Reservation V8 width after composition.
pub const DIRECT_RESERVATION_V8_BYTES: usize =
    DIRECT_RESERVATION_V2_BYTES + RESERVATION_RETIREMENT_TAIL_V2_BYTES;
/// Exact Position tombstone width.
pub const POSITION_TOMBSTONE_V1_BYTES: usize = 76;
/// Exact full-identity permanent Position V3 tombstone body.
pub const POSITION_TOMBSTONE_V3_BYTES: usize = 280;
/// Exact general Epoch tombstone width.
pub const GENERAL_EPOCH_TOMBSTONE_V1_BYTES: usize = 84;

const _: () = assert!(POSITION_V2_BYTES == 280);
const _: () = assert!(POSITION_V3_BYTES == 480);
const _: () = assert!(POSITION_TOMBSTONE_V3_BYTES == 280);
const _: () = assert!(EPOCH_V5_BYTES == 429);
const _: () = assert!(MARKET_V2_BYTES == 734);
const _: () = assert!(RESERVATION_V5_BYTES == 627);
const _: () = assert!(DIRECT_RESERVATION_V6_BYTES == 627);
const _: () = assert!(RESERVATION_V7_BYTES == 675);
const _: () = assert!(DIRECT_RESERVATION_V8_BYTES == 675);
const _: () = assert!(PROJECTED_REPLAY_SUCCESSOR_BYTES == 132);

/// Refusals owned by the counted-retirement seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementErrorV1 {
    /// The byte slice is shorter than the exact codec length.
    Truncated,
    /// The byte slice is longer than the exact codec length.
    TrailingBytes,
    /// The account discriminator is not the expected family.
    WrongTag,
    /// The schema byte is not the one exact supported version.
    WrongVersion,
    /// A required persisted identity is the zero sentinel.
    ZeroIdentity,
    /// A generation is zero or does not match its authenticated parent.
    WrongGeneration,
    /// An enum, boolean byte, phase, or candidate status is unknown.
    InvalidEnum,
    /// A count or field combination is noncanonical.
    NonCanonicalState,
    /// Checked integer arithmetic overflowed.
    ArithmeticOverflow,
    /// A decrement would underflow its authoritative count.
    CounterUnderflow,
    /// The caller did not request the Market's exact next Epoch index.
    NonmonotoneEpoch,
    /// The cursor is exhausted at `u64::MAX`.
    EpochIndexExhausted,
    /// Local Position balances are not economically zero.
    EconomicBalanceOutstanding,
    /// A live reservation still owns Position assets.
    ReservationOutstanding,
    /// At least one authenticated Epoch child remains live.
    ChildOutstanding,
    /// A replay targeted an already terminal state.
    AlreadyTerminal,
    /// The requested lifecycle transition is not admitted from this phase.
    WrongPhase,
    /// A child creation targeted an occupied canonical slot.
    ChildAlreadyPresent,
    /// A child close targeted an absent canonical slot.
    ChildAbsent,
    /// A generic child operation was used for a candidate bundle or vice versa.
    WrongChildKind,
    /// Candidate retirement was attempted while its canonical ClearWork survives.
    ClearWorkOutstanding,
    /// The closing account balance cannot cover every persisted compartment.
    AccountBalanceShortfall,
    /// The rent payer and frozen neutral sink are the same identity.
    PayerIsNeutralSink,
}

/// Refusals owned by successor counted-retirement APIs.
///
/// This is a distinct source surface so the exhaustive frozen
/// [`RetirementErrorV1`] enum retains its exact committed variants and order.
/// Conversion from V1 is lossless; no conversion from V2 to V1 exists because
/// successor-only refusals cannot be represented by the frozen enum.
///
/// ```compile_fail
/// use clutch_retirement::{RetirementErrorV1, RetirementErrorV2};
/// let frozen: RetirementErrorV1 = RetirementErrorV2::WrongParent.into();
/// # let _ = frozen;
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementErrorV2 {
    /// The byte slice is shorter than the exact codec length.
    Truncated,
    /// The byte slice is longer than the exact codec length.
    TrailingBytes,
    /// The account discriminator is not the expected family.
    WrongTag,
    /// The schema byte is not the one exact supported version.
    WrongVersion,
    /// A required persisted identity is the zero sentinel.
    ZeroIdentity,
    /// A generation is zero or does not match its exact parent.
    WrongGeneration,
    /// Projected parent identities disagree across a transition bundle.
    WrongParent,
    /// An admission plan was prepared against a different frozen neutral sink.
    WrongNeutralSink,
    /// An admission plan was prepared from a different target account balance.
    WrongFundingTarget,
    /// An enum, boolean byte, phase, or candidate status is unknown.
    InvalidEnum,
    /// A count or field combination is noncanonical.
    NonCanonicalState,
    /// Checked integer arithmetic overflowed.
    ArithmeticOverflow,
    /// A decrement would underflow its authoritative count.
    CounterUnderflow,
    /// The caller did not request the Market's exact next Epoch index.
    NonmonotoneEpoch,
    /// The cursor is exhausted at `u64::MAX`.
    EpochIndexExhausted,
    /// Local Position balances are not economically zero.
    EconomicBalanceOutstanding,
    /// A live reservation still owns Position assets.
    ReservationOutstanding,
    /// At least one counted Epoch child projection remains live.
    ChildOutstanding,
    /// A replay targeted an already terminal state.
    AlreadyTerminal,
    /// The requested lifecycle transition is not admitted from this phase.
    WrongPhase,
    /// A child creation targeted an occupied canonical slot.
    ChildAlreadyPresent,
    /// A child close targeted an absent canonical slot.
    ChildAbsent,
    /// A generic child operation was used for a candidate bundle or vice versa.
    WrongChildKind,
    /// Candidate retirement was attempted while its canonical ClearWork survives.
    ClearWorkOutstanding,
    /// The closing account balance cannot cover every persisted compartment.
    AccountBalanceShortfall,
    /// The recorded payer cannot fund the full principal without a prefund discount.
    PayerBalanceShortfall,
    /// Bundle members supplied inconsistent starting balances for one payer.
    InconsistentPayerBalance,
    /// The rent payer and frozen neutral sink are the same identity.
    PayerIsNeutralSink,
    /// Two source accounts, or a source and recipient account, alias.
    AccountAlias,
    /// A required recipient and supplied starting balance are absent.
    MissingRecipient,
    /// A Replay sibling does not bind the exact Position identity and generation.
    ReplayMismatch,
    /// Candidate admission nodes or their authoritative Window ledger remain live.
    AdmissionLedgerOutstanding,
    /// The authoritative Budget owner has not supplied complete reward funding.
    BudgetFundingUnauthenticated,
    /// The authoritative Budget owner has not supplied a terminal disposition.
    BudgetRetirementUnauthenticated,
}

/// Losslessly lift one frozen refusal into the successor error surface.
pub(crate) const fn retirement_error_v2_from_v1(error: RetirementErrorV1) -> RetirementErrorV2 {
    match error {
        RetirementErrorV1::Truncated => RetirementErrorV2::Truncated,
        RetirementErrorV1::TrailingBytes => RetirementErrorV2::TrailingBytes,
        RetirementErrorV1::WrongTag => RetirementErrorV2::WrongTag,
        RetirementErrorV1::WrongVersion => RetirementErrorV2::WrongVersion,
        RetirementErrorV1::ZeroIdentity => RetirementErrorV2::ZeroIdentity,
        RetirementErrorV1::WrongGeneration => RetirementErrorV2::WrongGeneration,
        RetirementErrorV1::InvalidEnum => RetirementErrorV2::InvalidEnum,
        RetirementErrorV1::NonCanonicalState => RetirementErrorV2::NonCanonicalState,
        RetirementErrorV1::ArithmeticOverflow => RetirementErrorV2::ArithmeticOverflow,
        RetirementErrorV1::CounterUnderflow => RetirementErrorV2::CounterUnderflow,
        RetirementErrorV1::NonmonotoneEpoch => RetirementErrorV2::NonmonotoneEpoch,
        RetirementErrorV1::EpochIndexExhausted => RetirementErrorV2::EpochIndexExhausted,
        RetirementErrorV1::EconomicBalanceOutstanding => {
            RetirementErrorV2::EconomicBalanceOutstanding
        }
        RetirementErrorV1::ReservationOutstanding => RetirementErrorV2::ReservationOutstanding,
        RetirementErrorV1::ChildOutstanding => RetirementErrorV2::ChildOutstanding,
        RetirementErrorV1::AlreadyTerminal => RetirementErrorV2::AlreadyTerminal,
        RetirementErrorV1::WrongPhase => RetirementErrorV2::WrongPhase,
        RetirementErrorV1::ChildAlreadyPresent => RetirementErrorV2::ChildAlreadyPresent,
        RetirementErrorV1::ChildAbsent => RetirementErrorV2::ChildAbsent,
        RetirementErrorV1::WrongChildKind => RetirementErrorV2::WrongChildKind,
        RetirementErrorV1::ClearWorkOutstanding => RetirementErrorV2::ClearWorkOutstanding,
        RetirementErrorV1::AccountBalanceShortfall => RetirementErrorV2::AccountBalanceShortfall,
        RetirementErrorV1::PayerIsNeutralSink => RetirementErrorV2::PayerIsNeutralSink,
    }
}

impl From<RetirementErrorV1> for RetirementErrorV2 {
    fn from(error: RetirementErrorV1) -> Self {
        retirement_error_v2_from_v1(error)
    }
}

/// One of the nine frozen Epoch-owned child classes in ADR-0007.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EpochChildKindV1 {
    /// Candidate record + feed + their funding identity.
    CandidateBundle = 0,
    /// One CandidateIndex page from ADR-0006.
    CandidateIndexPage = 1,
    /// One immutable candidate verdict.
    CandidateVerdict = 2,
    /// One candidate escrow.
    CandidateEscrow = 3,
    /// Growing or complete ClearWork + funding identity.
    ClearWorkBundle = 4,
    /// One order page.
    OrderPage = 5,
    /// One terminal-or-live reservation archive.
    ReservationArchive = 6,
    /// One settlement receipt.
    SettlementReceipt = 7,
    /// The unique final pot.
    FinalPot = 8,
}

impl TryFrom<u8> for EpochChildKindV1 {
    type Error = RetirementErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::CandidateBundle),
            1 => Ok(Self::CandidateIndexPage),
            2 => Ok(Self::CandidateVerdict),
            3 => Ok(Self::CandidateEscrow),
            4 => Ok(Self::ClearWorkBundle),
            5 => Ok(Self::OrderPage),
            6 => Ok(Self::ReservationArchive),
            7 => Ok(Self::SettlementReceipt),
            8 => Ok(Self::FinalPot),
            _ => Err(RetirementErrorV1::InvalidEnum),
        }
    }
}

impl From<EpochChildKindV1> for u8 {
    fn from(value: EpochChildKindV1) -> Self {
        match value {
            EpochChildKindV1::CandidateBundle => 0,
            EpochChildKindV1::CandidateIndexPage => 1,
            EpochChildKindV1::CandidateVerdict => 2,
            EpochChildKindV1::CandidateEscrow => 3,
            EpochChildKindV1::ClearWorkBundle => 4,
            EpochChildKindV1::OrderPage => 5,
            EpochChildKindV1::ReservationArchive => 6,
            EpochChildKindV1::SettlementReceipt => 7,
            EpochChildKindV1::FinalPot => 8,
        }
    }
}

/// Opaque, schema-qualified status witnessed by a candidate lifecycle owner.
///
/// Retirement deliberately does not interpret or persist this token. The
/// adapter constructs it only after the account family's exact decoder and
/// lifecycle transition have validated the `(tag, version, status)` triple.
/// Keeping the bytes opaque gives candidate semantics one owner while proving
/// that every status in every admitted schema remains one counted child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateStatusWitnessV1 {
    schema_tag: u8,
    schema_version: u8,
    status: u8,
}

impl CandidateStatusWitnessV1 {
    /// Record a status triple already validated by its lifecycle owner.
    ///
    /// This is an adapter trust-boundary constructor, not a candidate decoder.
    /// No byte value is rejected here because only the named external schema
    /// owns which status values are canonical.
    pub const fn from_validated_account(schema_tag: u8, schema_version: u8, status: u8) -> Self {
        Self {
            schema_tag,
            schema_version,
            status,
        }
    }

    /// Candidate account discriminator supplied by the owning decoder.
    pub const fn schema_tag(self) -> u8 {
        self.schema_tag
    }

    /// Candidate account version supplied by the owning decoder.
    pub const fn schema_version(self) -> u8 {
        self.schema_version
    }

    /// Opaque status byte supplied by the owning decoder.
    pub const fn status(self) -> u8 {
        self.status
    }
}

/// Semantic reservation ownership phase shared by counted reservation families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReservationStateV1 {
    /// Reservation owns a live order envelope.
    Active = 0,
    /// Remaining envelope returned.
    Released = 1,
    /// Immutable settlement entitlement exists.
    Entitled = 2,
    /// Entitlement, quantity, and payment are complete.
    Consumed = 3,
}

impl ReservationStateV1 {
    /// Whether this state must carry `position_counted = 1`.
    pub const fn is_position_counted(self) -> bool {
        matches!(self, Self::Active | Self::Entitled)
    }

    /// Whether no economic reservation obligation remains.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Released | Self::Consumed)
    }
}

impl TryFrom<u8> for ReservationStateV1 {
    type Error = RetirementErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::Released),
            2 => Ok(Self::Entitled),
            3 => Ok(Self::Consumed),
            _ => Err(RetirementErrorV1::InvalidEnum),
        }
    }
}

impl From<ReservationStateV1> for u8 {
    fn from(value: ReservationStateV1) -> Self {
        match value {
            ReservationStateV1::Active => 0,
            ReservationStateV1::Released => 1,
            ReservationStateV1::Entitled => 2,
            ReservationStateV1::Consumed => 3,
        }
    }
}

/// Live or terminal general Epoch phase needed by retirement transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralEpochPhaseV1 {
    /// Placement/candidate work remains admitted.
    Open,
    /// A candidate cleared under the original three-phase projection.
    Cleared,
    /// The Epoch terminally lapsed.
    Lapsed,
}

/// Successor general-Epoch lifecycle used by complete retirement planning.
///
/// This keeps the frozen three-variant [`GeneralEpochPhaseV1`] API intact while
/// naming the distinct frozen-work and economically-settled states required by
/// counted cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralEpochPhaseV2 {
    /// Placement/candidate work remains admitted.
    Open,
    /// The order set is frozen and candidate/clear work is admitted.
    Frozen,
    /// A candidate cleared and entitlement/settlement work is admitted.
    Cleared,
    /// Every settlement dependency is economically terminal.
    Settled,
    /// The Epoch terminally lapsed.
    Lapsed,
}
