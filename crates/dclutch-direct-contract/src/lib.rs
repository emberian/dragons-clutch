#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout, SDK-free direct signed-intent settlement checks.
//!
//! An adapter verifies a signature over [`DirectIntentV1::signed_preimage`] and
//! supplies [`OwnerAuthorization`]. This crate checks only the resulting
//! economic contract; it has no SVM, token, hashing, account, or allocation
//! dependency. An untrusted matcher may choose compatible fill and price, never
//! alter the signed Market binding, maker, nonce, time interval, or limits.

use core::convert::TryInto;
use dclutch_realm_contract::{PositionV1, MAX_OUTCOMES, MIN_OUTCOMES};

/// Canonical signed-intent preimage magic.
pub const DIRECT_INTENT_MAGIC: [u8; 8] = *b"DCLTDIR1";
/// Signed-intent schema version.
pub const DIRECT_INTENT_SCHEMA_VERSION: u16 = 1;
/// Exact signed-intent preimage width.
pub const DIRECT_INTENT_BYTES: usize = 168;
/// Canonical replay-state magic.
pub const INTENT_STATE_MAGIC: [u8; 8] = *b"DCLTDST1";
/// Replay-state schema version.
pub const INTENT_STATE_SCHEMA_VERSION: u16 = 1;
/// Exact replay-state width.
pub const INTENT_STATE_BYTES: usize = 104;
/// Canonical fee-policy magic.
pub const VENUE_FEE_POLICY_MAGIC: [u8; 8] = *b"DCLTFEE1";
/// Fee-policy schema version.
pub const VENUE_FEE_POLICY_SCHEMA_VERSION: u16 = 1;
/// Exact venue-fee-policy width.
pub const VENUE_FEE_POLICY_BYTES: usize = 120;
/// Exact scaled integer price denominator.
pub const PRICE_SCALE: u64 = 1_000_000;
/// Fee rate denominator.
pub const FEE_BASIS_POINTS_DENOMINATOR: u64 = 10_000;

const SIDE: usize = 10;
const OUTCOME: usize = 11;
const INTENT_RESERVED: usize = 12;
const MARKET: usize = 16;
const GENERATION: usize = 48;
const MAKER: usize = 56;
const NONCE: usize = 88;
const START: usize = 96;
const END: usize = 104;
const MAX_FILL: usize = 112;
const LIMIT: usize = 120;
const FEE_CONFIG: usize = 128;
const INTENT_FEE_BPS: usize = 160;
const INTENT_FEE_RESERVED: usize = 162;
const STATE_STATUS: usize = 10;
const STATE_RESERVED: usize = 11;
const STATE_FILLED: usize = 96;
const FEE_BPS: usize = 10;
const FEE_RESERVED: usize = 12;
const FEE_RECIPIENT: usize = 16;

/// Explicit refusal from a direct-contract parser or pure settlement checker.
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
    /// Required authority identity was all zero.
    ZeroIdentifier,
    /// Intent side was unknown.
    UnknownSide,
    /// Intent-state status was unknown.
    UnknownIntentStatus,
    /// A slot interval was inverted.
    InvalidSlotInterval,
    /// A required positive quantity was zero.
    ZeroQuantity,
    /// A limit price exceeded a one-collateral payout.
    InvalidLimitPrice,
    /// Fee exceeded 10,000 basis points.
    InvalidFeeRate,
    /// Fee policy was not the adapter-authenticated Market-selected config signed by each maker.
    VenueUnauthorized,
    /// Adapter authorization did not equal signed maker and Position owner.
    OwnerUnauthorized,
    /// Position Market or generation differed from its signed intent.
    PositionMarketMismatch,
    /// Position owner differed from signed maker.
    PositionOwnerMismatch,
    /// State locator did not equal signed intent locator.
    StateLocatorMismatch,
    /// State fill exceeded signed capacity.
    StateOverfilled,
    /// State was cancelled.
    IntentCancelled,
    /// Slot was outside signed inclusive validity interval.
    IntentExpired,
    /// Fill was zero or exceeded remaining signed capacity.
    InvalidFill,
    /// Inputs were not complementary sides on the same outcome.
    IncompatibleSides,
    /// Matcher price was outside signed limits.
    PriceIncompatible,
    /// A scaled quote was not an exact number of collateral atoms.
    NonIntegralQuote,
    /// Exact checked arithmetic overflowed.
    ArithmeticOverflow,
    /// A Position lacked an outcome balance.
    InsufficientPositionBalance,
    /// An owner, Position, or intent locator was repeated where it must be distinct.
    Alias,
    /// Complementary buy array was not exact canonical outcome order.
    NonCanonicalComplement,
    /// Complementary bids did not fund exactly one collateral atom per set.
    SplitFundingMismatch,
    /// Active Position width was outside the selected profile.
    InvalidOutcomeWidth,
    /// Selected outcome is outside the active Position width.
    InvalidOutcome,
}

/// Result alias for this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Signed direction of a direct intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Side {
    /// Buy one selected outcome.
    Buy = 0,
    /// Sell one selected outcome.
    Sell = 1,
}
impl Side {
    const fn decode(value: u8) -> Result<Self> { match value { 0 => Ok(Self::Buy), 1 => Ok(Self::Sell), _ => Err(Error::UnknownSide) } }
    const fn byte(self) -> u8 { match self { Self::Buy => 0, Self::Sell => 1 } }
}

/// Input facts for a signed direct intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIntentInput {
    /// Exact nonzero Market identity used by native Positions.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact nonzero maker identity.
    pub maker: [u8; 32],
    /// Maker-local replay nonce.
    pub nonce: u64,
    /// Inclusive starting slot.
    pub valid_from_slot: u64,
    /// Inclusive expiry slot.
    pub valid_through_slot: u64,
    /// Signed buy or sell side.
    pub side: Side,
    /// Signed selected canonical outcome.
    pub outcome: u8,
    /// Exact aggregate capacity for all partial fills.
    pub max_fill: u64,
    /// Signed price in [`PRICE_SCALE`] units per claim atom.
    pub limit_price: u64,
    /// Nonzero identity of the Market-selected venue fee configuration/release.
    pub fee_config: [u8; 32],
    /// Exact fee rate to which this maker consents.
    pub fee_basis_points: u16,
}

/// Fixed-layout canonical direct signed-intent facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIntentV1 {
    market: [u8; 32],
    generation: u64,
    maker: [u8; 32],
    nonce: u64,
    from: u64,
    through: u64,
    side: Side,
    outcome: u8,
    max_fill: u64,
    limit: u64,
    fee_config: [u8; 32],
    fee_bps: u16,
}
impl DirectIntentV1 {
    /// Validate one direct intent.
    pub fn new(input: DirectIntentInput) -> Result<Self> {
        nonzero(&input.market)?; nonzero(&input.maker)?;
        if input.valid_from_slot > input.valid_through_slot { return Err(Error::InvalidSlotInterval); }
        if input.max_fill == 0 { return Err(Error::ZeroQuantity); }
        if input.limit_price > PRICE_SCALE { return Err(Error::InvalidLimitPrice); }
        nonzero(&input.fee_config)?;
        if u64::from(input.fee_basis_points) > FEE_BASIS_POINTS_DENOMINATOR { return Err(Error::InvalidFeeRate); }
        Ok(Self { market: input.market, generation: input.generation, maker: input.maker, nonce: input.nonce, from: input.valid_from_slot, through: input.valid_through_slot, side: input.side, outcome: input.outcome, max_fill: input.max_fill, limit: input.limit_price, fee_config: input.fee_config, fee_bps: input.fee_basis_points })
    }
    /// Decode the one canonical sequence which must be signed.
    pub fn decode_signed_preimage(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != DIRECT_INTENT_BYTES { return Err(Error::InvalidLength); }
        if array::<8>(bytes, 0)? != DIRECT_INTENT_MAGIC { return Err(Error::InvalidMagic); }
        if u16::from_le_bytes(array(bytes, 8)?) != DIRECT_INTENT_SCHEMA_VERSION { return Err(Error::UnsupportedSchema); }
        zeros(bytes, INTENT_RESERVED, 4)?; zeros(bytes, INTENT_FEE_RESERVED, 6)?;
        Self::new(DirectIntentInput { market: array(bytes, MARKET)?, generation: u64::from_le_bytes(array(bytes, GENERATION)?), maker: array(bytes, MAKER)?, nonce: u64::from_le_bytes(array(bytes, NONCE)?), valid_from_slot: u64::from_le_bytes(array(bytes, START)?), valid_through_slot: u64::from_le_bytes(array(bytes, END)?), side: Side::decode(one(bytes, SIDE)?)?, outcome: one(bytes, OUTCOME)?, max_fill: u64::from_le_bytes(array(bytes, MAX_FILL)?), limit_price: u64::from_le_bytes(array(bytes, LIMIT)?), fee_config: array(bytes, FEE_CONFIG)?, fee_basis_points: u16::from_le_bytes(array(bytes, INTENT_FEE_BPS)?) })
    }
    /// Return exact canonical bytes for external signing.
    pub fn signed_preimage(self) -> [u8; DIRECT_INTENT_BYTES] {
        let mut out = [0; DIRECT_INTENT_BYTES];
        put(&mut out, 0, &DIRECT_INTENT_MAGIC); put(&mut out, 8, &DIRECT_INTENT_SCHEMA_VERSION.to_le_bytes());
        out[SIDE] = self.side.byte(); out[OUTCOME] = self.outcome;
        put(&mut out, MARKET, &self.market); put(&mut out, GENERATION, &self.generation.to_le_bytes()); put(&mut out, MAKER, &self.maker); put(&mut out, NONCE, &self.nonce.to_le_bytes()); put(&mut out, START, &self.from.to_le_bytes()); put(&mut out, END, &self.through.to_le_bytes()); put(&mut out, MAX_FILL, &self.max_fill.to_le_bytes()); put(&mut out, LIMIT, &self.limit.to_le_bytes()); put(&mut out, FEE_CONFIG, &self.fee_config); put(&mut out, INTENT_FEE_BPS, &self.fee_bps.to_le_bytes()); out
    }
    /// Return Market identity.
    pub const fn market(&self) -> &[u8; 32] { &self.market }
    /// Return Market generation.
    pub const fn generation(&self) -> u64 { self.generation }
    /// Return maker identity.
    pub const fn maker(&self) -> &[u8; 32] { &self.maker }
    /// Return nonce.
    pub const fn nonce(&self) -> u64 { self.nonce }
    /// Return signed side.
    pub const fn side(&self) -> Side { self.side }
    /// Return signed outcome.
    pub const fn outcome(&self) -> u8 { self.outcome }
    /// Return signed maximum aggregate fill.
    pub const fn max_fill(&self) -> u64 { self.max_fill }
    /// Return signed price limit.
    pub const fn limit_price(&self) -> u64 { self.limit }
    /// Return the signed venue fee configuration identity.
    pub const fn fee_config(&self) -> &[u8; 32] { &self.fee_config }
    /// Return the signed venue fee rate.
    pub const fn fee_basis_points(&self) -> u16 { self.fee_bps }
}

/// Adapter-attested authorization for precisely one authenticated owner.
///
/// The adapter must obtain this from verified signature or direct transaction
/// authority; this type intentionally does not purport to perform cryptography.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerAuthorization {
    /// Authenticated owner bytes.
    pub owner: [u8; 32],
}

/// Fixed-layout persistent cancellation and partial-fill replay state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentStateV1 { market: [u8; 32], generation: u64, maker: [u8; 32], nonce: u64, filled: u64, cancelled: bool }
impl IntentStateV1 {
    /// Construct sole initial open state for an intent.
    pub const fn open(intent: DirectIntentV1) -> Self { Self { market: intent.market, generation: intent.generation, maker: intent.maker, nonce: intent.nonce, filled: 0, cancelled: false } }
    /// Decode one canonical replay-state record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != INTENT_STATE_BYTES { return Err(Error::InvalidLength); }
        if array::<8>(bytes, 0)? != INTENT_STATE_MAGIC { return Err(Error::InvalidMagic); }
        if u16::from_le_bytes(array(bytes, 8)?) != INTENT_STATE_SCHEMA_VERSION { return Err(Error::UnsupportedSchema); }
        zeros(bytes, STATE_RESERVED, 5)?;
        let cancelled = match one(bytes, STATE_STATUS)? { 0 => false, 1 => true, _ => return Err(Error::UnknownIntentStatus) };
        let state = Self { market: array(bytes, MARKET)?, generation: u64::from_le_bytes(array(bytes, GENERATION)?), maker: array(bytes, MAKER)?, nonce: u64::from_le_bytes(array(bytes, NONCE)?), filled: u64::from_le_bytes(array(bytes, STATE_FILLED)?), cancelled };
        nonzero(&state.market)?; nonzero(&state.maker)?; Ok(state)
    }
    /// Encode one canonical replay-state record.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != INTENT_STATE_BYTES { return Err(Error::OutputLength); }
        nonzero(&self.market)?; nonzero(&self.maker)?; output.fill(0);
        put(output, 0, &INTENT_STATE_MAGIC); put(output, 8, &INTENT_STATE_SCHEMA_VERSION.to_le_bytes());
        if let Some(status) = output.get_mut(STATE_STATUS) { *status = if self.cancelled { 1 } else { 0 }; }
        put(output, MARKET, &self.market); put(output, GENERATION, &self.generation.to_le_bytes()); put(output, MAKER, &self.maker); put(output, NONCE, &self.nonce.to_le_bytes()); put(output, STATE_FILLED, &self.filled.to_le_bytes()); Ok(())
    }
    /// Cancel this state, requiring exact maker authorization.
    pub fn cancel(self, intent: DirectIntentV1, auth: OwnerAuthorization) -> Result<Self> { self.for_intent(intent)?; authorized(intent, auth)?; Ok(Self { cancelled: true, ..self }) }
    /// Return aggregate consumed fill.
    pub const fn filled(&self) -> u64 { self.filled }
    /// Return cancellation state.
    pub const fn is_cancelled(&self) -> bool { self.cancelled }
    fn for_intent(&self, intent: DirectIntentV1) -> Result<()> {
        if self.market != intent.market || self.generation != intent.generation || self.maker != intent.maker || self.nonce != intent.nonce { return Err(Error::StateLocatorMismatch); }
        if self.filled > intent.max_fill { return Err(Error::StateOverfilled); } Ok(())
    }
    fn consume(self, intent: DirectIntentV1, slot: u64, fill: u64) -> Result<Self> {
        self.for_intent(intent)?; if self.cancelled { return Err(Error::IntentCancelled); }
        if slot < intent.from || slot > intent.through { return Err(Error::IntentExpired); }
        if fill == 0 || fill > intent.max_fill.checked_sub(self.filled).ok_or(Error::StateOverfilled)? { return Err(Error::InvalidFill); }
        Ok(Self { filled: self.filled.checked_add(fill).ok_or(Error::ArithmeticOverflow)?, ..self })
    }
}

/// Adapter-authenticated selection of one Market-local venue configuration.
///
/// A composing adapter may construct this only after reading the immutable
/// Market-selected config/release. It is the trust boundary that prevents a
/// caller from inventing a fee policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketVenueAuthorization {
    /// Exact Market identity whose configuration was authenticated.
    pub market: [u8; 32],
    /// Exact immutable Market generation.
    pub generation: u64,
    /// Nonzero selected venue configuration/release identity.
    pub fee_config: [u8; 32],
}

/// Fixed-layout immutable local venue fee policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VenueFeePolicyV1 { market: [u8; 32], generation: u64, config: [u8; 32], recipient: [u8; 32], bps: u16 }
impl VenueFeePolicyV1 {
    /// Validate one local policy.
    pub fn new(market: [u8; 32], generation: u64, config: [u8; 32], recipient: [u8; 32], fee_basis_points: u16) -> Result<Self> { nonzero(&market)?; nonzero(&config)?; nonzero(&recipient)?; if u64::from(fee_basis_points) > FEE_BASIS_POINTS_DENOMINATOR { return Err(Error::InvalidFeeRate); } Ok(Self { market, generation, config, recipient, bps: fee_basis_points }) }
    /// Decode one canonical local policy record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != VENUE_FEE_POLICY_BYTES { return Err(Error::InvalidLength); }
        if array::<8>(bytes, 0)? != VENUE_FEE_POLICY_MAGIC { return Err(Error::InvalidMagic); }
        if u16::from_le_bytes(array(bytes, 8)?) != VENUE_FEE_POLICY_SCHEMA_VERSION { return Err(Error::UnsupportedSchema); }
        zeros(bytes, FEE_RESERVED, 4)?; Self::new(array(bytes, FEE_RECIPIENT)?, u64::from_le_bytes(array(bytes, 48)?), array(bytes, 56)?, array(bytes, 88)?, u16::from_le_bytes(array(bytes, FEE_BPS)?))
    }
    /// Encode one canonical local policy record.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != VENUE_FEE_POLICY_BYTES { return Err(Error::OutputLength); }
        output.fill(0); put(output, 0, &VENUE_FEE_POLICY_MAGIC); put(output, 8, &VENUE_FEE_POLICY_SCHEMA_VERSION.to_le_bytes()); put(output, FEE_BPS, &self.bps.to_le_bytes()); put(output, FEE_RECIPIENT, &self.market); put(output, 48, &self.generation.to_le_bytes()); put(output, 56, &self.config); put(output, 88, &self.recipient); Ok(())
    }
    /// Return local fee recipient.
    pub const fn recipient(&self) -> &[u8; 32] { &self.recipient }
    /// Return fee rate in basis points.
    pub const fn fee_basis_points(&self) -> u16 { self.bps }
}

/// The sole integer rounding boundary: floor one fee after exact quote settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeRounding {
    /// `floor(gross_collateral * bps / 10_000)`.
    Floor,
}

/// Pure output for one ordinary outcome transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinarySettlement<const N: usize> {
    /// Seller replacement Position.
    pub seller_position: PositionV1<N>,
    /// Buyer replacement Position.
    pub buyer_position: PositionV1<N>,
    /// Seller replacement replay state.
    pub seller_state: IntentStateV1,
    /// Buyer replacement replay state.
    pub buyer_state: IntentStateV1,
    /// Selected outcome debit and credit.
    pub outcome_quantity: u64,
    /// Seller collateral transfer.
    pub seller_collateral_credit: u64,
    /// Buyer exact quote debit before fee.
    pub buyer_gross_collateral_debit: u64,
    /// Fee transfer to local venue recipient.
    pub venue_fee_transfer: u64,
    /// Buyer total collateral debit.
    pub buyer_total_collateral_debit: u64,
}

/// Inputs to an ordinary signed-intent transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinaryMatch<const N: usize> {
    /// Current adapter-provided slot.
    pub slot: u64,
    /// Signed seller ask.
    pub seller_intent: DirectIntentV1,
    /// Signed buyer bid.
    pub buyer_intent: DirectIntentV1,
    /// Seller replay state.
    pub seller_state: IntentStateV1,
    /// Buyer replay state.
    pub buyer_state: IntentStateV1,
    /// Seller adapter authorization.
    pub seller_authorization: OwnerAuthorization,
    /// Buyer adapter authorization.
    pub buyer_authorization: OwnerAuthorization,
    /// Seller Position.
    pub seller_position: PositionV1<N>,
    /// Buyer Position.
    pub buyer_position: PositionV1<N>,
    /// Matcher-selected fill.
    pub fill: u64,
    /// Matcher-selected scaled execution price.
    pub execution_price: u64,
    /// Immutable local fee policy.
    pub fee_policy: VenueFeePolicyV1,
    /// Adapter-authenticated Market selection for the fee configuration.
    pub venue_authorization: MarketVenueAuthorization,
}

/// Check an atomic ordinary transfer and return exact Position, collateral, fee, and replay effects.
pub fn settle_ordinary<const N: usize>(input: OrdinaryMatch<N>) -> Result<OrdinarySettlement<N>> {
    width(N)?;
    let ask = input.seller_intent; let bid = input.buyer_intent;
    if ask.side != Side::Sell || bid.side != Side::Buy || ask.market != bid.market || ask.generation != bid.generation || ask.outcome != bid.outcome { return Err(Error::IncompatibleSides); }
    if ask.maker == bid.maker { return Err(Error::Alias); }
    authorized(ask, input.seller_authorization)?; authorized(bid, input.buyer_authorization)?;
    position_matches(input.seller_position, ask)?; position_matches(input.buyer_position, bid)?;
    venue_authorized(ask, input.fee_policy, input.venue_authorization)?;
    venue_authorized(bid, input.fee_policy, input.venue_authorization)?;
    if input.execution_price < ask.limit || input.execution_price > bid.limit { return Err(Error::PriceIncompatible); }
    let seller_state = input.seller_state.consume(ask, input.slot, input.fill)?;
    let buyer_state = input.buyer_state.consume(bid, input.slot, input.fill)?;
    let gross = quote(input.fill, input.execution_price)?; let fee = fee(gross, input.fee_policy)?; let total = gross.checked_add(fee).ok_or(Error::ArithmeticOverflow)?;
    let outcome = usize::from(ask.outcome); if outcome >= N { return Err(Error::InvalidOutcome); }
    let mut seller_position = input.seller_position; seller_position.debit_outcome(outcome, input.fill).map_err(position_error)?;
    let mut buyer_position = input.buyer_position; buyer_position.credit_outcome(outcome, input.fill).map_err(position_error)?;
    Ok(OrdinarySettlement { seller_position, buyer_position, seller_state, buyer_state, outcome_quantity: input.fill, seller_collateral_credit: gross, buyer_gross_collateral_debit: gross, venue_fee_transfer: fee, buyer_total_collateral_debit: total })
}

/// Pure output for an exhaustive complementary-buy complete-set split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitSettlement<const N: usize> {
    /// Replacement buyer Positions, canonical outcome order.
    pub buyer_positions: [PositionV1<N>; N],
    /// Replacement buyer replay states, canonical outcome order.
    pub buyer_states: [IntentStateV1; N],
    /// Buyer gross quote debits, canonical outcome order.
    pub buyer_gross_collateral_debits: [u64; N],
    /// Buyer local-fee debits, canonical outcome order.
    pub buyer_fee_debits: [u64; N],
    /// Exact collateral transfer to Market vault, equal to fill.
    pub market_vault_collateral_credit: u64,
    /// Exact aggregate fee transfer to venue recipient.
    pub venue_fee_transfer: u64,
}

/// Inputs to one exhaustive complementary-buy complete-set split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementaryBuyMatch<const N: usize> {
    /// Current adapter-provided slot.
    pub slot: u64,
    /// Buy intents in canonical outcome order.
    pub buyer_intents: [DirectIntentV1; N],
    /// Matching replay states in canonical outcome order.
    pub buyer_states: [IntentStateV1; N],
    /// Matching adapter authorizations in canonical outcome order.
    pub buyer_authorizations: [OwnerAuthorization; N],
    /// Matching Positions in canonical outcome order.
    pub buyer_positions: [PositionV1<N>; N],
    /// Common matcher-selected fill.
    pub fill: u64,
    /// Scaled prices summing exactly to [`PRICE_SCALE`].
    pub execution_prices: [u64; N],
    /// Immutable local fee policy.
    pub fee_policy: VenueFeePolicyV1,
    /// Adapter-authenticated Market selection for the fee configuration.
    pub venue_authorization: MarketVenueAuthorization,
}

/// Exact output from an exhaustive complementary-sell complete-set merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeSettlement<const N: usize> {
    /// Seller replacement Position after the complete-set debit.
    pub seller_position: PositionV1<N>,
    /// Replacement replay states in canonical outcome order.
    pub seller_states: [IntentStateV1; N],
    /// Exact collateral debit from the Market vault.
    pub market_vault_collateral_debit: u64,
    /// Exact collateral credit released to seller after the fee.
    pub seller_collateral_credit: u64,
    /// Exact local venue fee transfer.
    pub venue_fee_transfer: u64,
}

/// Inputs to one exhaustive complementary-sell atomic merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementarySellMatch<const N: usize> {
    /// Current adapter-provided slot.
    pub slot: u64,
    /// Sell intents in canonical outcome order from one owner.
    pub seller_intents: [DirectIntentV1; N],
    /// Matching replay states in canonical outcome order.
    pub seller_states: [IntentStateV1; N],
    /// Matching adapter authorizations in canonical outcome order.
    pub seller_authorizations: [OwnerAuthorization; N],
    /// One seller Position holding the complete set.
    pub seller_position: PositionV1<N>,
    /// Common matcher-selected fill.
    pub fill: u64,
    /// Scaled prices summing to PRICE_SCALE and meeting each sell limit.
    pub execution_prices: [u64; N],
    /// Immutable local fee policy.
    pub fee_policy: VenueFeePolicyV1,
    /// Adapter-authenticated Market selection for the fee configuration.
    pub venue_authorization: MarketVenueAuthorization,
}

/// Check an atomic complete-set merge and return exact seller, vault, fee, and replay effects.
pub fn settle_merge<const N: usize>(input: ComplementarySellMatch<N>) -> Result<MergeSettlement<N>> {
    width(N)?; if input.fill == 0 { return Err(Error::ZeroQuantity); }
    let first = *input.seller_intents.first().ok_or(Error::InvalidOutcomeWidth)?;
    let mut states = input.seller_states; let mut price_sum = 0_u64;
    let mut nonces = [0_u64; N]; let mut nonce_count = 0usize;
    for (index, (((intent, state), authorization), price)) in input.seller_intents.iter().zip(states.iter_mut()).zip(input.seller_authorizations.iter()).zip(input.execution_prices.iter()).enumerate() {
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        if intent.side != Side::Sell || intent.market != first.market || intent.generation != first.generation || intent.maker != first.maker || intent.outcome != expected { return Err(Error::NonCanonicalComplement); }
        if nonces.iter().take(nonce_count).any(|nonce| nonce == &intent.nonce) { return Err(Error::Alias); }
        if let Some(slot) = nonces.get_mut(nonce_count) { *slot = intent.nonce; } else { return Err(Error::ArithmeticOverflow); }
        nonce_count = nonce_count.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        authorized(*intent, *authorization)?; position_matches(input.seller_position, *intent)?;
        venue_authorized(*intent, input.fee_policy, input.venue_authorization)?;
        if *price < intent.limit { return Err(Error::PriceIncompatible); }
        price_sum = price_sum.checked_add(*price).ok_or(Error::ArithmeticOverflow)?;
        *state = state.consume(*intent, input.slot, input.fill)?;
    }
    if price_sum != PRICE_SCALE { return Err(Error::SplitFundingMismatch); }
    let mut seller_position = input.seller_position;
    seller_position.debit_complete_set(input.fill).map_err(position_error)?;
    let venue_fee_transfer = fee(input.fill, input.fee_policy)?;
    let seller_collateral_credit = input.fill.checked_sub(venue_fee_transfer).ok_or(Error::ArithmeticOverflow)?;
    Ok(MergeSettlement { seller_position, seller_states: states, market_vault_collateral_debit: input.fill, seller_collateral_credit, venue_fee_transfer })
}

/// Check an atomic complete-set split and return exact buyer, vault, fee, and replay effects.
pub fn settle_split<const N: usize>(input: ComplementaryBuyMatch<N>) -> Result<SplitSettlement<N>> {
    width(N)?; if input.fill == 0 { return Err(Error::ZeroQuantity); }
    let first = *input.buyer_intents.first().ok_or(Error::InvalidOutcomeWidth)?;
    let mut positions = input.buyer_positions; let mut states = input.buyer_states;
    let mut gross = [0; N]; let mut fees = [0; N]; let mut price_sum = 0_u64; let mut fee_sum = 0_u64;
    let mut makers = [[0_u8; 32]; N]; let mut maker_count = 0usize;
    for (index, (((((intent, state), authorization), position), price), gross_slot)) in input.buyer_intents.iter().zip(states.iter_mut()).zip(input.buyer_authorizations.iter()).zip(positions.iter_mut()).zip(input.execution_prices.iter()).zip(gross.iter_mut()).enumerate() {
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        if intent.side != Side::Buy || intent.market != first.market || intent.generation != first.generation || intent.outcome != expected { return Err(Error::NonCanonicalComplement); }
        if makers.iter().take(maker_count).any(|maker| maker == &intent.maker) { return Err(Error::Alias); }
        if let Some(slot) = makers.get_mut(maker_count) { *slot = intent.maker; } else { return Err(Error::ArithmeticOverflow); }
        maker_count = maker_count.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        authorized(*intent, *authorization)?; position_matches(*position, *intent)?;
        venue_authorized(*intent, input.fee_policy, input.venue_authorization)?;
        if *price > intent.limit { return Err(Error::PriceIncompatible); }
        price_sum = price_sum.checked_add(*price).ok_or(Error::ArithmeticOverflow)?;
        *state = state.consume(*intent, input.slot, input.fill)?;
        *gross_slot = quote(input.fill, *price)?;
        let fee_slot = fees.get_mut(index).ok_or(Error::ArithmeticOverflow)?;
        *fee_slot = fee(*gross_slot, input.fee_policy)?;
        position.credit_outcome(index, input.fill).map_err(position_error)?;
        fee_sum = fee_sum.checked_add(*fee_slot).ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != PRICE_SCALE { return Err(Error::SplitFundingMismatch); }
    let mut gross_sum = 0_u64; for debit in gross { gross_sum = gross_sum.checked_add(debit).ok_or(Error::ArithmeticOverflow)?; }
    if gross_sum != input.fill { return Err(Error::SplitFundingMismatch); }
    Ok(SplitSettlement { buyer_positions: positions, buyer_states: states, buyer_gross_collateral_debits: gross, buyer_fee_debits: fees, market_vault_collateral_credit: input.fill, venue_fee_transfer: fee_sum })
}

fn authorized(intent: DirectIntentV1, auth: OwnerAuthorization) -> Result<()> { if auth.owner != intent.maker { Err(Error::OwnerUnauthorized) } else { Ok(()) } }
fn venue_authorized(intent: DirectIntentV1, policy: VenueFeePolicyV1, authorization: MarketVenueAuthorization) -> Result<()> {
    nonzero(&authorization.market)?; nonzero(&authorization.fee_config)?;
    if policy.market != authorization.market || policy.generation != authorization.generation || policy.config != authorization.fee_config || intent.market != authorization.market || intent.generation != authorization.generation || intent.fee_config != authorization.fee_config || intent.fee_bps != policy.bps { return Err(Error::VenueUnauthorized); }
    Ok(())
}
fn position_matches<const N: usize>(position: PositionV1<N>, intent: DirectIntentV1) -> Result<()> {
    if position.market() != intent.market() || position.generation() != intent.generation { return Err(Error::PositionMarketMismatch); }
    if position.owner() != intent.maker() { return Err(Error::PositionOwnerMismatch); } Ok(())
}
fn width(value: usize) -> Result<()> { if (MIN_OUTCOMES..=MAX_OUTCOMES).contains(&value) { Ok(()) } else { Err(Error::InvalidOutcomeWidth) } }
fn quote(quantity: u64, price: u64) -> Result<u64> { let product = quantity.checked_mul(price).ok_or(Error::ArithmeticOverflow)?; if product % PRICE_SCALE != 0 { Err(Error::NonIntegralQuote) } else { Ok(product / PRICE_SCALE) } }
fn fee(gross: u64, policy: VenueFeePolicyV1) -> Result<u64> { gross.checked_mul(u64::from(policy.bps)).ok_or(Error::ArithmeticOverflow).map(|value| value / FEE_BASIS_POINTS_DENOMINATOR) }
fn position_error(value: dclutch_realm_contract::Error) -> Error { match value { dclutch_realm_contract::Error::InsufficientBalance => Error::InsufficientPositionBalance, dclutch_realm_contract::Error::ArithmeticOverflow => Error::ArithmeticOverflow, _ => Error::InvalidOutcome } }
fn nonzero(value: &[u8; 32]) -> Result<()> { if value.iter().all(|item| *item == 0) { Err(Error::ZeroIdentifier) } else { Ok(()) } }
fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> { let end = offset.checked_add(N).ok_or(Error::InvalidLength)?; bytes.get(offset..end).ok_or(Error::InvalidLength)?.try_into().map_err(|_| Error::InvalidLength) }
fn one(bytes: &[u8], offset: usize) -> Result<u8> { bytes.get(offset).copied().ok_or(Error::InvalidLength) }
fn zeros(bytes: &[u8], offset: usize, len: usize) -> Result<()> { let end = offset.checked_add(len).ok_or(Error::InvalidLength)?; if bytes.get(offset..end).ok_or(Error::InvalidLength)?.iter().any(|item| *item != 0) { Err(Error::NonCanonicalReservedBytes) } else { Ok(()) } }
fn put(out: &mut [u8], offset: usize, value: &[u8]) { if let Some(dest) = out.get_mut(offset..offset.saturating_add(value.len())) { dest.copy_from_slice(value); } }

#[cfg(test)]
mod tests {
    use super::*;
    fn key(v: u8) -> [u8; 32] { [v; 32] }
    fn auth(v: u8) -> OwnerAuthorization { OwnerAuthorization { owner: key(v) } }
    fn policy(bps: u16) -> Result<VenueFeePolicyV1> { VenueFeePolicyV1::new(key(7), 3, key(8), key(99), bps) }
    fn venue() -> MarketVenueAuthorization { MarketVenueAuthorization { market: key(7), generation: 3, fee_config: key(8) } }
    fn order(maker: u8, side: Side, outcome: u8, limit: u64, nonce: u64) -> Result<DirectIntentV1> { order_fee(maker, side, outcome, limit, nonce, 0) }
    fn order_fee(maker: u8, side: Side, outcome: u8, limit: u64, nonce: u64, fee_basis_points: u16) -> Result<DirectIntentV1> { DirectIntentV1::new(DirectIntentInput { market: key(7), generation: 3, maker: key(maker), nonce, valid_from_slot: 10, valid_through_slot: 20, side, outcome, max_fill: 10, limit_price: limit, fee_config: key(8), fee_basis_points }) }
    fn position<const N: usize>(owner: u8, balances: [u64; N]) -> Result<PositionV1<N>> { PositionV1::new(key(7), key(owner), 3, balances).map_err(position_error) }

    #[test]
    fn canonical_preimage_binds_all_signed_facts() -> Result<()> {
        let value = order(1, Side::Buy, 1, 600_000, 4)?; let bytes = value.signed_preimage();
        assert_eq!(DirectIntentV1::decode_signed_preimage(&bytes)?, value);
        let mut hostile = bytes; hostile[12] = 1;
        assert_eq!(DirectIntentV1::decode_signed_preimage(&hostile), Err(Error::NonCanonicalReservedBytes));
        hostile = bytes; hostile[LIMIT..LIMIT + 8].copy_from_slice(&(PRICE_SCALE + 1).to_le_bytes());
        assert_eq!(DirectIntentV1::decode_signed_preimage(&hostile), Err(Error::InvalidLimitPrice)); Ok(())
    }
    #[test]
    fn ordinary_partial_transfer_is_conservative() -> Result<()> {
        let ask = order(1, Side::Sell, 0, 400_000, 1)?; let bid = order(2, Side::Buy, 0, 600_000, 2)?;
        let out = settle_ordinary(OrdinaryMatch { slot: 12, seller_intent: ask, buyer_intent: bid, seller_state: IntentStateV1::open(ask), buyer_state: IntentStateV1::open(bid), seller_authorization: auth(1), buyer_authorization: auth(2), seller_position: position(1, [7, 0])?, buyer_position: position(2, [0, 0])?, fill: 5, execution_price: 600_000, fee_policy: policy(0)?, venue_authorization: venue() })?;
        assert_eq!(out.seller_position.balances(), &[2, 0]); assert_eq!(out.buyer_position.balances(), &[5, 0]); assert_eq!(out.seller_collateral_credit, 3); assert_eq!(out.venue_fee_transfer, 0); assert_eq!(out.seller_state.filled(), 5); Ok(())
    }
    #[test]
    fn ordinary_refuses_expiry_replay_alias_bad_auth_and_fractional_quote() -> Result<()> {
        let ask = order(1, Side::Sell, 0, 400_000, 1)?; let bid = order(2, Side::Buy, 0, 600_000, 2)?;
        let base = OrdinaryMatch { slot: 12, seller_intent: ask, buyer_intent: bid, seller_state: IntentStateV1::open(ask), buyer_state: IntentStateV1::open(bid), seller_authorization: auth(1), buyer_authorization: auth(2), seller_position: position(1, [10, 0])?, buyer_position: position(2, [0, 0])?, fill: 1, execution_price: 500_000, fee_policy: policy(0)?, venue_authorization: venue() };
        assert_eq!(settle_ordinary(base), Err(Error::NonIntegralQuote));
        let mut expired = base; expired.execution_price = 600_000; expired.slot = 21; assert_eq!(settle_ordinary(expired), Err(Error::IntentExpired));
        let mut bad_auth = base; bad_auth.execution_price = 600_000; bad_auth.buyer_authorization = auth(3); assert_eq!(settle_ordinary(bad_auth), Err(Error::OwnerUnauthorized));
        let mut replay = base; replay.execution_price = 600_000; replay.seller_state = IntentStateV1 { filled: 11, ..IntentStateV1::open(ask) }; assert_eq!(settle_ordinary(replay), Err(Error::StateOverfilled));
        let self_bid = order(1, Side::Buy, 0, PRICE_SCALE, 3)?; let alias = OrdinaryMatch { buyer_intent: self_bid, buyer_state: IntentStateV1::open(self_bid), buyer_authorization: auth(1), buyer_position: position(1, [0, 0])?, ..base }; assert_eq!(settle_ordinary(alias), Err(Error::Alias)); Ok(())
    }
    #[test]
    fn cancellation_is_canonical_and_terminal() -> Result<()> {
        let bid = order(1, Side::Buy, 0, PRICE_SCALE, 1)?; let cancelled = IntentStateV1::open(bid).cancel(bid, auth(1))?;
        let mut bytes = [0; INTENT_STATE_BYTES]; cancelled.encode(&mut bytes)?; assert_eq!(IntentStateV1::decode(&bytes)?, cancelled);
        let ask = order(2, Side::Sell, 0, PRICE_SCALE, 2)?;
        assert_eq!(settle_ordinary(OrdinaryMatch { slot: 12, seller_intent: ask, buyer_intent: bid, seller_state: IntentStateV1::open(ask), buyer_state: cancelled, seller_authorization: auth(2), buyer_authorization: auth(1), seller_position: position(2, [1, 0])?, buyer_position: position(1, [0, 0])?, fill: 1, execution_price: PRICE_SCALE, fee_policy: policy(0)?, venue_authorization: venue() }), Err(Error::IntentCancelled)); Ok(())
    }
    #[test]
    fn canonical_complementary_buys_fund_exact_complete_set() -> Result<()> {
        let a = order(1, Side::Buy, 0, 500_000, 1)?; let b = order(2, Side::Buy, 1, 500_000, 2)?;
        let out = settle_split(ComplementaryBuyMatch { slot: 12, buyer_intents: [a, b], buyer_states: [IntentStateV1::open(a), IntentStateV1::open(b)], buyer_authorizations: [auth(1), auth(2)], buyer_positions: [position(1, [0, 0])?, position(2, [0, 0])?], fill: 10, execution_prices: [500_000, 500_000], fee_policy: policy(0)?, venue_authorization: venue() })?;
        assert_eq!(out.buyer_positions[0].balances(), &[10, 0]); assert_eq!(out.buyer_positions[1].balances(), &[0, 10]); assert_eq!(out.buyer_gross_collateral_debits, [5, 5]); assert_eq!(out.market_vault_collateral_credit, 10); Ok(())
    }
    #[test]
    fn canonical_complementary_sells_merge_and_release_only_vault_collateral() -> Result<()> {
        let a = order_fee(1, Side::Sell, 0, 500_000, 1, 1_000)?; let b = order_fee(1, Side::Sell, 1, 500_000, 2, 1_000)?;
        let out = settle_merge(ComplementarySellMatch { slot: 12, seller_intents: [a, b], seller_states: [IntentStateV1::open(a), IntentStateV1::open(b)], seller_authorizations: [auth(1), auth(1)], seller_position: position(1, [10, 10])?, fill: 10, execution_prices: [500_000, 500_000], fee_policy: policy(1_000)?, venue_authorization: venue() })?;
        assert_eq!(out.seller_position.balances(), &[0, 0]); assert_eq!(out.market_vault_collateral_debit, 10); assert_eq!(out.seller_collateral_credit, 9); assert_eq!(out.venue_fee_transfer, 1); Ok(())
    }
    #[test]
    fn venue_config_must_be_signed_and_adapter_authenticated() -> Result<()> {
        let ask = order(1, Side::Sell, 0, PRICE_SCALE, 1)?; let bid = order(2, Side::Buy, 0, PRICE_SCALE, 2)?;
        let mut bad_venue = venue(); bad_venue.fee_config = key(9);
        assert_eq!(settle_ordinary(OrdinaryMatch { slot: 12, seller_intent: ask, buyer_intent: bid, seller_state: IntentStateV1::open(ask), buyer_state: IntentStateV1::open(bid), seller_authorization: auth(1), buyer_authorization: auth(2), seller_position: position(1, [1, 0])?, buyer_position: position(2, [0, 0])?, fill: 1, execution_price: PRICE_SCALE, fee_policy: policy(0)?, venue_authorization: bad_venue }), Err(Error::VenueUnauthorized)); Ok(())
    }
    #[test]
    fn complement_refuses_alias_missing_outcome_and_nonintegral_funding() -> Result<()> {
        let a = order(1, Side::Buy, 0, PRICE_SCALE, 1)?; let b = order(1, Side::Buy, 1, PRICE_SCALE, 2)?;
        let base = ComplementaryBuyMatch { slot: 12, buyer_intents: [a, b], buyer_states: [IntentStateV1::open(a), IntentStateV1::open(b)], buyer_authorizations: [auth(1), auth(1)], buyer_positions: [position(1, [0, 0])?, position(1, [0, 0])?], fill: 10, execution_prices: [500_000, 500_000], fee_policy: policy(0)?, venue_authorization: venue() };
        assert_eq!(settle_split(base), Err(Error::Alias));
        let c = order(2, Side::Buy, 0, PRICE_SCALE, 3)?; let missing = ComplementaryBuyMatch { buyer_intents: [a, c], buyer_states: [IntentStateV1::open(a), IntentStateV1::open(c)], buyer_authorizations: [auth(1), auth(2)], buyer_positions: [position(1, [0, 0])?, position(2, [0, 0])?], ..base }; assert_eq!(settle_split(missing), Err(Error::NonCanonicalComplement));
        let b2 = order(2, Side::Buy, 1, PRICE_SCALE, 2)?; let fractional = ComplementaryBuyMatch { buyer_intents: [a, b2], buyer_states: [IntentStateV1::open(a), IntentStateV1::open(b2)], buyer_authorizations: [auth(1), auth(2)], buyer_positions: [position(1, [0, 0])?, position(2, [0, 0])?], fill: 1, execution_prices: [400_000, 600_000], ..base }; assert_eq!(settle_split(fractional), Err(Error::NonIntegralQuote)); Ok(())
    }
}
