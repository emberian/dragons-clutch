//! Atomic transfer plan over two authenticated base Position projections.

use clutch_structured_claim::MarketPhase;

use crate::{Amount, Error, Result, MAX_OUTCOMES};

/// Explicit phase policy for a structured-claim custody transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssetTransferPhasePolicyV1 {
    /// Admit only before base resolution.
    ActiveOnly = 0,
    /// Admit either before or after resolution because the move is supply and
    /// Hoard neutral.
    ActiveOrResolved = 1,
}

/// Authenticated semantic projection of one base Position and its Replay.
///
/// This is not a persisted DTO. The SBF adapter reconstructs it from the
/// authoritative Position and current-generation Replay accounts and writes
/// those accounts only after every local/CPI precondition has passed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PositionProjectionV1 {
    /// Canonical Market account.
    pub market: [u8; 32],
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Current generation selected by the Position tombstone/reopen policy.
    pub generation: u64,
    /// Current-generation mutation sequence.
    pub replay_sequence: u64,
    /// Total free plus reserved Realm-collateral cash.
    pub cash_atoms: Amount,
    /// Encumbered cash that this transfer may not spend.
    pub reserved_cash_atoms: Amount,
    /// Free native Eggs; seller reservations have already left this vector.
    pub internal: [Amount; MAX_OUTCOMES],
    /// Closed Positions cannot participate.
    pub closed: bool,
}

impl PositionProjectionV1 {
    pub(crate) fn validate(&self, outcome_count: u8) -> Result<()> {
        let width = usize::from(outcome_count);
        if !(2..=MAX_OUTCOMES).contains(&width) {
            return Err(Error::InvalidPosition);
        }
        if self.market == [0; 32] || self.owner == [0; 32] || self.closed {
            return Err(Error::InvalidPosition);
        }
        if self.reserved_cash_atoms > self.cash_atoms {
            return Err(Error::InvalidPosition);
        }
        let mut index = width;
        while index < MAX_OUTCOMES {
            if self.internal[index] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }

    /// Unencumbered Realm-collateral cash available to a custody move.
    pub fn free_cash_atoms(&self) -> Result<Amount> {
        self.cash_atoms
            .checked_sub(self.reserved_cash_atoms)
            .ok_or(Error::InvalidPosition)
    }
}

/// Exact expected identities and asset quantities for one atomic transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AtomicPositionAssetTransferRequestV1 {
    /// Shared canonical Market.
    pub market: [u8; 32],
    /// Expected semantic source owner.
    pub source_owner: [u8; 32],
    /// Expected semantic destination owner.
    pub destination_owner: [u8; 32],
    /// Exact source generation.
    pub source_generation: u64,
    /// Exact destination generation.
    pub destination_generation: u64,
    /// Exact source Replay sequence before execution.
    pub source_replay_sequence: u64,
    /// Exact destination Replay sequence before execution.
    pub destination_replay_sequence: u64,
    /// Free cash to move.
    pub cash_atoms: Amount,
    /// Free native Eggs to move, canonically padded to the Market width.
    pub internal: [Amount; MAX_OUTCOMES],
    /// Explicit Active-only or Active-or-Resolved policy.
    pub phase_policy: AssetTransferPhasePolicyV1,
}

/// Fully staged post-state and supply-neutral execution delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AtomicPositionAssetTransferResultV1 {
    /// Prospective source Position/Replay projection.
    pub source: PositionProjectionV1,
    /// Prospective destination Position/Replay projection.
    pub destination: PositionProjectionV1,
    /// Exact cash delta the adapter must observe.
    pub cash_atoms: Amount,
    /// Exact native-Egg deltas the adapter must observe.
    pub internal: [Amount; MAX_OUTCOMES],
}

/// Stage a supply-neutral, Hoard-neutral Position asset transfer.
///
/// All arithmetic and conservation checks complete on copies. The function
/// cannot partially mutate either input; an SBF adapter must preserve that
/// property by authenticating first, applying the returned exact deltas, and
/// checking the two current-generation Replay writes atomically.
pub fn prepare_atomic_position_asset_transfer_v1(
    outcome_count: u8,
    market_phase: MarketPhase,
    source: PositionProjectionV1,
    destination: PositionProjectionV1,
    request: AtomicPositionAssetTransferRequestV1,
) -> Result<AtomicPositionAssetTransferResultV1> {
    source.validate(outcome_count)?;
    destination.validate(outcome_count)?;
    if request.market == [0; 32]
        || request.source_owner == [0; 32]
        || request.destination_owner == [0; 32]
        || request.source_owner == request.destination_owner
    {
        return Err(Error::InvalidIdentity);
    }
    if source.market != request.market
        || destination.market != request.market
        || source.owner != request.source_owner
        || destination.owner != request.destination_owner
        || source.generation != request.source_generation
        || destination.generation != request.destination_generation
        || source.replay_sequence != request.source_replay_sequence
        || destination.replay_sequence != request.destination_replay_sequence
    {
        return Err(Error::DifferentPositionDomain);
    }
    match (request.phase_policy, market_phase) {
        (AssetTransferPhasePolicyV1::ActiveOnly, MarketPhase::Active)
        | (AssetTransferPhasePolicyV1::ActiveOrResolved, MarketPhase::Active)
        | (AssetTransferPhasePolicyV1::ActiveOrResolved, MarketPhase::Resolved) => {}
        (AssetTransferPhasePolicyV1::ActiveOnly, MarketPhase::Resolved) => {
            return Err(Error::InvalidPhase);
        }
    }
    let width = usize::from(outcome_count);
    let mut any = request.cash_atoms != 0;
    let mut index = 0_usize;
    while index < MAX_OUTCOMES {
        if index < width {
            any |= request.internal[index] != 0;
        } else if request.internal[index] != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    if !any {
        return Err(Error::ZeroQuantity);
    }
    if source.free_cash_atoms()? < request.cash_atoms {
        return Err(Error::InsufficientFreeAssets);
    }

    let before_cash = u128::from(source.cash_atoms)
        .checked_add(u128::from(destination.cash_atoms))
        .ok_or(Error::ArithmeticOverflow)?;
    let mut next_source = source;
    let mut next_destination = destination;
    next_source.cash_atoms = next_source
        .cash_atoms
        .checked_sub(request.cash_atoms)
        .ok_or(Error::ArithmeticUnderflow)?;
    next_destination.cash_atoms = next_destination
        .cash_atoms
        .checked_add(request.cash_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    let after_cash = u128::from(next_source.cash_atoms)
        .checked_add(u128::from(next_destination.cash_atoms))
        .ok_or(Error::ArithmeticOverflow)?;
    if before_cash != after_cash {
        return Err(Error::InvariantViolation);
    }

    index = 0;
    while index < width {
        let quantity = request.internal[index];
        if next_source.internal[index] < quantity {
            return Err(Error::InsufficientFreeAssets);
        }
        let before = u128::from(next_source.internal[index])
            .checked_add(u128::from(next_destination.internal[index]))
            .ok_or(Error::ArithmeticOverflow)?;
        next_source.internal[index] = next_source.internal[index]
            .checked_sub(quantity)
            .ok_or(Error::ArithmeticUnderflow)?;
        next_destination.internal[index] = next_destination.internal[index]
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        let after = u128::from(next_source.internal[index])
            .checked_add(u128::from(next_destination.internal[index]))
            .ok_or(Error::ArithmeticOverflow)?;
        if before != after {
            return Err(Error::InvariantViolation);
        }
        index += 1;
    }
    next_source.replay_sequence = next_source
        .replay_sequence
        .checked_add(1)
        .ok_or(Error::ReplayExhausted)?;
    next_destination.replay_sequence = next_destination
        .replay_sequence
        .checked_add(1)
        .ok_or(Error::ReplayExhausted)?;
    next_source.validate(outcome_count)?;
    next_destination.validate(outcome_count)?;
    Ok(AtomicPositionAssetTransferResultV1 {
        source: next_source,
        destination: next_destination,
        cash_atoms: request.cash_atoms,
        internal: request.internal,
    })
}
