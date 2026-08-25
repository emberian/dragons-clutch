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
    ComplementaryBuyMatchInPlaceV2, ComplementarySellMatchInPlaceV2, InlineComplementaryMatchV2,
    InlineComplementarySettlementV2, InlineOrdinaryMatchV2, InlineOrdinarySettlementV2,
    MergeSettlementEffectsV2, OrdinaryMatchV2, OrdinarySettlementV2, SplitSettlementEffectsV2,
    settle_inline_complementary_v2, settle_inline_ordinary_v2, settle_merge_in_place_v2,
    settle_ordinary_v2, settle_split_in_place_v2,
};
pub use state::{
    CancelThroughV1, CancellationInputV2, CancellationV2, DIRECT_CANCEL_BYTES_V2,
    DIRECT_CANCEL_MAGIC_V2, DIRECT_CANCEL_SCHEMA_VERSION_V2, DIRECT_CANCEL_THROUGH_BYTES_V1,
    DIRECT_CANCEL_THROUGH_MAGIC_V1, DIRECT_CANCEL_THROUGH_SCHEMA_VERSION_V1,
    DIRECT_INTENT_BYTES_V2, DIRECT_INTENT_ESCROW_PDA_DOMAIN_V2, DIRECT_INTENT_MAGIC_V2,
    DIRECT_INTENT_RECORD_BYTES_V2, DIRECT_INTENT_RECORD_MAGIC_V2,
    DIRECT_INTENT_RECORD_PDA_DOMAIN_V2, DIRECT_INTENT_RECORD_SCHEMA_VERSION_V2,
    DIRECT_INTENT_SCHEMA_VERSION_V2, DirectCancelV2, DirectIntentInputV2, DirectIntentRecordV2,
    DirectIntentV2, DirectRentCreditClosePlanV1, ExpirationInputV2, ExpirationV2,
    InlineParticipantAccountsV2, IntentLifecycleV2, InvalidatedCloseInputV1, LiveRecordCloseV2,
    MAKER_REPLAY_ROOT_BYTES_V2, MAKER_REPLAY_ROOT_MAGIC_V2, MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
    MAKER_REPLAY_ROOT_SCHEMA_VERSION_V2, MakerReplayRootV2, ParticipantAccountsV2,
    RecordAfterFillV2, RegistrationInputV2, RegistrationV2, ReplayRegistrationStatusV2,
    ReplayRootStateV2, RootClosureV2, Side, TerminalRentTransitionV2, VENUE_FEE_POLICY_BYTES_V3,
    VENUE_FEE_POLICY_MAGIC_V3, VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
    VENUE_FEE_POLICY_SCHEMA_VERSION_V3, VenueFeePolicyV3, cancel_intent_v2, cancel_through_v1,
    close_invalidated_intent_v1, close_replay_registration_v2, expire_intent_v2,
    prepare_replay_root_close_v2, register_intent_v2, terminal_rent_credit_close_plan_v1,
    terminal_rent_transition_v2, validate_venue_policy_selection_v3,
};

/// Exact scaled integer price denominator.
pub const PRICE_SCALE: u64 = 1_000_000;
/// Fee rate denominator.
pub const FEE_BASIS_POINTS_DENOMINATOR: u64 = 10_000;

/// SHA-256 of `dclutch/capability/direct/v2`, the canonical Direct kind.
pub const DIRECT_CAPABILITY_KIND_ID_V2: [u8; 32] = [
    0x84, 0x3c, 0xf4, 0x76, 0x33, 0x18, 0xac, 0x99, 0xa6, 0x9e, 0x15, 0x66, 0x39, 0xac, 0xd2, 0xae,
    0x6c, 0x5e, 0x3f, 0xb0, 0x6c, 0x2f, 0x90, 0x98, 0x72, 0xa9, 0x44, 0x9e, 0xa8, 0x54, 0xcc, 0xd2,
];
/// SHA-256 of `dclutch/release/direct-adapter-v2`.
///
/// This identifies the semantic adapter code coordinate. It is not a claim
/// that any particular deployed ELF has passed a checked release manifest.
pub const DIRECT_ADAPTER_RELEASE_ID_V2: [u8; 32] = [
    0x9d, 0x8a, 0xd1, 0x7f, 0xd1, 0x38, 0x95, 0x6e, 0x55, 0x44, 0xde, 0x0d, 0xad, 0x5d, 0x16, 0x31,
    0xd5, 0xf6, 0xee, 0x52, 0x54, 0x9f, 0xcf, 0x91, 0x6d, 0xe8, 0xb4, 0xa6, 0x18, 0x01, 0x8e, 0x23,
];
/// SHA-256 of `dclutch/capacity/direct-n2-n16-v2`.
pub const DIRECT_CAPACITY_PROFILE_ID_V2: [u8; 32] = [
    0x6f, 0x86, 0xe9, 0x93, 0x33, 0x5d, 0x11, 0x68, 0x20, 0xf0, 0x88, 0x5b, 0x19, 0x7c, 0x3e, 0x73,
    0x68, 0x03, 0xab, 0x24, 0xc0, 0x27, 0xc5, 0x77, 0x4b, 0xff, 0xa2, 0x78, 0xe4, 0xf3, 0x6f, 0xbc,
];
/// SHA-256 of `dclutch/schema/direct-child-set-v2`.
pub const DIRECT_CHILD_SCHEMA_ID_V2: [u8; 32] = [
    0x04, 0xec, 0xcd, 0x25, 0x53, 0x53, 0x88, 0x47, 0x1f, 0xd7, 0xd6, 0x28, 0xb6, 0xdc, 0xbd, 0x71,
    0x98, 0xa5, 0xba, 0xb5, 0x7c, 0xb3, 0xcf, 0xc9, 0x56, 0xfe, 0x25, 0xe6, 0x70, 0xb7, 0x6d, 0x1a,
];
/// SHA-256 of `dclutch/derivation/direct-pdas-v2`.
pub const DIRECT_CHILD_DERIVATION_ID_V2: [u8; 32] = [
    0xf6, 0xaf, 0x89, 0x52, 0xe5, 0xe3, 0xe0, 0xbc, 0xa8, 0x00, 0xac, 0x7c, 0xa3, 0x13, 0xff, 0x1d,
    0x36, 0x3e, 0x81, 0x39, 0xf5, 0x64, 0x55, 0xb4, 0x4a, 0x1b, 0x07, 0x96, 0x82, 0x0f, 0x77, 0xda,
];

/// Adapter projection of the uniquely selected manifest entry for Direct V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCapabilitySelectionV2 {
    /// Capability kind coordinate.
    pub kind_id: [u8; 32],
    /// Semantic adapter-release coordinate.
    pub release_id: [u8; 32],
    /// Exact canonical venue-policy content digest.
    pub config_id: [u8; 32],
    /// Measured N=2..16 capacity coordinate.
    pub capacity_profile_id: [u8; 32],
    /// Direct child-set schema coordinate.
    pub child_schema_id: [u8; 32],
    /// Direct PDA-derivation coordinate.
    pub child_derivation_id: [u8; 32],
    /// Whether activation is required during Market founding.
    pub required_at_founding: bool,
    /// Activation deadline, which is zero for founding-required Direct.
    pub activation_deadline_slot: u64,
    /// Number of manifest dependencies; Direct V2 has none.
    pub dependency_count: u8,
    /// Checked native-lamport funding total.
    pub native_funding_total: u64,
    /// Checked Realm-collateral funding total.
    pub realm_funding_total: u64,
    /// Whether a Realm-collateral funding binding is present.
    pub has_realm_funding_binding: bool,
}

/// Authenticate the exact stateless Direct V2 manifest coordinate.
///
/// User-funded replay/order rent is intentionally outside capability funding;
/// therefore every manifest funding compartment is canonically zero and no
/// Realm funding binding or dependency is admitted.
pub fn validate_direct_capability_selection_v2(
    selection: DirectCapabilitySelectionV2,
    expected_fee_config: [u8; 32],
) -> Result<()> {
    if selection.kind_id != DIRECT_CAPABILITY_KIND_ID_V2
        || selection.release_id != DIRECT_ADAPTER_RELEASE_ID_V2
        || selection.config_id != expected_fee_config
        || selection.capacity_profile_id != DIRECT_CAPACITY_PROFILE_ID_V2
        || selection.child_schema_id != DIRECT_CHILD_SCHEMA_ID_V2
        || selection.child_derivation_id != DIRECT_CHILD_DERIVATION_ID_V2
        || !selection.required_at_founding
        || selection.activation_deadline_slot != 0
        || selection.dependency_count != 0
        || selection.native_funding_total != 0
        || selection.realm_funding_total != 0
        || selection.has_realm_funding_binding
    {
        return Err(Error::DirectCapabilityUnauthorized);
    }
    nonzero(&expected_fee_config)?;
    Ok(())
}

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
    /// Manifest entry did not select the exact Direct V2 capability coordinate.
    DirectCapabilityUnauthorized,
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
    /// The canonical RentCredit record, binding, or exact close-credit plan refused.
    RentCreditContract(dclutch_rent_contract::Error),
}

impl From<dclutch_rent_contract::Error> for Error {
    fn from(error: dclutch_rent_contract::Error) -> Self {
        Self::RentCreditContract(error)
    }
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
