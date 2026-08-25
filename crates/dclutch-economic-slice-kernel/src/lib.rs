#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Runtime-width economic state with dynamic per-holder Position accounts.
//!
//! This is a physical refinement of the shared exact Economic semantics. One
//! Market byte owner persists aggregate facts; each Position byte owner
//! persists one holder's claims. The executor validates complete pre-state and
//! candidate arithmetic before applying infallible field writes.

/// Market header bytes before three runtime-width `u64` vectors.
pub const MARKET_HEADER_BYTES: usize = 144;
/// Position header bytes before two runtime-width `u64` vectors.
pub const POSITION_HEADER_BYTES: usize = 96;
/// Bytes in one exact claim scalar.
pub const SCALAR_BYTES: usize = 8;
/// Canonical Market magic.
pub const MARKET_MAGIC: [u8; 8] = *b"DCLTEMK2";
/// Canonical Position magic.
pub const POSITION_MAGIC: [u8; 8] = *b"DCLTEPS2";
/// Implemented physical schema.
pub const SCHEMA_VERSION: u16 = 2;

const PHASE_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const OUTCOME_COUNT_OFFSET: usize = 16;
const WINNER_OFFSET: usize = 20;
const REVISION_OFFSET: usize = 24;
const HOARD_OFFSET: usize = 32;
const MARKET_RESERVED_OFFSET: usize = 40;
const MARKET_ID_OFFSET: usize = 48;
const RELEASE_SET_ID_OFFSET: usize = 80;
const REGISTRY_PROGRAM_OFFSET: usize = 112;

const POSITION_MARKET_ID_OFFSET: usize = 16;
const POSITION_OWNER_OFFSET: usize = 48;
const POSITION_REVISION_OFFSET: usize = 80;
const POSITION_RESERVED_OFFSET: usize = 88;

/// Stable hostile-decoding or economic refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Account bytes did not have the exact runtime-derived width.
    InvalidLength,
    /// Magic selected another account family.
    InvalidMagic,
    /// The account schema version is unsupported.
    UnsupportedVersion,
    /// Padding, phase, winner, or Boolean bytes were noncanonical.
    NonCanonical,
    /// A required identity was zero or two holders aliased.
    InvalidIdentity,
    /// The runtime outcome width was zero or could not fit address arithmetic.
    InvalidOutcomeCount,
    /// Market and Position identities or widths did not join exactly.
    AccountMismatch,
    /// An optimistic revision coordinate was stale or future.
    RevisionMismatch,
    /// The selected outcome was outside the Product-owned width.
    InvalidOutcome,
    /// The command was inadmissible in the current Market phase.
    InvalidPhase,
    /// A zero-value move was requested.
    ZeroQuantity,
    /// A claim or Hoard debit exceeded its exact balance.
    InsufficientBalance,
    /// Checked scalar or revision arithmetic overflowed.
    ArithmeticOverflow,
    /// Aggregate native/materialized supply did not partition total supply.
    SupplyPartitionMismatch,
    /// The two projected Positions exceeded aggregate representation supply.
    PositionProjectionExceedsSupply,
    /// Hoard principal did not cover all open outcomes or the terminal winner.
    Insolvent,
    /// A staged post-state failed complete invariant validation.
    CandidateInvariantFailure,
}

/// Result alias for this kernel.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact Market lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// Open complete-set and representation operations.
    Open,
    /// One Product outcome has resolved.
    Terminal(u32),
    /// Terminal redemption and closure are underway.
    Retiring(u32),
    /// Every aggregate liability and Hoard atom is gone.
    Retired,
}

/// Runtime-width dynamic-holder command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    /// Move native claims between two dynamic holder Positions.
    TransferNative {
        /// Product-owned outcome selector.
        outcome: u32,
        /// Exact claim atoms.
        quantity: u64,
    },
    /// Convert source-native claims into destination materialized claims.
    Materialize {
        /// Product-owned outcome selector.
        outcome: u32,
        /// Exact claim atoms.
        quantity: u64,
    },
    /// Convert source materialized claims into destination-native claims.
    Dematerialize {
        /// Product-owned outcome selector.
        outcome: u32,
        /// Exact claim atoms.
        quantity: u64,
    },
    /// Burn source-native terminal claims and derive an exact payout.
    RedeemTerminal {
        /// Product-owned outcome selector.
        outcome: u32,
        /// Exact claim atoms.
        quantity: u64,
    },
}

/// Optimistic transition coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame {
    /// Exact Market revision.
    pub expected_market_revision: u64,
    /// Exact source Position revision.
    pub expected_source_revision: u64,
    /// Exact destination Position revision.
    pub expected_destination_revision: u64,
    /// Semantic command.
    pub command: Command,
}

/// Exact collateral payout derived by terminal redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Payout {
    /// Position owner receiving collateral.
    pub recipient: [u8; 32],
    /// Exact collateral atoms; zero for a losing claim.
    pub amount: u64,
}

/// One runtime-width basket transition style.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasketAction {
    /// Move native claims between two Positions.
    TransferNative,
    /// Convert source native claims into destination materialized claims.
    Materialize,
    /// Convert source materialized claims into destination native claims.
    Dematerialize,
    /// Burn source native terminal claims.
    RedeemNativeTerminal,
    /// Burn source materialized terminal claims.
    RedeemMaterializedTerminal,
    /// Mint coordinate-equal complete sets into one destination Position.
    MintCompleteSet,
    /// Merge coordinate-equal complete sets from one source Position.
    MergeCompleteSet,
}

/// Optimistic coordinates for one borrowed runtime-width basket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasketFrame<'a> {
    /// Exact Market revision.
    pub expected_market_revision: u64,
    /// Exact source Position revision, absent only for complete-set mint.
    pub expected_source_revision: Option<u64>,
    /// Exact destination Position revision, absent for terminal redemption or merge.
    pub expected_destination_revision: Option<u64>,
    /// Semantic basket style.
    pub action: BasketAction,
    /// Exact little-endian `u64[outcome_count]` claim vector.
    pub quantities: &'a [u8],
    /// Checked multiplier applied to every encoded quantity.
    pub quantity_multiplier: u64,
}

#[derive(Clone, Copy)]
struct BasketCandidate {
    supply: u64,
    native: u64,
    materialized: u64,
    source_native: u64,
    source_materialized: u64,
    destination_native: u64,
    destination_materialized: u64,
}

#[derive(Clone, Copy)]
struct MarketMeta {
    outcome_count: u32,
    phase: Phase,
    revision: u64,
    hoard: u64,
    market_id: [u8; 32],
    release_set_id: [u8; 32],
    registry_program: [u8; 32],
}

#[derive(Clone, Copy)]
struct PositionMeta {
    market_id: [u8; 32],
    owner: [u8; 32],
    revision: u64,
}

/// Initialize one canonical aggregate Market account into zeroed exact-width bytes.
pub fn initialize_market(
    output: &mut [u8],
    market_id: [u8; 32],
    release_set_id: [u8; 32],
    registry_program: [u8; 32],
    outcome_count: u32,
    phase: Phase,
    hoard: u64,
) -> Result<()> {
    require_nonzero(market_id)?;
    require_nonzero(release_set_id)?;
    require_nonzero(registry_program)?;
    exact_market_width(output, outcome_count)?;
    if output.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonical);
    }
    validate_phase(outcome_count, phase)?;
    put(output, 0, &MARKET_MAGIC)?;
    put(output, 8, &SCHEMA_VERSION.to_le_bytes())?;
    encode_phase(output, phase)?;
    put(output, OUTCOME_COUNT_OFFSET, &outcome_count.to_le_bytes())?;
    put(output, HOARD_OFFSET, &hoard.to_le_bytes())?;
    put(output, MARKET_ID_OFFSET, &market_id)?;
    put(output, RELEASE_SET_ID_OFFSET, &release_set_id)?;
    put(output, REGISTRY_PROGRAM_OFFSET, &registry_program)?;
    validate_market(output)
}

/// Initialize one empty canonical Position into zeroed exact-width bytes.
pub fn initialize_position(
    output: &mut [u8],
    market_id: [u8; 32],
    owner: [u8; 32],
    outcome_count: u32,
) -> Result<()> {
    require_nonzero(market_id)?;
    require_nonzero(owner)?;
    exact_position_width(output, outcome_count)?;
    if output.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonical);
    }
    put(output, 0, &POSITION_MAGIC)?;
    put(output, 8, &SCHEMA_VERSION.to_le_bytes())?;
    put(output, POSITION_MARKET_ID_OFFSET, &market_id)?;
    put(output, POSITION_OWNER_OFFSET, &owner)?;
    validate_position(output, outcome_count).map(|_| ())
}

/// Validate and execute one dynamic-holder transition in place.
///
/// Every refusal before mutation leaves all three slices byte-for-byte intact.
/// The final writes are infallible because all offsets, debits, credits,
/// revisions, phase invariants, and the full candidate invariant are checked
/// first. A Solana adapter must still rely on transaction rollback for a later
/// CPI refusal.
pub fn execute(
    market: &mut [u8],
    source: &mut [u8],
    destination: &mut [u8],
    frame: Frame,
) -> Result<Payout> {
    let market_meta = decode_market(market)?;
    let source_meta = validate_position(source, market_meta.outcome_count)?;
    let destination_meta = validate_position(destination, market_meta.outcome_count)?;
    if source_meta.market_id != market_meta.market_id
        || destination_meta.market_id != market_meta.market_id
        || source_meta.owner == destination_meta.owner
    {
        return Err(Error::AccountMismatch);
    }
    validate_joined(market, source, destination, market_meta.outcome_count)?;
    if frame.expected_market_revision != market_meta.revision
        || frame.expected_source_revision != source_meta.revision
        || frame.expected_destination_revision != destination_meta.revision
    {
        return Err(Error::RevisionMismatch);
    }
    let (outcome, quantity) = command_coordinates(frame.command);
    if outcome >= market_meta.outcome_count {
        return Err(Error::InvalidOutcome);
    }
    if quantity == 0 {
        return Err(Error::ZeroQuantity);
    }
    admit_phase(market_meta.phase, frame.command)?;
    let next_market_revision = market_meta
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let next_source_revision = source_meta
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let next_destination_revision = destination_meta
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let index = usize::try_from(outcome).map_err(|_| Error::InvalidOutcome)?;
    let offsets = vector_offsets(market_meta.outcome_count, index)?;
    let source_offsets = position_offsets(market_meta.outcome_count, index)?;
    let destination_offsets = position_offsets(market_meta.outcome_count, index)?;

    let mut supply = u64_at(market, offsets.supply)?;
    let mut native = u64_at(market, offsets.native)?;
    let mut materialized = u64_at(market, offsets.materialized)?;
    let mut source_native = u64_at(source, source_offsets.native)?;
    let mut source_materialized = u64_at(source, source_offsets.materialized)?;
    let mut destination_native = u64_at(destination, destination_offsets.native)?;
    let mut destination_materialized = u64_at(destination, destination_offsets.materialized)?;
    let mut hoard = market_meta.hoard;
    let payout = match frame.command {
        Command::TransferNative { .. } => {
            source_native = debit(source_native, quantity)?;
            destination_native = credit(destination_native, quantity)?;
            0
        }
        Command::Materialize { .. } => {
            native = debit(native, quantity)?;
            source_native = debit(source_native, quantity)?;
            materialized = credit(materialized, quantity)?;
            destination_materialized = credit(destination_materialized, quantity)?;
            0
        }
        Command::Dematerialize { .. } => {
            materialized = debit(materialized, quantity)?;
            source_materialized = debit(source_materialized, quantity)?;
            native = credit(native, quantity)?;
            destination_native = credit(destination_native, quantity)?;
            0
        }
        Command::RedeemTerminal { .. } => {
            supply = debit(supply, quantity)?;
            native = debit(native, quantity)?;
            source_native = debit(source_native, quantity)?;
            let payout = if terminal_winner(market_meta.phase) == Some(outcome) {
                quantity
            } else {
                0
            };
            hoard = debit(hoard, payout)?;
            payout
        }
    };
    if supply
        != native
            .checked_add(materialized)
            .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::CandidateInvariantFailure);
    }
    if source_native
        .checked_add(destination_native)
        .ok_or(Error::ArithmeticOverflow)?
        > native
        || source_materialized
            .checked_add(destination_materialized)
            .ok_or(Error::ArithmeticOverflow)?
            > materialized
    {
        return Err(Error::CandidateInvariantFailure);
    }
    validate_candidate_solvency(market, market_meta, outcome, supply, hoard)?;

    put_u64(market, offsets.supply, supply);
    put_u64(market, offsets.native, native);
    put_u64(market, offsets.materialized, materialized);
    put_u64(market, REVISION_OFFSET, next_market_revision);
    put_u64(market, HOARD_OFFSET, hoard);
    put_u64(source, source_offsets.native, source_native);
    put_u64(source, source_offsets.materialized, source_materialized);
    put_u64(source, POSITION_REVISION_OFFSET, next_source_revision);
    put_u64(destination, destination_offsets.native, destination_native);
    put_u64(
        destination,
        destination_offsets.materialized,
        destination_materialized,
    );
    put_u64(
        destination,
        POSITION_REVISION_OFFSET,
        next_destination_revision,
    );
    Ok(Payout {
        recipient: source_meta.owner,
        amount: payout,
    })
}

/// Execute one complete runtime-width basket and advance each participating
/// account revision exactly once.
///
/// The full basket, candidate Hoard, every outcome balance, and all optimistic
/// coordinates are preflighted before the first write. Complete-set actions
/// require a positive coordinate-equal vector. Other actions allow sparse
/// vectors but require at least one positive coordinate.
pub fn execute_basket(
    market: &mut [u8],
    mut source: Option<&mut [u8]>,
    mut destination: Option<&mut [u8]>,
    frame: BasketFrame<'_>,
) -> Result<Payout> {
    let market_meta = decode_market(market)?;
    if frame.quantities.len() != vector_bytes(market_meta.outcome_count)? {
        return Err(Error::InvalidLength);
    }
    validate_basket_shape(frame.action, source.is_some(), destination.is_some())?;
    validate_basket_quantities(frame, market_meta.outcome_count)?;
    let source_meta = source
        .as_deref()
        .map(|bytes| validate_position(bytes, market_meta.outcome_count))
        .transpose()?;
    let destination_meta = destination
        .as_deref()
        .map(|bytes| validate_position(bytes, market_meta.outcome_count))
        .transpose()?;
    for position in [source_meta, destination_meta].into_iter().flatten() {
        if position.market_id != market_meta.market_id {
            return Err(Error::AccountMismatch);
        }
    }
    if matches!((source_meta, destination_meta), (Some(left), Some(right)) if left.owner == right.owner)
    {
        return Err(Error::AccountMismatch);
    }
    if frame.expected_market_revision != market_meta.revision
        || frame.expected_source_revision != source_meta.map(|value| value.revision)
        || frame.expected_destination_revision != destination_meta.map(|value| value.revision)
    {
        return Err(Error::RevisionMismatch);
    }
    admit_basket_phase(market_meta.phase, frame.action)?;
    validate_optional_positions(
        market,
        source.as_deref(),
        destination.as_deref(),
        market_meta.outcome_count,
    )?;
    let (candidate_hoard, payout) = basket_hoard_and_payout(market_meta, frame)?;
    let count =
        usize::try_from(market_meta.outcome_count).map_err(|_| Error::InvalidOutcomeCount)?;
    let mut index = 0_usize;
    while index < count {
        let quantity = basket_quantity(frame, index)?;
        if quantity != 0 {
            let candidate = basket_candidate(
                market,
                source.as_deref(),
                destination.as_deref(),
                market_meta.outcome_count,
                index,
                frame.action,
                quantity,
            )?;
            validate_basket_candidate(market_meta.phase, index, candidate_hoard, candidate)?;
        }
        index = index.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
    }
    let next_market_revision = market_meta
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let next_source_revision = source_meta
        .map(|value| {
            value
                .revision
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)
        })
        .transpose()?;
    let next_destination_revision = destination_meta
        .map(|value| {
            value
                .revision
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)
        })
        .transpose()?;

    index = 0;
    while index < count {
        let quantity = basket_quantity(frame, index)?;
        if quantity != 0 {
            let candidate = basket_candidate(
                market,
                source.as_deref(),
                destination.as_deref(),
                market_meta.outcome_count,
                index,
                frame.action,
                quantity,
            )?;
            let market_offsets = vector_offsets(market_meta.outcome_count, index)?;
            put_u64(market, market_offsets.supply, candidate.supply);
            put_u64(market, market_offsets.native, candidate.native);
            put_u64(market, market_offsets.materialized, candidate.materialized);
            let position_offsets = position_offsets(market_meta.outcome_count, index)?;
            if let Some(bytes) = source.as_deref_mut() {
                put_u64(bytes, position_offsets.native, candidate.source_native);
                put_u64(
                    bytes,
                    position_offsets.materialized,
                    candidate.source_materialized,
                );
            }
            if let Some(bytes) = destination.as_deref_mut() {
                put_u64(bytes, position_offsets.native, candidate.destination_native);
                put_u64(
                    bytes,
                    position_offsets.materialized,
                    candidate.destination_materialized,
                );
            }
        }
        index = index.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
    }
    put_u64(market, HOARD_OFFSET, candidate_hoard);
    put_u64(market, REVISION_OFFSET, next_market_revision);
    if let (Some(bytes), Some(revision)) = (source, next_source_revision) {
        put_u64(bytes, POSITION_REVISION_OFFSET, revision);
    }
    if let (Some(bytes), Some(revision)) = (destination, next_destination_revision) {
        put_u64(bytes, POSITION_REVISION_OFFSET, revision);
    }
    Ok(Payout {
        recipient: if payout == 0 {
            [0; 32]
        } else {
            source_meta.map_or([0; 32], |value| value.owner)
        },
        amount: payout,
    })
}

/// Return one Market's exact runtime outcome width.
pub fn market_outcome_count(bytes: &[u8]) -> Result<u32> {
    decode_market(bytes).map(|value| value.outcome_count)
}

/// Return one Position's immutable owner coordinate.
pub fn position_owner(bytes: &[u8], outcome_count: u32) -> Result<[u8; 32]> {
    validate_position(bytes, outcome_count).map(|value| value.owner)
}

/// Return the immutable release-set identity selected by one Market.
pub fn market_release_set_id(bytes: &[u8]) -> Result<[u8; 32]> {
    decode_market(bytes).map(|value| value.release_set_id)
}

/// Return one Market's immutable account identity.
pub fn market_identity(bytes: &[u8]) -> Result<[u8; 32]> {
    decode_market(bytes).map(|value| value.market_id)
}

/// Return the immutable Registry program trusted by one Market.
pub fn market_registry_program(bytes: &[u8]) -> Result<[u8; 32]> {
    decode_market(bytes).map(|value| value.registry_program)
}

/// Return one Market's optimistic revision coordinate.
pub fn market_revision(bytes: &[u8]) -> Result<u64> {
    decode_market(bytes).map(|value| value.revision)
}

/// Return one Market's exact lifecycle phase.
pub fn market_phase(bytes: &[u8]) -> Result<Phase> {
    decode_market(bytes).map(|value| value.phase)
}

/// Return one Position's optimistic revision coordinate.
pub fn position_revision(bytes: &[u8], outcome_count: u32) -> Result<u64> {
    validate_position(bytes, outcome_count).map(|value| value.revision)
}

/// Return one native claim balance.
pub fn position_native(bytes: &[u8], outcome_count: u32, outcome: u32) -> Result<u64> {
    let index = checked_outcome_index(outcome_count, outcome)?;
    validate_position(bytes, outcome_count)?;
    u64_at(bytes, position_offsets(outcome_count, index)?.native)
}

/// Return one materialized claim balance.
pub fn position_materialized(bytes: &[u8], outcome_count: u32, outcome: u32) -> Result<u64> {
    let index = checked_outcome_index(outcome_count, outcome)?;
    validate_position(bytes, outcome_count)?;
    u64_at(bytes, position_offsets(outcome_count, index)?.materialized)
}

fn checked_outcome_index(outcome_count: u32, outcome: u32) -> Result<usize> {
    if outcome >= outcome_count {
        return Err(Error::InvalidOutcome);
    }
    usize::try_from(outcome).map_err(|_| Error::InvalidOutcome)
}

fn validate_basket_shape(
    action: BasketAction,
    source_present: bool,
    destination_present: bool,
) -> Result<()> {
    let expected = match action {
        BasketAction::TransferNative | BasketAction::Materialize | BasketAction::Dematerialize => {
            (true, true)
        }
        BasketAction::RedeemNativeTerminal
        | BasketAction::RedeemMaterializedTerminal
        | BasketAction::MergeCompleteSet => (true, false),
        BasketAction::MintCompleteSet => (false, true),
    };
    if (source_present, destination_present) == expected {
        Ok(())
    } else {
        Err(Error::AccountMismatch)
    }
}

fn validate_basket_quantities(frame: BasketFrame<'_>, outcome_count: u32) -> Result<()> {
    if frame.quantity_multiplier == 0 {
        return Err(Error::ZeroQuantity);
    }
    let count = usize::try_from(outcome_count).map_err(|_| Error::InvalidOutcomeCount)?;
    let first = basket_quantity(frame, 0)?;
    let complete_set = matches!(
        frame.action,
        BasketAction::MintCompleteSet | BasketAction::MergeCompleteSet
    );
    let mut any_positive = false;
    let mut index = 0_usize;
    while index < count {
        let quantity = basket_quantity(frame, index)?;
        if quantity != 0 {
            any_positive = true;
        }
        if complete_set && quantity != first {
            return Err(Error::CandidateInvariantFailure);
        }
        index = index.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
    }
    if any_positive {
        Ok(())
    } else {
        Err(Error::ZeroQuantity)
    }
}

fn admit_basket_phase(phase: Phase, action: BasketAction) -> Result<()> {
    match (phase, action) {
        (
            Phase::Open,
            BasketAction::TransferNative
            | BasketAction::Materialize
            | BasketAction::Dematerialize
            | BasketAction::MintCompleteSet
            | BasketAction::MergeCompleteSet,
        )
        | (
            Phase::Terminal(_) | Phase::Retiring(_),
            BasketAction::Dematerialize
            | BasketAction::RedeemNativeTerminal
            | BasketAction::RedeemMaterializedTerminal,
        ) => Ok(()),
        _ => Err(Error::InvalidPhase),
    }
}

fn validate_optional_positions(
    market: &[u8],
    source: Option<&[u8]>,
    destination: Option<&[u8]>,
    outcome_count: u32,
) -> Result<()> {
    validate_market(market)?;
    let count = usize::try_from(outcome_count).map_err(|_| Error::InvalidOutcomeCount)?;
    let mut index = 0_usize;
    while index < count {
        let market_offsets = vector_offsets(outcome_count, index)?;
        let position_offsets = position_offsets(outcome_count, index)?;
        let aggregate_native = u64_at(market, market_offsets.native)?;
        let aggregate_materialized = u64_at(market, market_offsets.materialized)?;
        let source_native = source
            .map(|bytes| u64_at(bytes, position_offsets.native))
            .transpose()?
            .unwrap_or(0);
        let destination_native = destination
            .map(|bytes| u64_at(bytes, position_offsets.native))
            .transpose()?
            .unwrap_or(0);
        let source_materialized = source
            .map(|bytes| u64_at(bytes, position_offsets.materialized))
            .transpose()?
            .unwrap_or(0);
        let destination_materialized = destination
            .map(|bytes| u64_at(bytes, position_offsets.materialized))
            .transpose()?
            .unwrap_or(0);
        if source_native
            .checked_add(destination_native)
            .ok_or(Error::ArithmeticOverflow)?
            > aggregate_native
            || source_materialized
                .checked_add(destination_materialized)
                .ok_or(Error::ArithmeticOverflow)?
                > aggregate_materialized
        {
            return Err(Error::PositionProjectionExceedsSupply);
        }
        index = index.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
    }
    Ok(())
}

fn basket_hoard_and_payout(market: MarketMeta, frame: BasketFrame<'_>) -> Result<(u64, u64)> {
    let complete_quantity = basket_quantity(frame, 0)?;
    match frame.action {
        BasketAction::MintCompleteSet => Ok((credit(market.hoard, complete_quantity)?, 0)),
        BasketAction::MergeCompleteSet => {
            Ok((debit(market.hoard, complete_quantity)?, complete_quantity))
        }
        BasketAction::RedeemNativeTerminal | BasketAction::RedeemMaterializedTerminal => {
            let winner = terminal_winner(market.phase).ok_or(Error::InvalidPhase)?;
            let index = usize::try_from(winner).map_err(|_| Error::InvalidOutcome)?;
            let payout = basket_quantity(frame, index)?;
            Ok((debit(market.hoard, payout)?, payout))
        }
        BasketAction::TransferNative | BasketAction::Materialize | BasketAction::Dematerialize => {
            Ok((market.hoard, 0))
        }
    }
}

fn basket_quantity(frame: BasketFrame<'_>, index: usize) -> Result<u64> {
    u64_at(
        frame.quantities,
        index
            .checked_mul(SCALAR_BYTES)
            .ok_or(Error::InvalidOutcomeCount)?,
    )?
    .checked_mul(frame.quantity_multiplier)
    .ok_or(Error::ArithmeticOverflow)
}

fn basket_candidate(
    market: &[u8],
    source: Option<&[u8]>,
    destination: Option<&[u8]>,
    outcome_count: u32,
    index: usize,
    action: BasketAction,
    quantity: u64,
) -> Result<BasketCandidate> {
    let market_offsets = vector_offsets(outcome_count, index)?;
    let position_offsets = position_offsets(outcome_count, index)?;
    let mut candidate = BasketCandidate {
        supply: u64_at(market, market_offsets.supply)?,
        native: u64_at(market, market_offsets.native)?,
        materialized: u64_at(market, market_offsets.materialized)?,
        source_native: source
            .map(|bytes| u64_at(bytes, position_offsets.native))
            .transpose()?
            .unwrap_or(0),
        source_materialized: source
            .map(|bytes| u64_at(bytes, position_offsets.materialized))
            .transpose()?
            .unwrap_or(0),
        destination_native: destination
            .map(|bytes| u64_at(bytes, position_offsets.native))
            .transpose()?
            .unwrap_or(0),
        destination_materialized: destination
            .map(|bytes| u64_at(bytes, position_offsets.materialized))
            .transpose()?
            .unwrap_or(0),
    };
    match action {
        BasketAction::TransferNative => {
            candidate.source_native = debit(candidate.source_native, quantity)?;
            candidate.destination_native = credit(candidate.destination_native, quantity)?;
        }
        BasketAction::Materialize => {
            candidate.native = debit(candidate.native, quantity)?;
            candidate.source_native = debit(candidate.source_native, quantity)?;
            candidate.materialized = credit(candidate.materialized, quantity)?;
            candidate.destination_materialized =
                credit(candidate.destination_materialized, quantity)?;
        }
        BasketAction::Dematerialize => {
            candidate.materialized = debit(candidate.materialized, quantity)?;
            candidate.source_materialized = debit(candidate.source_materialized, quantity)?;
            candidate.native = credit(candidate.native, quantity)?;
            candidate.destination_native = credit(candidate.destination_native, quantity)?;
        }
        BasketAction::RedeemNativeTerminal => {
            candidate.supply = debit(candidate.supply, quantity)?;
            candidate.native = debit(candidate.native, quantity)?;
            candidate.source_native = debit(candidate.source_native, quantity)?;
        }
        BasketAction::RedeemMaterializedTerminal => {
            candidate.supply = debit(candidate.supply, quantity)?;
            candidate.materialized = debit(candidate.materialized, quantity)?;
            candidate.source_materialized = debit(candidate.source_materialized, quantity)?;
        }
        BasketAction::MintCompleteSet => {
            candidate.supply = credit(candidate.supply, quantity)?;
            candidate.native = credit(candidate.native, quantity)?;
            candidate.destination_native = credit(candidate.destination_native, quantity)?;
        }
        BasketAction::MergeCompleteSet => {
            candidate.supply = debit(candidate.supply, quantity)?;
            candidate.native = debit(candidate.native, quantity)?;
            candidate.source_native = debit(candidate.source_native, quantity)?;
        }
    }
    Ok(candidate)
}

fn validate_basket_candidate(
    phase: Phase,
    index: usize,
    hoard: u64,
    candidate: BasketCandidate,
) -> Result<()> {
    if candidate.supply
        != candidate
            .native
            .checked_add(candidate.materialized)
            .ok_or(Error::ArithmeticOverflow)?
        || candidate
            .source_native
            .checked_add(candidate.destination_native)
            .ok_or(Error::ArithmeticOverflow)?
            > candidate.native
        || candidate
            .source_materialized
            .checked_add(candidate.destination_materialized)
            .ok_or(Error::ArithmeticOverflow)?
            > candidate.materialized
    {
        return Err(Error::CandidateInvariantFailure);
    }
    match phase {
        Phase::Open if candidate.supply > hoard => Err(Error::CandidateInvariantFailure),
        Phase::Terminal(winner) | Phase::Retiring(winner)
            if usize::try_from(winner).map_err(|_| Error::InvalidOutcome)? == index
                && candidate.supply > hoard =>
        {
            Err(Error::CandidateInvariantFailure)
        }
        Phase::Retired => Err(Error::InvalidPhase),
        _ => Ok(()),
    }
}

fn command_coordinates(command: Command) -> (u32, u64) {
    match command {
        Command::TransferNative { outcome, quantity }
        | Command::Materialize { outcome, quantity }
        | Command::Dematerialize { outcome, quantity }
        | Command::RedeemTerminal { outcome, quantity } => (outcome, quantity),
    }
}

fn admit_phase(phase: Phase, command: Command) -> Result<()> {
    match (phase, command) {
        (
            Phase::Open,
            Command::TransferNative { .. }
            | Command::Materialize { .. }
            | Command::Dematerialize { .. },
        )
        | (
            Phase::Terminal(_) | Phase::Retiring(_),
            Command::Dematerialize { .. } | Command::RedeemTerminal { .. },
        ) => Ok(()),
        _ => Err(Error::InvalidPhase),
    }
}

fn terminal_winner(phase: Phase) -> Option<u32> {
    match phase {
        Phase::Terminal(winner) | Phase::Retiring(winner) => Some(winner),
        Phase::Open | Phase::Retired => None,
    }
}

fn validate_market(bytes: &[u8]) -> Result<()> {
    let meta = decode_market(bytes)?;
    let count = usize::try_from(meta.outcome_count).map_err(|_| Error::InvalidOutcomeCount)?;
    let mut index = 0_usize;
    while index < count {
        let offsets = vector_offsets(meta.outcome_count, index)?;
        let supply = u64_at(bytes, offsets.supply)?;
        let native = u64_at(bytes, offsets.native)?;
        let materialized = u64_at(bytes, offsets.materialized)?;
        if native
            .checked_add(materialized)
            .ok_or(Error::ArithmeticOverflow)?
            != supply
        {
            return Err(Error::SupplyPartitionMismatch);
        }
        match meta.phase {
            Phase::Open if supply > meta.hoard => return Err(Error::Insolvent),
            Phase::Terminal(winner) | Phase::Retiring(winner)
                if usize::try_from(winner).map_err(|_| Error::InvalidOutcome)? == index
                    && supply > meta.hoard =>
            {
                return Err(Error::Insolvent);
            }
            Phase::Retired if supply != 0 || meta.hoard != 0 => return Err(Error::Insolvent),
            _ => {}
        }
        index = index.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
    }
    Ok(())
}

fn validate_joined(
    market: &[u8],
    source: &[u8],
    destination: &[u8],
    outcome_count: u32,
) -> Result<()> {
    validate_market(market)?;
    let count = usize::try_from(outcome_count).map_err(|_| Error::InvalidOutcomeCount)?;
    let mut index = 0_usize;
    while index < count {
        let market_offsets = vector_offsets(outcome_count, index)?;
        let position_offsets = position_offsets(outcome_count, index)?;
        let native = u64_at(market, market_offsets.native)?;
        let materialized = u64_at(market, market_offsets.materialized)?;
        if u64_at(source, position_offsets.native)?
            .checked_add(u64_at(destination, position_offsets.native)?)
            .ok_or(Error::ArithmeticOverflow)?
            > native
            || u64_at(source, position_offsets.materialized)?
                .checked_add(u64_at(destination, position_offsets.materialized)?)
                .ok_or(Error::ArithmeticOverflow)?
                > materialized
        {
            return Err(Error::PositionProjectionExceedsSupply);
        }
        index = index.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
    }
    Ok(())
}

fn validate_candidate_solvency(
    market: &[u8],
    meta: MarketMeta,
    changed_outcome: u32,
    changed_supply: u64,
    hoard: u64,
) -> Result<()> {
    let count = usize::try_from(meta.outcome_count).map_err(|_| Error::InvalidOutcomeCount)?;
    let mut index = 0_usize;
    while index < count {
        let selected = u32::try_from(index).map_err(|_| Error::InvalidOutcomeCount)?;
        let supply = if selected == changed_outcome {
            changed_supply
        } else {
            u64_at(market, vector_offsets(meta.outcome_count, index)?.supply)?
        };
        match meta.phase {
            Phase::Open if supply > hoard => return Err(Error::CandidateInvariantFailure),
            Phase::Terminal(winner) | Phase::Retiring(winner)
                if winner == selected && supply > hoard =>
            {
                return Err(Error::CandidateInvariantFailure);
            }
            _ => {}
        }
        index = index.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
    }
    Ok(())
}

fn decode_market(bytes: &[u8]) -> Result<MarketMeta> {
    if bytes.len() < MARKET_HEADER_BYTES {
        return Err(Error::InvalidLength);
    }
    exact(bytes, 0, &MARKET_MAGIC)?;
    if u16_at(bytes, 8)? != SCHEMA_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, 5)?;
    require_zero(bytes, MARKET_RESERVED_OFFSET, 8)?;
    let outcome_count = u32_at(bytes, OUTCOME_COUNT_OFFSET)?;
    exact_market_width(bytes, outcome_count)?;
    let phase = decode_phase(bytes, outcome_count)?;
    let market_id = nonzero_array(bytes, MARKET_ID_OFFSET)?;
    let release_set_id = nonzero_array(bytes, RELEASE_SET_ID_OFFSET)?;
    let registry_program = nonzero_array(bytes, REGISTRY_PROGRAM_OFFSET)?;
    Ok(MarketMeta {
        outcome_count,
        phase,
        revision: u64_at(bytes, REVISION_OFFSET)?,
        hoard: u64_at(bytes, HOARD_OFFSET)?,
        market_id,
        release_set_id,
        registry_program,
    })
}

fn validate_position(bytes: &[u8], outcome_count: u32) -> Result<PositionMeta> {
    exact_position_width(bytes, outcome_count)?;
    exact(bytes, 0, &POSITION_MAGIC)?;
    if u16_at(bytes, 8)? != SCHEMA_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    require_zero(bytes, 10, 6)?;
    require_zero(bytes, POSITION_RESERVED_OFFSET, 8)?;
    Ok(PositionMeta {
        market_id: nonzero_array(bytes, POSITION_MARKET_ID_OFFSET)?,
        owner: nonzero_array(bytes, POSITION_OWNER_OFFSET)?,
        revision: u64_at(bytes, POSITION_REVISION_OFFSET)?,
    })
}

fn decode_phase(bytes: &[u8], outcome_count: u32) -> Result<Phase> {
    let winner = u32_at(bytes, WINNER_OFFSET)?;
    let phase = match byte_at(bytes, PHASE_OFFSET)? {
        0 if winner == u32::MAX => Phase::Open,
        1 if winner < outcome_count => Phase::Terminal(winner),
        2 if winner < outcome_count => Phase::Retiring(winner),
        3 if winner == u32::MAX => Phase::Retired,
        _ => return Err(Error::NonCanonical),
    };
    Ok(phase)
}

fn encode_phase(output: &mut [u8], phase: Phase) -> Result<()> {
    let (tag, winner) = match phase {
        Phase::Open => (0, u32::MAX),
        Phase::Terminal(winner) => (1, winner),
        Phase::Retiring(winner) => (2, winner),
        Phase::Retired => (3, u32::MAX),
    };
    put_byte(output, PHASE_OFFSET, tag)?;
    put(output, WINNER_OFFSET, &winner.to_le_bytes())
}

fn validate_phase(outcome_count: u32, phase: Phase) -> Result<()> {
    if outcome_count == 0 {
        return Err(Error::InvalidOutcomeCount);
    }
    match phase {
        Phase::Terminal(winner) | Phase::Retiring(winner) if winner >= outcome_count => {
            Err(Error::NonCanonical)
        }
        _ => Ok(()),
    }
}

struct VectorOffsets {
    supply: usize,
    native: usize,
    materialized: usize,
}

struct PositionOffsets {
    native: usize,
    materialized: usize,
}

fn vector_offsets(outcome_count: u32, index: usize) -> Result<VectorOffsets> {
    let vector_bytes = vector_bytes(outcome_count)?;
    let scalar = index
        .checked_mul(SCALAR_BYTES)
        .ok_or(Error::InvalidOutcomeCount)?;
    Ok(VectorOffsets {
        supply: MARKET_HEADER_BYTES
            .checked_add(scalar)
            .ok_or(Error::InvalidOutcomeCount)?,
        native: MARKET_HEADER_BYTES
            .checked_add(vector_bytes)
            .and_then(|value| value.checked_add(scalar))
            .ok_or(Error::InvalidOutcomeCount)?,
        materialized: MARKET_HEADER_BYTES
            .checked_add(
                vector_bytes
                    .checked_mul(2)
                    .ok_or(Error::InvalidOutcomeCount)?,
            )
            .and_then(|value| value.checked_add(scalar))
            .ok_or(Error::InvalidOutcomeCount)?,
    })
}

fn position_offsets(outcome_count: u32, index: usize) -> Result<PositionOffsets> {
    let vector_bytes = vector_bytes(outcome_count)?;
    let scalar = index
        .checked_mul(SCALAR_BYTES)
        .ok_or(Error::InvalidOutcomeCount)?;
    Ok(PositionOffsets {
        native: POSITION_HEADER_BYTES
            .checked_add(scalar)
            .ok_or(Error::InvalidOutcomeCount)?,
        materialized: POSITION_HEADER_BYTES
            .checked_add(vector_bytes)
            .and_then(|value| value.checked_add(scalar))
            .ok_or(Error::InvalidOutcomeCount)?,
    })
}

fn exact_market_width(bytes: &[u8], outcome_count: u32) -> Result<()> {
    validate_phase(outcome_count, Phase::Open)?;
    let expected = MARKET_HEADER_BYTES
        .checked_add(
            vector_bytes(outcome_count)?
                .checked_mul(3)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn exact_position_width(bytes: &[u8], outcome_count: u32) -> Result<()> {
    if outcome_count == 0 {
        return Err(Error::InvalidOutcomeCount);
    }
    let expected = POSITION_HEADER_BYTES
        .checked_add(
            vector_bytes(outcome_count)?
                .checked_mul(2)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn vector_bytes(outcome_count: u32) -> Result<usize> {
    usize::try_from(outcome_count)
        .map_err(|_| Error::InvalidOutcomeCount)?
        .checked_mul(SCALAR_BYTES)
        .ok_or(Error::InvalidOutcomeCount)
}

fn debit(value: u64, quantity: u64) -> Result<u64> {
    value
        .checked_sub(quantity)
        .ok_or(Error::InsufficientBalance)
}

fn credit(value: u64, quantity: u64) -> Result<u64> {
    value.checked_add(quantity).ok_or(Error::ArithmeticOverflow)
}

fn require_nonzero(identity: [u8; 32]) -> Result<()> {
    if identity.iter().all(|byte| *byte == 0) {
        Err(Error::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> Result<()> {
    if slice(input, offset, expected.len())? == expected {
        Ok(())
    } else {
        Err(Error::InvalidMagic)
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(input, offset, width)?.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(Error::NonCanonical)
    }
}

fn nonzero_array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array_at(input, offset)?;
    require_nonzero(value)?;
    Ok(value)
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array_at(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    slice(input, offset, 32)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(input, offset, 2)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        slice(input, offset, 4)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        slice(input, offset, 8)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let target = output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    target.copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(8)) {
        target.copy_from_slice(&value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;

    fn widths(count: u32) -> (usize, usize) {
        let count = usize::try_from(count).unwrap_or(0);
        (
            MARKET_HEADER_BYTES + count * SCALAR_BYTES * 3,
            POSITION_HEADER_BYTES + count * SCALAR_BYTES * 2,
        )
    }

    fn fixture(count: u32) -> (std::vec::Vec<u8>, std::vec::Vec<u8>, std::vec::Vec<u8>) {
        let (market_bytes, position_bytes) = widths(count);
        let mut market = vec![0; market_bytes];
        let mut source = vec![0; position_bytes];
        let mut destination = vec![0; position_bytes];
        assert_eq!(
            initialize_market(
                &mut market,
                [1; 32],
                [2; 32],
                [8; 32],
                count,
                Phase::Open,
                10,
            ),
            Ok(())
        );
        assert_eq!(
            initialize_position(&mut source, [1; 32], [3; 32], count),
            Ok(())
        );
        assert_eq!(
            initialize_position(&mut destination, [1; 32], [4; 32], count),
            Ok(())
        );
        (market, source, destination)
    }

    fn set_claim(bytes: &mut [u8], offset: usize, value: u64) {
        bytes
            .get_mut(offset..offset + 8)
            .unwrap_or(&mut [])
            .copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn runtime_width_exceeds_old_profile_without_monomorphization() {
        let count = 257;
        let (market, source, destination) = fixture(count);
        assert_eq!(market_outcome_count(&market), Ok(count));
        assert_eq!(position_owner(&source, count), Ok([3; 32]));
        assert_eq!(position_owner(&destination, count), Ok([4; 32]));
    }

    #[test]
    fn native_transfer_changes_positions_without_forking_aggregate_supply() -> Result<()> {
        let count = 3;
        let (mut market, mut source, mut destination) = fixture(count);
        let market_offsets = vector_offsets(count, 2)?;
        let position_offsets = position_offsets(count, 2)?;
        set_claim(&mut market, market_offsets.supply, 9);
        set_claim(&mut market, market_offsets.native, 9);
        set_claim(&mut source, position_offsets.native, 9);
        assert_eq!(
            execute(
                &mut market,
                &mut source,
                &mut destination,
                Frame {
                    expected_market_revision: 0,
                    expected_source_revision: 0,
                    expected_destination_revision: 0,
                    command: Command::TransferNative {
                        outcome: 2,
                        quantity: 4,
                    },
                },
            ),
            Ok(Payout {
                recipient: [3; 32],
                amount: 0,
            })
        );
        assert_eq!(position_native(&source, count, 2), Ok(5));
        assert_eq!(position_native(&destination, count, 2), Ok(4));
        assert_eq!(u64_at(&market, market_offsets.supply), Ok(9));
        assert_eq!(u64_at(&market, market_offsets.native), Ok(9));
        Ok(())
    }

    #[test]
    fn complete_set_basket_advances_each_account_once_and_roundtrips() -> Result<()> {
        let count = 257;
        let (mut market, _unused_source, mut holder) = fixture(count);
        set_claim(&mut market, HOARD_OFFSET, 0);
        let quantities = [5_u8, 0, 0, 0, 0, 0, 0, 0].repeat(257);
        assert_eq!(
            execute_basket(
                &mut market,
                None,
                Some(&mut holder),
                BasketFrame {
                    expected_market_revision: 0,
                    expected_source_revision: None,
                    expected_destination_revision: Some(0),
                    action: BasketAction::MintCompleteSet,
                    quantities: &quantities,
                    quantity_multiplier: 1,
                },
            ),
            Ok(Payout {
                recipient: [0; 32],
                amount: 0,
            })
        );
        assert_eq!(market_revision(&market), Ok(1));
        assert_eq!(position_revision(&holder, count), Ok(1));
        assert_eq!(position_native(&holder, count, 256), Ok(5));
        assert_eq!(u64_at(&market, HOARD_OFFSET), Ok(5));
        assert_eq!(
            execute_basket(
                &mut market,
                Some(&mut holder),
                None,
                BasketFrame {
                    expected_market_revision: 1,
                    expected_source_revision: Some(1),
                    expected_destination_revision: None,
                    action: BasketAction::MergeCompleteSet,
                    quantities: &quantities,
                    quantity_multiplier: 1,
                },
            ),
            Ok(Payout {
                recipient: [4; 32],
                amount: 5,
            })
        );
        assert_eq!(market_revision(&market), Ok(2));
        assert_eq!(position_revision(&holder, count), Ok(2));
        assert_eq!(position_native(&holder, count, 256), Ok(0));
        assert_eq!(u64_at(&market, HOARD_OFFSET), Ok(0));
        Ok(())
    }

    #[test]
    fn basket_late_coordinate_refusal_is_exact_rollback() -> Result<()> {
        let count = 3;
        let (mut market, _unused_source, mut holder) = fixture(count);
        set_claim(&mut market, HOARD_OFFSET, 0);
        let quantities = [5_u64, 5, u64::MAX]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<std::vec::Vec<_>>();
        let before_market = market.clone();
        let before_holder = holder.clone();
        assert!(
            execute_basket(
                &mut market,
                None,
                Some(&mut holder),
                BasketFrame {
                    expected_market_revision: 0,
                    expected_source_revision: None,
                    expected_destination_revision: Some(0),
                    action: BasketAction::MintCompleteSet,
                    quantities: &quantities,
                    quantity_multiplier: 1,
                },
            )
            .is_err()
        );
        assert_eq!(market, before_market);
        assert_eq!(holder, before_holder);
        Ok(())
    }

    #[test]
    fn issue_then_transferred_receipt_redeems_to_a_different_holder() -> Result<()> {
        let count = 3;
        let (mut market, mut issuer, mut wrapper) = fixture(count);
        let market_offsets = vector_offsets(count, 1)?;
        let issuer_offsets = position_offsets(count, 1)?;
        set_claim(&mut market, market_offsets.supply, 10);
        set_claim(&mut market, market_offsets.native, 10);
        set_claim(&mut issuer, issuer_offsets.native, 10);
        assert_eq!(validate_joined(&market, &issuer, &wrapper, count), Ok(()));
        assert_eq!(
            execute(
                &mut market,
                &mut issuer,
                &mut wrapper,
                Frame {
                    expected_market_revision: 0,
                    expected_source_revision: 0,
                    expected_destination_revision: 0,
                    command: Command::Materialize {
                        outcome: 1,
                        quantity: 4,
                    },
                },
            ),
            Ok(Payout {
                recipient: [3; 32],
                amount: 0,
            })
        );

        let (_, position_bytes) = widths(count);
        let mut current_holder = vec![0; position_bytes];
        assert_eq!(
            initialize_position(&mut current_holder, [1; 32], [9; 32], count),
            Ok(())
        );
        assert_eq!(
            execute(
                &mut market,
                &mut wrapper,
                &mut current_holder,
                Frame {
                    expected_market_revision: 1,
                    expected_source_revision: 1,
                    expected_destination_revision: 0,
                    command: Command::Dematerialize {
                        outcome: 1,
                        quantity: 4,
                    },
                },
            ),
            Ok(Payout {
                recipient: [4; 32],
                amount: 0,
            })
        );
        let holder_offsets = position_offsets(count, 1)?;
        assert_eq!(u64_at(&current_holder, holder_offsets.native), Ok(4));
        assert_eq!(u64_at(&wrapper, holder_offsets.materialized), Ok(0));
        assert_eq!(u64_at(&market, market_offsets.native), Ok(10));
        assert_eq!(u64_at(&market, market_offsets.materialized), Ok(0));
        Ok(())
    }

    #[test]
    fn hostile_refusals_are_byte_exact_rollbacks() {
        let count = 3;
        let (mut market, mut source, mut destination) = fixture(count);
        let before_market = market.clone();
        let before_source = source.clone();
        let before_destination = destination.clone();
        for frame in [
            Frame {
                expected_market_revision: 1,
                expected_source_revision: 0,
                expected_destination_revision: 0,
                command: Command::Materialize {
                    outcome: 0,
                    quantity: 1,
                },
            },
            Frame {
                expected_market_revision: 0,
                expected_source_revision: 0,
                expected_destination_revision: 0,
                command: Command::Materialize {
                    outcome: 3,
                    quantity: 1,
                },
            },
            Frame {
                expected_market_revision: 0,
                expected_source_revision: 0,
                expected_destination_revision: 0,
                command: Command::Materialize {
                    outcome: 0,
                    quantity: 0,
                },
            },
        ] {
            assert!(execute(&mut market, &mut source, &mut destination, frame).is_err());
            assert_eq!(market, before_market);
            assert_eq!(source, before_source);
            assert_eq!(destination, before_destination);
        }
    }
}
