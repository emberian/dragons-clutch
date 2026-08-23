// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]

//! MODEL-ONLY successor accounting for `ClosePosition` and
//! `CloseGeneralEpoch`.
//!
//! This crate is not an account codec or an SBF implementation. It is a
//! fixed-capacity transition model for the persisted facts the adapter must
//! authenticate. All mutators consume a `Copy` snapshot and return the next
//! snapshot only on success. [`Fault`] checkpoints expose the required
//! transaction boundary: a caller retains the original snapshot after any
//! injected failure, just as all account writes roll back when one Solana
//! instruction returns an error.

pub const MAX_EPOCHS: usize = 4;
pub const MAX_CANDIDATES: usize = 8;
pub const MAX_CLEAR_WORK: usize = 8;
pub const MAX_RESERVATIONS: usize = 8;
pub const MAX_AUX_CHILDREN: usize = 16;

pub type Id = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ZeroIdentity,
    LegacyStop,
    WrongPhase,
    WrongParent,
    WrongGeneration,
    NonmonotoneEpoch,
    EpochOverflow,
    GenerationOverflow,
    Capacity,
    DuplicateChild,
    MissingChild,
    WrongChildKind,
    InvalidCandidateState,
    ClearWorkOutstanding,
    InsufficientAssets,
    EconomicBalanceOutstanding,
    ReservationOutstanding,
    ChildOutstanding,
    CounterOverflow,
    CounterUnderflow,
    CounterMismatch,
    InvalidReservationState,
    AlreadyTerminal,
    InjectedCrash,
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Version {
    LegacyV1,
    CountedV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    Never,
    At(u8),
}

fn checkpoint(fault: Fault, stage: u8) -> Result<()> {
    if fault == Fault::At(stage) {
        Err(Error::InjectedCrash)
    } else {
        Ok(())
    }
}

fn live(id: Id) -> Result<()> {
    if id == 0 {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn increment(value: u32) -> Result<u32> {
    value.checked_add(1).ok_or(Error::CounterOverflow)
}

fn decrement(value: u32) -> Result<u32> {
    value.checked_sub(1).ok_or(Error::CounterUnderflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionPhase {
    Open,
    Tombstone,
}

/// The semantic contents of one owner/Market Position anchor.
///
/// A runtime may keep these fields in one reallocatable PDA or split a compact
/// permanent identity anchor from a bulky balance account. It must not delete
/// the last copy of `generation`, `phase`, or `outstanding_reservations`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionAnchor {
    pub market: Id,
    pub owner: Id,
    pub generation: u64,
    pub phase: PositionPhase,
    pub cash_atoms: u64,
    pub reserved_cash_atoms: u64,
    pub egg_atoms: u64,
    pub outstanding_reservations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpochPhase {
    Open,
    Terminal,
    Tombstone,
}

/// Exhaustive counts of independently addressed child bundles.
///
/// Funding ledgers/tails are part of their governed bundle and must be created
/// and closed in the same transaction. Epoch + Window + root funding are the
/// root bundle itself, not three independently counted children.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChildCounts {
    pub candidate_bundles: u32,
    pub candidate_index_pages: u32,
    pub candidate_verdicts: u32,
    pub candidate_escrows: u32,
    pub clear_work_bundles: u32,
    pub order_pages: u32,
    pub reservation_archives: u32,
    pub receipts: u32,
    pub pots: u32,
}

impl ChildCounts {
    pub const fn is_zero(self) -> bool {
        self.candidate_bundles == 0
            && self.candidate_index_pages == 0
            && self.candidate_verdicts == 0
            && self.candidate_escrows == 0
            && self.clear_work_bundles == 0
            && self.order_pages == 0
            && self.reservation_archives == 0
            && self.receipts == 0
            && self.pots == 0
    }

    fn get(self, kind: ChildKind) -> u32 {
        match kind {
            ChildKind::CandidateBundle => self.candidate_bundles,
            ChildKind::CandidateIndexPage => self.candidate_index_pages,
            ChildKind::CandidateVerdict => self.candidate_verdicts,
            ChildKind::CandidateEscrow => self.candidate_escrows,
            ChildKind::ClearWorkBundle => self.clear_work_bundles,
            ChildKind::OrderPage => self.order_pages,
            ChildKind::ReservationArchive => self.reservation_archives,
            ChildKind::Receipt => self.receipts,
            ChildKind::Pot => self.pots,
        }
    }

    fn set(&mut self, kind: ChildKind, value: u32) {
        match kind {
            ChildKind::CandidateBundle => self.candidate_bundles = value,
            ChildKind::CandidateIndexPage => self.candidate_index_pages = value,
            ChildKind::CandidateVerdict => self.candidate_verdicts = value,
            ChildKind::CandidateEscrow => self.candidate_escrows = value,
            ChildKind::ClearWorkBundle => self.clear_work_bundles = value,
            ChildKind::OrderPage => self.order_pages = value,
            ChildKind::ReservationArchive => self.reservation_archives = value,
            ChildKind::Receipt => self.receipts = value,
            ChildKind::Pot => self.pots = value,
        }
    }

    fn add_one(&mut self, kind: ChildKind) -> Result<()> {
        self.set(kind, increment(self.get(kind))?);
        Ok(())
    }

    fn sub_one(&mut self, kind: ChildKind) -> Result<()> {
        self.set(kind, decrement(self.get(kind))?);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochAnchor {
    pub occupied: bool,
    pub market: Id,
    pub epoch: Id,
    pub epoch_index: u64,
    pub generation: u64,
    pub phase: EpochPhase,
    pub children: ChildCounts,
}

impl EpochAnchor {
    const EMPTY: Self = Self {
        occupied: false,
        market: 0,
        epoch: 0,
        epoch_index: 0,
        generation: 0,
        phase: EpochPhase::Tombstone,
        children: ChildCounts {
            candidate_bundles: 0,
            candidate_index_pages: 0,
            candidate_verdicts: 0,
            candidate_escrows: 0,
            clear_work_bundles: 0,
            order_pages: 0,
            reservation_archives: 0,
            receipts: 0,
            pots: 0,
        },
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateState {
    Submitted,
    Staging,
    SealedUnverified,
    VerifiedValid,
    VerifiedRetained,
    Superseded,
    Refused,
    ExpiredStaging,
    ExpiredUnverified,
    Selected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildKind {
    CandidateBundle,
    CandidateIndexPage,
    CandidateVerdict,
    CandidateEscrow,
    ClearWorkBundle,
    OrderPage,
    ReservationArchive,
    Receipt,
    Pot,
}

/// Authenticated registration carried by every V2 child.
///
/// In the adapter, equality means program ownership, exact version/length,
/// canonical PDA derivation, and equality of all four persisted fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Registration {
    pub market: Id,
    pub epoch: Id,
    pub epoch_generation: u64,
    pub child: Id,
    pub kind: ChildKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Candidate {
    occupied: bool,
    registration: Registration,
    state: CandidateState,
}

impl Candidate {
    const EMPTY: Self = Self {
        occupied: false,
        registration: Registration {
            market: 0,
            epoch: 0,
            epoch_generation: 0,
            child: 0,
            kind: ChildKind::CandidateBundle,
        },
        state: CandidateState::Submitted,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClearWork {
    occupied: bool,
    registration: Registration,
    candidate: Id,
}

impl ClearWork {
    const EMPTY: Self = Self {
        occupied: false,
        registration: Registration {
            market: 0,
            epoch: 0,
            epoch_generation: 0,
            child: 0,
            kind: ChildKind::ClearWorkBundle,
        },
        candidate: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReservationState {
    Active,
    Entitled,
    Released,
    Consumed,
}

impl ReservationState {
    const fn is_live(self) -> bool {
        matches!(self, Self::Active | Self::Entitled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Reservation {
    occupied: bool,
    registration: Registration,
    position_generation: u64,
    state: ReservationState,
    remaining_assets: u64,
    position_counted: bool,
}

impl Reservation {
    const EMPTY: Self = Self {
        occupied: false,
        registration: Registration {
            market: 0,
            epoch: 0,
            epoch_generation: 0,
            child: 0,
            kind: ChildKind::ReservationArchive,
        },
        position_generation: 0,
        state: ReservationState::Released,
        remaining_assets: 0,
        position_counted: false,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuxChild {
    occupied: bool,
    registration: Registration,
}

impl AuxChild {
    const EMPTY: Self = Self {
        occupied: false,
        registration: Registration {
            market: 0,
            epoch: 0,
            epoch_generation: 0,
            child: 0,
            kind: ChildKind::OrderPage,
        },
    };
}

/// Fixed-capacity protocol snapshot used by the adversarial model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Protocol {
    pub version: Version,
    pub market: Id,
    /// Market-owned monotone cursor. It advances at epoch admission and is
    /// never changed by retirement.
    pub next_epoch_index: u64,
    pub position: PositionAnchor,
    epochs: [EpochAnchor; MAX_EPOCHS],
    candidates: [Candidate; MAX_CANDIDATES],
    clear_work: [ClearWork; MAX_CLEAR_WORK],
    reservations: [Reservation; MAX_RESERVATIONS],
    aux: [AuxChild; MAX_AUX_CHILDREN],
}

impl Protocol {
    pub fn new_v2(market: Id, owner: Id, egg_atoms: u64) -> Result<Self> {
        live(market)?;
        live(owner)?;
        let model = Self {
            version: Version::CountedV2,
            market,
            next_epoch_index: 0,
            position: PositionAnchor {
                market,
                owner,
                generation: 1,
                phase: PositionPhase::Open,
                cash_atoms: 0,
                reserved_cash_atoms: 0,
                egg_atoms,
                outstanding_reservations: 0,
            },
            epochs: [EpochAnchor::EMPTY; MAX_EPOCHS],
            candidates: [Candidate::EMPTY; MAX_CANDIDATES],
            clear_work: [ClearWork::EMPTY; MAX_CLEAR_WORK],
            reservations: [Reservation::EMPTY; MAX_RESERVATIONS],
            aux: [AuxChild::EMPTY; MAX_AUX_CHILDREN],
        };
        model.check()?;
        Ok(model)
    }

    /// A legacy snapshot is intentionally not migratable from local account
    /// state: it has no authenticated census. Both dangerous closes return
    /// [`Error::LegacyStop`].
    pub fn legacy_for_stop(market: Id, owner: Id) -> Result<Self> {
        let mut model = Self::new_v2(market, owner, 0)?;
        model.version = Version::LegacyV1;
        model.epochs[0] = EpochAnchor {
            occupied: true,
            market,
            epoch: 9,
            epoch_index: 0,
            generation: 1,
            phase: EpochPhase::Terminal,
            children: ChildCounts::default(),
        };
        model.next_epoch_index = 1;
        Ok(model)
    }

    pub fn epoch(&self, epoch: Id) -> Option<EpochAnchor> {
        self.epochs
            .iter()
            .find(|entry| entry.occupied && entry.epoch == epoch)
            .copied()
    }

    pub fn reservation_state(&self, child: Id) -> Option<ReservationState> {
        self.reservations
            .iter()
            .find(|entry| entry.occupied && entry.registration.child == child)
            .map(|entry| entry.state)
    }

    fn require_v2(&self) -> Result<()> {
        if self.version == Version::CountedV2 {
            Ok(())
        } else {
            Err(Error::LegacyStop)
        }
    }

    fn epoch_index(&self, epoch: Id) -> Result<usize> {
        self.epochs
            .iter()
            .position(|entry| entry.occupied && entry.epoch == epoch)
            .ok_or(Error::WrongParent)
    }

    fn authenticate(&self, registration: Registration, kind: ChildKind) -> Result<usize> {
        if registration.kind != kind {
            return Err(Error::WrongChildKind);
        }
        let index = self.epoch_index(registration.epoch)?;
        let epoch = self.epochs[index];
        if registration.market != self.market || epoch.market != self.market {
            return Err(Error::WrongParent);
        }
        if registration.epoch_generation != epoch.generation {
            return Err(Error::WrongGeneration);
        }
        Ok(index)
    }

    fn registration(&self, epoch_index: usize, child: Id, kind: ChildKind) -> Registration {
        let epoch = self.epochs[epoch_index];
        Registration {
            market: self.market,
            epoch: epoch.epoch,
            epoch_generation: epoch.generation,
            child,
            kind,
        }
    }

    fn duplicate_child(&self, child: Id) -> bool {
        self.candidates
            .iter()
            .any(|entry| entry.occupied && entry.registration.child == child)
            || self
                .clear_work
                .iter()
                .any(|entry| entry.occupied && entry.registration.child == child)
            || self
                .reservations
                .iter()
                .any(|entry| entry.occupied && entry.registration.child == child)
            || self
                .aux
                .iter()
                .any(|entry| entry.occupied && entry.registration.child == child)
    }

    pub fn open_epoch(self, epoch: Id, epoch_index: u64, fault: Fault) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        live(epoch)?;
        if epoch_index != self.next_epoch_index {
            return Err(Error::NonmonotoneEpoch);
        }
        if self.epochs.iter().any(|entry| {
            entry.occupied && (entry.epoch == epoch || entry.epoch_index == epoch_index)
        }) {
            return Err(Error::DuplicateChild);
        }
        let slot = self
            .epochs
            .iter()
            .position(|entry| !entry.occupied)
            .ok_or(Error::Capacity)?;
        let next_index = epoch_index.checked_add(1).ok_or(Error::EpochOverflow)?;
        let generation = next_index;
        let mut next = self;
        next.epochs[slot] = EpochAnchor {
            occupied: true,
            market: self.market,
            epoch,
            epoch_index,
            generation,
            phase: EpochPhase::Open,
            children: ChildCounts::default(),
        };
        checkpoint(fault, 1)?;
        next.next_epoch_index = next_index;
        checkpoint(fault, 2)?;
        next.check()?;
        Ok(next)
    }

    pub fn mark_epoch_terminal(self, epoch: Id) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        let index = self.epoch_index(epoch)?;
        if self.epochs[index].phase != EpochPhase::Open {
            return Err(Error::WrongPhase);
        }
        let mut next = self;
        next.epochs[index].phase = EpochPhase::Terminal;
        next.check()?;
        Ok(next)
    }

    pub fn create_candidate(
        self,
        epoch: Id,
        child: Id,
        state: CandidateState,
        fault: Fault,
    ) -> Result<(Self, Registration)> {
        self.require_v2()?;
        self.check()?;
        live(child)?;
        let epoch_index = self.epoch_index(epoch)?;
        if self.epochs[epoch_index].phase != EpochPhase::Open {
            return Err(Error::WrongPhase);
        }
        if self.duplicate_child(child) {
            return Err(Error::DuplicateChild);
        }
        let slot = self
            .candidates
            .iter()
            .position(|entry| !entry.occupied)
            .ok_or(Error::Capacity)?;
        let registration = self.registration(epoch_index, child, ChildKind::CandidateBundle);
        let mut next = self;
        next.epochs[epoch_index]
            .children
            .add_one(ChildKind::CandidateBundle)?;
        checkpoint(fault, 1)?;
        next.candidates[slot] = Candidate {
            occupied: true,
            registration,
            state,
        };
        checkpoint(fault, 2)?;
        next.check()?;
        Ok((next, registration))
    }

    pub fn set_candidate_state(
        self,
        registration: Registration,
        state: CandidateState,
    ) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        self.authenticate(registration, ChildKind::CandidateBundle)?;
        let slot = self
            .candidates
            .iter()
            .position(|entry| entry.occupied && entry.registration == registration)
            .ok_or(Error::MissingChild)?;
        let mut next = self;
        next.candidates[slot].state = state;
        next.check()?;
        Ok(next)
    }

    pub fn create_clear_work(
        self,
        candidate: Registration,
        child: Id,
        fault: Fault,
    ) -> Result<(Self, Registration)> {
        self.require_v2()?;
        self.check()?;
        live(child)?;
        let epoch_index = self.authenticate(candidate, ChildKind::CandidateBundle)?;
        if self.epochs[epoch_index].phase != EpochPhase::Open {
            return Err(Error::WrongPhase);
        }
        if !self
            .candidates
            .iter()
            .any(|entry| entry.occupied && entry.registration == candidate)
        {
            return Err(Error::MissingChild);
        }
        if self.duplicate_child(child)
            || self.clear_work.iter().any(|entry| {
                entry.occupied
                    && entry.registration.epoch == candidate.epoch
                    && entry.candidate == candidate.child
            })
        {
            return Err(Error::DuplicateChild);
        }
        let slot = self
            .clear_work
            .iter()
            .position(|entry| !entry.occupied)
            .ok_or(Error::Capacity)?;
        let registration = self.registration(epoch_index, child, ChildKind::ClearWorkBundle);
        let mut next = self;
        next.epochs[epoch_index]
            .children
            .add_one(ChildKind::ClearWorkBundle)?;
        checkpoint(fault, 1)?;
        next.clear_work[slot] = ClearWork {
            occupied: true,
            registration,
            candidate: candidate.child,
        };
        checkpoint(fault, 2)?;
        next.check()?;
        Ok((next, registration))
    }

    pub fn close_clear_work(self, registration: Registration, fault: Fault) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        let epoch_index = self.authenticate(registration, ChildKind::ClearWorkBundle)?;
        if self.epochs[epoch_index].phase != EpochPhase::Terminal {
            return Err(Error::WrongPhase);
        }
        let slot = self
            .clear_work
            .iter()
            .position(|entry| entry.occupied && entry.registration == registration)
            .ok_or(Error::MissingChild)?;
        let mut next = self;
        next.epochs[epoch_index]
            .children
            .sub_one(ChildKind::ClearWorkBundle)?;
        checkpoint(fault, 1)?;
        next.clear_work[slot] = ClearWork::EMPTY;
        checkpoint(fault, 2)?;
        next.check()?;
        Ok(next)
    }

    pub fn close_candidate(self, registration: Registration, fault: Fault) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        let epoch_index = self.authenticate(registration, ChildKind::CandidateBundle)?;
        if self.epochs[epoch_index].phase != EpochPhase::Terminal {
            return Err(Error::WrongPhase);
        }
        let slot = self
            .candidates
            .iter()
            .position(|entry| entry.occupied && entry.registration == registration)
            .ok_or(Error::MissingChild)?;
        if self.clear_work.iter().any(|entry| {
            entry.occupied
                && entry.registration.epoch == registration.epoch
                && entry.candidate == registration.child
        }) {
            return Err(Error::ClearWorkOutstanding);
        }
        let mut next = self;
        next.epochs[epoch_index]
            .children
            .sub_one(ChildKind::CandidateBundle)?;
        checkpoint(fault, 1)?;
        next.candidates[slot] = Candidate::EMPTY;
        checkpoint(fault, 2)?;
        next.check()?;
        Ok(next)
    }

    pub fn create_aux(
        self,
        epoch: Id,
        child: Id,
        kind: ChildKind,
        fault: Fault,
    ) -> Result<(Self, Registration)> {
        self.require_v2()?;
        self.check()?;
        live(child)?;
        if matches!(
            kind,
            ChildKind::CandidateBundle | ChildKind::ClearWorkBundle | ChildKind::ReservationArchive
        ) {
            return Err(Error::WrongChildKind);
        }
        let epoch_index = self.epoch_index(epoch)?;
        if self.epochs[epoch_index].phase != EpochPhase::Open {
            return Err(Error::WrongPhase);
        }
        if self.duplicate_child(child)
            || (kind == ChildKind::Pot
                && self.aux.iter().any(|entry| {
                    entry.occupied
                        && entry.registration.epoch == epoch
                        && entry.registration.kind == ChildKind::Pot
                }))
        {
            return Err(Error::DuplicateChild);
        }
        let slot = self
            .aux
            .iter()
            .position(|entry| !entry.occupied)
            .ok_or(Error::Capacity)?;
        let registration = self.registration(epoch_index, child, kind);
        let mut next = self;
        next.epochs[epoch_index].children.add_one(kind)?;
        checkpoint(fault, 1)?;
        next.aux[slot] = AuxChild {
            occupied: true,
            registration,
        };
        checkpoint(fault, 2)?;
        next.check()?;
        Ok((next, registration))
    }

    pub fn close_aux(self, registration: Registration, fault: Fault) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        if matches!(
            registration.kind,
            ChildKind::CandidateBundle | ChildKind::ClearWorkBundle | ChildKind::ReservationArchive
        ) {
            return Err(Error::WrongChildKind);
        }
        let epoch_index = self.authenticate(registration, registration.kind)?;
        if self.epochs[epoch_index].phase != EpochPhase::Terminal {
            return Err(Error::WrongPhase);
        }
        let slot = self
            .aux
            .iter()
            .position(|entry| entry.occupied && entry.registration == registration)
            .ok_or(Error::MissingChild)?;
        let mut next = self;
        next.epochs[epoch_index]
            .children
            .sub_one(registration.kind)?;
        checkpoint(fault, 1)?;
        next.aux[slot] = AuxChild::EMPTY;
        checkpoint(fault, 2)?;
        next.check()?;
        Ok(next)
    }

    /// Move seller assets into a reservation and increment both semantic
    /// owners in one transaction: Position economic ownership and Epoch rent
    /// child ownership.
    pub fn create_sell_reservation(
        self,
        epoch: Id,
        child: Id,
        egg_atoms: u64,
        fault: Fault,
    ) -> Result<(Self, Registration)> {
        self.require_v2()?;
        self.check()?;
        live(child)?;
        if self.position.phase != PositionPhase::Open {
            return Err(Error::WrongPhase);
        }
        if egg_atoms == 0 || egg_atoms > self.position.egg_atoms {
            return Err(Error::InsufficientAssets);
        }
        let epoch_index = self.epoch_index(epoch)?;
        if self.epochs[epoch_index].phase != EpochPhase::Open {
            return Err(Error::WrongPhase);
        }
        if self.duplicate_child(child) {
            return Err(Error::DuplicateChild);
        }
        let slot = self
            .reservations
            .iter()
            .position(|entry| !entry.occupied)
            .ok_or(Error::Capacity)?;
        let position_count = increment(self.position.outstanding_reservations)?;
        let epoch_count = increment(self.epochs[epoch_index].children.reservation_archives)?;
        let registration = self.registration(epoch_index, child, ChildKind::ReservationArchive);
        let mut next = self;
        next.position.egg_atoms -= egg_atoms;
        checkpoint(fault, 1)?;
        next.position.outstanding_reservations = position_count;
        checkpoint(fault, 2)?;
        next.epochs[epoch_index].children.reservation_archives = epoch_count;
        checkpoint(fault, 3)?;
        next.reservations[slot] = Reservation {
            occupied: true,
            registration,
            position_generation: self.position.generation,
            state: ReservationState::Active,
            remaining_assets: egg_atoms,
            position_counted: true,
        };
        checkpoint(fault, 4)?;
        next.check()?;
        Ok((next, registration))
    }

    pub fn entitle_reservation(self, registration: Registration) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        self.authenticate(registration, ChildKind::ReservationArchive)?;
        let slot = self
            .reservations
            .iter()
            .position(|entry| entry.occupied && entry.registration == registration)
            .ok_or(Error::MissingChild)?;
        if self.reservations[slot].state != ReservationState::Active {
            return Err(Error::InvalidReservationState);
        }
        let mut next = self;
        next.reservations[slot].state = ReservationState::Entitled;
        next.check()?;
        Ok(next)
    }

    /// Make the first terminal reservation transition. Released assets return
    /// to the same live Position generation; consumed assets do not. In both
    /// cases the persisted counted marker and Position counter move exactly
    /// once in this transaction.
    pub fn terminate_reservation(
        self,
        registration: Registration,
        disposition: ReservationState,
        fault: Fault,
    ) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        self.authenticate(registration, ChildKind::ReservationArchive)?;
        if !matches!(
            disposition,
            ReservationState::Released | ReservationState::Consumed
        ) {
            return Err(Error::InvalidReservationState);
        }
        let slot = self
            .reservations
            .iter()
            .position(|entry| entry.occupied && entry.registration == registration)
            .ok_or(Error::MissingChild)?;
        let reservation = self.reservations[slot];
        if !reservation.state.is_live() || !reservation.position_counted {
            return Err(Error::AlreadyTerminal);
        }
        if reservation.position_generation != self.position.generation
            || self.position.phase != PositionPhase::Open
        {
            return Err(Error::WrongGeneration);
        }
        let returned = if disposition == ReservationState::Released {
            self.position
                .egg_atoms
                .checked_add(reservation.remaining_assets)
                .ok_or(Error::CounterOverflow)?
        } else {
            self.position.egg_atoms
        };
        let position_count = decrement(self.position.outstanding_reservations)?;
        let mut next = self;
        next.position.egg_atoms = returned;
        checkpoint(fault, 1)?;
        next.reservations[slot].state = disposition;
        next.reservations[slot].remaining_assets = 0;
        checkpoint(fault, 2)?;
        next.position.outstanding_reservations = position_count;
        checkpoint(fault, 3)?;
        next.reservations[slot].position_counted = false;
        checkpoint(fault, 4)?;
        next.check()?;
        Ok(next)
    }

    /// Delete only the terminal reservation archive bundle. Economic
    /// accounting was already debited by [`Self::terminate_reservation`].
    pub fn close_reservation_archive(
        self,
        registration: Registration,
        fault: Fault,
    ) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        let epoch_index = self.authenticate(registration, ChildKind::ReservationArchive)?;
        if self.epochs[epoch_index].phase != EpochPhase::Terminal {
            return Err(Error::WrongPhase);
        }
        let slot = self
            .reservations
            .iter()
            .position(|entry| entry.occupied && entry.registration == registration)
            .ok_or(Error::MissingChild)?;
        let reservation = self.reservations[slot];
        if reservation.state.is_live() || reservation.position_counted {
            return Err(Error::ReservationOutstanding);
        }
        let mut next = self;
        next.epochs[epoch_index]
            .children
            .sub_one(ChildKind::ReservationArchive)?;
        checkpoint(fault, 1)?;
        next.reservations[slot] = Reservation::EMPTY;
        checkpoint(fault, 2)?;
        next.check()?;
        Ok(next)
    }

    /// Shrink the Position to its permanent tombstone. Local economic zero is
    /// necessary but never substitutes for the authenticated reservation
    /// counter.
    pub fn close_position(self, fault: Fault) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        if self.position.phase != PositionPhase::Open {
            return Err(Error::AlreadyTerminal);
        }
        if self.position.cash_atoms != 0
            || self.position.reserved_cash_atoms != 0
            || self.position.egg_atoms != 0
        {
            return Err(Error::EconomicBalanceOutstanding);
        }
        if self.position.outstanding_reservations != 0 {
            return Err(Error::ReservationOutstanding);
        }
        let mut next = self;
        next.position.phase = PositionPhase::Tombstone;
        checkpoint(fault, 1)?;
        next.check()?;
        Ok(next)
    }

    /// Re-expand the same permanent Position identity with the next generation.
    pub fn reopen_position(self, egg_atoms: u64, fault: Fault) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        if self.position.phase != PositionPhase::Tombstone {
            return Err(Error::WrongPhase);
        }
        let generation = self
            .position
            .generation
            .checked_add(1)
            .ok_or(Error::GenerationOverflow)?;
        let mut next = self;
        next.position.generation = generation;
        checkpoint(fault, 1)?;
        next.position.phase = PositionPhase::Open;
        next.position.egg_atoms = egg_atoms;
        checkpoint(fault, 2)?;
        next.check()?;
        Ok(next)
    }

    /// Shrink Epoch + Window + root funding to the permanent epoch identity.
    pub fn close_epoch(self, epoch: Id, fault: Fault) -> Result<Self> {
        self.require_v2()?;
        self.check()?;
        let index = self.epoch_index(epoch)?;
        if self.epochs[index].phase != EpochPhase::Terminal {
            return Err(Error::WrongPhase);
        }
        if !self.epochs[index].children.is_zero() {
            return Err(Error::ChildOutstanding);
        }
        let mut next = self;
        next.epochs[index].phase = EpochPhase::Tombstone;
        checkpoint(fault, 1)?;
        next.check()?;
        Ok(next)
    }

    /// Recompute every count from authenticated child state and reject any
    /// mismatch. A runtime cannot scan global accounts this way; its induction
    /// starts only at a fresh V2 root and is preserved by the transitions above.
    pub fn check(&self) -> Result<()> {
        live(self.market)?;
        live(self.position.owner)?;
        if self.position.market != self.market || self.position.generation == 0 {
            return Err(Error::WrongParent);
        }
        let mut position_count = 0u32;
        let mut derived = [ChildCounts::default(); MAX_EPOCHS];

        let mut i = 0usize;
        while i < MAX_EPOCHS {
            let epoch = self.epochs[i];
            if epoch.occupied {
                if epoch.market != self.market
                    || epoch.epoch == 0
                    || epoch.generation == 0
                    || epoch.epoch_index >= self.next_epoch_index
                {
                    return Err(Error::WrongParent);
                }
                if epoch.phase == EpochPhase::Tombstone && !epoch.children.is_zero() {
                    return Err(Error::ChildOutstanding);
                }
                let mut j = i + 1;
                while j < MAX_EPOCHS {
                    let other = self.epochs[j];
                    if other.occupied
                        && (other.epoch == epoch.epoch || other.epoch_index == epoch.epoch_index)
                    {
                        return Err(Error::DuplicateChild);
                    }
                    j += 1;
                }
            }
            i += 1;
        }

        for candidate in self.candidates.iter().filter(|entry| entry.occupied) {
            let index = self.authenticate(candidate.registration, ChildKind::CandidateBundle)?;
            derived[index].add_one(ChildKind::CandidateBundle)?;
        }
        for work in self.clear_work.iter().filter(|entry| entry.occupied) {
            let index = self.authenticate(work.registration, ChildKind::ClearWorkBundle)?;
            if !self.candidates.iter().any(|candidate| {
                candidate.occupied
                    && candidate.registration.epoch == work.registration.epoch
                    && candidate.registration.child == work.candidate
            }) {
                return Err(Error::MissingChild);
            }
            derived[index].add_one(ChildKind::ClearWorkBundle)?;
        }
        for reservation in self.reservations.iter().filter(|entry| entry.occupied) {
            let index =
                self.authenticate(reservation.registration, ChildKind::ReservationArchive)?;
            derived[index].add_one(ChildKind::ReservationArchive)?;
            if reservation.state.is_live() {
                if !reservation.position_counted || reservation.remaining_assets == 0 {
                    return Err(Error::InvalidReservationState);
                }
            } else if reservation.position_counted || reservation.remaining_assets != 0 {
                return Err(Error::InvalidReservationState);
            }
            if reservation.position_counted {
                if reservation.position_generation != self.position.generation
                    || self.position.phase != PositionPhase::Open
                {
                    return Err(Error::WrongGeneration);
                }
                position_count = increment(position_count)?;
            }
        }
        for child in self.aux.iter().filter(|entry| entry.occupied) {
            let index = self.authenticate(child.registration, child.registration.kind)?;
            if matches!(
                child.registration.kind,
                ChildKind::CandidateBundle
                    | ChildKind::ClearWorkBundle
                    | ChildKind::ReservationArchive
            ) {
                return Err(Error::WrongChildKind);
            }
            derived[index].add_one(child.registration.kind)?;
        }
        if position_count != self.position.outstanding_reservations {
            return Err(Error::CounterMismatch);
        }
        if self.position.phase == PositionPhase::Tombstone
            && (position_count != 0
                || self.position.cash_atoms != 0
                || self.position.reserved_cash_atoms != 0
                || self.position.egg_atoms != 0)
        {
            return Err(Error::ReservationOutstanding);
        }
        for (index, epoch) in self.epochs.iter().enumerate() {
            if epoch.occupied && epoch.children != derived[index] {
                return Err(Error::CounterMismatch);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch() -> Protocol {
        Protocol::new_v2(1, 2, 0)
            .unwrap()
            .open_epoch(10, 0, Fault::Never)
            .unwrap()
    }

    #[test]
    fn corrupted_zero_counts_never_authorize_close() {
        let (model, candidate) = epoch()
            .create_candidate(10, 20, CandidateState::SealedUnverified, Fault::Never)
            .unwrap();
        let mut forged = model;
        forged.epochs[0].children.candidate_bundles = 0;
        assert_eq!(forged.check(), Err(Error::CounterMismatch));
        assert_eq!(
            forged.close_epoch(10, Fault::Never),
            Err(Error::CounterMismatch)
        );

        let mut wrong_ticket = candidate;
        wrong_ticket.epoch_generation += 1;
        assert_eq!(
            model.close_candidate(wrong_ticket, Fault::Never),
            Err(Error::WrongGeneration)
        );
    }

    #[test]
    fn legacy_population_has_no_local_upgrade_to_counted_close() {
        let legacy = Protocol::legacy_for_stop(1, 2).unwrap();
        assert_eq!(legacy.close_position(Fault::Never), Err(Error::LegacyStop));
        assert_eq!(legacy.close_epoch(9, Fault::Never), Err(Error::LegacyStop));
    }

    #[test]
    fn arithmetic_edges_and_forged_persisted_registrations_refuse() {
        let mut counts = ChildCounts::default();
        assert_eq!(
            counts.sub_one(ChildKind::CandidateBundle),
            Err(Error::CounterUnderflow)
        );
        counts.candidate_bundles = u32::MAX;
        assert_eq!(
            counts.add_one(ChildKind::CandidateBundle),
            Err(Error::CounterOverflow)
        );

        let (mut model, _) = epoch()
            .create_candidate(10, 20, CandidateState::Submitted, Fault::Never)
            .unwrap();
        model.candidates[0].registration.epoch_generation += 1;
        assert_eq!(model.check(), Err(Error::WrongGeneration));
        assert_eq!(
            model.close_epoch(10, Fault::Never),
            Err(Error::WrongGeneration)
        );
    }
}
