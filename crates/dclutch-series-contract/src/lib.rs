#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact, SDK-free semantics for a finite, presently capitalized Series.
//!
//! A Series is a standalone factory. It is never a Market child and never
//! owns Market truth. Immutable content selects recurrence and derivation
//! releases; one mutable root owns only gap-free progress and conservation.

use core::convert::{TryFrom, TryInto};
use dclutch_product_contract::{
    ContentId,
    capacity::CapacityProfileId,
    product::{InstanceV1, InstanceV1Input},
};
use sha2::{Digest, Sha256};

/// Width of a content identity or Solana-compatible account key.
pub const IDENTITY_BYTES: usize = 32;
/// Exact width of [`SeriesRecipeV1`].
pub const SERIES_RECIPE_BYTES_V1: usize = 496;
/// Exact width of [`DerivedOccurrenceV1`].
pub const DERIVED_OCCURRENCE_BYTES_V1: usize = 400;
/// Exact width of [`CapitalizationAggregateV1`].
pub const CAPITALIZATION_AGGREGATE_BYTES_V1: usize = 128;
/// Exact width of [`OccurrenceCapitalizationV1`].
pub const OCCURRENCE_CAPITALIZATION_BYTES_V1: usize = 144;
/// Exact width of [`SeriesRootV1`].
pub const SERIES_ROOT_BYTES_V1: usize = 216;
/// Exact width of [`SeriesEscrowV1`].
pub const SERIES_ESCROW_BYTES_V1: usize = 144;
/// Exact width of the permanent [`SeriesReplayGuardV1`].
pub const SERIES_REPLAY_GUARD_BYTES_V1: usize = 48;
/// Exact width of [`OccurrenceTicketV1`].
pub const OCCURRENCE_TICKET_BYTES_V1: usize = 248;

/// Exact width of a shared instruction header.
pub const INSTRUCTION_HEADER_BYTES_V1: usize = 16;
/// Exact width of [`CreateSeriesV1`].
pub const CREATE_SERIES_BYTES_V1: usize = 56;
/// Exact width of [`InstantiateNextV1`].
pub const INSTANTIATE_NEXT_BYTES_V1: usize = 40;
/// Exact width of [`ConsumeTicketV1`].
pub const CONSUME_TICKET_BYTES_V1: usize = 24;
/// Exact width of [`CloseExhaustedV1`].
pub const CLOSE_EXHAUSTED_BYTES_V1: usize = 24;

/// Persistent recipe magic.
pub const SERIES_RECIPE_MAGIC_V1: [u8; 8] = *b"DCLTSER1";
/// Persistent derived-occurrence magic.
pub const DERIVED_OCCURRENCE_MAGIC_V1: [u8; 8] = *b"DCLTSDV1";
/// Persistent capitalization-aggregate magic.
pub const CAPITALIZATION_AGGREGATE_MAGIC_V1: [u8; 8] = *b"DCLTSCA1";
/// Persistent occurrence-capitalization magic.
pub const OCCURRENCE_CAPITALIZATION_MAGIC_V1: [u8; 8] = *b"DCLTSCV1";
/// Persistent root magic.
pub const SERIES_ROOT_MAGIC_V1: [u8; 8] = *b"DCLTSRT1";
/// Persistent escrow magic.
pub const SERIES_ESCROW_MAGIC_V1: [u8; 8] = *b"DCLTSES1";
/// Persistent permanent replay-guard magic.
pub const SERIES_REPLAY_GUARD_MAGIC_V1: [u8; 8] = *b"DCLTSGD1";
/// Persistent one-use ticket magic.
pub const OCCURRENCE_TICKET_MAGIC_V1: [u8; 8] = *b"DCLTSTK1";
/// Instruction magic shared by all V1 Series actions.
pub const SERIES_INSTRUCTION_MAGIC_V1: [u8; 8] = *b"DCLTSRI1";
/// Implemented persistent and instruction schema version.
pub const SERIES_SCHEMA_VERSION_V1: u16 = 1;

/// Exact release label for V1's fixed canonical Product construction.
pub const PRODUCT_COMPILER_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/series-product-compiler/fixed-result-domain-occurrence-v2";
/// SHA-256 identity of [`PRODUCT_COMPILER_RELEASE_PREIMAGE_V1`].
pub const PRODUCT_COMPILER_RELEASE_ID_V1: [u8; 32] = [
    0x81, 0x2b, 0x11, 0x25, 0xf6, 0x99, 0xc8, 0xdf, 0xd0, 0x2e, 0x87, 0x02, 0x67, 0x3b, 0x99, 0xea,
    0xde, 0x92, 0x1e, 0xb8, 0x74, 0x44, 0x99, 0x38, 0x5a, 0xa5, 0x8f, 0x60, 0x8b, 0x1b, 0x93, 0x11,
];
/// Exact release label for V1's fixed occurrence-artifact/Product derivation.
pub const OCCURRENCE_DERIVATION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/series-occurrence-derivation/fixed-v1";
/// SHA-256 identity of [`OCCURRENCE_DERIVATION_RELEASE_PREIMAGE_V1`].
pub const OCCURRENCE_DERIVATION_RELEASE_ID_V1: [u8; 32] = [
    0xa2, 0x4a, 0x68, 0xe4, 0x1b, 0x4f, 0x00, 0x3a, 0xf2, 0xa5, 0x06, 0xca, 0xa0, 0xa6, 0x31, 0x61,
    0x7f, 0x9e, 0x78, 0x60, 0x35, 0xf7, 0xdc, 0xf9, 0x7b, 0xd5, 0x67, 0x2f, 0x5f, 0x2e, 0x3c, 0x4b,
];
/// Exact release label for V1's immutable shared source-policy selection.
pub const SOURCE_DERIVATION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/series-source-derivation/static-policy-v1";
/// SHA-256 identity of [`SOURCE_DERIVATION_RELEASE_PREIMAGE_V1`].
pub const SOURCE_DERIVATION_RELEASE_ID_V1: [u8; 32] = [
    0x93, 0x95, 0xcc, 0xf5, 0xf0, 0x49, 0xf9, 0xc3, 0xbb, 0xcb, 0xfa, 0xcb, 0x41, 0x85, 0x44, 0x0f,
    0x45, 0x9e, 0x24, 0x0b, 0xa5, 0x99, 0xaa, 0x25, 0x4b, 0x0b, 0x33, 0x12, 0xda, 0x2e, 0xdf, 0x43,
];
/// Exact release label for V1's immutable shared capability-manifest selection.
pub const CAPABILITY_DERIVATION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/series-capability-derivation/static-manifest-v1";
/// SHA-256 identity of [`CAPABILITY_DERIVATION_RELEASE_PREIMAGE_V1`].
pub const CAPABILITY_DERIVATION_RELEASE_ID_V1: [u8; 32] = [
    0x0b, 0x93, 0x01, 0x4b, 0xd4, 0xb4, 0x85, 0x44, 0xed, 0x77, 0x83, 0x43, 0x09, 0x38, 0xf6, 0xa2,
    0x1d, 0x3c, 0x5f, 0x88, 0xac, 0x45, 0xaf, 0x10, 0x30, 0x1c, 0x72, 0xdf, 0xaf, 0x9d, 0xa1, 0x89,
];
/// Exact release label for V1's canonical Market-identity derivation.
pub const MARKET_DERIVATION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/series-market-derivation/canonical-v1";
/// SHA-256 identity of [`MARKET_DERIVATION_RELEASE_PREIMAGE_V1`].
pub const MARKET_DERIVATION_RELEASE_ID_V1: [u8; 32] = [
    0xcf, 0x7f, 0x9e, 0xfc, 0xc5, 0x68, 0x36, 0x83, 0x84, 0x54, 0xd6, 0x0c, 0x52, 0x36, 0x97, 0xe5,
    0x2f, 0x36, 0x63, 0xe7, 0x57, 0xcc, 0x67, 0x07, 0x73, 0xcd, 0x46, 0x1a, 0xe5, 0x28, 0x33, 0x21,
];

/// Fixed V1 occurrence artifact width: header, recipe/schedule, index, time, generation.
pub const SERIES_OCCURRENCE_ARTIFACT_BYTES_V1: usize = 104;

const SERIES_OCCURRENCE_ARTIFACT_MAGIC_V1: [u8; 8] = *b"DCLTSAO1";
const PRODUCT_OCCURRENCE_BYTES_V1: usize = 128;
const MARKET_IDENTITY_BYTES_V1: usize = 168;

/// Root PDA domain. Its 22 bytes are below the chain-derived 32-byte seed limit.
pub const SERIES_ROOT_PDA_DOMAIN_V1: &[u8] = b"dclutch/series-root/v1";
/// Escrow PDA domain. Its 24 bytes are below the chain-derived seed limit.
pub const SERIES_ESCROW_PDA_DOMAIN_V1: &[u8] = b"dclutch/series-escrow/v1";
/// Ticket PDA domain. Its 24 bytes are below the chain-derived seed limit.
pub const SERIES_TICKET_PDA_DOMAIN_V1: &[u8] = b"dclutch/series-ticket/v1";
/// Replay-guard PDA domain. Its 23 bytes are below the chain-derived seed limit.
pub const SERIES_REPLAY_GUARD_PDA_DOMAIN_V1: &[u8] = b"dclutch/series-guard/v1";
/// Width of the exact root PDA derivation preimage.
pub const SERIES_ROOT_PDA_PREIMAGE_BYTES_V1: usize = 119;
/// Width of the exact escrow PDA derivation preimage.
pub const SERIES_ESCROW_PDA_PREIMAGE_BYTES_V1: usize = 57;
/// Width of the exact ticket PDA derivation preimage.
pub const SERIES_TICKET_PDA_PREIMAGE_BYTES_V1: usize = 65;
/// Width of the exact replay-guard PDA derivation preimage.
pub const SERIES_REPLAY_GUARD_PDA_PREIMAGE_BYTES_V1: usize = 56;

/// Canonical System Program key bytes.
pub const SYSTEM_PROGRAM_ID: [u8; IDENTITY_BYTES] = [0; IDENTITY_BYTES];
/// Canonical Rent sysvar key bytes.
pub const RENT_SYSVAR_ID: [u8; IDENTITY_BYTES] = [
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
];

const PROVISIONAL_MIN_OUTCOMES_V1: u16 = 2;
const PROVISIONAL_MAX_OUTCOMES_V1: u16 = 16;

/// A refusal from hostile decoding, frame validation, or a pure transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have its one canonical byte width.
    InvalidLength,
    /// Magic did not identify the expected record or instruction.
    InvalidMagic,
    /// Schema version is not implemented.
    UnsupportedSchema,
    /// Reserved bytes were not canonical zeroes.
    NonCanonicalReservedBytes,
    /// An action discriminator is not implemented.
    UnknownAction,
    /// A persisted phase discriminator is not implemented.
    UnknownPhase,
    /// An identity, authority, or ordinary account key was zero.
    ZeroIdentity,
    /// Outcome count is outside the provisional categorical profile.
    UnsupportedOutcomeCount,
    /// Cadence was zero.
    InvalidCadence,
    /// A Series must contain at least one occurrence.
    EmptySeries,
    /// Final occurrence time did not fit the chain clock representation.
    ScheduleOverflow,
    /// Final generation did not fit its representation.
    GenerationOverflow,
    /// An occurrence index is outside the immutable recipe.
    IndexOutOfRange,
    /// Immutable recipe identity or content did not match.
    RecipeMismatch,
    /// Aggregate identity or content did not match.
    AggregateMismatch,
    /// Per-occurrence capitalization did not match.
    CapitalizationMismatch,
    /// A derived occurrence did not match the selected recipe/index.
    DerivationMismatch,
    /// The recipe selected a derivation release combination absent from V1.
    DerivationReleaseUnavailable,
    /// Root conservation or phase invariants were false.
    RootInvariant,
    /// The action is not legal in the current root phase.
    InvalidPhase,
    /// The submitted occurrence index was stale or skipped.
    IndexMismatch,
    /// The submitted occurrence time was not exactly next.
    TimeMismatch,
    /// The authenticated chain clock had not reached the next scheduled time.
    OccurrenceNotDue,
    /// Required presently funded principal was absent.
    Underfunded,
    /// Observed escrow balance could not cover rent plus root-owned principal.
    PresentPrincipalMismatch,
    /// A destination was not a data-empty, nonexecutable System-owned account.
    AccountNotVacant,
    /// Exact checked integer arithmetic failed.
    ArithmeticOverflow,
    /// An account did not have its exact required privileges.
    InvalidAccountPrivilege,
    /// Two roles that must be distinct aliased one account.
    AccountAlias,
    /// The System Program role was not canonical.
    InvalidSystemProgram,
    /// The Rent sysvar role was not canonical.
    InvalidRentSysvar,
    /// A ticket did not bind the requested Series occurrence.
    TicketMismatch,
    /// A ticket account balance could not fund its exact obligations.
    TicketBalanceMismatch,
    /// A close was attempted before finite exhaustion.
    SeriesNotExhausted,
    /// Tickets remain to be consumed through Found.
    OutstandingTickets,
    /// Spendable Series principal remained at close.
    ClosePrincipalRemaining,
    /// A permanent replay guard was below its authenticated rent floor.
    ReplayGuardUnderfunded,
    /// A replay guard did not bind the canonical Series root address.
    ReplayGuardMismatch,
    /// V1 replay guards are intentionally permanent and cannot close.
    PermanentReplayGuard,
}

/// Result alias for Series operations.
pub type Result<T> = core::result::Result<T, Error>;

/// A validated nonzero 32-byte content identity, authority, or account key.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct IdentityV1([u8; IDENTITY_BYTES]);

impl IdentityV1 {
    /// Construct a nonzero identity.
    pub fn new(bytes: [u8; IDENTITY_BYTES]) -> Result<Self> {
        if is_zero(&bytes) {
            return Err(Error::ZeroIdentity);
        }
        Ok(Self(bytes))
    }

    /// Decode a nonzero identity at an exact offset.
    pub fn decode_at(bytes: &[u8], offset: usize) -> Result<Self> {
        Self::new(read_array(bytes, offset)?)
    }

    /// Return canonical identity bytes.
    pub const fn to_bytes(self) -> [u8; IDENTITY_BYTES] {
        self.0
    }
}

/// Exact ordered PDA preimage for one Series root.
///
/// The adapter derives with five seeds in this order: root domain, recipe ID,
/// aggregate ID, refund authority, and the one-byte bump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRootPdaPreimageV1 {
    /// Immutable recipe identity seed.
    pub recipe_id: IdentityV1,
    /// Immutable capitalization aggregate identity seed.
    pub aggregate_id: IdentityV1,
    /// Immutable refund authority seed.
    pub refund_authority: IdentityV1,
    /// Canonical one-byte bump seed.
    pub bump: u8,
}

impl SeriesRootPdaPreimageV1 {
    /// Encode the fixed-width concatenation used by derivation fixtures.
    pub fn to_bytes(self) -> [u8; SERIES_ROOT_PDA_PREIMAGE_BYTES_V1] {
        let mut output = [0; SERIES_ROOT_PDA_PREIMAGE_BYTES_V1];
        put(&mut output, 0, SERIES_ROOT_PDA_DOMAIN_V1);
        put(&mut output, 22, &self.recipe_id.to_bytes());
        put(&mut output, 54, &self.aggregate_id.to_bytes());
        put(&mut output, 86, &self.refund_authority.to_bytes());
        put(&mut output, 118, &[self.bump]);
        output
    }
}

/// Exact ordered PDA preimage for the one escrow belonging to a root.
///
/// The adapter derives with three seeds in this order: escrow domain, root
/// address, and the one-byte bump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesEscrowPdaPreimageV1 {
    /// Root account address seed.
    pub series_root_address: IdentityV1,
    /// Canonical one-byte bump seed.
    pub bump: u8,
}

impl SeriesEscrowPdaPreimageV1 {
    /// Encode the fixed-width concatenation used by derivation fixtures.
    pub fn to_bytes(self) -> [u8; SERIES_ESCROW_PDA_PREIMAGE_BYTES_V1] {
        let mut output = [0; SERIES_ESCROW_PDA_PREIMAGE_BYTES_V1];
        put(&mut output, 0, SERIES_ESCROW_PDA_DOMAIN_V1);
        put(&mut output, 24, &self.series_root_address.to_bytes());
        put(&mut output, 56, &[self.bump]);
        output
    }
}

/// Exact ordered PDA preimage for one root/index one-use ticket.
///
/// The adapter derives with four seeds in this order: ticket domain, root
/// address, little-endian occurrence index, and the one-byte bump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceTicketPdaPreimageV1 {
    /// Root account address seed.
    pub series_root_address: IdentityV1,
    /// Exact occurrence index seed.
    pub occurrence_index: u64,
    /// Canonical one-byte bump seed.
    pub bump: u8,
}

impl OccurrenceTicketPdaPreimageV1 {
    /// Encode the fixed-width concatenation used by derivation fixtures.
    pub fn to_bytes(self) -> [u8; SERIES_TICKET_PDA_PREIMAGE_BYTES_V1] {
        let mut output = [0; SERIES_TICKET_PDA_PREIMAGE_BYTES_V1];
        put(&mut output, 0, SERIES_TICKET_PDA_DOMAIN_V1);
        put(&mut output, 24, &self.series_root_address.to_bytes());
        put(&mut output, 56, &self.occurrence_index.to_le_bytes());
        put(&mut output, 64, &[self.bump]);
        output
    }
}

/// Exact ordered PDA preimage for the permanent guard belonging to a root.
///
/// The adapter derives with three seeds in this order: replay-guard domain,
/// root address, and the one-byte bump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesReplayGuardPdaPreimageV1 {
    /// Root account address seed.
    pub series_root_address: IdentityV1,
    /// Canonical one-byte bump seed.
    pub bump: u8,
}

impl SeriesReplayGuardPdaPreimageV1 {
    /// Encode the fixed-width concatenation used by derivation fixtures.
    pub fn to_bytes(self) -> [u8; SERIES_REPLAY_GUARD_PDA_PREIMAGE_BYTES_V1] {
        let mut output = [0; SERIES_REPLAY_GUARD_PDA_PREIMAGE_BYTES_V1];
        put(&mut output, 0, SERIES_REPLAY_GUARD_PDA_DOMAIN_V1);
        put(&mut output, 23, &self.series_root_address.to_bytes());
        put(&mut output, 55, &[self.bump]);
        output
    }
}

/// Immutable content preimage selecting recurrence and every derivation release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRecipeV1 {
    /// Realm identity selecting collateral and protocol constitution.
    pub realm_id: IdentityV1,
    /// Terms identity.
    pub terms_id: IdentityV1,
    /// Exact categorical claim-basis identity.
    pub claim_basis_id: IdentityV1,
    /// Product-owned finite result-domain identity, including failure outcome.
    pub result_domain_id: IdentityV1,
    /// Capacity-profile identity.
    pub capacity_profile_id: IdentityV1,
    /// Product compiler release identity.
    pub compiler_release_id: IdentityV1,
    /// Occurrence schedule identity.
    pub occurrence_schedule_id: IdentityV1,
    /// Source schedule identity.
    pub source_schedule_id: IdentityV1,
    /// Capability-template identity.
    pub capability_template_id: IdentityV1,
    /// Occurrence derivation release identity.
    pub occurrence_derivation_release_id: IdentityV1,
    /// Source derivation release identity.
    pub source_derivation_release_id: IdentityV1,
    /// Capability-manifest derivation release identity.
    pub capability_derivation_release_id: IdentityV1,
    /// Market-identity derivation release identity.
    pub market_derivation_release_id: IdentityV1,
    /// Capitalization-schedule identity.
    pub capitalization_schedule_id: IdentityV1,
    /// First scheduled Unix timestamp.
    pub first_occurrence_time: i64,
    /// Positive seconds between consecutive occurrences.
    pub cadence_seconds: u64,
    /// Finite number of occurrences.
    pub occurrence_count: u64,
    /// Generation assigned to occurrence zero.
    pub first_generation: u64,
    /// Exact number of mutually exclusive categorical outcomes.
    pub outcome_count: u16,
}

impl SeriesRecipeV1 {
    /// Validate all V1 bounds and prove last time/generation are representable.
    pub fn validate(&self) -> Result<()> {
        if !(PROVISIONAL_MIN_OUTCOMES_V1..=PROVISIONAL_MAX_OUTCOMES_V1)
            .contains(&self.outcome_count)
        {
            return Err(Error::UnsupportedOutcomeCount);
        }
        if self.cadence_seconds == 0 {
            return Err(Error::InvalidCadence);
        }
        if self.occurrence_count == 0 {
            return Err(Error::EmptySeries);
        }
        self.validate_derivation_releases()?;
        let last = self
            .occurrence_count
            .checked_sub(1)
            .ok_or(Error::EmptySeries)?;
        self.time_at(last)?;
        self.generation_at(last)?;
        Ok(())
    }

    /// Require the one closed derivation release set implemented by V1.
    pub fn validate_derivation_releases(&self) -> Result<()> {
        if self.compiler_release_id.to_bytes() != PRODUCT_COMPILER_RELEASE_ID_V1
            || self.occurrence_derivation_release_id.to_bytes()
                != OCCURRENCE_DERIVATION_RELEASE_ID_V1
            || self.source_derivation_release_id.to_bytes() != SOURCE_DERIVATION_RELEASE_ID_V1
            || self.capability_derivation_release_id.to_bytes()
                != CAPABILITY_DERIVATION_RELEASE_ID_V1
            || self.market_derivation_release_id.to_bytes() != MARKET_DERIVATION_RELEASE_ID_V1
        {
            return Err(Error::DerivationReleaseUnavailable);
        }
        Ok(())
    }

    /// Return the exact scheduled time for an in-range occurrence index.
    pub fn time_at(&self, index: u64) -> Result<i64> {
        if index >= self.occurrence_count {
            return Err(Error::IndexOutOfRange);
        }
        let cadence = i128::from(self.cadence_seconds);
        let index_i128 = i128::from(index);
        let delta = cadence
            .checked_mul(index_i128)
            .ok_or(Error::ScheduleOverflow)?;
        let exact = i128::from(self.first_occurrence_time)
            .checked_add(delta)
            .ok_or(Error::ScheduleOverflow)?;
        i64::try_from(exact).map_err(|_| Error::ScheduleOverflow)
    }

    /// Return the exact generation for an in-range occurrence index.
    pub fn generation_at(&self, index: u64) -> Result<u64> {
        if index >= self.occurrence_count {
            return Err(Error::IndexOutOfRange);
        }
        self.first_generation
            .checked_add(index)
            .ok_or(Error::GenerationOverflow)
    }

    /// Hostile-decode one exact canonical recipe content preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_record_header(bytes, SERIES_RECIPE_BYTES_V1, SERIES_RECIPE_MAGIC_V1)?;
        let value = Self {
            realm_id: IdentityV1::decode_at(bytes, 16)?,
            terms_id: IdentityV1::decode_at(bytes, 48)?,
            claim_basis_id: IdentityV1::decode_at(bytes, 80)?,
            capacity_profile_id: IdentityV1::decode_at(bytes, 112)?,
            compiler_release_id: IdentityV1::decode_at(bytes, 144)?,
            occurrence_schedule_id: IdentityV1::decode_at(bytes, 176)?,
            source_schedule_id: IdentityV1::decode_at(bytes, 208)?,
            capability_template_id: IdentityV1::decode_at(bytes, 240)?,
            occurrence_derivation_release_id: IdentityV1::decode_at(bytes, 272)?,
            source_derivation_release_id: IdentityV1::decode_at(bytes, 304)?,
            capability_derivation_release_id: IdentityV1::decode_at(bytes, 336)?,
            market_derivation_release_id: IdentityV1::decode_at(bytes, 368)?,
            capitalization_schedule_id: IdentityV1::decode_at(bytes, 400)?,
            result_domain_id: IdentityV1::decode_at(bytes, 432)?,
            first_occurrence_time: read_i64(bytes, 464)?,
            cadence_seconds: read_u64(bytes, 472)?,
            occurrence_count: read_u64(bytes, 480)?,
            first_generation: read_u64(bytes, 488)?,
            outcome_count: read_u16(bytes, 10)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the one canonical content preimage used by the hashing adapter.
    pub fn to_bytes(self) -> [u8; SERIES_RECIPE_BYTES_V1] {
        let mut output = [0; SERIES_RECIPE_BYTES_V1];
        put_record_header(&mut output, &SERIES_RECIPE_MAGIC_V1);
        put(&mut output, 10, &self.outcome_count.to_le_bytes());
        let identities = [
            self.realm_id,
            self.terms_id,
            self.claim_basis_id,
            self.capacity_profile_id,
            self.compiler_release_id,
            self.occurrence_schedule_id,
            self.source_schedule_id,
            self.capability_template_id,
            self.occurrence_derivation_release_id,
            self.source_derivation_release_id,
            self.capability_derivation_release_id,
            self.market_derivation_release_id,
            self.capitalization_schedule_id,
            self.result_domain_id,
        ];
        let offsets = [
            16, 48, 80, 112, 144, 176, 208, 240, 272, 304, 336, 368, 400, 432,
        ];
        for (identity, offset) in identities.iter().zip(offsets.iter()) {
            put(&mut output, *offset, &identity.to_bytes());
        }
        put(&mut output, 464, &self.first_occurrence_time.to_le_bytes());
        put(&mut output, 472, &self.cadence_seconds.to_le_bytes());
        put(&mut output, 480, &self.occurrence_count.to_le_bytes());
        put(&mut output, 488, &self.first_generation.to_le_bytes());
        output
    }
}

/// Immutable output of the selected derivation releases for one occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedOccurrenceV1 {
    /// Recipe content identity.
    pub recipe_id: IdentityV1,
    /// Gap-free occurrence index.
    pub occurrence_index: u64,
    /// Exact scheduled time.
    pub occurrence_time: i64,
    /// Exact Market generation.
    pub generation: u64,
    /// Occurrence artifact identity.
    pub occurrence_artifact_id: IdentityV1,
    /// Occurrence identity consumed by Product construction.
    pub occurrence_id: IdentityV1,
    /// Product instance identity.
    pub product_instance_id: IdentityV1,
    /// Source specification identity.
    pub source_spec_id: IdentityV1,
    /// Source window identity.
    pub source_window_id: IdentityV1,
    /// Statistic identity.
    pub statistic_id: IdentityV1,
    /// Resolution-policy identity.
    pub resolution_policy_id: IdentityV1,
    /// Capability-manifest identity.
    pub capability_manifest_id: IdentityV1,
    /// Market identity that Found must create exactly.
    pub market_identity_id: IdentityV1,
    /// Exact occurrence-capitalization identity.
    pub capitalization_id: IdentityV1,
}

impl DerivedOccurrenceV1 {
    /// Check recipe identity, gap-free index, scheduled time, and generation.
    pub fn validate_for(
        &self,
        recipe_id: IdentityV1,
        recipe: &SeriesRecipeV1,
        index: u64,
    ) -> Result<()> {
        if self.recipe_id != recipe_id {
            return Err(Error::RecipeMismatch);
        }
        if self.occurrence_index != index {
            return Err(Error::DerivationMismatch);
        }
        if self.occurrence_time != recipe.time_at(index)?
            || self.generation != recipe.generation_at(index)?
        {
            return Err(Error::DerivationMismatch);
        }
        Ok(())
    }

    /// Hostile-decode one exact immutable derivation record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_record_header(
            bytes,
            DERIVED_OCCURRENCE_BYTES_V1,
            DERIVED_OCCURRENCE_MAGIC_V1,
        )?;
        require_zero(bytes, 10, 6)?;
        require_zero(bytes, 72, 8)?;
        Ok(Self {
            recipe_id: IdentityV1::decode_at(bytes, 16)?,
            occurrence_index: read_u64(bytes, 48)?,
            occurrence_time: read_i64(bytes, 56)?,
            generation: read_u64(bytes, 64)?,
            occurrence_artifact_id: IdentityV1::decode_at(bytes, 80)?,
            occurrence_id: IdentityV1::decode_at(bytes, 112)?,
            product_instance_id: IdentityV1::decode_at(bytes, 144)?,
            source_spec_id: IdentityV1::decode_at(bytes, 176)?,
            source_window_id: IdentityV1::decode_at(bytes, 208)?,
            statistic_id: IdentityV1::decode_at(bytes, 240)?,
            resolution_policy_id: IdentityV1::decode_at(bytes, 272)?,
            capability_manifest_id: IdentityV1::decode_at(bytes, 304)?,
            market_identity_id: IdentityV1::decode_at(bytes, 336)?,
            capitalization_id: IdentityV1::decode_at(bytes, 368)?,
        })
    }

    /// Encode the one canonical immutable derivation preimage.
    pub fn to_bytes(self) -> [u8; DERIVED_OCCURRENCE_BYTES_V1] {
        let mut output = [0; DERIVED_OCCURRENCE_BYTES_V1];
        put_record_header(&mut output, &DERIVED_OCCURRENCE_MAGIC_V1);
        put(&mut output, 16, &self.recipe_id.to_bytes());
        put(&mut output, 48, &self.occurrence_index.to_le_bytes());
        put(&mut output, 56, &self.occurrence_time.to_le_bytes());
        put(&mut output, 64, &self.generation.to_le_bytes());
        let identities = [
            self.occurrence_artifact_id,
            self.occurrence_id,
            self.product_instance_id,
            self.source_spec_id,
            self.source_window_id,
            self.statistic_id,
            self.resolution_policy_id,
            self.capability_manifest_id,
            self.market_identity_id,
            self.capitalization_id,
        ];
        let offsets = [80, 112, 144, 176, 208, 240, 272, 304, 336, 368];
        for (identity, offset) in identities.iter().zip(offsets.iter()) {
            put(&mut output, *offset, &identity.to_bytes());
        }
        output
    }
}

/// Recompute the only V1 derivation output from authenticated immutable inputs.
///
/// The fixed occurrence artifact is content-addressed. Its canonical Product
/// occurrence and Product instance preimages are then content-addressed in
/// order. V1 deliberately shares one immutable source-policy record and one
/// immutable capability manifest across the finite Series. The canonical
/// Market identity commits those results and the occurrence generation. The
/// capitalization identity is the SHA-256 identity of the exact item bytes.
pub fn derive_occurrence_v1(
    recipe_id: IdentityV1,
    recipe: &SeriesRecipeV1,
    occurrence_index: u64,
    capitalization: &OccurrenceCapitalizationV1,
) -> Result<DerivedOccurrenceV1> {
    recipe.validate()?;
    capitalization.validate()?;
    if capitalization.recipe_id != recipe_id
        || capitalization.capitalization_schedule_id != recipe.capitalization_schedule_id
        || capitalization.occurrence_index != occurrence_index
    {
        return Err(Error::CapitalizationMismatch);
    }
    let occurrence_time = recipe.time_at(occurrence_index)?;
    let generation = recipe.generation_at(occurrence_index)?;

    let mut artifact = [0u8; SERIES_OCCURRENCE_ARTIFACT_BYTES_V1];
    put(&mut artifact, 0, &SERIES_OCCURRENCE_ARTIFACT_MAGIC_V1);
    put(&mut artifact, 8, &SERIES_SCHEMA_VERSION_V1.to_le_bytes());
    put(&mut artifact, 16, &recipe_id.to_bytes());
    put(&mut artifact, 48, &recipe.occurrence_schedule_id.to_bytes());
    put(&mut artifact, 80, &occurrence_index.to_le_bytes());
    put(&mut artifact, 88, &occurrence_time.to_le_bytes());
    put(&mut artifact, 96, &generation.to_le_bytes());
    let occurrence_artifact_id = content_identity(&artifact)?;

    // Canonical dclutch-product-contract OccurrenceV1 bytes. The fixed
    // artifact is one nonempty canonical page in this selected release.
    let mut occurrence = [0u8; PRODUCT_OCCURRENCE_BYTES_V1];
    put(&mut occurrence, 0, b"DCLTOCC1");
    put(&mut occurrence, 8, &1u16.to_le_bytes());
    put(&mut occurrence, 16, &recipe.terms_id.to_bytes());
    put(&mut occurrence, 48, &recipe.capacity_profile_id.to_bytes());
    put(&mut occurrence, 80, &occurrence_artifact_id.to_bytes());
    let artifact_bytes = u32::try_from(SERIES_OCCURRENCE_ARTIFACT_BYTES_V1)
        .map_err(|_| Error::ArithmeticOverflow)?;
    put(&mut occurrence, 112, &artifact_bytes.to_le_bytes());
    put(&mut occurrence, 116, &1u32.to_le_bytes());
    let occurrence_id = content_identity(&occurrence)?;

    // The Product contract is the sole owner of the instance preimage. Series
    // supplies only authenticated recipe/occurrence facts to its constructor.
    let product = InstanceV1::new(InstanceV1Input {
        terms_id: product_content_id(recipe.terms_id)?,
        occurrence_id: product_content_id(occurrence_id)?,
        claim_basis_id: product_content_id(recipe.claim_basis_id)?,
        result_domain_id: product_content_id(recipe.result_domain_id)?,
        capacity_profile_id: CapacityProfileId::new(product_content_id(
            recipe.capacity_profile_id,
        )?),
        partition_cell_count: u32::from(recipe.outcome_count),
    })
    .map_err(|_| Error::DerivationMismatch)?;
    let product_instance_id = content_identity(&product.to_bytes())?;

    let source_spec_id = recipe.source_schedule_id;
    let source_window_id = recipe.source_schedule_id;
    let statistic_id = recipe.source_schedule_id;
    let resolution_policy_id = recipe.source_schedule_id;
    let capability_manifest_id = recipe.capability_template_id;

    // Canonical dclutch-core-contract MarketIdentity bytes.
    let mut market = [0u8; MARKET_IDENTITY_BYTES_V1];
    put(&mut market, 0, &recipe.realm_id.to_bytes());
    put(&mut market, 32, &product_instance_id.to_bytes());
    put(&mut market, 64, &recipe.claim_basis_id.to_bytes());
    put(&mut market, 96, &resolution_policy_id.to_bytes());
    put(&mut market, 128, &capability_manifest_id.to_bytes());
    put(&mut market, 160, &generation.to_le_bytes());

    Ok(DerivedOccurrenceV1 {
        recipe_id,
        occurrence_index,
        occurrence_time,
        generation,
        occurrence_artifact_id,
        occurrence_id,
        product_instance_id,
        source_spec_id,
        source_window_id,
        statistic_id,
        resolution_policy_id,
        capability_manifest_id,
        market_identity_id: content_identity(&market)?,
        capitalization_id: content_identity(&capitalization.to_bytes())?,
    })
}

/// Return the SHA-256 content identity of one exact canonical preimage.
pub fn content_identity(bytes: &[u8]) -> Result<IdentityV1> {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    IdentityV1::new(digest)
}

fn product_content_id(identity: IdentityV1) -> Result<ContentId> {
    ContentId::new(identity.to_bytes()).map_err(|_| Error::DerivationMismatch)
}

/// Immutable aggregate proving how much finite Series principal exists now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapitalizationAggregateV1 {
    /// Recipe identity.
    pub recipe_id: IdentityV1,
    /// Capitalization schedule identity.
    pub capitalization_schedule_id: IdentityV1,
    /// Exact finite item count.
    pub occurrence_count: u64,
    /// Exact sum of all Market and ticket-rent allocations.
    pub total_principal: u64,
    /// Content identity of the exact first item in the gap-free funding chain.
    pub first_capitalization_id: IdentityV1,
}

impl CapitalizationAggregateV1 {
    /// Validate nonempty, presently funded aggregate semantics.
    pub fn validate(&self) -> Result<()> {
        if self.occurrence_count == 0 {
            return Err(Error::EmptySeries);
        }
        if self.total_principal == 0 {
            return Err(Error::Underfunded);
        }
        Ok(())
    }

    /// Hostile-decode one exact aggregate record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_record_header(
            bytes,
            CAPITALIZATION_AGGREGATE_BYTES_V1,
            CAPITALIZATION_AGGREGATE_MAGIC_V1,
        )?;
        require_zero(bytes, 10, 6)?;
        let value = Self {
            recipe_id: IdentityV1::decode_at(bytes, 16)?,
            capitalization_schedule_id: IdentityV1::decode_at(bytes, 48)?,
            occurrence_count: read_u64(bytes, 80)?,
            total_principal: read_u64(bytes, 88)?,
            first_capitalization_id: IdentityV1::decode_at(bytes, 96)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one canonical aggregate content preimage.
    pub fn to_bytes(self) -> [u8; CAPITALIZATION_AGGREGATE_BYTES_V1] {
        let mut output = [0; CAPITALIZATION_AGGREGATE_BYTES_V1];
        put_record_header(&mut output, &CAPITALIZATION_AGGREGATE_MAGIC_V1);
        put(&mut output, 16, &self.recipe_id.to_bytes());
        put(&mut output, 48, &self.capitalization_schedule_id.to_bytes());
        put(&mut output, 80, &self.occurrence_count.to_le_bytes());
        put(&mut output, 88, &self.total_principal.to_le_bytes());
        put(&mut output, 96, &self.first_capitalization_id.to_bytes());
        output
    }
}

/// Immutable exact allocation for one occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceCapitalizationV1 {
    /// Recipe identity.
    pub recipe_id: IdentityV1,
    /// Capitalization schedule identity.
    pub capitalization_schedule_id: IdentityV1,
    /// Exact occurrence index.
    pub occurrence_index: u64,
    /// Principal routed only to Market Found obligations.
    pub market_principal: u64,
    /// Principal reserved only for the one-use ticket's rent.
    pub ticket_rent: u64,
    /// Checked sum of Market principal and ticket rent.
    pub total_principal: u64,
    /// Exact next item identity, or `None` only for the final occurrence.
    pub next_capitalization_id: Option<IdentityV1>,
}

impl OccurrenceCapitalizationV1 {
    /// Validate the exact positive split and checked sum.
    pub fn validate(&self) -> Result<()> {
        if self.market_principal == 0 || self.ticket_rent == 0 {
            return Err(Error::Underfunded);
        }
        if self
            .market_principal
            .checked_add(self.ticket_rent)
            .ok_or(Error::ArithmeticOverflow)?
            != self.total_principal
        {
            return Err(Error::CapitalizationMismatch);
        }
        Ok(())
    }

    /// Hostile-decode one exact capitalization item.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_record_header(
            bytes,
            OCCURRENCE_CAPITALIZATION_BYTES_V1,
            OCCURRENCE_CAPITALIZATION_MAGIC_V1,
        )?;
        require_zero(bytes, 10, 6)?;
        let value = Self {
            recipe_id: IdentityV1::decode_at(bytes, 16)?,
            capitalization_schedule_id: IdentityV1::decode_at(bytes, 48)?,
            occurrence_index: read_u64(bytes, 80)?,
            market_principal: read_u64(bytes, 88)?,
            ticket_rent: read_u64(bytes, 96)?,
            total_principal: read_u64(bytes, 104)?,
            next_capitalization_id: read_optional_identity(bytes, 112)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one canonical capitalization item content preimage.
    pub fn to_bytes(self) -> [u8; OCCURRENCE_CAPITALIZATION_BYTES_V1] {
        let mut output = [0; OCCURRENCE_CAPITALIZATION_BYTES_V1];
        put_record_header(&mut output, &OCCURRENCE_CAPITALIZATION_MAGIC_V1);
        put(&mut output, 16, &self.recipe_id.to_bytes());
        put(&mut output, 48, &self.capitalization_schedule_id.to_bytes());
        put(&mut output, 80, &self.occurrence_index.to_le_bytes());
        put(&mut output, 88, &self.market_principal.to_le_bytes());
        put(&mut output, 96, &self.ticket_rent.to_le_bytes());
        put(&mut output, 104, &self.total_principal.to_le_bytes());
        put_optional_identity(&mut output, 112, self.next_capitalization_id);
        output
    }
}

/// Mutable lifecycle phase of one finite Series root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesPhaseV1 {
    /// At least one exact next allocation remains.
    Active = 0,
    /// Every finite allocation has been released into a ticket.
    Exhausted = 1,
}

impl TryFrom<u8> for SeriesPhaseV1 {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::Exhausted),
            _ => Err(Error::UnknownPhase),
        }
    }
}

impl SeriesPhaseV1 {
    /// Return the canonical persisted discriminator without an unchecked cast.
    pub const fn discriminator(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Exhausted => 1,
        }
    }
}

/// Mutable semantic owner of gap-free progress and finite principal conservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRootV1 {
    /// Immutable recipe identity.
    pub recipe_id: IdentityV1,
    /// Immutable capitalization aggregate identity.
    pub aggregate_id: IdentityV1,
    /// Exact next index, equal to released allocation count.
    pub next_occurrence_index: u64,
    /// Exact next time while active; last released time while exhausted.
    pub next_occurrence_time: i64,
    /// Immutable finite allocation count.
    pub total_allocations: u64,
    /// Allocations still held in escrow.
    pub remaining_allocations: u64,
    /// Allocations released to one-use tickets.
    pub released_allocations: u64,
    /// Immutable initially deposited Series principal.
    pub initial_principal: u64,
    /// Exact principal still spendable in escrow.
    pub remaining_principal: u64,
    /// Exact principal released into tickets.
    pub released_principal: u64,
    /// Tickets not yet atomically consumed through Found.
    pub outstanding_tickets: u64,
    /// Exact next funding item in the immutable content-addressed chain.
    pub next_capitalization_id: Option<IdentityV1>,
    /// Immutable authority whose permanent RentCredit receives closes.
    pub refund_authority: IdentityV1,
    /// Lifecycle phase.
    pub phase: SeriesPhaseV1,
    /// Canonical root PDA bump.
    pub pda_bump: u8,
}

impl SeriesRootV1 {
    /// Construct a new fully prepaid root from immutable authenticated records.
    pub fn new(
        recipe_id: IdentityV1,
        aggregate_id: IdentityV1,
        recipe: &SeriesRecipeV1,
        aggregate: &CapitalizationAggregateV1,
        refund_authority: IdentityV1,
        pda_bump: u8,
    ) -> Result<Self> {
        recipe.validate()?;
        aggregate.validate()?;
        if aggregate.recipe_id != recipe_id
            || aggregate.capitalization_schedule_id != recipe.capitalization_schedule_id
            || aggregate.occurrence_count != recipe.occurrence_count
        {
            return Err(Error::AggregateMismatch);
        }
        let value = Self {
            recipe_id,
            aggregate_id,
            next_occurrence_index: 0,
            next_occurrence_time: recipe.first_occurrence_time,
            total_allocations: recipe.occurrence_count,
            remaining_allocations: recipe.occurrence_count,
            released_allocations: 0,
            initial_principal: aggregate.total_principal,
            remaining_principal: aggregate.total_principal,
            released_principal: 0,
            outstanding_tickets: 0,
            next_capitalization_id: Some(aggregate.first_capitalization_id),
            refund_authority,
            phase: SeriesPhaseV1::Active,
            pda_bump,
        };
        value.validate_internal()?;
        Ok(value)
    }

    /// Validate all representation-local conservation and phase invariants.
    pub fn validate_internal(&self) -> Result<()> {
        if self.total_allocations == 0 || self.initial_principal == 0 {
            return Err(Error::RootInvariant);
        }
        if self
            .remaining_allocations
            .checked_add(self.released_allocations)
            .ok_or(Error::RootInvariant)?
            != self.total_allocations
            || self
                .remaining_principal
                .checked_add(self.released_principal)
                .ok_or(Error::RootInvariant)?
                != self.initial_principal
            || self.next_occurrence_index != self.released_allocations
            || self.outstanding_tickets > self.released_allocations
        {
            return Err(Error::RootInvariant);
        }
        match self.phase {
            SeriesPhaseV1::Active
                if self.remaining_allocations > 0 && self.next_capitalization_id.is_some() =>
            {
                Ok(())
            }
            SeriesPhaseV1::Exhausted
                if self.remaining_allocations == 0
                    && self.remaining_principal == 0
                    && self.next_capitalization_id.is_none() =>
            {
                Ok(())
            }
            _ => Err(Error::RootInvariant),
        }
    }

    /// Check root content against its authenticated immutable owners.
    pub fn validate_for(
        &self,
        recipe_id: IdentityV1,
        aggregate_id: IdentityV1,
        recipe: &SeriesRecipeV1,
        aggregate: &CapitalizationAggregateV1,
    ) -> Result<()> {
        self.validate_internal()?;
        recipe.validate()?;
        aggregate.validate()?;
        if self.recipe_id != recipe_id || aggregate.recipe_id != recipe_id {
            return Err(Error::RecipeMismatch);
        }
        if self.aggregate_id != aggregate_id
            || aggregate.capitalization_schedule_id != recipe.capitalization_schedule_id
            || aggregate.occurrence_count != recipe.occurrence_count
            || self.total_allocations != aggregate.occurrence_count
            || self.initial_principal != aggregate.total_principal
        {
            return Err(Error::AggregateMismatch);
        }
        if self.released_allocations == 0
            && self.next_capitalization_id != Some(aggregate.first_capitalization_id)
        {
            return Err(Error::CapitalizationMismatch);
        }
        if self.phase == SeriesPhaseV1::Active
            && self.next_occurrence_time != recipe.time_at(self.next_occurrence_index)?
        {
            return Err(Error::TimeMismatch);
        }
        if self.phase == SeriesPhaseV1::Exhausted {
            let last = self
                .total_allocations
                .checked_sub(1)
                .ok_or(Error::RootInvariant)?;
            if self.next_occurrence_time != recipe.time_at(last)? {
                return Err(Error::TimeMismatch);
            }
        }
        Ok(())
    }

    /// Hostile-decode one exact mutable root.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_exact_magic_schema(bytes, SERIES_ROOT_BYTES_V1, SERIES_ROOT_MAGIC_V1)?;
        require_zero(bytes, 12, 4)?;
        let value = Self {
            recipe_id: IdentityV1::decode_at(bytes, 16)?,
            aggregate_id: IdentityV1::decode_at(bytes, 48)?,
            next_occurrence_index: read_u64(bytes, 80)?,
            next_occurrence_time: read_i64(bytes, 88)?,
            total_allocations: read_u64(bytes, 96)?,
            remaining_allocations: read_u64(bytes, 104)?,
            released_allocations: read_u64(bytes, 112)?,
            initial_principal: read_u64(bytes, 120)?,
            remaining_principal: read_u64(bytes, 128)?,
            released_principal: read_u64(bytes, 136)?,
            outstanding_tickets: read_u64(bytes, 144)?,
            refund_authority: IdentityV1::decode_at(bytes, 152)?,
            next_capitalization_id: read_optional_identity(bytes, 184)?,
            phase: SeriesPhaseV1::try_from(read_byte(bytes, 10)?)?,
            pda_bump: read_byte(bytes, 11)?,
        };
        value.validate_internal()?;
        Ok(value)
    }

    /// Encode one canonical mutable root representation.
    pub fn to_bytes(self) -> [u8; SERIES_ROOT_BYTES_V1] {
        let mut output = [0; SERIES_ROOT_BYTES_V1];
        put_record_header(&mut output, &SERIES_ROOT_MAGIC_V1);
        put(&mut output, 10, &[self.phase.discriminator()]);
        put(&mut output, 11, &[self.pda_bump]);
        put(&mut output, 16, &self.recipe_id.to_bytes());
        put(&mut output, 48, &self.aggregate_id.to_bytes());
        put(&mut output, 80, &self.next_occurrence_index.to_le_bytes());
        put(&mut output, 88, &self.next_occurrence_time.to_le_bytes());
        put(&mut output, 96, &self.total_allocations.to_le_bytes());
        put(&mut output, 104, &self.remaining_allocations.to_le_bytes());
        put(&mut output, 112, &self.released_allocations.to_le_bytes());
        put(&mut output, 120, &self.initial_principal.to_le_bytes());
        put(&mut output, 128, &self.remaining_principal.to_le_bytes());
        put(&mut output, 136, &self.released_principal.to_le_bytes());
        put(&mut output, 144, &self.outstanding_tickets.to_le_bytes());
        put(&mut output, 152, &self.refund_authority.to_bytes());
        put_optional_identity(&mut output, 184, self.next_capitalization_id);
        output
    }
}

/// Immutable binding of the physical Series escrow to its semantic owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesEscrowV1 {
    /// Root account address.
    pub series_root_address: IdentityV1,
    /// Recipe identity.
    pub recipe_id: IdentityV1,
    /// Capitalization aggregate identity.
    pub aggregate_id: IdentityV1,
    /// Immutable close beneficiary authority.
    pub refund_authority: IdentityV1,
    /// Canonical escrow PDA bump.
    pub pda_bump: u8,
}

impl SeriesEscrowV1 {
    /// Hostile-decode one exact escrow binding.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_exact_magic_schema(bytes, SERIES_ESCROW_BYTES_V1, SERIES_ESCROW_MAGIC_V1)?;
        require_zero(bytes, 11, 5)?;
        Ok(Self {
            series_root_address: IdentityV1::decode_at(bytes, 16)?,
            recipe_id: IdentityV1::decode_at(bytes, 48)?,
            aggregate_id: IdentityV1::decode_at(bytes, 80)?,
            refund_authority: IdentityV1::decode_at(bytes, 112)?,
            pda_bump: read_byte(bytes, 10)?,
        })
    }

    /// Encode one canonical escrow binding.
    pub fn to_bytes(self) -> [u8; SERIES_ESCROW_BYTES_V1] {
        let mut output = [0; SERIES_ESCROW_BYTES_V1];
        put_record_header(&mut output, &SERIES_ESCROW_MAGIC_V1);
        put(&mut output, 10, &[self.pda_bump]);
        put(&mut output, 16, &self.series_root_address.to_bytes());
        put(&mut output, 48, &self.recipe_id.to_bytes());
        put(&mut output, 80, &self.aggregate_id.to_bytes());
        put(&mut output, 112, &self.refund_authority.to_bytes());
        output
    }
}

/// Permanent small marker preventing a closed Series root from being recreated.
///
/// The guard is created atomically with the root and never closes. Exhausted
/// close retains its authenticated current rent floor and credits only guard
/// surplus to the immutable beneficiary's RentCredit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesReplayGuardV1 {
    /// Root account address protected for all time.
    pub series_root_address: IdentityV1,
    /// Canonical replay-guard PDA bump.
    pub pda_bump: u8,
}

impl SeriesReplayGuardV1 {
    /// Hostile-decode one exact permanent replay guard.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_exact_magic_schema(
            bytes,
            SERIES_REPLAY_GUARD_BYTES_V1,
            SERIES_REPLAY_GUARD_MAGIC_V1,
        )?;
        require_zero(bytes, 11, 5)?;
        Ok(Self {
            series_root_address: IdentityV1::decode_at(bytes, 16)?,
            pda_bump: read_byte(bytes, 10)?,
        })
    }

    /// Encode one canonical permanent replay guard.
    pub fn to_bytes(self) -> [u8; SERIES_REPLAY_GUARD_BYTES_V1] {
        let mut output = [0; SERIES_REPLAY_GUARD_BYTES_V1];
        put_record_header(&mut output, &SERIES_REPLAY_GUARD_MAGIC_V1);
        put(&mut output, 10, &[self.pda_bump]);
        put(&mut output, 16, &self.series_root_address.to_bytes());
        output
    }

    /// Refuse deletion: retaining this marker is the cross-lifecycle replay proof.
    pub const fn plan_close(self) -> Result<()> {
        let _ = self;
        Err(Error::PermanentReplayGuard)
    }
}

/// Compact one-use bridge between Series release and exact Market Found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceTicketV1 {
    /// Root account address.
    pub series_root_address: IdentityV1,
    /// Recipe identity.
    pub recipe_id: IdentityV1,
    /// Immutable derived-occurrence record identity.
    pub derived_occurrence_id: IdentityV1,
    /// Exact occurrence index.
    pub occurrence_index: u64,
    /// Exact scheduled occurrence time.
    pub occurrence_time: i64,
    /// Exact Market generation.
    pub generation: u64,
    /// Principal routed only to Market Found.
    pub market_principal: u64,
    /// Minimum ticket rent principal reserved at instantiation.
    pub ticket_rent: u64,
    /// Immutable permanent RentCredit beneficiary authority.
    pub refund_authority: IdentityV1,
    /// Immutable occurrence-capitalization identity.
    pub capitalization_id: IdentityV1,
    /// Exact Market identity that Found must create.
    pub market_identity_id: IdentityV1,
    /// Canonical ticket PDA bump.
    pub pda_bump: u8,
}

impl OccurrenceTicketV1 {
    /// Validate positive exact ticket funding semantics.
    pub fn validate(&self) -> Result<()> {
        self.market_principal
            .checked_add(self.ticket_rent)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.market_principal == 0 || self.ticket_rent == 0 {
            return Err(Error::Underfunded);
        }
        Ok(())
    }

    /// Hostile-decode one exact one-use ticket.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_exact_magic_schema(
            bytes,
            OCCURRENCE_TICKET_BYTES_V1,
            OCCURRENCE_TICKET_MAGIC_V1,
        )?;
        require_zero(bytes, 11, 5)?;
        let value = Self {
            series_root_address: IdentityV1::decode_at(bytes, 16)?,
            recipe_id: IdentityV1::decode_at(bytes, 48)?,
            derived_occurrence_id: IdentityV1::decode_at(bytes, 80)?,
            occurrence_index: read_u64(bytes, 112)?,
            occurrence_time: read_i64(bytes, 120)?,
            generation: read_u64(bytes, 128)?,
            market_principal: read_u64(bytes, 136)?,
            ticket_rent: read_u64(bytes, 144)?,
            refund_authority: IdentityV1::decode_at(bytes, 152)?,
            capitalization_id: IdentityV1::decode_at(bytes, 184)?,
            market_identity_id: IdentityV1::decode_at(bytes, 216)?,
            pda_bump: read_byte(bytes, 10)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one canonical one-use ticket representation.
    pub fn to_bytes(self) -> [u8; OCCURRENCE_TICKET_BYTES_V1] {
        let mut output = [0; OCCURRENCE_TICKET_BYTES_V1];
        put_record_header(&mut output, &OCCURRENCE_TICKET_MAGIC_V1);
        put(&mut output, 10, &[self.pda_bump]);
        put(&mut output, 16, &self.series_root_address.to_bytes());
        put(&mut output, 48, &self.recipe_id.to_bytes());
        put(&mut output, 80, &self.derived_occurrence_id.to_bytes());
        put(&mut output, 112, &self.occurrence_index.to_le_bytes());
        put(&mut output, 120, &self.occurrence_time.to_le_bytes());
        put(&mut output, 128, &self.generation.to_le_bytes());
        put(&mut output, 136, &self.market_principal.to_le_bytes());
        put(&mut output, 144, &self.ticket_rent.to_le_bytes());
        put(&mut output, 152, &self.refund_authority.to_bytes());
        put(&mut output, 184, &self.capitalization_id.to_bytes());
        put(&mut output, 216, &self.market_identity_id.to_bytes());
        output
    }
}

/// Canonical action discriminator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesActionV1 {
    /// Create a fully prepaid finite Series.
    CreateSeries = 1,
    /// Permissionlessly release the exact next occurrence.
    InstantiateNext = 2,
    /// Consume one ticket atomically through exact Market Found.
    ConsumeTicket = 3,
    /// Close a fully exhausted root and escrow.
    CloseExhausted = 4,
}

impl TryFrom<u8> for SeriesActionV1 {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::CreateSeries),
            2 => Ok(Self::InstantiateNext),
            3 => Ok(Self::ConsumeTicket),
            4 => Ok(Self::CloseExhausted),
            _ => Err(Error::UnknownAction),
        }
    }
}

impl SeriesActionV1 {
    /// Return the canonical wire discriminator without an unchecked cast.
    pub const fn discriminator(self) -> u8 {
        match self {
            Self::CreateSeries => 1,
            Self::InstantiateNext => 2,
            Self::ConsumeTicket => 3,
            Self::CloseExhausted => 4,
        }
    }
}

/// Canonical create instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateSeriesV1 {
    /// Immutable refund beneficiary authority.
    pub refund_authority: IdentityV1,
    /// Root PDA bump.
    pub root_bump: u8,
    /// Escrow PDA bump.
    pub escrow_bump: u8,
    /// Permanent replay-guard PDA bump.
    pub replay_guard_bump: u8,
}

impl CreateSeriesV1 {
    /// Hostile-decode an exact create instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_instruction_header(bytes, CREATE_SERIES_BYTES_V1, SeriesActionV1::CreateSeries)?;
        require_zero(bytes, 51, 5)?;
        Ok(Self {
            refund_authority: IdentityV1::decode_at(bytes, 16)?,
            root_bump: read_byte(bytes, 48)?,
            escrow_bump: read_byte(bytes, 49)?,
            replay_guard_bump: read_byte(bytes, 50)?,
        })
    }

    /// Encode the exact create instruction.
    pub fn to_bytes(self) -> [u8; CREATE_SERIES_BYTES_V1] {
        let mut output = [0; CREATE_SERIES_BYTES_V1];
        put_instruction_header(&mut output, SeriesActionV1::CreateSeries);
        put(&mut output, 16, &self.refund_authority.to_bytes());
        put(&mut output, 48, &[self.root_bump]);
        put(&mut output, 49, &[self.escrow_bump]);
        put(&mut output, 50, &[self.replay_guard_bump]);
        output
    }
}

/// Canonical permissionless exact-next instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstantiateNextV1 {
    /// Exact next index repeated to reject stale/replayed requests.
    pub expected_index: u64,
    /// Exact next time repeated to reject schedule substitution.
    pub expected_time: i64,
    /// Canonical one-use ticket PDA bump.
    pub ticket_bump: u8,
}

impl InstantiateNextV1 {
    /// Hostile-decode an exact instantiate instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_instruction_header(
            bytes,
            INSTANTIATE_NEXT_BYTES_V1,
            SeriesActionV1::InstantiateNext,
        )?;
        require_zero(bytes, 33, 7)?;
        Ok(Self {
            expected_index: read_u64(bytes, 16)?,
            expected_time: read_i64(bytes, 24)?,
            ticket_bump: read_byte(bytes, 32)?,
        })
    }

    /// Encode the exact instantiate instruction.
    pub fn to_bytes(self) -> [u8; INSTANTIATE_NEXT_BYTES_V1] {
        let mut output = [0; INSTANTIATE_NEXT_BYTES_V1];
        put_instruction_header(&mut output, SeriesActionV1::InstantiateNext);
        put(&mut output, 16, &self.expected_index.to_le_bytes());
        put(&mut output, 24, &self.expected_time.to_le_bytes());
        put(&mut output, 32, &[self.ticket_bump]);
        output
    }
}

/// Semantic ticket-consume instruction; physical Found composition is unmeasured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsumeTicketV1 {
    /// Exact occurrence index repeated against the one-use ticket.
    pub expected_index: u64,
}

impl ConsumeTicketV1 {
    /// Hostile-decode one exact ticket-consume instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_instruction_header(
            bytes,
            CONSUME_TICKET_BYTES_V1,
            SeriesActionV1::ConsumeTicket,
        )?;
        Ok(Self {
            expected_index: read_u64(bytes, 16)?,
        })
    }

    /// Encode the exact ticket-consume instruction.
    pub fn to_bytes(self) -> [u8; CONSUME_TICKET_BYTES_V1] {
        let mut output = [0; CONSUME_TICKET_BYTES_V1];
        put_instruction_header(&mut output, SeriesActionV1::ConsumeTicket);
        put(&mut output, 16, &self.expected_index.to_le_bytes());
        output
    }
}

/// Canonical exhausted-close instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseExhaustedV1 {
    /// Exact final released count repeated against root truth.
    pub expected_released_allocations: u64,
}

impl CloseExhaustedV1 {
    /// Hostile-decode one exact exhausted-close instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_instruction_header(
            bytes,
            CLOSE_EXHAUSTED_BYTES_V1,
            SeriesActionV1::CloseExhausted,
        )?;
        Ok(Self {
            expected_released_allocations: read_u64(bytes, 16)?,
        })
    }

    /// Encode the exact exhausted-close instruction.
    pub fn to_bytes(self) -> [u8; CLOSE_EXHAUSTED_BYTES_V1] {
        let mut output = [0; CLOSE_EXHAUSTED_BYTES_V1];
        put_instruction_header(&mut output, SeriesActionV1::CloseExhausted);
        put(
            &mut output,
            16,
            &self.expected_released_allocations.to_le_bytes(),
        );
        output
    }
}

/// Hostile-decoded union of exact V1 instructions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesInstructionV1 {
    /// Create action.
    CreateSeries(CreateSeriesV1),
    /// Instantiate-next action.
    InstantiateNext(InstantiateNextV1),
    /// Consume-ticket action.
    ConsumeTicket(ConsumeTicketV1),
    /// Exhausted-close action.
    CloseExhausted(CloseExhaustedV1),
}

impl SeriesInstructionV1 {
    /// Hostile-decode by exact action and exact action-specific width.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < INSTRUCTION_HEADER_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if read_array(bytes, 0)? != SERIES_INSTRUCTION_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SERIES_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 11, 5)?;
        match SeriesActionV1::try_from(read_byte(bytes, 10)?)? {
            SeriesActionV1::CreateSeries => CreateSeriesV1::decode(bytes).map(Self::CreateSeries),
            SeriesActionV1::InstantiateNext => {
                InstantiateNextV1::decode(bytes).map(Self::InstantiateNext)
            }
            SeriesActionV1::ConsumeTicket => {
                ConsumeTicketV1::decode(bytes).map(Self::ConsumeTicket)
            }
            SeriesActionV1::CloseExhausted => {
                CloseExhaustedV1::decode(bytes).map(Self::CloseExhausted)
            }
        }
    }
}

/// SDK-free account metadata needed for exact physical-frame validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountMetaV1 {
    /// Account key bytes.
    pub key: [u8; IDENTITY_BYTES],
    /// Whether the account signed the instruction.
    pub is_signer: bool,
    /// Whether the account is writable.
    pub is_writable: bool,
    /// Whether the account is executable.
    pub is_executable: bool,
}

/// Authenticated ownership/data facts for a creatable PDA destination.
///
/// A System-owned, data-empty, nonexecutable address is vacant even when an
/// unsolicited transfer pre-funded it. Treating nonzero lamports as occupied
/// would make deterministic Series PDAs publicly dust-DoSable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VacantAccountFactsV1 {
    /// Complete observed lamport balance.
    pub lamports: u64,
    /// Authenticated owner program key.
    pub owner: [u8; IDENTITY_BYTES],
    /// Authenticated current data length.
    pub data_len: u64,
    /// Authenticated executable flag.
    pub is_executable: bool,
}

impl VacantAccountFactsV1 {
    /// Validate a destination that the adapter may allocate and assign by PDA signature.
    pub fn validate(&self) -> Result<()> {
        if self.owner != SYSTEM_PROGRAM_ID || self.data_len != 0 || self.is_executable {
            return Err(Error::AccountNotVacant);
        }
        Ok(())
    }
}

/// Exact account-role frame for Series creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateSeriesFrameV1 {
    /// Funding payer.
    pub payer: AccountMetaV1,
    /// Immutable recipe record.
    pub recipe: AccountMetaV1,
    /// Immutable capitalization aggregate record.
    pub capitalization_aggregate: AccountMetaV1,
    /// Vacant writable Series root PDA.
    pub series_root: AccountMetaV1,
    /// Vacant writable Series escrow PDA.
    pub series_escrow: AccountMetaV1,
    /// Vacant writable permanent replay-guard PDA.
    pub replay_guard: AccountMetaV1,
    /// Existing permanent RentCredit.
    pub rent_credit: AccountMetaV1,
    /// Canonical System Program.
    pub system_program: AccountMetaV1,
    /// Canonical Rent sysvar.
    pub rent_sysvar: AccountMetaV1,
}

impl CreateSeriesFrameV1 {
    /// Validate exact role order, privileges, canonical programs, and non-aliasing.
    pub fn validate(accounts: &[AccountMetaV1; 9]) -> Result<Self> {
        let [
            payer,
            recipe,
            aggregate,
            root,
            escrow,
            guard,
            rent_credit,
            system,
            rent,
        ] = *accounts;
        require_privilege(payer, true, true, false)?;
        require_privilege(recipe, false, false, false)?;
        require_privilege(aggregate, false, false, false)?;
        require_privilege(root, false, true, false)?;
        require_privilege(escrow, false, true, false)?;
        require_privilege(guard, false, true, false)?;
        require_privilege(rent_credit, false, false, false)?;
        require_system(system)?;
        require_rent(rent)?;
        require_distinct(accounts)?;
        require_nonzero_roles(&[payer, recipe, aggregate, root, escrow, guard, rent_credit])?;
        Ok(Self {
            payer,
            recipe,
            capitalization_aggregate: aggregate,
            series_root: root,
            series_escrow: escrow,
            replay_guard: guard,
            rent_credit,
            system_program: system,
            rent_sysvar: rent,
        })
    }
}

/// Exact account-role frame for permissionless next-occurrence release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstantiateNextFrameV1 {
    /// Permissionless transaction actor; not a Series authority.
    pub actor: AccountMetaV1,
    /// Writable Series root.
    pub series_root: AccountMetaV1,
    /// Immutable recipe record.
    pub recipe: AccountMetaV1,
    /// Immutable derivation record.
    pub derived_occurrence: AccountMetaV1,
    /// Immutable exact capitalization item.
    pub occurrence_capitalization: AccountMetaV1,
    /// Writable Series escrow.
    pub series_escrow: AccountMetaV1,
    /// Vacant writable ticket PDA.
    pub occurrence_ticket: AccountMetaV1,
    /// Canonical System Program.
    pub system_program: AccountMetaV1,
    /// Canonical Rent sysvar.
    pub rent_sysvar: AccountMetaV1,
}

impl InstantiateNextFrameV1 {
    /// Validate exact role order, privileges, canonical programs, and non-aliasing.
    pub fn validate(accounts: &[AccountMetaV1; 9]) -> Result<Self> {
        let [
            actor,
            root,
            recipe,
            derived,
            capitalization,
            escrow,
            ticket,
            system,
            rent,
        ] = *accounts;
        require_privilege(actor, true, false, false)?;
        require_privilege(root, false, true, false)?;
        require_privilege(recipe, false, false, false)?;
        require_privilege(derived, false, false, false)?;
        require_privilege(capitalization, false, false, false)?;
        require_privilege(escrow, false, true, false)?;
        require_privilege(ticket, false, true, false)?;
        require_system(system)?;
        require_rent(rent)?;
        require_distinct(accounts)?;
        require_nonzero_roles(&[actor, root, recipe, derived, capitalization, escrow, ticket])?;
        Ok(Self {
            actor,
            series_root: root,
            recipe,
            derived_occurrence: derived,
            occurrence_capitalization: capitalization,
            series_escrow: escrow,
            occurrence_ticket: ticket,
            system_program: system,
            rent_sysvar: rent,
        })
    }
}

/// Exact account-role frame for exhausted Series close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseExhaustedFrameV1 {
    /// Permissionless transaction actor.
    pub actor: AccountMetaV1,
    /// Writable exhausted root.
    pub series_root: AccountMetaV1,
    /// Writable empty-principal escrow.
    pub series_escrow: AccountMetaV1,
    /// Writable permanent replay guard, retained at its rent floor.
    pub replay_guard: AccountMetaV1,
    /// Writable permanent RentCredit.
    pub rent_credit: AccountMetaV1,
    /// Canonical Rent sysvar.
    pub rent_sysvar: AccountMetaV1,
}

impl CloseExhaustedFrameV1 {
    /// Validate exact role order, privileges, canonical Rent, and non-aliasing.
    pub fn validate(accounts: &[AccountMetaV1; 6]) -> Result<Self> {
        let [actor, root, escrow, guard, rent_credit, rent] = *accounts;
        require_privilege(actor, true, false, false)?;
        require_privilege(root, false, true, false)?;
        require_privilege(escrow, false, true, false)?;
        require_privilege(guard, false, true, false)?;
        require_privilege(rent_credit, false, true, false)?;
        require_rent(rent)?;
        require_distinct(accounts)?;
        require_nonzero_roles(&[actor, root, escrow, guard, rent_credit])?;
        Ok(Self {
            actor,
            series_root: root,
            series_escrow: escrow,
            replay_guard: guard,
            rent_credit,
            rent_sysvar: rent,
        })
    }
}

/// Exact balance and state plan for initial present capitalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateSeriesPlanV1 {
    /// Newly initialized root state.
    pub root: SeriesRootV1,
    /// Newly initialized escrow binding.
    pub escrow: SeriesEscrowV1,
    /// Newly initialized permanent replay guard.
    pub replay_guard: SeriesReplayGuardV1,
    /// Payer balance before creation.
    pub payer_before: u64,
    /// Payer balance after exact creation funding.
    pub payer_after: u64,
    /// Root balance before allocation/assignment.
    pub root_before: u64,
    /// Root balance after exact rent funding.
    pub root_after: u64,
    /// Escrow balance before allocation/assignment.
    pub escrow_before: u64,
    /// Escrow balance after rent plus all finite principal.
    pub escrow_after: u64,
    /// Replay-guard balance before allocation/assignment.
    pub replay_guard_before: u64,
    /// Permanent replay-guard balance after exact rent funding.
    pub replay_guard_after: u64,
}

/// Plan exact creation from vacant accounts and authenticated immutable content.
#[allow(clippy::too_many_arguments)]
pub fn plan_create_series_v1(
    root_address: IdentityV1,
    recipe_id: IdentityV1,
    aggregate_id: IdentityV1,
    recipe: &SeriesRecipeV1,
    aggregate: &CapitalizationAggregateV1,
    instruction: CreateSeriesV1,
    payer_before: u64,
    root_before: VacantAccountFactsV1,
    escrow_before: VacantAccountFactsV1,
    replay_guard_before: VacantAccountFactsV1,
    root_rent: u64,
    escrow_rent: u64,
    replay_guard_rent: u64,
) -> Result<CreateSeriesPlanV1> {
    root_before.validate()?;
    escrow_before.validate()?;
    replay_guard_before.validate()?;
    if root_rent == 0 || escrow_rent == 0 || replay_guard_rent == 0 {
        return Err(Error::Underfunded);
    }
    let root = SeriesRootV1::new(
        recipe_id,
        aggregate_id,
        recipe,
        aggregate,
        instruction.refund_authority,
        instruction.root_bump,
    )?;
    let escrow = SeriesEscrowV1 {
        series_root_address: root_address,
        recipe_id,
        aggregate_id,
        refund_authority: instruction.refund_authority,
        pda_bump: instruction.escrow_bump,
    };
    let replay_guard = SeriesReplayGuardV1 {
        series_root_address: root_address,
        pda_bump: instruction.replay_guard_bump,
    };
    let escrow_target = escrow_rent
        .checked_add(aggregate.total_principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let root_top_up = required_top_up(root_before.lamports, root_rent)?;
    let escrow_top_up = required_top_up(escrow_before.lamports, escrow_target)?;
    let replay_guard_top_up = required_top_up(replay_guard_before.lamports, replay_guard_rent)?;
    let debit = root_top_up
        .checked_add(escrow_top_up)
        .and_then(|subtotal| subtotal.checked_add(replay_guard_top_up))
        .ok_or(Error::ArithmeticOverflow)?;
    let payer_after = payer_before.checked_sub(debit).ok_or(Error::Underfunded)?;
    let root_after = root_before
        .lamports
        .checked_add(root_top_up)
        .ok_or(Error::ArithmeticOverflow)?;
    let escrow_after = escrow_before
        .lamports
        .checked_add(escrow_top_up)
        .ok_or(Error::ArithmeticOverflow)?;
    let replay_guard_after = replay_guard_before
        .lamports
        .checked_add(replay_guard_top_up)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(CreateSeriesPlanV1 {
        root,
        escrow,
        replay_guard,
        payer_before,
        payer_after,
        root_before: root_before.lamports,
        root_after,
        escrow_before: escrow_before.lamports,
        escrow_after,
        replay_guard_before: replay_guard_before.lamports,
        replay_guard_after,
    })
}

/// Exact atomic state/balance plan for one permissionless next release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstantiationPlanV1 {
    /// Root before the transition.
    pub root_before: SeriesRootV1,
    /// Root after the transition.
    pub root_after: SeriesRootV1,
    /// Complete observed escrow balance before the transition.
    pub escrow_lamports_before: u64,
    /// Complete escrow balance after the exact ticket transfer.
    pub escrow_lamports_after: u64,
    /// Complete prefunded ticket balance before allocation and assignment.
    pub ticket_lamports_before: u64,
    /// Exact escrow top-up transferred after accounting for harmless dust.
    pub ticket_top_up: u64,
    /// Authenticated immutable derivation-record identity.
    pub derived_occurrence_id: IdentityV1,
    /// Exact occurrence/Product/source/manifest/Market obligations.
    pub derived_occurrence: DerivedOccurrenceV1,
    /// Exact newly created ticket.
    pub ticket: OccurrenceTicketV1,
    /// Exact lamports transferred from escrow into the ticket.
    pub ticket_lamports_after: u64,
}

/// Plan an all-or-nothing exact-next release into a one-use ticket.
#[allow(clippy::too_many_arguments)]
pub fn plan_instantiate_next_v1(
    root: SeriesRootV1,
    root_address: IdentityV1,
    escrow: SeriesEscrowV1,
    recipe_id: IdentityV1,
    recipe: &SeriesRecipeV1,
    aggregate_id: IdentityV1,
    aggregate: &CapitalizationAggregateV1,
    derived_occurrence_id: IdentityV1,
    derived: &DerivedOccurrenceV1,
    capitalization_id: IdentityV1,
    capitalization: &OccurrenceCapitalizationV1,
    instruction: InstantiateNextV1,
    authenticated_clock_time: i64,
    authenticated_escrow_rent_minimum: u64,
    authenticated_ticket_rent_minimum: u64,
    observed_escrow_lamports: u64,
    ticket_before: VacantAccountFactsV1,
) -> Result<InstantiationPlanV1> {
    root.validate_for(recipe_id, aggregate_id, recipe, aggregate)?;
    validate_escrow(&escrow, root_address, &root)?;
    if root.phase != SeriesPhaseV1::Active {
        return Err(Error::InvalidPhase);
    }
    if instruction.expected_index != root.next_occurrence_index {
        return Err(Error::IndexMismatch);
    }
    if instruction.expected_time != root.next_occurrence_time {
        return Err(Error::TimeMismatch);
    }
    if authenticated_clock_time < root.next_occurrence_time {
        return Err(Error::OccurrenceNotDue);
    }
    ticket_before.validate()?;
    let required_escrow_balance = authenticated_escrow_rent_minimum
        .checked_add(root.remaining_principal)
        .ok_or(Error::ArithmeticOverflow)?;
    if observed_escrow_lamports < required_escrow_balance {
        return Err(Error::PresentPrincipalMismatch);
    }
    derived.validate_for(recipe_id, recipe, root.next_occurrence_index)?;
    capitalization.validate()?;
    if capitalization.ticket_rent < authenticated_ticket_rent_minimum {
        return Err(Error::Underfunded);
    }
    let expected_derived = derive_occurrence_v1(
        recipe_id,
        recipe,
        root.next_occurrence_index,
        capitalization,
    )?;
    let expected_derived_id = content_identity(&expected_derived.to_bytes())?;
    if *derived != expected_derived || derived_occurrence_id != expected_derived_id {
        return Err(Error::DerivationMismatch);
    }
    if capitalization_id != expected_derived.capitalization_id
        || capitalization_id != derived.capitalization_id
        || root.next_capitalization_id != Some(capitalization_id)
        || capitalization.recipe_id != recipe_id
        || capitalization.capitalization_schedule_id != recipe.capitalization_schedule_id
        || capitalization.occurrence_index != root.next_occurrence_index
    {
        return Err(Error::CapitalizationMismatch);
    }

    let remaining_allocations = root
        .remaining_allocations
        .checked_sub(1)
        .ok_or(Error::RootInvariant)?;
    if remaining_allocations == 0 {
        if capitalization.next_capitalization_id.is_some()
            || capitalization.total_principal != root.remaining_principal
        {
            return Err(Error::CapitalizationMismatch);
        }
    } else if capitalization.next_capitalization_id.is_none() {
        return Err(Error::CapitalizationMismatch);
    }
    let released_allocations = root
        .released_allocations
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let remaining_principal = root
        .remaining_principal
        .checked_sub(capitalization.total_principal)
        .ok_or(Error::Underfunded)?;
    let ticket_top_up = required_top_up(ticket_before.lamports, capitalization.total_principal)?;
    let escrow_lamports_after = observed_escrow_lamports
        .checked_sub(ticket_top_up)
        .ok_or(Error::Underfunded)?;
    let released_principal = root
        .released_principal
        .checked_add(capitalization.total_principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let outstanding_tickets = root
        .outstanding_tickets
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let (phase, next_occurrence_time) = if remaining_allocations == 0 {
        (SeriesPhaseV1::Exhausted, root.next_occurrence_time)
    } else {
        (SeriesPhaseV1::Active, recipe.time_at(released_allocations)?)
    };
    let root_after = SeriesRootV1 {
        next_occurrence_index: released_allocations,
        next_occurrence_time,
        remaining_allocations,
        released_allocations,
        remaining_principal,
        released_principal,
        outstanding_tickets,
        next_capitalization_id: capitalization.next_capitalization_id,
        phase,
        ..root
    };
    root_after.validate_for(recipe_id, aggregate_id, recipe, aggregate)?;
    let ticket = OccurrenceTicketV1 {
        series_root_address: root_address,
        recipe_id,
        derived_occurrence_id,
        occurrence_index: derived.occurrence_index,
        occurrence_time: derived.occurrence_time,
        generation: derived.generation,
        market_principal: capitalization.market_principal,
        ticket_rent: capitalization.ticket_rent,
        refund_authority: root.refund_authority,
        capitalization_id,
        market_identity_id: derived.market_identity_id,
        pda_bump: instruction.ticket_bump,
    };
    ticket.validate()?;
    Ok(InstantiationPlanV1 {
        root_before: root,
        root_after,
        escrow_lamports_before: observed_escrow_lamports,
        escrow_lamports_after,
        ticket_lamports_before: ticket_before.lamports,
        ticket_top_up,
        derived_occurrence_id,
        derived_occurrence: *derived,
        ticket,
        ticket_lamports_after: ticket_before
            .lamports
            .checked_add(ticket_top_up)
            .ok_or(Error::ArithmeticOverflow)?,
    })
}

/// Exact authenticated identity and funding bundle the Found owner must accept.
///
/// This is output from ticket validation, never caller wire authority. The SBF
/// adapter must obtain an accepted Found transition for this entire bundle and
/// commit it atomically with the surrounding ticket/root/RentCredit plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundCompositionObligationsV1 {
    /// Realm identity selecting collateral.
    pub realm_id: IdentityV1,
    /// Terms identity.
    pub terms_id: IdentityV1,
    /// Exact categorical basis identity.
    pub claim_basis_id: IdentityV1,
    /// Capacity-profile identity.
    pub capacity_profile_id: IdentityV1,
    /// Compiler release that owns Product construction.
    pub compiler_release_id: IdentityV1,
    /// Occurrence artifact identity.
    pub occurrence_artifact_id: IdentityV1,
    /// Occurrence identity.
    pub occurrence_id: IdentityV1,
    /// Product instance identity.
    pub product_instance_id: IdentityV1,
    /// Source specification identity.
    pub source_spec_id: IdentityV1,
    /// Source window identity.
    pub source_window_id: IdentityV1,
    /// Statistic identity.
    pub statistic_id: IdentityV1,
    /// Resolution-policy identity.
    pub resolution_policy_id: IdentityV1,
    /// Capability-manifest identity.
    pub capability_manifest_id: IdentityV1,
    /// Exact Market identity to Found.
    pub market_identity_id: IdentityV1,
    /// Exact occurrence index.
    pub occurrence_index: u64,
    /// Exact scheduled time.
    pub occurrence_time: i64,
    /// Exact Market generation.
    pub generation: u64,
    /// Exact ticket principal admitted only to Found obligations.
    pub market_principal: u64,
    /// Immutable Market/Fund/custody rent-refund beneficiary.
    pub refund_authority: IdentityV1,
}

/// Pure obligations that a measured Found adapter must satisfy atomically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TicketConsumptionPlanV1 {
    /// Root before ticket consumption.
    pub root_before: SeriesRootV1,
    /// Root after decrementing the exact outstanding count.
    pub root_after: SeriesRootV1,
    /// Exact Market identity Found must create.
    pub market_identity_id: IdentityV1,
    /// Exact principal routed only into authenticated Found obligations.
    pub market_principal: u64,
    /// Complete exact identity/funding input required by Found.
    pub found_obligations: FoundCompositionObligationsV1,
    /// Ticket lamports before complete deletion.
    pub ticket_lamports_before: u64,
    /// Ticket lamports after complete deletion.
    pub ticket_lamports_after: u64,
    /// RentCredit balance before credit.
    pub rent_credit_before: u64,
    /// RentCredit balance after crediting rent and unsolicited donations.
    pub rent_credit_after: u64,
}

/// Validate and plan one-use ticket consumption through an exact Found operation.
#[allow(clippy::too_many_arguments)]
pub fn plan_consume_ticket_v1(
    root: SeriesRootV1,
    root_address: IdentityV1,
    recipe_id: IdentityV1,
    recipe: &SeriesRecipeV1,
    aggregate_id: IdentityV1,
    aggregate: &CapitalizationAggregateV1,
    derived_occurrence_id: IdentityV1,
    derived: &DerivedOccurrenceV1,
    capitalization_id: IdentityV1,
    capitalization: &OccurrenceCapitalizationV1,
    ticket: OccurrenceTicketV1,
    instruction: ConsumeTicketV1,
    observed_ticket_lamports: u64,
    rent_credit_before: u64,
) -> Result<TicketConsumptionPlanV1> {
    root.validate_for(recipe_id, aggregate_id, recipe, aggregate)?;
    derived.validate_for(recipe_id, recipe, instruction.expected_index)?;
    capitalization.validate()?;
    let expected_derived = derive_occurrence_v1(
        recipe_id,
        recipe,
        instruction.expected_index,
        capitalization,
    )?;
    if *derived != expected_derived
        || derived_occurrence_id != content_identity(&expected_derived.to_bytes())?
    {
        return Err(Error::DerivationMismatch);
    }
    if capitalization_id != expected_derived.capitalization_id {
        return Err(Error::CapitalizationMismatch);
    }
    if ticket.series_root_address != root_address
        || ticket.recipe_id != recipe_id
        || ticket.derived_occurrence_id != derived_occurrence_id
        || ticket.occurrence_index != instruction.expected_index
        || ticket.occurrence_time != derived.occurrence_time
        || ticket.generation != derived.generation
        || ticket.market_principal != capitalization.market_principal
        || ticket.ticket_rent != capitalization.ticket_rent
        || ticket.refund_authority != root.refund_authority
        || ticket.capitalization_id != capitalization_id
        || ticket.market_identity_id != derived.market_identity_id
    {
        return Err(Error::TicketMismatch);
    }
    if capitalization_id != derived.capitalization_id
        || capitalization.recipe_id != recipe_id
        || capitalization.capitalization_schedule_id != recipe.capitalization_schedule_id
        || capitalization.occurrence_index != instruction.expected_index
    {
        return Err(Error::CapitalizationMismatch);
    }
    if root.outstanding_tickets == 0 {
        return Err(Error::OutstandingTickets);
    }
    if ticket.occurrence_index >= root.released_allocations {
        return Err(Error::TicketMismatch);
    }
    let minimum = ticket
        .market_principal
        .checked_add(ticket.ticket_rent)
        .ok_or(Error::ArithmeticOverflow)?;
    if observed_ticket_lamports < minimum {
        return Err(Error::TicketBalanceMismatch);
    }
    let credit = observed_ticket_lamports
        .checked_sub(ticket.market_principal)
        .ok_or(Error::TicketBalanceMismatch)?;
    let rent_credit_after = rent_credit_before
        .checked_add(credit)
        .ok_or(Error::ArithmeticOverflow)?;
    let root_after = SeriesRootV1 {
        outstanding_tickets: root
            .outstanding_tickets
            .checked_sub(1)
            .ok_or(Error::OutstandingTickets)?,
        ..root
    };
    root_after.validate_for(recipe_id, aggregate_id, recipe, aggregate)?;
    let found_obligations = FoundCompositionObligationsV1 {
        realm_id: recipe.realm_id,
        terms_id: recipe.terms_id,
        claim_basis_id: recipe.claim_basis_id,
        capacity_profile_id: recipe.capacity_profile_id,
        compiler_release_id: recipe.compiler_release_id,
        occurrence_artifact_id: derived.occurrence_artifact_id,
        occurrence_id: derived.occurrence_id,
        product_instance_id: derived.product_instance_id,
        source_spec_id: derived.source_spec_id,
        source_window_id: derived.source_window_id,
        statistic_id: derived.statistic_id,
        resolution_policy_id: derived.resolution_policy_id,
        capability_manifest_id: derived.capability_manifest_id,
        market_identity_id: derived.market_identity_id,
        occurrence_index: derived.occurrence_index,
        occurrence_time: derived.occurrence_time,
        generation: derived.generation,
        market_principal: ticket.market_principal,
        refund_authority: ticket.refund_authority,
    };
    Ok(TicketConsumptionPlanV1 {
        root_before: root,
        root_after,
        market_identity_id: ticket.market_identity_id,
        market_principal: ticket.market_principal,
        found_obligations,
        ticket_lamports_before: observed_ticket_lamports,
        ticket_lamports_after: 0,
        rent_credit_before,
        rent_credit_after,
    })
}

/// Exact exhausted-close balance plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseExhaustedPlanV1 {
    /// Root balance before complete close.
    pub root_lamports_before: u64,
    /// Escrow balance before complete close.
    pub escrow_lamports_before: u64,
    /// Replay-guard balance before retaining its current rent floor.
    pub replay_guard_lamports_before: u64,
    /// Permanent RentCredit balance before close.
    pub rent_credit_before: u64,
    /// Permanent RentCredit balance after root, escrow, and guard-surplus credit.
    pub rent_credit_after: u64,
    /// Root balance after close.
    pub root_lamports_after: u64,
    /// Escrow balance after close.
    pub escrow_lamports_after: u64,
    /// Replay-guard balance retained permanently after close.
    pub replay_guard_lamports_after: u64,
}

/// Plan permissionless close only after exhaustion and ticket consumption.
#[allow(clippy::too_many_arguments)]
pub fn plan_close_exhausted_v1(
    root: SeriesRootV1,
    root_address: IdentityV1,
    escrow: SeriesEscrowV1,
    replay_guard: SeriesReplayGuardV1,
    instruction: CloseExhaustedV1,
    root_lamports_before: u64,
    escrow_lamports_before: u64,
    replay_guard_lamports_before: u64,
    authenticated_replay_guard_rent_minimum: u64,
    rent_credit_before: u64,
) -> Result<CloseExhaustedPlanV1> {
    root.validate_internal()?;
    validate_escrow(&escrow, root_address, &root)?;
    validate_replay_guard(&replay_guard, root_address)?;
    if root.phase != SeriesPhaseV1::Exhausted
        || instruction.expected_released_allocations != root.released_allocations
    {
        return Err(Error::SeriesNotExhausted);
    }
    if root.outstanding_tickets != 0 {
        return Err(Error::OutstandingTickets);
    }
    if root.remaining_principal != 0 {
        return Err(Error::ClosePrincipalRemaining);
    }
    if authenticated_replay_guard_rent_minimum == 0
        || replay_guard_lamports_before < authenticated_replay_guard_rent_minimum
    {
        return Err(Error::ReplayGuardUnderfunded);
    }
    let replay_guard_surplus = replay_guard_lamports_before
        .checked_sub(authenticated_replay_guard_rent_minimum)
        .ok_or(Error::ReplayGuardUnderfunded)?;
    let credit = root_lamports_before
        .checked_add(escrow_lamports_before)
        .and_then(|subtotal| subtotal.checked_add(replay_guard_surplus))
        .ok_or(Error::ArithmeticOverflow)?;
    let rent_credit_after = rent_credit_before
        .checked_add(credit)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(CloseExhaustedPlanV1 {
        root_lamports_before,
        escrow_lamports_before,
        replay_guard_lamports_before,
        rent_credit_before,
        rent_credit_after,
        root_lamports_after: 0,
        escrow_lamports_after: 0,
        replay_guard_lamports_after: authenticated_replay_guard_rent_minimum,
    })
}

fn validate_escrow(
    escrow: &SeriesEscrowV1,
    root_address: IdentityV1,
    root: &SeriesRootV1,
) -> Result<()> {
    if escrow.series_root_address != root_address
        || escrow.recipe_id != root.recipe_id
        || escrow.aggregate_id != root.aggregate_id
        || escrow.refund_authority != root.refund_authority
    {
        return Err(Error::AggregateMismatch);
    }
    Ok(())
}

fn validate_replay_guard(guard: &SeriesReplayGuardV1, root_address: IdentityV1) -> Result<()> {
    if guard.series_root_address != root_address {
        return Err(Error::ReplayGuardMismatch);
    }
    Ok(())
}

fn required_top_up(observed: u64, target: u64) -> Result<u64> {
    if observed >= target {
        return Ok(0);
    }
    target
        .checked_sub(observed)
        .ok_or(Error::ArithmeticOverflow)
}

fn check_record_header(bytes: &[u8], width: usize, magic: [u8; 8]) -> Result<()> {
    check_exact_magic_schema(bytes, width, magic)?;
    require_zero(bytes, 12, 4)
}

fn check_exact_magic_schema(bytes: &[u8], width: usize, magic: [u8; 8]) -> Result<()> {
    if bytes.len() != width {
        return Err(Error::InvalidLength);
    }
    if read_array(bytes, 0)? != magic {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, 8)? != SERIES_SCHEMA_VERSION_V1 {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}

fn check_instruction_header(bytes: &[u8], width: usize, action: SeriesActionV1) -> Result<()> {
    check_exact_magic_schema(bytes, width, SERIES_INSTRUCTION_MAGIC_V1)?;
    if SeriesActionV1::try_from(read_byte(bytes, 10)?)? != action {
        return Err(Error::UnknownAction);
    }
    require_zero(bytes, 11, 5)
}

fn put_record_header<const N: usize>(output: &mut [u8; N], magic: &[u8; 8]) {
    put(output, 0, magic);
    put(output, 8, &SERIES_SCHEMA_VERSION_V1.to_le_bytes());
}

fn put_instruction_header<const N: usize>(output: &mut [u8; N], action: SeriesActionV1) {
    put(output, 0, &SERIES_INSTRUCTION_MAGIC_V1);
    put(output, 8, &SERIES_SCHEMA_VERSION_V1.to_le_bytes());
    put(output, 10, &[action.discriminator()]);
}

fn require_privilege(
    account: AccountMetaV1,
    signer: bool,
    writable: bool,
    executable: bool,
) -> Result<()> {
    if account.is_signer != signer
        || account.is_writable != writable
        || account.is_executable != executable
    {
        return Err(Error::InvalidAccountPrivilege);
    }
    Ok(())
}

fn require_system(account: AccountMetaV1) -> Result<()> {
    if account.key != SYSTEM_PROGRAM_ID
        || account.is_signer
        || account.is_writable
        || !account.is_executable
    {
        return Err(Error::InvalidSystemProgram);
    }
    Ok(())
}

fn require_rent(account: AccountMetaV1) -> Result<()> {
    if account.key != RENT_SYSVAR_ID
        || account.is_signer
        || account.is_writable
        || account.is_executable
    {
        return Err(Error::InvalidRentSysvar);
    }
    Ok(())
}

fn require_nonzero_roles(accounts: &[AccountMetaV1]) -> Result<()> {
    for account in accounts {
        if is_zero(&account.key) {
            return Err(Error::ZeroIdentity);
        }
    }
    Ok(())
}

fn require_distinct<const N: usize>(accounts: &[AccountMetaV1; N]) -> Result<()> {
    for left in 0..N {
        for right in left.checked_add(1).ok_or(Error::ArithmeticOverflow)?..N {
            let left_key = accounts.get(left).ok_or(Error::InvalidLength)?.key;
            let right_key = accounts.get(right).ok_or(Error::InvalidLength)?.key;
            if left_key == right_key {
                return Err(Error::AccountAlias);
            }
        }
    }
    Ok(())
}

fn is_zero(bytes: &[u8; IDENTITY_BYTES]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    let reserved = bytes.get(offset..end).ok_or(Error::InvalidLength)?;
    if reserved.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_optional_identity(bytes: &[u8], offset: usize) -> Result<Option<IdentityV1>> {
    let value = read_array(bytes, offset)?;
    if is_zero(&value) {
        Ok(None)
    } else {
        IdentityV1::new(value).map(Some)
    }
}

fn put_optional_identity(output: &mut [u8], offset: usize, value: Option<IdentityV1>) {
    if let Some(identity) = value {
        put(output, offset, &identity.to_bytes());
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    let Some(end) = offset.checked_add(value.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(value);
}

#[cfg(test)]
mod tests;
