//! Borrowed, runtime-profile execution over canonical Dealer account bytes.
//!
//! This is the sole executable transition surface for an SBF adapter.  The
//! caller supplies bounded scratch storage, so the kernel remains
//! `no_std`/`no_alloc` and no maximum-width config, Pool, or fill matrix is
//! copied onto the VM stack.  Mutations are committed only after every
//! fallible precondition and conservation equation has been checked.

use dclutch_core_contract::ContentId;

use crate::{
    BASIS_POINTS_DENOMINATOR, CONFIG_BID_PRICE_OFFSET, CONFIG_FEE_BPS_OFFSET, CONFIG_MAGIC,
    CONFIG_MAX_QUANTITY_OFFSET, CONFIG_OWNER_OFFSET, CONFIG_PRICE_SCALE_OFFSET,
    CONFIG_RESERVED_BYTES, CONFIG_RESERVED_OFFSET, CONFIG_RESET_INTERVAL_OFFSET, Error,
    LIQUIDITY_ATTACHMENT_BYTES, LP_POSITION_BYTES, LiquidityAmounts, LiquidityAttachment,
    LiquidityChangeKind, LiquidityChangeReceipt, LpPosition, MAX_NATIVE_CLAIMS, MAX_QUOTE_BINS,
    MIN_NATIVE_CLAIMS, MIN_QUOTE_BINS, PARENT_POOL_BYTES, POOL_MAGIC, POSITION_MAGIC,
    POSITION_OWNER_OFFSET, POSITION_PARENT_OFFSET, POSITION_RENT_OFFSET, POSITION_RESERVED_BYTES,
    POSITION_RESERVED_OFFSET, POSITION_SEQUENCE_OFFSET, POSITION_SHARES_OFFSET,
    POSITION_STATUS_OFFSET, ParentPool, PoolRetirementReceipt, PoolStatus, PositionCloseReceipt,
    PositionCreationReceipt, PositionStatus, RENT_CREDIT_TERMS_BYTES, RentCreditTerms, Result,
    STATE_ATTACHMENT_OFFSET, STATE_CLAIMS_OFFSET, STATE_FEES_OFFSET, STATE_LIVE_POSITIONS_OFFSET,
    STATE_NEXT_RESET_SLOT_OFFSET, STATE_PRINCIPAL_OFFSET, STATE_RENT_OFFSET, STATE_RESERVED_BYTES,
    STATE_RESERVED_OFFSET, STATE_RESET_OFFSET, STATE_SEQUENCE_OFFSET, STATE_SERVICE_OFFSET,
    STATE_STATUS_OFFSET, STATE_TOTAL_SHARES_OFFSET, TradeRequest, TradeSide, all_zero, checked_add,
    checked_offset, checked_sub, decode_header, encode_header, mul_div_ceil, mul_div_floor,
    parent_for, put, put_u64, read_array, read_u8, read_u16, read_u64, require_amounts_at_least,
    require_amounts_at_most, require_zero, subslice, validate_position_identity,
};

/// Exact runtime claim/bin geometry authenticated from Market and config width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityProfileV1 {
    outcomes: usize,
    bins: usize,
}

impl LiquidityProfileV1 {
    /// Construct one supported exact profile.
    pub fn new(outcomes: usize, bins: usize) -> Result<Self> {
        if !(MIN_NATIVE_CLAIMS..=MAX_NATIVE_CLAIMS).contains(&outcomes)
            || !(MIN_QUOTE_BINS..=MAX_QUOTE_BINS).contains(&bins)
        {
            return Err(Error::UnsupportedProfile);
        }
        Ok(Self { outcomes, bins })
    }

    /// Infer bin count from an authenticated Market outcome count and config width.
    pub fn from_config_len(outcomes: usize, config_len: usize) -> Result<Self> {
        if !(MIN_NATIVE_CLAIMS..=MAX_NATIVE_CLAIMS).contains(&outcomes)
            || config_len < CONFIG_BID_PRICE_OFFSET
        {
            return Err(Error::UnsupportedProfile);
        }
        let payload = config_len
            .checked_sub(CONFIG_BID_PRICE_OFFSET)
            .ok_or(Error::InvalidLength)?;
        let divisor = 32usize
            .checked_mul(outcomes)
            .ok_or(Error::ArithmeticOverflow)?;
        if divisor == 0 || payload % divisor != 0 {
            return Err(Error::InvalidLength);
        }
        let profile = Self::new(outcomes, payload / divisor)?;
        if profile.config_len()? != config_len {
            return Err(Error::InvalidLength);
        }
        Ok(profile)
    }

    /// Return exact outcome count.
    pub const fn outcomes(self) -> usize {
        self.outcomes
    }

    /// Return exact bins per outcome and side.
    pub const fn bins(self) -> usize {
        self.bins
    }

    /// Return canonical config width.
    pub fn config_len(self) -> Result<usize> {
        CONFIG_BID_PRICE_OFFSET
            .checked_add(
                32usize
                    .checked_mul(self.cells()?)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Return canonical Pool width.
    pub fn pool_len(self) -> Result<usize> {
        let fills = 16usize
            .checked_mul(self.cells()?)
            .ok_or(Error::ArithmeticOverflow)?;
        STATE_CLAIMS_OFFSET
            .checked_add(
                8usize
                    .checked_mul(self.outcomes)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .and_then(|value| value.checked_add(fills))
            .ok_or(Error::ArithmeticOverflow)
    }

    fn cells(self) -> Result<usize> {
        self.outcomes
            .checked_mul(self.bins)
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Borrowed immutable config view with runtime geometry and no large value copy.
#[derive(Clone, Copy)]
pub struct LiquidityConfigViewV1<'a> {
    content_id: ContentId,
    profile: LiquidityProfileV1,
    bytes: &'a [u8],
}

impl<'a> LiquidityConfigViewV1<'a> {
    /// Authenticate canonical config encoding and every economic ladder invariant.
    pub fn new(
        content_id: ContentId,
        profile: LiquidityProfileV1,
        bytes: &'a [u8],
    ) -> Result<Self> {
        if bytes.len() != profile.config_len()? {
            return Err(Error::InvalidLength);
        }
        decode_header(bytes, CONFIG_MAGIC)?;
        require_zero(bytes, CONFIG_RESERVED_OFFSET, CONFIG_RESERVED_BYTES)?;
        let view = Self {
            content_id,
            profile,
            bytes,
        };
        view.validate()?;
        Ok(view)
    }

    /// Return immutable content identity authenticated by the adapter.
    pub const fn content_id(self) -> ContentId {
        self.content_id
    }

    /// Return exact runtime geometry.
    pub const fn profile(self) -> LiquidityProfileV1 {
        self.profile
    }

    /// Return immutable bootstrap LP and service-refund owner.
    pub fn liquidity_owner(self) -> Result<[u8; 32]> {
        read_array(self.bytes, CONFIG_OWNER_OFFSET)
    }

    /// Return price integer scale.
    pub fn price_scale(self) -> Result<u64> {
        read_u64(self.bytes, CONFIG_PRICE_SCALE_OFFSET)
    }

    /// Return trader-paid fee in basis points.
    pub fn fee_bps(self) -> Result<u16> {
        read_u16(self.bytes, CONFIG_FEE_BPS_OFFSET)
    }

    /// Return maximum quantity per atomic trade.
    pub fn max_trade_quantity(self) -> Result<u64> {
        read_u64(self.bytes, CONFIG_MAX_QUANTITY_OFFSET)
    }

    /// Return immutable reset cadence.
    pub fn reset_interval_slots(self) -> Result<u64> {
        read_u64(self.bytes, CONFIG_RESET_INTERVAL_OFFSET)
    }

    /// Return one exact price.
    pub fn price(self, side: TradeSide, claim: usize, bin: usize) -> Result<u64> {
        let base = match side {
            TradeSide::SellClaimToPool => CONFIG_BID_PRICE_OFFSET,
            TradeSide::BuyClaimFromPool => self.section_offset(1)?,
        };
        self.cell(base, claim, bin)
    }

    /// Return one exact per-window capacity.
    pub fn capacity(self, side: TradeSide, claim: usize, bin: usize) -> Result<u64> {
        let section = match side {
            TradeSide::SellClaimToPool => 2,
            TradeSide::BuyClaimFromPool => 3,
        };
        self.cell(self.section_offset(section)?, claim, bin)
    }

    fn validate(self) -> Result<()> {
        let owner = self.liquidity_owner()?;
        let scale = self.price_scale()?;
        let fee = self.fee_bps()?;
        if all_zero(&owner) {
            return Err(Error::ZeroIdentity);
        }
        if scale == 0 || self.max_trade_quantity()? == 0 {
            return Err(Error::InvalidPrice);
        }
        if fee == 0 || u64::from(fee) > BASIS_POINTS_DENOMINATOR {
            return Err(Error::InvalidFeeRate);
        }
        if self.reset_interval_slots()? == 0 {
            return Err(Error::InvalidResetInterval);
        }
        let mut best_bid_sum = 0u64;
        let mut best_ask_sum = 0u64;
        for claim in 0..self.profile.outcomes {
            let mut previous_bid = 0u64;
            let mut previous_ask = 0u64;
            for bin in 0..self.profile.bins {
                let bid = self.price(TradeSide::SellClaimToPool, claim, bin)?;
                let ask = self.price(TradeSide::BuyClaimFromPool, claim, bin)?;
                let bid_capacity = self.capacity(TradeSide::SellClaimToPool, claim, bin)?;
                let ask_capacity = self.capacity(TradeSide::BuyClaimFromPool, claim, bin)?;
                if bid == 0 || ask == 0 || bid > scale || ask > scale {
                    return Err(Error::InvalidPrice);
                }
                if bid_capacity == 0 || ask_capacity == 0 {
                    return Err(Error::EmptyBin);
                }
                if bid >= ask || (bin > 0 && (bid >= previous_bid || ask <= previous_ask)) {
                    return Err(Error::InvalidLadder);
                }
                if bin == 0 {
                    best_bid_sum = checked_add(best_bid_sum, bid)?;
                    best_ask_sum = checked_add(best_ask_sum, ask)?;
                }
                previous_bid = bid;
                previous_ask = ask;
            }
        }
        if best_bid_sum > scale || best_ask_sum < scale {
            return Err(Error::CompleteSetArbitrage);
        }
        Ok(())
    }

    fn section_offset(self, section: usize) -> Result<usize> {
        checked_offset(
            CONFIG_BID_PRICE_OFFSET,
            8,
            self.profile
                .cells()?
                .checked_mul(section)
                .ok_or(Error::ArithmeticOverflow)?,
        )
    }

    fn cell(self, base: usize, claim: usize, bin: usize) -> Result<u64> {
        if claim >= self.profile.outcomes || bin >= self.profile.bins {
            return Err(Error::ClaimIndexOutOfRange);
        }
        let index = claim
            .checked_mul(self.profile.bins)
            .and_then(|value| value.checked_add(bin))
            .ok_or(Error::ArithmeticOverflow)?;
        read_u64(self.bytes, checked_offset(base, 8, index)?)
    }
}

/// Borrowed authenticated Pool state; all large vectors remain in account bytes.
#[derive(Clone, Copy)]
pub struct PoolViewV1<'a> {
    profile: LiquidityProfileV1,
    bytes: &'a [u8],
}

impl<'a> PoolViewV1<'a> {
    /// Authenticate canonical Pool bytes against address and immutable config.
    pub fn new(
        profile: LiquidityProfileV1,
        bytes: &'a [u8],
        pool_address: [u8; 32],
        config: LiquidityConfigViewV1<'_>,
    ) -> Result<Self> {
        if profile != config.profile() || bytes.len() != profile.pool_len()? {
            return Err(Error::InvalidLength);
        }
        decode_header(bytes, POOL_MAGIC)?;
        require_zero(bytes, STATE_RESERVED_OFFSET, STATE_RESERVED_BYTES)?;
        let view = Self { profile, bytes };
        view.validate(pool_address, config)?;
        Ok(view)
    }

    /// Return sole full immutable attachment.
    pub fn attachment(self) -> Result<LiquidityAttachment> {
        LiquidityAttachment::decode(subslice(
            self.bytes,
            STATE_ATTACHMENT_OFFSET,
            LIQUIDITY_ATTACHMENT_BYTES,
        )?)
    }

    /// Return Pool rent attribution.
    pub fn rent_credit(self) -> Result<RentCreditTerms> {
        RentCreditTerms::decode(subslice(
            self.bytes,
            STATE_RENT_OFFSET,
            RENT_CREDIT_TERMS_BYTES,
        )?)
    }

    /// Return current reset number.
    pub fn reset_number(self) -> Result<u64> {
        read_u64(self.bytes, STATE_RESET_OFFSET)
    }

    /// Return next global replay sequence.
    pub fn next_sequence(self) -> Result<u64> {
        read_u64(self.bytes, STATE_SEQUENCE_OFFSET)
    }

    /// Return earliest next reset slot.
    pub fn next_reset_slot(self) -> Result<u64> {
        read_u64(self.bytes, STATE_NEXT_RESET_SLOT_OFFSET)
    }

    /// Return lifecycle status.
    pub fn status(self) -> Result<PoolStatus> {
        PoolStatus::decode(read_u8(self.bytes, STATE_STATUS_OFFSET)?)
    }

    /// Return live LP-position count.
    pub fn live_positions(self) -> Result<u64> {
        read_u64(self.bytes, STATE_LIVE_POSITIONS_OFFSET)
    }

    /// Return total LP shares.
    pub fn total_shares(self) -> Result<u64> {
        read_u64(self.bytes, STATE_TOTAL_SHARES_OFFSET)
    }

    /// Return LP principal collateral.
    pub fn principal_collateral(self) -> Result<u64> {
        read_u64(self.bytes, STATE_PRINCIPAL_OFFSET)
    }

    /// Return realized trader-paid LP fee collateral.
    pub fn realized_fee_collateral(self) -> Result<u64> {
        read_u64(self.bytes, STATE_FEES_OFFSET)
    }

    /// Return segregated service funding.
    pub fn service_funding(self) -> Result<u64> {
        read_u64(self.bytes, STATE_SERVICE_OFFSET)
    }

    /// Return one native-claim reserve.
    pub fn claim_reserve(self, claim: usize) -> Result<u64> {
        if claim >= self.profile.outcomes {
            return Err(Error::ClaimIndexOutOfRange);
        }
        read_u64(self.bytes, checked_offset(STATE_CLAIMS_OFFSET, 8, claim)?)
    }

    /// Copy the bounded exact-N LP vector, never a bin matrix or full Pool.
    pub fn liquidity<const N: usize>(self) -> Result<LiquidityAmounts<N>> {
        if N != self.profile.outcomes {
            return Err(Error::UnsupportedProfile);
        }
        let mut claims = [0u64; N];
        for (index, value) in claims.iter_mut().enumerate() {
            *value = self.claim_reserve(index)?;
        }
        LiquidityAmounts::new(
            self.principal_collateral()?,
            self.realized_fee_collateral()?,
            claims,
        )
    }

    /// Return one selected-side fill counter.
    pub fn fill(self, side: TradeSide, claim: usize, bin: usize) -> Result<u64> {
        if claim >= self.profile.outcomes || bin >= self.profile.bins {
            return Err(Error::ClaimIndexOutOfRange);
        }
        let section = match side {
            TradeSide::SellClaimToPool => 0,
            TradeSide::BuyClaimFromPool => 1,
        };
        let base = self.fill_section_offset(section)?;
        let index = claim
            .checked_mul(self.profile.bins)
            .and_then(|value| value.checked_add(bin))
            .ok_or(Error::ArithmeticOverflow)?;
        read_u64(self.bytes, checked_offset(base, 8, index)?)
    }

    fn validate(self, pool_address: [u8; 32], config: LiquidityConfigViewV1<'_>) -> Result<()> {
        let attachment = self.attachment()?;
        parent_for(attachment, pool_address)?;
        if attachment.liquidity_config_id() != config.content_id() {
            return Err(Error::ConfigurationMismatch);
        }
        self.rent_credit()?;
        if self.next_sequence()? == 0 || self.next_reset_slot()? == 0 {
            return Err(Error::SequenceMismatch);
        }
        let total_shares = self.total_shares()?;
        let live_positions = self.live_positions()?;
        match self.status()? {
            PoolStatus::Active if total_shares == 0 || live_positions == 0 => {
                return Err(Error::ShareInvariant);
            }
            PoolStatus::Retiring => {
                if total_shares != 0 || !self.liquidity_is_zero()? {
                    return Err(Error::ShareInvariant);
                }
            }
            PoolStatus::Retired => {
                if total_shares != 0
                    || live_positions != 0
                    || self.service_funding()? != 0
                    || !self.liquidity_is_zero()?
                {
                    return Err(Error::PoolNotQuiescent);
                }
            }
            PoolStatus::Active => {}
        }
        for claim in 0..self.profile.outcomes {
            for bin in 0..self.profile.bins {
                for side in [TradeSide::SellClaimToPool, TradeSide::BuyClaimFromPool] {
                    if self.fill(side, claim, bin)? > config.capacity(side, claim, bin)? {
                        return Err(Error::ConservationMismatch);
                    }
                }
            }
        }
        Ok(())
    }

    fn require_active(self) -> Result<()> {
        if self.status()? != PoolStatus::Active {
            return Err(Error::InvalidPoolStatus);
        }
        Ok(())
    }

    fn require_sequence(self, expected: u64) -> Result<()> {
        if self.next_sequence()? != expected {
            return Err(Error::SequenceMismatch);
        }
        Ok(())
    }

    fn liquidity_is_zero(self) -> Result<bool> {
        if self.principal_collateral()? != 0 || self.realized_fee_collateral()? != 0 {
            return Ok(false);
        }
        for claim in 0..self.profile.outcomes {
            if self.claim_reserve(claim)? != 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn fill_section_offset(self, section: usize) -> Result<usize> {
        let claims_end = checked_offset(STATE_CLAIMS_OFFSET, 8, self.profile.outcomes)?;
        checked_offset(
            claims_end,
            8,
            self.profile
                .cells()?
                .checked_mul(section)
                .ok_or(Error::ArithmeticOverflow)?,
        )
    }
}

/// Compact reset evidence; fill matrices remain observable in pre-state bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LadderResetReceiptV1 {
    parent: ParentPool,
    pool_sequence: u64,
    old_reset_number: u64,
    new_reset_number: u64,
    observed_slot: u64,
    next_reset_slot: u64,
}

impl LadderResetReceiptV1 {
    /// Return accepted global sequence.
    pub const fn pool_sequence(self) -> u64 {
        self.pool_sequence
    }
    /// Return prior reset number.
    pub const fn old_reset_number(self) -> u64 {
        self.old_reset_number
    }
    /// Return new reset number.
    pub const fn new_reset_number(self) -> u64 {
        self.new_reset_number
    }
    /// Return authenticated reset slot.
    pub const fn observed_slot(self) -> u64 {
        self.observed_slot
    }
    /// Return next reset boundary.
    pub const fn next_reset_slot(self) -> u64 {
        self.next_reset_slot
    }
    /// Return compact parent Pool.
    pub const fn parent(self) -> ParentPool {
        self.parent
    }
}

/// Fixed-small transient execution delta with an exact runtime bin prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionReceiptV1 {
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
    bin_count: u8,
    bin_before: [u64; MAX_QUOTE_BINS],
    bin_after: [u64; MAX_QUOTE_BINS],
}

impl ExecutionReceiptV1 {
    /// Return exact transient encoded width, identical to the selected V1 B ABI.
    pub fn encoded_len(self) -> Result<usize> {
        crate::EXECUTION_BIN_BEFORE_OFFSET
            .checked_add(
                16usize
                    .checked_mul(usize::from(self.bin_count))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)
    }
    /// Encode only the authenticated runtime bin prefix.
    pub fn encode_into(self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() != self.encoded_len()? {
            return Err(Error::InvalidLength);
        }
        out.fill(0);
        encode_header(out, crate::EXECUTION_MAGIC);
        put(out, crate::EXECUTION_PARENT_OFFSET, &self.parent.to_bytes());
        put_u64(out, crate::EXECUTION_RESET_OFFSET, self.reset_number);
        put_u64(out, crate::EXECUTION_SEQUENCE_OFFSET, self.sequence);
        put(out, crate::EXECUTION_SIDE_OFFSET, &[self.side.byte()]);
        put(out, crate::EXECUTION_CLAIM_OFFSET, &[self.claim_index]);
        put_u64(out, crate::EXECUTION_QUANTITY_OFFSET, self.quantity);
        put_u64(
            out,
            crate::EXECUTION_NOTIONAL_OFFSET,
            self.notional_collateral,
        );
        put_u64(out, crate::EXECUTION_FEE_OFFSET, self.trader_fee_collateral);
        put_u64(
            out,
            crate::EXECUTION_TRADER_COLLATERAL_DEBIT_OFFSET,
            self.trader_collateral_debit,
        );
        put_u64(
            out,
            crate::EXECUTION_TRADER_COLLATERAL_CREDIT_OFFSET,
            self.trader_collateral_credit,
        );
        put_u64(
            out,
            crate::EXECUTION_TRADER_CLAIM_DEBIT_OFFSET,
            self.trader_claim_debit,
        );
        put_u64(
            out,
            crate::EXECUTION_TRADER_CLAIM_CREDIT_OFFSET,
            self.trader_claim_credit,
        );
        put_u64(
            out,
            crate::EXECUTION_PRINCIPAL_BEFORE_OFFSET,
            self.principal_before,
        );
        put_u64(
            out,
            crate::EXECUTION_PRINCIPAL_AFTER_OFFSET,
            self.principal_after,
        );
        put_u64(out, crate::EXECUTION_FEES_BEFORE_OFFSET, self.fees_before);
        put_u64(out, crate::EXECUTION_FEES_AFTER_OFFSET, self.fees_after);
        put_u64(out, crate::EXECUTION_CLAIM_BEFORE_OFFSET, self.claim_before);
        put_u64(out, crate::EXECUTION_CLAIM_AFTER_OFFSET, self.claim_after);
        for bin in 0..usize::from(self.bin_count) {
            let before = self
                .bin_before
                .get(bin)
                .copied()
                .ok_or(Error::InvalidLength)?;
            let after = self
                .bin_after
                .get(bin)
                .copied()
                .ok_or(Error::InvalidLength)?;
            put_u64(
                out,
                checked_offset(crate::EXECUTION_BIN_BEFORE_OFFSET, 8, bin)?,
                before,
            );
            put_u64(
                out,
                checked_offset(
                    crate::EXECUTION_BIN_BEFORE_OFFSET + 8 * usize::from(self.bin_count),
                    8,
                    bin,
                )?,
                after,
            );
        }
        Ok(())
    }
    /// Validate every scalar and per-bin conservation equation.
    pub fn validate(self) -> Result<()> {
        let bins = usize::from(self.bin_count);
        if !(MIN_QUOTE_BINS..=MAX_QUOTE_BINS).contains(&bins)
            || self.quantity == 0
            || self.notional_collateral == 0
            || self.trader_fee_collateral == 0
            || self.fees_before.checked_add(self.trader_fee_collateral) != Some(self.fees_after)
        {
            return Err(Error::ConservationMismatch);
        }
        let mut filled = 0u64;
        for bin in 0..bins {
            let after = self
                .bin_after
                .get(bin)
                .copied()
                .ok_or(Error::ConservationMismatch)?;
            let before = self
                .bin_before
                .get(bin)
                .copied()
                .ok_or(Error::ConservationMismatch)?;
            filled = checked_add(
                filled,
                after
                    .checked_sub(before)
                    .ok_or(Error::ConservationMismatch)?,
            )?;
        }
        if filled != self.quantity {
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
    /// Return side.
    pub const fn side(self) -> TradeSide {
        self.side
    }
    /// Return claim index.
    pub const fn claim_index(self) -> u8 {
        self.claim_index
    }
    /// Return quantity.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
    /// Return principal notional.
    pub const fn notional_collateral(self) -> u64 {
        self.notional_collateral
    }
    /// Return trader-paid fee.
    pub const fn trader_fee_collateral(self) -> u64 {
        self.trader_fee_collateral
    }
}

/// Initialize canonical Pool bytes and first compact LP without a large return value.
#[allow(clippy::too_many_arguments)]
pub fn initialize_pool<const N: usize>(
    out: &mut [u8],
    profile: LiquidityProfileV1,
    attachment: LiquidityAttachment,
    pool_address: [u8; 32],
    config: LiquidityConfigViewV1<'_>,
    pool_rent: RentCreditTerms,
    opened_at_slot: u64,
    initial_liquidity: LiquidityAmounts<N>,
    service_funding: u64,
    initial_position_id: [u8; 32],
    initial_owner: [u8; 32],
    initial_position_rent: RentCreditTerms,
    initial_shares: u64,
) -> Result<(LpPosition, LiquidityChangeReceipt<N>)> {
    if N != profile.outcomes || profile != config.profile() || out.len() != profile.pool_len()? {
        return Err(Error::UnsupportedProfile);
    }
    if attachment.liquidity_config_id() != config.content_id() {
        return Err(Error::ConfigurationMismatch);
    }
    if !initial_liquidity.is_initially_complete() {
        return Err(Error::IncompleteInitialLiquidity);
    }
    if initial_shares == 0 {
        return Err(Error::InvalidQuantity);
    }
    let parent = parent_for(attachment, pool_address)?;
    validate_position_identity(initial_position_id, parent, initial_owner)?;
    let next_reset_slot = opened_at_slot
        .checked_add(config.reset_interval_slots()?)
        .ok_or(Error::InvalidResetInterval)?;
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
    out.fill(0);
    encode_header(out, POOL_MAGIC);
    put(out, STATE_ATTACHMENT_OFFSET, &attachment.to_bytes());
    put(out, STATE_RENT_OFFSET, &pool_rent.to_bytes());
    put_u64(out, STATE_RESET_OFFSET, 0);
    put_u64(out, STATE_SEQUENCE_OFFSET, 1);
    put_u64(out, STATE_NEXT_RESET_SLOT_OFFSET, next_reset_slot);
    put(out, STATE_STATUS_OFFSET, &[PoolStatus::Active.byte()]);
    put_u64(out, STATE_LIVE_POSITIONS_OFFSET, 1);
    put_u64(out, STATE_TOTAL_SHARES_OFFSET, initial_shares);
    put_u64(
        out,
        STATE_PRINCIPAL_OFFSET,
        initial_liquidity.principal_collateral,
    );
    put_u64(out, STATE_FEES_OFFSET, 0);
    put_u64(out, STATE_SERVICE_OFFSET, service_funding);
    for (index, claim) in initial_liquidity.claim_reserves.iter().copied().enumerate() {
        put_u64(out, checked_offset(STATE_CLAIMS_OFFSET, 8, index)?, claim);
    }
    Ok((position, receipt))
}

/// Create a zero-share LP while mutating only the supplied Pool scratch bytes.
#[allow(clippy::too_many_arguments)]
pub fn create_position(
    pool_bytes: &mut [u8],
    profile: LiquidityProfileV1,
    pool_address: [u8; 32],
    config: LiquidityConfigViewV1<'_>,
    expected_pool_sequence: u64,
    position_id: [u8; 32],
    owner: [u8; 32],
    rent_credit: RentCreditTerms,
) -> Result<(LpPosition, PositionCreationReceipt)> {
    let view = PoolViewV1::new(profile, pool_bytes, pool_address, config)?;
    view.require_active()?;
    view.require_sequence(expected_pool_sequence)?;
    let parent = parent_for(view.attachment()?, pool_address)?;
    validate_position_identity(position_id, parent, owner)?;
    let position = LpPosition::new(parent, owner, rent_credit, 0, PositionStatus::Empty)?;
    let live = checked_add(view.live_positions()?, 1)?;
    let next_sequence = checked_add(view.next_sequence()?, 1)?;
    let receipt = PositionCreationReceipt {
        parent,
        pool_sequence: expected_pool_sequence,
        position_id,
        owner,
        rent_credit,
    };
    put_u64(pool_bytes, STATE_LIVE_POSITIONS_OFFSET, live);
    put_u64(pool_bytes, STATE_SEQUENCE_OFFSET, next_sequence);
    Ok((position, receipt))
}

fn require_position(
    position: &LpPosition,
    parent: ParentPool,
    total_shares: u64,
    expected_sequence: u64,
    position_id: [u8; 32],
) -> Result<()> {
    position.validate()?;
    if position.parent != parent
        || position.status == PositionStatus::Closed
        || position.next_sequence != expected_sequence
        || position.shares > total_shares
    {
        return Err(Error::PositionMismatch);
    }
    validate_position_identity(position_id, parent, position.owner)
}

/// Add exact-N proportional LP liquidity with ceiling rounding and atomic mutation.
pub fn add_liquidity<const N: usize>(
    pool_bytes: &mut [u8],
    profile: LiquidityProfileV1,
    pool_address: [u8; 32],
    config: LiquidityConfigViewV1<'_>,
    position_id: [u8; 32],
    position: &mut LpPosition,
    request: crate::AddLiquidityRequest<N>,
) -> Result<LiquidityChangeReceipt<N>> {
    let view = PoolViewV1::new(profile, pool_bytes, pool_address, config)?;
    view.require_active()?;
    view.require_sequence(request.expected_pool_sequence)?;
    let parent = parent_for(view.attachment()?, pool_address)?;
    require_position(
        position,
        parent,
        view.total_shares()?,
        request.expected_position_sequence,
        position_id,
    )?;
    let before = view.liquidity::<N>()?;
    let required =
        super::proportional_amounts(before, request.shares_to_mint, view.total_shares()?, true)?;
    require_amounts_at_most(required, request.maximum_deposit)?;
    let principal_after = checked_add(before.principal_collateral, required.principal_collateral)?;
    let fees_after = checked_add(
        before.realized_fee_collateral,
        required.realized_fee_collateral,
    )?;
    let mut claims_after = before.claim_reserves;
    for (after, add) in claims_after.iter_mut().zip(required.claim_reserves.iter()) {
        *after = checked_add(*after, *add)?;
    }
    let total_before = view.total_shares()?;
    let total_after = checked_add(total_before, request.shares_to_mint)?;
    let position_before = position.shares;
    let position_after = checked_add(position_before, request.shares_to_mint)?;
    let next_pool_sequence = checked_add(view.next_sequence()?, 1)?;
    let next_position_sequence = checked_add(position.next_sequence, 1)?;
    let after = LiquidityAmounts::new(principal_after, fees_after, claims_after)?;
    let receipt = LiquidityChangeReceipt {
        kind: LiquidityChangeKind::Add,
        parent,
        pool_sequence: request.expected_pool_sequence,
        position_id,
        owner: position.owner,
        amounts_before: before,
        amounts_transferred: required,
        amounts_after: after,
        total_shares_before: total_before,
        shares_changed: request.shares_to_mint,
        total_shares_after: total_after,
        position_shares_before: position_before,
        position_shares_after: position_after,
    };
    receipt.validate()?;
    put_u64(pool_bytes, STATE_PRINCIPAL_OFFSET, principal_after);
    put_u64(pool_bytes, STATE_FEES_OFFSET, fees_after);
    for (index, value) in claims_after.iter().copied().enumerate() {
        put_u64(
            pool_bytes,
            checked_offset(STATE_CLAIMS_OFFSET, 8, index)?,
            value,
        );
    }
    put_u64(pool_bytes, STATE_TOTAL_SHARES_OFFSET, total_after);
    put_u64(pool_bytes, STATE_SEQUENCE_OFFSET, next_pool_sequence);
    position.shares = position_after;
    position.status = PositionStatus::Active;
    position.next_sequence = next_position_sequence;
    Ok(receipt)
}

/// Remove exact-N proportional LP liquidity with floor rounding and exact last-LP drain.
pub fn remove_liquidity<const N: usize>(
    pool_bytes: &mut [u8],
    profile: LiquidityProfileV1,
    pool_address: [u8; 32],
    config: LiquidityConfigViewV1<'_>,
    position_id: [u8; 32],
    position: &mut LpPosition,
    request: crate::RemoveLiquidityRequest<N>,
) -> Result<LiquidityChangeReceipt<N>> {
    let view = PoolViewV1::new(profile, pool_bytes, pool_address, config)?;
    view.require_active()?;
    view.require_sequence(request.expected_pool_sequence)?;
    let parent = parent_for(view.attachment()?, pool_address)?;
    let total_before = view.total_shares()?;
    require_position(
        position,
        parent,
        total_before,
        request.expected_position_sequence,
        position_id,
    )?;
    if position.status != PositionStatus::Active
        || request.shares_to_burn > position.shares
        || request.shares_to_burn > total_before
    {
        return Err(Error::InvalidQuantity);
    }
    let before = view.liquidity::<N>()?;
    let withdrawal = if request.shares_to_burn == total_before {
        before
    } else {
        super::proportional_amounts(before, request.shares_to_burn, total_before, false)?
    };
    if withdrawal.is_zero() {
        return Err(Error::ZeroNotional);
    }
    require_amounts_at_least(withdrawal, request.minimum_withdrawal)?;
    let principal_after =
        checked_sub(before.principal_collateral, withdrawal.principal_collateral)?;
    let fees_after = checked_sub(
        before.realized_fee_collateral,
        withdrawal.realized_fee_collateral,
    )?;
    let mut claims_after = before.claim_reserves;
    for (after, amount) in claims_after
        .iter_mut()
        .zip(withdrawal.claim_reserves.iter())
    {
        *after = checked_sub(*after, *amount)?;
    }
    let total_after = checked_sub(total_before, request.shares_to_burn)?;
    let position_before = position.shares;
    let position_after = checked_sub(position_before, request.shares_to_burn)?;
    if total_after == 0
        && (principal_after != 0 || fees_after != 0 || claims_after.iter().any(|value| *value != 0))
    {
        return Err(Error::ConservationMismatch);
    }
    let next_pool_sequence = checked_add(view.next_sequence()?, 1)?;
    let next_position_sequence = checked_add(position.next_sequence, 1)?;
    let after = LiquidityAmounts::new(principal_after, fees_after, claims_after)?;
    let receipt = LiquidityChangeReceipt {
        kind: LiquidityChangeKind::Remove,
        parent,
        pool_sequence: request.expected_pool_sequence,
        position_id,
        owner: position.owner,
        amounts_before: before,
        amounts_transferred: withdrawal,
        amounts_after: after,
        total_shares_before: total_before,
        shares_changed: request.shares_to_burn,
        total_shares_after: total_after,
        position_shares_before: position_before,
        position_shares_after: position_after,
    };
    receipt.validate()?;
    put_u64(pool_bytes, STATE_PRINCIPAL_OFFSET, principal_after);
    put_u64(pool_bytes, STATE_FEES_OFFSET, fees_after);
    for (index, value) in claims_after.iter().copied().enumerate() {
        put_u64(
            pool_bytes,
            checked_offset(STATE_CLAIMS_OFFSET, 8, index)?,
            value,
        );
    }
    put_u64(pool_bytes, STATE_TOTAL_SHARES_OFFSET, total_after);
    put_u64(pool_bytes, STATE_SEQUENCE_OFFSET, next_pool_sequence);
    if total_after == 0 {
        put(
            pool_bytes,
            STATE_STATUS_OFFSET,
            &[PoolStatus::Retiring.byte()],
        );
    }
    position.shares = position_after;
    position.status = if position_after == 0 {
        PositionStatus::Empty
    } else {
        PositionStatus::Active
    };
    position.next_sequence = next_position_sequence;
    Ok(receipt)
}

/// Close an empty LP position and advance both replay clocks atomically.
#[allow(clippy::too_many_arguments)]
pub fn close_position(
    pool_bytes: &mut [u8],
    profile: LiquidityProfileV1,
    pool_address: [u8; 32],
    config: LiquidityConfigViewV1<'_>,
    position_id: [u8; 32],
    position: &mut LpPosition,
    expected_pool_sequence: u64,
    expected_position_sequence: u64,
) -> Result<PositionCloseReceipt> {
    let view = PoolViewV1::new(profile, pool_bytes, pool_address, config)?;
    if view.status()? == PoolStatus::Retired {
        return Err(Error::InvalidPoolStatus);
    }
    view.require_sequence(expected_pool_sequence)?;
    let parent = parent_for(view.attachment()?, pool_address)?;
    require_position(
        position,
        parent,
        view.total_shares()?,
        expected_position_sequence,
        position_id,
    )?;
    if position.status != PositionStatus::Empty || position.shares != 0 {
        return Err(Error::InvalidPositionStatus);
    }
    let live_after = view
        .live_positions()?
        .checked_sub(1)
        .ok_or(Error::ShareInvariant)?;
    let pool_next = checked_add(view.next_sequence()?, 1)?;
    let position_next = checked_add(position.next_sequence, 1)?;
    let receipt = PositionCloseReceipt {
        parent,
        pool_sequence: expected_pool_sequence,
        position_id,
        owner: position.owner,
        rent_credit: position.rent_credit,
    };
    put_u64(pool_bytes, STATE_LIVE_POSITIONS_OFFSET, live_after);
    put_u64(pool_bytes, STATE_SEQUENCE_OFFSET, pool_next);
    position.status = PositionStatus::Closed;
    position.next_sequence = position_next;
    Ok(receipt)
}

/// Execute one covered trade against runtime bins and commit only after full preflight.
pub fn execute(
    pool_bytes: &mut [u8],
    profile: LiquidityProfileV1,
    pool_address: [u8; 32],
    config: LiquidityConfigViewV1<'_>,
    request: TradeRequest,
) -> Result<ExecutionReceiptV1> {
    let view = PoolViewV1::new(profile, pool_bytes, pool_address, config)?;
    view.require_active()?;
    view.require_sequence(request.expected_sequence)?;
    if request.reset_number != view.reset_number()? {
        return Err(Error::InvalidReset);
    }
    if request.claim_index >= profile.outcomes {
        return Err(Error::ClaimIndexOutOfRange);
    }
    if request.quantity == 0 || request.quantity > config.max_trade_quantity()? {
        return Err(Error::InvalidQuantity);
    }
    let mut remaining = request.quantity;
    let mut notional = 0u64;
    let mut before = [0u64; MAX_QUOTE_BINS];
    let mut after = [0u64; MAX_QUOTE_BINS];
    for bin in 0..profile.bins {
        let filled = view.fill(request.side, request.claim_index, bin)?;
        *before.get_mut(bin).ok_or(Error::UnsupportedProfile)? = filled;
        let available = config
            .capacity(request.side, request.claim_index, bin)?
            .checked_sub(filled)
            .ok_or(Error::ConservationMismatch)?;
        let taken = core::cmp::min(remaining, available);
        let segment = if taken == 0 {
            0
        } else {
            match request.side {
                TradeSide::BuyClaimFromPool => mul_div_ceil(
                    taken,
                    config.price(request.side, request.claim_index, bin)?,
                    config.price_scale()?,
                )?,
                TradeSide::SellClaimToPool => mul_div_floor(
                    taken,
                    config.price(request.side, request.claim_index, bin)?,
                    config.price_scale()?,
                )?,
            }
        };
        if taken > 0 && segment == 0 {
            return Err(Error::ZeroNotional);
        }
        notional = checked_add(notional, segment)?;
        remaining = checked_sub(remaining, taken)?;
        *after.get_mut(bin).ok_or(Error::UnsupportedProfile)? = checked_add(filled, taken)?;
    }
    if remaining != 0 {
        return Err(Error::InsufficientBinDepth);
    }
    if notional == 0 {
        return Err(Error::ZeroNotional);
    }
    let fee = mul_div_ceil(
        notional,
        u64::from(config.fee_bps()?),
        BASIS_POINTS_DENOMINATOR,
    )?;
    if fee == 0 {
        return Err(Error::ZeroNotional);
    }
    let principal_before = view.principal_collateral()?;
    let fees_before = view.realized_fee_collateral()?;
    let claim_before = view.claim_reserve(request.claim_index)?;
    let (principal_after, claim_after, debit, credit, claim_debit, claim_credit) =
        match request.side {
            TradeSide::BuyClaimFromPool => {
                let claim_after = claim_before
                    .checked_sub(request.quantity)
                    .ok_or(Error::InsufficientClaimInventory)?;
                let principal_after = checked_add(principal_before, notional)?;
                let debit = checked_add(notional, fee)?;
                if debit > request.collateral_limit {
                    return Err(Error::LimitExceeded);
                }
                (principal_after, claim_after, debit, 0, 0, request.quantity)
            }
            TradeSide::SellClaimToPool => {
                let principal_after = principal_before
                    .checked_sub(notional)
                    .ok_or(Error::InsufficientPrincipalCollateral)?;
                if notional < request.collateral_limit {
                    return Err(Error::LimitExceeded);
                }
                (
                    principal_after,
                    checked_add(claim_before, request.quantity)?,
                    fee,
                    notional,
                    request.quantity,
                    0,
                )
            }
        };
    let fees_after = checked_add(fees_before, fee)?;
    let next_sequence = checked_add(view.next_sequence()?, 1)?;
    let receipt = ExecutionReceiptV1 {
        parent: parent_for(view.attachment()?, pool_address)?,
        reset_number: view.reset_number()?,
        sequence: request.expected_sequence,
        side: request.side,
        claim_index: u8::try_from(request.claim_index).map_err(|_| Error::ClaimIndexOutOfRange)?,
        quantity: request.quantity,
        notional_collateral: notional,
        trader_fee_collateral: fee,
        trader_collateral_debit: debit,
        trader_collateral_credit: credit,
        trader_claim_debit: claim_debit,
        trader_claim_credit: claim_credit,
        principal_before,
        principal_after,
        fees_before,
        fees_after,
        claim_before,
        claim_after,
        bin_count: u8::try_from(profile.bins).map_err(|_| Error::UnsupportedProfile)?,
        bin_before: before,
        bin_after: after,
    };
    receipt.validate()?;
    let section = match request.side {
        TradeSide::SellClaimToPool => 0,
        TradeSide::BuyClaimFromPool => 1,
    };
    let fill_base = view.fill_section_offset(section)?;
    put_u64(pool_bytes, STATE_PRINCIPAL_OFFSET, principal_after);
    put_u64(pool_bytes, STATE_FEES_OFFSET, fees_after);
    put_u64(
        pool_bytes,
        checked_offset(STATE_CLAIMS_OFFSET, 8, request.claim_index)?,
        claim_after,
    );
    put_u64(pool_bytes, STATE_SEQUENCE_OFFSET, next_sequence);
    for (bin, value) in after.iter().copied().enumerate().take(profile.bins) {
        let flat = request
            .claim_index
            .checked_mul(profile.bins)
            .and_then(|value| value.checked_add(bin))
            .ok_or(Error::ArithmeticOverflow)?;
        put_u64(pool_bytes, checked_offset(fill_base, 8, flat)?, value);
    }
    Ok(receipt)
}

/// Reopen the identical immutable ladder only after its slot boundary.
pub fn reset_ladder(
    pool_bytes: &mut [u8],
    profile: LiquidityProfileV1,
    pool_address: [u8; 32],
    config: LiquidityConfigViewV1<'_>,
    expected_pool_sequence: u64,
    authenticated_now_slot: u64,
) -> Result<LadderResetReceiptV1> {
    let view = PoolViewV1::new(profile, pool_bytes, pool_address, config)?;
    view.require_active()?;
    view.require_sequence(expected_pool_sequence)?;
    if authenticated_now_slot < view.next_reset_slot()? {
        return Err(Error::ResetTooEarly);
    }
    let new_reset = checked_add(view.reset_number()?, 1)?;
    let next_reset_slot = authenticated_now_slot
        .checked_add(config.reset_interval_slots()?)
        .ok_or(Error::InvalidResetInterval)?;
    let next_sequence = checked_add(view.next_sequence()?, 1)?;
    let receipt = LadderResetReceiptV1 {
        parent: parent_for(view.attachment()?, pool_address)?,
        pool_sequence: expected_pool_sequence,
        old_reset_number: view.reset_number()?,
        new_reset_number: new_reset,
        observed_slot: authenticated_now_slot,
        next_reset_slot,
    };
    let fills_start = view.fill_section_offset(0)?;
    let fills_len = 16usize
        .checked_mul(profile.cells()?)
        .ok_or(Error::ArithmeticOverflow)?;
    pool_bytes
        .get_mut(
            fills_start
                ..fills_start
                    .checked_add(fills_len)
                    .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::InvalidLength)?
        .fill(0);
    put_u64(pool_bytes, STATE_RESET_OFFSET, new_reset);
    put_u64(pool_bytes, STATE_NEXT_RESET_SLOT_OFFSET, next_reset_slot);
    put_u64(pool_bytes, STATE_SEQUENCE_OFFSET, next_sequence);
    Ok(receipt)
}

/// Retire one quiescent Pool in supplied scratch bytes.
pub fn retire_pool(
    pool_bytes: &mut [u8],
    profile: LiquidityProfileV1,
    pool_address: [u8; 32],
    config: LiquidityConfigViewV1<'_>,
    expected_pool_sequence: u64,
) -> Result<PoolRetirementReceipt> {
    let view = PoolViewV1::new(profile, pool_bytes, pool_address, config)?;
    view.require_sequence(expected_pool_sequence)?;
    if view.status()? != PoolStatus::Retiring
        || view.total_shares()? != 0
        || view.live_positions()? != 0
        || !view.liquidity_is_zero()?
    {
        return Err(Error::PoolNotQuiescent);
    }
    let service = view.service_funding()?;
    let attachment = view.attachment()?;
    let rent = view.rent_credit()?;
    let next_sequence = checked_add(view.next_sequence()?, 1)?;
    let receipt = PoolRetirementReceipt {
        parent: parent_for(attachment, pool_address)?,
        pool_sequence: expected_pool_sequence,
        service_refund_beneficiary: attachment.service_refund_beneficiary(),
        service_refund_collateral: service,
        pool_rent_credit: rent,
    };
    put_u64(pool_bytes, STATE_SERVICE_OFFSET, 0);
    put(
        pool_bytes,
        STATE_STATUS_OFFSET,
        &[PoolStatus::Retired.byte()],
    );
    put_u64(pool_bytes, STATE_SEQUENCE_OFFSET, next_sequence);
    Ok(receipt)
}

/// Decode and canonicalize a compact LP position without heap allocation.
pub fn decode_lp(bytes: &[u8]) -> Result<LpPosition> {
    if bytes.len() != LP_POSITION_BYTES {
        return Err(Error::InvalidLength);
    }
    decode_header(bytes, POSITION_MAGIC)?;
    require_zero(bytes, POSITION_RESERVED_OFFSET, POSITION_RESERVED_BYTES)?;
    let position = LpPosition {
        parent: ParentPool::decode(subslice(bytes, POSITION_PARENT_OFFSET, PARENT_POOL_BYTES)?)?,
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
