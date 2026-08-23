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

mod codec;
mod transition;

pub use codec::{
    ChildGenerationV1, EpochChildCountsV1, EpochRetirementTailV1, GeneralEpochTombstoneV1,
    Identity32V1, MarketEpochCursorV1, PositionRetirementTailV1, PositionTombstoneV1, RentSplitV2,
    ReservationCountTailV1,
};
pub use transition::{
    close_epoch, close_epoch_child, close_general_reservation_archive, close_position,
    close_registered_candidate, create_epoch_child, create_registered_candidate_after_validation,
    entitle_reservation, open_general_epoch, register_direct_reservation,
    register_general_reservation, reopen_position, terminate_reservation,
    update_registered_candidate_status_after_validation, AuthenticatedEpochChildV1, ChildSlotV1,
    CountedReservationV1, EpochLifecycleStateV3, LiveEpochV3, LivePositionV2,
    PositionEconomicStateV1, PositionLifecycleStateV2, RentDispositionV2,
};

/// Number of bytes in every persisted identity.
pub const IDENTITY_BYTES: usize = 32;
/// Fixed active outcome width inherited from the current Position layout.
pub const MAX_OUTCOMES: usize = 16;

/// Existing Position account discriminator.
pub const POSITION_ACCOUNT_TAG: u8 = 6;
/// Counted Position schema.
pub const POSITION_ACCOUNT_VERSION_V2: u8 = 2;
/// Existing Epoch account discriminator.
pub const EPOCH_ACCOUNT_TAG: u8 = 11;
/// Counted general Epoch schema.
pub const EPOCH_ACCOUNT_VERSION_V3: u8 = 3;
/// Existing Market account discriminator.
pub const MARKET_ACCOUNT_TAG: u8 = 3;
/// Monotone-cursor Market schema.
pub const MARKET_ACCOUNT_VERSION_V2: u8 = 2;
/// Existing general reservation discriminator.
pub const RESERVATION_ACCOUNT_TAG: u8 = 19;
/// Counted general reservation schema.
pub const RESERVATION_ACCOUNT_VERSION_V5: u8 = 5;

/// Codec-local provisional Position tombstone discriminator.
///
/// This is not a live wire allocation; integration must first reserve it in
/// the authoritative account-tag registry and live router.
pub const POSITION_TOMBSTONE_TAG: u8 = 0x75;
/// First Position tombstone schema.
pub const POSITION_TOMBSTONE_VERSION_V1: u8 = 1;
/// Codec-local provisional general Epoch tombstone discriminator.
///
/// This is not a live wire allocation; integration must first reserve it in
/// the authoritative account-tag registry and live router.
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

/// Exact rent split tail width.
pub const RENT_SPLIT_V2_BYTES: usize = 56;
/// Exact counted Position extension width.
pub const POSITION_RETIREMENT_TAIL_V1_BYTES: usize = 60;
/// Exact epoch-child counter width.
pub const EPOCH_CHILD_COUNTS_V1_BYTES: usize = 36;
/// Exact counted Epoch extension width.
pub const EPOCH_RETIREMENT_TAIL_V1_BYTES: usize = 100;
/// Exact monotone Market cursor width.
pub const MARKET_EPOCH_CURSOR_V1_BYTES: usize = 8;
/// Exact reservation counter-marker extension width.
pub const RESERVATION_COUNT_TAIL_V1_BYTES: usize = 9;
/// Exact common epoch-generation extension width for other child schemas.
pub const CHILD_GENERATION_V1_BYTES: usize = 8;

/// Exact full Position V2 width after composition.
pub const POSITION_V2_BYTES: usize = POSITION_V1_BYTES + POSITION_RETIREMENT_TAIL_V1_BYTES;
/// Exact full Epoch V3 width after composition.
pub const EPOCH_V3_BYTES: usize = EPOCH_V2_BYTES + EPOCH_RETIREMENT_TAIL_V1_BYTES;
/// Exact full Market V2 width after composition.
pub const MARKET_V2_BYTES: usize = MARKET_V1_BYTES + MARKET_EPOCH_CURSOR_V1_BYTES;
/// Exact full general Reservation V5 width after composition.
pub const RESERVATION_V5_BYTES: usize = RESERVATION_V4_BYTES + RESERVATION_COUNT_TAIL_V1_BYTES;
/// Exact Position tombstone width.
pub const POSITION_TOMBSTONE_V1_BYTES: usize = 76;
/// Exact general Epoch tombstone width.
pub const GENERAL_EPOCH_TOMBSTONE_V1_BYTES: usize = 84;

const _: () = assert!(POSITION_V2_BYTES == 280);
const _: () = assert!(EPOCH_V3_BYTES == 429);
const _: () = assert!(MARKET_V2_BYTES == 734);
const _: () = assert!(RESERVATION_V5_BYTES == 627);

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

/// One of the nine exhaustive Epoch child classes in ADR-0007.
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

/// Live or terminal general Epoch phase needed by retirement transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralEpochPhaseV1 {
    /// Placement/candidate work remains admitted.
    Open,
    /// A candidate cleared and settlement is terminal.
    Cleared,
    /// The Epoch terminally lapsed.
    Lapsed,
}
