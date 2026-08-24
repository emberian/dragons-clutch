#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact, SDK-free contract for a fully covered multi-LP quote-bin venue.
//!
//! The Pool is the sole full Market attachment owner. The immutable config is
//! finalized before Pool derivation and therefore contains no Pool address;
//! compact mutable children and receipts carry only their parent Pool address
//! and Market generation. Prices are immutable for this release; time-gated
//! resets only reopen the identical ladder. Hoard principal and future revenue
//! are not representable.

pub mod activation;
pub mod frame;
pub mod instruction;

use core::convert::TryFrom;

use dclutch_core_contract::{ContentId, MARKET_IDENTITY_BYTES, MarketIdentity};

/// Mathematical minimum for an exhaustive claim partition.
pub const MIN_NATIVE_CLAIMS: usize = 2;
/// Provisional exact-width profile maximum; pagination is the lifting path.
pub const MAX_NATIVE_CLAIMS: usize = 16;
/// Measured-profile minimum number of price bins per claim and side.
pub const MIN_QUOTE_BINS: usize = 1;
/// Provisional account/compute profile maximum for bins per claim and side.
pub const MAX_QUOTE_BINS: usize = 8;
/// Mathematical denominator for fee basis points.
pub const BASIS_POINTS_DENOMINATOR: u64 = 10_000;
/// Exact byte width of [`LiquidityAttachment`].
pub const LIQUIDITY_ATTACHMENT_BYTES: usize = 264;
/// Exact byte width of [`ParentPool`].
pub const PARENT_POOL_BYTES: usize = 40;
/// Exact byte width of [`RentCreditTerms`].
pub const RENT_CREDIT_TERMS_BYTES: usize = 40;
/// Exact byte width of [`LpPosition`].
pub const LP_POSITION_BYTES: usize = 152;

const HEADER_BYTES: usize = 16;
const SCHEMA_VERSION: u16 = 1;
const HEADER_RESERVED_OFFSET: usize = 10;
const HEADER_RESERVED_BYTES: usize = 6;

const POOL_MAGIC: [u8; 8] = *b"DCLTPOOL";
const CONFIG_MAGIC: [u8; 8] = *b"DCLTBIN1";
const POSITION_MAGIC: [u8; 8] = *b"DCLTLPV1";
const EXECUTION_MAGIC: [u8; 8] = *b"DCLTEXV1";

const ATTACHMENT_MARKET_OFFSET: usize = 0;
const ATTACHMENT_RELEASE_OFFSET: usize = MARKET_IDENTITY_BYTES;
const ATTACHMENT_CONFIG_OFFSET: usize = ATTACHMENT_RELEASE_OFFSET + 32;
const ATTACHMENT_SERVICE_BENEFICIARY_OFFSET: usize = ATTACHMENT_CONFIG_OFFSET + 32;

const PARENT_ADDRESS_OFFSET: usize = 0;
const PARENT_GENERATION_OFFSET: usize = 32;
const RENT_BENEFICIARY_OFFSET: usize = 0;
const RENT_PRINCIPAL_OFFSET: usize = 32;

const CONFIG_OWNER_OFFSET: usize = HEADER_BYTES;
const CONFIG_PRICE_SCALE_OFFSET: usize = CONFIG_OWNER_OFFSET + 32;
const CONFIG_FEE_BPS_OFFSET: usize = CONFIG_PRICE_SCALE_OFFSET + 8;
const CONFIG_RESERVED_OFFSET: usize = CONFIG_FEE_BPS_OFFSET + 2;
const CONFIG_RESERVED_BYTES: usize = 6;
const CONFIG_MAX_QUANTITY_OFFSET: usize = CONFIG_RESERVED_OFFSET + CONFIG_RESERVED_BYTES;
const CONFIG_RESET_INTERVAL_OFFSET: usize = CONFIG_MAX_QUANTITY_OFFSET + 8;
const CONFIG_BID_PRICE_OFFSET: usize = CONFIG_RESET_INTERVAL_OFFSET + 8;

const STATE_ATTACHMENT_OFFSET: usize = HEADER_BYTES;
const STATE_RENT_OFFSET: usize = STATE_ATTACHMENT_OFFSET + LIQUIDITY_ATTACHMENT_BYTES;
const STATE_RESET_OFFSET: usize = STATE_RENT_OFFSET + RENT_CREDIT_TERMS_BYTES;
const STATE_SEQUENCE_OFFSET: usize = STATE_RESET_OFFSET + 8;
const STATE_NEXT_RESET_SLOT_OFFSET: usize = STATE_SEQUENCE_OFFSET + 8;
const STATE_STATUS_OFFSET: usize = STATE_NEXT_RESET_SLOT_OFFSET + 8;
const STATE_RESERVED_OFFSET: usize = STATE_STATUS_OFFSET + 1;
const STATE_RESERVED_BYTES: usize = 7;
const STATE_LIVE_POSITIONS_OFFSET: usize = STATE_RESERVED_OFFSET + STATE_RESERVED_BYTES;
const STATE_TOTAL_SHARES_OFFSET: usize = STATE_LIVE_POSITIONS_OFFSET + 8;
const STATE_PRINCIPAL_OFFSET: usize = STATE_TOTAL_SHARES_OFFSET + 8;
const STATE_FEES_OFFSET: usize = STATE_PRINCIPAL_OFFSET + 8;
const STATE_SERVICE_OFFSET: usize = STATE_FEES_OFFSET + 8;
const STATE_CLAIMS_OFFSET: usize = STATE_SERVICE_OFFSET + 8;

const POSITION_PARENT_OFFSET: usize = HEADER_BYTES;
const POSITION_OWNER_OFFSET: usize = POSITION_PARENT_OFFSET + PARENT_POOL_BYTES;
const POSITION_RENT_OFFSET: usize = POSITION_OWNER_OFFSET + 32;
const POSITION_SHARES_OFFSET: usize = POSITION_RENT_OFFSET + RENT_CREDIT_TERMS_BYTES;
const POSITION_SEQUENCE_OFFSET: usize = POSITION_SHARES_OFFSET + 8;
const POSITION_STATUS_OFFSET: usize = POSITION_SEQUENCE_OFFSET + 8;
const POSITION_RESERVED_OFFSET: usize = POSITION_STATUS_OFFSET + 1;
const POSITION_RESERVED_BYTES: usize = 7;

const EXECUTION_PARENT_OFFSET: usize = HEADER_BYTES;
const EXECUTION_RESET_OFFSET: usize = EXECUTION_PARENT_OFFSET + PARENT_POOL_BYTES;
const EXECUTION_SEQUENCE_OFFSET: usize = EXECUTION_RESET_OFFSET + 8;
const EXECUTION_SIDE_OFFSET: usize = EXECUTION_SEQUENCE_OFFSET + 8;
const EXECUTION_CLAIM_OFFSET: usize = EXECUTION_SIDE_OFFSET + 1;
const EXECUTION_RESERVED_OFFSET: usize = EXECUTION_CLAIM_OFFSET + 1;
const EXECUTION_RESERVED_BYTES: usize = 6;
const EXECUTION_QUANTITY_OFFSET: usize = EXECUTION_RESERVED_OFFSET + EXECUTION_RESERVED_BYTES;
const EXECUTION_NOTIONAL_OFFSET: usize = EXECUTION_QUANTITY_OFFSET + 8;
const EXECUTION_FEE_OFFSET: usize = EXECUTION_NOTIONAL_OFFSET + 8;
const EXECUTION_TRADER_COLLATERAL_DEBIT_OFFSET: usize = EXECUTION_FEE_OFFSET + 8;
const EXECUTION_TRADER_COLLATERAL_CREDIT_OFFSET: usize =
    EXECUTION_TRADER_COLLATERAL_DEBIT_OFFSET + 8;
const EXECUTION_TRADER_CLAIM_DEBIT_OFFSET: usize = EXECUTION_TRADER_COLLATERAL_CREDIT_OFFSET + 8;
const EXECUTION_TRADER_CLAIM_CREDIT_OFFSET: usize = EXECUTION_TRADER_CLAIM_DEBIT_OFFSET + 8;
const EXECUTION_PRINCIPAL_BEFORE_OFFSET: usize = EXECUTION_TRADER_CLAIM_CREDIT_OFFSET + 8;
const EXECUTION_PRINCIPAL_AFTER_OFFSET: usize = EXECUTION_PRINCIPAL_BEFORE_OFFSET + 8;
const EXECUTION_FEES_BEFORE_OFFSET: usize = EXECUTION_PRINCIPAL_AFTER_OFFSET + 8;
const EXECUTION_FEES_AFTER_OFFSET: usize = EXECUTION_FEES_BEFORE_OFFSET + 8;
const EXECUTION_CLAIM_BEFORE_OFFSET: usize = EXECUTION_FEES_AFTER_OFFSET + 8;
const EXECUTION_CLAIM_AFTER_OFFSET: usize = EXECUTION_CLAIM_BEFORE_OFFSET + 8;
const EXECUTION_BIN_BEFORE_OFFSET: usize = EXECUTION_CLAIM_AFTER_OFFSET + 8;

/// Refusal from hostile decoding or a covered-liquidity transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A byte slice did not have the one exact selected-profile width.
    InvalidLength,
    /// Magic bytes did not identify the expected account contract.
    InvalidMagic,
    /// The schema version is not implemented.
    UnsupportedSchema,
    /// Reserved bytes were nonzero.
    NonCanonicalReservedBytes,
    /// A physical or authority identity was zero.
    ZeroIdentity,
    /// Physical account roles aliased.
    IdentityAlias,
    /// Exact claim or bin width fell outside the labeled release profile.
    UnsupportedProfile,
    /// A discriminant was unknown.
    UnknownDiscriminant,
    /// Parent Pool address or Market generation differed.
    ParentMismatch,
    /// Immutable Pool attachment did not select this child configuration.
    ConfigurationMismatch,
    /// A price scale or price was zero or a price exceeded one collateral unit.
    InvalidPrice,
    /// Bid/ask ladders crossed or were not strictly best-to-worst.
    InvalidLadder,
    /// Raw top-of-book prices admitted a categorical complete-set arbitrage.
    CompleteSetArbitrage,
    /// A bin had zero capacity.
    EmptyBin,
    /// Fee basis points were zero or exceeded the mathematical denominator.
    InvalidFeeRate,
    /// Reset number did not match current Pool state.
    InvalidReset,
    /// A reset interval was zero or its slot arithmetic was invalid.
    InvalidResetInterval,
    /// A time-window reset was attempted before its authenticated slot.
    ResetTooEarly,
    /// A replay sequence was stale.
    SequenceMismatch,
    /// A requested quantity or share amount was zero or out of bounds.
    InvalidQuantity,
    /// A claim index was outside the exact-N profile.
    ClaimIndexOutOfRange,
    /// A ladder lacked enough remaining immutable capacity.
    InsufficientBinDepth,
    /// Exact price arithmetic rounded a trade segment to zero.
    ZeroNotional,
    /// A caller's exact price/deposit/withdrawal limit was not met.
    LimitExceeded,
    /// Checked integer arithmetic overflowed or underflowed.
    ArithmeticOverflow,
    /// LP principal collateral could not cover the gross bid payment.
    InsufficientPrincipalCollateral,
    /// LP native-claim reserves could not cover the gross ask delivery.
    InsufficientClaimInventory,
    /// Segregated service funding could not cover a service payment.
    InsufficientServiceFunding,
    /// The pool lifecycle did not admit the transition.
    InvalidPoolStatus,
    /// The LP-position lifecycle did not admit the transition.
    InvalidPositionStatus,
    /// Position parent, owner, or shares did not match Pool facts.
    PositionMismatch,
    /// Total shares, position shares, or live-position counts were inconsistent.
    ShareInvariant,
    /// Initial liquidity was not positive in collateral and every exact claim.
    IncompleteInitialLiquidity,
    /// An old/delta/new conservation equation did not balance.
    ConservationMismatch,
    /// Rent principal had no exact positive beneficiary/amount contract.
    InvalidRentCredit,
    /// Pool retirement was attempted before all LP value and children closed.
    PoolNotQuiescent,
}

/// Result alias for covered-liquidity operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Sole full Market attachment persisted only by the Pool root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityAttachment {
    market: MarketIdentity,
    capability_release_id: ContentId,
    liquidity_config_id: ContentId,
    service_refund_beneficiary: [u8; 32],
}

impl LiquidityAttachment {
    /// Construct one immutable Pool attachment without persisting its own address.
    pub fn new(
        market: MarketIdentity,
        capability_release_id: ContentId,
        liquidity_config_id: ContentId,
        service_refund_beneficiary: [u8; 32],
    ) -> Result<Self> {
        if all_zero(&service_refund_beneficiary) {
            return Err(Error::ZeroIdentity);
        }
        Ok(Self {
            market,
            capability_release_id,
            liquidity_config_id,
            service_refund_beneficiary,
        })
    }

    /// Decode the exact canonical Pool attachment.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != LIQUIDITY_ATTACHMENT_BYTES {
            return Err(Error::InvalidLength);
        }
        Self::new(
            MarketIdentity::decode(subslice(bytes, 0, MARKET_IDENTITY_BYTES)?)
                .map_err(|_| Error::ConfigurationMismatch)?,
            ContentId::decode(subslice(bytes, ATTACHMENT_RELEASE_OFFSET, 32)?)
                .map_err(|_| Error::ZeroIdentity)?,
            ContentId::decode(subslice(bytes, ATTACHMENT_CONFIG_OFFSET, 32)?)
                .map_err(|_| Error::ZeroIdentity)?,
            read_array(bytes, ATTACHMENT_SERVICE_BENEFICIARY_OFFSET)?,
        )
    }

    /// Encode the exact canonical Pool attachment.
    pub fn to_bytes(self) -> [u8; LIQUIDITY_ATTACHMENT_BYTES] {
        let mut out = [0u8; LIQUIDITY_ATTACHMENT_BYTES];
        put(&mut out, ATTACHMENT_MARKET_OFFSET, &self.market.to_bytes());
        put(
            &mut out,
            ATTACHMENT_RELEASE_OFFSET,
            self.capability_release_id.as_bytes(),
        );
        put(
            &mut out,
            ATTACHMENT_CONFIG_OFFSET,
            self.liquidity_config_id.as_bytes(),
        );
        put(
            &mut out,
            ATTACHMENT_SERVICE_BENEFICIARY_OFFSET,
            &self.service_refund_beneficiary,
        );
        out
    }

    /// Return occurrence-specific Market identity, generation, and ClaimBasis.
    pub const fn market(self) -> MarketIdentity {
        self.market
    }
    /// Return selected Dealer capability release identity.
    pub const fn capability_release_id(self) -> ContentId {
        self.capability_release_id
    }
    /// Return immutable V1 configuration content identity.
    pub const fn liquidity_config_id(self) -> ContentId {
        self.liquidity_config_id
    }
    /// Return sole beneficiary of unused service collateral.
    pub const fn service_refund_beneficiary(self) -> [u8; 32] {
        self.service_refund_beneficiary
    }
}

/// Compact child reference to the physical parent Pool and Market generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentPool {
    address: [u8; 32],
    market_generation: u64,
}

impl ParentPool {
    /// Construct a compact parent reference from an authenticated Pool address.
    pub fn new(address: [u8; 32], market_generation: u64) -> Result<Self> {
        if all_zero(&address) {
            return Err(Error::ZeroIdentity);
        }
        Ok(Self {
            address,
            market_generation,
        })
    }

    /// Decode an exact compact parent reference.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PARENT_POOL_BYTES {
            return Err(Error::InvalidLength);
        }
        Self::new(
            read_array(bytes, PARENT_ADDRESS_OFFSET)?,
            read_u64(bytes, PARENT_GENERATION_OFFSET)?,
        )
    }

    /// Encode an exact compact parent reference.
    pub fn to_bytes(self) -> [u8; PARENT_POOL_BYTES] {
        let mut out = [0u8; PARENT_POOL_BYTES];
        put(&mut out, PARENT_ADDRESS_OFFSET, &self.address);
        put_u64(&mut out, PARENT_GENERATION_OFFSET, self.market_generation);
        out
    }

    /// Return authenticated parent Pool address.
    pub const fn address(self) -> [u8; 32] {
        self.address
    }
    /// Return immutable Market generation.
    pub const fn market_generation(self) -> u64 {
        self.market_generation
    }
}

/// Compact rent principal attribution to a permanent derived RentCredit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentCreditTerms {
    beneficiary: [u8; 32],
    funded_rent_principal: u64,
}

impl RentCreditTerms {
    /// Construct exact funded rent attribution.
    pub fn new(beneficiary: [u8; 32], funded_rent_principal: u64) -> Result<Self> {
        if all_zero(&beneficiary) {
            return Err(Error::ZeroIdentity);
        }
        if funded_rent_principal == 0 {
            return Err(Error::InvalidRentCredit);
        }
        Ok(Self {
            beneficiary,
            funded_rent_principal,
        })
    }

    /// Decode exact funded rent attribution.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != RENT_CREDIT_TERMS_BYTES {
            return Err(Error::InvalidLength);
        }
        Self::new(
            read_array(bytes, RENT_BENEFICIARY_OFFSET)?,
            read_u64(bytes, RENT_PRINCIPAL_OFFSET)?,
        )
    }

    /// Encode exact funded rent attribution.
    pub fn to_bytes(self) -> [u8; RENT_CREDIT_TERMS_BYTES] {
        let mut out = [0u8; RENT_CREDIT_TERMS_BYTES];
        put(&mut out, RENT_BENEFICIARY_OFFSET, &self.beneficiary);
        put_u64(&mut out, RENT_PRINCIPAL_OFFSET, self.funded_rent_principal);
        out
    }

    /// Return beneficiary whose permanent RentCredit receives all close lamports.
    pub const fn beneficiary(self) -> [u8; 32] {
        self.beneficiary
    }
    /// Return only the attributable funded rent principal.
    pub const fn funded_rent_principal(self) -> u64 {
        self.funded_rent_principal
    }
}

/// Immutable, capability-bound V1 prices, capacities, fee, and reset cadence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityConfigV1<const N: usize, const B: usize> {
    content_id: ContentId,
    liquidity_owner: [u8; 32],
    price_scale: u64,
    fee_bps: u16,
    max_trade_quantity: u64,
    reset_interval_slots: u64,
    bid_prices: [[u64; B]; N],
    ask_prices: [[u64; B]; N],
    bid_capacity: [[u64; B]; N],
    ask_capacity: [[u64; B]; N],
}

impl<const N: usize, const B: usize> LiquidityConfigV1<N, B> {
    /// Construct the one immutable V1 ladder selected by the capability content ID.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        content_id: ContentId,
        liquidity_owner: [u8; 32],
        price_scale: u64,
        fee_bps: u16,
        max_trade_quantity: u64,
        reset_interval_slots: u64,
        bid_prices: [[u64; B]; N],
        ask_prices: [[u64; B]; N],
        bid_capacity: [[u64; B]; N],
        ask_capacity: [[u64; B]; N],
    ) -> Result<Self> {
        validate_profile::<N, B>()?;
        if price_scale == 0 || max_trade_quantity == 0 {
            return Err(Error::InvalidPrice);
        }
        if fee_bps == 0 || u64::from(fee_bps) > BASIS_POINTS_DENOMINATOR {
            return Err(Error::InvalidFeeRate);
        }
        if reset_interval_slots == 0 {
            return Err(Error::InvalidResetInterval);
        }
        if all_zero(&liquidity_owner) {
            return Err(Error::ZeroIdentity);
        }
        validate_ladder(
            price_scale,
            &bid_prices,
            &ask_prices,
            &bid_capacity,
            &ask_capacity,
        )?;
        Ok(Self {
            content_id,
            liquidity_owner,
            price_scale,
            fee_bps,
            max_trade_quantity,
            reset_interval_slots,
            bid_prices,
            ask_prices,
            bid_capacity,
            ask_capacity,
        })
    }

    /// Return the exact selected-profile account width.
    pub fn encoded_len() -> Result<usize> {
        validate_profile::<N, B>()?;
        checked_profile_width(CONFIG_BID_PRICE_OFFSET, 32, N, B)
    }

    /// Decode bytes after the adapter authenticates their immutable content ID.
    pub fn decode(content_id: ContentId, bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::encoded_len()? {
            return Err(Error::InvalidLength);
        }
        decode_header(bytes, CONFIG_MAGIC)?;
        require_zero(bytes, CONFIG_RESERVED_OFFSET, CONFIG_RESERVED_BYTES)?;
        let cells = N.checked_mul(B).ok_or(Error::ArithmeticOverflow)?;
        let bid_offset = CONFIG_BID_PRICE_OFFSET;
        let ask_offset = checked_offset(bid_offset, 8, cells)?;
        let bid_capacity_offset = checked_offset(ask_offset, 8, cells)?;
        let ask_capacity_offset = checked_offset(bid_capacity_offset, 8, cells)?;
        Self::new(
            content_id,
            read_array(bytes, CONFIG_OWNER_OFFSET)?,
            read_u64(bytes, CONFIG_PRICE_SCALE_OFFSET)?,
            read_u16(bytes, CONFIG_FEE_BPS_OFFSET)?,
            read_u64(bytes, CONFIG_MAX_QUANTITY_OFFSET)?,
            read_u64(bytes, CONFIG_RESET_INTERVAL_OFFSET)?,
            read_matrix(bytes, bid_offset)?,
            read_matrix(bytes, ask_offset)?,
            read_matrix(bytes, bid_capacity_offset)?,
            read_matrix(bytes, ask_capacity_offset)?,
        )
    }

    /// Encode into an exact selected-profile destination.
    pub fn encode_into(&self, out: &mut [u8]) -> Result<()> {
        if out.len() != Self::encoded_len()? {
            return Err(Error::InvalidLength);
        }
        out.fill(0);
        encode_header(out, CONFIG_MAGIC);
        put(out, CONFIG_OWNER_OFFSET, &self.liquidity_owner);
        put_u64(out, CONFIG_PRICE_SCALE_OFFSET, self.price_scale);
        put_u16(out, CONFIG_FEE_BPS_OFFSET, self.fee_bps);
        put_u64(out, CONFIG_MAX_QUANTITY_OFFSET, self.max_trade_quantity);
        put_u64(out, CONFIG_RESET_INTERVAL_OFFSET, self.reset_interval_slots);
        let cells = N.checked_mul(B).ok_or(Error::ArithmeticOverflow)?;
        let bid_offset = CONFIG_BID_PRICE_OFFSET;
        let ask_offset = checked_offset(bid_offset, 8, cells)?;
        let bid_capacity_offset = checked_offset(ask_offset, 8, cells)?;
        let ask_capacity_offset = checked_offset(bid_capacity_offset, 8, cells)?;
        put_matrix(out, bid_offset, &self.bid_prices)?;
        put_matrix(out, ask_offset, &self.ask_prices)?;
        put_matrix(out, bid_capacity_offset, &self.bid_capacity)?;
        put_matrix(out, ask_capacity_offset, &self.ask_capacity)?;
        Ok(())
    }

    /// Return adapter-authenticated content identity; it is not self-persisted.
    pub const fn content_id(&self) -> ContentId {
        self.content_id
    }
    /// Return immutable bootstrap LP and unused-service beneficiary authority.
    pub const fn liquidity_owner(&self) -> [u8; 32] {
        self.liquidity_owner
    }
    /// Return named collateral price scale.
    pub const fn price_scale(&self) -> u64 {
        self.price_scale
    }
    /// Return trader-paid fee basis points.
    pub const fn fee_bps(&self) -> u16 {
        self.fee_bps
    }
    /// Return per-trade claim quantity bound.
    pub const fn max_trade_quantity(&self) -> u64 {
        self.max_trade_quantity
    }
    /// Return immutable positive ladder reset cadence.
    pub const fn reset_interval_slots(&self) -> u64 {
        self.reset_interval_slots
    }
    /// Return all bid prices, best first.
    pub const fn bid_prices(&self) -> [[u64; B]; N] {
        self.bid_prices
    }
    /// Return all ask prices, best first.
    pub const fn ask_prices(&self) -> [[u64; B]; N] {
        self.ask_prices
    }
    /// Return all per-window bid capacities.
    pub const fn bid_capacity(&self) -> [[u64; B]; N] {
        self.bid_capacity
    }
    /// Return all per-window ask capacities.
    pub const fn ask_capacity(&self) -> [[u64; B]; N] {
        self.ask_capacity
    }
}

/// Exact LP-owned value; service funding is deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityAmounts<const N: usize> {
    principal_collateral: u64,
    realized_fee_collateral: u64,
    claim_reserves: [u64; N],
}

impl<const N: usize> LiquidityAmounts<N> {
    /// Construct one exact-N LP value vector.
    pub fn new(
        principal_collateral: u64,
        realized_fee_collateral: u64,
        claim_reserves: [u64; N],
    ) -> Result<Self> {
        validate_claim_profile::<N>()?;
        Ok(Self {
            principal_collateral,
            realized_fee_collateral,
            claim_reserves,
        })
    }

    /// Return LP principal collateral.
    pub const fn principal_collateral(self) -> u64 {
        self.principal_collateral
    }
    /// Return segregated trader-paid fee collateral owned by LP shares.
    pub const fn realized_fee_collateral(self) -> u64 {
        self.realized_fee_collateral
    }
    /// Return all exact-N native-claim reserves.
    pub const fn claim_reserves(self) -> [u64; N] {
        self.claim_reserves
    }

    fn is_initially_complete(self) -> bool {
        self.principal_collateral > 0
            && self.realized_fee_collateral == 0
            && self.claim_reserves.iter().all(|value| *value > 0)
    }

    fn is_zero(self) -> bool {
        self.principal_collateral == 0
            && self.realized_fee_collateral == 0
            && self.claim_reserves.iter().all(|value| *value == 0)
    }
}

/// Lifecycle of the Pool account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PoolStatus {
    /// Trades, LP changes, service movements, and timed resets are admitted.
    Active = 0,
    /// All LP shares were removed; empty positions and service funding remain.
    Retiring = 1,
    /// Service funding and rent were routed and the physical Pool may close.
    Retired = 2,
}

impl PoolStatus {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::Retiring),
            2 => Ok(Self::Retired),
            _ => Err(Error::UnknownDiscriminant),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Retiring => 1,
            Self::Retired => 2,
        }
    }
}

/// Fully covered mutable Pool state for one exact-N, fixed-bin profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolState<const N: usize, const B: usize> {
    attachment: LiquidityAttachment,
    rent_credit: RentCreditTerms,
    reset_number: u64,
    next_sequence: u64,
    next_reset_slot: u64,
    status: PoolStatus,
    live_positions: u64,
    total_shares: u64,
    principal_collateral: u64,
    realized_fee_collateral: u64,
    service_funding: u64,
    claim_reserves: [u64; N],
    bid_filled: [[u64; B]; N],
    ask_filled: [[u64; B]; N],
}

impl<const N: usize, const B: usize> PoolState<N, B> {
    /// Return the exact selected-profile account width.
    pub fn encoded_len() -> Result<usize> {
        validate_profile::<N, B>()?;
        let claims = 8usize.checked_mul(N).ok_or(Error::ArithmeticOverflow)?;
        let fills = 16usize
            .checked_mul(N.checked_mul(B).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)?;
        STATE_CLAIMS_OFFSET
            .checked_add(claims)
            .and_then(|value| value.checked_add(fills))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Open a Pool with complete exact-N inventory and its first LP position.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open(
        attachment: LiquidityAttachment,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        pool_rent_credit: RentCreditTerms,
        opened_at_slot: u64,
        initial_liquidity: LiquidityAmounts<N>,
        service_funding: u64,
        initial_position_id: [u8; 32],
        initial_owner: [u8; 32],
        initial_position_rent: RentCreditTerms,
        initial_shares: u64,
    ) -> Result<(Self, LpPosition, LiquidityChangeReceipt<N>)> {
        let parent = parent_for(attachment, pool_address)?;
        require_selected_config(attachment, config)?;
        if !initial_liquidity.is_initially_complete() {
            return Err(Error::IncompleteInitialLiquidity);
        }
        if initial_shares == 0 {
            return Err(Error::InvalidQuantity);
        }
        validate_position_identity(initial_position_id, parent, initial_owner)?;
        let next_reset_slot = opened_at_slot
            .checked_add(config.reset_interval_slots)
            .ok_or(Error::InvalidResetInterval)?;
        let state = Self {
            attachment,
            rent_credit: pool_rent_credit,
            reset_number: 0,
            next_sequence: 1,
            next_reset_slot,
            status: PoolStatus::Active,
            live_positions: 1,
            total_shares: initial_shares,
            principal_collateral: initial_liquidity.principal_collateral,
            realized_fee_collateral: 0,
            service_funding,
            claim_reserves: initial_liquidity.claim_reserves,
            bid_filled: [[0u64; B]; N],
            ask_filled: [[0u64; B]; N],
        };
        state.validate_against(pool_address, config)?;
        let position = LpPosition::new(
            parent,
            initial_owner,
            initial_position_rent,
            initial_shares,
            PositionStatus::Active,
        )?;
        let zero = LiquidityAmounts::new(0, 0, [0u64; N])?;
        let receipt = LiquidityChangeReceipt {
            kind: LiquidityChangeKind::Open,
            parent,
            pool_sequence: 0,
            position_id: initial_position_id,
            owner: initial_owner,
            amounts_before: zero,
            amounts_transferred: initial_liquidity,
            amounts_after: initial_liquidity,
            total_shares_before: 0,
            shares_changed: initial_shares,
            total_shares_after: initial_shares,
            position_shares_before: 0,
            position_shares_after: initial_shares,
        };
        receipt.validate()?;
        Ok((state, position, receipt))
    }

    /// Decode and validate one exact selected-profile Pool account.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::encoded_len()? {
            return Err(Error::InvalidLength);
        }
        decode_header(bytes, POOL_MAGIC)?;
        require_zero(bytes, STATE_RESERVED_OFFSET, STATE_RESERVED_BYTES)?;
        let bid_offset = checked_offset(STATE_CLAIMS_OFFSET, 8, N)?;
        let ask_offset = checked_offset(
            bid_offset,
            8,
            N.checked_mul(B).ok_or(Error::ArithmeticOverflow)?,
        )?;
        let state = Self {
            attachment: LiquidityAttachment::decode(subslice(
                bytes,
                STATE_ATTACHMENT_OFFSET,
                LIQUIDITY_ATTACHMENT_BYTES,
            )?)?,
            rent_credit: RentCreditTerms::decode(subslice(
                bytes,
                STATE_RENT_OFFSET,
                RENT_CREDIT_TERMS_BYTES,
            )?)?,
            reset_number: read_u64(bytes, STATE_RESET_OFFSET)?,
            next_sequence: read_u64(bytes, STATE_SEQUENCE_OFFSET)?,
            next_reset_slot: read_u64(bytes, STATE_NEXT_RESET_SLOT_OFFSET)?,
            status: PoolStatus::decode(read_u8(bytes, STATE_STATUS_OFFSET)?)?,
            live_positions: read_u64(bytes, STATE_LIVE_POSITIONS_OFFSET)?,
            total_shares: read_u64(bytes, STATE_TOTAL_SHARES_OFFSET)?,
            principal_collateral: read_u64(bytes, STATE_PRINCIPAL_OFFSET)?,
            realized_fee_collateral: read_u64(bytes, STATE_FEES_OFFSET)?,
            service_funding: read_u64(bytes, STATE_SERVICE_OFFSET)?,
            claim_reserves: read_vector(bytes, STATE_CLAIMS_OFFSET)?,
            bid_filled: read_matrix(bytes, bid_offset)?,
            ask_filled: read_matrix(bytes, ask_offset)?,
        };
        state.validate()?;
        Ok(state)
    }

    /// Encode one exact selected-profile Pool account.
    pub fn encode_into(&self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() != Self::encoded_len()? {
            return Err(Error::InvalidLength);
        }
        out.fill(0);
        encode_header(out, POOL_MAGIC);
        put(out, STATE_ATTACHMENT_OFFSET, &self.attachment.to_bytes());
        put(out, STATE_RENT_OFFSET, &self.rent_credit.to_bytes());
        put_u64(out, STATE_RESET_OFFSET, self.reset_number);
        put_u64(out, STATE_SEQUENCE_OFFSET, self.next_sequence);
        put_u64(out, STATE_NEXT_RESET_SLOT_OFFSET, self.next_reset_slot);
        put(out, STATE_STATUS_OFFSET, &[self.status.byte()]);
        put_u64(out, STATE_LIVE_POSITIONS_OFFSET, self.live_positions);
        put_u64(out, STATE_TOTAL_SHARES_OFFSET, self.total_shares);
        put_u64(out, STATE_PRINCIPAL_OFFSET, self.principal_collateral);
        put_u64(out, STATE_FEES_OFFSET, self.realized_fee_collateral);
        put_u64(out, STATE_SERVICE_OFFSET, self.service_funding);
        put_vector(out, STATE_CLAIMS_OFFSET, &self.claim_reserves)?;
        let bid_offset = checked_offset(STATE_CLAIMS_OFFSET, 8, N)?;
        let ask_offset = checked_offset(
            bid_offset,
            8,
            N.checked_mul(B).ok_or(Error::ArithmeticOverflow)?,
        )?;
        put_matrix(out, bid_offset, &self.bid_filled)?;
        put_matrix(out, ask_offset, &self.ask_filled)?;
        Ok(())
    }

    /// Return sole full Pool attachment.
    pub const fn attachment(&self) -> LiquidityAttachment {
        self.attachment
    }
    /// Return Pool-account rent attribution.
    pub const fn rent_credit(&self) -> RentCreditTerms {
        self.rent_credit
    }
    /// Return current immutable-ladder reset number.
    pub const fn reset_number(&self) -> u64 {
        self.reset_number
    }
    /// Return next globally accepted replay sequence.
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
    /// Return earliest authenticated slot for the next identical-ladder reset.
    pub const fn next_reset_slot(&self) -> u64 {
        self.next_reset_slot
    }
    /// Return lifecycle status.
    pub const fn status(&self) -> PoolStatus {
        self.status
    }
    /// Return number of non-closed LP-position accounts.
    pub const fn live_positions(&self) -> u64 {
        self.live_positions
    }
    /// Return total outstanding LP shares.
    pub const fn total_shares(&self) -> u64 {
        self.total_shares
    }
    /// Return all LP-owned compartments.
    pub const fn liquidity(&self) -> LiquidityAmounts<N> {
        LiquidityAmounts {
            principal_collateral: self.principal_collateral,
            realized_fee_collateral: self.realized_fee_collateral,
            claim_reserves: self.claim_reserves,
        }
    }
    /// Return separately prepaid service funding.
    pub const fn service_funding(&self) -> u64 {
        self.service_funding
    }
    /// Return consumed bid capacities in this time window.
    pub const fn bid_filled(&self) -> [[u64; B]; N] {
        self.bid_filled
    }
    /// Return consumed ask capacities in this time window.
    pub const fn ask_filled(&self) -> [[u64; B]; N] {
        self.ask_filled
    }

    /// Validate state-only conservation and lifecycle conditions.
    pub fn validate(&self) -> Result<()> {
        validate_profile::<N, B>()?;
        if self.next_sequence == 0 || self.next_reset_slot == 0 {
            return Err(Error::SequenceMismatch);
        }
        match self.status {
            PoolStatus::Active => {
                if self.total_shares == 0 || self.live_positions == 0 {
                    return Err(Error::ShareInvariant);
                }
            }
            PoolStatus::Retiring => {
                if self.total_shares != 0 || !self.liquidity().is_zero() {
                    return Err(Error::ShareInvariant);
                }
            }
            PoolStatus::Retired => {
                if self.total_shares != 0
                    || self.live_positions != 0
                    || self.service_funding != 0
                    || !self.liquidity().is_zero()
                {
                    return Err(Error::PoolNotQuiescent);
                }
            }
        }
        Ok(())
    }

    /// Validate compact parent and fills against the one immutable V1 config.
    pub fn validate_against(
        &self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
    ) -> Result<()> {
        self.validate()?;
        parent_for(self.attachment, pool_address)?;
        require_selected_config(self.attachment, config)?;
        for (((bid_fill_row, ask_fill_row), bid_capacity_row), ask_capacity_row) in self
            .bid_filled
            .iter()
            .zip(self.ask_filled.iter())
            .zip(config.bid_capacity.iter())
            .zip(config.ask_capacity.iter())
        {
            for (((bid_fill, ask_fill), bid_capacity), ask_capacity) in bid_fill_row
                .iter()
                .zip(ask_fill_row.iter())
                .zip(bid_capacity_row.iter())
                .zip(ask_capacity_row.iter())
            {
                if bid_fill > bid_capacity || ask_fill > ask_capacity {
                    return Err(Error::ConservationMismatch);
                }
            }
        }
        Ok(())
    }

    fn require_active(
        &self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
    ) -> Result<()> {
        self.validate_against(pool_address, config)?;
        if self.status != PoolStatus::Active {
            return Err(Error::InvalidPoolStatus);
        }
        Ok(())
    }

    fn require_sequence(&self, expected: u64) -> Result<()> {
        if self.next_sequence != expected {
            return Err(Error::SequenceMismatch);
        }
        Ok(())
    }

    fn bump_sequence(&mut self) -> Result<u64> {
        let accepted = self.next_sequence;
        self.next_sequence = accepted.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        Ok(accepted)
    }
}

/// Exact record for reopening the same immutable ladder after its slot interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LadderResetReceipt<const N: usize, const B: usize> {
    parent: ParentPool,
    pool_sequence: u64,
    old_reset_number: u64,
    new_reset_number: u64,
    observed_slot: u64,
    next_reset_slot: u64,
    old_bid_filled: [[u64; B]; N],
    old_ask_filled: [[u64; B]; N],
}

impl<const N: usize, const B: usize> LadderResetReceipt<N, B> {
    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return accepted global Pool sequence.
    pub const fn pool_sequence(&self) -> u64 {
        self.pool_sequence
    }
    /// Return prior time-window reset number.
    pub const fn old_reset_number(&self) -> u64 {
        self.old_reset_number
    }
    /// Return new time-window reset number.
    pub const fn new_reset_number(&self) -> u64 {
        self.new_reset_number
    }
    /// Return adapter-authenticated slot used for reset admission.
    pub const fn observed_slot(&self) -> u64 {
        self.observed_slot
    }
    /// Return earliest slot of the next reset.
    pub const fn next_reset_slot(&self) -> u64 {
        self.next_reset_slot
    }
    /// Return terminal bid fills of the old time window.
    pub const fn old_bid_filled(&self) -> [[u64; B]; N] {
        self.old_bid_filled
    }
    /// Return terminal ask fills of the old time window.
    pub const fn old_ask_filled(&self) -> [[u64; B]; N] {
        self.old_ask_filled
    }
}

/// Direction of an explicitly segregated service-funding movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceFlowKind {
    /// Present collateral entered the service compartment.
    Fund,
    /// Present service collateral was paid to a named recipient.
    Spend,
}

/// Exact adapter-applied service-funding movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceFundingReceipt {
    parent: ParentPool,
    pool_sequence: u64,
    kind: ServiceFlowKind,
    counterparty: [u8; 32],
    amount: u64,
    before: u64,
    after: u64,
}

impl ServiceFundingReceipt {
    /// Validate segregated service conservation.
    pub fn validate(&self) -> Result<()> {
        if all_zero(&self.counterparty) || self.amount == 0 {
            return Err(Error::ConservationMismatch);
        }
        let valid = match self.kind {
            ServiceFlowKind::Fund => self.before.checked_add(self.amount) == Some(self.after),
            ServiceFlowKind::Spend => self.after.checked_add(self.amount) == Some(self.before),
        };
        if !valid {
            return Err(Error::ConservationMismatch);
        }
        Ok(())
    }
    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return accepted global Pool sequence.
    pub const fn pool_sequence(&self) -> u64 {
        self.pool_sequence
    }
    /// Return service movement direction.
    pub const fn kind(&self) -> ServiceFlowKind {
        self.kind
    }
    /// Return present-collateral funder or payment recipient.
    pub const fn counterparty(&self) -> [u8; 32] {
        self.counterparty
    }
    /// Return exact present collateral amount.
    pub const fn amount(&self) -> u64 {
        self.amount
    }
    /// Return service balance before.
    pub const fn before(&self) -> u64 {
        self.before
    }
    /// Return service balance after.
    pub const fn after(&self) -> u64 {
        self.after
    }
}

/// Terminal custody and compact RentCredit routing for a quiescent Pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolRetirementReceipt {
    parent: ParentPool,
    pool_sequence: u64,
    service_refund_beneficiary: [u8; 32],
    service_refund_collateral: u64,
    pool_rent_credit: RentCreditTerms,
}

impl PoolRetirementReceipt {
    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return accepted terminal global sequence.
    pub const fn pool_sequence(&self) -> u64 {
        self.pool_sequence
    }
    /// Return immutable beneficiary of unused service collateral.
    pub const fn service_refund_beneficiary(&self) -> [u8; 32] {
        self.service_refund_beneficiary
    }
    /// Return unused service collateral routed to its beneficiary.
    pub const fn service_refund_collateral(&self) -> u64 {
        self.service_refund_collateral
    }
    /// Return Pool-account funded rent attribution.
    pub const fn pool_rent_credit(&self) -> RentCreditTerms {
        self.pool_rent_credit
    }
}

impl<const N: usize, const B: usize> PoolState<N, B> {
    /// Reopen only the identical immutable ladder after its exact time boundary.
    pub fn reset_ladder(
        &mut self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        expected_pool_sequence: u64,
        authenticated_now_slot: u64,
    ) -> Result<LadderResetReceipt<N, B>> {
        self.require_active(pool_address, config)?;
        self.require_sequence(expected_pool_sequence)?;
        if authenticated_now_slot < self.next_reset_slot {
            return Err(Error::ResetTooEarly);
        }
        let next_reset_slot = authenticated_now_slot
            .checked_add(config.reset_interval_slots)
            .ok_or(Error::InvalidResetInterval)?;
        let mut next = *self;
        let old_reset_number = next.reset_number;
        next.reset_number = next
            .reset_number
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.next_reset_slot = next_reset_slot;
        let receipt = LadderResetReceipt {
            parent: parent_for(self.attachment, pool_address)?,
            pool_sequence: next.next_sequence,
            old_reset_number,
            new_reset_number: next.reset_number,
            observed_slot: authenticated_now_slot,
            next_reset_slot,
            old_bid_filled: next.bid_filled,
            old_ask_filled: next.ask_filled,
        };
        next.bid_filled = [[0u64; B]; N];
        next.ask_filled = [[0u64; B]; N];
        next.bump_sequence()?;
        next.validate_against(pool_address, config)?;
        *self = next;
        Ok(receipt)
    }

    /// Add present collateral to service funding without minting LP shares.
    pub fn fund_service(
        &mut self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        expected_pool_sequence: u64,
        funder: [u8; 32],
        amount: u64,
    ) -> Result<ServiceFundingReceipt> {
        self.validate_against(pool_address, config)?;
        if self.status == PoolStatus::Retired {
            return Err(Error::InvalidPoolStatus);
        }
        self.require_sequence(expected_pool_sequence)?;
        if all_zero(&funder) || amount == 0 {
            return Err(Error::InvalidQuantity);
        }
        let parent = parent_for(self.attachment, pool_address)?;
        let mut next = *self;
        let before = next.service_funding;
        next.service_funding = checked_add(next.service_funding, amount)?;
        let sequence = next.bump_sequence()?;
        let receipt = ServiceFundingReceipt {
            parent,
            pool_sequence: sequence,
            kind: ServiceFlowKind::Fund,
            counterparty: funder,
            amount,
            before,
            after: next.service_funding,
        };
        receipt.validate()?;
        next.validate()?;
        *self = next;
        Ok(receipt)
    }

    /// Pay a named recipient only from prepaid service funding.
    pub fn spend_service(
        &mut self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        expected_pool_sequence: u64,
        recipient: [u8; 32],
        amount: u64,
    ) -> Result<ServiceFundingReceipt> {
        self.validate_against(pool_address, config)?;
        if self.status == PoolStatus::Retired {
            return Err(Error::InvalidPoolStatus);
        }
        self.require_sequence(expected_pool_sequence)?;
        if all_zero(&recipient) || amount == 0 {
            return Err(Error::InvalidQuantity);
        }
        let parent = parent_for(self.attachment, pool_address)?;
        let mut next = *self;
        let before = next.service_funding;
        next.service_funding = next
            .service_funding
            .checked_sub(amount)
            .ok_or(Error::InsufficientServiceFunding)?;
        let sequence = next.bump_sequence()?;
        let receipt = ServiceFundingReceipt {
            parent,
            pool_sequence: sequence,
            kind: ServiceFlowKind::Spend,
            counterparty: recipient,
            amount,
            before,
            after: next.service_funding,
        };
        receipt.validate()?;
        next.validate()?;
        *self = next;
        Ok(receipt)
    }

    /// Retire a quiescent Pool and route service and all close lamports exactly.
    pub(crate) fn retire(
        &mut self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        expected_pool_sequence: u64,
    ) -> Result<PoolRetirementReceipt> {
        self.validate_against(pool_address, config)?;
        self.require_sequence(expected_pool_sequence)?;
        if self.status != PoolStatus::Retiring
            || self.total_shares != 0
            || self.live_positions != 0
            || !self.liquidity().is_zero()
        {
            return Err(Error::PoolNotQuiescent);
        }
        let mut next = *self;
        let service_refund = next.service_funding;
        next.service_funding = 0;
        next.status = PoolStatus::Retired;
        let sequence = next.bump_sequence()?;
        let receipt = PoolRetirementReceipt {
            parent: parent_for(self.attachment, pool_address)?,
            pool_sequence: sequence,
            service_refund_beneficiary: next.attachment.service_refund_beneficiary,
            service_refund_collateral: service_refund,
            pool_rent_credit: next.rent_credit,
        };
        next.validate()?;
        *self = next;
        Ok(receipt)
    }
}

fn parent_for(attachment: LiquidityAttachment, pool_address: [u8; 32]) -> Result<ParentPool> {
    ParentPool::new(pool_address, attachment.market.generation())
}

fn require_selected_config<const N: usize, const B: usize>(
    attachment: LiquidityAttachment,
    config: &LiquidityConfigV1<N, B>,
) -> Result<()> {
    if config.content_id != attachment.liquidity_config_id {
        return Err(Error::ConfigurationMismatch);
    }
    Ok(())
}

fn validate_position_identity(
    position_id: [u8; 32],
    parent: ParentPool,
    owner: [u8; 32],
) -> Result<()> {
    if all_zero(&position_id) || all_zero(&owner) {
        return Err(Error::ZeroIdentity);
    }
    if position_id == parent.address || position_id == owner {
        return Err(Error::IdentityAlias);
    }
    Ok(())
}

fn validate_ladder<const N: usize, const B: usize>(
    price_scale: u64,
    bid_prices: &[[u64; B]; N],
    ask_prices: &[[u64; B]; N],
    bid_capacity: &[[u64; B]; N],
    ask_capacity: &[[u64; B]; N],
) -> Result<()> {
    let mut best_bid_sum = 0u64;
    let mut best_ask_sum = 0u64;
    for (((bid_row, ask_row), bid_capacity_row), ask_capacity_row) in bid_prices
        .iter()
        .zip(ask_prices.iter())
        .zip(bid_capacity.iter())
        .zip(ask_capacity.iter())
    {
        best_bid_sum = checked_add(
            best_bid_sum,
            bid_row.first().copied().ok_or(Error::UnsupportedProfile)?,
        )?;
        best_ask_sum = checked_add(
            best_ask_sum,
            ask_row.first().copied().ok_or(Error::UnsupportedProfile)?,
        )?;
        let mut previous_bid = None;
        let mut previous_ask = None;
        for (((bid, ask), bid_depth), ask_depth) in bid_row
            .iter()
            .zip(ask_row.iter())
            .zip(bid_capacity_row.iter())
            .zip(ask_capacity_row.iter())
        {
            let bid = *bid;
            let ask = *ask;
            if bid == 0 || ask == 0 || bid > price_scale || ask > price_scale {
                return Err(Error::InvalidPrice);
            }
            if bid > ask {
                return Err(Error::InvalidLadder);
            }
            if *bid_depth == 0 || *ask_depth == 0 {
                return Err(Error::EmptyBin);
            }
            if let (Some(old_bid), Some(old_ask)) = (previous_bid, previous_ask)
                && (old_bid <= bid || old_ask >= ask)
            {
                return Err(Error::InvalidLadder);
            }
            previous_bid = Some(bid);
            previous_ask = Some(ask);
        }
    }
    if best_bid_sum > price_scale || best_ask_sum < price_scale {
        return Err(Error::CompleteSetArbitrage);
    }
    Ok(())
}

fn validate_claim_profile<const N: usize>() -> Result<()> {
    if !(MIN_NATIVE_CLAIMS..=MAX_NATIVE_CLAIMS).contains(&N) {
        return Err(Error::UnsupportedProfile);
    }
    Ok(())
}

fn validate_bin_profile<const B: usize>() -> Result<()> {
    if !(MIN_QUOTE_BINS..=MAX_QUOTE_BINS).contains(&B) {
        return Err(Error::UnsupportedProfile);
    }
    Ok(())
}

fn validate_profile<const N: usize, const B: usize>() -> Result<()> {
    validate_claim_profile::<N>()?;
    validate_bin_profile::<B>()
}

fn checked_profile_width(base: usize, bytes_per_cell: usize, n: usize, b: usize) -> Result<usize> {
    let cells = n.checked_mul(b).ok_or(Error::ArithmeticOverflow)?;
    base.checked_add(
        bytes_per_cell
            .checked_mul(cells)
            .ok_or(Error::ArithmeticOverflow)?,
    )
    .ok_or(Error::ArithmeticOverflow)
}

fn checked_offset(base: usize, bytes_per_cell: usize, cells: usize) -> Result<usize> {
    base.checked_add(
        bytes_per_cell
            .checked_mul(cells)
            .ok_or(Error::ArithmeticOverflow)?,
    )
    .ok_or(Error::ArithmeticOverflow)
}

fn proportional_amounts<const N: usize>(
    amounts: LiquidityAmounts<N>,
    numerator: u64,
    denominator: u64,
    round_up: bool,
) -> Result<LiquidityAmounts<N>> {
    if numerator == 0 || denominator == 0 || numerator > denominator && !round_up {
        return Err(Error::InvalidQuantity);
    }
    let calculate = |value| {
        if round_up {
            mul_div_ceil(value, numerator, denominator)
        } else {
            mul_div_floor(value, numerator, denominator)
        }
    };
    let mut claims = [0u64; N];
    for (output, reserve) in claims.iter_mut().zip(amounts.claim_reserves.iter()) {
        *output = calculate(*reserve)?;
    }
    LiquidityAmounts::new(
        calculate(amounts.principal_collateral)?,
        calculate(amounts.realized_fee_collateral)?,
        claims,
    )
}

fn require_amounts_at_most<const N: usize>(
    actual: LiquidityAmounts<N>,
    maximum: LiquidityAmounts<N>,
) -> Result<()> {
    if actual.principal_collateral > maximum.principal_collateral
        || actual.realized_fee_collateral > maximum.realized_fee_collateral
        || actual
            .claim_reserves
            .iter()
            .zip(maximum.claim_reserves.iter())
            .any(|(actual_claim, maximum_claim)| actual_claim > maximum_claim)
    {
        return Err(Error::LimitExceeded);
    }
    Ok(())
}

fn require_amounts_at_least<const N: usize>(
    actual: LiquidityAmounts<N>,
    minimum: LiquidityAmounts<N>,
) -> Result<()> {
    if actual.principal_collateral < minimum.principal_collateral
        || actual.realized_fee_collateral < minimum.realized_fee_collateral
        || actual
            .claim_reserves
            .iter()
            .zip(minimum.claim_reserves.iter())
            .any(|(actual_claim, minimum_claim)| actual_claim < minimum_claim)
    {
        return Err(Error::LimitExceeded);
    }
    Ok(())
}

fn require_amounts_add<const N: usize>(
    left: LiquidityAmounts<N>,
    right: LiquidityAmounts<N>,
    total: LiquidityAmounts<N>,
) -> Result<()> {
    if left
        .principal_collateral
        .checked_add(right.principal_collateral)
        != Some(total.principal_collateral)
        || left
            .realized_fee_collateral
            .checked_add(right.realized_fee_collateral)
            != Some(total.realized_fee_collateral)
        || left
            .claim_reserves
            .iter()
            .zip(right.claim_reserves.iter())
            .zip(total.claim_reserves.iter())
            .any(|((left_claim, right_claim), total_claim)| {
                left_claim.checked_add(*right_claim) != Some(*total_claim)
            })
    {
        return Err(Error::ConservationMismatch);
    }
    Ok(())
}

fn checked_add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right).ok_or(Error::ArithmeticOverflow)
}

fn checked_sub(left: u64, right: u64) -> Result<u64> {
    left.checked_sub(right).ok_or(Error::ArithmeticOverflow)
}

fn mul_div_floor(left: u64, right: u64, denominator: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::ArithmeticOverflow);
    }
    let product = u128::from(left)
        .checked_mul(u128::from(right))
        .ok_or(Error::ArithmeticOverflow)?;
    u64::try_from(product / u128::from(denominator)).map_err(|_| Error::ArithmeticOverflow)
}

fn mul_div_ceil(left: u64, right: u64, denominator: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::ArithmeticOverflow);
    }
    let product = u128::from(left)
        .checked_mul(u128::from(right))
        .ok_or(Error::ArithmeticOverflow)?;
    let divisor = u128::from(denominator);
    let rounded = product
        .checked_add(divisor.checked_sub(1).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::ArithmeticOverflow)?
        / divisor;
    u64::try_from(rounded).map_err(|_| Error::ArithmeticOverflow)
}

fn all_zero(bytes: &[u8; 32]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn encode_header(out: &mut [u8], magic: [u8; 8]) {
    put(out, 0, &magic);
    put_u16(out, 8, SCHEMA_VERSION);
}

fn decode_header(bytes: &[u8], magic: [u8; 8]) -> Result<()> {
    if read_array::<8>(bytes, 0)? != magic {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, 8)? != SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if subslice(bytes, offset, width)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn subslice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn read_array<const W: usize>(bytes: &[u8], offset: usize) -> Result<[u8; W]> {
    let mut output = [0u8; W];
    output.copy_from_slice(subslice(bytes, offset, W)?);
    Ok(output)
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_vector<const W: usize>(bytes: &[u8], offset: usize) -> Result<[u64; W]> {
    let mut output = [0u64; W];
    for (index, value) in output.iter_mut().enumerate() {
        *value = read_u64(bytes, checked_offset(offset, 8, index)?)?;
    }
    Ok(output)
}

fn read_matrix<const N: usize, const B: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[[u64; B]; N]> {
    let mut output = [[0u64; B]; N];
    for (claim, row) in output.iter_mut().enumerate() {
        for (bin, value) in row.iter_mut().enumerate() {
            let flat = claim
                .checked_mul(B)
                .and_then(|item| item.checked_add(bin))
                .ok_or(Error::ArithmeticOverflow)?;
            *value = read_u64(bytes, checked_offset(offset, 8, flat)?)?;
        }
    }
    Ok(output)
}

fn put(out: &mut [u8], offset: usize, value: &[u8]) {
    let Some(end) = offset.checked_add(value.len()) else {
        return;
    };
    let Some(destination) = out.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(value);
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    put(out, offset, &value.to_le_bytes());
}

fn put_u64(out: &mut [u8], offset: usize, value: u64) {
    put(out, offset, &value.to_le_bytes());
}

fn put_vector<const W: usize>(out: &mut [u8], offset: usize, values: &[u64; W]) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        put_u64(out, checked_offset(offset, 8, index)?, *value);
    }
    Ok(())
}

fn put_matrix<const N: usize, const B: usize>(
    out: &mut [u8],
    offset: usize,
    values: &[[u64; B]; N],
) -> Result<()> {
    for (claim, row) in values.iter().enumerate() {
        for (bin, value) in row.iter().enumerate() {
            let flat = claim
                .checked_mul(B)
                .and_then(|item| item.checked_add(bin))
                .ok_or(Error::ArithmeticOverflow)?;
            put_u64(out, checked_offset(offset, 8, flat)?, *value);
        }
    }
    Ok(())
}

#[cfg(test)]
mod authority_tests;
#[cfg(test)]
mod tests;

/// Lifecycle of one compact physical LP-position account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PositionStatus {
    /// Position holds a positive number of shares.
    Active = 0,
    /// Position is live with zero shares and may be reused or closed.
    Empty = 1,
    /// Rent was routed and the physical account may close.
    Closed = 2,
}

impl PositionStatus {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::Empty),
            2 => Ok(Self::Closed),
            _ => Err(Error::UnknownDiscriminant),
        }
    }
    const fn byte(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Empty => 1,
            Self::Closed => 2,
        }
    }
}

/// Compact persisted share position belonging to one liquidity provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LpPosition {
    parent: ParentPool,
    owner: [u8; 32],
    rent_credit: RentCreditTerms,
    shares: u64,
    next_sequence: u64,
    status: PositionStatus,
}

impl LpPosition {
    fn new(
        parent: ParentPool,
        owner: [u8; 32],
        rent_credit: RentCreditTerms,
        shares: u64,
        status: PositionStatus,
    ) -> Result<Self> {
        if all_zero(&owner) {
            return Err(Error::ZeroIdentity);
        }
        if owner == parent.address {
            return Err(Error::IdentityAlias);
        }
        let position = Self {
            parent,
            owner,
            rent_credit,
            shares,
            next_sequence: 1,
            status,
        };
        position.validate()?;
        Ok(position)
    }

    /// Decode and validate one exact compact LP-position account.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != LP_POSITION_BYTES {
            return Err(Error::InvalidLength);
        }
        decode_header(bytes, POSITION_MAGIC)?;
        require_zero(bytes, POSITION_RESERVED_OFFSET, POSITION_RESERVED_BYTES)?;
        let position = Self {
            parent: ParentPool::decode(subslice(
                bytes,
                POSITION_PARENT_OFFSET,
                PARENT_POOL_BYTES,
            )?)?,
            owner: read_array(bytes, POSITION_OWNER_OFFSET)?,
            rent_credit: RentCreditTerms::decode(subslice(
                bytes,
                POSITION_RENT_OFFSET,
                RENT_CREDIT_TERMS_BYTES,
            )?)?,
            shares: read_u64(bytes, POSITION_SHARES_OFFSET)?,
            next_sequence: read_u64(bytes, POSITION_SEQUENCE_OFFSET)?,
            status: PositionStatus::decode(read_u8(bytes, POSITION_STATUS_OFFSET)?)?,
        };
        position.validate()?;
        Ok(position)
    }

    /// Encode one exact compact LP-position account.
    pub fn to_bytes(&self) -> Result<[u8; LP_POSITION_BYTES]> {
        self.validate()?;
        let mut out = [0u8; LP_POSITION_BYTES];
        encode_header(&mut out, POSITION_MAGIC);
        put(&mut out, POSITION_PARENT_OFFSET, &self.parent.to_bytes());
        put(&mut out, POSITION_OWNER_OFFSET, &self.owner);
        put(&mut out, POSITION_RENT_OFFSET, &self.rent_credit.to_bytes());
        put_u64(&mut out, POSITION_SHARES_OFFSET, self.shares);
        put_u64(&mut out, POSITION_SEQUENCE_OFFSET, self.next_sequence);
        put(&mut out, POSITION_STATUS_OFFSET, &[self.status.byte()]);
        Ok(out)
    }

    /// Validate compact parent, owner, share, and lifecycle facts.
    pub fn validate(&self) -> Result<()> {
        if all_zero(&self.owner) || self.next_sequence == 0 {
            return Err(Error::PositionMismatch);
        }
        if self.owner == self.parent.address {
            return Err(Error::IdentityAlias);
        }
        match self.status {
            PositionStatus::Active if self.shares == 0 => Err(Error::ShareInvariant),
            PositionStatus::Empty | PositionStatus::Closed if self.shares != 0 => {
                Err(Error::ShareInvariant)
            }
            _ => Ok(()),
        }
    }

    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return sole share owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }
    /// Return exact funded rent attribution.
    pub const fn rent_credit(&self) -> RentCreditTerms {
        self.rent_credit
    }
    /// Return position shares.
    pub const fn shares(&self) -> u64 {
        self.shares
    }
    /// Return next position-local replay sequence.
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
    /// Return lifecycle status.
    pub const fn status(&self) -> PositionStatus {
        self.status
    }

    fn require_for_pool<const N: usize, const B: usize>(
        &self,
        parent: ParentPool,
        pool: &PoolState<N, B>,
        expected_sequence: u64,
    ) -> Result<()> {
        self.validate()?;
        if self.parent != parent {
            return Err(Error::ParentMismatch);
        }
        if self.status == PositionStatus::Closed {
            return Err(Error::InvalidPositionStatus);
        }
        if self.next_sequence != expected_sequence {
            return Err(Error::SequenceMismatch);
        }
        if self.shares > pool.total_shares {
            return Err(Error::ShareInvariant);
        }
        Ok(())
    }

    fn bump_sequence(&mut self) -> Result<()> {
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Kind of proportional LP custody change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiquidityChangeKind {
    /// First LP funded every required compartment.
    Open,
    /// An LP deposited the conservative proportional vector.
    Add,
    /// An LP withdrew the conservative proportional vector.
    Remove,
}

/// Exact adapter-applied LP custody and share delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityChangeReceipt<const N: usize> {
    kind: LiquidityChangeKind,
    parent: ParentPool,
    pool_sequence: u64,
    position_id: [u8; 32],
    owner: [u8; 32],
    amounts_before: LiquidityAmounts<N>,
    amounts_transferred: LiquidityAmounts<N>,
    amounts_after: LiquidityAmounts<N>,
    total_shares_before: u64,
    shares_changed: u64,
    total_shares_after: u64,
    position_shares_before: u64,
    position_shares_after: u64,
}

impl<const N: usize> LiquidityChangeReceipt<N> {
    /// Validate physical identities, every compartment, and share conservation.
    pub fn validate(&self) -> Result<()> {
        validate_position_identity(self.position_id, self.parent, self.owner)?;
        if self.shares_changed == 0 {
            return Err(Error::ConservationMismatch);
        }
        match self.kind {
            LiquidityChangeKind::Open | LiquidityChangeKind::Add => {
                require_amounts_add(
                    self.amounts_before,
                    self.amounts_transferred,
                    self.amounts_after,
                )?;
                if self.total_shares_before.checked_add(self.shares_changed)
                    != Some(self.total_shares_after)
                    || self.position_shares_before.checked_add(self.shares_changed)
                        != Some(self.position_shares_after)
                {
                    return Err(Error::ConservationMismatch);
                }
            }
            LiquidityChangeKind::Remove => {
                require_amounts_add(
                    self.amounts_after,
                    self.amounts_transferred,
                    self.amounts_before,
                )?;
                if self.total_shares_after.checked_add(self.shares_changed)
                    != Some(self.total_shares_before)
                    || self.position_shares_after.checked_add(self.shares_changed)
                        != Some(self.position_shares_before)
                {
                    return Err(Error::ConservationMismatch);
                }
            }
        }
        Ok(())
    }

    /// Return change kind.
    pub const fn kind(&self) -> LiquidityChangeKind {
        self.kind
    }
    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return accepted global Pool sequence.
    pub const fn pool_sequence(&self) -> u64 {
        self.pool_sequence
    }
    /// Return transient physical position identity authenticated by adapter.
    pub const fn position_id(&self) -> [u8; 32] {
        self.position_id
    }
    /// Return position owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }
    /// Return LP custody before change.
    pub const fn amounts_before(&self) -> LiquidityAmounts<N> {
        self.amounts_before
    }
    /// Return exact deposit or withdrawal vector.
    pub const fn amounts_transferred(&self) -> LiquidityAmounts<N> {
        self.amounts_transferred
    }
    /// Return LP custody after change.
    pub const fn amounts_after(&self) -> LiquidityAmounts<N> {
        self.amounts_after
    }
    /// Return total shares before change.
    pub const fn total_shares_before(&self) -> u64 {
        self.total_shares_before
    }
    /// Return minted or burned shares.
    pub const fn shares_changed(&self) -> u64 {
        self.shares_changed
    }
    /// Return total shares after change.
    pub const fn total_shares_after(&self) -> u64 {
        self.total_shares_after
    }
    /// Return position shares before change.
    pub const fn position_shares_before(&self) -> u64 {
        self.position_shares_before
    }
    /// Return position shares after change.
    pub const fn position_shares_after(&self) -> u64 {
        self.position_shares_after
    }
}

/// Exact maximum-deposit request for newly minted shares.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddLiquidityRequest<const N: usize> {
    expected_pool_sequence: u64,
    expected_position_sequence: u64,
    shares_to_mint: u64,
    maximum_deposit: LiquidityAmounts<N>,
}

impl<const N: usize> AddLiquidityRequest<N> {
    /// Construct one bounded proportional deposit request.
    pub fn new(
        expected_pool_sequence: u64,
        expected_position_sequence: u64,
        shares_to_mint: u64,
        maximum_deposit: LiquidityAmounts<N>,
    ) -> Result<Self> {
        if shares_to_mint == 0 {
            return Err(Error::InvalidQuantity);
        }
        Ok(Self {
            expected_pool_sequence,
            expected_position_sequence,
            shares_to_mint,
            maximum_deposit,
        })
    }

    /// Return Pool replay guard.
    pub const fn expected_pool_sequence(self) -> u64 {
        self.expected_pool_sequence
    }

    /// Return position-local replay guard.
    pub const fn expected_position_sequence(self) -> u64 {
        self.expected_position_sequence
    }

    /// Return exact shares requested for minting.
    pub const fn shares_to_mint(self) -> u64 {
        self.shares_to_mint
    }

    /// Return caller's exact per-compartment maximum deposit vector.
    pub const fn maximum_deposit(self) -> LiquidityAmounts<N> {
        self.maximum_deposit
    }
}

/// Exact minimum-withdrawal request for burned shares.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoveLiquidityRequest<const N: usize> {
    expected_pool_sequence: u64,
    expected_position_sequence: u64,
    shares_to_burn: u64,
    minimum_withdrawal: LiquidityAmounts<N>,
}

impl<const N: usize> RemoveLiquidityRequest<N> {
    /// Construct one bounded proportional withdrawal request.
    pub fn new(
        expected_pool_sequence: u64,
        expected_position_sequence: u64,
        shares_to_burn: u64,
        minimum_withdrawal: LiquidityAmounts<N>,
    ) -> Result<Self> {
        if shares_to_burn == 0 {
            return Err(Error::InvalidQuantity);
        }
        Ok(Self {
            expected_pool_sequence,
            expected_position_sequence,
            shares_to_burn,
            minimum_withdrawal,
        })
    }

    /// Return Pool replay guard.
    pub const fn expected_pool_sequence(self) -> u64 {
        self.expected_pool_sequence
    }

    /// Return position-local replay guard.
    pub const fn expected_position_sequence(self) -> u64 {
        self.expected_position_sequence
    }

    /// Return exact shares requested for burning.
    pub const fn shares_to_burn(self) -> u64 {
        self.shares_to_burn
    }

    /// Return caller's exact per-compartment minimum withdrawal vector.
    pub const fn minimum_withdrawal(self) -> LiquidityAmounts<N> {
        self.minimum_withdrawal
    }
}

/// Receipt for creating a zero-share child position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionCreationReceipt {
    parent: ParentPool,
    pool_sequence: u64,
    position_id: [u8; 32],
    owner: [u8; 32],
    rent_credit: RentCreditTerms,
}

impl PositionCreationReceipt {
    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return accepted global Pool sequence.
    pub const fn pool_sequence(&self) -> u64 {
        self.pool_sequence
    }
    /// Return transient new position identity.
    pub const fn position_id(&self) -> [u8; 32] {
        self.position_id
    }
    /// Return new position owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }
    /// Return exact funded rent attribution.
    pub const fn rent_credit(&self) -> RentCreditTerms {
        self.rent_credit
    }
}

/// Exact RentCredit routing emitted when a position becomes closeable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionCloseReceipt {
    parent: ParentPool,
    pool_sequence: u64,
    position_id: [u8; 32],
    owner: [u8; 32],
    rent_credit: RentCreditTerms,
}

impl PositionCloseReceipt {
    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return accepted global Pool sequence.
    pub const fn pool_sequence(&self) -> u64 {
        self.pool_sequence
    }
    /// Return transient closed position identity.
    pub const fn position_id(&self) -> [u8; 32] {
        self.position_id
    }
    /// Return closed position owner.
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }
    /// Return funded principal attribution; all actual lamports go to its RentCredit.
    pub const fn rent_credit(&self) -> RentCreditTerms {
        self.rent_credit
    }
}

impl<const N: usize, const B: usize> PoolState<N, B> {
    /// Create a reusable zero-share LP position under the Pool replay clock.
    #[allow(clippy::too_many_arguments)]
    pub fn create_position(
        &mut self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        expected_pool_sequence: u64,
        position_id: [u8; 32],
        owner: [u8; 32],
        rent_credit: RentCreditTerms,
    ) -> Result<(LpPosition, PositionCreationReceipt)> {
        self.require_active(pool_address, config)?;
        self.require_sequence(expected_pool_sequence)?;
        let parent = parent_for(self.attachment, pool_address)?;
        validate_position_identity(position_id, parent, owner)?;
        let position = LpPosition::new(parent, owner, rent_credit, 0, PositionStatus::Empty)?;
        let mut next = *self;
        next.live_positions = checked_add(next.live_positions, 1)?;
        let sequence = next.bump_sequence()?;
        next.validate_against(pool_address, config)?;
        *self = next;
        Ok((
            position,
            PositionCreationReceipt {
                parent,
                pool_sequence: sequence,
                position_id,
                owner,
                rent_credit,
            },
        ))
    }

    /// Deposit every LP-owned compartment proportionally with ceiling rounding.
    pub fn add_liquidity(
        &mut self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        position_id: [u8; 32],
        position: &mut LpPosition,
        request: AddLiquidityRequest<N>,
    ) -> Result<LiquidityChangeReceipt<N>> {
        self.require_active(pool_address, config)?;
        self.require_sequence(request.expected_pool_sequence)?;
        let parent = parent_for(self.attachment, pool_address)?;
        position.require_for_pool(parent, self, request.expected_position_sequence)?;
        validate_position_identity(position_id, parent, position.owner)?;
        let before = self.liquidity();
        let required =
            proportional_amounts(before, request.shares_to_mint, self.total_shares, true)?;
        require_amounts_at_most(required, request.maximum_deposit)?;
        let mut next = *self;
        let mut next_position = *position;
        next.principal_collateral =
            checked_add(next.principal_collateral, required.principal_collateral)?;
        next.realized_fee_collateral = checked_add(
            next.realized_fee_collateral,
            required.realized_fee_collateral,
        )?;
        for (reserve, deposit) in next
            .claim_reserves
            .iter_mut()
            .zip(required.claim_reserves.iter())
        {
            *reserve = checked_add(*reserve, *deposit)?;
        }
        let total_before = next.total_shares;
        let position_before = next_position.shares;
        next.total_shares = checked_add(next.total_shares, request.shares_to_mint)?;
        next_position.shares = checked_add(next_position.shares, request.shares_to_mint)?;
        next_position.status = PositionStatus::Active;
        let sequence = next.bump_sequence()?;
        next_position.bump_sequence()?;
        let receipt = LiquidityChangeReceipt {
            kind: LiquidityChangeKind::Add,
            parent,
            pool_sequence: sequence,
            position_id,
            owner: next_position.owner,
            amounts_before: before,
            amounts_transferred: required,
            amounts_after: next.liquidity(),
            total_shares_before: total_before,
            shares_changed: request.shares_to_mint,
            total_shares_after: next.total_shares,
            position_shares_before: position_before,
            position_shares_after: next_position.shares,
        };
        receipt.validate()?;
        next.validate_against(pool_address, config)?;
        next_position.validate()?;
        *self = next;
        *position = next_position;
        Ok(receipt)
    }

    /// Withdraw every LP-owned compartment proportionally with floor rounding.
    pub fn remove_liquidity(
        &mut self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        position_id: [u8; 32],
        position: &mut LpPosition,
        request: RemoveLiquidityRequest<N>,
    ) -> Result<LiquidityChangeReceipt<N>> {
        self.require_active(pool_address, config)?;
        self.require_sequence(request.expected_pool_sequence)?;
        let parent = parent_for(self.attachment, pool_address)?;
        position.require_for_pool(parent, self, request.expected_position_sequence)?;
        validate_position_identity(position_id, parent, position.owner)?;
        if position.status != PositionStatus::Active
            || request.shares_to_burn > position.shares
            || request.shares_to_burn > self.total_shares
        {
            return Err(Error::InvalidQuantity);
        }
        let before = self.liquidity();
        let withdrawal = if request.shares_to_burn == self.total_shares {
            before
        } else {
            proportional_amounts(before, request.shares_to_burn, self.total_shares, false)?
        };
        if withdrawal.is_zero() {
            return Err(Error::ZeroNotional);
        }
        require_amounts_at_least(withdrawal, request.minimum_withdrawal)?;
        let mut next = *self;
        let mut next_position = *position;
        next.principal_collateral =
            checked_sub(next.principal_collateral, withdrawal.principal_collateral)?;
        next.realized_fee_collateral = checked_sub(
            next.realized_fee_collateral,
            withdrawal.realized_fee_collateral,
        )?;
        for (reserve, amount) in next
            .claim_reserves
            .iter_mut()
            .zip(withdrawal.claim_reserves.iter())
        {
            *reserve = checked_sub(*reserve, *amount)?;
        }
        let total_before = next.total_shares;
        let position_before = next_position.shares;
        next.total_shares = checked_sub(next.total_shares, request.shares_to_burn)?;
        next_position.shares = checked_sub(next_position.shares, request.shares_to_burn)?;
        if next_position.shares == 0 {
            next_position.status = PositionStatus::Empty;
        }
        if next.total_shares == 0 {
            if !next.liquidity().is_zero() {
                return Err(Error::ConservationMismatch);
            }
            next.status = PoolStatus::Retiring;
        }
        let sequence = next.bump_sequence()?;
        next_position.bump_sequence()?;
        let receipt = LiquidityChangeReceipt {
            kind: LiquidityChangeKind::Remove,
            parent,
            pool_sequence: sequence,
            position_id,
            owner: next_position.owner,
            amounts_before: before,
            amounts_transferred: withdrawal,
            amounts_after: next.liquidity(),
            total_shares_before: total_before,
            shares_changed: request.shares_to_burn,
            total_shares_after: next.total_shares,
            position_shares_before: position_before,
            position_shares_after: next_position.shares,
        };
        receipt.validate()?;
        next.validate_against(pool_address, config)?;
        next_position.validate()?;
        *self = next;
        *position = next_position;
        Ok(receipt)
    }

    /// Close an empty compact position and expose exact RentCredit attribution.
    pub fn close_position(
        &mut self,
        pool_address: [u8; 32],
        position_id: [u8; 32],
        position: &mut LpPosition,
        expected_pool_sequence: u64,
        expected_position_sequence: u64,
    ) -> Result<PositionCloseReceipt> {
        self.validate()?;
        if self.status == PoolStatus::Retired {
            return Err(Error::InvalidPoolStatus);
        }
        self.require_sequence(expected_pool_sequence)?;
        let parent = parent_for(self.attachment, pool_address)?;
        position.require_for_pool(parent, self, expected_position_sequence)?;
        validate_position_identity(position_id, parent, position.owner)?;
        if position.status != PositionStatus::Empty || position.shares != 0 {
            return Err(Error::InvalidPositionStatus);
        }
        let mut next = *self;
        let mut next_position = *position;
        next.live_positions = next
            .live_positions
            .checked_sub(1)
            .ok_or(Error::ShareInvariant)?;
        let sequence = next.bump_sequence()?;
        next_position.status = PositionStatus::Closed;
        next_position.bump_sequence()?;
        let receipt = PositionCloseReceipt {
            parent,
            pool_sequence: sequence,
            position_id,
            owner: next_position.owner,
            rent_credit: next_position.rent_credit,
        };
        next.validate()?;
        next_position.validate()?;
        *self = next;
        *position = next_position;
        Ok(receipt)
    }
}

/// Persistable exact custody delta for one immediate trade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionReceipt<const B: usize> {
    parent: ParentPool,
    reset_number: u64,
    sequence: u64,
    side: TradeSide,
    claim_index: u8,
    quantity: u64,
    notional_collateral: u64,
    trader_fee_collateral: u64,
    trader_collateral_debit: u64,
    trader_collateral_credit: u64,
    trader_claim_debit: u64,
    trader_claim_credit: u64,
    principal_before: u64,
    principal_after: u64,
    fees_before: u64,
    fees_after: u64,
    claim_before: u64,
    claim_after: u64,
    bin_before: [u64; B],
    bin_after: [u64; B],
}

impl<const B: usize> ExecutionReceipt<B> {
    /// Return exact selected-profile receipt width.
    pub fn encoded_len() -> Result<usize> {
        validate_bin_profile::<B>()?;
        EXECUTION_BIN_BEFORE_OFFSET
            .checked_add(16usize.checked_mul(B).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Decode and validate one exact compact receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::encoded_len()? {
            return Err(Error::InvalidLength);
        }
        decode_header(bytes, EXECUTION_MAGIC)?;
        require_zero(bytes, EXECUTION_RESERVED_OFFSET, EXECUTION_RESERVED_BYTES)?;
        let receipt = Self {
            parent: ParentPool::decode(subslice(
                bytes,
                EXECUTION_PARENT_OFFSET,
                PARENT_POOL_BYTES,
            )?)?,
            reset_number: read_u64(bytes, EXECUTION_RESET_OFFSET)?,
            sequence: read_u64(bytes, EXECUTION_SEQUENCE_OFFSET)?,
            side: TradeSide::decode(read_u8(bytes, EXECUTION_SIDE_OFFSET)?)?,
            claim_index: read_u8(bytes, EXECUTION_CLAIM_OFFSET)?,
            quantity: read_u64(bytes, EXECUTION_QUANTITY_OFFSET)?,
            notional_collateral: read_u64(bytes, EXECUTION_NOTIONAL_OFFSET)?,
            trader_fee_collateral: read_u64(bytes, EXECUTION_FEE_OFFSET)?,
            trader_collateral_debit: read_u64(bytes, EXECUTION_TRADER_COLLATERAL_DEBIT_OFFSET)?,
            trader_collateral_credit: read_u64(bytes, EXECUTION_TRADER_COLLATERAL_CREDIT_OFFSET)?,
            trader_claim_debit: read_u64(bytes, EXECUTION_TRADER_CLAIM_DEBIT_OFFSET)?,
            trader_claim_credit: read_u64(bytes, EXECUTION_TRADER_CLAIM_CREDIT_OFFSET)?,
            principal_before: read_u64(bytes, EXECUTION_PRINCIPAL_BEFORE_OFFSET)?,
            principal_after: read_u64(bytes, EXECUTION_PRINCIPAL_AFTER_OFFSET)?,
            fees_before: read_u64(bytes, EXECUTION_FEES_BEFORE_OFFSET)?,
            fees_after: read_u64(bytes, EXECUTION_FEES_AFTER_OFFSET)?,
            claim_before: read_u64(bytes, EXECUTION_CLAIM_BEFORE_OFFSET)?,
            claim_after: read_u64(bytes, EXECUTION_CLAIM_AFTER_OFFSET)?,
            bin_before: read_vector(bytes, EXECUTION_BIN_BEFORE_OFFSET)?,
            bin_after: read_vector(bytes, EXECUTION_BIN_BEFORE_OFFSET + 8 * B)?,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Encode into an exact selected-profile destination.
    pub fn encode_into(&self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() != Self::encoded_len()? {
            return Err(Error::InvalidLength);
        }
        out.fill(0);
        encode_header(out, EXECUTION_MAGIC);
        put(out, EXECUTION_PARENT_OFFSET, &self.parent.to_bytes());
        put_u64(out, EXECUTION_RESET_OFFSET, self.reset_number);
        put_u64(out, EXECUTION_SEQUENCE_OFFSET, self.sequence);
        put(out, EXECUTION_SIDE_OFFSET, &[self.side.byte()]);
        put(out, EXECUTION_CLAIM_OFFSET, &[self.claim_index]);
        put_u64(out, EXECUTION_QUANTITY_OFFSET, self.quantity);
        put_u64(out, EXECUTION_NOTIONAL_OFFSET, self.notional_collateral);
        put_u64(out, EXECUTION_FEE_OFFSET, self.trader_fee_collateral);
        put_u64(
            out,
            EXECUTION_TRADER_COLLATERAL_DEBIT_OFFSET,
            self.trader_collateral_debit,
        );
        put_u64(
            out,
            EXECUTION_TRADER_COLLATERAL_CREDIT_OFFSET,
            self.trader_collateral_credit,
        );
        put_u64(
            out,
            EXECUTION_TRADER_CLAIM_DEBIT_OFFSET,
            self.trader_claim_debit,
        );
        put_u64(
            out,
            EXECUTION_TRADER_CLAIM_CREDIT_OFFSET,
            self.trader_claim_credit,
        );
        put_u64(
            out,
            EXECUTION_PRINCIPAL_BEFORE_OFFSET,
            self.principal_before,
        );
        put_u64(out, EXECUTION_PRINCIPAL_AFTER_OFFSET, self.principal_after);
        put_u64(out, EXECUTION_FEES_BEFORE_OFFSET, self.fees_before);
        put_u64(out, EXECUTION_FEES_AFTER_OFFSET, self.fees_after);
        put_u64(out, EXECUTION_CLAIM_BEFORE_OFFSET, self.claim_before);
        put_u64(out, EXECUTION_CLAIM_AFTER_OFFSET, self.claim_after);
        put_vector(out, EXECUTION_BIN_BEFORE_OFFSET, &self.bin_before)?;
        put_vector(out, EXECUTION_BIN_BEFORE_OFFSET + 8 * B, &self.bin_after)?;
        Ok(())
    }

    /// Validate all exact conservation equations without an adapter.
    pub fn validate(&self) -> Result<()> {
        validate_bin_profile::<B>()?;
        if self.quantity == 0 || self.notional_collateral == 0 || self.trader_fee_collateral == 0 {
            return Err(Error::ConservationMismatch);
        }
        if self.fees_before.checked_add(self.trader_fee_collateral) != Some(self.fees_after) {
            return Err(Error::ConservationMismatch);
        }
        let mut filled_quantity = 0u64;
        for (after, before) in self.bin_after.iter().zip(self.bin_before.iter()) {
            let delta = after
                .checked_sub(*before)
                .ok_or(Error::ConservationMismatch)?;
            filled_quantity = checked_add(filled_quantity, delta)?;
        }
        if filled_quantity != self.quantity {
            return Err(Error::ConservationMismatch);
        }
        match self.side {
            TradeSide::BuyClaimFromPool => {
                if self.trader_collateral_debit
                    != checked_add(self.notional_collateral, self.trader_fee_collateral)?
                    || self.trader_collateral_credit != 0
                    || self.trader_claim_debit != 0
                    || self.trader_claim_credit != self.quantity
                    || self.principal_before.checked_add(self.notional_collateral)
                        != Some(self.principal_after)
                    || self.claim_after.checked_add(self.quantity) != Some(self.claim_before)
                {
                    return Err(Error::ConservationMismatch);
                }
            }
            TradeSide::SellClaimToPool => {
                if self.trader_collateral_debit != self.trader_fee_collateral
                    || self.trader_collateral_credit != self.notional_collateral
                    || self.trader_claim_debit != self.quantity
                    || self.trader_claim_credit != 0
                    || self.principal_after.checked_add(self.notional_collateral)
                        != Some(self.principal_before)
                    || self.claim_before.checked_add(self.quantity) != Some(self.claim_after)
                {
                    return Err(Error::ConservationMismatch);
                }
            }
        }
        Ok(())
    }

    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return active ladder reset number.
    pub const fn reset_number(&self) -> u64 {
        self.reset_number
    }
    /// Return accepted global replay sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Return trade direction.
    pub const fn side(&self) -> TradeSide {
        self.side
    }
    /// Return native-claim index.
    pub const fn claim_index(&self) -> u8 {
        self.claim_index
    }
    /// Return claim quantity.
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }
    /// Return principal notional.
    pub const fn notional_collateral(&self) -> u64 {
        self.notional_collateral
    }
    /// Return trader-paid fee.
    pub const fn trader_fee_collateral(&self) -> u64 {
        self.trader_fee_collateral
    }
    /// Return gross trader collateral debit.
    pub const fn trader_collateral_debit(&self) -> u64 {
        self.trader_collateral_debit
    }
    /// Return gross trader collateral credit.
    pub const fn trader_collateral_credit(&self) -> u64 {
        self.trader_collateral_credit
    }
    /// Return trader native-claim debit.
    pub const fn trader_claim_debit(&self) -> u64 {
        self.trader_claim_debit
    }
    /// Return trader native-claim credit.
    pub const fn trader_claim_credit(&self) -> u64 {
        self.trader_claim_credit
    }
}

impl<const N: usize, const B: usize> PoolState<N, B> {
    /// Execute a covered exchange and return exact adapter-applied deltas.
    pub fn execute(
        &mut self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        request: TradeRequest,
    ) -> Result<ExecutionReceipt<B>> {
        let quote = self.quote(pool_address, config, request)?;
        let mut next = *self;
        let claim = quote.claim_index;
        let principal_before = next.principal_collateral;
        let fees_before = next.realized_fee_collateral;
        let claim_before = next
            .claim_reserves
            .get(claim)
            .copied()
            .ok_or(Error::ClaimIndexOutOfRange)?;
        match quote.side {
            TradeSide::BuyClaimFromPool => {
                next.principal_collateral =
                    checked_add(next.principal_collateral, quote.notional_collateral)?;
                let reserve = next
                    .claim_reserves
                    .get_mut(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)?;
                *reserve = reserve
                    .checked_sub(quote.quantity)
                    .ok_or(Error::InsufficientClaimInventory)?;
                *next
                    .ask_filled
                    .get_mut(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)? = quote.bin_after;
            }
            TradeSide::SellClaimToPool => {
                next.principal_collateral = next
                    .principal_collateral
                    .checked_sub(quote.notional_collateral)
                    .ok_or(Error::InsufficientPrincipalCollateral)?;
                let reserve = next
                    .claim_reserves
                    .get_mut(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)?;
                *reserve = checked_add(*reserve, quote.quantity)?;
                *next
                    .bid_filled
                    .get_mut(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)? = quote.bin_after;
            }
        }
        next.realized_fee_collateral =
            checked_add(next.realized_fee_collateral, quote.trader_fee_collateral)?;
        let sequence = next.bump_sequence()?;
        let receipt = ExecutionReceipt {
            parent: quote.parent,
            reset_number: next.reset_number,
            sequence,
            side: quote.side,
            claim_index: u8::try_from(claim).map_err(|_| Error::UnsupportedProfile)?,
            quantity: quote.quantity,
            notional_collateral: quote.notional_collateral,
            trader_fee_collateral: quote.trader_fee_collateral,
            trader_collateral_debit: quote.trader_collateral_debit,
            trader_collateral_credit: quote.trader_collateral_credit,
            trader_claim_debit: quote.trader_claim_debit,
            trader_claim_credit: quote.trader_claim_credit,
            principal_before,
            principal_after: next.principal_collateral,
            fees_before,
            fees_after: next.realized_fee_collateral,
            claim_before,
            claim_after: next
                .claim_reserves
                .get(claim)
                .copied()
                .ok_or(Error::ClaimIndexOutOfRange)?,
            bin_before: quote.bin_before,
            bin_after: quote.bin_after,
        };
        receipt.validate()?;
        next.validate_against(pool_address, config)?;
        *self = next;
        Ok(receipt)
    }
}

/// Direction of the trader's immediate inventory exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TradeSide {
    /// Trader pays collateral and receives one native claim from the Pool.
    BuyClaimFromPool = 0,
    /// Trader pays one native claim plus fee collateral and receives principal.
    SellClaimToPool = 1,
}

impl TradeSide {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::BuyClaimFromPool),
            1 => Ok(Self::SellClaimToPool),
            _ => Err(Error::UnknownDiscriminant),
        }
    }
    const fn byte(self) -> u8 {
        match self {
            Self::BuyClaimFromPool => 0,
            Self::SellClaimToPool => 1,
        }
    }
}

/// Replay-bound, all-or-nothing immediate trade request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TradeRequest {
    reset_number: u64,
    expected_sequence: u64,
    side: TradeSide,
    claim_index: usize,
    quantity: u64,
    collateral_limit: u64,
}

impl TradeRequest {
    /// Construct one request. Buy limit is maximum gross debit; sell limit is
    /// minimum gross principal credit while fee remains a separate debit.
    pub fn new(
        reset_number: u64,
        expected_sequence: u64,
        side: TradeSide,
        claim_index: usize,
        quantity: u64,
        collateral_limit: u64,
    ) -> Result<Self> {
        if quantity == 0 {
            return Err(Error::InvalidQuantity);
        }
        Ok(Self {
            reset_number,
            expected_sequence,
            side,
            claim_index,
            quantity,
            collateral_limit,
        })
    }
    /// Return selected reset number.
    pub const fn reset_number(self) -> u64 {
        self.reset_number
    }
    /// Return selected global replay sequence.
    pub const fn expected_sequence(self) -> u64 {
        self.expected_sequence
    }
    /// Return trade direction.
    pub const fn side(self) -> TradeSide {
        self.side
    }
    /// Return exact native-claim index.
    pub const fn claim_index(self) -> usize {
        self.claim_index
    }
    /// Return exact claim quantity.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
    /// Return maximum buy debit or minimum sell credit.
    pub const fn collateral_limit(self) -> u64 {
        self.collateral_limit
    }
}

/// Deterministic all-or-nothing quote derived entirely from persisted state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TradeQuote<const B: usize> {
    parent: ParentPool,
    reset_number: u64,
    sequence: u64,
    side: TradeSide,
    claim_index: usize,
    quantity: u64,
    notional_collateral: u64,
    trader_fee_collateral: u64,
    trader_collateral_debit: u64,
    trader_collateral_credit: u64,
    trader_claim_debit: u64,
    trader_claim_credit: u64,
    bin_before: [u64; B],
    bin_after: [u64; B],
}

impl<const B: usize> TradeQuote<B> {
    /// Return compact parent Pool reference.
    pub const fn parent(&self) -> ParentPool {
        self.parent
    }
    /// Return current ladder reset number.
    pub const fn reset_number(&self) -> u64 {
        self.reset_number
    }
    /// Return accepted global replay sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Return trade direction.
    pub const fn side(&self) -> TradeSide {
        self.side
    }
    /// Return exact native-claim index.
    pub const fn claim_index(&self) -> usize {
        self.claim_index
    }
    /// Return total claim quantity.
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }
    /// Return principal collateral exchanged before fee.
    pub const fn notional_collateral(&self) -> u64 {
        self.notional_collateral
    }
    /// Return present collateral paid separately by trader as fee.
    pub const fn trader_fee_collateral(&self) -> u64 {
        self.trader_fee_collateral
    }
    /// Return total present collateral debited from trader.
    pub const fn trader_collateral_debit(&self) -> u64 {
        self.trader_collateral_debit
    }
    /// Return total principal collateral credited to trader.
    pub const fn trader_collateral_credit(&self) -> u64 {
        self.trader_collateral_credit
    }
    /// Return exact native claims debited from trader.
    pub const fn trader_claim_debit(&self) -> u64 {
        self.trader_claim_debit
    }
    /// Return exact native claims credited to trader.
    pub const fn trader_claim_credit(&self) -> u64 {
        self.trader_claim_credit
    }
    /// Return selected-side fill counters before execution.
    pub const fn bin_before(&self) -> [u64; B] {
        self.bin_before
    }
    /// Return selected-side fill counters after execution.
    pub const fn bin_after(&self) -> [u64; B] {
        self.bin_after
    }
}

impl<const N: usize, const B: usize> PoolState<N, B> {
    /// Derive one covered, all-or-nothing quote without mutating state.
    pub fn quote(
        &self,
        pool_address: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        request: TradeRequest,
    ) -> Result<TradeQuote<B>> {
        self.require_active(pool_address, config)?;
        self.require_sequence(request.expected_sequence)?;
        if request.reset_number != self.reset_number {
            return Err(Error::InvalidReset);
        }
        if request.claim_index >= N {
            return Err(Error::ClaimIndexOutOfRange);
        }
        if request.quantity == 0 || request.quantity > config.max_trade_quantity {
            return Err(Error::InvalidQuantity);
        }
        let claim = request.claim_index;
        let (fill_row, capacity_row, price_row) = match request.side {
            TradeSide::BuyClaimFromPool => (
                self.ask_filled
                    .get(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)?,
                config
                    .ask_capacity
                    .get(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)?,
                config
                    .ask_prices
                    .get(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)?,
            ),
            TradeSide::SellClaimToPool => (
                self.bid_filled
                    .get(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)?,
                config
                    .bid_capacity
                    .get(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)?,
                config
                    .bid_prices
                    .get(claim)
                    .ok_or(Error::ClaimIndexOutOfRange)?,
            ),
        };
        let mut remaining = request.quantity;
        let mut notional = 0u64;
        let mut before = [0u64; B];
        let mut after = [0u64; B];
        for (((before_slot, after_slot), filled), (capacity, price)) in before
            .iter_mut()
            .zip(after.iter_mut())
            .zip(fill_row.iter())
            .zip(capacity_row.iter().zip(price_row.iter()))
        {
            *before_slot = *filled;
            let available = capacity
                .checked_sub(*filled)
                .ok_or(Error::ConservationMismatch)?;
            let taken = core::cmp::min(remaining, available);
            if taken > 0 {
                let segment = match request.side {
                    TradeSide::BuyClaimFromPool => mul_div_ceil(taken, *price, config.price_scale)?,
                    TradeSide::SellClaimToPool => mul_div_floor(taken, *price, config.price_scale)?,
                };
                if segment == 0 {
                    return Err(Error::ZeroNotional);
                }
                notional = checked_add(notional, segment)?;
                remaining = checked_sub(remaining, taken)?;
                *after_slot = checked_add(*filled, taken)?;
            } else {
                *after_slot = *filled;
            }
        }
        if remaining != 0 {
            return Err(Error::InsufficientBinDepth);
        }
        if notional == 0 {
            return Err(Error::ZeroNotional);
        }
        let fee = mul_div_ceil(
            notional,
            u64::from(config.fee_bps),
            BASIS_POINTS_DENOMINATOR,
        )?;
        let (collateral_debit, collateral_credit, claim_debit, claim_credit) = match request.side {
            TradeSide::BuyClaimFromPool => {
                if self
                    .claim_reserves
                    .get(claim)
                    .copied()
                    .ok_or(Error::ClaimIndexOutOfRange)?
                    < request.quantity
                {
                    return Err(Error::InsufficientClaimInventory);
                }
                let gross = checked_add(notional, fee)?;
                if gross > request.collateral_limit {
                    return Err(Error::LimitExceeded);
                }
                (gross, 0, 0, request.quantity)
            }
            TradeSide::SellClaimToPool => {
                if self.principal_collateral < notional {
                    return Err(Error::InsufficientPrincipalCollateral);
                }
                if notional < request.collateral_limit {
                    return Err(Error::LimitExceeded);
                }
                (fee, notional, request.quantity, 0)
            }
        };
        Ok(TradeQuote {
            parent: parent_for(self.attachment, pool_address)?,
            reset_number: self.reset_number,
            sequence: self.next_sequence,
            side: request.side,
            claim_index: claim,
            quantity: request.quantity,
            notional_collateral: notional,
            trader_fee_collateral: fee,
            trader_collateral_debit: collateral_debit,
            trader_collateral_credit: collateral_credit,
            trader_claim_debit: claim_debit,
            trader_claim_credit: claim_credit,
            bin_before: before,
            bin_after: after,
        })
    }
}
