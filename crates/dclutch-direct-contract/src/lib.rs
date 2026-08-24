#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout, SDK-free contracts for permissionless Direct matching.
//!
//! A maker authorizes one exact [`DirectIntentV2`] through the native Ed25519
//! program. An inline FOK/IOC fill or resting-intent registration consumes the
//! same gap-free [`MakerReplayRootV2`] nonce for `(Market, generation, maker)`.
//! Inline execution transfers atomically without persistent order state;
//! registration creates a program-owned live intent and reserves every asset
//! needed for partial fills. Full fill, cancellation, or expiry closes that
//! live intent and refunds all per-order rent. In both lifecycles an untrusted
//! matcher may choose only a compatible price and fill, while the compact maker
//! root remains the sole replay high-water mark.
//!
//! This crate has no Solana SDK, token, CPI, hashing, account-memory, or
//! allocation dependency. [`adapter`] specifies the exact hostile-decodable
//! instruction/account/native-signature boundary and pinned measured v0 packet
//! profile which a composing SBF adapter must enforce.

use core::convert::TryInto;

pub mod adapter;
mod settlement;
mod state;

pub use settlement::{
    ComplementaryBuyMatchV2, ComplementarySellMatchV2, InlineComplementaryMatchV2,
    InlineComplementarySettlementV2, InlineOrdinaryMatchV2, InlineOrdinarySettlementV2,
    MergeSettlementV2, OrdinaryMatchV2, OrdinarySettlementV2, SplitSettlementV2,
    settle_inline_complementary_v2, settle_inline_ordinary_v2, settle_merge_v2, settle_ordinary_v2,
    settle_split_v2,
};
pub use state::{
    CancelThroughV1, CancellationInputV2, CancellationV2, DIRECT_CANCEL_BYTES_V2,
    DIRECT_CANCEL_MAGIC_V2, DIRECT_CANCEL_SCHEMA_VERSION_V2, DIRECT_CANCEL_THROUGH_BYTES_V1,
    DIRECT_CANCEL_THROUGH_MAGIC_V1, DIRECT_CANCEL_THROUGH_SCHEMA_VERSION_V1,
    DIRECT_INTENT_BYTES_V2, DIRECT_INTENT_ESCROW_PDA_DOMAIN_V2, DIRECT_INTENT_MAGIC_V2,
    DIRECT_INTENT_RECORD_BYTES_V2, DIRECT_INTENT_RECORD_MAGIC_V2,
    DIRECT_INTENT_RECORD_PDA_DOMAIN_V2, DIRECT_INTENT_RECORD_SCHEMA_VERSION_V2,
    DIRECT_INTENT_SCHEMA_VERSION_V2, DirectCancelV2, DirectIntentInputV2, DirectIntentRecordV2,
    DirectIntentV2, ExpirationInputV2, ExpirationV2, InlineParticipantAccountsV2,
    IntentLifecycleV2, InvalidatedCloseInputV1, LiveRecordCloseV2, MAKER_REPLAY_ROOT_BYTES_V2,
    MAKER_REPLAY_ROOT_MAGIC_V2, MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
    MAKER_REPLAY_ROOT_SCHEMA_VERSION_V2, MakerReplayRootV2, ParticipantAccountsV2,
    RecordAfterFillV2, RegistrationInputV2, RegistrationV2, ReplayRegistrationStatusV2,
    ReplayRootStateV2, RootClosureV2, Side, TerminalRentTransitionV2, VENUE_FEE_POLICY_BYTES_V2,
    VENUE_FEE_POLICY_MAGIC_V2, VENUE_FEE_POLICY_SCHEMA_VERSION_V2, VenueFeePolicyV2,
    cancel_intent_v2, cancel_through_v1, close_invalidated_intent_v1, close_replay_registration_v2,
    expire_intent_v2, prepare_replay_root_close_v2, register_intent_v2,
    terminal_rent_transition_v2,
};

/// Exact scaled integer price denominator.
pub const PRICE_SCALE: u64 = 1_000_000;
/// Fee rate denominator.
pub const FEE_BASIS_POINTS_DENOMINATOR: u64 = 10_000;

/// Explicit refusal from a Direct parser or pure transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have its one exact width.
    InvalidLength,
    /// Output did not have its one exact width.
    OutputLength,
    /// Magic is not canonical for this record type.
    InvalidMagic,
    /// Schema is unsupported.
    UnsupportedSchema,
    /// Reserved bytes were nonzero.
    NonCanonicalReservedBytes,
    /// Required identity was all zero.
    ZeroIdentifier,
    /// Intent side was unknown.
    UnknownSide,
    /// Signed immediate/resting lifecycle byte was unknown.
    UnknownIntentLifecycle,
    /// Requested adapter lifecycle differed from maker-signed lifecycle.
    IntentLifecycleMismatch,
    /// Persisted status was unknown.
    UnknownIntentStatus,
    /// A slot interval was inverted.
    InvalidSlotInterval,
    /// A required positive quantity was zero.
    ZeroQuantity,
    /// A limit price exceeded a one-collateral payout.
    InvalidLimitPrice,
    /// Fee exceeded 10,000 basis points.
    InvalidFeeRate,
    /// Persisted reservation was inconsistent with signed intent and fill.
    InvalidReservation,
    /// Exact Position, record, root, or collateral account binding differed.
    AccountBindingMismatch,
    /// Position Market or generation differed from signed intent.
    PositionMarketMismatch,
    /// Position owner differed from signed maker.
    PositionOwnerMismatch,
    /// Replay root differed from intent Market, generation, or maker.
    ReplayRootMismatch,
    /// Intent nonce was below or above the exact gap-free next nonce.
    NonceMismatch,
    /// Replay root registration was irreversibly closed.
    RegistrationClosed,
    /// Replay root live count could not be incremented or decremented.
    LiveCountInvariant,
    /// Maker-signed cancel-through threshold was non-monotone or beyond next nonce.
    InvalidCancelThrough,
    /// Settlement attempted to consume a record below the root's live threshold.
    IntentInvalidated,
    /// Permissionless invalidation close targeted a still-live nonce.
    IntentNotInvalidated,
    /// A root close was attempted while live intents remained.
    LiveIntentsRemain,
    /// Root close was attempted before Market retirement closed registration.
    RegistrationStillOpen,
    /// State fill exceeded signed capacity.
    StateOverfilled,
    /// Slot was outside signed inclusive validity interval.
    IntentExpired,
    /// Permissionless expiry was attempted at or before inclusive expiry.
    IntentNotExpired,
    /// Fill was zero or exceeded remaining capacity.
    InvalidFill,
    /// Inputs were not compatible sides and Market coordinates.
    IncompatibleSides,
    /// Matcher price was outside signed limits.
    PriceIncompatible,
    /// A scaled quote was not an exact collateral atom count.
    NonIntegralQuote,
    /// Exact checked arithmetic overflowed.
    ArithmeticOverflow,
    /// A Position lacked claims needed for reservation.
    InsufficientPositionBalance,
    /// An owner or physical account was repeated where it must be distinct.
    Alias,
    /// Complementary array was not exact canonical outcome order.
    NonCanonicalComplement,
    /// Complementary prices or quotes did not fund exactly one complete set.
    SplitFundingMismatch,
    /// Active Position width was outside the selected measured profile.
    InvalidOutcomeWidth,
    /// Selected outcome is outside active Position width.
    InvalidOutcome,
    /// Persisted fee selection differed from canonical Market fee policy.
    VenueUnauthorized,
    /// Ed25519 program identity was not the pinned native program.
    InvalidSignatureProgram,
    /// Signature-verification instruction was not immediately preceding.
    InvalidSignatureInstructionOrder,
    /// Native Ed25519 layout was not exact canonical single-signature form.
    InvalidSignatureInstruction,
    /// Ed25519 public key did not equal exact maker.
    SignatureSignerMismatch,
    /// Ed25519 message did not equal exact intent or cancellation preimage.
    SignatureMessageMismatch,
    /// A trivially forged all-zero signature was supplied.
    ForgedSignature,
    /// Adapter action was unknown or wrong for requested decoder.
    UnknownAdapterAction,
    /// Canonical Market phase did not admit this Direct action.
    MarketPhaseRefused,
    /// Adapter participant count did not equal canonical action width.
    InvalidParticipantCount,
    /// Settlement authorization modes were not all identical.
    MixedAuthorizationModes,
    /// Mode bytes differed from action's exact inline or registered lifecycle.
    AuthorizationLifecycleMismatch,
    /// Inline complementary execution exceeded measured N=2 packet profile.
    InvalidInlineWidth,
    /// Adapter account count, privilege, or aliasing was invalid.
    InvalidAccountFrame,
    /// Buy source owner, delegate, allowance, or account binding was invalid.
    InvalidBuyDebitAuthority,
    /// Registered collateral escrow was not controlled by its live-record PDA.
    InvalidEscrowAuthority,
    /// Serialized v0 transaction exceeded pinned packet/account limits.
    PacketEnvelopeExceeded,
    /// Signature, LUT, instruction data, or account profile was noncanonical.
    PacketProfileMismatch,
    /// Live-account rent inputs were not monotone or solvent.
    InvalidRentTransition,
}

/// Result alias for this crate.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn width(value: usize) -> Result<()> {
    if (dclutch_realm_contract::MIN_OUTCOMES..=dclutch_realm_contract::MAX_OUTCOMES)
        .contains(&value)
    {
        Ok(())
    } else {
        Err(Error::InvalidOutcomeWidth)
    }
}

pub(crate) fn quote(quantity: u64, price: u64) -> Result<u64> {
    let product = u128::from(quantity)
        .checked_mul(u128::from(price))
        .ok_or(Error::ArithmeticOverflow)?;
    if product % u128::from(PRICE_SCALE) != 0 {
        return Err(Error::NonIntegralQuote);
    }
    u64::try_from(product / u128::from(PRICE_SCALE)).map_err(|_| Error::ArithmeticOverflow)
}

pub(crate) fn fee(gross: u64, bps: u16) -> Result<u64> {
    let product = u128::from(gross)
        .checked_mul(u128::from(bps))
        .ok_or(Error::ArithmeticOverflow)?;
    u64::try_from(product / u128::from(FEE_BASIS_POINTS_DENOMINATOR))
        .map_err(|_| Error::ArithmeticOverflow)
}

pub(crate) fn position_error(value: dclutch_realm_contract::Error) -> Error {
    match value {
        dclutch_realm_contract::Error::InsufficientBalance => Error::InsufficientPositionBalance,
        dclutch_realm_contract::Error::ArithmeticOverflow => Error::ArithmeticOverflow,
        _ => Error::InvalidOutcome,
    }
}

pub(crate) fn nonzero(value: &[u8; 32]) -> Result<()> {
    if value.iter().all(|item| *item == 0) {
        Err(Error::ZeroIdentifier)
    } else {
        Ok(())
    }
}

pub(crate) fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

pub(crate) fn one(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

pub(crate) fn zeros(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|item| *item != 0)
    {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

pub(crate) fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(destination) = output.get_mut(offset..offset.saturating_add(value.len())) {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests;
