// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    EpochChildKindV1, RetirementErrorV1, CHILD_GENERATION_V1_BYTES, EPOCH_CHILD_COUNTS_V1_BYTES,
    EPOCH_RETIREMENT_TAIL_V1_BYTES, GENERAL_EPOCH_TOMBSTONE_TAG, GENERAL_EPOCH_TOMBSTONE_V1_BYTES,
    GENERAL_EPOCH_TOMBSTONE_VERSION_V1, IDENTITY_BYTES, MARKET_EPOCH_CURSOR_V1_BYTES,
    POSITION_RETIREMENT_TAIL_V1_BYTES, POSITION_TOMBSTONE_TAG, POSITION_TOMBSTONE_V1_BYTES,
    POSITION_TOMBSTONE_VERSION_V1, RENT_SPLIT_V2_BYTES, RESERVATION_COUNT_TAIL_V1_BYTES,
};

fn exact(input: &[u8], expected: usize) -> Result<(), RetirementErrorV1> {
    if input.len() < expected {
        Err(RetirementErrorV1::Truncated)
    } else if input.len() > expected {
        Err(RetirementErrorV1::TrailingBytes)
    } else {
        Ok(())
    }
}

fn read_u32(input: &[u8], at: usize) -> u32 {
    let mut word = [0u8; 4];
    word.copy_from_slice(&input[at..at + 4]);
    u32::from_le_bytes(word)
}

fn read_u64(input: &[u8], at: usize) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&input[at..at + 8]);
    u64::from_le_bytes(word)
}

fn read_identity(input: &[u8], at: usize) -> Result<Identity32V1, RetirementErrorV1> {
    let mut bytes = [0u8; IDENTITY_BYTES];
    bytes.copy_from_slice(&input[at..at + IDENTITY_BYTES]);
    Identity32V1::new(bytes)
}

/// Validated full-width persisted identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Identity32V1([u8; IDENTITY_BYTES]);

impl Identity32V1 {
    /// Construct a nonzero identity.
    pub const fn new(bytes: [u8; IDENTITY_BYTES]) -> Result<Self, RetirementErrorV1> {
        let mut index = 0usize;
        while index < bytes.len() {
            if bytes[index] != 0 {
                return Ok(Self(bytes));
            }
            index += 1;
        }
        Err(RetirementErrorV1::ZeroIdentity)
    }

    /// Return the exact persisted bytes.
    pub const fn bytes(self) -> [u8; IDENTITY_BYTES] {
        self.0
    }
}

/// Exact funding split embedded in a live closeable account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentSplitV2 {
    /// Exact payer and sole recipient of refundable live principal.
    pub payer: Identity32V1,
    /// Live-account rent delta returned at close.
    pub refundable_live_principal: u64,
    /// Independently prepaid principal retained in the permanent tombstone.
    pub permanent_tombstone_principal: u64,
    /// Monotone lower bound of unsolicited lamports routed to the neutral sink.
    pub donation_floor: u64,
}

impl RentSplitV2 {
    /// Validate that both payer-owned and permanent principal compartments exist.
    pub const fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.refundable_live_principal == 0 || self.permanent_tombstone_principal == 0 {
            return Err(RetirementErrorV1::NonCanonicalState);
        }
        let principal = match self
            .refundable_live_principal
            .checked_add(self.permanent_tombstone_principal)
        {
            Some(value) => value,
            None => return Err(RetirementErrorV1::ArithmeticOverflow),
        };
        match principal.checked_add(self.donation_floor) {
            Some(_) => Ok(()),
            None => Err(RetirementErrorV1::ArithmeticOverflow),
        }
    }

    /// Encode the exact 56-byte retirement funding tail.
    pub fn encode(self) -> Result<[u8; RENT_SPLIT_V2_BYTES], RetirementErrorV1> {
        self.validate()?;
        let mut out = [0u8; RENT_SPLIT_V2_BYTES];
        out[..32].copy_from_slice(&self.payer.bytes());
        out[32..40].copy_from_slice(&self.refundable_live_principal.to_le_bytes());
        out[40..48].copy_from_slice(&self.permanent_tombstone_principal.to_le_bytes());
        out[48..56].copy_from_slice(&self.donation_floor.to_le_bytes());
        Ok(out)
    }

    /// Decode exactly 56 bytes, refusing truncation, trailing bytes, zero payer,
    /// and a missing principal compartment.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, RENT_SPLIT_V2_BYTES)?;
        let value = Self {
            payer: read_identity(input, 0)?,
            refundable_live_principal: read_u64(input, 32),
            permanent_tombstone_principal: read_u64(input, 40),
            donation_floor: read_u64(input, 48),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Exact appended Position V2 accounting tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionRetirementTailV1 {
    /// Authoritative number of reservations still owning economic assets.
    pub outstanding_reservations: u32,
    /// Funding compartments for shrink-to-tombstone.
    pub rent: RentSplitV2,
}

impl PositionRetirementTailV1 {
    /// Validate the embedded rent split.
    pub const fn validate(self) -> Result<(), RetirementErrorV1> {
        self.rent.validate()
    }

    /// Encode the exact 60-byte Position extension.
    pub fn encode(self) -> Result<[u8; POSITION_RETIREMENT_TAIL_V1_BYTES], RetirementErrorV1> {
        self.validate()?;
        let mut out = [0u8; POSITION_RETIREMENT_TAIL_V1_BYTES];
        out[..4].copy_from_slice(&self.outstanding_reservations.to_le_bytes());
        out[4..].copy_from_slice(&self.rent.encode()?);
        Ok(out)
    }

    /// Decode exactly 60 bytes and validate the funding split.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, POSITION_RETIREMENT_TAIL_V1_BYTES)?;
        let value = Self {
            outstanding_reservations: read_u32(input, 0),
            rent: RentSplitV2::decode(&input[4..])?,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Nine exhaustive, typed child counts owned by one Epoch generation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EpochChildCountsV1 {
    /// Candidate record/feed/funding bundles.
    pub candidate_bundles: u32,
    /// CandidateIndex pages.
    pub candidate_index_pages: u32,
    /// Immutable candidate verdicts.
    pub candidate_verdicts: u32,
    /// Candidate escrows.
    pub candidate_escrows: u32,
    /// Growing or complete ClearWork bundles.
    pub clear_work_bundles: u32,
    /// Order pages.
    pub order_pages: u32,
    /// Reservation archives, independent of Position economic counts.
    pub reservation_archives: u32,
    /// Settlement receipts.
    pub settlement_receipts: u32,
    /// Final pot count, canonically zero or one.
    pub final_pots: u32,
}

impl EpochChildCountsV1 {
    /// Validate the singleton pot geometry.
    pub const fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.final_pots > 1 {
            Err(RetirementErrorV1::NonCanonicalState)
        } else {
            Ok(())
        }
    }

    /// Whether every independently addressed child class is empty.
    pub const fn is_zero(self) -> bool {
        self.candidate_bundles == 0
            && self.candidate_index_pages == 0
            && self.candidate_verdicts == 0
            && self.candidate_escrows == 0
            && self.clear_work_bundles == 0
            && self.order_pages == 0
            && self.reservation_archives == 0
            && self.settlement_receipts == 0
            && self.final_pots == 0
    }

    /// Return one typed count.
    pub const fn get(self, kind: EpochChildKindV1) -> u32 {
        match kind {
            EpochChildKindV1::CandidateBundle => self.candidate_bundles,
            EpochChildKindV1::CandidateIndexPage => self.candidate_index_pages,
            EpochChildKindV1::CandidateVerdict => self.candidate_verdicts,
            EpochChildKindV1::CandidateEscrow => self.candidate_escrows,
            EpochChildKindV1::ClearWorkBundle => self.clear_work_bundles,
            EpochChildKindV1::OrderPage => self.order_pages,
            EpochChildKindV1::ReservationArchive => self.reservation_archives,
            EpochChildKindV1::SettlementReceipt => self.settlement_receipts,
            EpochChildKindV1::FinalPot => self.final_pots,
        }
    }

    pub(crate) fn checked_increment(
        self,
        kind: EpochChildKindV1,
    ) -> Result<Self, RetirementErrorV1> {
        let value = self
            .get(kind)
            .checked_add(1)
            .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
        if kind == EpochChildKindV1::FinalPot && value > 1 {
            return Err(RetirementErrorV1::NonCanonicalState);
        }
        Ok(self.with(kind, value))
    }

    pub(crate) fn checked_decrement(
        self,
        kind: EpochChildKindV1,
    ) -> Result<Self, RetirementErrorV1> {
        let value = self
            .get(kind)
            .checked_sub(1)
            .ok_or(RetirementErrorV1::CounterUnderflow)?;
        Ok(self.with(kind, value))
    }

    const fn with(mut self, kind: EpochChildKindV1, value: u32) -> Self {
        match kind {
            EpochChildKindV1::CandidateBundle => self.candidate_bundles = value,
            EpochChildKindV1::CandidateIndexPage => self.candidate_index_pages = value,
            EpochChildKindV1::CandidateVerdict => self.candidate_verdicts = value,
            EpochChildKindV1::CandidateEscrow => self.candidate_escrows = value,
            EpochChildKindV1::ClearWorkBundle => self.clear_work_bundles = value,
            EpochChildKindV1::OrderPage => self.order_pages = value,
            EpochChildKindV1::ReservationArchive => self.reservation_archives = value,
            EpochChildKindV1::SettlementReceipt => self.settlement_receipts = value,
            EpochChildKindV1::FinalPot => self.final_pots = value,
        }
        self
    }

    /// Encode all nine counters in their frozen order as little-endian words.
    pub fn encode(self) -> Result<[u8; EPOCH_CHILD_COUNTS_V1_BYTES], RetirementErrorV1> {
        self.validate()?;
        let words = [
            self.candidate_bundles,
            self.candidate_index_pages,
            self.candidate_verdicts,
            self.candidate_escrows,
            self.clear_work_bundles,
            self.order_pages,
            self.reservation_archives,
            self.settlement_receipts,
            self.final_pots,
        ];
        let mut out = [0u8; EPOCH_CHILD_COUNTS_V1_BYTES];
        let mut index = 0usize;
        while index < words.len() {
            let at = index * 4;
            out[at..at + 4].copy_from_slice(&words[index].to_le_bytes());
            index += 1;
        }
        Ok(out)
    }

    /// Decode exactly nine little-endian counters and validate singleton roles.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, EPOCH_CHILD_COUNTS_V1_BYTES)?;
        let value = Self {
            candidate_bundles: read_u32(input, 0),
            candidate_index_pages: read_u32(input, 4),
            candidate_verdicts: read_u32(input, 8),
            candidate_escrows: read_u32(input, 12),
            clear_work_bundles: read_u32(input, 16),
            order_pages: read_u32(input, 20),
            reservation_archives: read_u32(input, 24),
            settlement_receipts: read_u32(input, 28),
            final_pots: read_u32(input, 32),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Exact appended counted-general-Epoch retirement tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRetirementTailV1 {
    /// Nonzero generation copied into every child version.
    pub epoch_generation: u64,
    /// Exhaustive independently addressed child counts.
    pub children: EpochChildCountsV1,
    /// Funding compartments for shrink-to-tombstone.
    pub rent: RentSplitV2,
}

impl EpochRetirementTailV1 {
    /// Validate generation, child geometry, and funding compartments.
    pub const fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.epoch_generation == 0 {
            return Err(RetirementErrorV1::WrongGeneration);
        }
        match self.children.validate() {
            Ok(()) => self.rent.validate(),
            Err(error) => Err(error),
        }
    }

    /// Encode the exact 100-byte Epoch extension.
    pub fn encode(self) -> Result<[u8; EPOCH_RETIREMENT_TAIL_V1_BYTES], RetirementErrorV1> {
        self.validate()?;
        let mut out = [0u8; EPOCH_RETIREMENT_TAIL_V1_BYTES];
        out[..8].copy_from_slice(&self.epoch_generation.to_le_bytes());
        out[8..44].copy_from_slice(&self.children.encode()?);
        out[44..].copy_from_slice(&self.rent.encode()?);
        Ok(out)
    }

    /// Decode exactly 100 bytes and validate every nested component.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, EPOCH_RETIREMENT_TAIL_V1_BYTES)?;
        let value = Self {
            epoch_generation: read_u64(input, 0),
            children: EpochChildCountsV1::decode(&input[8..44])?,
            rent: RentSplitV2::decode(&input[44..])?,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Exact Market V2 monotone general-Epoch cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketEpochCursorV1 {
    /// The only general Epoch index the next open may consume.
    pub next_general_epoch_index: u64,
}

impl MarketEpochCursorV1 {
    /// Encode the exact eight-byte little-endian cursor.
    pub const fn encode(self) -> [u8; MARKET_EPOCH_CURSOR_V1_BYTES] {
        self.next_general_epoch_index.to_le_bytes()
    }

    /// Decode exactly eight bytes. `u64::MAX` is a canonical exhausted cursor;
    /// opening an Epoch from it refuses in the transition layer.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, MARKET_EPOCH_CURSOR_V1_BYTES)?;
        Ok(Self {
            next_general_epoch_index: read_u64(input, 0),
        })
    }
}

/// Exact reservation extension carrying epoch generation and the once-only
/// Position counter marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservationCountTailV1 {
    /// Authenticated parent Epoch generation.
    pub epoch_generation: u64,
    /// Whether this reservation is included in Position's outstanding count.
    pub position_counted: bool,
}

impl ReservationCountTailV1 {
    /// Validate the nonzero parent generation.
    pub const fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.epoch_generation == 0 {
            Err(RetirementErrorV1::WrongGeneration)
        } else {
            Ok(())
        }
    }

    /// Encode exactly nine bytes; a boolean is canonically zero or one.
    pub fn encode(self) -> Result<[u8; RESERVATION_COUNT_TAIL_V1_BYTES], RetirementErrorV1> {
        self.validate()?;
        let mut out = [0u8; RESERVATION_COUNT_TAIL_V1_BYTES];
        out[..8].copy_from_slice(&self.epoch_generation.to_le_bytes());
        out[8] = u8::from(self.position_counted);
        Ok(out)
    }

    /// Decode exactly nine bytes, refusing every non-boolean marker.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, RESERVATION_COUNT_TAIL_V1_BYTES)?;
        let position_counted = match input[8] {
            0 => false,
            1 => true,
            _ => return Err(RetirementErrorV1::InvalidEnum),
        };
        let value = Self {
            epoch_generation: read_u64(input, 0),
            position_counted,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Exact generation-only extension for non-reservation Epoch children.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildGenerationV1 {
    /// Authenticated parent Epoch generation.
    pub epoch_generation: u64,
}

impl ChildGenerationV1 {
    /// Validate the nonzero generation.
    pub const fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.epoch_generation == 0 {
            Err(RetirementErrorV1::WrongGeneration)
        } else {
            Ok(())
        }
    }

    /// Encode exactly eight bytes.
    pub fn encode(self) -> Result<[u8; CHILD_GENERATION_V1_BYTES], RetirementErrorV1> {
        self.validate()?;
        Ok(self.epoch_generation.to_le_bytes())
    }

    /// Decode exactly eight bytes and refuse generation zero.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, CHILD_GENERATION_V1_BYTES)?;
        let value = Self {
            epoch_generation: read_u64(input, 0),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Permanent compact Position identity occupying the original Position PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionTombstoneV1 {
    /// Market identity.
    pub market: Identity32V1,
    /// Position owner identity.
    pub owner: Identity32V1,
    /// Closed generation; reopen must increment it exactly once.
    pub generation: u64,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl PositionTombstoneV1 {
    /// Validate fields owned by this codec. Identities are valid by
    /// construction and generation zero is the canonical founding generation.
    pub const fn validate(self) -> Result<(), RetirementErrorV1> {
        Ok(())
    }

    /// Encode the exact 76-byte tagged tombstone. Byte 74 is the frozen CLOSED
    /// phase value `1`.
    pub fn encode(self) -> Result<[u8; POSITION_TOMBSTONE_V1_BYTES], RetirementErrorV1> {
        self.validate()?;
        let mut out = [0u8; POSITION_TOMBSTONE_V1_BYTES];
        out[0] = POSITION_TOMBSTONE_TAG;
        out[1] = POSITION_TOMBSTONE_VERSION_V1;
        out[2..34].copy_from_slice(&self.market.bytes());
        out[34..66].copy_from_slice(&self.owner.bytes());
        out[66..74].copy_from_slice(&self.generation.to_le_bytes());
        out[74] = 1;
        out[75] = self.stored_bump;
        Ok(out)
    }

    /// Decode and validate one exact Position tombstone.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, POSITION_TOMBSTONE_V1_BYTES)?;
        if input[0] != POSITION_TOMBSTONE_TAG {
            return Err(RetirementErrorV1::WrongTag);
        }
        if input[1] != POSITION_TOMBSTONE_VERSION_V1 {
            return Err(RetirementErrorV1::WrongVersion);
        }
        if input[74] != 1 {
            return Err(RetirementErrorV1::InvalidEnum);
        }
        let value = Self {
            market: read_identity(input, 2)?,
            owner: read_identity(input, 34)?,
            generation: read_u64(input, 66),
            stored_bump: input[75],
        };
        value.validate()?;
        Ok(value)
    }
}

/// Permanent compact general Epoch identity occupying the original Epoch PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEpochTombstoneV1 {
    /// Canonical Epoch identity.
    pub epoch: Identity32V1,
    /// Market identity.
    pub market: Identity32V1,
    /// Monotone index consumed from Market V2.
    pub epoch_index: u64,
    /// Closed generation copied from the live counted general Epoch.
    pub epoch_generation: u64,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl GeneralEpochTombstoneV1 {
    /// Validate nonzero generation. Index zero is a normal first Epoch.
    pub const fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.epoch_generation == 0 {
            Err(RetirementErrorV1::WrongGeneration)
        } else {
            Ok(())
        }
    }

    /// Encode the exact 84-byte tagged tombstone. Byte 82 is the frozen CLOSED
    /// phase value `1`.
    pub fn encode(self) -> Result<[u8; GENERAL_EPOCH_TOMBSTONE_V1_BYTES], RetirementErrorV1> {
        self.validate()?;
        let mut out = [0u8; GENERAL_EPOCH_TOMBSTONE_V1_BYTES];
        out[0] = GENERAL_EPOCH_TOMBSTONE_TAG;
        out[1] = GENERAL_EPOCH_TOMBSTONE_VERSION_V1;
        out[2..34].copy_from_slice(&self.epoch.bytes());
        out[34..66].copy_from_slice(&self.market.bytes());
        out[66..74].copy_from_slice(&self.epoch_index.to_le_bytes());
        out[74..82].copy_from_slice(&self.epoch_generation.to_le_bytes());
        out[82] = 1;
        out[83] = self.stored_bump;
        Ok(out)
    }

    /// Decode and validate one exact general Epoch tombstone.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV1> {
        exact(input, GENERAL_EPOCH_TOMBSTONE_V1_BYTES)?;
        if input[0] != GENERAL_EPOCH_TOMBSTONE_TAG {
            return Err(RetirementErrorV1::WrongTag);
        }
        if input[1] != GENERAL_EPOCH_TOMBSTONE_VERSION_V1 {
            return Err(RetirementErrorV1::WrongVersion);
        }
        if input[82] != 1 {
            return Err(RetirementErrorV1::InvalidEnum);
        }
        let value = Self {
            epoch: read_identity(input, 2)?,
            market: read_identity(input, 34)?,
            epoch_index: read_u64(input, 66),
            epoch_generation: read_u64(input, 74),
            stored_bump: input[83],
        };
        value.validate()?;
        Ok(value)
    }
}
