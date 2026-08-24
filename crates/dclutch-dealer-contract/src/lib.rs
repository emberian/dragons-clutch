#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact, SDK-free contract for one covered Dealer capital facility.
//!
//! A Dealer belongs to exactly one Market identity, generation, and capability
//! release. Cash, native claims, sponsor loss capital, realized fees, and
//! prepaid service funding remain separate compartments. The collateral Hoard
//! is intentionally not representable by this contract.

use core::convert::TryFrom;

use dclutch_core_contract::{ContentId, MARKET_IDENTITY_BYTES, MarketIdentity};

/// Provisional artifact-profile bound on native claims held by one Dealer.
///
/// This is not a mathematical or protocol ontology bound. The lifting path is
/// a paginated inventory child contract with committed aggregate reservations;
/// the quote and receipt semantics remain indexed and unchanged.
pub const MAX_NATIVE_CLAIMS: usize = 16;

/// Mathematical denominator used for fee basis points.
pub const BASIS_POINTS_DENOMINATOR: u64 = 10_000;

/// Reserved epoch marker used only in entry and exit receipts.
pub const NO_EPOCH: u64 = u64::MAX;

/// Exact canonical byte width of [`DealerBinding`].
pub const DEALER_BINDING_BYTES: usize = 264;
/// Exact canonical byte width of [`CapitalSnapshot`].
pub const CAPITAL_SNAPSHOT_BYTES: usize = 160;
/// Exact canonical byte width of [`DealerState`].
pub const DEALER_STATE_BYTES: usize = 736;
/// Exact canonical byte width of [`CapitalEpoch`].
pub const CAPITAL_EPOCH_BYTES: usize = 832;
/// Exact canonical byte width of [`QuoteReservation`].
pub const QUOTE_RESERVATION_BYTES: usize = 344;
/// Exact canonical byte width of [`ExecutionReceipt`].
pub const EXECUTION_RECEIPT_BYTES: usize = 456;
/// Exact canonical byte width of [`CapitalTransitionReceipt`].
pub const CAPITAL_TRANSITION_RECEIPT_BYTES: usize = 952;

const HEADER_BYTES: usize = 16;
const SCHEMA_VERSION: u16 = 1;
const HEADER_RESERVED_OFFSET: usize = 10;
const HEADER_RESERVED_BYTES: usize = 6;

const STATE_MAGIC: [u8; 8] = *b"DCLTDEAL";
const EPOCH_MAGIC: [u8; 8] = *b"DCLTEPOC";
const QUOTE_MAGIC: [u8; 8] = *b"DCLTQUOT";
const EXECUTION_MAGIC: [u8; 8] = *b"DCLTEXEC";
const CAPITAL_MAGIC: [u8; 8] = *b"DCLTCAPT";

const BINDING_MARKET_OFFSET: usize = 0;
const BINDING_RELEASE_OFFSET: usize = MARKET_IDENTITY_BYTES;
const BINDING_DEALER_OFFSET: usize = BINDING_RELEASE_OFFSET + 32;
const BINDING_SPONSOR_OFFSET: usize = BINDING_DEALER_OFFSET + 32;

const STATE_BINDING_OFFSET: usize = HEADER_BYTES;
const STATE_EPOCH_OFFSET: usize = STATE_BINDING_OFFSET + DEALER_BINDING_BYTES;
const STATE_SEQUENCE_OFFSET: usize = STATE_EPOCH_OFFSET + 8;
const STATE_STATUS_OFFSET: usize = STATE_SEQUENCE_OFFSET + 8;
const STATE_CLAIM_COUNT_OFFSET: usize = STATE_STATUS_OFFSET + 1;
const STATE_BODY_RESERVED_OFFSET: usize = STATE_CLAIM_COUNT_OFFSET + 1;
const STATE_BODY_RESERVED_BYTES: usize = 6;
const STATE_OUTSTANDING_OFFSET: usize = STATE_BODY_RESERVED_OFFSET + STATE_BODY_RESERVED_BYTES;
const STATE_CASH_OFFSET: usize = STATE_OUTSTANDING_OFFSET + 8;
const STATE_LOSS_OFFSET: usize = STATE_CASH_OFFSET + 8;
const STATE_FEES_OFFSET: usize = STATE_LOSS_OFFSET + 8;
const STATE_SERVICE_OFFSET: usize = STATE_FEES_OFFSET + 8;
const STATE_RESERVED_CASH_OFFSET: usize = STATE_SERVICE_OFFSET + 8;
const STATE_INVENTORY_OFFSET: usize = STATE_RESERVED_CASH_OFFSET + 8;
const STATE_RESERVED_OUT_OFFSET: usize = STATE_INVENTORY_OFFSET + MAX_NATIVE_CLAIMS * 8;
const STATE_RESERVED_IN_OFFSET: usize = STATE_RESERVED_OUT_OFFSET + MAX_NATIVE_CLAIMS * 8;

const EPOCH_BINDING_OFFSET: usize = HEADER_BYTES;
const EPOCH_NUMBER_OFFSET: usize = EPOCH_BINDING_OFFSET + DEALER_BINDING_BYTES;
const EPOCH_CLAIM_COUNT_OFFSET: usize = EPOCH_NUMBER_OFFSET + 8;
const EPOCH_FEE_BPS_OFFSET: usize = EPOCH_CLAIM_COUNT_OFFSET + 1;
const EPOCH_RESERVED_OFFSET: usize = EPOCH_FEE_BPS_OFFSET + 2;
const EPOCH_RESERVED_BYTES: usize = 5;
const EPOCH_PRICE_SCALE_OFFSET: usize = EPOCH_RESERVED_OFFSET + EPOCH_RESERVED_BYTES;
const EPOCH_MAX_LIFETIME_OFFSET: usize = EPOCH_PRICE_SCALE_OFFSET + 8;
const EPOCH_MAX_QUANTITY_OFFSET: usize = EPOCH_MAX_LIFETIME_OFFSET + 8;
const EPOCH_CAPS_OFFSET: usize = EPOCH_MAX_QUANTITY_OFFSET + 8;
const EPOCH_RISK_OFFSET: usize = EPOCH_CAPS_OFFSET + MAX_NATIVE_CLAIMS * 8;
const EPOCH_BID_OFFSET: usize = EPOCH_RISK_OFFSET + MAX_NATIVE_CLAIMS * 8;
const EPOCH_ASK_OFFSET: usize = EPOCH_BID_OFFSET + MAX_NATIVE_CLAIMS * 8;

const QUOTE_BINDING_OFFSET: usize = HEADER_BYTES;
const QUOTE_EPOCH_OFFSET: usize = QUOTE_BINDING_OFFSET + DEALER_BINDING_BYTES;
const QUOTE_SEQUENCE_OFFSET: usize = QUOTE_EPOCH_OFFSET + 8;
const QUOTE_SIDE_OFFSET: usize = QUOTE_SEQUENCE_OFFSET + 8;
const QUOTE_CLAIM_OFFSET: usize = QUOTE_SIDE_OFFSET + 1;
const QUOTE_STATUS_OFFSET: usize = QUOTE_CLAIM_OFFSET + 1;
const QUOTE_RESERVED_OFFSET: usize = QUOTE_STATUS_OFFSET + 1;
const QUOTE_RESERVED_BYTES: usize = 5;
const QUOTE_QUANTITY_OFFSET: usize = QUOTE_RESERVED_OFFSET + QUOTE_RESERVED_BYTES;
const QUOTE_NOTIONAL_OFFSET: usize = QUOTE_QUANTITY_OFFSET + 8;
const QUOTE_FEE_OFFSET: usize = QUOTE_NOTIONAL_OFFSET + 8;
const QUOTE_CUSTOMER_CASH_OFFSET: usize = QUOTE_FEE_OFFSET + 8;
const QUOTE_EXPIRY_OFFSET: usize = QUOTE_CUSTOMER_CASH_OFFSET + 8;

const EXECUTION_BINDING_OFFSET: usize = HEADER_BYTES;
const EXECUTION_EPOCH_OFFSET: usize = EXECUTION_BINDING_OFFSET + DEALER_BINDING_BYTES;
const EXECUTION_SEQUENCE_OFFSET: usize = EXECUTION_EPOCH_OFFSET + 8;
const EXECUTION_SIDE_OFFSET: usize = EXECUTION_SEQUENCE_OFFSET + 8;
const EXECUTION_CLAIM_OFFSET: usize = EXECUTION_SIDE_OFFSET + 1;
const EXECUTION_RESERVED_OFFSET: usize = EXECUTION_CLAIM_OFFSET + 1;
const EXECUTION_RESERVED_BYTES: usize = 6;
const EXECUTION_QUANTITY_OFFSET: usize = EXECUTION_RESERVED_OFFSET + EXECUTION_RESERVED_BYTES;
const EXECUTION_NOTIONAL_OFFSET: usize = EXECUTION_QUANTITY_OFFSET + 8;
const EXECUTION_FEE_OFFSET: usize = EXECUTION_NOTIONAL_OFFSET + 8;
const EXECUTION_CUSTOMER_CASH_OFFSET: usize = EXECUTION_FEE_OFFSET + 8;
const EXECUTION_EXPIRY_OFFSET: usize = EXECUTION_CUSTOMER_CASH_OFFSET + 8;
const EXECUTION_VALUES_OFFSET: usize = EXECUTION_EXPIRY_OFFSET + 8;

const CAPITAL_BINDING_OFFSET: usize = HEADER_BYTES;
const CAPITAL_SEQUENCE_OFFSET: usize = CAPITAL_BINDING_OFFSET + DEALER_BINDING_BYTES;
const CAPITAL_KIND_OFFSET: usize = CAPITAL_SEQUENCE_OFFSET + 8;
const CAPITAL_RESERVED_OFFSET: usize = CAPITAL_KIND_OFFSET + 1;
const CAPITAL_RESERVED_BYTES: usize = 7;
const CAPITAL_OLD_EPOCH_OFFSET: usize = CAPITAL_RESERVED_OFFSET + CAPITAL_RESERVED_BYTES;
const CAPITAL_NEW_EPOCH_OFFSET: usize = CAPITAL_OLD_EPOCH_OFFSET + 8;
const CAPITAL_OLD_OFFSET: usize = CAPITAL_NEW_EPOCH_OFFSET + 8;
const CAPITAL_DEPOSITS_OFFSET: usize = CAPITAL_OLD_OFFSET + CAPITAL_SNAPSHOT_BYTES;
const CAPITAL_WITHDRAWALS_OFFSET: usize = CAPITAL_DEPOSITS_OFFSET + CAPITAL_SNAPSHOT_BYTES;
const CAPITAL_NEW_OFFSET: usize = CAPITAL_WITHDRAWALS_OFFSET + CAPITAL_SNAPSHOT_BYTES;

/// Refusal from canonical decoding or a Dealer state transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A byte slice did not have the one exact width.
    InvalidLength,
    /// Magic bytes did not identify the expected contract.
    InvalidMagic,
    /// The schema version is not implemented.
    UnsupportedSchema,
    /// Reserved bytes were nonzero.
    NonCanonicalReservedBytes,
    /// An opaque account or release identity was zero.
    ZeroIdentity,
    /// The Market identity did not match the Dealer binding.
    MarketMismatch,
    /// The immutable Market generation was stale.
    GenerationMismatch,
    /// The capability release did not match the Dealer binding.
    CapabilityReleaseMismatch,
    /// The Dealer or sponsor identity did not match.
    DealerBindingMismatch,
    /// A claim count was outside the provisional artifact profile.
    InvalidClaimCount,
    /// An inactive claim array entry was nonzero.
    NonCanonicalInactiveClaim,
    /// A discriminant was unknown.
    UnknownDiscriminant,
    /// A price scale was zero.
    ZeroPriceScale,
    /// A bid exceeded its ask.
    CrossedPrices,
    /// Fee basis points exceeded the mathematical denominator.
    InvalidFeeRate,
    /// A quote lifetime or maximum quantity was zero.
    EmptyQuotePolicy,
    /// An epoch used the reserved sentinel or was not the exact successor.
    InvalidEpoch,
    /// The supplied epoch was stale.
    EpochMismatch,
    /// The expected operation sequence was stale.
    SequenceMismatch,
    /// A quote quantity was zero or exceeded the epoch maximum.
    InvalidQuantity,
    /// A claim index was not active in this Dealer.
    ClaimIndexOutOfRange,
    /// Quote expiry was not in the admitted current-slot window.
    InvalidExpiry,
    /// Exact price arithmetic rounded to zero cash atoms.
    ZeroNotional,
    /// A quote child did not reproduce the immutable epoch price and fee.
    QuoteTermsMismatch,
    /// Arithmetic did not fit the exact fixed-width representation.
    ArithmeticOverflow,
    /// Segregated Dealer cash could not cover a customer sale.
    InsufficientCash,
    /// Native inventory could not cover a customer purchase.
    InsufficientInventory,
    /// A projected inventory exceeded its immutable epoch cap.
    InventoryCapExceeded,
    /// Sponsor loss capital did not cover immutable epoch risk weights.
    InsufficientLossCapital,
    /// The Dealer is not active.
    DealerNotActive,
    /// The quote child was already executed or cancelled.
    QuoteNotActive,
    /// The quote expired before execution.
    QuoteExpired,
    /// Aggregate quote reservations were structurally inconsistent.
    ReservationInvariant,
    /// Capital changed while reservations were live.
    DealerNotQuiescent,
    /// A capital flow attempted both deposit and withdrawal in one compartment.
    OverlappingCapitalFlow,
    /// A requested withdrawal exceeded its exact compartment.
    InsufficientCapital,
    /// Old, external, and new capital did not conserve exactly.
    CapitalConservationMismatch,
    /// An exited Dealer retained capital or reservations.
    ExitedDealerNotEmpty,
    /// A receipt transition kind did not match its epoch sentinels.
    InvalidCapitalTransition,
}

/// Result alias for Dealer contract operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Immutable attachment of one Dealer to one Market and capability release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerBinding {
    market: MarketIdentity,
    capability_release_id: ContentId,
    dealer_id: [u8; 32],
    sponsor: [u8; 32],
}

impl DealerBinding {
    /// Construct and validate one immutable Dealer attachment.
    pub fn new(
        market: MarketIdentity,
        capability_release_id: ContentId,
        dealer_id: [u8; 32],
        sponsor: [u8; 32],
    ) -> Result<Self> {
        if all_zero(&dealer_id) || all_zero(&sponsor) {
            return Err(Error::ZeroIdentity);
        }
        Ok(Self {
            market,
            capability_release_id,
            dealer_id,
            sponsor,
        })
    }

    /// Decode one exact canonical binding.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != DEALER_BINDING_BYTES {
            return Err(Error::InvalidLength);
        }
        let market = MarketIdentity::decode(subslice(
            bytes,
            BINDING_MARKET_OFFSET,
            MARKET_IDENTITY_BYTES,
        )?)
        .map_err(|_| Error::MarketMismatch)?;
        let release = ContentId::decode(subslice(bytes, BINDING_RELEASE_OFFSET, 32)?)
            .map_err(|_| Error::ZeroIdentity)?;
        Self::new(
            market,
            release,
            read_array(bytes, BINDING_DEALER_OFFSET)?,
            read_array(bytes, BINDING_SPONSOR_OFFSET)?,
        )
    }

    /// Encode this exact canonical binding.
    pub fn to_bytes(self) -> [u8; DEALER_BINDING_BYTES] {
        let mut out = [0u8; DEALER_BINDING_BYTES];
        put(&mut out, BINDING_MARKET_OFFSET, &self.market.to_bytes());
        put(
            &mut out,
            BINDING_RELEASE_OFFSET,
            self.capability_release_id.as_bytes(),
        );
        put(&mut out, BINDING_DEALER_OFFSET, &self.dealer_id);
        put(&mut out, BINDING_SPONSOR_OFFSET, &self.sponsor);
        out
    }

    /// Return the immutable Market identity.
    pub const fn market(self) -> MarketIdentity {
        self.market
    }

    /// Return the immutable capability release identity.
    pub const fn capability_release_id(self) -> ContentId {
        self.capability_release_id
    }

    /// Return the adapter-authenticated Dealer account identity.
    pub const fn dealer_id(self) -> [u8; 32] {
        self.dealer_id
    }

    /// Return the sponsor authorized by the composing adapter.
    pub const fn sponsor(self) -> [u8; 32] {
        self.sponsor
    }

    fn require_generation(self, expected: u64) -> Result<()> {
        if self.market.generation() != expected {
            return Err(Error::GenerationMismatch);
        }
        Ok(())
    }

    fn require_same(self, other: Self) -> Result<()> {
        if self.market.generation() != other.market.generation() {
            return Err(Error::GenerationMismatch);
        }
        if self.capability_release_id != other.capability_release_id {
            return Err(Error::CapabilityReleaseMismatch);
        }
        if self.market != other.market {
            return Err(Error::MarketMismatch);
        }
        if self.dealer_id != other.dealer_id || self.sponsor != other.sponsor {
            return Err(Error::DealerBindingMismatch);
        }
        Ok(())
    }
}

/// Exact five-compartment capital and native-claim inventory snapshot.
///
/// The four cash-like values have distinct owners and uses. In particular,
/// only `cash` covers bid execution; loss capital, fees, and service funding
/// cannot leak into quote settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapitalSnapshot {
    cash: u64,
    sponsor_loss_capital: u64,
    realized_fees: u64,
    prepaid_service_funding: u64,
    inventory: [u64; MAX_NATIVE_CLAIMS],
}

impl CapitalSnapshot {
    /// Construct one exact compartment snapshot.
    pub const fn new(
        cash: u64,
        sponsor_loss_capital: u64,
        realized_fees: u64,
        prepaid_service_funding: u64,
        inventory: [u64; MAX_NATIVE_CLAIMS],
    ) -> Self {
        Self {
            cash,
            sponsor_loss_capital,
            realized_fees,
            prepaid_service_funding,
            inventory,
        }
    }

    /// Return the all-zero snapshot.
    pub const fn zero() -> Self {
        Self::new(0, 0, 0, 0, [0; MAX_NATIVE_CLAIMS])
    }

    /// Decode one exact canonical snapshot.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CAPITAL_SNAPSHOT_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(Self {
            cash: read_u64(bytes, 0)?,
            sponsor_loss_capital: read_u64(bytes, 8)?,
            realized_fees: read_u64(bytes, 16)?,
            prepaid_service_funding: read_u64(bytes, 24)?,
            inventory: read_u64_array(bytes, 32)?,
        })
    }

    /// Encode one exact canonical snapshot.
    pub fn to_bytes(self) -> [u8; CAPITAL_SNAPSHOT_BYTES] {
        let mut out = [0u8; CAPITAL_SNAPSHOT_BYTES];
        put_u64(&mut out, 0, self.cash);
        put_u64(&mut out, 8, self.sponsor_loss_capital);
        put_u64(&mut out, 16, self.realized_fees);
        put_u64(&mut out, 24, self.prepaid_service_funding);
        put_u64_array(&mut out, 32, &self.inventory);
        out
    }

    /// Return segregated quote-settlement cash.
    pub const fn cash(self) -> u64 {
        self.cash
    }

    /// Return sponsor loss capital.
    pub const fn sponsor_loss_capital(self) -> u64 {
        self.sponsor_loss_capital
    }

    /// Return segregated realized fees.
    pub const fn realized_fees(self) -> u64 {
        self.realized_fees
    }

    /// Return prepaid service funding.
    pub const fn prepaid_service_funding(self) -> u64 {
        self.prepaid_service_funding
    }

    /// Return the complete fixed-profile native inventory.
    pub const fn inventory(self) -> [u64; MAX_NATIVE_CLAIMS] {
        self.inventory
    }

    fn is_zero(self) -> bool {
        self.cash == 0
            && self.sponsor_loss_capital == 0
            && self.realized_fees == 0
            && self.prepaid_service_funding == 0
            && array_all_zero(&self.inventory)
    }

    fn require_inactive_zero(self, claim_count: u8) -> Result<()> {
        require_inactive_zero(&self.inventory, claim_count)
    }
}

/// Lifecycle state retained by one Dealer facility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerStatus {
    /// Quotes and capital-epoch transitions are admitted.
    Active = 0,
    /// Exit completed; the account is empty replay evidence.
    Exited = 1,
}

impl DealerStatus {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::Exited),
            _ => Err(Error::UnknownDiscriminant),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Exited => 1,
        }
    }
}

/// Canonical mutable Dealer capital and aggregate reservation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerState {
    binding: DealerBinding,
    epoch: u64,
    next_sequence: u64,
    status: DealerStatus,
    claim_count: u8,
    outstanding_quotes: u64,
    cash: u64,
    sponsor_loss_capital: u64,
    realized_fees: u64,
    prepaid_service_funding: u64,
    reserved_cash: u64,
    inventory: [u64; MAX_NATIVE_CLAIMS],
    reserved_outgoing: [u64; MAX_NATIVE_CLAIMS],
    reserved_incoming: [u64; MAX_NATIVE_CLAIMS],
}

impl DealerState {
    /// Enter the first immutable capital epoch from exact external deposits.
    pub fn enter(
        epoch: &CapitalEpoch,
        initial: CapitalSnapshot,
    ) -> Result<(Self, CapitalTransitionReceipt)> {
        epoch.validate()?;
        initial.require_inactive_zero(epoch.claim_count)?;
        epoch.validate_snapshot(initial)?;
        let state = Self {
            binding: epoch.binding,
            epoch: epoch.number,
            next_sequence: 1,
            status: DealerStatus::Active,
            claim_count: epoch.claim_count,
            outstanding_quotes: 0,
            cash: initial.cash,
            sponsor_loss_capital: initial.sponsor_loss_capital,
            realized_fees: initial.realized_fees,
            prepaid_service_funding: initial.prepaid_service_funding,
            reserved_cash: 0,
            inventory: initial.inventory,
            reserved_outgoing: [0; MAX_NATIVE_CLAIMS],
            reserved_incoming: [0; MAX_NATIVE_CLAIMS],
        };
        state.validate()?;
        let receipt = CapitalTransitionReceipt::new(
            epoch.binding,
            0,
            CapitalTransitionKind::Enter,
            NO_EPOCH,
            epoch.number,
            CapitalSnapshot::zero(),
            initial,
            CapitalSnapshot::zero(),
            initial,
        )?;
        Ok((state, receipt))
    }

    /// Decode one exact canonical Dealer state.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_header(bytes, DEALER_STATE_BYTES, STATE_MAGIC)?;
        require_zero(bytes, STATE_BODY_RESERVED_OFFSET, STATE_BODY_RESERVED_BYTES)?;
        let state = Self {
            binding: DealerBinding::decode(subslice(
                bytes,
                STATE_BINDING_OFFSET,
                DEALER_BINDING_BYTES,
            )?)?,
            epoch: read_u64(bytes, STATE_EPOCH_OFFSET)?,
            next_sequence: read_u64(bytes, STATE_SEQUENCE_OFFSET)?,
            status: DealerStatus::decode(read_byte(bytes, STATE_STATUS_OFFSET)?)?,
            claim_count: read_byte(bytes, STATE_CLAIM_COUNT_OFFSET)?,
            outstanding_quotes: read_u64(bytes, STATE_OUTSTANDING_OFFSET)?,
            cash: read_u64(bytes, STATE_CASH_OFFSET)?,
            sponsor_loss_capital: read_u64(bytes, STATE_LOSS_OFFSET)?,
            realized_fees: read_u64(bytes, STATE_FEES_OFFSET)?,
            prepaid_service_funding: read_u64(bytes, STATE_SERVICE_OFFSET)?,
            reserved_cash: read_u64(bytes, STATE_RESERVED_CASH_OFFSET)?,
            inventory: read_u64_array(bytes, STATE_INVENTORY_OFFSET)?,
            reserved_outgoing: read_u64_array(bytes, STATE_RESERVED_OUT_OFFSET)?,
            reserved_incoming: read_u64_array(bytes, STATE_RESERVED_IN_OFFSET)?,
        };
        state.validate()?;
        Ok(state)
    }

    /// Encode one exact canonical Dealer state.
    pub fn to_bytes(self) -> [u8; DEALER_STATE_BYTES] {
        let mut out = [0u8; DEALER_STATE_BYTES];
        put_header(&mut out, STATE_MAGIC);
        put(&mut out, STATE_BINDING_OFFSET, &self.binding.to_bytes());
        put_u64(&mut out, STATE_EPOCH_OFFSET, self.epoch);
        put_u64(&mut out, STATE_SEQUENCE_OFFSET, self.next_sequence);
        put(&mut out, STATE_STATUS_OFFSET, &[self.status.byte()]);
        put(&mut out, STATE_CLAIM_COUNT_OFFSET, &[self.claim_count]);
        put_u64(&mut out, STATE_OUTSTANDING_OFFSET, self.outstanding_quotes);
        put_u64(&mut out, STATE_CASH_OFFSET, self.cash);
        put_u64(&mut out, STATE_LOSS_OFFSET, self.sponsor_loss_capital);
        put_u64(&mut out, STATE_FEES_OFFSET, self.realized_fees);
        put_u64(&mut out, STATE_SERVICE_OFFSET, self.prepaid_service_funding);
        put_u64(&mut out, STATE_RESERVED_CASH_OFFSET, self.reserved_cash);
        put_u64_array(&mut out, STATE_INVENTORY_OFFSET, &self.inventory);
        put_u64_array(&mut out, STATE_RESERVED_OUT_OFFSET, &self.reserved_outgoing);
        put_u64_array(&mut out, STATE_RESERVED_IN_OFFSET, &self.reserved_incoming);
        out
    }

    /// Validate structural invariants independent of an epoch's risk policy.
    pub fn validate(&self) -> Result<()> {
        validate_claim_count(self.claim_count)?;
        require_inactive_zero(&self.inventory, self.claim_count)?;
        require_inactive_zero(&self.reserved_outgoing, self.claim_count)?;
        require_inactive_zero(&self.reserved_incoming, self.claim_count)?;
        if self.epoch == NO_EPOCH {
            return Err(Error::InvalidEpoch);
        }
        let mut any_reservation = self.reserved_cash != 0;
        let mut index = 0usize;
        while index < MAX_NATIVE_CLAIMS {
            let inventory = value_at(&self.inventory, index)?;
            let outgoing = value_at(&self.reserved_outgoing, index)?;
            if outgoing > inventory {
                return Err(Error::ReservationInvariant);
            }
            let incoming = value_at(&self.reserved_incoming, index)?;
            let _ = inventory
                .checked_add(incoming)
                .ok_or(Error::ArithmeticOverflow)?;
            any_reservation |= outgoing != 0 || incoming != 0;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if (self.outstanding_quotes == 0) != !any_reservation {
            return Err(Error::ReservationInvariant);
        }
        if self.reserved_cash > self.cash {
            return Err(Error::ReservationInvariant);
        }
        if self.status == DealerStatus::Exited
            && (!self.snapshot().is_zero() || !self.is_quiescent())
        {
            return Err(Error::ExitedDealerNotEmpty);
        }
        Ok(())
    }

    /// Admit and reserve one covered quote under the current immutable epoch.
    pub fn admit_quote(
        &mut self,
        epoch: &CapitalEpoch,
        request: QuoteRequest,
        current_slot: u64,
    ) -> Result<QuoteReservation> {
        self.validate()?;
        epoch.validate()?;
        self.require_active()?;
        epoch.validate_snapshot(self.snapshot())?;
        self.binding
            .require_generation(request.expected_generation)?;
        self.binding.require_same(epoch.binding)?;
        self.require_epoch(epoch.number)?;
        if request.expected_epoch != self.epoch {
            return Err(Error::EpochMismatch);
        }
        if request.expected_sequence != self.next_sequence {
            return Err(Error::SequenceMismatch);
        }
        if request.quantity == 0 || request.quantity > epoch.max_quantity_per_quote {
            return Err(Error::InvalidQuantity);
        }
        let claim_index = usize::from(request.claim_index);
        require_active_index(claim_index, self.claim_count)?;
        if request.expiry_slot <= current_slot {
            return Err(Error::InvalidExpiry);
        }
        let lifetime = request
            .expiry_slot
            .checked_sub(current_slot)
            .ok_or(Error::InvalidExpiry)?;
        if lifetime > epoch.max_quote_lifetime_slots {
            return Err(Error::InvalidExpiry);
        }

        let price = match request.side {
            QuoteSide::CustomerBuys => value_at(&epoch.ask_price, claim_index)?,
            QuoteSide::CustomerSells => value_at(&epoch.bid_price, claim_index)?,
        };
        let notional = match request.side {
            QuoteSide::CustomerBuys => mul_div_ceil(request.quantity, price, epoch.price_scale)?,
            QuoteSide::CustomerSells => mul_div_floor(request.quantity, price, epoch.price_scale)?,
        };
        if notional == 0 {
            return Err(Error::ZeroNotional);
        }
        let fee = mul_div_ceil(notional, u64::from(epoch.fee_bps), BASIS_POINTS_DENOMINATOR)?;
        let customer_cash = match request.side {
            QuoteSide::CustomerBuys => {
                notional.checked_add(fee).ok_or(Error::ArithmeticOverflow)?
            }
            QuoteSide::CustomerSells => {
                notional.checked_sub(fee).ok_or(Error::ArithmeticOverflow)?
            }
        };

        let mut next = *self;
        match request.side {
            QuoteSide::CustomerBuys => {
                let inventory = value_at(&next.inventory, claim_index)?;
                let reserved = value_at(&next.reserved_outgoing, claim_index)?;
                let available = inventory
                    .checked_sub(reserved)
                    .ok_or(Error::ReservationInvariant)?;
                if request.quantity > available {
                    return Err(Error::InsufficientInventory);
                }
                set_value(
                    &mut next.reserved_outgoing,
                    claim_index,
                    reserved
                        .checked_add(request.quantity)
                        .ok_or(Error::ArithmeticOverflow)?,
                )?;
            }
            QuoteSide::CustomerSells => {
                let available_cash = next
                    .cash
                    .checked_sub(next.reserved_cash)
                    .ok_or(Error::ReservationInvariant)?;
                if notional > available_cash {
                    return Err(Error::InsufficientCash);
                }
                next.reserved_cash = next
                    .reserved_cash
                    .checked_add(notional)
                    .ok_or(Error::ArithmeticOverflow)?;
                let prior = value_at(&next.reserved_incoming, claim_index)?;
                set_value(
                    &mut next.reserved_incoming,
                    claim_index,
                    prior
                        .checked_add(request.quantity)
                        .ok_or(Error::ArithmeticOverflow)?,
                )?;
                epoch.validate_projected_incoming(&next)?;
            }
        }
        next.outstanding_quotes = next
            .outstanding_quotes
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.next_sequence = next
            .next_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        let quote = QuoteReservation {
            binding: self.binding,
            epoch: self.epoch,
            sequence: self.next_sequence,
            side: request.side,
            claim_index: request.claim_index,
            status: QuoteStatus::Active,
            quantity: request.quantity,
            notional,
            fee,
            customer_cash,
            expiry_slot: request.expiry_slot,
        };
        quote.validate()?;
        *self = next;
        Ok(quote)
    }

    /// Execute one still-covered quote and produce its exact transfer receipt.
    pub fn execute_quote(
        &mut self,
        epoch: &CapitalEpoch,
        quote: &mut QuoteReservation,
        expected_generation: u64,
        current_slot: u64,
    ) -> Result<ExecutionReceipt> {
        self.validate()?;
        epoch.validate()?;
        self.require_active()?;
        self.binding.require_generation(expected_generation)?;
        self.binding.require_same(epoch.binding)?;
        self.binding.require_same(quote.binding)?;
        self.require_epoch(epoch.number)?;
        self.require_epoch(quote.epoch)?;
        epoch.validate_snapshot(self.snapshot())?;
        quote.validate()?;
        if quote.status != QuoteStatus::Active {
            return Err(Error::QuoteNotActive);
        }
        if current_slot > quote.expiry_slot {
            return Err(Error::QuoteExpired);
        }
        epoch.validate_quote_terms(quote, current_slot)?;
        let index = usize::from(quote.claim_index);
        require_active_index(index, self.claim_count)?;
        let prior = ReceiptValues::read(self, index)?;
        let mut next = *self;
        match quote.side {
            QuoteSide::CustomerBuys => {
                subtract_at(&mut next.reserved_outgoing, index, quote.quantity)?;
                subtract_at(&mut next.inventory, index, quote.quantity)?;
                next.cash = next
                    .cash
                    .checked_add(quote.notional)
                    .ok_or(Error::ArithmeticOverflow)?;
                next.realized_fees = next
                    .realized_fees
                    .checked_add(quote.fee)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            QuoteSide::CustomerSells => {
                next.reserved_cash = next
                    .reserved_cash
                    .checked_sub(quote.notional)
                    .ok_or(Error::ReservationInvariant)?;
                subtract_at(&mut next.reserved_incoming, index, quote.quantity)?;
                add_at(&mut next.inventory, index, quote.quantity)?;
                next.cash = next
                    .cash
                    .checked_sub(quote.notional)
                    .ok_or(Error::InsufficientCash)?;
                next.realized_fees = next
                    .realized_fees
                    .checked_add(quote.fee)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        next.outstanding_quotes = next
            .outstanding_quotes
            .checked_sub(1)
            .ok_or(Error::ReservationInvariant)?;
        epoch.validate_snapshot(next.snapshot())?;
        next.validate()?;
        let after = ReceiptValues::read(&next, index)?;
        let receipt = ExecutionReceipt {
            binding: self.binding,
            epoch: self.epoch,
            quote_sequence: quote.sequence,
            side: quote.side,
            claim_index: quote.claim_index,
            quantity: quote.quantity,
            notional: quote.notional,
            fee: quote.fee,
            customer_cash: quote.customer_cash,
            expiry_slot: quote.expiry_slot,
            prior,
            next: after,
        };
        receipt.validate()?;
        let mut next_quote = *quote;
        next_quote.status = QuoteStatus::Executed;
        *self = next;
        *quote = next_quote;
        Ok(receipt)
    }

    /// Cancel one quote and release its exact reservation without moving value.
    ///
    /// Signer and permission policy belong to the composing adapter. This
    /// semantic transition only proves that no reserved value remains attached
    /// to the quote child.
    pub fn cancel_quote(&mut self, quote: &mut QuoteReservation) -> Result<()> {
        self.validate()?;
        self.require_active()?;
        self.binding.require_same(quote.binding)?;
        self.require_epoch(quote.epoch)?;
        if quote.status != QuoteStatus::Active {
            return Err(Error::QuoteNotActive);
        }
        let index = usize::from(quote.claim_index);
        require_active_index(index, self.claim_count)?;
        let mut next = *self;
        match quote.side {
            QuoteSide::CustomerBuys => {
                subtract_at(&mut next.reserved_outgoing, index, quote.quantity)?;
            }
            QuoteSide::CustomerSells => {
                next.reserved_cash = next
                    .reserved_cash
                    .checked_sub(quote.notional)
                    .ok_or(Error::ReservationInvariant)?;
                subtract_at(&mut next.reserved_incoming, index, quote.quantity)?;
            }
        }
        next.outstanding_quotes = next
            .outstanding_quotes
            .checked_sub(1)
            .ok_or(Error::ReservationInvariant)?;
        next.validate()?;
        let mut next_quote = *quote;
        next_quote.status = QuoteStatus::Cancelled;
        *self = next;
        *quote = next_quote;
        Ok(())
    }

    /// Reconfigure to the exact successor epoch with explicit external flows.
    pub fn reconfigure(
        &mut self,
        current_epoch: &CapitalEpoch,
        next_epoch: &CapitalEpoch,
        expected_generation: u64,
        expected_sequence: u64,
        deposits: CapitalSnapshot,
        withdrawals: CapitalSnapshot,
    ) -> Result<CapitalTransitionReceipt> {
        self.validate()?;
        current_epoch.validate()?;
        next_epoch.validate()?;
        self.require_active()?;
        self.require_quiescent()?;
        self.binding.require_generation(expected_generation)?;
        self.binding.require_same(current_epoch.binding)?;
        self.binding.require_same(next_epoch.binding)?;
        self.require_epoch(current_epoch.number)?;
        current_epoch.validate_snapshot(self.snapshot())?;
        if expected_sequence != self.next_sequence {
            return Err(Error::SequenceMismatch);
        }
        let expected_next = self.epoch.checked_add(1).ok_or(Error::InvalidEpoch)?;
        if next_epoch.number != expected_next || next_epoch.claim_count != self.claim_count {
            return Err(Error::InvalidEpoch);
        }
        deposits.require_inactive_zero(self.claim_count)?;
        withdrawals.require_inactive_zero(self.claim_count)?;
        let old = self.snapshot();
        let new = apply_capital_flow(old, deposits, withdrawals)?;
        next_epoch.validate_snapshot(new)?;
        let receipt = CapitalTransitionReceipt::new(
            self.binding,
            self.next_sequence,
            CapitalTransitionKind::Reconfigure,
            self.epoch,
            next_epoch.number,
            old,
            deposits,
            withdrawals,
            new,
        )?;
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut next_state = *self;
        next_state.epoch = next_epoch.number;
        next_state.next_sequence = next_sequence;
        next_state.cash = new.cash;
        next_state.sponsor_loss_capital = new.sponsor_loss_capital;
        next_state.realized_fees = new.realized_fees;
        next_state.prepaid_service_funding = new.prepaid_service_funding;
        next_state.inventory = new.inventory;
        next_state.validate()?;
        *self = next_state;
        Ok(receipt)
    }

    /// Exit a quiescent Dealer and return every compartment exactly.
    pub fn exit(
        &mut self,
        epoch: &CapitalEpoch,
        expected_generation: u64,
        expected_sequence: u64,
    ) -> Result<CapitalTransitionReceipt> {
        self.validate()?;
        epoch.validate()?;
        self.require_active()?;
        self.require_quiescent()?;
        self.binding.require_generation(expected_generation)?;
        self.binding.require_same(epoch.binding)?;
        self.require_epoch(epoch.number)?;
        epoch.validate_snapshot(self.snapshot())?;
        if expected_sequence != self.next_sequence {
            return Err(Error::SequenceMismatch);
        }
        let old = self.snapshot();
        let receipt = CapitalTransitionReceipt::new(
            self.binding,
            self.next_sequence,
            CapitalTransitionKind::Exit,
            self.epoch,
            NO_EPOCH,
            old,
            CapitalSnapshot::zero(),
            old,
            CapitalSnapshot::zero(),
        )?;
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut next_state = *self;
        next_state.cash = 0;
        next_state.sponsor_loss_capital = 0;
        next_state.realized_fees = 0;
        next_state.prepaid_service_funding = 0;
        next_state.inventory = [0; MAX_NATIVE_CLAIMS];
        next_state.status = DealerStatus::Exited;
        next_state.next_sequence = next_sequence;
        next_state.validate()?;
        *self = next_state;
        Ok(receipt)
    }

    /// Return the immutable Dealer binding.
    pub const fn binding(self) -> DealerBinding {
        self.binding
    }

    /// Return the current immutable capital epoch number.
    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Return the next globally unique operation sequence.
    pub const fn next_sequence(self) -> u64 {
        self.next_sequence
    }

    /// Return the lifecycle status.
    pub const fn status(self) -> DealerStatus {
        self.status
    }

    /// Return the exact number of live quote reservations.
    pub const fn outstanding_quotes(self) -> u64 {
        self.outstanding_quotes
    }

    /// Return the exact current capital snapshot, excluding reservations.
    pub const fn snapshot(self) -> CapitalSnapshot {
        CapitalSnapshot::new(
            self.cash,
            self.sponsor_loss_capital,
            self.realized_fees,
            self.prepaid_service_funding,
            self.inventory,
        )
    }

    /// Return whether no quote reservation is live.
    pub fn is_quiescent(self) -> bool {
        self.outstanding_quotes == 0
            && self.reserved_cash == 0
            && array_all_zero(&self.reserved_outgoing)
            && array_all_zero(&self.reserved_incoming)
    }

    fn require_active(&self) -> Result<()> {
        if self.status != DealerStatus::Active {
            return Err(Error::DealerNotActive);
        }
        Ok(())
    }

    fn require_epoch(&self, epoch: u64) -> Result<()> {
        if self.epoch != epoch {
            return Err(Error::EpochMismatch);
        }
        Ok(())
    }

    fn require_quiescent(&self) -> Result<()> {
        if !self.is_quiescent() {
            return Err(Error::DealerNotQuiescent);
        }
        Ok(())
    }
}

/// Immutable quote pricing and inventory-risk policy for one capital epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapitalEpoch {
    binding: DealerBinding,
    number: u64,
    claim_count: u8,
    fee_bps: u16,
    price_scale: u64,
    max_quote_lifetime_slots: u64,
    max_quantity_per_quote: u64,
    inventory_caps: [u64; MAX_NATIVE_CLAIMS],
    risk_weight: [u64; MAX_NATIVE_CLAIMS],
    bid_price: [u64; MAX_NATIVE_CLAIMS],
    ask_price: [u64; MAX_NATIVE_CLAIMS],
}

impl CapitalEpoch {
    /// Construct and validate immutable pricing and risk parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        binding: DealerBinding,
        number: u64,
        claim_count: u8,
        fee_bps: u16,
        price_scale: u64,
        max_quote_lifetime_slots: u64,
        max_quantity_per_quote: u64,
        inventory_caps: [u64; MAX_NATIVE_CLAIMS],
        risk_weight: [u64; MAX_NATIVE_CLAIMS],
        bid_price: [u64; MAX_NATIVE_CLAIMS],
        ask_price: [u64; MAX_NATIVE_CLAIMS],
    ) -> Result<Self> {
        let epoch = Self {
            binding,
            number,
            claim_count,
            fee_bps,
            price_scale,
            max_quote_lifetime_slots,
            max_quantity_per_quote,
            inventory_caps,
            risk_weight,
            bid_price,
            ask_price,
        };
        epoch.validate()?;
        Ok(epoch)
    }

    /// Decode one exact canonical epoch.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_header(bytes, CAPITAL_EPOCH_BYTES, EPOCH_MAGIC)?;
        require_zero(bytes, EPOCH_RESERVED_OFFSET, EPOCH_RESERVED_BYTES)?;
        Self::new(
            DealerBinding::decode(subslice(bytes, EPOCH_BINDING_OFFSET, DEALER_BINDING_BYTES)?)?,
            read_u64(bytes, EPOCH_NUMBER_OFFSET)?,
            read_byte(bytes, EPOCH_CLAIM_COUNT_OFFSET)?,
            read_u16(bytes, EPOCH_FEE_BPS_OFFSET)?,
            read_u64(bytes, EPOCH_PRICE_SCALE_OFFSET)?,
            read_u64(bytes, EPOCH_MAX_LIFETIME_OFFSET)?,
            read_u64(bytes, EPOCH_MAX_QUANTITY_OFFSET)?,
            read_u64_array(bytes, EPOCH_CAPS_OFFSET)?,
            read_u64_array(bytes, EPOCH_RISK_OFFSET)?,
            read_u64_array(bytes, EPOCH_BID_OFFSET)?,
            read_u64_array(bytes, EPOCH_ASK_OFFSET)?,
        )
    }

    /// Encode one exact canonical epoch.
    pub fn to_bytes(self) -> [u8; CAPITAL_EPOCH_BYTES] {
        let mut out = [0u8; CAPITAL_EPOCH_BYTES];
        put_header(&mut out, EPOCH_MAGIC);
        put(&mut out, EPOCH_BINDING_OFFSET, &self.binding.to_bytes());
        put_u64(&mut out, EPOCH_NUMBER_OFFSET, self.number);
        put(&mut out, EPOCH_CLAIM_COUNT_OFFSET, &[self.claim_count]);
        put(&mut out, EPOCH_FEE_BPS_OFFSET, &self.fee_bps.to_le_bytes());
        put_u64(&mut out, EPOCH_PRICE_SCALE_OFFSET, self.price_scale);
        put_u64(
            &mut out,
            EPOCH_MAX_LIFETIME_OFFSET,
            self.max_quote_lifetime_slots,
        );
        put_u64(
            &mut out,
            EPOCH_MAX_QUANTITY_OFFSET,
            self.max_quantity_per_quote,
        );
        put_u64_array(&mut out, EPOCH_CAPS_OFFSET, &self.inventory_caps);
        put_u64_array(&mut out, EPOCH_RISK_OFFSET, &self.risk_weight);
        put_u64_array(&mut out, EPOCH_BID_OFFSET, &self.bid_price);
        put_u64_array(&mut out, EPOCH_ASK_OFFSET, &self.ask_price);
        out
    }

    /// Validate canonical policy constraints.
    pub fn validate(&self) -> Result<()> {
        validate_claim_count(self.claim_count)?;
        if self.number == NO_EPOCH {
            return Err(Error::InvalidEpoch);
        }
        if self.price_scale == 0 {
            return Err(Error::ZeroPriceScale);
        }
        if u64::from(self.fee_bps) > BASIS_POINTS_DENOMINATOR {
            return Err(Error::InvalidFeeRate);
        }
        if self.max_quote_lifetime_slots == 0 || self.max_quantity_per_quote == 0 {
            return Err(Error::EmptyQuotePolicy);
        }
        require_inactive_zero(&self.inventory_caps, self.claim_count)?;
        require_inactive_zero(&self.risk_weight, self.claim_count)?;
        require_inactive_zero(&self.bid_price, self.claim_count)?;
        require_inactive_zero(&self.ask_price, self.claim_count)?;
        let mut index = 0usize;
        while index < usize::from(self.claim_count) {
            if value_at(&self.bid_price, index)? > value_at(&self.ask_price, index)? {
                return Err(Error::CrossedPrices);
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    /// Return the immutable Dealer binding.
    pub const fn binding(self) -> DealerBinding {
        self.binding
    }

    /// Return this immutable epoch number.
    pub const fn number(self) -> u64 {
        self.number
    }

    /// Return the mathematical price denominator.
    pub const fn price_scale(self) -> u64 {
        self.price_scale
    }

    /// Return the maximum admitted quote lifetime in slots.
    pub const fn max_quote_lifetime_slots(self) -> u64 {
        self.max_quote_lifetime_slots
    }

    /// Compute required sponsor loss capital with one aggregate ceiling.
    ///
    /// The named rounding boundary is
    /// `ceil(sum(inventory[i] * risk_weight[i]) / price_scale)`.
    pub fn required_loss_capital(&self, inventory: &[u64; MAX_NATIVE_CLAIMS]) -> Result<u64> {
        require_inactive_zero(inventory, self.claim_count)?;
        let mut numerator = 0u128;
        let mut index = 0usize;
        while index < usize::from(self.claim_count) {
            let term = u128::from(value_at(inventory, index)?)
                .checked_mul(u128::from(value_at(&self.risk_weight, index)?))
                .ok_or(Error::ArithmeticOverflow)?;
            numerator = numerator
                .checked_add(term)
                .ok_or(Error::ArithmeticOverflow)?;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        ceil_u128(numerator, u128::from(self.price_scale))
    }

    fn validate_snapshot(&self, snapshot: CapitalSnapshot) -> Result<()> {
        snapshot.require_inactive_zero(self.claim_count)?;
        let mut index = 0usize;
        while index < usize::from(self.claim_count) {
            if value_at(&snapshot.inventory, index)? > value_at(&self.inventory_caps, index)? {
                return Err(Error::InventoryCapExceeded);
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if self.required_loss_capital(&snapshot.inventory)? > snapshot.sponsor_loss_capital {
            return Err(Error::InsufficientLossCapital);
        }
        Ok(())
    }

    fn validate_projected_incoming(&self, state: &DealerState) -> Result<()> {
        let mut projected = state.inventory;
        let mut index = 0usize;
        while index < usize::from(self.claim_count) {
            let value = value_at(&state.inventory, index)?
                .checked_add(value_at(&state.reserved_incoming, index)?)
                .ok_or(Error::ArithmeticOverflow)?;
            if value > value_at(&self.inventory_caps, index)? {
                return Err(Error::InventoryCapExceeded);
            }
            set_value(&mut projected, index, value)?;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if self.required_loss_capital(&projected)? > state.sponsor_loss_capital {
            return Err(Error::InsufficientLossCapital);
        }
        Ok(())
    }

    fn validate_quote_terms(&self, quote: &QuoteReservation, current_slot: u64) -> Result<()> {
        if quote.quantity > self.max_quantity_per_quote {
            return Err(Error::InvalidQuantity);
        }
        let index = usize::from(quote.claim_index);
        require_active_index(index, self.claim_count)?;
        let remaining_lifetime = quote
            .expiry_slot
            .checked_sub(current_slot)
            .ok_or(Error::QuoteExpired)?;
        if remaining_lifetime > self.max_quote_lifetime_slots {
            return Err(Error::InvalidExpiry);
        }
        let price = match quote.side {
            QuoteSide::CustomerBuys => value_at(&self.ask_price, index)?,
            QuoteSide::CustomerSells => value_at(&self.bid_price, index)?,
        };
        let notional = match quote.side {
            QuoteSide::CustomerBuys => mul_div_ceil(quote.quantity, price, self.price_scale)?,
            QuoteSide::CustomerSells => mul_div_floor(quote.quantity, price, self.price_scale)?,
        };
        let fee = mul_div_ceil(notional, u64::from(self.fee_bps), BASIS_POINTS_DENOMINATOR)?;
        let customer_cash = match quote.side {
            QuoteSide::CustomerBuys => {
                notional.checked_add(fee).ok_or(Error::ArithmeticOverflow)?
            }
            QuoteSide::CustomerSells => {
                notional.checked_sub(fee).ok_or(Error::ArithmeticOverflow)?
            }
        };
        if quote.notional != notional || quote.fee != fee || quote.customer_cash != customer_cash {
            return Err(Error::QuoteTermsMismatch);
        }
        Ok(())
    }
}

/// Direction of the customer-facing claim transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum QuoteSide {
    /// Customer pays cash plus fee and receives native claims.
    CustomerBuys = 0,
    /// Customer delivers native claims and receives cash minus fee.
    CustomerSells = 1,
}

impl QuoteSide {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::CustomerBuys),
            1 => Ok(Self::CustomerSells),
            _ => Err(Error::UnknownDiscriminant),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::CustomerBuys => 0,
            Self::CustomerSells => 1,
        }
    }
}

/// Caller request for a policy-priced, fully reserved quote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuoteRequest {
    /// Immutable Market generation expected by the caller.
    pub expected_generation: u64,
    /// Current capital epoch expected by the caller.
    pub expected_epoch: u64,
    /// Next global operation sequence expected by the caller.
    pub expected_sequence: u64,
    /// Customer-facing transfer direction.
    pub side: QuoteSide,
    /// Native claim index.
    pub claim_index: u8,
    /// Exact native claim quantity.
    pub quantity: u64,
    /// Last slot at which execution remains admitted.
    pub expiry_slot: u64,
}

/// Persistent lifecycle state of one quote child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum QuoteStatus {
    /// Aggregate coverage is reserved in the Dealer state.
    Active = 0,
    /// Value transfer executed exactly once.
    Executed = 1,
    /// Reservation was released without value transfer.
    Cancelled = 2,
}

impl QuoteStatus {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Active),
            1 => Ok(Self::Executed),
            2 => Ok(Self::Cancelled),
            _ => Err(Error::UnknownDiscriminant),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Executed => 1,
            Self::Cancelled => 2,
        }
    }
}

/// Exact per-quote reservation and replay authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuoteReservation {
    binding: DealerBinding,
    epoch: u64,
    sequence: u64,
    side: QuoteSide,
    claim_index: u8,
    status: QuoteStatus,
    quantity: u64,
    notional: u64,
    fee: u64,
    customer_cash: u64,
    expiry_slot: u64,
}

impl QuoteReservation {
    /// Decode one exact canonical quote child.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_header(bytes, QUOTE_RESERVATION_BYTES, QUOTE_MAGIC)?;
        require_zero(bytes, QUOTE_RESERVED_OFFSET, QUOTE_RESERVED_BYTES)?;
        let quote = Self {
            binding: DealerBinding::decode(subslice(
                bytes,
                QUOTE_BINDING_OFFSET,
                DEALER_BINDING_BYTES,
            )?)?,
            epoch: read_u64(bytes, QUOTE_EPOCH_OFFSET)?,
            sequence: read_u64(bytes, QUOTE_SEQUENCE_OFFSET)?,
            side: QuoteSide::decode(read_byte(bytes, QUOTE_SIDE_OFFSET)?)?,
            claim_index: read_byte(bytes, QUOTE_CLAIM_OFFSET)?,
            status: QuoteStatus::decode(read_byte(bytes, QUOTE_STATUS_OFFSET)?)?,
            quantity: read_u64(bytes, QUOTE_QUANTITY_OFFSET)?,
            notional: read_u64(bytes, QUOTE_NOTIONAL_OFFSET)?,
            fee: read_u64(bytes, QUOTE_FEE_OFFSET)?,
            customer_cash: read_u64(bytes, QUOTE_CUSTOMER_CASH_OFFSET)?,
            expiry_slot: read_u64(bytes, QUOTE_EXPIRY_OFFSET)?,
        };
        quote.validate()?;
        Ok(quote)
    }

    /// Encode one exact canonical quote child.
    pub fn to_bytes(self) -> [u8; QUOTE_RESERVATION_BYTES] {
        let mut out = [0u8; QUOTE_RESERVATION_BYTES];
        put_header(&mut out, QUOTE_MAGIC);
        put(&mut out, QUOTE_BINDING_OFFSET, &self.binding.to_bytes());
        put_u64(&mut out, QUOTE_EPOCH_OFFSET, self.epoch);
        put_u64(&mut out, QUOTE_SEQUENCE_OFFSET, self.sequence);
        put(&mut out, QUOTE_SIDE_OFFSET, &[self.side.byte()]);
        put(&mut out, QUOTE_CLAIM_OFFSET, &[self.claim_index]);
        put(&mut out, QUOTE_STATUS_OFFSET, &[self.status.byte()]);
        put_u64(&mut out, QUOTE_QUANTITY_OFFSET, self.quantity);
        put_u64(&mut out, QUOTE_NOTIONAL_OFFSET, self.notional);
        put_u64(&mut out, QUOTE_FEE_OFFSET, self.fee);
        put_u64(&mut out, QUOTE_CUSTOMER_CASH_OFFSET, self.customer_cash);
        put_u64(&mut out, QUOTE_EXPIRY_OFFSET, self.expiry_slot);
        out
    }

    /// Validate self-contained quote arithmetic.
    pub fn validate(&self) -> Result<()> {
        if self.epoch == NO_EPOCH {
            return Err(Error::InvalidEpoch);
        }
        if self.quantity == 0 {
            return Err(Error::InvalidQuantity);
        }
        if usize::from(self.claim_index) >= MAX_NATIVE_CLAIMS {
            return Err(Error::ClaimIndexOutOfRange);
        }
        if self.notional == 0 {
            return Err(Error::ZeroNotional);
        }
        let expected = match self.side {
            QuoteSide::CustomerBuys => self
                .notional
                .checked_add(self.fee)
                .ok_or(Error::ArithmeticOverflow)?,
            QuoteSide::CustomerSells => self
                .notional
                .checked_sub(self.fee)
                .ok_or(Error::ArithmeticOverflow)?,
        };
        if expected != self.customer_cash {
            return Err(Error::CapitalConservationMismatch);
        }
        Ok(())
    }

    /// Return the immutable quote sequence.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    /// Return the quote lifecycle status.
    pub const fn status(self) -> QuoteStatus {
        self.status
    }

    /// Return customer cash debit or credit, according to [`QuoteSide`].
    pub const fn customer_cash(self) -> u64 {
        self.customer_cash
    }

    /// Return exact native claim quantity.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReceiptValues {
    cash: u64,
    fees: u64,
    inventory: u64,
    reserved_cash: u64,
    reserved_outgoing: u64,
    reserved_incoming: u64,
    outstanding_quotes: u64,
}

impl ReceiptValues {
    fn read(state: &DealerState, index: usize) -> Result<Self> {
        Ok(Self {
            cash: state.cash,
            fees: state.realized_fees,
            inventory: value_at(&state.inventory, index)?,
            reserved_cash: state.reserved_cash,
            reserved_outgoing: value_at(&state.reserved_outgoing, index)?,
            reserved_incoming: value_at(&state.reserved_incoming, index)?,
            outstanding_quotes: state.outstanding_quotes,
        })
    }
}

/// Exact value-transfer receipt for one covered quote execution.
///
/// Sponsor loss capital and prepaid service funding are absent because they
/// are invariant across execution. The receipt records prior and next values
/// for every mutable field touched by settlement, so an adapter can compose
/// token transfers only when its authenticated account state matches exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionReceipt {
    binding: DealerBinding,
    epoch: u64,
    quote_sequence: u64,
    side: QuoteSide,
    claim_index: u8,
    quantity: u64,
    notional: u64,
    fee: u64,
    customer_cash: u64,
    expiry_slot: u64,
    prior: ReceiptValues,
    next: ReceiptValues,
}

impl ExecutionReceipt {
    /// Decode and validate one exact execution receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_header(bytes, EXECUTION_RECEIPT_BYTES, EXECUTION_MAGIC)?;
        require_zero(bytes, EXECUTION_RESERVED_OFFSET, EXECUTION_RESERVED_BYTES)?;
        let receipt = Self {
            binding: DealerBinding::decode(subslice(
                bytes,
                EXECUTION_BINDING_OFFSET,
                DEALER_BINDING_BYTES,
            )?)?,
            epoch: read_u64(bytes, EXECUTION_EPOCH_OFFSET)?,
            quote_sequence: read_u64(bytes, EXECUTION_SEQUENCE_OFFSET)?,
            side: QuoteSide::decode(read_byte(bytes, EXECUTION_SIDE_OFFSET)?)?,
            claim_index: read_byte(bytes, EXECUTION_CLAIM_OFFSET)?,
            quantity: read_u64(bytes, EXECUTION_QUANTITY_OFFSET)?,
            notional: read_u64(bytes, EXECUTION_NOTIONAL_OFFSET)?,
            fee: read_u64(bytes, EXECUTION_FEE_OFFSET)?,
            customer_cash: read_u64(bytes, EXECUTION_CUSTOMER_CASH_OFFSET)?,
            expiry_slot: read_u64(bytes, EXECUTION_EXPIRY_OFFSET)?,
            prior: read_receipt_values(bytes, EXECUTION_VALUES_OFFSET)?,
            next: read_receipt_values(bytes, EXECUTION_VALUES_OFFSET + 56)?,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Encode one exact execution receipt.
    pub fn to_bytes(self) -> [u8; EXECUTION_RECEIPT_BYTES] {
        let mut out = [0u8; EXECUTION_RECEIPT_BYTES];
        put_header(&mut out, EXECUTION_MAGIC);
        put(&mut out, EXECUTION_BINDING_OFFSET, &self.binding.to_bytes());
        put_u64(&mut out, EXECUTION_EPOCH_OFFSET, self.epoch);
        put_u64(&mut out, EXECUTION_SEQUENCE_OFFSET, self.quote_sequence);
        put(&mut out, EXECUTION_SIDE_OFFSET, &[self.side.byte()]);
        put(&mut out, EXECUTION_CLAIM_OFFSET, &[self.claim_index]);
        put_u64(&mut out, EXECUTION_QUANTITY_OFFSET, self.quantity);
        put_u64(&mut out, EXECUTION_NOTIONAL_OFFSET, self.notional);
        put_u64(&mut out, EXECUTION_FEE_OFFSET, self.fee);
        put_u64(&mut out, EXECUTION_CUSTOMER_CASH_OFFSET, self.customer_cash);
        put_u64(&mut out, EXECUTION_EXPIRY_OFFSET, self.expiry_slot);
        put_receipt_values(&mut out, EXECUTION_VALUES_OFFSET, self.prior);
        put_receipt_values(&mut out, EXECUTION_VALUES_OFFSET + 56, self.next);
        out
    }

    /// Validate the exact internal and external cash conservation equation.
    pub fn validate(&self) -> Result<()> {
        if self.epoch == NO_EPOCH || self.quantity == 0 || self.notional == 0 {
            return Err(Error::InvalidCapitalTransition);
        }
        if self.next.outstanding_quotes
            != self
                .prior
                .outstanding_quotes
                .checked_sub(1)
                .ok_or(Error::ReservationInvariant)?
        {
            return Err(Error::ReservationInvariant);
        }
        let expected_customer = match self.side {
            QuoteSide::CustomerBuys => self
                .notional
                .checked_add(self.fee)
                .ok_or(Error::ArithmeticOverflow)?,
            QuoteSide::CustomerSells => self
                .notional
                .checked_sub(self.fee)
                .ok_or(Error::ArithmeticOverflow)?,
        };
        if expected_customer != self.customer_cash {
            return Err(Error::CapitalConservationMismatch);
        }
        match self.side {
            QuoteSide::CustomerBuys => {
                require_pair_add(self.prior.cash, self.notional, self.next.cash)?;
                require_pair_add(self.prior.fees, self.fee, self.next.fees)?;
                require_pair_sub(self.prior.inventory, self.quantity, self.next.inventory)?;
                require_pair_sub(
                    self.prior.reserved_outgoing,
                    self.quantity,
                    self.next.reserved_outgoing,
                )?;
                require_equal(self.prior.reserved_cash, self.next.reserved_cash)?;
                require_equal(self.prior.reserved_incoming, self.next.reserved_incoming)?;
            }
            QuoteSide::CustomerSells => {
                require_pair_sub(self.prior.cash, self.notional, self.next.cash)?;
                require_pair_add(self.prior.fees, self.fee, self.next.fees)?;
                require_pair_add(self.prior.inventory, self.quantity, self.next.inventory)?;
                require_pair_sub(
                    self.prior.reserved_cash,
                    self.notional,
                    self.next.reserved_cash,
                )?;
                require_equal(self.prior.reserved_outgoing, self.next.reserved_outgoing)?;
                require_pair_sub(
                    self.prior.reserved_incoming,
                    self.quantity,
                    self.next.reserved_incoming,
                )?;
            }
        }
        Ok(())
    }

    /// Return the immutable quote sequence consumed by this receipt.
    pub const fn quote_sequence(self) -> u64 {
        self.quote_sequence
    }

    /// Return customer cash debit or credit, according to side.
    pub const fn customer_cash(self) -> u64 {
        self.customer_cash
    }
}

/// Type of quiescent capital transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CapitalTransitionKind {
    /// Create the Dealer from external deposits.
    Enter = 0,
    /// Replace immutable epoch parameters and move exact capital.
    Reconfigure = 1,
    /// Return all capital and retain empty replay state.
    Exit = 2,
}

impl CapitalTransitionKind {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Enter),
            1 => Ok(Self::Reconfigure),
            2 => Ok(Self::Exit),
            _ => Err(Error::UnknownDiscriminant),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Enter => 0,
            Self::Reconfigure => 1,
            Self::Exit => 2,
        }
    }
}

/// Exact old/external/new value-transfer receipt for capital entry or change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapitalTransitionReceipt {
    binding: DealerBinding,
    sequence: u64,
    kind: CapitalTransitionKind,
    old_epoch: u64,
    new_epoch: u64,
    old: CapitalSnapshot,
    deposits: CapitalSnapshot,
    withdrawals: CapitalSnapshot,
    new: CapitalSnapshot,
}

impl CapitalTransitionReceipt {
    #[allow(clippy::too_many_arguments)]
    fn new(
        binding: DealerBinding,
        sequence: u64,
        kind: CapitalTransitionKind,
        old_epoch: u64,
        new_epoch: u64,
        old: CapitalSnapshot,
        deposits: CapitalSnapshot,
        withdrawals: CapitalSnapshot,
        new: CapitalSnapshot,
    ) -> Result<Self> {
        let receipt = Self {
            binding,
            sequence,
            kind,
            old_epoch,
            new_epoch,
            old,
            deposits,
            withdrawals,
            new,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Decode and validate one exact capital transition receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        check_header(bytes, CAPITAL_TRANSITION_RECEIPT_BYTES, CAPITAL_MAGIC)?;
        require_zero(bytes, CAPITAL_RESERVED_OFFSET, CAPITAL_RESERVED_BYTES)?;
        Self::new(
            DealerBinding::decode(subslice(
                bytes,
                CAPITAL_BINDING_OFFSET,
                DEALER_BINDING_BYTES,
            )?)?,
            read_u64(bytes, CAPITAL_SEQUENCE_OFFSET)?,
            CapitalTransitionKind::decode(read_byte(bytes, CAPITAL_KIND_OFFSET)?)?,
            read_u64(bytes, CAPITAL_OLD_EPOCH_OFFSET)?,
            read_u64(bytes, CAPITAL_NEW_EPOCH_OFFSET)?,
            CapitalSnapshot::decode(subslice(bytes, CAPITAL_OLD_OFFSET, CAPITAL_SNAPSHOT_BYTES)?)?,
            CapitalSnapshot::decode(subslice(
                bytes,
                CAPITAL_DEPOSITS_OFFSET,
                CAPITAL_SNAPSHOT_BYTES,
            )?)?,
            CapitalSnapshot::decode(subslice(
                bytes,
                CAPITAL_WITHDRAWALS_OFFSET,
                CAPITAL_SNAPSHOT_BYTES,
            )?)?,
            CapitalSnapshot::decode(subslice(bytes, CAPITAL_NEW_OFFSET, CAPITAL_SNAPSHOT_BYTES)?)?,
        )
    }

    /// Encode one exact canonical capital transition receipt.
    pub fn to_bytes(self) -> [u8; CAPITAL_TRANSITION_RECEIPT_BYTES] {
        let mut out = [0u8; CAPITAL_TRANSITION_RECEIPT_BYTES];
        put_header(&mut out, CAPITAL_MAGIC);
        put(&mut out, CAPITAL_BINDING_OFFSET, &self.binding.to_bytes());
        put_u64(&mut out, CAPITAL_SEQUENCE_OFFSET, self.sequence);
        put(&mut out, CAPITAL_KIND_OFFSET, &[self.kind.byte()]);
        put_u64(&mut out, CAPITAL_OLD_EPOCH_OFFSET, self.old_epoch);
        put_u64(&mut out, CAPITAL_NEW_EPOCH_OFFSET, self.new_epoch);
        put(&mut out, CAPITAL_OLD_OFFSET, &self.old.to_bytes());
        put(&mut out, CAPITAL_DEPOSITS_OFFSET, &self.deposits.to_bytes());
        put(
            &mut out,
            CAPITAL_WITHDRAWALS_OFFSET,
            &self.withdrawals.to_bytes(),
        );
        put(&mut out, CAPITAL_NEW_OFFSET, &self.new.to_bytes());
        out
    }

    /// Validate epoch shape and exact component-wise conservation.
    pub fn validate(&self) -> Result<()> {
        match self.kind {
            CapitalTransitionKind::Enter => {
                if self.old_epoch != NO_EPOCH
                    || self.new_epoch == NO_EPOCH
                    || !self.old.is_zero()
                    || !self.withdrawals.is_zero()
                {
                    return Err(Error::InvalidCapitalTransition);
                }
            }
            CapitalTransitionKind::Reconfigure => {
                if self.old_epoch == NO_EPOCH
                    || self.new_epoch
                        != self
                            .old_epoch
                            .checked_add(1)
                            .ok_or(Error::InvalidCapitalTransition)?
                {
                    return Err(Error::InvalidCapitalTransition);
                }
            }
            CapitalTransitionKind::Exit => {
                if self.old_epoch == NO_EPOCH
                    || self.new_epoch != NO_EPOCH
                    || !self.deposits.is_zero()
                    || !self.new.is_zero()
                {
                    return Err(Error::InvalidCapitalTransition);
                }
            }
        }
        let computed = apply_capital_flow(self.old, self.deposits, self.withdrawals)?;
        if computed != self.new {
            return Err(Error::CapitalConservationMismatch);
        }
        Ok(())
    }

    /// Return the globally unique operation sequence.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    /// Return exact external deposits.
    pub const fn deposits(self) -> CapitalSnapshot {
        self.deposits
    }

    /// Return exact external withdrawals.
    pub const fn withdrawals(self) -> CapitalSnapshot {
        self.withdrawals
    }

    /// Return the exact post-transition capital snapshot.
    pub const fn new_snapshot(self) -> CapitalSnapshot {
        self.new
    }
}

fn apply_capital_flow(
    old: CapitalSnapshot,
    deposits: CapitalSnapshot,
    withdrawals: CapitalSnapshot,
) -> Result<CapitalSnapshot> {
    let cash = apply_component(old.cash, deposits.cash, withdrawals.cash)?;
    let loss = apply_component(
        old.sponsor_loss_capital,
        deposits.sponsor_loss_capital,
        withdrawals.sponsor_loss_capital,
    )?;
    let fees = apply_component(
        old.realized_fees,
        deposits.realized_fees,
        withdrawals.realized_fees,
    )?;
    let service = apply_component(
        old.prepaid_service_funding,
        deposits.prepaid_service_funding,
        withdrawals.prepaid_service_funding,
    )?;
    let mut inventory = [0u64; MAX_NATIVE_CLAIMS];
    let mut index = 0usize;
    while index < MAX_NATIVE_CLAIMS {
        let value = apply_component(
            value_at(&old.inventory, index)?,
            value_at(&deposits.inventory, index)?,
            value_at(&withdrawals.inventory, index)?,
        )?;
        set_value(&mut inventory, index, value)?;
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(CapitalSnapshot::new(cash, loss, fees, service, inventory))
}

fn apply_component(old: u64, deposit: u64, withdrawal: u64) -> Result<u64> {
    if deposit != 0 && withdrawal != 0 {
        return Err(Error::OverlappingCapitalFlow);
    }
    old.checked_add(deposit)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_sub(withdrawal)
        .ok_or(Error::InsufficientCapital)
}

fn mul_div_floor(left: u64, right: u64, denominator: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::ZeroPriceScale);
    }
    let value = u128::from(left)
        .checked_mul(u128::from(right))
        .ok_or(Error::ArithmeticOverflow)?
        / u128::from(denominator);
    u64::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

fn mul_div_ceil(left: u64, right: u64, denominator: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::ZeroPriceScale);
    }
    let numerator = u128::from(left)
        .checked_mul(u128::from(right))
        .ok_or(Error::ArithmeticOverflow)?;
    ceil_u128(numerator, u128::from(denominator))
}

fn ceil_u128(numerator: u128, denominator: u128) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::ZeroPriceScale);
    }
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let rounded = if remainder == 0 {
        quotient
    } else {
        quotient.checked_add(1).ok_or(Error::ArithmeticOverflow)?
    };
    u64::try_from(rounded).map_err(|_| Error::ArithmeticOverflow)
}

fn validate_claim_count(claim_count: u8) -> Result<()> {
    if claim_count == 0 || usize::from(claim_count) > MAX_NATIVE_CLAIMS {
        return Err(Error::InvalidClaimCount);
    }
    Ok(())
}

fn require_active_index(index: usize, claim_count: u8) -> Result<()> {
    if index >= usize::from(claim_count) {
        return Err(Error::ClaimIndexOutOfRange);
    }
    Ok(())
}

fn require_inactive_zero(values: &[u64; MAX_NATIVE_CLAIMS], claim_count: u8) -> Result<()> {
    validate_claim_count(claim_count)?;
    let mut index = usize::from(claim_count);
    while index < MAX_NATIVE_CLAIMS {
        if value_at(values, index)? != 0 {
            return Err(Error::NonCanonicalInactiveClaim);
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn array_all_zero(values: &[u64; MAX_NATIVE_CLAIMS]) -> bool {
    values.iter().all(|value| *value == 0)
}

fn all_zero<const N: usize>(bytes: &[u8; N]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn value_at(values: &[u64; MAX_NATIVE_CLAIMS], index: usize) -> Result<u64> {
    values
        .get(index)
        .copied()
        .ok_or(Error::ClaimIndexOutOfRange)
}

fn set_value(values: &mut [u64; MAX_NATIVE_CLAIMS], index: usize, value: u64) -> Result<()> {
    let target = values.get_mut(index).ok_or(Error::ClaimIndexOutOfRange)?;
    *target = value;
    Ok(())
}

fn add_at(values: &mut [u64; MAX_NATIVE_CLAIMS], index: usize, amount: u64) -> Result<()> {
    let prior = value_at(values, index)?;
    set_value(
        values,
        index,
        prior.checked_add(amount).ok_or(Error::ArithmeticOverflow)?,
    )
}

fn subtract_at(values: &mut [u64; MAX_NATIVE_CLAIMS], index: usize, amount: u64) -> Result<()> {
    let prior = value_at(values, index)?;
    set_value(
        values,
        index,
        prior
            .checked_sub(amount)
            .ok_or(Error::ReservationInvariant)?,
    )
}

fn require_pair_add(prior: u64, amount: u64, next: u64) -> Result<()> {
    if prior.checked_add(amount).ok_or(Error::ArithmeticOverflow)? != next {
        return Err(Error::CapitalConservationMismatch);
    }
    Ok(())
}

fn require_pair_sub(prior: u64, amount: u64, next: u64) -> Result<()> {
    if prior
        .checked_sub(amount)
        .ok_or(Error::CapitalConservationMismatch)?
        != next
    {
        return Err(Error::CapitalConservationMismatch);
    }
    Ok(())
}

fn require_equal(prior: u64, next: u64) -> Result<()> {
    if prior != next {
        return Err(Error::CapitalConservationMismatch);
    }
    Ok(())
}

fn check_header(bytes: &[u8], exact_len: usize, magic: [u8; 8]) -> Result<()> {
    if bytes.len() != exact_len {
        return Err(Error::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != magic {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, 8)? != SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)
}

fn put_header<const N: usize>(out: &mut [u8; N], magic: [u8; 8]) {
    put(out, 0, &magic);
    put(out, 8, &SCHEMA_VERSION.to_le_bytes());
}

fn read_receipt_values(bytes: &[u8], offset: usize) -> Result<ReceiptValues> {
    Ok(ReceiptValues {
        cash: read_u64(bytes, offset)?,
        fees: read_u64(bytes, offset + 8)?,
        inventory: read_u64(bytes, offset + 16)?,
        reserved_cash: read_u64(bytes, offset + 24)?,
        reserved_outgoing: read_u64(bytes, offset + 32)?,
        reserved_incoming: read_u64(bytes, offset + 40)?,
        outstanding_quotes: read_u64(bytes, offset + 48)?,
    })
}

fn put_receipt_values<const N: usize>(out: &mut [u8; N], offset: usize, values: ReceiptValues) {
    put_u64(out, offset, values.cash);
    put_u64(out, offset + 8, values.fees);
    put_u64(out, offset + 16, values.inventory);
    put_u64(out, offset + 24, values.reserved_cash);
    put_u64(out, offset + 32, values.reserved_outgoing);
    put_u64(out, offset + 40, values.reserved_incoming);
    put_u64(out, offset + 48, values.outstanding_quotes);
}

fn read_u64_array(bytes: &[u8], offset: usize) -> Result<[u64; MAX_NATIVE_CLAIMS]> {
    let mut values = [0u64; MAX_NATIVE_CLAIMS];
    let mut index = 0usize;
    while index < MAX_NATIVE_CLAIMS {
        let byte_offset = offset
            .checked_add(index.checked_mul(8).ok_or(Error::InvalidLength)?)
            .ok_or(Error::InvalidLength)?;
        set_value(&mut values, index, read_u64(bytes, byte_offset)?)?;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(values)
}

fn put_u64_array<const N: usize>(
    out: &mut [u8; N],
    offset: usize,
    values: &[u64; MAX_NATIVE_CLAIMS],
) {
    for (index, value) in values.iter().enumerate() {
        if let Some(byte_offset) = index.checked_mul(8).and_then(|i| offset.checked_add(i)) {
            put_u64(out, byte_offset, *value);
        }
    }
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    let source = bytes.get(offset..end).ok_or(Error::InvalidLength)?;
    source.try_into().map_err(|_| Error::InvalidLength)
}

fn subslice(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    if subslice(bytes, offset, len)?.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn put<const N: usize>(out: &mut [u8; N], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(target) = out.get_mut(offset..end)
    {
        target.copy_from_slice(value);
    }
}

fn put_u64<const N: usize>(out: &mut [u8; N], offset: usize, value: u64) {
    put(out, offset, &value.to_le_bytes());
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    fn content(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero content ID")
    }

    fn binding(generation: u64, release: u8) -> DealerBinding {
        DealerBinding::new(
            MarketIdentity::new(
                content(1),
                content(2),
                content(3),
                content(4),
                content(5),
                generation,
            ),
            content(release),
            [7; 32],
            [8; 32],
        )
        .expect("binding")
    }

    fn array(first: u64, second: u64) -> [u64; MAX_NATIVE_CLAIMS] {
        let mut values = [0u64; MAX_NATIVE_CLAIMS];
        *values.first_mut().expect("first") = first;
        *values.get_mut(1).expect("second") = second;
        values
    }

    #[allow(clippy::too_many_arguments)]
    fn epoch_with(
        number: u64,
        binding: DealerBinding,
        fee_bps: u16,
        scale: u64,
        cap: [u64; MAX_NATIVE_CLAIMS],
        risk: [u64; MAX_NATIVE_CLAIMS],
        bid: [u64; MAX_NATIVE_CLAIMS],
        ask: [u64; MAX_NATIVE_CLAIMS],
    ) -> CapitalEpoch {
        CapitalEpoch::new(
            binding, number, 2, fee_bps, scale, 20, 1_000, cap, risk, bid, ask,
        )
        .expect("epoch")
    }

    fn standard() -> (CapitalEpoch, DealerState) {
        let epoch = epoch_with(
            4,
            binding(9, 6),
            100,
            100,
            array(1_000, 1_000),
            array(10, 20),
            array(40, 60),
            array(50, 70),
        );
        let (state, _) = DealerState::enter(
            &epoch,
            CapitalSnapshot::new(10_000, 1_000, 11, 12, array(100, 100)),
        )
        .expect("entry");
        (epoch, state)
    }

    fn request(
        state: DealerState,
        side: QuoteSide,
        claim_index: u8,
        quantity: u64,
    ) -> QuoteRequest {
        QuoteRequest {
            expected_generation: state.binding().market().generation(),
            expected_epoch: state.epoch(),
            expected_sequence: state.next_sequence(),
            side,
            claim_index,
            quantity,
            expiry_slot: 110,
        }
    }

    #[test]
    fn exact_encodings_round_trip_and_reject_reserved_or_trailing_bytes() {
        let (epoch, mut state) = standard();
        let entry = DealerState::enter(&epoch, state.snapshot())
            .expect("entry receipt")
            .1;
        let quote = state
            .admit_quote(&epoch, request(state, QuoteSide::CustomerBuys, 0, 10), 100)
            .expect("quote");
        assert_eq!(
            DealerBinding::decode(&state.binding().to_bytes()),
            Ok(state.binding())
        );
        assert_eq!(CapitalEpoch::decode(&epoch.to_bytes()), Ok(epoch));
        assert_eq!(DealerState::decode(&state.to_bytes()), Ok(state));
        assert_eq!(QuoteReservation::decode(&quote.to_bytes()), Ok(quote));
        assert_eq!(
            CapitalTransitionReceipt::decode(&entry.to_bytes()),
            Ok(entry)
        );
        assert_eq!(state.to_bytes().len(), DEALER_STATE_BYTES);
        assert_eq!(epoch.to_bytes().len(), CAPITAL_EPOCH_BYTES);
        assert_eq!(quote.to_bytes().len(), QUOTE_RESERVATION_BYTES);
        assert_eq!(entry.to_bytes().len(), CAPITAL_TRANSITION_RECEIPT_BYTES);

        let mut hostile = state.to_bytes();
        *hostile
            .get_mut(HEADER_RESERVED_OFFSET)
            .expect("reserved byte") = 1;
        assert_eq!(
            DealerState::decode(&hostile),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut trailing = std::vec::Vec::from(state.to_bytes());
        trailing.push(0);
        assert_eq!(DealerState::decode(&trailing), Err(Error::InvalidLength));
    }

    #[test]
    fn covered_quotes_refuse_cash_and_inventory_insolvency() {
        let (epoch, mut state) = standard();
        let cash_epoch = epoch_with(
            0,
            binding(1, 11),
            0,
            1,
            array(1_000, 1_000),
            array(0, 0),
            array(1, 1),
            array(1, 1),
        );
        let (mut cash_state, _) =
            DealerState::enter(&cash_epoch, CapitalSnapshot::new(50, 0, 0, 0, array(0, 0)))
                .expect("cash entry");
        let excessive_sell = request(cash_state, QuoteSide::CustomerSells, 1, 51);
        assert_eq!(
            cash_state.admit_quote(&cash_epoch, excessive_sell, 100),
            Err(Error::InsufficientCash)
        );
        let excessive_buy = request(state, QuoteSide::CustomerBuys, 0, 101);
        assert_eq!(
            state.admit_quote(&epoch, excessive_buy, 100),
            Err(Error::InsufficientInventory)
        );
        assert_eq!(state.outstanding_quotes(), 0);
    }

    #[test]
    fn other_compartments_never_cover_cash_quotes() {
        let epoch = epoch_with(
            0,
            binding(1, 9),
            0,
            1,
            array(1_000, 1_000),
            array(0, 0),
            array(10, 10),
            array(10, 10),
        );
        let (mut state, _) = DealerState::enter(
            &epoch,
            CapitalSnapshot::new(0, 1_000, 1_000, 1_000, array(0, 0)),
        )
        .expect("entry");
        let quote = request(state, QuoteSide::CustomerSells, 0, 1);
        assert_eq!(
            state.admit_quote(&epoch, quote, 100),
            Err(Error::InsufficientCash)
        );
        assert_eq!(state.snapshot().sponsor_loss_capital(), 1_000);
        assert_eq!(state.snapshot().realized_fees(), 1_000);
        assert_eq!(state.snapshot().prepaid_service_funding(), 1_000);
    }

    #[test]
    fn projected_inventory_requires_caps_and_loss_capital() {
        let epoch = epoch_with(
            0,
            binding(1, 9),
            0,
            1,
            array(5, 5),
            array(10, 10),
            array(1, 1),
            array(1, 1),
        );
        let (mut state, _) =
            DealerState::enter(&epoch, CapitalSnapshot::new(100, 20, 0, 0, array(1, 1)))
                .expect("entry");
        let quote = request(state, QuoteSide::CustomerSells, 0, 2);
        assert_eq!(
            state.admit_quote(&epoch, quote, 100),
            Err(Error::InsufficientLossCapital)
        );

        let roomy_risk = epoch_with(
            0,
            state.binding(),
            0,
            1,
            array(2, 5),
            array(1, 1),
            array(1, 1),
            array(1, 1),
        );
        let quote = request(state, QuoteSide::CustomerSells, 0, 2);
        assert_eq!(
            state.admit_quote(&roomy_risk, quote, 100),
            Err(Error::InventoryCapExceeded)
        );
    }

    #[test]
    fn stale_generation_epoch_release_and_sequence_are_refused() {
        let (epoch, mut state) = standard();
        let mut stale_generation = request(state, QuoteSide::CustomerBuys, 0, 1);
        stale_generation.expected_generation = 8;
        assert_eq!(
            state.admit_quote(&epoch, stale_generation, 100),
            Err(Error::GenerationMismatch)
        );
        let mut stale_epoch = request(state, QuoteSide::CustomerBuys, 0, 1);
        stale_epoch.expected_epoch = 3;
        assert_eq!(
            state.admit_quote(&epoch, stale_epoch, 100),
            Err(Error::EpochMismatch)
        );
        let wrong_release = epoch_with(
            4,
            binding(9, 10),
            100,
            100,
            array(1_000, 1_000),
            array(10, 20),
            array(40, 60),
            array(50, 70),
        );
        assert_eq!(
            state.admit_quote(
                &wrong_release,
                request(state, QuoteSide::CustomerBuys, 0, 1),
                100
            ),
            Err(Error::CapabilityReleaseMismatch)
        );
        let mut stale_sequence = request(state, QuoteSide::CustomerBuys, 0, 1);
        stale_sequence.expected_sequence = 0;
        assert_eq!(
            state.admit_quote(&epoch, stale_sequence, 100),
            Err(Error::SequenceMismatch)
        );
    }

    #[test]
    fn execution_is_exact_and_replay_protected() {
        let (epoch, mut state) = standard();
        let mut quote = state
            .admit_quote(&epoch, request(state, QuoteSide::CustomerBuys, 0, 10), 100)
            .expect("quote");
        let receipt = state
            .execute_quote(&epoch, &mut quote, 9, 110)
            .expect("execution");
        assert_eq!(receipt.customer_cash(), 6);
        assert_eq!(state.snapshot().cash(), 10_005);
        assert_eq!(state.snapshot().realized_fees(), 12);
        assert_eq!(
            value_at(&state.snapshot().inventory(), 0).expect("inventory"),
            90
        );
        assert_eq!(quote.status(), QuoteStatus::Executed);
        assert_eq!(ExecutionReceipt::decode(&receipt.to_bytes()), Ok(receipt));
        assert_eq!(receipt.to_bytes().len(), EXECUTION_RECEIPT_BYTES);
        assert_eq!(
            state.execute_quote(&epoch, &mut quote, 9, 110),
            Err(Error::QuoteNotActive)
        );
    }

    #[test]
    fn expiry_and_forged_epoch_terms_are_refused_atomically() {
        let (epoch, mut state) = standard();
        let mut too_long = request(state, QuoteSide::CustomerBuys, 0, 1);
        too_long.expiry_slot = 121;
        assert_eq!(
            state.admit_quote(&epoch, too_long, 100),
            Err(Error::InvalidExpiry)
        );

        let mut quote = state
            .admit_quote(&epoch, request(state, QuoteSide::CustomerBuys, 0, 10), 100)
            .expect("quote");
        let state_before_expiry = state;
        let quote_before_expiry = quote;
        assert_eq!(
            state.execute_quote(&epoch, &mut quote, 9, 111),
            Err(Error::QuoteExpired)
        );
        assert_eq!(state, state_before_expiry);
        assert_eq!(quote, quote_before_expiry);

        quote.notional = 4;
        quote.customer_cash = 5;
        let forged_quote = quote;
        assert_eq!(
            state.execute_quote(&epoch, &mut quote, 9, 109),
            Err(Error::QuoteTermsMismatch)
        );
        assert_eq!(state, state_before_expiry);
        assert_eq!(quote, forged_quote);
    }

    #[test]
    fn sell_execution_conserves_cash_and_realizes_fee() {
        let (epoch, mut state) = standard();
        let mut quote = state
            .admit_quote(&epoch, request(state, QuoteSide::CustomerSells, 1, 10), 100)
            .expect("quote");
        assert_eq!(quote.customer_cash(), 5);
        let receipt = state
            .execute_quote(&epoch, &mut quote, 9, 109)
            .expect("execution");
        assert_eq!(receipt.customer_cash(), 5);
        assert_eq!(state.snapshot().cash(), 9_994);
        assert_eq!(state.snapshot().realized_fees(), 12);
        assert_eq!(
            value_at(&state.snapshot().inventory(), 1).expect("inventory"),
            110
        );
    }

    #[test]
    fn cancellation_releases_coverage_and_cannot_replay() {
        let (epoch, mut state) = standard();
        let mut quote = state
            .admit_quote(&epoch, request(state, QuoteSide::CustomerSells, 0, 10), 100)
            .expect("quote");
        assert!(!state.is_quiescent());
        state.cancel_quote(&mut quote).expect("cancel");
        assert!(state.is_quiescent());
        assert_eq!(quote.status(), QuoteStatus::Cancelled);
        assert_eq!(state.cancel_quote(&mut quote), Err(Error::QuoteNotActive));
    }

    #[test]
    fn capital_change_and_exit_require_quiescence() {
        let (epoch, mut state) = standard();
        let mut quote = state
            .admit_quote(&epoch, request(state, QuoteSide::CustomerBuys, 0, 1), 100)
            .expect("quote");
        let next = epoch_with(
            5,
            state.binding(),
            100,
            100,
            array(1_000, 1_000),
            array(10, 20),
            array(40, 60),
            array(50, 70),
        );
        assert_eq!(
            state.reconfigure(
                &epoch,
                &next,
                9,
                state.next_sequence(),
                CapitalSnapshot::zero(),
                CapitalSnapshot::zero(),
            ),
            Err(Error::DealerNotQuiescent)
        );
        assert_eq!(
            state.exit(&epoch, 9, state.next_sequence()),
            Err(Error::DealerNotQuiescent)
        );
        state.cancel_quote(&mut quote).expect("cancel");
        let withdrawal = CapitalSnapshot::new(100, 0, 11, 0, array(0, 0));
        let receipt = state
            .reconfigure(
                &epoch,
                &next,
                9,
                state.next_sequence(),
                CapitalSnapshot::zero(),
                withdrawal,
            )
            .expect("reconfigure");
        assert_eq!(receipt.withdrawals(), withdrawal);
        assert_eq!(state.snapshot().cash(), 9_900);
        assert_eq!(state.snapshot().realized_fees(), 0);
        let exit = state.exit(&next, 9, state.next_sequence()).expect("exit");
        assert_eq!(exit.withdrawals().cash(), 9_900);
        assert_eq!(state.status(), DealerStatus::Exited);
        assert_eq!(state.snapshot(), CapitalSnapshot::zero());
    }

    #[test]
    fn rounding_is_named_and_overflow_is_atomic() {
        let epoch = epoch_with(
            0,
            binding(1, 9),
            1,
            3,
            array(u64::MAX, u64::MAX),
            array(0, 0),
            array(1, 1),
            array(1, 1),
        );
        let (mut state, _) =
            DealerState::enter(&epoch, CapitalSnapshot::new(u64::MAX, 0, 0, 0, array(1, 0)))
                .expect("entry");
        let mut quote = state
            .admit_quote(&epoch, request(state, QuoteSide::CustomerBuys, 0, 1), 100)
            .expect("ceil quote");
        assert_eq!(quote.customer_cash(), 2);
        let state_before = state;
        let quote_before = quote;
        assert_eq!(
            state.execute_quote(&epoch, &mut quote, 1, 101),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(state, state_before);
        assert_eq!(quote, quote_before);

        let (mut sell_state, _) =
            DealerState::enter(&epoch, CapitalSnapshot::new(10, 0, 0, 0, array(0, 0)))
                .expect("entry");
        assert_eq!(
            sell_state.admit_quote(
                &epoch,
                request(sell_state, QuoteSide::CustomerSells, 0, 1),
                100,
            ),
            Err(Error::ZeroNotional)
        );
    }

    #[test]
    fn invalid_risk_reconfiguration_is_atomic() {
        let (epoch, mut state) = standard();
        let next = epoch_with(
            5,
            state.binding(),
            100,
            100,
            array(1_000, 1_000),
            array(1_000, 1_000),
            array(40, 60),
            array(50, 70),
        );
        let prior = state;
        assert_eq!(
            state.reconfigure(
                &epoch,
                &next,
                9,
                state.next_sequence(),
                CapitalSnapshot::zero(),
                CapitalSnapshot::zero(),
            ),
            Err(Error::InsufficientLossCapital)
        );
        assert_eq!(state, prior);
    }

    #[test]
    fn exit_sequence_overflow_is_atomic() {
        let (epoch, mut state) = standard();
        state.next_sequence = u64::MAX;
        let prior = state;
        assert_eq!(
            state.exit(&epoch, 9, u64::MAX),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(state, prior);
    }

    #[test]
    fn capital_receipt_rejects_nonconservation_and_overlapping_flows() {
        assert_eq!(
            apply_capital_flow(
                CapitalSnapshot::new(10, 0, 0, 0, array(0, 0)),
                CapitalSnapshot::new(1, 0, 0, 0, array(0, 0)),
                CapitalSnapshot::new(1, 0, 0, 0, array(0, 0)),
            ),
            Err(Error::OverlappingCapitalFlow)
        );
        let (epoch, state) = standard();
        let (_, receipt) = DealerState::enter(&epoch, state.snapshot()).expect("entry");
        let mut bytes = receipt.to_bytes();
        let new_cash_offset = CAPITAL_NEW_OFFSET;
        *bytes.get_mut(new_cash_offset).expect("new cash byte") ^= 1;
        assert_eq!(
            CapitalTransitionReceipt::decode(&bytes),
            Err(Error::CapitalConservationMismatch)
        );
    }
}
