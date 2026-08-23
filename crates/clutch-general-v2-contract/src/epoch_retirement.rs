// SPDX-License-Identifier: AGPL-3.0-or-later

//! Settlement-complete counted General V2 Epoch and tombstone codecs.
//!
//! These types own no SBF route.  A concrete adapter must still authenticate
//! owners, PDA derivations, exact account lengths, writable roles, rent
//! minimums, and an atomic write set before using a transition produced here.

use crate::{
    CodecError, Id32, Reader, Writer, EPOCH_SEED_DOMAIN_V1, GENERAL_EPOCH_ACCOUNT_TAG,
    GENERAL_EPOCH_ACCOUNT_VERSION, GENERAL_EPOCH_TOMBSTONE_ACCOUNT_TAG,
    GENERAL_EPOCH_TOMBSTONE_ACCOUNT_VERSION, ID_BYTES, MARKET_RUNTIME_ACCOUNT_TAG,
    MARKET_RUNTIME_ACCOUNT_VERSION,
};

/// Exact bytes in [`MarketRuntimeV3AccountV1`].
pub const MARKET_RUNTIME_ACCOUNT_BYTES: usize = 148;
/// Exact bytes in the settlement-complete [`GeneralEpochV6AccountV1`].
pub const GENERAL_EPOCH_ACCOUNT_BYTES: usize = 353;
/// Exact bytes in [`GeneralEpochTombstoneV2`].
pub const GENERAL_EPOCH_TOMBSTONE_ACCOUNT_BYTES: usize = 156;

fn live(value: Id32) -> Result<(), CodecError> {
    if value.is_zero() {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn absent(value: Id32) -> Result<(), CodecError> {
    if value.is_zero() {
        Ok(())
    } else {
        Err(CodecError::InvalidState)
    }
}

fn read_id(reader: &mut Reader<'_>) -> Result<Id32, CodecError> {
    Id32::new(reader.array()?)
}

/// Canonical General V2 Epoch PDA seed tuple.
///
/// The adapter derives the PDA from three distinct seeds in this order:
/// `general-epoch:v2`, the full MarketBinding PDA, and the little-endian
/// monotone Epoch index.  This never uses a legacy semantic Epoch id as the
/// parent namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEpochSeedTupleV1 {
    market_binding: [u8; ID_BYTES],
    epoch_index_le: [u8; 8],
}

impl GeneralEpochSeedTupleV1 {
    /// Construct the exact ordered seed tuple.
    pub fn new(market_binding: Id32, epoch_index: u64) -> Result<Self, CodecError> {
        live(market_binding)?;
        Ok(Self {
            market_binding: market_binding.bytes(),
            epoch_index_le: epoch_index.to_le_bytes(),
        })
    }

    /// First seed: fresh General V2 Epoch domain.
    pub const fn domain(&self) -> &'static [u8] {
        EPOCH_SEED_DOMAIN_V1
    }

    /// Second seed: full authenticated MarketBinding PDA.
    pub const fn market_binding(&self) -> &[u8; ID_BYTES] {
        &self.market_binding
    }

    /// Third seed: monotone Epoch index in little-endian order.
    pub const fn epoch_index_le(&self) -> &[u8; 8] {
        &self.epoch_index_le
    }
}

/// Payer-owned live and permanent Epoch rent compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochRentOwnerV2 {
    /// Sole refund recipient for live-account principal.
    pub payer: Id32,
    /// Principal refunded only when the live root becomes a tombstone.
    pub refundable_live_principal: u64,
    /// Independently prepaid principal retained forever in the tombstone.
    pub permanent_tombstone_principal: u64,
    /// Prefund observed before creation; it can flow only to the neutral sink.
    pub donation_floor: u64,
}

impl EpochRentOwnerV2 {
    /// Validate both principal compartments and checked balance geometry.
    pub fn validate(self) -> Result<(), CodecError> {
        live(self.payer)?;
        if self.refundable_live_principal == 0 || self.permanent_tombstone_principal == 0 {
            return Err(CodecError::InvalidState);
        }
        self.refundable_live_principal
            .checked_add(self.permanent_tombstone_principal)
            .and_then(|value| value.checked_add(self.donation_floor))
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Disjoint semantic families whose live roots are owned by one General V2
/// Epoch.  A physical account class may contain dependent subaccounts, but it
/// must have exactly one entry in this exhaustive logical partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralV2EpochChildKindV1 {
    /// AdmissionNode plus its non-selected Feed/Stage dependents.
    CandidateBundle = 0,
    /// One CandidateIndex page.
    CandidateIndexPage = 1,
    /// One immutable candidate verdict.
    CandidateVerdict = 2,
    /// One candidate economic escrow.
    CandidateEscrow = 3,
    /// Active-width candidate ClearWork.
    ClearWorkBundle = 4,
    /// Frozen-order paging family.
    OrderPage = 5,
    /// Live Reservation or its counted terminal archive.
    ReservationArchive = 6,
    /// Once-only settlement receipts.
    SettlementReceipt = 7,
    /// Candidate-wide FinalPot liability ledger.
    FinalPot = 8,
}

/// Exhaustive live-child counts in canonical family order.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneralV2EpochChildCountsV1 {
    /// Live AdmissionNode logical bundles.
    pub candidate_bundles: u32,
    /// Live CandidateIndex pages.
    pub candidate_index_pages: u32,
    /// Live immutable candidate verdicts.
    pub candidate_verdicts: u32,
    /// Live candidate economic escrows.
    pub candidate_escrows: u32,
    /// Live active-width ClearWork accounts.
    pub clear_work_bundles: u32,
    /// Live frozen-order pages.
    pub order_pages: u32,
    /// Live Reservation/archive logical bundles.
    pub reservation_archives: u32,
    /// Live once-only settlement receipts.
    pub settlement_receipts: u32,
    /// Live FinalPot accounts; zero or one.
    pub final_pots: u32,
}

impl GeneralV2EpochChildCountsV1 {
    /// Validate the singleton FinalPot bound.
    pub fn validate(self) -> Result<(), CodecError> {
        if self.final_pots > 1 {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }

    /// Whether every logical child family is exhausted.
    pub fn is_zero(self) -> bool {
        self == Self::default()
    }

    /// Return one authoritative count by semantic child class.
    pub const fn get(self, kind: GeneralV2EpochChildKindV1) -> u32 {
        match kind {
            GeneralV2EpochChildKindV1::CandidateBundle => self.candidate_bundles,
            GeneralV2EpochChildKindV1::CandidateIndexPage => self.candidate_index_pages,
            GeneralV2EpochChildKindV1::CandidateVerdict => self.candidate_verdicts,
            GeneralV2EpochChildKindV1::CandidateEscrow => self.candidate_escrows,
            GeneralV2EpochChildKindV1::ClearWorkBundle => self.clear_work_bundles,
            GeneralV2EpochChildKindV1::OrderPage => self.order_pages,
            GeneralV2EpochChildKindV1::ReservationArchive => self.reservation_archives,
            GeneralV2EpochChildKindV1::SettlementReceipt => self.settlement_receipts,
            GeneralV2EpochChildKindV1::FinalPot => self.final_pots,
        }
    }

    /// Exact-once checked increment for one authenticated creation.
    pub fn incremented(self, kind: GeneralV2EpochChildKindV1) -> Result<Self, CodecError> {
        let mut next = self;
        let count = next.count_mut(kind);
        *count = count.checked_add(1).ok_or(CodecError::ArithmeticOverflow)?;
        next.validate()?;
        Ok(next)
    }

    /// Exact-once checked decrement for one authenticated terminal close.
    pub fn decremented(self, kind: GeneralV2EpochChildKindV1) -> Result<Self, CodecError> {
        let mut next = self;
        let count = next.count_mut(kind);
        *count = count.checked_sub(1).ok_or(CodecError::InvalidCount)?;
        next.validate()?;
        Ok(next)
    }

    fn count_mut(&mut self, kind: GeneralV2EpochChildKindV1) -> &mut u32 {
        match kind {
            GeneralV2EpochChildKindV1::CandidateBundle => &mut self.candidate_bundles,
            GeneralV2EpochChildKindV1::CandidateIndexPage => &mut self.candidate_index_pages,
            GeneralV2EpochChildKindV1::CandidateVerdict => &mut self.candidate_verdicts,
            GeneralV2EpochChildKindV1::CandidateEscrow => &mut self.candidate_escrows,
            GeneralV2EpochChildKindV1::ClearWorkBundle => &mut self.clear_work_bundles,
            GeneralV2EpochChildKindV1::OrderPage => &mut self.order_pages,
            GeneralV2EpochChildKindV1::ReservationArchive => &mut self.reservation_archives,
            GeneralV2EpochChildKindV1::SettlementReceipt => &mut self.settlement_receipts,
            GeneralV2EpochChildKindV1::FinalPot => &mut self.final_pots,
        }
    }
}

/// Canonical lifecycle phase of one counted General V2 Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralEpochPhaseV1 {
    /// Orders may still be admitted; the frozen order-set identity is absent.
    Open = 0,
    /// Orders and candidate boundaries are frozen.
    Frozen = 1,
    /// Selection is terminal; counted settlement and retirement may continue.
    Finalized = 2,
}

impl GeneralEpochPhaseV1 {
    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Open),
            1 => Ok(Self::Frozen),
            2 => Ok(Self::Finalized),
            _ => Err(CodecError::InvalidState),
        }
    }
}

/// Settlement-complete counted General V2 Epoch root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEpochV6AccountV1 {
    /// Immutable MarketBinding PDA.
    pub market_binding: Id32,
    /// Mutable monotone MarketRuntime PDA.
    pub market_runtime: Id32,
    /// Full Product successor MarketInstanceV2 identity.
    pub market_instance_v2_id: Id32,
    /// Canonical EconomicDomainV2 artifact PDA.
    pub economic_domain: Id32,
    /// Canonical Window PDA.
    pub window: Id32,
    /// Canonical root Budget PDA.
    pub budget: Id32,
    /// Frozen order-set identity; absent only while Open.
    pub order_set: Id32,
    /// Runtime-owned monotone Epoch index.
    pub epoch_index: u64,
    /// Runtime-owned nonzero generation.
    pub generation: u64,
    /// Earliest slot at which freeze may succeed.
    pub freeze_deadline_slot: u64,
    /// Actual freeze slot; zero only while Open.
    pub frozen_slot: u64,
    /// Authoritative exhaustive child counts.
    pub children: GeneralV2EpochChildCountsV1,
    /// Disjoint live/tombstone rent ownership.
    pub rent: EpochRentOwnerV2,
    /// Canonical lifecycle phase.
    pub phase: GeneralEpochPhaseV1,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl GeneralEpochV6AccountV1 {
    /// Validate exact identity, phase, count, and funding partitions.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.market_binding,
            self.market_runtime,
            self.market_instance_v2_id,
            self.economic_domain,
            self.window,
            self.budget,
        ] {
            live(id)?;
        }
        self.children.validate()?;
        self.rent.validate()?;
        if self.generation == 0 || self.freeze_deadline_slot == 0 || self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        match self.phase {
            GeneralEpochPhaseV1::Open => {
                absent(self.order_set)?;
                if self.frozen_slot != 0 || !self.children.is_zero() {
                    return Err(CodecError::InvalidState);
                }
            }
            GeneralEpochPhaseV1::Frozen => {
                live(self.order_set)?;
                if self.frozen_slot < self.freeze_deadline_slot || self.children.final_pots != 0 {
                    return Err(CodecError::InvalidState);
                }
            }
            GeneralEpochPhaseV1::Finalized => {
                live(self.order_set)?;
                if self.frozen_slot < self.freeze_deadline_slot {
                    return Err(CodecError::InvalidState);
                }
            }
        }
        Ok(())
    }

    /// Apply one authenticated exact-once child creation.
    pub fn child_created(
        mut self,
        kind: GeneralV2EpochChildKindV1,
    ) -> Result<Self, CodecError> {
        if self.phase == GeneralEpochPhaseV1::Open {
            return Err(CodecError::InvalidState);
        }
        if self.phase == GeneralEpochPhaseV1::Frozen
            && matches!(
                kind,
                GeneralV2EpochChildKindV1::ReservationArchive
                    | GeneralV2EpochChildKindV1::SettlementReceipt
                    | GeneralV2EpochChildKindV1::FinalPot
            )
        {
            return Err(CodecError::InvalidState);
        }
        self.children = self.children.incremented(kind)?;
        self.validate()?;
        Ok(self)
    }

    /// Apply one authenticated exact-once terminal child close.
    pub fn child_retired(
        mut self,
        kind: GeneralV2EpochChildKindV1,
    ) -> Result<Self, CodecError> {
        self.children = self.children.decremented(kind)?;
        self.validate()?;
        Ok(self)
    }

    /// Produce a terminal root capability only after every child family is zero.
    pub fn retirement_disposition(self) -> Result<GeneralEpochRetirementDispositionV1, CodecError> {
        self.validate()?;
        if self.phase != GeneralEpochPhaseV1::Finalized || !self.children.is_zero() {
            return Err(CodecError::InvalidState);
        }
        Ok(GeneralEpochRetirementDispositionV1 {
            market_binding: self.market_binding,
            market_runtime: self.market_runtime,
            market_instance_v2_id: self.market_instance_v2_id,
            economic_domain: self.economic_domain,
            epoch_index: self.epoch_index,
            generation: self.generation,
            rent: self.rent,
            stored_bump: self.stored_bump,
        })
    }

    /// Encode exactly [`GENERAL_EPOCH_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut writer = Writer::exact(output, GENERAL_EPOCH_ACCOUNT_BYTES)?;
        writer.u8(GENERAL_EPOCH_ACCOUNT_TAG)?;
        writer.u8(GENERAL_EPOCH_ACCOUNT_VERSION)?;
        for id in [
            self.market_binding,
            self.market_runtime,
            self.market_instance_v2_id,
            self.economic_domain,
            self.window,
            self.budget,
            self.order_set,
        ] {
            writer.bytes(&id.bytes())?;
        }
        writer.u64(self.epoch_index)?;
        writer.u64(self.generation)?;
        writer.u64(self.freeze_deadline_slot)?;
        writer.u64(self.frozen_slot)?;
        for count in [
            self.children.candidate_bundles,
            self.children.candidate_index_pages,
            self.children.candidate_verdicts,
            self.children.candidate_escrows,
            self.children.clear_work_bundles,
            self.children.order_pages,
            self.children.reservation_archives,
            self.children.settlement_receipts,
            self.children.final_pots,
        ] {
            writer.u32(count)?;
        }
        writer.bytes(&self.rent.payer.bytes())?;
        writer.u64(self.rent.refundable_live_principal)?;
        writer.u64(self.rent.permanent_tombstone_principal)?;
        writer.u64(self.rent.donation_floor)?;
        writer.u8(self.phase as u8)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode and totally validate one exact hostile byte frame.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, GENERAL_EPOCH_ACCOUNT_BYTES)?;
        if reader.u8()? != GENERAL_EPOCH_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != GENERAL_EPOCH_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let value = Self {
            market_binding: read_id(&mut reader)?,
            market_runtime: read_id(&mut reader)?,
            market_instance_v2_id: read_id(&mut reader)?,
            economic_domain: read_id(&mut reader)?,
            window: read_id(&mut reader)?,
            budget: read_id(&mut reader)?,
            order_set: Id32::from_bytes(reader.array()?),
            epoch_index: reader.u64()?,
            generation: reader.u64()?,
            freeze_deadline_slot: reader.u64()?,
            frozen_slot: reader.u64()?,
            children: GeneralV2EpochChildCountsV1 {
                candidate_bundles: reader.u32()?,
                candidate_index_pages: reader.u32()?,
                candidate_verdicts: reader.u32()?,
                candidate_escrows: reader.u32()?,
                clear_work_bundles: reader.u32()?,
                order_pages: reader.u32()?,
                reservation_archives: reader.u32()?,
                settlement_receipts: reader.u32()?,
                final_pots: reader.u32()?,
            },
            rent: EpochRentOwnerV2 {
                payer: read_id(&mut reader)?,
                refundable_live_principal: reader.u64()?,
                permanent_tombstone_principal: reader.u64()?,
                donation_floor: reader.u64()?,
            },
            phase: GeneralEpochPhaseV1::decode(reader.u8()?)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Opaque terminal root facts emitted only by the exhaustive root owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEpochRetirementDispositionV1 {
    market_binding: Id32,
    market_runtime: Id32,
    market_instance_v2_id: Id32,
    economic_domain: Id32,
    epoch_index: u64,
    generation: u64,
    rent: EpochRentOwnerV2,
    stored_bump: u8,
}

impl GeneralEpochRetirementDispositionV1 {
    /// Immutable MarketBinding PDA.
    pub const fn market_binding(self) -> Id32 { self.market_binding }
    /// Mutable MarketRuntime PDA that must record retirement exactly once.
    pub const fn market_runtime(self) -> Id32 { self.market_runtime }
    /// Product MarketInstance identity.
    pub const fn market_instance_v2_id(self) -> Id32 { self.market_instance_v2_id }
    /// EconomicDomain child required in the terminal root bundle.
    pub const fn economic_domain(self) -> Id32 { self.economic_domain }
    /// Monotone Epoch index.
    pub const fn epoch_index(self) -> u64 { self.epoch_index }
    /// Nonzero Epoch generation.
    pub const fn generation(self) -> u64 { self.generation }
    /// Exact live/permanent/donation funding split.
    pub const fn rent(self) -> EpochRentOwnerV2 { self.rent }
    /// Canonical root PDA bump retained in the tombstone.
    pub const fn stored_bump(self) -> u8 { self.stored_bump }
}

/// Genesis-assisted mutable cursor for one immutable MarketBinding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketRuntimeV3AccountV1 {
    /// Immutable MarketBinding PDA anchoring this runtime.
    pub market_binding: Id32,
    /// Full Product MarketInstance identity.
    pub market_instance_v2_id: Id32,
    /// Exact next Epoch index.
    pub next_epoch_index: u64,
    /// Exact nonzero generation for the next Epoch.
    pub next_epoch_generation: u64,
    /// Number of Epochs created through this runtime.
    pub created_epoch_count: u64,
    /// Number atomically retired to permanent tombstones.
    pub retired_epoch_count: u64,
    /// Sole refundable runtime rent owner.
    pub rent_payer: Id32,
    /// Exact refundable runtime rent principal.
    pub rent_principal: u64,
    /// Hostile prefund floor, owned only by the neutral sink.
    pub donation_floor: u64,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl MarketRuntimeV3AccountV1 {
    /// Validate monotone cursor, count, identity, and rent geometry.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [self.market_binding, self.market_instance_v2_id, self.rent_payer] {
            live(id)?;
        }
        if self.next_epoch_generation == 0
            || self.retired_epoch_count > self.created_epoch_count
            || self.rent_principal == 0
            || self.rent_principal.checked_add(self.donation_floor).is_none()
            || self.flags != 0
        {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Advance both identity cursors and the creation count exactly once.
    pub fn advanced_for_epoch(
        mut self,
        requested_index: u64,
        requested_generation: u64,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        if requested_index != self.next_epoch_index
            || requested_generation != self.next_epoch_generation
        {
            return Err(CodecError::MismatchedBinding);
        }
        self.next_epoch_index = self
            .next_epoch_index
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        self.next_epoch_generation = self
            .next_epoch_generation
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        self.created_epoch_count = self
            .created_epoch_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        self.validate()?;
        Ok(self)
    }

    /// Record one exhaustive root retirement exactly once in the Market owner.
    pub fn recorded_retirement(
        mut self,
        disposition: GeneralEpochRetirementDispositionV1,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        if disposition.market_binding != self.market_binding
            || disposition.market_runtime.is_zero()
            || disposition.market_instance_v2_id != self.market_instance_v2_id
            || disposition.generation == 0
        {
            return Err(CodecError::MismatchedBinding);
        }
        self.retired_epoch_count = self
            .retired_epoch_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        self.validate()?;
        Ok(self)
    }

    /// Encode exactly [`MARKET_RUNTIME_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut writer = Writer::exact(output, MARKET_RUNTIME_ACCOUNT_BYTES)?;
        writer.u8(MARKET_RUNTIME_ACCOUNT_TAG)?;
        writer.u8(MARKET_RUNTIME_ACCOUNT_VERSION)?;
        writer.bytes(&self.market_binding.bytes())?;
        writer.bytes(&self.market_instance_v2_id.bytes())?;
        writer.u64(self.next_epoch_index)?;
        writer.u64(self.next_epoch_generation)?;
        writer.u64(self.created_epoch_count)?;
        writer.u64(self.retired_epoch_count)?;
        writer.bytes(&self.rent_payer.bytes())?;
        writer.u64(self.rent_principal)?;
        writer.u64(self.donation_floor)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode and totally validate one exact hostile byte frame.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, MARKET_RUNTIME_ACCOUNT_BYTES)?;
        if reader.u8()? != MARKET_RUNTIME_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != MARKET_RUNTIME_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let value = Self {
            market_binding: read_id(&mut reader)?,
            market_instance_v2_id: read_id(&mut reader)?,
            next_epoch_index: reader.u64()?,
            next_epoch_generation: reader.u64()?,
            created_epoch_count: reader.u64()?,
            retired_epoch_count: reader.u64()?,
            rent_payer: read_id(&mut reader)?,
            rent_principal: reader.u64()?,
            donation_floor: reader.u64()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Permanent replay-resistant General V2 Epoch identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEpochTombstoneV2 {
    /// Immutable MarketBinding PDA.
    pub market_binding: Id32,
    /// MarketRuntime PDA that recorded the once-only retirement.
    pub market_runtime: Id32,
    /// Product MarketInstance identity.
    pub market_instance_v2_id: Id32,
    /// Historical EconomicDomain identity.
    pub economic_domain: Id32,
    /// Closed monotone Epoch index.
    pub epoch_index: u64,
    /// Closed nonzero generation.
    pub generation: u64,
    /// Exact prepaid permanent principal retained forever.
    pub permanent_tombstone_principal: u64,
    /// Stored canonical Epoch PDA bump.
    pub stored_bump: u8,
}

impl GeneralEpochTombstoneV2 {
    /// Construct the permanent tombstone from the exhaustive terminal owner.
    pub fn from_disposition(
        disposition: GeneralEpochRetirementDispositionV1,
    ) -> Result<Self, CodecError> {
        let value = Self {
            market_binding: disposition.market_binding,
            market_runtime: disposition.market_runtime,
            market_instance_v2_id: disposition.market_instance_v2_id,
            economic_domain: disposition.economic_domain,
            epoch_index: disposition.epoch_index,
            generation: disposition.generation,
            permanent_tombstone_principal: disposition.rent.permanent_tombstone_principal,
            stored_bump: disposition.stored_bump,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate permanent identity and funding.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.market_binding,
            self.market_runtime,
            self.market_instance_v2_id,
            self.economic_domain,
        ] {
            live(id)?;
        }
        if self.generation == 0 || self.permanent_tombstone_principal == 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode exactly [`GENERAL_EPOCH_TOMBSTONE_ACCOUNT_BYTES`] bytes.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut writer = Writer::exact(output, GENERAL_EPOCH_TOMBSTONE_ACCOUNT_BYTES)?;
        writer.u8(GENERAL_EPOCH_TOMBSTONE_ACCOUNT_TAG)?;
        writer.u8(GENERAL_EPOCH_TOMBSTONE_ACCOUNT_VERSION)?;
        for id in [
            self.market_binding,
            self.market_runtime,
            self.market_instance_v2_id,
            self.economic_domain,
        ] {
            writer.bytes(&id.bytes())?;
        }
        writer.u64(self.epoch_index)?;
        writer.u64(self.generation)?;
        writer.u64(self.permanent_tombstone_principal)?;
        writer.u8(1)?;
        writer.u8(self.stored_bump)?;
        writer.finish()
    }

    /// Decode and totally validate one exact hostile byte frame.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, GENERAL_EPOCH_TOMBSTONE_ACCOUNT_BYTES)?;
        if reader.u8()? != GENERAL_EPOCH_TOMBSTONE_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != GENERAL_EPOCH_TOMBSTONE_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let value = Self {
            market_binding: read_id(&mut reader)?,
            market_runtime: read_id(&mut reader)?,
            market_instance_v2_id: read_id(&mut reader)?,
            economic_domain: read_id(&mut reader)?,
            epoch_index: reader.u64()?,
            generation: reader.u64()?,
            permanent_tombstone_principal: reader.u64()?,
            stored_bump: {
                if reader.u8()? != 1 {
                    return Err(CodecError::InvalidState);
                }
                reader.u8()?
            },
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(GENERAL_EPOCH_ACCOUNT_BYTES == 2 + (7 * 32) + 32 + 36 + 56 + 3);
const _: () = assert!(GENERAL_EPOCH_TOMBSTONE_ACCOUNT_BYTES == 2 + (4 * 32) + 24 + 2);
