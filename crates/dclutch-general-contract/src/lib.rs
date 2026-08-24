#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Pure, fixed-capacity contract for dClutch's optional General frequent-batch
//! venue.
//!
//! The crate owns venue semantics, not Solana account access, signatures,
//! hashing, token transfers, or transaction construction. An adapter must
//! authenticate the content identities and transcript steps supplied here,
//! then persist returned state atomically.

use core::convert::{TryFrom, TryInto};

/// Exact width of an opaque content identity.
pub const CONTENT_ID_BYTES: usize = 32;
/// Provisional program-profile bound on claim-basis cells.
///
/// It is neither mathematical nor a product-family limit. A new capability
/// release and capacity-profile identity may lift it while preserving V1
/// Markets and receipts.
pub const MAX_OUTCOMES_V1: usize = 16;
/// Provisional program-profile bound on executions checked in one page.
///
/// A new capability release may change the page envelope without changing the
/// candidate objective or complete-set conservation law.
pub const MAX_EXECUTIONS_PER_PAGE_V1: usize = 4;
/// Exact canonical byte width of [`GeneralConfigV1`].
pub const GENERAL_CONFIG_BYTES: usize = 232;
/// Fixed byte prefix of a [`PortfolioOrderV1`] before its exact `N` coefficients.
pub const PORTFOLIO_ORDER_BASE_BYTES: usize = 200;
/// Fixed byte prefix of a [`SettlementReceiptV1`] before its exact `N` deltas.
pub const SETTLEMENT_RECEIPT_BASE_BYTES: usize = 176;

const CONFIG_MAGIC: [u8; 8] = *b"DCLTGEN1";
const ORDER_MAGIC: [u8; 8] = *b"DCLTGOR1";
const RECEIPT_MAGIC: [u8; 8] = *b"DCLTGSR1";
const SCHEMA_V1: u16 = 1;
const ARTIFACT_PROFILE_V1: u16 = 1;

/// Refusal returned by the General venue contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An exact-width byte record had another length.
    InvalidLength,
    /// A record did not carry its canonical magic.
    InvalidMagic,
    /// A schema or artifact profile is not implemented.
    UnsupportedSchema,
    /// Reserved or unused fields were not canonical zeroes.
    NonCanonicalReservedBytes,
    /// A required content identity was the all-zero sentinel.
    ZeroIdentifier,
    /// Two immutable authorities or occurrence generations differed.
    AuthorityMismatch,
    /// A provisional capacity bound was exceeded.
    CapacityExceeded,
    /// A configured capacity or duration was zero.
    ZeroCapacity,
    /// The outcome count was outside the selected V1 envelope.
    InvalidOutcomeCount,
    /// A price scale was zero.
    ZeroPriceScale,
    /// Simplex coordinates did not sum exactly to the configured scale.
    InvalidSimplexPrice,
    /// A price or coefficient was nonzero outside the exact ClaimBasis width.
    NonCanonicalPortfolio,
    /// An atomic portfolio contained no nonzero coefficient.
    EmptyPortfolio,
    /// Checked exact integer arithmetic overflowed.
    ArithmeticOverflow,
    /// A signed token amount did not fit the V1 adapter word.
    TokenAmountOutOfRange,
    /// A slot interval overflowed.
    SlotOverflow,
    /// A transition was not admitted from the current phase.
    InvalidPhase,
    /// A call occurred before or after its immutable slot window.
    OutsideWindow,
    /// A sequence, cursor, page, nonce, or transcript predecessor mismatched.
    CursorMismatch,
    /// A transcript successor was not a nonzero new commitment.
    InvalidTranscriptStep,
    /// Page entries or orders were not in strict canonical order.
    NonCanonicalOrder,
    /// A page count was zero or exceeded the V1 page envelope.
    InvalidPageCount,
    /// An order did not belong to this Market generation and batch.
    OrderBindingMismatch,
    /// An order was expired for the complete settlement window.
    OrderExpired,
    /// An order had been cancelled, consumed, or otherwise was not open.
    OrderUnavailable,
    /// A requested atomic lot fill was zero or exceeded remaining lots.
    InvalidFill,
    /// A portfolio fill violated the order's exact debit limit.
    LimitViolated,
    /// Candidate recomputation did not equal its committed claim.
    CandidateClaimMismatch,
    /// Candidate net inventory was not one virtual complete-set vector.
    IncompleteSetImbalance,
    /// Quote flow did not exactly capitalize or release the complete sets.
    QuoteConservationMismatch,
    /// A Candidate was not the batch's selected best valid submission.
    CandidateNotSelected,
    /// Prefix-carry settlement ended with fractional scale units outstanding.
    RoundingCarryOutstanding,
    /// Hoard principal and complete-set liabilities were unequal.
    HoardInvariantViolation,
    /// Hoard principal was insufficient for a complete-set burn.
    InsufficientHoardPrincipal,
    /// Funding compartment principal was insufficient.
    InsufficientFunding,
    /// A zero funding debit was requested.
    ZeroFundingDebit,
    /// Immutable funding did not equal remaining, spent, and refunded amounts.
    FundingConservationMismatch,
    /// Retirement was attempted before all owned state was quiescent.
    NotQuiescent,
}

/// Result alias for General contract operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Validated nonzero opaque content identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentId([u8; CONTENT_ID_BYTES]);

impl ContentId {
    /// Construct a nonzero identity.
    pub fn new(bytes: [u8; CONTENT_ID_BYTES]) -> Result<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroIdentifier);
        }
        Ok(Self(bytes))
    }

    /// Return the exact opaque bytes.
    pub const fn to_bytes(self) -> [u8; CONTENT_ID_BYTES] {
        self.0
    }

    /// Borrow the exact opaque bytes.
    pub const fn as_bytes(&self) -> &[u8; CONTENT_ID_BYTES] {
        &self.0
    }
}

/// Immutable capacity and authority contract for one General venue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigV1 {
    capacity_profile_id: ContentId,
    market_identity_id: ContentId,
    claim_basis_id: ContentId,
    capability_release_id: ContentId,
    settlement_asset_id: ContentId,
    generation: u64,
    price_scale: u64,
    collection_slots: u64,
    selection_slots: u64,
    settlement_slots: u64,
    max_orders_per_candidate: u32,
    max_pages_per_candidate: u32,
    outcome_count: u16,
}

/// Inputs for one immutable [`GeneralConfigV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigV1Input {
    /// Identity of the liftable capacity profile.
    pub capacity_profile_id: ContentId,
    /// Exact content identity of the immutable Market identity preimage.
    pub market_identity_id: ContentId,
    /// Exact ClaimBasis content identity from that Market identity.
    pub claim_basis_id: ContentId,
    /// Reviewed General capability release selected by the manifest.
    pub capability_release_id: ContentId,
    /// Realm-selected settlement asset profile identity.
    pub settlement_asset_id: ContentId,
    /// Immutable occurrence generation.
    pub generation: u64,
    /// Positive integer scale whose simplex coordinates sum exactly.
    pub price_scale: u64,
    /// Positive collection-window width in slots.
    pub collection_slots: u64,
    /// Positive candidate-selection width in slots.
    pub selection_slots: u64,
    /// Positive winning-candidate settlement width in slots.
    pub settlement_slots: u64,
    /// Profile bound on executions in one candidate.
    pub max_orders_per_candidate: u32,
    /// Profile bound on verification/settlement pages.
    pub max_pages_per_candidate: u32,
    /// Exact finite ClaimBasis width.
    pub outcome_count: u16,
}

impl GeneralConfigV1 {
    /// Validate and construct one immutable venue contract.
    pub fn new(input: GeneralConfigV1Input) -> Result<Self> {
        let outcome_count = usize::from(input.outcome_count);
        if !(2..=MAX_OUTCOMES_V1).contains(&outcome_count) {
            return Err(Error::InvalidOutcomeCount);
        }
        if input.price_scale == 0 {
            return Err(Error::ZeroPriceScale);
        }
        if input.collection_slots == 0
            || input.selection_slots == 0
            || input.settlement_slots == 0
            || input.max_orders_per_candidate == 0
            || input.max_pages_per_candidate == 0
        {
            return Err(Error::ZeroCapacity);
        }
        let page_capacity = input
            .max_pages_per_candidate
            .checked_mul(
                u32::try_from(MAX_EXECUTIONS_PER_PAGE_V1).map_err(|_| Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        if input.max_orders_per_candidate > page_capacity {
            return Err(Error::CapacityExceeded);
        }
        Ok(Self {
            capacity_profile_id: input.capacity_profile_id,
            market_identity_id: input.market_identity_id,
            claim_basis_id: input.claim_basis_id,
            capability_release_id: input.capability_release_id,
            settlement_asset_id: input.settlement_asset_id,
            generation: input.generation,
            price_scale: input.price_scale,
            collection_slots: input.collection_slots,
            selection_slots: input.selection_slots,
            settlement_slots: input.settlement_slots,
            max_orders_per_candidate: input.max_orders_per_candidate,
            max_pages_per_candidate: input.max_pages_per_candidate,
            outcome_count: input.outcome_count,
        })
    }

    /// Decode the exact canonical config preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, GENERAL_CONFIG_BYTES)?;
        if array::<8>(bytes, 0)? != CONFIG_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SCHEMA_V1 || read_u16(bytes, 10)? != ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 14, 2)?;
        require_zero(bytes, 224, 8)?;
        Self::new(GeneralConfigV1Input {
            outcome_count: read_u16(bytes, 12)?,
            capacity_profile_id: read_id(bytes, 16)?,
            market_identity_id: read_id(bytes, 48)?,
            claim_basis_id: read_id(bytes, 80)?,
            capability_release_id: read_id(bytes, 112)?,
            settlement_asset_id: read_id(bytes, 144)?,
            generation: read_u64(bytes, 176)?,
            price_scale: read_u64(bytes, 184)?,
            collection_slots: read_u64(bytes, 192)?,
            selection_slots: read_u64(bytes, 200)?,
            settlement_slots: read_u64(bytes, 208)?,
            max_orders_per_candidate: read_u32(bytes, 216)?,
            max_pages_per_candidate: read_u32(bytes, 220)?,
        })
    }

    /// Encode the exact canonical config preimage.
    pub fn to_bytes(self) -> [u8; GENERAL_CONFIG_BYTES] {
        let mut out = [0u8; GENERAL_CONFIG_BYTES];
        put(&mut out, 0, &CONFIG_MAGIC);
        put(&mut out, 8, &SCHEMA_V1.to_le_bytes());
        put(&mut out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(&mut out, 12, &self.outcome_count.to_le_bytes());
        put(&mut out, 16, self.capacity_profile_id.as_bytes());
        put(&mut out, 48, self.market_identity_id.as_bytes());
        put(&mut out, 80, self.claim_basis_id.as_bytes());
        put(&mut out, 112, self.capability_release_id.as_bytes());
        put(&mut out, 144, self.settlement_asset_id.as_bytes());
        put(&mut out, 176, &self.generation.to_le_bytes());
        put(&mut out, 184, &self.price_scale.to_le_bytes());
        put(&mut out, 192, &self.collection_slots.to_le_bytes());
        put(&mut out, 200, &self.selection_slots.to_le_bytes());
        put(&mut out, 208, &self.settlement_slots.to_le_bytes());
        put(&mut out, 216, &self.max_orders_per_candidate.to_le_bytes());
        put(&mut out, 220, &self.max_pages_per_candidate.to_le_bytes());
        out
    }

    /// Return the Market identity commitment.
    pub const fn market_identity_id(self) -> ContentId {
        self.market_identity_id
    }

    /// Return the selected liftable capacity-profile identity.
    pub const fn capacity_profile_id(self) -> ContentId {
        self.capacity_profile_id
    }

    /// Return the exact ClaimBasis identity.
    pub const fn claim_basis_id(self) -> ContentId {
        self.claim_basis_id
    }

    /// Return the selected capability release identity.
    pub const fn capability_release_id(self) -> ContentId {
        self.capability_release_id
    }

    /// Return the Realm-selected settlement-asset identity.
    pub const fn settlement_asset_id(self) -> ContentId {
        self.settlement_asset_id
    }

    /// Return the immutable occurrence generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the exact simplex scale.
    pub const fn price_scale(self) -> u64 {
        self.price_scale
    }

    /// Return the finite ClaimBasis width.
    pub const fn outcome_count(self) -> u16 {
        self.outcome_count
    }

    /// Return the immutable candidate execution bound.
    pub const fn max_orders_per_candidate(self) -> u32 {
        self.max_orders_per_candidate
    }

    /// Return the immutable candidate page bound.
    pub const fn max_pages_per_candidate(self) -> u32 {
        self.max_pages_per_candidate
    }
}

/// Lifecycle of the General capability root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GeneralPhase {
    /// New batches may be opened.
    Active = 0,
    /// New batches are refused while existing batches converge.
    Quiescing = 1,
    /// Every owned batch has retired and funding may be refunded.
    Terminal = 2,
    /// Funding/rent ownership has been discharged by the adapter.
    Retired = 3,
}

/// Compact mutable root for a General capability child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRootV1 {
    config_id: ContentId,
    generation: u64,
    next_batch_sequence: u64,
    open_batches: u32,
    phase: GeneralPhase,
}

impl GeneralRootV1 {
    /// Found one active General root bound to an authenticated config.
    pub const fn founding(config_id: ContentId, generation: u64) -> Self {
        Self {
            config_id,
            generation,
            next_batch_sequence: 0,
            open_batches: 0,
            phase: GeneralPhase::Active,
        }
    }

    /// Reserve the next unique batch sequence.
    pub fn open_batch(&mut self) -> Result<u64> {
        if self.phase != GeneralPhase::Active {
            return Err(Error::InvalidPhase);
        }
        let sequence = self.next_batch_sequence;
        self.next_batch_sequence = sequence.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        self.open_batches = self
            .open_batches
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(sequence)
    }

    /// Stop admitting batches while preserving existing settlement work.
    pub fn request_quiescence(&mut self) -> Result<()> {
        if self.phase != GeneralPhase::Active {
            return Err(Error::InvalidPhase);
        }
        self.phase = GeneralPhase::Quiescing;
        Ok(())
    }

    /// Account for one fully retired owned batch.
    pub fn close_batch(&mut self) -> Result<()> {
        if self.phase == GeneralPhase::Retired || self.open_batches == 0 {
            return Err(Error::InvalidPhase);
        }
        self.open_batches = self
            .open_batches
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Enter terminal state after every batch is quiescent and retired.
    pub fn enter_terminal(&mut self) -> Result<()> {
        if self.phase != GeneralPhase::Quiescing || self.open_batches != 0 {
            return Err(Error::NotQuiescent);
        }
        self.phase = GeneralPhase::Terminal;
        Ok(())
    }

    /// Retire after an adapter proves segregated funding has been discharged.
    pub fn retire(&mut self, funding_discharged: bool) -> Result<()> {
        if self.phase != GeneralPhase::Terminal || !funding_discharged {
            return Err(Error::NotQuiescent);
        }
        self.phase = GeneralPhase::Retired;
        Ok(())
    }

    /// Return the current lifecycle phase.
    pub const fn phase(self) -> GeneralPhase {
        self.phase
    }

    /// Return the authenticated config commitment.
    pub const fn config_id(self) -> ContentId {
        self.config_id
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the exact direct batch-child count.
    pub const fn open_batches(self) -> u32 {
        self.open_batches
    }
}

/// Exact signed coefficient portfolio and immutable execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioOrderV1<const N: usize> {
    market_identity_id: ContentId,
    claim_basis_id: ContentId,
    owner: ContentId,
    order_id: ContentId,
    generation: u64,
    batch_sequence: u64,
    nonce: u64,
    valid_until_slot: u64,
    max_lots: u64,
    max_quote_debit_per_lot_numerator: i128,
    coefficients: [i64; N],
    outcome_count: u16,
}

/// Inputs for one atomic coefficient portfolio order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioOrderV1Input<const N: usize> {
    /// Market identity commitment.
    pub market_identity_id: ContentId,
    /// Exact ClaimBasis identity.
    pub claim_basis_id: ContentId,
    /// Signing owner's adapter-authenticated identity.
    pub owner: ContentId,
    /// Unique content identity of the signed order preimage.
    pub order_id: ContentId,
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact batch sequence in which this order may execute.
    pub batch_sequence: u64,
    /// Owner-scoped replay nonce.
    pub nonce: u64,
    /// Slot through which winning settlement remains valid.
    pub valid_until_slot: u64,
    /// Maximum scalar lots; all coefficients fill by the same scalar.
    pub max_lots: u64,
    /// Upper bound on `dot(coefficients, prices)` per lot, in scale units.
    pub max_quote_debit_per_lot_numerator: i128,
    /// Signed, cell-canonical coefficients for one atomic lot.
    pub coefficients: [i64; N],
    /// Exact ClaimBasis width, which must equal the selected const width.
    pub outcome_count: u16,
}

impl<const N: usize> PortfolioOrderV1<N> {
    /// Validate and construct one atomic portfolio order.
    pub fn new(input: PortfolioOrderV1Input<N>) -> Result<Self> {
        validate_portfolio(&input.coefficients, input.outcome_count, true)?;
        if input.max_lots == 0 {
            return Err(Error::InvalidFill);
        }
        Ok(Self {
            market_identity_id: input.market_identity_id,
            claim_basis_id: input.claim_basis_id,
            owner: input.owner,
            order_id: input.order_id,
            generation: input.generation,
            batch_sequence: input.batch_sequence,
            nonce: input.nonce,
            valid_until_slot: input.valid_until_slot,
            max_lots: input.max_lots,
            max_quote_debit_per_lot_numerator: input.max_quote_debit_per_lot_numerator,
            coefficients: input.coefficients,
            outcome_count: input.outcome_count,
        })
    }

    /// Decode the exact canonical signed-order preimage.
    pub fn encoded_len() -> Result<usize> {
        validate_width(N)?;
        PORTFOLIO_ORDER_BASE_BYTES
            .checked_add(N.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Decode one exact-width canonical signed-order preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, Self::encoded_len()?)?;
        if array::<8>(bytes, 0)? != ORDER_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SCHEMA_V1 || read_u16(bytes, 10)? != ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 14, 2)?;
        let mut coefficients = [0i64; N];
        let mut index = 0usize;
        while index < N {
            let offset = 200usize
                .checked_add(index.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            let target = coefficients.get_mut(index).ok_or(Error::InvalidLength)?;
            *target = read_i64(bytes, offset)?;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Self::new(PortfolioOrderV1Input {
            outcome_count: read_u16(bytes, 12)?,
            market_identity_id: read_id(bytes, 16)?,
            claim_basis_id: read_id(bytes, 48)?,
            owner: read_id(bytes, 80)?,
            order_id: read_id(bytes, 112)?,
            generation: read_u64(bytes, 144)?,
            batch_sequence: read_u64(bytes, 152)?,
            nonce: read_u64(bytes, 160)?,
            valid_until_slot: read_u64(bytes, 168)?,
            max_lots: read_u64(bytes, 176)?,
            max_quote_debit_per_lot_numerator: read_i128(bytes, 184)?,
            coefficients,
        })
    }

    /// Encode into one exact-width caller-owned signed-order buffer.
    #[allow(clippy::needless_borrow)]
    pub fn encode(&self, mut out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len()?)?;
        out.fill(0);
        put(&mut out, 0, &ORDER_MAGIC);
        put(&mut out, 8, &SCHEMA_V1.to_le_bytes());
        put(&mut out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(&mut out, 12, &self.outcome_count.to_le_bytes());
        put(&mut out, 16, self.market_identity_id.as_bytes());
        put(&mut out, 48, self.claim_basis_id.as_bytes());
        put(&mut out, 80, self.owner.as_bytes());
        put(&mut out, 112, self.order_id.as_bytes());
        put(&mut out, 144, &self.generation.to_le_bytes());
        put(&mut out, 152, &self.batch_sequence.to_le_bytes());
        put(&mut out, 160, &self.nonce.to_le_bytes());
        put(&mut out, 168, &self.valid_until_slot.to_le_bytes());
        put(&mut out, 176, &self.max_lots.to_le_bytes());
        put(
            &mut out,
            184,
            &self.max_quote_debit_per_lot_numerator.to_le_bytes(),
        );
        for (index, coefficient) in self.coefficients.iter().enumerate() {
            if let Some(offset) = index
                .checked_mul(8)
                .and_then(|part| 200usize.checked_add(part))
            {
                put(&mut out, offset, &coefficient.to_le_bytes());
            }
        }
        Ok(())
    }

    /// Return the unique signed-order identity.
    pub const fn order_id(self) -> ContentId {
        self.order_id
    }

    /// Return the owner identity.
    pub const fn owner(self) -> ContentId {
        self.owner
    }

    /// Return the immutable replay nonce.
    pub const fn nonce(self) -> u64 {
        self.nonce
    }

    /// Return the Market identity commitment.
    pub const fn market_identity_id(self) -> ContentId {
        self.market_identity_id
    }

    /// Return the exact ClaimBasis commitment.
    pub const fn claim_basis_id(self) -> ContentId {
        self.claim_basis_id
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the exact batch sequence.
    pub const fn batch_sequence(self) -> u64 {
        self.batch_sequence
    }

    /// Return the last slot through which this order remains executable.
    pub const fn valid_until_slot(self) -> u64 {
        self.valid_until_slot
    }

    /// Return one atomic-lot coefficient vector.
    pub const fn coefficients(self) -> [i64; N] {
        self.coefficients
    }
}

/// Mutable lifecycle of one adapter-authenticated signed order record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OrderPhase {
    /// At least one lot remains executable.
    Open = 0,
    /// The owner cancelled before the batch lock boundary.
    Cancelled = 1,
    /// Every lot was consumed by one or more settlement receipts.
    Consumed = 2,
}

/// Replay and partial-fill state for one unique signed order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderStateV1 {
    order_id: ContentId,
    owner: ContentId,
    nonce: u64,
    remaining_lots: u64,
    phase: OrderPhase,
}

impl OrderStateV1 {
    /// Open replay state after the adapter authenticates the signature and
    /// reserves the unique `(owner, nonce, order_id)` key.
    pub const fn open<const N: usize>(order: PortfolioOrderV1<N>) -> Self {
        Self {
            order_id: order.order_id,
            owner: order.owner,
            nonce: order.nonce,
            remaining_lots: order.max_lots,
            phase: OrderPhase::Open,
        }
    }

    /// Cancel before the batch's immutable collection close.
    pub fn cancel(&mut self, owner: ContentId, now_slot: u64, collection_close: u64) -> Result<()> {
        if owner != self.owner {
            return Err(Error::AuthorityMismatch);
        }
        if self.phase != OrderPhase::Open {
            return Err(Error::OrderUnavailable);
        }
        if now_slot >= collection_close {
            return Err(Error::OutsideWindow);
        }
        self.phase = OrderPhase::Cancelled;
        Ok(())
    }

    fn validate_snapshot<const N: usize>(
        self,
        order: PortfolioOrderV1<N>,
        fill_lots: u64,
    ) -> Result<()> {
        if self.order_id != order.order_id || self.owner != order.owner || self.nonce != order.nonce
        {
            return Err(Error::OrderBindingMismatch);
        }
        if self.phase != OrderPhase::Open {
            return Err(Error::OrderUnavailable);
        }
        if fill_lots == 0 || fill_lots > self.remaining_lots {
            return Err(Error::InvalidFill);
        }
        Ok(())
    }

    fn consume<const N: usize>(
        &mut self,
        order: PortfolioOrderV1<N>,
        fill_lots: u64,
    ) -> Result<()> {
        self.validate_snapshot(order, fill_lots)?;
        self.remaining_lots = self
            .remaining_lots
            .checked_sub(fill_lots)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.remaining_lots == 0 {
            self.phase = OrderPhase::Consumed;
        }
        Ok(())
    }

    /// Return the exact remaining atomic lot count.
    pub const fn remaining_lots(self) -> u64 {
        self.remaining_lots
    }

    /// Return the replay lifecycle phase.
    pub const fn phase(self) -> OrderPhase {
        self.phase
    }

    /// Return the signed-order commitment.
    pub const fn order_id(self) -> ContentId {
        self.order_id
    }
}

/// One atomic scalar fill presented in a verification or settlement page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionV1<const N: usize> {
    /// Full signed atomic portfolio preimage.
    pub order: PortfolioOrderV1<N>,
    /// Adapter-authenticated replay state snapshot.
    pub order_state: OrderStateV1,
    /// One scalar applied uniformly to every coefficient.
    pub fill_lots: u64,
}

/// Permissionless candidate submission preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateSubmissionV1<const N: usize> {
    /// Exact Market identity commitment.
    pub market_identity_id: ContentId,
    /// Exact ClaimBasis identity.
    pub claim_basis_id: ContentId,
    /// Candidate author; no allowlist semantics attach to this identity.
    pub submitter: ContentId,
    /// First adapter-authenticated transcript commitment.
    pub initial_transcript_id: ContentId,
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact batch sequence.
    pub batch_sequence: u64,
    /// Last slot at which verification may finish.
    pub valid_until_slot: u64,
    /// Claimed execution count recomputed by the verifier.
    pub claimed_execution_count: u32,
    /// Claimed preference-surplus score recomputed by the verifier.
    pub claimed_score: u128,
    /// Exact scaled-integer simplex coordinates.
    pub prices: [u64; N],
    /// Exact ClaimBasis width, which must equal the selected const width.
    pub outcome_count: u16,
}

/// Verification lifecycle of a candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CandidatePhase {
    /// No verification page has been accepted.
    Submitted = 0,
    /// At least one page has committed progress.
    Verifying = 1,
    /// Every execution and conservation check succeeded.
    Valid = 2,
    /// The batch consumed this candidate's one consideration right.
    Considered = 3,
    /// The candidate cannot be resumed under this immutable identity.
    Rejected = 4,
}

/// Fixed-layout candidate state and committed resumable verifier cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateStateV1<const N: usize> {
    candidate_id: ContentId,
    submission: CandidateSubmissionV1<N>,
    phase: CandidatePhase,
    verified_pages: u32,
    verified_executions: u32,
    last_order_id: Option<ContentId>,
    transcript_id: ContentId,
    net_coefficients: [i128; N],
    total_quote_debit_numerator: i128,
    score: u128,
    complete_set_delta: i128,
}

/// One bounded verification page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerificationPageV1<const N: usize> {
    /// Zero-based page index committed in the candidate cursor.
    pub page_index: u32,
    /// Current transcript commitment.
    pub prior_transcript_id: ContentId,
    /// Adapter-derived commitment to this predecessor and canonical page.
    pub next_transcript_id: ContentId,
    /// Number of leading executions used in the fixed envelope.
    pub execution_count: u8,
    /// Fixed V1 execution envelope.
    pub executions: [Option<ExecutionV1<N>>; MAX_EXECUTIONS_PER_PAGE_V1],
}

impl<const N: usize> CandidateStateV1<N> {
    /// Create a permissionless submitted candidate after exact authority and
    /// simplex validation. Signature and digest authentication are adapter
    /// responsibilities.
    pub fn submit(
        candidate_id: ContentId,
        submission: CandidateSubmissionV1<N>,
        config: GeneralConfigV1,
        batch: BatchRootV1,
        now_slot: u64,
    ) -> Result<Self> {
        validate_authority(
            submission.market_identity_id,
            submission.claim_basis_id,
            submission.generation,
            config,
        )?;
        if submission.batch_sequence != batch.sequence || batch.phase != BatchPhase::Selecting {
            return Err(Error::InvalidPhase);
        }
        if now_slot < batch.collection_close || now_slot >= batch.selection_close {
            return Err(Error::OutsideWindow);
        }
        if submission.valid_until_slot < batch.selection_close
            || submission.claimed_execution_count == 0
            || submission.claimed_execution_count > config.max_orders_per_candidate
            || submission.outcome_count != config.outcome_count
        {
            return Err(Error::CandidateClaimMismatch);
        }
        validate_prices(
            &submission.prices,
            submission.outcome_count,
            config.price_scale,
        )?;
        Ok(Self {
            candidate_id,
            transcript_id: submission.initial_transcript_id,
            submission,
            phase: CandidatePhase::Submitted,
            verified_pages: 0,
            verified_executions: 0,
            last_order_id: None,
            net_coefficients: [0; N],
            total_quote_debit_numerator: 0,
            score: 0,
            complete_set_delta: 0,
        })
    }

    /// Check and commit one bounded page. Invalid inputs leave `self`
    /// unchanged because work is performed on a copied cursor first.
    pub fn verify_page(
        &mut self,
        page: VerificationPageV1<N>,
        config: GeneralConfigV1,
        batch: BatchRootV1,
        now_slot: u64,
    ) -> Result<()> {
        if self.phase != CandidatePhase::Submitted && self.phase != CandidatePhase::Verifying {
            return Err(Error::InvalidPhase);
        }
        if batch.phase != BatchPhase::Selecting
            || batch.sequence != self.submission.batch_sequence
            || now_slot >= batch.selection_close
            || now_slot > self.submission.valid_until_slot
        {
            return Err(Error::OutsideWindow);
        }
        if page.page_index != self.verified_pages || page.prior_transcript_id != self.transcript_id
        {
            return Err(Error::CursorMismatch);
        }
        if page.next_transcript_id == page.prior_transcript_id {
            return Err(Error::InvalidTranscriptStep);
        }
        let count = usize::from(page.execution_count);
        if count == 0 || count > MAX_EXECUTIONS_PER_PAGE_V1 {
            return Err(Error::InvalidPageCount);
        }
        if page.executions.iter().take(count).any(Option::is_none)
            || page.executions.iter().skip(count).any(Option::is_some)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let next_page_count = self
            .verified_pages
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if next_page_count > config.max_pages_per_candidate {
            return Err(Error::CapacityExceeded);
        }
        let mut next = *self;
        for execution in page.executions.iter().take(count).flatten() {
            next.verify_execution(*execution, config, batch)?;
        }
        next.verified_pages = next_page_count;
        next.transcript_id = page.next_transcript_id;
        next.phase = CandidatePhase::Verifying;
        *self = next;
        Ok(())
    }

    fn verify_execution(
        &mut self,
        execution: ExecutionV1<N>,
        config: GeneralConfigV1,
        batch: BatchRootV1,
    ) -> Result<()> {
        validate_execution_binding(execution, config, batch)?;
        if self
            .last_order_id
            .is_some_and(|last| execution.order.order_id <= last)
        {
            return Err(Error::NonCanonicalOrder);
        }
        execution
            .order_state
            .validate_snapshot(execution.order, execution.fill_lots)?;
        let debit_per_lot = portfolio_dot(
            &execution.order.coefficients,
            &self.submission.prices,
            config.outcome_count,
        )?;
        if debit_per_lot > execution.order.max_quote_debit_per_lot_numerator {
            return Err(Error::LimitViolated);
        }
        let fill = i128::from(execution.fill_lots);
        let debit = debit_per_lot
            .checked_mul(fill)
            .ok_or(Error::ArithmeticOverflow)?;
        self.total_quote_debit_numerator = self
            .total_quote_debit_numerator
            .checked_add(debit)
            .ok_or(Error::ArithmeticOverflow)?;
        let preference_per_lot = execution
            .order
            .max_quote_debit_per_lot_numerator
            .checked_sub(debit_per_lot)
            .ok_or(Error::ArithmeticOverflow)?;
        let preference = preference_per_lot
            .checked_mul(fill)
            .ok_or(Error::ArithmeticOverflow)?;
        let preference_score = u128::try_from(preference).map_err(|_| Error::ArithmeticOverflow)?;
        self.score = self
            .score
            .checked_add(preference_score)
            .ok_or(Error::ArithmeticOverflow)?;
        let width = usize::from(config.outcome_count);
        for (target, coefficient) in self
            .net_coefficients
            .iter_mut()
            .zip(execution.order.coefficients.iter())
            .take(width)
        {
            let delta = i128::from(*coefficient)
                .checked_mul(fill)
                .ok_or(Error::ArithmeticOverflow)?;
            *target = target.checked_add(delta).ok_or(Error::ArithmeticOverflow)?;
        }
        self.verified_executions = self
            .verified_executions
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.verified_executions > config.max_orders_per_candidate {
            return Err(Error::CapacityExceeded);
        }
        self.last_order_id = Some(execution.order.order_id);
        Ok(())
    }

    /// Finish verification and prove the execution set has one exact virtual
    /// complete-set inventory delta and matching quote capitalization.
    pub fn finish_verification(&mut self, config: GeneralConfigV1) -> Result<()> {
        if self.phase != CandidatePhase::Verifying {
            return Err(Error::InvalidPhase);
        }
        if self.verified_executions != self.submission.claimed_execution_count
            || self.score != self.submission.claimed_score
        {
            return Err(Error::CandidateClaimMismatch);
        }
        let width = usize::from(config.outcome_count);
        let complete_set_delta = *self
            .net_coefficients
            .first()
            .ok_or(Error::InvalidOutcomeCount)?;
        if self
            .net_coefficients
            .iter()
            .take(width)
            .any(|value| *value != complete_set_delta)
        {
            return Err(Error::IncompleteSetImbalance);
        }
        let expected_debit = complete_set_delta
            .checked_mul(i128::from(config.price_scale))
            .ok_or(Error::ArithmeticOverflow)?;
        if self.total_quote_debit_numerator != expected_debit {
            return Err(Error::QuoteConservationMismatch);
        }
        self.complete_set_delta = complete_set_delta;
        self.phase = CandidatePhase::Valid;
        Ok(())
    }

    /// Permanently reject a timed-out or invalid candidate account.
    pub fn reject(&mut self) -> Result<()> {
        if self.phase == CandidatePhase::Valid
            || self.phase == CandidatePhase::Considered
            || self.phase == CandidatePhase::Rejected
        {
            return Err(Error::InvalidPhase);
        }
        self.phase = CandidatePhase::Rejected;
        Ok(())
    }

    /// Return the candidate identity.
    pub const fn candidate_id(self) -> ContentId {
        self.candidate_id
    }

    /// Return the exact verified preference-surplus score.
    pub const fn score(self) -> u128 {
        self.score
    }

    /// Return the verified lifecycle phase.
    pub const fn phase(self) -> CandidatePhase {
        self.phase
    }

    /// Return the final verified transcript commitment.
    pub const fn transcript_id(self) -> ContentId {
        self.transcript_id
    }

    /// Return the permissionless candidate submitter identity.
    pub const fn submitter(self) -> ContentId {
        self.submission.submitter
    }

    /// Return the exact batch sequence.
    pub const fn batch_sequence(self) -> u64 {
        self.submission.batch_sequence
    }

    /// Return the exact verified execution count.
    pub const fn verified_executions(self) -> u32 {
        self.verified_executions
    }

    /// Return the verified virtual complete-set delta.
    pub const fn complete_set_delta(self) -> i128 {
        self.complete_set_delta
    }
}

/// Lifecycle of one frequent batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BatchPhase {
    /// Signed orders may be posted or cancelled.
    Collecting = 0,
    /// Permissionless candidates may be verified and considered.
    Selecting = 1,
    /// The deterministic best valid submitted candidate is being settled.
    Settling = 2,
    /// Hoard conversion is committed and paginated receipts must converge.
    Applying = 3,
    /// No further economic mutation is possible.
    Quiescent = 4,
    /// Owned candidate/receipt/rent state has been discharged.
    Retired = 5,
}

/// Fixed-layout root for one frequent batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchRootV1 {
    config_id: ContentId,
    sequence: u64,
    collection_close: u64,
    selection_close: u64,
    settlement_close: u64,
    candidate_count: u32,
    best_candidate_id: Option<ContentId>,
    best_score: u128,
    phase: BatchPhase,
}

impl BatchRootV1 {
    /// Create one batch from an already-reserved root sequence.
    pub fn open(
        config_id: ContentId,
        sequence: u64,
        open_slot: u64,
        config: GeneralConfigV1,
    ) -> Result<Self> {
        let collection_close = open_slot
            .checked_add(config.collection_slots)
            .ok_or(Error::SlotOverflow)?;
        let selection_close = collection_close
            .checked_add(config.selection_slots)
            .ok_or(Error::SlotOverflow)?;
        let settlement_close = selection_close
            .checked_add(config.settlement_slots)
            .ok_or(Error::SlotOverflow)?;
        Ok(Self {
            config_id,
            sequence,
            collection_close,
            selection_close,
            settlement_close,
            candidate_count: 0,
            best_candidate_id: None,
            best_score: 0,
            phase: BatchPhase::Collecting,
        })
    }

    /// Close collection at or after its immutable boundary.
    pub fn open_selection(&mut self, now_slot: u64) -> Result<()> {
        if self.phase != BatchPhase::Collecting || now_slot < self.collection_close {
            return Err(Error::OutsideWindow);
        }
        self.phase = BatchPhase::Selecting;
        Ok(())
    }

    /// Consider one fully verified candidate. Higher exact score wins; equal
    /// score uses lexicographically smaller content identity. This is the
    /// **best valid submitted candidate**, not an optimal clearing claim.
    pub fn consider_candidate<const N: usize>(
        &mut self,
        candidate: &mut CandidateStateV1<N>,
        now_slot: u64,
    ) -> Result<()> {
        if self.phase != BatchPhase::Selecting || now_slot >= self.selection_close {
            return Err(Error::OutsideWindow);
        }
        if candidate.phase != CandidatePhase::Valid
            || candidate.submission.batch_sequence != self.sequence
        {
            return Err(Error::CandidateClaimMismatch);
        }
        self.candidate_count = self
            .candidate_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let replace = match self.best_candidate_id {
            None => true,
            Some(best_id) => {
                candidate.score > self.best_score
                    || (candidate.score == self.best_score && candidate.candidate_id < best_id)
            }
        };
        if replace {
            self.best_candidate_id = Some(candidate.candidate_id);
            self.best_score = candidate.score;
        }
        candidate.phase = CandidatePhase::Considered;
        Ok(())
    }

    /// Freeze deterministic selection. An empty batch becomes quiescent.
    pub fn close_selection(&mut self, now_slot: u64) -> Result<Option<ContentId>> {
        if self.phase != BatchPhase::Selecting || now_slot < self.selection_close {
            return Err(Error::OutsideWindow);
        }
        match self.best_candidate_id {
            Some(id) => {
                self.phase = BatchPhase::Settling;
                Ok(Some(id))
            }
            None => {
                self.phase = BatchPhase::Quiescent;
                Ok(None)
            }
        }
    }

    /// Expire a winner only before Hoard conversion and receipt application
    /// begin. An applying settlement cannot time out; segregated liveness
    /// funding drives it to its committed conclusion.
    pub fn expire_unsettled(&mut self, now_slot: u64) -> Result<()> {
        if self.phase != BatchPhase::Settling || now_slot <= self.settlement_close {
            return Err(Error::OutsideWindow);
        }
        self.phase = BatchPhase::Quiescent;
        Ok(())
    }

    /// Retire after the adapter discharges every candidate and receipt child.
    pub fn retire(&mut self, children_discharged: bool) -> Result<()> {
        if self.phase != BatchPhase::Quiescent || !children_discharged {
            return Err(Error::NotQuiescent);
        }
        self.phase = BatchPhase::Retired;
        Ok(())
    }

    /// Return the batch phase.
    pub const fn phase(self) -> BatchPhase {
        self.phase
    }

    /// Return the immutable collection close.
    pub const fn collection_close(self) -> u64 {
        self.collection_close
    }

    /// Return the authenticated config commitment.
    pub const fn config_id(self) -> ContentId {
        self.config_id
    }

    /// Return the immutable batch sequence.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    /// Return the immutable candidate-selection close.
    pub const fn selection_close(self) -> u64 {
        self.selection_close
    }

    /// Return the immutable pre-application settlement close.
    pub const fn settlement_close(self) -> u64 {
        self.settlement_close
    }

    /// Return the selected candidate, if any.
    pub const fn best_candidate_id(self) -> Option<ContentId> {
        self.best_candidate_id
    }
}

/// Conserved collateral and equal per-outcome complete-set liability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HoardLedgerV1 {
    market_identity_id: ContentId,
    principal_atoms: u64,
    liability_units_per_outcome: u64,
}

impl HoardLedgerV1 {
    /// Construct a ledger only when principal exactly backs liabilities.
    pub fn new(
        market_identity_id: ContentId,
        principal_atoms: u64,
        liability_units_per_outcome: u64,
    ) -> Result<Self> {
        if principal_atoms != liability_units_per_outcome {
            return Err(Error::HoardInvariantViolation);
        }
        Ok(Self {
            market_identity_id,
            principal_atoms,
            liability_units_per_outcome,
        })
    }

    fn apply_complete_set_delta(&mut self, delta: i128) -> Result<()> {
        if self.principal_atoms != self.liability_units_per_outcome {
            return Err(Error::HoardInvariantViolation);
        }
        if delta >= 0 {
            let amount = u64::try_from(delta).map_err(|_| Error::TokenAmountOutOfRange)?;
            self.principal_atoms = self
                .principal_atoms
                .checked_add(amount)
                .ok_or(Error::ArithmeticOverflow)?;
            self.liability_units_per_outcome = self
                .liability_units_per_outcome
                .checked_add(amount)
                .ok_or(Error::ArithmeticOverflow)?;
        } else {
            let amount_i128 = delta.checked_neg().ok_or(Error::ArithmeticOverflow)?;
            let amount = u64::try_from(amount_i128).map_err(|_| Error::TokenAmountOutOfRange)?;
            self.principal_atoms = self
                .principal_atoms
                .checked_sub(amount)
                .ok_or(Error::InsufficientHoardPrincipal)?;
            self.liability_units_per_outcome = self
                .liability_units_per_outcome
                .checked_sub(amount)
                .ok_or(Error::InsufficientHoardPrincipal)?;
        }
        if self.principal_atoms != self.liability_units_per_outcome {
            return Err(Error::HoardInvariantViolation);
        }
        Ok(())
    }

    /// Return collateral principal; this value is never venue funding.
    pub const fn principal_atoms(self) -> u64 {
        self.principal_atoms
    }

    /// Return equal liabilities outstanding for every partition cell.
    pub const fn liability_units_per_outcome(self) -> u64 {
        self.liability_units_per_outcome
    }

    /// Return the Market identity whose Hoard this ledger projects.
    pub const fn market_identity_id(self) -> ContentId {
        self.market_identity_id
    }
}

/// Exact receipt for one atomic portfolio fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptV1<const N: usize> {
    /// Winning candidate commitment.
    pub candidate_id: ContentId,
    /// Signed order commitment.
    pub order_id: ContentId,
    /// Owner receiving the signed outcome and quote deltas.
    pub owner: ContentId,
    /// Market occurrence generation.
    pub generation: u64,
    /// Batch sequence.
    pub batch_sequence: u64,
    /// Replay nonce.
    pub nonce: u64,
    /// Filled scalar lots.
    pub fill_lots: u64,
    /// Remaining lots after this receipt is atomically applied.
    pub remaining_lots: u64,
    /// Signed integral settlement-asset delta.
    pub quote_delta_atoms: i64,
    /// Prefix carry before the one named rounding boundary.
    pub carry_before: u64,
    /// Prefix carry after the one named rounding boundary.
    pub carry_after: u64,
    /// Signed claim-token deltas in canonical ClaimBasis order.
    pub outcome_deltas: [i64; N],
    /// Exact ClaimBasis width, which must equal the selected const width.
    pub outcome_count: u16,
}

impl<const N: usize> SettlementReceiptV1<N> {
    /// Return the checked exact encoded receipt width for `N` deltas.
    pub fn encoded_len() -> Result<usize> {
        validate_width(N)?;
        SETTLEMENT_RECEIPT_BASE_BYTES
            .checked_add(N.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Encode into one exact-width adapter-consumable receipt buffer.
    #[allow(clippy::needless_borrow)]
    pub fn encode(&self, mut out: &mut [u8]) -> Result<()> {
        exact_len(out, Self::encoded_len()?)?;
        validate_portfolio(&self.outcome_deltas, self.outcome_count, false)?;
        out.fill(0);
        put(&mut out, 0, &RECEIPT_MAGIC);
        put(&mut out, 8, &SCHEMA_V1.to_le_bytes());
        put(&mut out, 10, &ARTIFACT_PROFILE_V1.to_le_bytes());
        put(&mut out, 12, &self.outcome_count.to_le_bytes());
        put(&mut out, 16, self.candidate_id.as_bytes());
        put(&mut out, 48, self.order_id.as_bytes());
        put(&mut out, 80, self.owner.as_bytes());
        put(&mut out, 112, &self.generation.to_le_bytes());
        put(&mut out, 120, &self.batch_sequence.to_le_bytes());
        put(&mut out, 128, &self.nonce.to_le_bytes());
        put(&mut out, 136, &self.fill_lots.to_le_bytes());
        put(&mut out, 144, &self.remaining_lots.to_le_bytes());
        put(&mut out, 152, &self.quote_delta_atoms.to_le_bytes());
        put(&mut out, 160, &self.carry_before.to_le_bytes());
        put(&mut out, 168, &self.carry_after.to_le_bytes());
        for (index, delta) in self.outcome_deltas.iter().enumerate() {
            if let Some(offset) = index
                .checked_mul(8)
                .and_then(|part| 176usize.checked_add(part))
            {
                put(&mut out, offset, &delta.to_le_bytes());
            }
        }
        Ok(())
    }

    /// Decode one exact receipt and reject wrong-width or width-substituted deltas.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        exact_len(bytes, Self::encoded_len()?)?;
        if array::<8>(bytes, 0)? != RECEIPT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != SCHEMA_V1 || read_u16(bytes, 10)? != ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 14, 2)?;
        let mut outcome_deltas = [0i64; N];
        for (index, target) in outcome_deltas.iter_mut().enumerate() {
            let offset = index
                .checked_mul(8)
                .and_then(|part| 176usize.checked_add(part))
                .ok_or(Error::ArithmeticOverflow)?;
            *target = read_i64(bytes, offset)?;
        }
        let outcome_count = read_u16(bytes, 12)?;
        validate_portfolio(&outcome_deltas, outcome_count, false)?;
        Ok(Self {
            candidate_id: read_id(bytes, 16)?,
            order_id: read_id(bytes, 48)?,
            owner: read_id(bytes, 80)?,
            generation: read_u64(bytes, 112)?,
            batch_sequence: read_u64(bytes, 120)?,
            nonce: read_u64(bytes, 128)?,
            fill_lots: read_u64(bytes, 136)?,
            remaining_lots: read_u64(bytes, 144)?,
            quote_delta_atoms: read_i64(bytes, 152)?,
            carry_before: read_u64(bytes, 160)?,
            carry_after: read_u64(bytes, 168)?,
            outcome_deltas,
            outcome_count,
        })
    }
}

/// Committed settlement replay cursor for the selected candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCursorV1<const N: usize> {
    candidate_id: ContentId,
    transcript_id: ContentId,
    settled_pages: u32,
    settled_executions: u32,
    last_order_id: Option<ContentId>,
    rounding_carry: u64,
    net_coefficients: [i128; N],
    total_quote_debit_numerator: i128,
    score: u128,
}

/// Fixed result envelope from one settlement page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementPageResultV1<const N: usize> {
    /// Number of leading results in both fixed arrays.
    pub execution_count: u8,
    /// Mutated replay states which the adapter must persist atomically.
    pub order_states: [Option<OrderStateV1>; MAX_EXECUTIONS_PER_PAGE_V1],
    /// Exact token-transfer receipts which the adapter must consume atomically.
    pub receipts: [Option<SettlementReceiptV1<N>>; MAX_EXECUTIONS_PER_PAGE_V1],
}

impl<const N: usize> SettlementCursorV1<N> {
    /// Begin replay of exactly the selected verified candidate.
    ///
    /// The adapter proves referenced order custody remains locked, then moves
    /// the exact complete-set collateral between that custody and Hoard in the
    /// same atomic call. Once applying begins, settlement cannot expire or
    /// select another result.
    pub fn begin(
        candidate: CandidateStateV1<N>,
        batch: &mut BatchRootV1,
        hoard: &mut HoardLedgerV1,
        config: GeneralConfigV1,
        now_slot: u64,
    ) -> Result<Self> {
        if candidate.phase != CandidatePhase::Considered
            || batch.phase != BatchPhase::Settling
            || batch.best_candidate_id != Some(candidate.candidate_id)
        {
            return Err(Error::CandidateNotSelected);
        }
        if now_slot > batch.settlement_close {
            return Err(Error::OutsideWindow);
        }
        if hoard.market_identity_id != config.market_identity_id {
            return Err(Error::AuthorityMismatch);
        }
        let mut next_hoard = *hoard;
        next_hoard.apply_complete_set_delta(candidate.complete_set_delta)?;
        let mut next_batch = *batch;
        next_batch.phase = BatchPhase::Applying;
        let cursor = Self {
            candidate_id: candidate.candidate_id,
            transcript_id: candidate.submission.initial_transcript_id,
            settled_pages: 0,
            settled_executions: 0,
            last_order_id: None,
            rounding_carry: 0,
            net_coefficients: [0; N],
            total_quote_debit_numerator: 0,
            score: 0,
        };
        *hoard = next_hoard;
        *batch = next_batch;
        Ok(cursor)
    }

    /// Replay one exact verified page and return adapter-consumable states and
    /// receipts. The adapter must authenticate the transcript hash and commit
    /// all returned mutations atomically with this cursor.
    pub fn settle_page(
        &mut self,
        page: VerificationPageV1<N>,
        candidate: CandidateStateV1<N>,
        config: GeneralConfigV1,
        batch: BatchRootV1,
    ) -> Result<SettlementPageResultV1<N>> {
        if candidate.candidate_id != self.candidate_id || batch.phase != BatchPhase::Applying {
            return Err(Error::CandidateNotSelected);
        }
        if page.page_index != self.settled_pages || page.prior_transcript_id != self.transcript_id {
            return Err(Error::CursorMismatch);
        }
        if page.next_transcript_id == page.prior_transcript_id {
            return Err(Error::InvalidTranscriptStep);
        }
        let count = usize::from(page.execution_count);
        if count == 0 || count > MAX_EXECUTIONS_PER_PAGE_V1 {
            return Err(Error::InvalidPageCount);
        }
        if page.executions.iter().take(count).any(Option::is_none)
            || page.executions.iter().skip(count).any(Option::is_some)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let mut next = *self;
        let mut states = [None; MAX_EXECUTIONS_PER_PAGE_V1];
        let mut receipts = [None; MAX_EXECUTIONS_PER_PAGE_V1];
        for (index, execution) in page.executions.iter().take(count).flatten().enumerate() {
            let (state, receipt) = next.settle_execution(*execution, candidate, config, batch)?;
            let state_target = states.get_mut(index).ok_or(Error::InvalidLength)?;
            *state_target = Some(state);
            let receipt_target = receipts.get_mut(index).ok_or(Error::InvalidLength)?;
            *receipt_target = Some(receipt);
        }
        next.settled_pages = next
            .settled_pages
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.transcript_id = page.next_transcript_id;
        *self = next;
        Ok(SettlementPageResultV1 {
            execution_count: page.execution_count,
            order_states: states,
            receipts,
        })
    }

    fn settle_execution(
        &mut self,
        execution: ExecutionV1<N>,
        candidate: CandidateStateV1<N>,
        config: GeneralConfigV1,
        batch: BatchRootV1,
    ) -> Result<(OrderStateV1, SettlementReceiptV1<N>)> {
        validate_execution_binding(execution, config, batch)?;
        if self
            .last_order_id
            .is_some_and(|last| execution.order.order_id <= last)
        {
            return Err(Error::NonCanonicalOrder);
        }
        let debit_per_lot = portfolio_dot(
            &execution.order.coefficients,
            &candidate.submission.prices,
            config.outcome_count,
        )?;
        if debit_per_lot > execution.order.max_quote_debit_per_lot_numerator {
            return Err(Error::LimitViolated);
        }
        let fill = i128::from(execution.fill_lots);
        let debit = debit_per_lot
            .checked_mul(fill)
            .ok_or(Error::ArithmeticOverflow)?;
        let quote_delta_numerator = debit.checked_neg().ok_or(Error::ArithmeticOverflow)?;
        let carry_before = self.rounding_carry;
        let combined = quote_delta_numerator
            .checked_add(i128::from(carry_before))
            .ok_or(Error::ArithmeticOverflow)?;
        let scale = i128::from(config.price_scale);
        let quote_delta_i128 = combined.div_euclid(scale);
        let carry_after_i128 = combined.rem_euclid(scale);
        let quote_delta_atoms =
            i64::try_from(quote_delta_i128).map_err(|_| Error::TokenAmountOutOfRange)?;
        let carry_after = u64::try_from(carry_after_i128).map_err(|_| Error::ArithmeticOverflow)?;
        let mut outcome_deltas = [0i64; N];
        let width = usize::from(config.outcome_count);
        for ((receipt_delta, net), coefficient) in outcome_deltas
            .iter_mut()
            .zip(self.net_coefficients.iter_mut())
            .zip(execution.order.coefficients.iter())
            .take(width)
        {
            let delta_i128 = i128::from(*coefficient)
                .checked_mul(fill)
                .ok_or(Error::ArithmeticOverflow)?;
            *receipt_delta = i64::try_from(delta_i128).map_err(|_| Error::TokenAmountOutOfRange)?;
            *net = net
                .checked_add(delta_i128)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        let preference = execution
            .order
            .max_quote_debit_per_lot_numerator
            .checked_sub(debit_per_lot)
            .and_then(|per_lot| per_lot.checked_mul(fill))
            .ok_or(Error::ArithmeticOverflow)?;
        self.score = self
            .score
            .checked_add(u128::try_from(preference).map_err(|_| Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)?;
        self.total_quote_debit_numerator = self
            .total_quote_debit_numerator
            .checked_add(debit)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut state = execution.order_state;
        state.consume(execution.order, execution.fill_lots)?;
        self.rounding_carry = carry_after;
        self.settled_executions = self
            .settled_executions
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        self.last_order_id = Some(execution.order.order_id);
        Ok((
            state,
            SettlementReceiptV1 {
                candidate_id: candidate.candidate_id,
                order_id: execution.order.order_id,
                owner: execution.order.owner,
                generation: config.generation,
                batch_sequence: batch.sequence,
                nonce: execution.order.nonce,
                fill_lots: execution.fill_lots,
                remaining_lots: state.remaining_lots,
                quote_delta_atoms,
                carry_before,
                carry_after,
                outcome_deltas,
                outcome_count: config.outcome_count,
            },
        ))
    }

    /// Finish exact replay and make the already-capitalized batch quiescent.
    /// Each page is atomic with its cursor, replay states, and custody transfer;
    /// prepaid liveness drives a started application to completion.
    pub fn finish(&self, candidate: CandidateStateV1<N>, batch: &mut BatchRootV1) -> Result<()> {
        if batch.phase != BatchPhase::Applying
            || batch.best_candidate_id != Some(self.candidate_id)
            || candidate.candidate_id != self.candidate_id
        {
            return Err(Error::CandidateNotSelected);
        }
        if self.settled_pages != candidate.verified_pages
            || self.settled_executions != candidate.verified_executions
            || self.transcript_id != candidate.transcript_id
            || self.net_coefficients != candidate.net_coefficients
            || self.total_quote_debit_numerator != candidate.total_quote_debit_numerator
            || self.score != candidate.score
        {
            return Err(Error::CandidateClaimMismatch);
        }
        if self.rounding_carry != 0 {
            return Err(Error::RoundingCarryOutstanding);
        }
        let mut next_batch = *batch;
        next_batch.phase = BatchPhase::Quiescent;
        *batch = next_batch;
        Ok(())
    }
}

/// Segregated prepaid General work compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FundingCompartment {
    /// Progress capital for expiry and convergence calls.
    Liveness = 0,
    /// Metered page verification and settlement work.
    Work = 1,
    /// Fixed successful-candidate or retirement bounties.
    Bounty = 2,
}

/// Immutable quote and mutable conservation state for General funding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralFundingV1 {
    capability_release_id: ContentId,
    committed: [u64; 3],
    remaining: [u64; 3],
    spent: [u64; 3],
    refunded: [u64; 3],
}

/// Auditable funding debit emitted to the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingDebitV1 {
    /// Segregated source compartment.
    pub compartment: FundingCompartment,
    /// Exact prepaid principal consumed.
    pub amount: u64,
    /// Recipient identity authenticated by the adapter.
    pub recipient: ContentId,
}

impl GeneralFundingV1 {
    /// Found segregated prepaid funding. Hoard and future fees are not inputs.
    pub const fn founding(
        capability_release_id: ContentId,
        liveness: u64,
        work: u64,
        bounty: u64,
    ) -> Self {
        Self {
            capability_release_id,
            committed: [liveness, work, bounty],
            remaining: [liveness, work, bounty],
            spent: [0; 3],
            refunded: [0; 3],
        }
    }

    /// Consume present principal from exactly one compartment.
    pub fn debit(
        &mut self,
        compartment: FundingCompartment,
        amount: u64,
        recipient: ContentId,
    ) -> Result<FundingDebitV1> {
        if amount == 0 {
            return Err(Error::ZeroFundingDebit);
        }
        let index = funding_index(compartment);
        let remaining = *self.remaining.get(index).ok_or(Error::InvalidLength)?;
        let next_remaining = remaining
            .checked_sub(amount)
            .ok_or(Error::InsufficientFunding)?;
        let spent = *self.spent.get(index).ok_or(Error::InvalidLength)?;
        let next_spent = spent.checked_add(amount).ok_or(Error::ArithmeticOverflow)?;
        let target_remaining = self.remaining.get_mut(index).ok_or(Error::InvalidLength)?;
        *target_remaining = next_remaining;
        let target_spent = self.spent.get_mut(index).ok_or(Error::InvalidLength)?;
        *target_spent = next_spent;
        self.validate()?;
        Ok(FundingDebitV1 {
            compartment,
            amount,
            recipient,
        })
    }

    /// Refund all unspent compartments only after General is terminal.
    pub fn refund_terminal(&mut self, phase: GeneralPhase) -> Result<[u64; 3]> {
        if phase != GeneralPhase::Terminal {
            return Err(Error::NotQuiescent);
        }
        let refund = self.remaining;
        for index in 0..3 {
            let value = *refund.get(index).ok_or(Error::InvalidLength)?;
            let remaining = self.remaining.get_mut(index).ok_or(Error::InvalidLength)?;
            *remaining = 0;
            let refunded = self.refunded.get_mut(index).ok_or(Error::InvalidLength)?;
            *refunded = refunded
                .checked_add(value)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        self.validate()?;
        Ok(refund)
    }

    /// Prove immutable compartment conservation.
    pub fn validate(self) -> Result<()> {
        for index in 0..3 {
            let remaining = *self.remaining.get(index).ok_or(Error::InvalidLength)?;
            let spent = *self.spent.get(index).ok_or(Error::InvalidLength)?;
            let refunded = *self.refunded.get(index).ok_or(Error::InvalidLength)?;
            let observed = remaining
                .checked_add(spent)
                .and_then(|value| value.checked_add(refunded))
                .ok_or(Error::ArithmeticOverflow)?;
            if observed != *self.committed.get(index).ok_or(Error::InvalidLength)? {
                return Err(Error::FundingConservationMismatch);
            }
        }
        Ok(())
    }

    /// Return true once all prepaid principal has been spent or refunded.
    pub fn is_discharged(self) -> bool {
        self.remaining.iter().all(|amount| *amount == 0)
    }

    /// Return the capability release whose manifest quote funded this state.
    pub const fn capability_release_id(self) -> ContentId {
        self.capability_release_id
    }

    /// Return exact remaining prepaid principal in one compartment.
    pub fn remaining(self, compartment: FundingCompartment) -> Result<u64> {
        self.remaining
            .get(funding_index(compartment))
            .copied()
            .ok_or(Error::InvalidLength)
    }
}

fn validate_execution_binding<const N: usize>(
    execution: ExecutionV1<N>,
    config: GeneralConfigV1,
    batch: BatchRootV1,
) -> Result<()> {
    validate_authority(
        execution.order.market_identity_id,
        execution.order.claim_basis_id,
        execution.order.generation,
        config,
    )?;
    if execution.order.batch_sequence != batch.sequence {
        return Err(Error::OrderBindingMismatch);
    }
    if execution.order.valid_until_slot < batch.settlement_close {
        return Err(Error::OrderExpired);
    }
    execution
        .order_state
        .validate_snapshot(execution.order, execution.fill_lots)
}

fn validate_authority(
    market_identity_id: ContentId,
    claim_basis_id: ContentId,
    generation: u64,
    config: GeneralConfigV1,
) -> Result<()> {
    if market_identity_id != config.market_identity_id
        || claim_basis_id != config.claim_basis_id
        || generation != config.generation
    {
        return Err(Error::AuthorityMismatch);
    }
    Ok(())
}

fn validate_prices<const N: usize>(
    prices: &[u64; N],
    outcome_count: u16,
    price_scale: u64,
) -> Result<()> {
    let width = usize::from(outcome_count);
    if width != N || !(2..=MAX_OUTCOMES_V1).contains(&width) {
        return Err(Error::InvalidOutcomeCount);
    }
    let mut sum = 0u64;
    for price in prices {
        sum = sum.checked_add(*price).ok_or(Error::ArithmeticOverflow)?;
    }
    if sum != price_scale {
        return Err(Error::InvalidSimplexPrice);
    }
    Ok(())
}

fn validate_portfolio<const N: usize>(
    coefficients: &[i64; N],
    outcome_count: u16,
    require_nonzero: bool,
) -> Result<()> {
    let width = usize::from(outcome_count);
    if width != N || !(2..=MAX_OUTCOMES_V1).contains(&width) {
        return Err(Error::InvalidOutcomeCount);
    }
    if require_nonzero && coefficients.iter().all(|coefficient| *coefficient == 0) {
        return Err(Error::EmptyPortfolio);
    }
    Ok(())
}

fn portfolio_dot<const N: usize>(
    coefficients: &[i64; N],
    prices: &[u64; N],
    outcome_count: u16,
) -> Result<i128> {
    if usize::from(outcome_count) != N {
        return Err(Error::InvalidOutcomeCount);
    }
    let mut total = 0i128;
    for (coefficient, price) in coefficients.iter().zip(prices.iter()) {
        let term = i128::from(*coefficient)
            .checked_mul(i128::from(*price))
            .ok_or(Error::ArithmeticOverflow)?;
        total = total.checked_add(term).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(total)
}

fn validate_width(width: usize) -> Result<()> {
    if !(2..=MAX_OUTCOMES_V1).contains(&width) {
        return Err(Error::InvalidOutcomeCount);
    }
    Ok(())
}

const fn funding_index(compartment: FundingCompartment) -> usize {
    match compartment {
        FundingCompartment::Liveness => 0,
        FundingCompartment::Work => 1,
        FundingCompartment::Bounty => 2,
    }
}

fn exact_len(bytes: &[u8], expected: usize) -> Result<()> {
    if bytes.len() != expected {
        return Err(Error::InvalidLength);
    }
    Ok(())
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(array(bytes, offset)?))
}

fn read_i128(bytes: &[u8], offset: usize) -> Result<i128> {
    Ok(i128::from_le_bytes(array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests;
