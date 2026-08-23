// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    CandidateStatusWitnessV1, EpochChildKindV1, EpochRetirementTailV1, GeneralEpochPhaseV1,
    GeneralEpochTombstoneV1, Identity32V1, MarketEpochCursorV1, PositionRetirementTailV1,
    PositionTombstoneV1, RentSplitV2, ReservationCountTailV1, ReservationStateV1,
    RetirementErrorV1, MAX_OUTCOMES,
};

/// Exact terminal distribution derived from a persisted rent split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentDispositionV2 {
    /// Stored payer receiving refundable live principal.
    pub payer: Identity32V1,
    /// Exact live principal returned to `payer`.
    pub payer_refund_lamports: u64,
    /// Principal left in the permanent tombstone.
    pub tombstone_lamports: u64,
    /// Frozen neutral sink receiving every unsolicited lamport.
    pub neutral_sink: Identity32V1,
    /// Exact surplus transferred to `neutral_sink`.
    pub neutral_lamports: u64,
}

fn rent_disposition(
    rent: RentSplitV2,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<RentDispositionV2, RetirementErrorV1> {
    rent.validate()?;
    if rent.payer == neutral_sink {
        return Err(RetirementErrorV1::PayerIsNeutralSink);
    }
    let principal = rent
        .refundable_live_principal
        .checked_add(rent.permanent_tombstone_principal)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    let floor = principal
        .checked_add(rent.donation_floor)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    if actual_balance < floor {
        return Err(RetirementErrorV1::AccountBalanceShortfall);
    }
    let neutral_lamports = actual_balance
        .checked_sub(principal)
        .ok_or(RetirementErrorV1::AccountBalanceShortfall)?;
    Ok(RentDispositionV2 {
        payer: rent.payer,
        payer_refund_lamports: rent.refundable_live_principal,
        tombstone_lamports: rent.permanent_tombstone_principal,
        neutral_sink,
        neutral_lamports,
    })
}

/// Local economic compartments that must be zero before Position retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionEconomicStateV1 {
    /// Venue-held trading cash.
    pub cash_atoms: u64,
    /// Encumbered trading cash.
    pub reserved_cash_atoms: u64,
    /// Internal Egg balances at the fixed maximum width.
    pub internal_atoms: [u64; MAX_OUTCOMES],
}

impl PositionEconomicStateV1 {
    /// Canonical zero economic state.
    pub const ZERO: Self = Self {
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        internal_atoms: [0; MAX_OUTCOMES],
    };

    /// Whether all local economic compartments are exactly zero.
    pub fn is_zero(self) -> bool {
        self.cash_atoms == 0
            && self.reserved_cash_atoms == 0
            && self.internal_atoms.iter().all(|amount| *amount == 0)
    }
}

/// Authenticated live Position V2 projection used by pure transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePositionV2 {
    /// Market identity authenticated from the base Position body.
    pub market: Identity32V1,
    /// Owner identity authenticated from the base Position body.
    pub owner: Identity32V1,
    /// Current nonzero Position generation.
    pub generation: u64,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Count and funding state owned by this crate.
    pub retirement: PositionRetirementTailV1,
}

impl LivePositionV2 {
    fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.generation == 0 {
            return Err(RetirementErrorV1::WrongGeneration);
        }
        self.retirement.validate()
    }
}

/// Live-or-tombstone Position state; absence is deliberately unrepresentable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionLifecycleStateV2 {
    /// Full live Position V2.
    Live(LivePositionV2),
    /// Compact permanent Position tombstone.
    Tombstone(PositionTombstoneV1),
}

/// Close a locally and aggregately empty Position into its permanent tombstone.
///
/// The function constructs the complete post-state and disposition before
/// returning. Passing the returned tombstone again refuses as a replay.
pub fn close_position(
    state: PositionLifecycleStateV2,
    economic: PositionEconomicStateV1,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<(PositionLifecycleStateV2, RentDispositionV2), RetirementErrorV1> {
    let live = match state {
        PositionLifecycleStateV2::Live(live) => live,
        PositionLifecycleStateV2::Tombstone(_) => return Err(RetirementErrorV1::AlreadyTerminal),
    };
    live.validate()?;
    if !economic.is_zero() {
        return Err(RetirementErrorV1::EconomicBalanceOutstanding);
    }
    if live.retirement.outstanding_reservations != 0 {
        return Err(RetirementErrorV1::ReservationOutstanding);
    }
    let disposition = rent_disposition(live.retirement.rent, actual_balance, neutral_sink)?;
    let tombstone = PositionTombstoneV1 {
        market: live.market,
        owner: live.owner,
        generation: live.generation,
        stored_bump: live.stored_bump,
    };
    tombstone.validate()?;
    Ok((PositionLifecycleStateV2::Tombstone(tombstone), disposition))
}

/// Reopen the next Position generation at the same permanent identity.
///
/// The adapter independently proves the retained tombstone minimum and exact
/// payer transfer before supplying `new_rent`. A live Position cannot reopen.
pub fn reopen_position(
    state: PositionLifecycleStateV2,
    new_rent: RentSplitV2,
    neutral_sink: Identity32V1,
) -> Result<PositionLifecycleStateV2, RetirementErrorV1> {
    let tombstone = match state {
        PositionLifecycleStateV2::Tombstone(tombstone) => tombstone,
        PositionLifecycleStateV2::Live(_) => return Err(RetirementErrorV1::WrongPhase),
    };
    tombstone.validate()?;
    new_rent.validate()?;
    if new_rent.payer == neutral_sink {
        return Err(RetirementErrorV1::PayerIsNeutralSink);
    }
    let generation = tombstone
        .generation
        .checked_add(1)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    Ok(PositionLifecycleStateV2::Live(LivePositionV2 {
        market: tombstone.market,
        owner: tombstone.owner,
        generation,
        stored_bump: tombstone.stored_bump,
        retirement: PositionRetirementTailV1 {
            outstanding_reservations: 0,
            rent: new_rent,
        },
    }))
}

/// Authenticated live general Epoch V3 projection used by pure transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveEpochV3 {
    /// Market identity authenticated from the base Epoch body.
    pub market: Identity32V1,
    /// Canonical Epoch identity authenticated from the base Epoch body.
    pub epoch: Identity32V1,
    /// Monotone Market-owned index.
    pub epoch_index: u64,
    /// Current Epoch lifecycle phase.
    pub phase: GeneralEpochPhaseV1,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Generation, counts, and rent state owned by this crate.
    pub retirement: EpochRetirementTailV1,
}

impl LiveEpochV3 {
    fn validate(self) -> Result<(), RetirementErrorV1> {
        self.retirement.validate()
    }
}

/// Live-or-tombstone general Epoch state; absence is deliberately unrepresentable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpochLifecycleStateV3 {
    /// Full live general Epoch V3.
    Live(LiveEpochV3),
    /// Compact permanent general Epoch tombstone.
    Tombstone(GeneralEpochTombstoneV1),
}

/// Consume the exact Market cursor and construct a fresh OPEN general Epoch.
///
/// The adapter creates the complete root bundle and writes the returned cursor
/// and Epoch in one transaction. Index `u64::MAX` is never admitted because no
/// strictly greater cursor exists.
pub fn open_general_epoch(
    cursor: MarketEpochCursorV1,
    requested_index: u64,
    market: Identity32V1,
    epoch: Identity32V1,
    stored_bump: u8,
    rent: RentSplitV2,
) -> Result<(MarketEpochCursorV1, LiveEpochV3), RetirementErrorV1> {
    if requested_index != cursor.next_general_epoch_index {
        return Err(RetirementErrorV1::NonmonotoneEpoch);
    }
    if requested_index == u64::MAX {
        return Err(RetirementErrorV1::EpochIndexExhausted);
    }
    rent.validate()?;
    let next_index = requested_index
        .checked_add(1)
        .ok_or(RetirementErrorV1::EpochIndexExhausted)?;
    let live = LiveEpochV3 {
        market,
        epoch,
        epoch_index: requested_index,
        phase: GeneralEpochPhaseV1::Open,
        stored_bump,
        retirement: EpochRetirementTailV1 {
            epoch_generation: next_index,
            children: Default::default(),
            rent,
        },
    };
    live.validate()?;
    Ok((
        MarketEpochCursorV1 {
            next_general_epoch_index: next_index,
        },
        live,
    ))
}

/// Close a terminal child-free Epoch into its permanent identity tombstone.
pub fn close_epoch(
    state: EpochLifecycleStateV3,
    actual_balance: u64,
    neutral_sink: Identity32V1,
) -> Result<(EpochLifecycleStateV3, RentDispositionV2), RetirementErrorV1> {
    let live = match state {
        EpochLifecycleStateV3::Live(live) => live,
        EpochLifecycleStateV3::Tombstone(_) => return Err(RetirementErrorV1::AlreadyTerminal),
    };
    live.validate()?;
    if live.phase == GeneralEpochPhaseV1::Open {
        return Err(RetirementErrorV1::WrongPhase);
    }
    if !live.retirement.children.is_zero() {
        return Err(RetirementErrorV1::ChildOutstanding);
    }
    let disposition = rent_disposition(live.retirement.rent, actual_balance, neutral_sink)?;
    let tombstone = GeneralEpochTombstoneV1 {
        epoch: live.epoch,
        market: live.market,
        epoch_index: live.epoch_index,
        epoch_generation: live.retirement.epoch_generation,
        stored_bump: live.stored_bump,
    };
    tombstone.validate()?;
    Ok((EpochLifecycleStateV3::Tombstone(tombstone), disposition))
}

/// Counted reservation projection spanning its existing body and new tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedReservationV1 {
    /// Position generation already persisted by current reservation schemas.
    pub position_generation: u64,
    /// Semantic ownership state already persisted by current reservation schemas.
    pub state: ReservationStateV1,
    /// New epoch-generation and once-only count marker.
    pub count: ReservationCountTailV1,
}

impl CountedReservationV1 {
    fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.position_generation == 0 {
            return Err(RetirementErrorV1::WrongGeneration);
        }
        self.count.validate()?;
        if self.state.is_position_counted() != self.count.position_counted {
            return Err(RetirementErrorV1::NonCanonicalState);
        }
        Ok(())
    }
}

fn increment_position(mut position: LivePositionV2) -> Result<LivePositionV2, RetirementErrorV1> {
    position.validate()?;
    position.retirement.outstanding_reservations = position
        .retirement
        .outstanding_reservations
        .checked_add(1)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    Ok(position)
}

/// Register a direct reservation against Position's aggregate count.
///
/// The direct Epoch adapter separately authenticates and supplies its nonzero
/// generation; its own root accounting remains owned by that direct family.
pub fn register_direct_reservation(
    position: LivePositionV2,
    direct_epoch_generation: u64,
) -> Result<(LivePositionV2, CountedReservationV1), RetirementErrorV1> {
    if direct_epoch_generation == 0 {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    let next_position = increment_position(position)?;
    let reservation = CountedReservationV1 {
        position_generation: position.generation,
        state: ReservationStateV1::Active,
        count: ReservationCountTailV1 {
            epoch_generation: direct_epoch_generation,
            position_counted: true,
        },
    };
    reservation.validate()?;
    Ok((next_position, reservation))
}

/// Register a general reservation in both authoritative aggregates atomically.
pub fn register_general_reservation(
    position: LivePositionV2,
    epoch: LiveEpochV3,
) -> Result<
    (
        LivePositionV2,
        LiveEpochV3,
        CountedReservationV1,
        ChildSlotV1,
    ),
    RetirementErrorV1,
> {
    position.validate()?;
    epoch.validate()?;
    if epoch.phase != GeneralEpochPhaseV1::Open {
        return Err(RetirementErrorV1::WrongPhase);
    }
    let next_position_count = position
        .retirement
        .outstanding_reservations
        .checked_add(1)
        .ok_or(RetirementErrorV1::ArithmeticOverflow)?;
    let next_epoch_counts = epoch
        .retirement
        .children
        .checked_increment(EpochChildKindV1::ReservationArchive)?;

    let mut next_position = position;
    next_position.retirement.outstanding_reservations = next_position_count;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = next_epoch_counts;
    let reservation = CountedReservationV1 {
        position_generation: position.generation,
        state: ReservationStateV1::Active,
        count: ReservationCountTailV1 {
            epoch_generation: epoch.retirement.epoch_generation,
            position_counted: true,
        },
    };
    reservation.validate()?;
    let archive = ChildSlotV1::Present(AuthenticatedEpochChildV1 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind: EpochChildKindV1::ReservationArchive,
        candidate_status: None,
    });
    Ok((next_position, next_epoch, reservation, archive))
}

/// Move ACTIVE to ENTITLED without changing Position's count.
pub fn entitle_reservation(
    reservation: CountedReservationV1,
) -> Result<CountedReservationV1, RetirementErrorV1> {
    reservation.validate()?;
    if reservation.state != ReservationStateV1::Active {
        return Err(if reservation.state.is_terminal() {
            RetirementErrorV1::AlreadyTerminal
        } else {
            RetirementErrorV1::WrongPhase
        });
    }
    let mut next = reservation;
    next.state = ReservationStateV1::Entitled;
    next.validate()?;
    Ok(next)
}

/// Apply the first terminal reservation transition and decrement Position once.
///
/// RELEASED asset return and CONSUMED entitlement/payment equalities remain
/// adapter inputs owned by their existing economic codecs. The adapter must
/// calculate those post-states before encoding this returned accounting state.
pub fn terminate_reservation(
    position: LivePositionV2,
    reservation: CountedReservationV1,
    target: ReservationStateV1,
) -> Result<(LivePositionV2, CountedReservationV1), RetirementErrorV1> {
    position.validate()?;
    reservation.validate()?;
    if !target.is_terminal() {
        return Err(RetirementErrorV1::WrongPhase);
    }
    if reservation.state.is_terminal() {
        return Err(RetirementErrorV1::AlreadyTerminal);
    }
    if reservation.position_generation != position.generation {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    let next_count = position
        .retirement
        .outstanding_reservations
        .checked_sub(1)
        .ok_or(RetirementErrorV1::CounterUnderflow)?;
    let mut next_position = position;
    next_position.retirement.outstanding_reservations = next_count;
    let mut next_reservation = reservation;
    next_reservation.state = target;
    next_reservation.count.position_counted = false;
    next_reservation.validate()?;
    Ok((next_position, next_reservation))
}

/// One adapter-authenticated Epoch child registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedEpochChildV1 {
    /// Parent Epoch generation copied from the child's versioned bytes.
    pub epoch_generation: u64,
    /// Account family whose authoritative count owns this child.
    pub kind: EpochChildKindV1,
    /// Version-qualified candidate state, present only for CandidateBundle.
    pub candidate_status: Option<CandidateStatusWitnessV1>,
}

impl AuthenticatedEpochChildV1 {
    fn validate(self) -> Result<(), RetirementErrorV1> {
        if self.epoch_generation == 0 {
            return Err(RetirementErrorV1::WrongGeneration);
        }
        let candidate = self.kind == EpochChildKindV1::CandidateBundle;
        if candidate != self.candidate_status.is_some() {
            return Err(RetirementErrorV1::WrongChildKind);
        }
        Ok(())
    }
}

/// Presence of one canonical child account after adapter PDA authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildSlotV1 {
    /// System-owned zero-data canonical PDA target.
    Absent,
    /// Present, versioned, program-owned authenticated child.
    Present(AuthenticatedEpochChildV1),
}

fn require_open_epoch(epoch: LiveEpochV3) -> Result<(), RetirementErrorV1> {
    epoch.validate()?;
    if epoch.phase != GeneralEpochPhaseV1::Open {
        Err(RetirementErrorV1::WrongPhase)
    } else {
        Ok(())
    }
}

fn require_terminal_epoch(epoch: LiveEpochV3) -> Result<(), RetirementErrorV1> {
    epoch.validate()?;
    if epoch.phase == GeneralEpochPhaseV1::Open {
        Err(RetirementErrorV1::WrongPhase)
    } else {
        Ok(())
    }
}

/// Create one generic Epoch child and increment its typed count once.
///
/// Candidate bundles use [`create_registered_candidate_after_validation`]. Reservation
/// archives must use [`register_general_reservation`] so their Position and
/// Epoch counts cannot diverge.
pub fn create_epoch_child(
    epoch: LiveEpochV3,
    slot: ChildSlotV1,
    kind: EpochChildKindV1,
) -> Result<(LiveEpochV3, ChildSlotV1), RetirementErrorV1> {
    require_open_epoch(epoch)?;
    if matches!(
        kind,
        EpochChildKindV1::CandidateBundle | EpochChildKindV1::ReservationArchive
    ) {
        return Err(RetirementErrorV1::WrongChildKind);
    }
    if slot != ChildSlotV1::Absent {
        return Err(RetirementErrorV1::ChildAlreadyPresent);
    }
    let counts = epoch.retirement.children.checked_increment(kind)?;
    let child = AuthenticatedEpochChildV1 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind,
        candidate_status: None,
    };
    child.validate()?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Present(child)))
}

/// Create one candidate bundle after its lifecycle owner validates its state.
pub fn create_registered_candidate_after_validation(
    epoch: LiveEpochV3,
    slot: ChildSlotV1,
    status: CandidateStatusWitnessV1,
) -> Result<(LiveEpochV3, ChildSlotV1), RetirementErrorV1> {
    require_open_epoch(epoch)?;
    if slot != ChildSlotV1::Absent {
        return Err(RetirementErrorV1::ChildAlreadyPresent);
    }
    let counts = epoch
        .retirement
        .children
        .checked_increment(EpochChildKindV1::CandidateBundle)?;
    let child = AuthenticatedEpochChildV1 {
        epoch_generation: epoch.retirement.epoch_generation,
        kind: EpochChildKindV1::CandidateBundle,
        candidate_status: Some(status),
    };
    child.validate()?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Present(child)))
}

/// Record a lifecycle-validated candidate status without changing its count.
///
/// This crate deliberately does not duplicate the current or ADR-0006
/// candidate state graph. The owning lifecycle adapter first validates that
/// transition, then supplies its version-qualified post-state here. The child
/// must already be a registered candidate in the same occupied slot.
pub fn update_registered_candidate_status_after_validation(
    slot: ChildSlotV1,
    status: CandidateStatusWitnessV1,
) -> Result<ChildSlotV1, RetirementErrorV1> {
    let mut child = match slot {
        ChildSlotV1::Present(child) => child,
        ChildSlotV1::Absent => return Err(RetirementErrorV1::ChildAbsent),
    };
    child.validate()?;
    if child.kind != EpochChildKindV1::CandidateBundle {
        return Err(RetirementErrorV1::WrongChildKind);
    }
    let prior = child
        .candidate_status
        .ok_or(RetirementErrorV1::WrongChildKind)?;
    if status.schema_tag() != prior.schema_tag() {
        return Err(RetirementErrorV1::WrongTag);
    }
    if status.schema_version() != prior.schema_version() {
        return Err(RetirementErrorV1::WrongVersion);
    }
    child.candidate_status = Some(status);
    child.validate()?;
    Ok(ChildSlotV1::Present(child))
}

fn authenticated_present(
    epoch: LiveEpochV3,
    slot: ChildSlotV1,
) -> Result<AuthenticatedEpochChildV1, RetirementErrorV1> {
    let child = match slot {
        ChildSlotV1::Absent => return Err(RetirementErrorV1::ChildAbsent),
        ChildSlotV1::Present(child) => child,
    };
    child.validate()?;
    if child.epoch_generation != epoch.retirement.epoch_generation {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    Ok(child)
}

/// Close one generic child and decrement its typed count exactly once.
///
/// The adapter first validates the account family's economic close condition.
/// Candidate bundles and reservation archives are deliberately refused here;
/// their specialized functions enforce additional dependencies.
pub fn close_epoch_child(
    epoch: LiveEpochV3,
    slot: ChildSlotV1,
) -> Result<(LiveEpochV3, ChildSlotV1), RetirementErrorV1> {
    require_terminal_epoch(epoch)?;
    let child = authenticated_present(epoch, slot)?;
    if matches!(
        child.kind,
        EpochChildKindV1::CandidateBundle | EpochChildKindV1::ReservationArchive
    ) {
        return Err(RetirementErrorV1::WrongChildKind);
    }
    let counts = epoch.retirement.children.checked_decrement(child.kind)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Absent))
}

/// Close one candidate bundle in any status after its canonical ClearWork is absent.
pub fn close_registered_candidate(
    epoch: LiveEpochV3,
    slot: ChildSlotV1,
    canonical_clear_work_present: bool,
) -> Result<(LiveEpochV3, ChildSlotV1), RetirementErrorV1> {
    require_terminal_epoch(epoch)?;
    let child = authenticated_present(epoch, slot)?;
    if child.kind != EpochChildKindV1::CandidateBundle {
        return Err(RetirementErrorV1::WrongChildKind);
    }
    if canonical_clear_work_present {
        return Err(RetirementErrorV1::ClearWorkOutstanding);
    }
    let counts = epoch
        .retirement
        .children
        .checked_decrement(EpochChildKindV1::CandidateBundle)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Absent))
}

/// Close a terminal, economically uncounted general reservation archive.
///
/// This decrements only the Epoch archive count. The Position count was
/// already decremented by [`terminate_reservation`].
pub fn close_general_reservation_archive(
    epoch: LiveEpochV3,
    slot: ChildSlotV1,
    reservation: CountedReservationV1,
) -> Result<(LiveEpochV3, ChildSlotV1), RetirementErrorV1> {
    reservation.validate()?;
    if !reservation.state.is_terminal() || reservation.count.position_counted {
        return Err(RetirementErrorV1::ReservationOutstanding);
    }
    if reservation.count.epoch_generation != epoch.retirement.epoch_generation {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    let child = match slot {
        ChildSlotV1::Present(child) if child.kind == EpochChildKindV1::ReservationArchive => child,
        ChildSlotV1::Present(_) => return Err(RetirementErrorV1::WrongChildKind),
        ChildSlotV1::Absent => return Err(RetirementErrorV1::ChildAbsent),
    };
    if child.epoch_generation != reservation.count.epoch_generation {
        return Err(RetirementErrorV1::WrongGeneration);
    }
    require_terminal_epoch(epoch)?;
    child.validate()?;
    let counts = epoch
        .retirement
        .children
        .checked_decrement(EpochChildKindV1::ReservationArchive)?;
    let mut next_epoch = epoch;
    next_epoch.retirement.children = counts;
    Ok((next_epoch, ChildSlotV1::Absent))
}
