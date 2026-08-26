#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout, allocation-free interpreter for successor Dealer liquidity.
//!
//! Lean owns the widths, offsets, tags, and semantic model. This crate checks
//! hostile bytes, exact cumulative quote and fee counters, inventory bounds,
//! prepaid work funding, release receipts, candidate replacement, and terminal
//! unwind. It emits bounded claim/custody intents; a future Solana adapter must
//! authenticate accounts, Registry ownership, signatures, CPI, persistence,
//! and atomic rollback.

/// Runtime-width finite-scenario collateral planning for the V2 successor.
pub mod scenario;

#[rustfmt::skip]
mod generated_dealer_liquidity;

#[rustfmt::skip]
mod generated_dealer_trading_profile;

/// Inventory-free mutable tail for the canonical composite Trading root.
pub mod root_tail;
/// Canonical Trading Dealer request with explicit Claims optimistic revision.
pub mod trading_request;

use generated_dealer_liquidity as generated;

/// Fixed identity width used by this physical profile.
pub type Identity = [u8; 32];
/// Maximum Product outcomes in this provisional physical profile.
pub const MAX_OUTCOMES: usize = generated::MAX_OUTCOMES;
/// Maximum runtime bands per side and outcome.
pub const MAX_BANDS_PER_SIDE: usize = generated::MAX_BANDS_PER_SIDE;
/// Exact immutable policy width.
pub const POLICY_BYTES: usize = generated::POLICY_BYTES;
/// Exact immutable candidate width.
pub const CANDIDATE_BYTES: usize = generated::CANDIDATE_BYTES;
/// Exact persistent state width.
pub const STATE_BYTES: usize = generated::STATE_BYTES;
/// Exact normalized release-receipt width.
pub const RECEIPT_BYTES: usize = generated::RECEIPT_BYTES;
/// Exact invocation request width.
pub const REQUEST_BYTES: usize = generated::REQUEST_BYTES;
/// Maximum exact custody transfers emitted by one transition.
pub const MAX_CUSTODY_TRANSFERS: usize = generated::MAX_CUSTODY_TRANSFERS;

/// Stable refusal from hostile decoding or total execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An input did not have its one exact generated width.
    InvalidLength,
    /// Magic bytes did not identify the requested type.
    InvalidMagic,
    /// The ABI version was not the generated version.
    UnsupportedVersion,
    /// Reserved or inactive fixed-capacity bytes were nonzero.
    NonCanonicalPadding,
    /// A boolean, action, phase, side, or role tag was outside its closed set.
    UnknownTag,
    /// A required identity, count, denominator, quantity, or revision was zero.
    ZeroCoordinate,
    /// Policy, Candidate, State, Receipt, or Request identities did not join.
    IdentityMismatch,
    /// A revision, timestamp, or optimistic state coordinate was stale.
    StaleCoordinate,
    /// Curve bands, spread, usage, or cumulative quote counters were invalid.
    InvalidCurve,
    /// Inventory escaped the immutable active Candidate risk box.
    InventoryRisk,
    /// A custody compartment did not contain the required present funds.
    Underfunded,
    /// The command was not admitted in the current lifecycle phase.
    InvalidPhase,
    /// Checked fixed-width arithmetic overflowed.
    ArithmeticOverflow,
    /// A bounded plan would exceed its generated capacity.
    PlanOverflow,
}

/// Result alias for Dealer decoding and execution.
pub type Result<T> = core::result::Result<T, Error>;

/// Immutable Market-selected Dealer policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Policy {
    /// Canonical Market identity.
    pub market_id: Identity,
    /// Immutable selected Registry release-set identity.
    pub release_set_id: Identity,
    /// Dealer authority and work-funding owner.
    pub dealer_id: Identity,
    /// Accumulated fee recipient.
    pub fee_recipient_id: Identity,
    /// Terminal quote-inventory recipient.
    pub unwind_recipient_id: Identity,
    /// Active finite Product width.
    pub outcome_count: u8,
    /// Sole curve denominator.
    pub quote_scale: u64,
    /// Cumulative fee numerator.
    pub fee_numerator: u64,
    /// Cumulative fee denominator.
    pub fee_denominator: u64,
    /// Minimum present work funding for every Candidate.
    pub minimum_work_funding: u64,
    /// Minimum scheduling delay before replacement activation.
    pub replacement_delay: u64,
}

impl Policy {
    /// Decode and validate one canonical immutable policy.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            POLICY_BYTES,
            &generated::POLICY_MAGIC,
            generated::POLICY_VERSION_OFFSET,
        )?;
        require_zero(input, generated::POLICY_RESERVED_OFFSET, 5)?;
        let value = Self {
            outcome_count: byte_at(input, generated::POLICY_OUTCOME_COUNT_OFFSET)?,
            market_id: array_at(input, generated::POLICY_MARKET_ID_OFFSET)?,
            release_set_id: array_at(input, generated::POLICY_RELEASE_SET_ID_OFFSET)?,
            dealer_id: array_at(input, generated::POLICY_DEALER_ID_OFFSET)?,
            fee_recipient_id: array_at(input, generated::POLICY_FEE_RECIPIENT_ID_OFFSET)?,
            unwind_recipient_id: array_at(input, generated::POLICY_UNWIND_RECIPIENT_ID_OFFSET)?,
            quote_scale: u64_at(input, generated::POLICY_QUOTE_SCALE_OFFSET)?,
            fee_numerator: u64_at(input, generated::POLICY_FEE_NUMERATOR_OFFSET)?,
            fee_denominator: u64_at(input, generated::POLICY_FEE_DENOMINATOR_OFFSET)?,
            minimum_work_funding: u64_at(input, generated::POLICY_MINIMUM_WORK_FUNDING_OFFSET)?,
            replacement_delay: u64_at(input, generated::POLICY_REPLACEMENT_DELAY_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one canonical immutable policy.
    pub fn to_bytes(self) -> Result<[u8; POLICY_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; POLICY_BYTES];
        put_header(
            &mut output,
            &generated::POLICY_MAGIC,
            generated::POLICY_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated::POLICY_OUTCOME_COUNT_OFFSET,
            self.outcome_count,
        )?;
        put(
            &mut output,
            generated::POLICY_MARKET_ID_OFFSET,
            &self.market_id,
        )?;
        put(
            &mut output,
            generated::POLICY_RELEASE_SET_ID_OFFSET,
            &self.release_set_id,
        )?;
        put(
            &mut output,
            generated::POLICY_DEALER_ID_OFFSET,
            &self.dealer_id,
        )?;
        put(
            &mut output,
            generated::POLICY_FEE_RECIPIENT_ID_OFFSET,
            &self.fee_recipient_id,
        )?;
        put(
            &mut output,
            generated::POLICY_UNWIND_RECIPIENT_ID_OFFSET,
            &self.unwind_recipient_id,
        )?;
        put_u64(
            &mut output,
            generated::POLICY_QUOTE_SCALE_OFFSET,
            self.quote_scale,
        )?;
        put_u64(
            &mut output,
            generated::POLICY_FEE_NUMERATOR_OFFSET,
            self.fee_numerator,
        )?;
        put_u64(
            &mut output,
            generated::POLICY_FEE_DENOMINATOR_OFFSET,
            self.fee_denominator,
        )?;
        put_u64(
            &mut output,
            generated::POLICY_MINIMUM_WORK_FUNDING_OFFSET,
            self.minimum_work_funding,
        )?;
        put_u64(
            &mut output,
            generated::POLICY_REPLACEMENT_DELAY_OFFSET,
            self.replacement_delay,
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.outcome_count == 0 || usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(Error::ZeroCoordinate);
        }
        if is_zero(&self.market_id)
            || is_zero(&self.release_set_id)
            || is_zero(&self.dealer_id)
            || is_zero(&self.fee_recipient_id)
            || is_zero(&self.unwind_recipient_id)
            || self.quote_scale == 0
            || self.fee_denominator == 0
            || self.minimum_work_funding == 0
            || self.replacement_delay == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        if self.fee_numerator > self.fee_denominator {
            return Err(Error::InvalidCurve);
        }
        Ok(())
    }
}

/// One immutable constant-price band supplied to the canonical Candidate encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurveBand {
    /// Positive capacity in claim atoms.
    pub capacity: u64,
    /// Positive price numerator under the Policy's sole quote scale.
    pub price_numerator: u64,
}

/// Borrowed bid/ask curve supplied to the canonical Candidate encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurveInput<'a> {
    /// Nonempty bids ordered by nonincreasing price.
    pub bids: &'a [CurveBand],
    /// Nonempty asks ordered by nondecreasing price.
    pub asks: &'a [CurveBand],
}

/// Borrowed immutable Candidate construction data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateInput<'a> {
    /// Immutable content identity.
    pub candidate_id: Identity,
    /// Strictly ordered nonzero revision.
    pub revision: u64,
    /// Earliest permitted activation time.
    pub valid_from: u64,
    /// Exclusive fill expiry time.
    pub expires_at: u64,
    /// Minimum quote custody retained during open trading.
    pub quote_reserve_floor: u64,
    /// Present liveness capital deposited with this Candidate.
    pub work_funding: u64,
    /// Positive reward for each permissionless unit of work.
    pub work_reward: u64,
    /// Exact minimum inventory for every active Product outcome.
    pub minimum_inventory: &'a [u64],
    /// Exact maximum inventory for every active Product outcome.
    pub maximum_inventory: &'a [u64],
    /// Exact bid/ask curve for every active Product outcome.
    pub curves: &'a [CurveInput<'a>],
}

/// Encode one canonical fixed-capacity Candidate from runtime-width borrowed data.
///
/// All hostile input is checked before `output` is modified. Inactive outcomes
/// and bands are then deterministically zeroed, so callers never reproduce
/// generated offsets or padding rules.
pub fn encode_candidate(output: &mut [u8], input: CandidateInput<'_>) -> Result<()> {
    validate_candidate_input(output.len(), input)?;
    output.fill(0);
    put_header(
        output,
        &generated::CANDIDATE_MAGIC,
        generated::CANDIDATE_VERSION_OFFSET,
    )?;
    put_byte(
        output,
        generated::CANDIDATE_OUTCOME_COUNT_OFFSET,
        u8::try_from(input.curves.len()).map_err(|_| Error::ZeroCoordinate)?,
    )?;
    put(
        output,
        generated::CANDIDATE_CANDIDATE_ID_OFFSET,
        &input.candidate_id,
    )?;
    for (offset, value) in [
        (generated::CANDIDATE_REVISION_OFFSET, input.revision),
        (generated::CANDIDATE_VALID_FROM_OFFSET, input.valid_from),
        (generated::CANDIDATE_EXPIRES_AT_OFFSET, input.expires_at),
        (
            generated::CANDIDATE_QUOTE_RESERVE_FLOOR_OFFSET,
            input.quote_reserve_floor,
        ),
        (generated::CANDIDATE_WORK_FUNDING_OFFSET, input.work_funding),
        (generated::CANDIDATE_WORK_REWARD_OFFSET, input.work_reward),
    ] {
        put_u64(output, offset, value)?;
    }
    for outcome in 0..input.curves.len() {
        put_u64(
            output,
            generated::CANDIDATE_MINIMUM_INVENTORY_OFFSET + outcome * 8,
            input.minimum_inventory[outcome],
        )?;
        put_u64(
            output,
            generated::CANDIDATE_MAXIMUM_INVENTORY_OFFSET + outcome * 8,
            input.maximum_inventory[outcome],
        )?;
        encode_curve(output, outcome, input.curves[outcome])?;
    }
    Ok(())
}

fn validate_candidate_input(output_len: usize, input: CandidateInput<'_>) -> Result<()> {
    let count = input.curves.len();
    if output_len != CANDIDATE_BYTES {
        return Err(Error::InvalidLength);
    }
    if count == 0
        || count > MAX_OUTCOMES
        || input.minimum_inventory.len() != count
        || input.maximum_inventory.len() != count
    {
        return Err(Error::ZeroCoordinate);
    }
    if is_zero(&input.candidate_id)
        || input.revision == 0
        || input.valid_from >= input.expires_at
        || input.work_funding == 0
        || input.work_reward == 0
        || input.work_reward > input.work_funding
    {
        return Err(Error::ZeroCoordinate);
    }
    for outcome in 0..count {
        if input.minimum_inventory[outcome] > input.maximum_inventory[outcome] {
            return Err(Error::InventoryRisk);
        }
        validate_curve_input(input.curves[outcome])?;
    }
    Ok(())
}

fn validate_curve_input(curve: CurveInput<'_>) -> Result<()> {
    if curve.bids.is_empty()
        || curve.asks.is_empty()
        || curve.bids.len() > MAX_BANDS_PER_SIDE
        || curve.asks.len() > MAX_BANDS_PER_SIDE
        || curve
            .bids
            .iter()
            .chain(curve.asks.iter())
            .any(|band| band.capacity == 0 || band.price_numerator == 0)
        || curve
            .bids
            .windows(2)
            .any(|pair| pair[0].price_numerator < pair[1].price_numerator)
        || curve
            .asks
            .windows(2)
            .any(|pair| pair[0].price_numerator > pair[1].price_numerator)
        || curve.bids.iter().any(|bid| {
            curve
                .asks
                .iter()
                .any(|ask| bid.price_numerator > ask.price_numerator)
        })
    {
        return Err(Error::InvalidCurve);
    }
    Ok(())
}

fn encode_curve(output: &mut [u8], outcome: usize, curve: CurveInput<'_>) -> Result<()> {
    let offset = curve_offset(outcome);
    put_byte(
        output,
        offset + generated::CURVE_BID_COUNT_OFFSET,
        u8::try_from(curve.bids.len()).map_err(|_| Error::InvalidCurve)?,
    )?;
    put_byte(
        output,
        offset + generated::CURVE_ASK_COUNT_OFFSET,
        u8::try_from(curve.asks.len()).map_err(|_| Error::InvalidCurve)?,
    )?;
    for (index, band) in curve.bids.iter().enumerate() {
        encode_band(
            output,
            offset + generated::CURVE_BIDS_OFFSET + index * generated::BAND_BYTES,
            *band,
        )?;
    }
    for (index, band) in curve.asks.iter().enumerate() {
        encode_band(
            output,
            offset + generated::CURVE_ASKS_OFFSET + index * generated::BAND_BYTES,
            *band,
        )?;
    }
    Ok(())
}

fn encode_band(output: &mut [u8], offset: usize, band: CurveBand) -> Result<()> {
    put_u64(
        output,
        offset + generated::BAND_CAPACITY_OFFSET,
        band.capacity,
    )?;
    put_u64(
        output,
        offset + generated::BAND_PRICE_OFFSET,
        band.price_numerator,
    )
}

/// Borrowed canonical Candidate with runtime curve data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateView<'a> {
    bytes: &'a [u8],
    /// Active Product width.
    pub outcome_count: u8,
    /// Immutable candidate content identity.
    pub candidate_id: Identity,
    /// Strictly ordered candidate revision.
    pub revision: u64,
    /// Earliest permitted activation time.
    pub valid_from: u64,
    /// Exclusive fill expiry time.
    pub expires_at: u64,
    /// Minimum quote custody retained during open trading.
    pub quote_reserve_floor: u64,
    /// Present liveness capital deposited with this Candidate.
    pub work_funding: u64,
    /// Per-transition permissionless work reward.
    pub work_reward: u64,
}

impl<'a> CandidateView<'a> {
    /// Decode structural coordinates without allocating or copying curve data.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        header(
            input,
            CANDIDATE_BYTES,
            &generated::CANDIDATE_MAGIC,
            generated::CANDIDATE_VERSION_OFFSET,
        )?;
        require_zero(input, generated::CANDIDATE_RESERVED_OFFSET, 5)?;
        let value = Self {
            bytes: input,
            outcome_count: byte_at(input, generated::CANDIDATE_OUTCOME_COUNT_OFFSET)?,
            candidate_id: array_at(input, generated::CANDIDATE_CANDIDATE_ID_OFFSET)?,
            revision: u64_at(input, generated::CANDIDATE_REVISION_OFFSET)?,
            valid_from: u64_at(input, generated::CANDIDATE_VALID_FROM_OFFSET)?,
            expires_at: u64_at(input, generated::CANDIDATE_EXPIRES_AT_OFFSET)?,
            quote_reserve_floor: u64_at(input, generated::CANDIDATE_QUOTE_RESERVE_FLOOR_OFFSET)?,
            work_funding: u64_at(input, generated::CANDIDATE_WORK_FUNDING_OFFSET)?,
            work_reward: u64_at(input, generated::CANDIDATE_WORK_REWARD_OFFSET)?,
        };
        if value.outcome_count == 0
            || usize::from(value.outcome_count) > MAX_OUTCOMES
            || is_zero(&value.candidate_id)
            || value.revision == 0
            || value.valid_from >= value.expires_at
            || value.work_funding == 0
            || value.work_reward == 0
            || value.work_reward > value.work_funding
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(value)
    }

    fn validate_against(self, policy: Policy) -> Result<()> {
        if self.outcome_count != policy.outcome_count
            || self.work_funding < policy.minimum_work_funding
        {
            return Err(Error::IdentityMismatch);
        }
        for outcome in 0..MAX_OUTCOMES {
            if outcome < usize::from(self.outcome_count) {
                let minimum = self.minimum_inventory(outcome)?;
                let maximum = self.maximum_inventory(outcome)?;
                if minimum > maximum {
                    return Err(Error::InventoryRisk);
                }
                self.validate_curve(policy, outcome)?;
            } else {
                if self.minimum_inventory(outcome)? != 0 || self.maximum_inventory(outcome)? != 0 {
                    return Err(Error::NonCanonicalPadding);
                }
                require_zero(
                    self.bytes,
                    curve_offset(outcome),
                    generated::OUTCOME_CURVE_BYTES,
                )?;
            }
        }
        Ok(())
    }

    fn minimum_inventory(self, outcome: usize) -> Result<u64> {
        u64_at(
            self.bytes,
            generated::CANDIDATE_MINIMUM_INVENTORY_OFFSET + outcome * 8,
        )
    }

    fn maximum_inventory(self, outcome: usize) -> Result<u64> {
        u64_at(
            self.bytes,
            generated::CANDIDATE_MAXIMUM_INVENTORY_OFFSET + outcome * 8,
        )
    }

    fn count(self, outcome: usize, side: Side) -> Result<usize> {
        let offset = curve_offset(outcome)
            + match side {
                Side::TakerBuys => generated::CURVE_ASK_COUNT_OFFSET,
                Side::TakerSells => generated::CURVE_BID_COUNT_OFFSET,
            };
        Ok(usize::from(byte_at(self.bytes, offset)?))
    }

    fn band(self, outcome: usize, side: Side, index: usize) -> Result<(u64, u64)> {
        if index >= MAX_BANDS_PER_SIDE {
            return Err(Error::InvalidCurve);
        }
        let side_offset = match side {
            Side::TakerBuys => generated::CURVE_ASKS_OFFSET,
            Side::TakerSells => generated::CURVE_BIDS_OFFSET,
        };
        let base = curve_offset(outcome) + side_offset + index * generated::BAND_BYTES;
        Ok((
            u64_at(self.bytes, base + generated::BAND_CAPACITY_OFFSET)?,
            u64_at(self.bytes, base + generated::BAND_PRICE_OFFSET)?,
        ))
    }

    fn validate_curve(self, policy: Policy, outcome: usize) -> Result<()> {
        require_zero(
            self.bytes,
            curve_offset(outcome) + generated::CURVE_RESERVED_OFFSET,
            6,
        )?;
        let bid_count = self.count(outcome, Side::TakerSells)?;
        let ask_count = self.count(outcome, Side::TakerBuys)?;
        if bid_count == 0
            || ask_count == 0
            || bid_count > MAX_BANDS_PER_SIDE
            || ask_count > MAX_BANDS_PER_SIDE
        {
            return Err(Error::InvalidCurve);
        }
        let mut prior_bid = u64::MAX;
        let mut prior_ask = 0_u64;
        let mut greatest_bid = 0_u64;
        let mut smallest_ask = u64::MAX;
        for index in 0..MAX_BANDS_PER_SIDE {
            let bid = self.band(outcome, Side::TakerSells, index)?;
            let ask = self.band(outcome, Side::TakerBuys, index)?;
            if index < bid_count {
                if bid.0 == 0 || bid.1 == 0 || bid.1 > policy.quote_scale || bid.1 > prior_bid {
                    return Err(Error::InvalidCurve);
                }
                prior_bid = bid.1;
                greatest_bid = greatest_bid.max(bid.1);
            } else if bid != (0, 0) {
                return Err(Error::NonCanonicalPadding);
            }
            if index < ask_count {
                if ask.0 == 0 || ask.1 == 0 || ask.1 > policy.quote_scale || ask.1 < prior_ask {
                    return Err(Error::InvalidCurve);
                }
                prior_ask = ask.1;
                smallest_ask = smallest_ask.min(ask.1);
            } else if ask != (0, 0) {
                return Err(Error::NonCanonicalPadding);
            }
        }
        if greatest_bid > smallest_ask {
            return Err(Error::InvalidCurve);
        }
        Ok(())
    }

    fn capacity(self, outcome: usize, side: Side) -> Result<u64> {
        let count = self.count(outcome, side)?;
        let mut total = 0_u64;
        for index in 0..count {
            total = total
                .checked_add(self.band(outcome, side, index)?.0)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(total)
    }

    fn cumulative_quote(
        self,
        policy: Policy,
        outcome: usize,
        side: Side,
        quantity: u64,
    ) -> Result<u64> {
        let count = self.count(outcome, side)?;
        let mut remaining = quantity;
        let mut numerator = 0_u128;
        for index in 0..count {
            let (capacity, price) = self.band(outcome, side, index)?;
            let taken = remaining.min(capacity);
            numerator = numerator
                .checked_add(u128::from(taken) * u128::from(price))
                .ok_or(Error::ArithmeticOverflow)?;
            remaining -= taken;
        }
        if remaining != 0 {
            return Err(Error::InvalidCurve);
        }
        let scale = u128::from(policy.quote_scale);
        let rounded = match side {
            Side::TakerBuys => {
                numerator
                    .checked_add(scale - 1)
                    .ok_or(Error::ArithmeticOverflow)?
                    / scale
            }
            Side::TakerSells => numerator / scale,
        };
        u64_from(rounded)
    }
}

/// Dealer lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// Candidate accepts fills and replacements.
    Open,
    /// Fills are closed; inventory is redeemed or burned.
    Terminal,
    /// Inventory and all custody compartments are closed.
    Retired,
}

impl Phase {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated::PHASE_OPEN => Ok(Self::Open),
            generated::PHASE_TERMINAL => Ok(Self::Terminal),
            generated::PHASE_RETIRED => Ok(Self::Retired),
            _ => Err(Error::UnknownTag),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Open => generated::PHASE_OPEN,
            Self::Terminal => generated::PHASE_TERMINAL,
            Self::Retired => generated::PHASE_RETIRED,
        }
    }
}

/// Persistent fixed-capacity Dealer state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    /// Lifecycle phase.
    pub phase: Phase,
    /// Product width repeated for hostile join validation.
    pub outcome_count: u8,
    /// Terminal winner; zero outside terminal state.
    pub winner: u8,
    /// Active Candidate identity.
    pub active_candidate_id: Identity,
    /// Pending Candidate identity or all zero.
    pub pending_candidate_id: Identity,
    /// Immutable Market-selected release set.
    pub release_set_id: Identity,
    /// Active Candidate revision.
    pub active_revision: u64,
    /// Pending Candidate revision or zero.
    pub pending_revision: u64,
    /// Optimistic transition revision.
    pub state_revision: u64,
    /// Exact Dealer claim-risk projection.
    pub inventory: [u64; MAX_OUTCOMES],
    /// Cumulative ask-curve usage.
    pub buy_used: [u64; MAX_OUTCOMES],
    /// Cumulative bid-curve usage.
    pub sell_used: [u64; MAX_OUTCOMES],
    /// Cumulative rounded ask debits paid.
    pub buy_quote_paid: [u64; MAX_OUTCOMES],
    /// Cumulative rounded bid proceeds paid.
    pub sell_quote_paid: [u64; MAX_OUTCOMES],
    /// Cumulative fragmentation-independent fee base.
    pub fee_base: u64,
    /// Exact cumulative fee due.
    pub fee_paid: u64,
    /// Dealer-owned quote custody.
    pub quote_custody: u64,
    /// Fee-recipient custody.
    pub fee_custody: u64,
    /// Present work-funding custody.
    pub liveness_custody: u64,
    /// Active Candidate work funds remaining.
    pub active_work_remaining: u64,
    /// Pending Candidate funding or zero.
    pub pending_work_funding: u64,
}

impl State {
    /// Decode one canonical persistent state.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            STATE_BYTES,
            &generated::STATE_MAGIC,
            generated::STATE_VERSION_OFFSET,
        )?;
        require_zero(input, generated::STATE_RESERVED_A_OFFSET, 2)?;
        require_zero(input, generated::STATE_RESERVED_B_OFFSET, 8)?;
        let has_pending = bool_at(input, generated::STATE_HAS_PENDING_OFFSET)?;
        let value = Self {
            phase: Phase::decode(byte_at(input, generated::STATE_PHASE_OFFSET)?)?,
            outcome_count: byte_at(input, generated::STATE_OUTCOME_COUNT_OFFSET)?,
            winner: byte_at(input, generated::STATE_WINNER_OFFSET)?,
            active_candidate_id: array_at(input, generated::STATE_ACTIVE_CANDIDATE_ID_OFFSET)?,
            pending_candidate_id: array_at(input, generated::STATE_PENDING_CANDIDATE_ID_OFFSET)?,
            release_set_id: array_at(input, generated::STATE_RELEASE_SET_ID_OFFSET)?,
            active_revision: u64_at(input, generated::STATE_ACTIVE_REVISION_OFFSET)?,
            pending_revision: u64_at(input, generated::STATE_PENDING_REVISION_OFFSET)?,
            state_revision: u64_at(input, generated::STATE_STATE_REVISION_OFFSET)?,
            inventory: u64_array_at(input, generated::STATE_INVENTORY_OFFSET)?,
            buy_used: u64_array_at(input, generated::STATE_BUY_USED_OFFSET)?,
            sell_used: u64_array_at(input, generated::STATE_SELL_USED_OFFSET)?,
            buy_quote_paid: u64_array_at(input, generated::STATE_BUY_QUOTE_PAID_OFFSET)?,
            sell_quote_paid: u64_array_at(input, generated::STATE_SELL_QUOTE_PAID_OFFSET)?,
            fee_base: u64_at(input, generated::STATE_FEE_BASE_OFFSET)?,
            fee_paid: u64_at(input, generated::STATE_FEE_PAID_OFFSET)?,
            quote_custody: u64_at(input, generated::STATE_QUOTE_CUSTODY_OFFSET)?,
            fee_custody: u64_at(input, generated::STATE_FEE_CUSTODY_OFFSET)?,
            liveness_custody: u64_at(input, generated::STATE_LIVENESS_CUSTODY_OFFSET)?,
            active_work_remaining: u64_at(input, generated::STATE_ACTIVE_WORK_REMAINING_OFFSET)?,
            pending_work_funding: u64_at(input, generated::STATE_PENDING_WORK_FUNDING_OFFSET)?,
        };
        if has_pending != !is_zero(&value.pending_candidate_id) {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// Encode one structurally canonical state.
    pub fn to_bytes(self) -> Result<[u8; STATE_BYTES]> {
        let mut output = [0_u8; STATE_BYTES];
        self.encode_into(&mut output)?;
        Ok(output)
    }

    /// Encode one structurally canonical state into exact caller-owned storage.
    ///
    /// The borrowed form lets an SBF adapter write directly into an already
    /// authenticated State account without placing a second 840-byte State
    /// image in its bounded stack frame.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != STATE_BYTES {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        put_header(
            output,
            &generated::STATE_MAGIC,
            generated::STATE_VERSION_OFFSET,
        )?;
        put_byte(output, generated::STATE_PHASE_OFFSET, self.phase.tag())?;
        put_byte(
            output,
            generated::STATE_OUTCOME_COUNT_OFFSET,
            self.outcome_count,
        )?;
        put_byte(
            output,
            generated::STATE_HAS_PENDING_OFFSET,
            u8::from(!is_zero(&self.pending_candidate_id)),
        )?;
        put_byte(output, generated::STATE_WINNER_OFFSET, self.winner)?;
        put(
            output,
            generated::STATE_ACTIVE_CANDIDATE_ID_OFFSET,
            &self.active_candidate_id,
        )?;
        put(
            output,
            generated::STATE_PENDING_CANDIDATE_ID_OFFSET,
            &self.pending_candidate_id,
        )?;
        put(
            output,
            generated::STATE_RELEASE_SET_ID_OFFSET,
            &self.release_set_id,
        )?;
        put_u64(
            output,
            generated::STATE_ACTIVE_REVISION_OFFSET,
            self.active_revision,
        )?;
        put_u64(
            output,
            generated::STATE_PENDING_REVISION_OFFSET,
            self.pending_revision,
        )?;
        put_u64(
            output,
            generated::STATE_STATE_REVISION_OFFSET,
            self.state_revision,
        )?;
        put_u64_array(output, generated::STATE_INVENTORY_OFFSET, &self.inventory)?;
        put_u64_array(output, generated::STATE_BUY_USED_OFFSET, &self.buy_used)?;
        put_u64_array(output, generated::STATE_SELL_USED_OFFSET, &self.sell_used)?;
        put_u64_array(
            output,
            generated::STATE_BUY_QUOTE_PAID_OFFSET,
            &self.buy_quote_paid,
        )?;
        put_u64_array(
            output,
            generated::STATE_SELL_QUOTE_PAID_OFFSET,
            &self.sell_quote_paid,
        )?;
        put_u64(output, generated::STATE_FEE_BASE_OFFSET, self.fee_base)?;
        put_u64(output, generated::STATE_FEE_PAID_OFFSET, self.fee_paid)?;
        put_u64(
            output,
            generated::STATE_QUOTE_CUSTODY_OFFSET,
            self.quote_custody,
        )?;
        put_u64(
            output,
            generated::STATE_FEE_CUSTODY_OFFSET,
            self.fee_custody,
        )?;
        put_u64(
            output,
            generated::STATE_LIVENESS_CUSTODY_OFFSET,
            self.liveness_custody,
        )?;
        put_u64(
            output,
            generated::STATE_ACTIVE_WORK_REMAINING_OFFSET,
            self.active_work_remaining,
        )?;
        put_u64(
            output,
            generated::STATE_PENDING_WORK_FUNDING_OFFSET,
            self.pending_work_funding,
        )?;
        Ok(())
    }
}

/// Normalized current Registry/Core Trading receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseReceipt {
    /// Registry program which owns and authenticated the receipt account.
    pub registry_program: Identity,
    /// Exact selected release-set identity.
    pub release_set_id: Identity,
    /// Trading program identity.
    pub program: Identity,
    /// Current deployed artifact identity.
    pub artifact_release: Identity,
    /// Trading semantic release identity.
    pub semantic_release: Identity,
}

impl ReleaseReceipt {
    /// Decode a receipt requiring authenticated activation and current deployment flags.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            RECEIPT_BYTES,
            &generated::RECEIPT_MAGIC,
            generated::RECEIPT_VERSION_OFFSET,
        )?;
        require_zero(input, generated::RECEIPT_RESERVED_OFFSET, 4)?;
        if byte_at(input, generated::RECEIPT_ROLE_OFFSET)? != generated::TRADING_ROLE
            || byte_at(input, generated::RECEIPT_FLAGS_OFFSET)? != generated::RECEIPT_REQUIRED_FLAGS
        {
            return Err(Error::UnknownTag);
        }
        let value = Self {
            registry_program: array_at(input, generated::RECEIPT_REGISTRY_PROGRAM_OFFSET)?,
            release_set_id: array_at(input, generated::RECEIPT_RELEASE_SET_ID_OFFSET)?,
            program: array_at(input, generated::RECEIPT_PROGRAM_OFFSET)?,
            artifact_release: array_at(input, generated::RECEIPT_ARTIFACT_RELEASE_OFFSET)?,
            semantic_release: array_at(input, generated::RECEIPT_SEMANTIC_RELEASE_OFFSET)?,
        };
        if is_zero(&value.registry_program)
            || is_zero(&value.release_set_id)
            || is_zero(&value.program)
            || is_zero(&value.artifact_release)
            || is_zero(&value.semantic_release)
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(value)
    }

    /// Encode a normalized authenticated/current receipt.
    pub fn to_bytes(self) -> Result<[u8; RECEIPT_BYTES]> {
        if is_zero(&self.registry_program)
            || is_zero(&self.release_set_id)
            || is_zero(&self.program)
            || is_zero(&self.artifact_release)
            || is_zero(&self.semantic_release)
        {
            return Err(Error::ZeroCoordinate);
        }
        let mut output = [0_u8; RECEIPT_BYTES];
        put_header(
            &mut output,
            &generated::RECEIPT_MAGIC,
            generated::RECEIPT_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated::RECEIPT_ROLE_OFFSET,
            generated::TRADING_ROLE,
        )?;
        put_byte(
            &mut output,
            generated::RECEIPT_FLAGS_OFFSET,
            generated::RECEIPT_REQUIRED_FLAGS,
        )?;
        put(
            &mut output,
            generated::RECEIPT_REGISTRY_PROGRAM_OFFSET,
            &self.registry_program,
        )?;
        put(
            &mut output,
            generated::RECEIPT_RELEASE_SET_ID_OFFSET,
            &self.release_set_id,
        )?;
        put(
            &mut output,
            generated::RECEIPT_PROGRAM_OFFSET,
            &self.program,
        )?;
        put(
            &mut output,
            generated::RECEIPT_ARTIFACT_RELEASE_OFFSET,
            &self.artifact_release,
        )?;
        put(
            &mut output,
            generated::RECEIPT_SEMANTIC_RELEASE_OFFSET,
            &self.semantic_release,
        )?;
        Ok(output)
    }
}

/// Data-defined transition action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Schedule or replace one prepaid pending Candidate.
    ScheduleReplacement,
    /// Activate the pending Candidate after its delay.
    ActivateReplacement,
    /// Execute one bounded bid or ask fill.
    Fill,
    /// Permissionless transition from an adapter-authenticated Core terminal state.
    EnterTerminal,
    /// Redeem or burn one terminal inventory coordinate.
    Unwind,
    /// Close all custody after inventory reaches zero.
    Retire,
    /// Add Dealer-owned quote principal or one native-claim coordinate.
    AddLiquidity,
    /// Remove Dealer-owned quote principal or one native-claim coordinate.
    RemoveLiquidity,
}

impl Action {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated::ACTION_SCHEDULE_REPLACEMENT => Ok(Self::ScheduleReplacement),
            generated::ACTION_ACTIVATE_REPLACEMENT => Ok(Self::ActivateReplacement),
            generated::ACTION_FILL => Ok(Self::Fill),
            generated::ACTION_ENTER_TERMINAL => Ok(Self::EnterTerminal),
            generated::ACTION_UNWIND => Ok(Self::Unwind),
            generated::ACTION_RETIRE => Ok(Self::Retire),
            generated::ACTION_ADD_LIQUIDITY => Ok(Self::AddLiquidity),
            generated::ACTION_REMOVE_LIQUIDITY => Ok(Self::RemoveLiquidity),
            _ => Err(Error::UnknownTag),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::ScheduleReplacement => generated::ACTION_SCHEDULE_REPLACEMENT,
            Self::ActivateReplacement => generated::ACTION_ACTIVATE_REPLACEMENT,
            Self::Fill => generated::ACTION_FILL,
            Self::EnterTerminal => generated::ACTION_ENTER_TERMINAL,
            Self::Unwind => generated::ACTION_UNWIND,
            Self::Retire => generated::ACTION_RETIRE,
            Self::AddLiquidity => generated::ACTION_ADD_LIQUIDITY,
            Self::RemoveLiquidity => generated::ACTION_REMOVE_LIQUIDITY,
        }
    }
}

/// Taker direction relative to Dealer inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    /// Taker buys claims; Dealer inventory decreases at the ask curve.
    TakerBuys,
    /// Taker sells claims; Dealer inventory increases at the bid curve.
    TakerSells,
}

impl Side {
    fn decode(value: u8) -> Result<Self> {
        match value {
            generated::SIDE_BUY => Ok(Self::TakerBuys),
            generated::SIDE_SELL => Ok(Self::TakerSells),
            _ => Err(Error::UnknownTag),
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::TakerBuys => generated::SIDE_BUY,
            Self::TakerSells => generated::SIDE_SELL,
        }
    }
}

/// Canonical fixed request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    /// Transition action.
    pub action: Action,
    /// Fill direction; canonical buy tag for non-fill actions.
    pub side: Side,
    /// Fill/unwind outcome, terminal winner, or liquidity asset coordinate.
    /// For liquidity changes, `outcome_count` is the quote-principal sentinel.
    pub outcome: u8,
    /// Exact expected state revision.
    pub expected_state_revision: u64,
    /// Current time for time-sensitive actions.
    pub now: u64,
    /// Fill or unwind quantity.
    pub quantity: u64,
    /// Exact active Candidate identity expected by the caller.
    pub expected_candidate_id: Identity,
    /// Dealer signer for scheduling, canonical Core Market for terminal entry,
    /// or zero for all other permissionless actions.
    pub actor_id: Identity,
    /// Proposed/pending Candidate identity for replacement actions.
    pub replacement_candidate_id: Identity,
    /// Exact expected active/proposed Candidate revision.
    pub expected_candidate_revision: u64,
}

impl Request {
    /// Decode and validate one exact action-shaped request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            REQUEST_BYTES,
            &generated::REQUEST_MAGIC,
            generated::REQUEST_VERSION_OFFSET,
        )?;
        require_zero(input, generated::REQUEST_RESERVED_OFFSET, 3)?;
        let value = Self {
            action: Action::decode(byte_at(input, generated::REQUEST_ACTION_OFFSET)?)?,
            side: Side::decode(byte_at(input, generated::REQUEST_SIDE_OFFSET)?)?,
            outcome: byte_at(input, generated::REQUEST_OUTCOME_OFFSET)?,
            expected_state_revision: u64_at(
                input,
                generated::REQUEST_EXPECTED_STATE_REVISION_OFFSET,
            )?,
            now: u64_at(input, generated::REQUEST_NOW_OFFSET)?,
            quantity: u64_at(input, generated::REQUEST_QUANTITY_OFFSET)?,
            expected_candidate_id: array_at(
                input,
                generated::REQUEST_EXPECTED_CANDIDATE_ID_OFFSET,
            )?,
            actor_id: array_at(input, generated::REQUEST_ACTOR_ID_OFFSET)?,
            replacement_candidate_id: array_at(
                input,
                generated::REQUEST_REPLACEMENT_CANDIDATE_ID_OFFSET,
            )?,
            expected_candidate_revision: u64_at(
                input,
                generated::REQUEST_EXPECTED_CANDIDATE_REVISION_OFFSET,
            )?,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Encode one exact action-shaped request.
    pub fn to_bytes(self) -> Result<[u8; REQUEST_BYTES]> {
        self.validate_shape()?;
        let mut output = [0_u8; REQUEST_BYTES];
        put_header(
            &mut output,
            &generated::REQUEST_MAGIC,
            generated::REQUEST_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated::REQUEST_ACTION_OFFSET,
            self.action.tag(),
        )?;
        put_byte(&mut output, generated::REQUEST_SIDE_OFFSET, self.side.tag())?;
        put_byte(&mut output, generated::REQUEST_OUTCOME_OFFSET, self.outcome)?;
        put_u64(
            &mut output,
            generated::REQUEST_EXPECTED_STATE_REVISION_OFFSET,
            self.expected_state_revision,
        )?;
        put_u64(&mut output, generated::REQUEST_NOW_OFFSET, self.now)?;
        put_u64(
            &mut output,
            generated::REQUEST_QUANTITY_OFFSET,
            self.quantity,
        )?;
        put(
            &mut output,
            generated::REQUEST_EXPECTED_CANDIDATE_ID_OFFSET,
            &self.expected_candidate_id,
        )?;
        put(
            &mut output,
            generated::REQUEST_ACTOR_ID_OFFSET,
            &self.actor_id,
        )?;
        put(
            &mut output,
            generated::REQUEST_REPLACEMENT_CANDIDATE_ID_OFFSET,
            &self.replacement_candidate_id,
        )?;
        put_u64(
            &mut output,
            generated::REQUEST_EXPECTED_CANDIDATE_REVISION_OFFSET,
            self.expected_candidate_revision,
        )?;
        Ok(output)
    }

    fn validate_shape(self) -> Result<()> {
        if self.expected_state_revision == 0
            || is_zero(&self.expected_candidate_id)
            || self.expected_candidate_revision == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        let zero_actor = is_zero(&self.actor_id);
        let zero_replacement = is_zero(&self.replacement_candidate_id);
        let canonical_non_fill_side = self.side == Side::TakerBuys;
        match self.action {
            Action::ScheduleReplacement => {
                if !canonical_non_fill_side
                    || self.outcome != 0
                    || self.quantity != 0
                    || zero_actor
                    || zero_replacement
                {
                    return Err(Error::NonCanonicalPadding);
                }
            }
            Action::ActivateReplacement => {
                if !canonical_non_fill_side
                    || self.outcome != 0
                    || self.quantity != 0
                    || !zero_actor
                    || zero_replacement
                {
                    return Err(Error::NonCanonicalPadding);
                }
            }
            Action::Fill => {
                if self.quantity == 0 || !zero_actor || !zero_replacement {
                    return Err(Error::NonCanonicalPadding);
                }
            }
            Action::EnterTerminal => {
                if !canonical_non_fill_side || self.quantity != 0 || zero_actor || !zero_replacement
                {
                    return Err(Error::NonCanonicalPadding);
                }
            }
            Action::Unwind => {
                if !canonical_non_fill_side
                    || self.quantity == 0
                    || !zero_actor
                    || !zero_replacement
                {
                    return Err(Error::NonCanonicalPadding);
                }
            }
            Action::Retire => {
                if !canonical_non_fill_side
                    || self.outcome != 0
                    || self.now != 0
                    || self.quantity != 0
                    || !zero_actor
                    || !zero_replacement
                {
                    return Err(Error::NonCanonicalPadding);
                }
            }
            Action::AddLiquidity | Action::RemoveLiquidity => {
                if !canonical_non_fill_side
                    || self.now != 0
                    || self.quantity == 0
                    || zero_actor
                    || !zero_replacement
                {
                    return Err(Error::NonCanonicalPadding);
                }
            }
        }
        Ok(())
    }
}

/// Semantic claim action to be refined by the Claims adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimAction {
    /// No claim mutation for policy/cursor-only transitions.
    None,
    /// Transfer native claims between Dealer and taker.
    Transfer {
        /// Fill direction.
        side: Side,
        /// Product outcome.
        outcome: u8,
        /// Exact claim quantity.
        quantity: u64,
    },
    /// Redeem winning or burn losing Dealer inventory.
    Redeem {
        /// Product outcome.
        outcome: u8,
        /// Exact claim quantity.
        quantity: u64,
        /// Exact categorical payout to Dealer quote custody.
        payout: u64,
    },
    /// Move one native-claim coordinate between the Dealer owner and the
    /// Trading child-root Position without mirroring either balance.
    AdjustLiquidity {
        /// `true` moves owner to child root; `false` moves child root to owner.
        add: bool,
        /// Product outcome.
        outcome: u8,
        /// Exact claim quantity.
        quantity: u64,
    },
}

/// Named custody compartment used by the adapter plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodyRole {
    /// Dealer-owned quote inventory.
    DealerQuote,
    /// Taker quote account.
    TakerQuote,
    /// Fee-recipient vault.
    FeeVault,
    /// Prepaid liveness vault.
    LivenessVault,
    /// Permissionless executor reward account.
    Executor,
    /// Dealer authority funding/refund account.
    DealerOwner,
    /// Terminal Dealer quote recipient.
    UnwindRecipient,
    /// Policy-selected accumulated-fee recipient.
    FeeRecipient,
    /// Realm Hoard used only for terminal categorical payout.
    MarketHoard,
}

/// One indivisible exact custody transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyTransfer {
    /// Debit compartment.
    pub source: CustodyRole,
    /// Credit compartment.
    pub destination: CustodyRole,
    /// Exact quote quantity.
    pub amount: u64,
}

/// Bounded generated transition plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Plan {
    /// Claims-side action.
    pub claim: ClaimAction,
    /// Fixed-capacity exact custody transfers; inactive entries are `None`.
    pub custody: [Option<CustodyTransfer>; MAX_CUSTODY_TRANSFERS],
}

impl Plan {
    const fn empty() -> Self {
        Self {
            claim: ClaimAction::None,
            custody: [None; MAX_CUSTODY_TRANSFERS],
        }
    }

    fn push(&mut self, source: CustodyRole, destination: CustodyRole, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        for slot in &mut self.custody {
            if slot.is_none() {
                *slot = Some(CustodyTransfer {
                    source,
                    destination,
                    amount,
                });
                return Ok(());
            }
        }
        Err(Error::PlanOverflow)
    }
}

/// Successful total transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Transition {
    /// Revalidated exact post-state.
    pub post: State,
    /// Bounded claim/custody plan for atomic adapter execution.
    pub plan: Plan,
}

/// Borrowed byte inputs for one transition.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    /// Immutable Policy account bytes.
    pub policy: &'a [u8],
    /// Current active Candidate account bytes.
    pub active_candidate: &'a [u8],
    /// Current pending Candidate bytes when State has one.
    pub pending_candidate: Option<&'a [u8]>,
    /// Newly proposed Candidate bytes for scheduling.
    pub proposed_candidate: Option<&'a [u8]>,
    /// Current normalized Registry/Core Trading receipt.
    pub release_receipt: &'a [u8],
    /// Current persistent State bytes.
    pub state: &'a [u8],
    /// Exact action request bytes.
    pub request: &'a [u8],
}

/// Decode, join, and execute one Dealer transition without allocation.
#[inline(never)]
pub fn interpret(inputs: Inputs<'_>) -> Result<Transition> {
    let policy = Policy::decode(inputs.policy)?;
    let active = CandidateView::decode(inputs.active_candidate)?;
    active.validate_against(policy)?;
    let pending = match inputs.pending_candidate {
        Some(bytes) => {
            let candidate = CandidateView::decode(bytes)?;
            candidate.validate_against(policy)?;
            Some(candidate)
        }
        None => None,
    };
    let proposed = match inputs.proposed_candidate {
        Some(bytes) => {
            let candidate = CandidateView::decode(bytes)?;
            candidate.validate_against(policy)?;
            Some(candidate)
        }
        None => None,
    };
    let receipt = ReleaseReceipt::decode(inputs.release_receipt)?;
    let state = State::decode(inputs.state)?;
    let request = Request::decode(inputs.request)?;
    if receipt.release_set_id != policy.release_set_id {
        return Err(Error::IdentityMismatch);
    }
    interpret_projected(policy, active, pending, proposed, state, request)
}

/// Execute the total Dealer machine from already authenticated projections.
///
/// The canonical Trading adapter uses this boundary after it authenticates
/// the selected config, Candidate accounts, composite root tail, Claims
/// Position, Custody vaults, and fixed Trading release context. This function
/// owns no Registry or Solana authority and therefore accepts no parallel
/// release receipt. The returned `State` remains transaction-local; an
/// adapter persists only Dealer-owned coordinates from it.
pub fn interpret_projected(
    policy: Policy,
    active: CandidateView<'_>,
    pending: Option<CandidateView<'_>>,
    proposed: Option<CandidateView<'_>>,
    state: State,
    request: Request,
) -> Result<Transition> {
    policy.validate()?;
    active.validate_against(policy)?;
    if let Some(candidate) = pending {
        candidate.validate_against(policy)?;
    }
    if let Some(candidate) = proposed {
        candidate.validate_against(policy)?;
    }
    validate_projected_joins(policy, active, pending, state, request)?;
    let transition = match request.action {
        Action::ScheduleReplacement => schedule(policy, active, pending, proposed, state, request),
        Action::ActivateReplacement => activate(policy, pending, proposed, state, request),
        Action::Fill => fill(policy, active, pending, state, request),
        Action::EnterTerminal => enter_terminal(policy, active, state, request),
        Action::Unwind => unwind(policy, active, state, request),
        Action::Retire => retire(policy, active, state),
        Action::AddLiquidity => adjust_liquidity(policy, active, pending, state, request, true),
        Action::RemoveLiquidity => adjust_liquidity(policy, active, pending, state, request, false),
    }?;
    Ok(transition)
}

fn validate_projected_joins(
    policy: Policy,
    active: CandidateView<'_>,
    pending: Option<CandidateView<'_>>,
    state: State,
    request: Request,
) -> Result<()> {
    if state.release_set_id != policy.release_set_id
        || state.active_candidate_id != active.candidate_id
        || state.active_revision != active.revision
        || request.expected_candidate_id != active.candidate_id
        || request.expected_candidate_revision != active.revision
    {
        return Err(Error::IdentityMismatch);
    }
    if state.outcome_count != policy.outcome_count
        || request.expected_state_revision != state.state_revision
    {
        return Err(Error::StaleCoordinate);
    }
    match (is_zero(&state.pending_candidate_id), pending) {
        (true, None) => {
            if state.pending_revision != 0 || state.pending_work_funding != 0 {
                return Err(Error::NonCanonicalPadding);
            }
        }
        (false, Some(candidate)) => {
            if state.pending_candidate_id != candidate.candidate_id
                || state.pending_revision != candidate.revision
                || state.pending_work_funding != candidate.work_funding
                || candidate.revision <= active.revision
            {
                return Err(Error::IdentityMismatch);
            }
        }
        _ => return Err(Error::IdentityMismatch),
    }
    validate_state(policy, active, pending, state)
}

fn validate_state(
    policy: Policy,
    active: CandidateView<'_>,
    pending: Option<CandidateView<'_>>,
    state: State,
) -> Result<()> {
    if state.state_revision == 0 || state.active_revision == 0 {
        return Err(Error::ZeroCoordinate);
    }
    let pending_work = pending.map_or(0, |candidate| candidate.work_funding);
    if state.pending_work_funding != pending_work
        || state.liveness_custody
            != state
                .active_work_remaining
                .checked_add(pending_work)
                .ok_or(Error::ArithmeticOverflow)?
        || state.active_work_remaining > active.work_funding
    {
        return Err(Error::Underfunded);
    }
    if state.fee_paid != fee_due(policy, state.fee_base)? {
        return Err(Error::InvalidCurve);
    }
    if state.phase != Phase::Retired && state.fee_custody != state.fee_paid {
        return Err(Error::Underfunded);
    }
    let count = usize::from(policy.outcome_count);
    for outcome in 0..MAX_OUTCOMES {
        if outcome < count {
            let buy_capacity = active.capacity(outcome, Side::TakerBuys)?;
            let sell_capacity = active.capacity(outcome, Side::TakerSells)?;
            if state.buy_used[outcome] > buy_capacity || state.sell_used[outcome] > sell_capacity {
                return Err(Error::InvalidCurve);
            }
            if state.buy_quote_paid[outcome]
                != active.cumulative_quote(
                    policy,
                    outcome,
                    Side::TakerBuys,
                    state.buy_used[outcome],
                )?
                || state.sell_quote_paid[outcome]
                    != active.cumulative_quote(
                        policy,
                        outcome,
                        Side::TakerSells,
                        state.sell_used[outcome],
                    )?
            {
                return Err(Error::InvalidCurve);
            }
            if state.phase == Phase::Open
                && (state.inventory[outcome] < active.minimum_inventory(outcome)?
                    || state.inventory[outcome] > active.maximum_inventory(outcome)?)
            {
                return Err(Error::InventoryRisk);
            }
        } else if state.inventory[outcome] != 0
            || state.buy_used[outcome] != 0
            || state.sell_used[outcome] != 0
            || state.buy_quote_paid[outcome] != 0
            || state.sell_quote_paid[outcome] != 0
        {
            return Err(Error::NonCanonicalPadding);
        }
    }
    match state.phase {
        Phase::Open => {
            if state.winner != 0 || state.quote_custody < active.quote_reserve_floor {
                return Err(Error::InvalidPhase);
            }
        }
        Phase::Terminal => {
            if usize::from(state.winner) >= count {
                return Err(Error::InvalidPhase);
            }
        }
        Phase::Retired => {
            if state.winner != 0
                || pending.is_some()
                || state.inventory.iter().any(|quantity| *quantity != 0)
                || state.quote_custody != 0
                || state.fee_custody != 0
                || state.liveness_custody != 0
                || state.active_work_remaining != 0
            {
                return Err(Error::InvalidPhase);
            }
        }
    }
    Ok(())
}

fn schedule(
    policy: Policy,
    active: CandidateView<'_>,
    pending: Option<CandidateView<'_>>,
    proposed: Option<CandidateView<'_>>,
    mut state: State,
    request: Request,
) -> Result<Transition> {
    if state.phase != Phase::Open || request.actor_id != policy.dealer_id {
        return Err(Error::InvalidPhase);
    }
    let proposed = proposed.ok_or(Error::IdentityMismatch)?;
    if request.replacement_candidate_id != proposed.candidate_id
        || proposed.revision <= active.revision
        || pending.is_some_and(|candidate| proposed.revision <= candidate.revision)
    {
        return Err(Error::StaleCoordinate);
    }
    if request
        .now
        .checked_add(policy.replacement_delay)
        .ok_or(Error::ArithmeticOverflow)?
        > proposed.valid_from
    {
        return Err(Error::StaleCoordinate);
    }
    let mut plan = Plan::empty();
    plan.push(
        CustodyRole::LivenessVault,
        CustodyRole::DealerOwner,
        state.pending_work_funding,
    )?;
    plan.push(
        CustodyRole::DealerOwner,
        CustodyRole::LivenessVault,
        proposed.work_funding,
    )?;
    state.pending_candidate_id = proposed.candidate_id;
    state.pending_revision = proposed.revision;
    state.pending_work_funding = proposed.work_funding;
    state.liveness_custody = state
        .active_work_remaining
        .checked_add(proposed.work_funding)
        .ok_or(Error::ArithmeticOverflow)?;
    bump_revision(&mut state)?;
    validate_state(policy, active, Some(proposed), state)?;
    Ok(Transition { post: state, plan })
}

fn activate(
    policy: Policy,
    pending: Option<CandidateView<'_>>,
    proposed: Option<CandidateView<'_>>,
    mut state: State,
    request: Request,
) -> Result<Transition> {
    if state.phase != Phase::Open || proposed.is_some() {
        return Err(Error::InvalidPhase);
    }
    let pending = pending.ok_or(Error::IdentityMismatch)?;
    if request.replacement_candidate_id != pending.candidate_id
        || request.now < pending.valid_from
        || request.now >= pending.expires_at
        || state.quote_custody < pending.quote_reserve_floor
    {
        return Err(Error::StaleCoordinate);
    }
    for outcome in 0..usize::from(policy.outcome_count) {
        if state.inventory[outcome] < pending.minimum_inventory(outcome)?
            || state.inventory[outcome] > pending.maximum_inventory(outcome)?
        {
            return Err(Error::InventoryRisk);
        }
    }
    let mut plan = Plan::empty();
    plan.push(
        CustodyRole::LivenessVault,
        CustodyRole::DealerOwner,
        state.active_work_remaining,
    )?;
    state.active_candidate_id = pending.candidate_id;
    state.active_revision = pending.revision;
    state.pending_candidate_id = [0; 32];
    state.pending_revision = 0;
    state.buy_used = [0; MAX_OUTCOMES];
    state.sell_used = [0; MAX_OUTCOMES];
    state.buy_quote_paid = [0; MAX_OUTCOMES];
    state.sell_quote_paid = [0; MAX_OUTCOMES];
    state.active_work_remaining = state.pending_work_funding;
    state.pending_work_funding = 0;
    state.liveness_custody = state.active_work_remaining;
    bump_revision(&mut state)?;
    validate_state(policy, pending, None, state)?;
    Ok(Transition { post: state, plan })
}

fn fill(
    policy: Policy,
    active: CandidateView<'_>,
    pending: Option<CandidateView<'_>>,
    mut state: State,
    request: Request,
) -> Result<Transition> {
    if state.phase != Phase::Open || request.now >= active.expires_at {
        return Err(Error::InvalidPhase);
    }
    let outcome = usize::from(request.outcome);
    if outcome >= usize::from(policy.outcome_count) || request.quantity == 0 {
        return Err(Error::InvalidCurve);
    }
    let (used, paid) = match request.side {
        Side::TakerBuys => (state.buy_used[outcome], state.buy_quote_paid[outcome]),
        Side::TakerSells => (state.sell_used[outcome], state.sell_quote_paid[outcome]),
    };
    let used_after = used
        .checked_add(request.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    if used_after > active.capacity(outcome, request.side)? {
        return Err(Error::InvalidCurve);
    }
    let due_after = active.cumulative_quote(policy, outcome, request.side, used_after)?;
    let gross = due_after.checked_sub(paid).ok_or(Error::InvalidCurve)?;
    if gross == 0 || state.active_work_remaining < active.work_reward {
        return Err(Error::Underfunded);
    }
    let fee_base_after = state
        .fee_base
        .checked_add(gross)
        .ok_or(Error::ArithmeticOverflow)?;
    let fee_paid_after = fee_due(policy, fee_base_after)?;
    let fee = fee_paid_after
        .checked_sub(state.fee_paid)
        .ok_or(Error::InvalidCurve)?;
    let inventory_after = match request.side {
        Side::TakerBuys => state.inventory[outcome]
            .checked_sub(request.quantity)
            .ok_or(Error::InventoryRisk)?,
        Side::TakerSells => state.inventory[outcome]
            .checked_add(request.quantity)
            .ok_or(Error::ArithmeticOverflow)?,
    };
    if inventory_after < active.minimum_inventory(outcome)?
        || inventory_after > active.maximum_inventory(outcome)?
    {
        return Err(Error::InventoryRisk);
    }
    let mut plan = Plan::empty();
    plan.claim = ClaimAction::Transfer {
        side: request.side,
        outcome: request.outcome,
        quantity: request.quantity,
    };
    match request.side {
        Side::TakerBuys => {
            state.quote_custody = state
                .quote_custody
                .checked_add(gross)
                .ok_or(Error::ArithmeticOverflow)?;
            state.buy_used[outcome] = used_after;
            state.buy_quote_paid[outcome] = due_after;
            plan.push(CustodyRole::TakerQuote, CustodyRole::DealerQuote, gross)?;
            plan.push(CustodyRole::TakerQuote, CustodyRole::FeeVault, fee)?;
        }
        Side::TakerSells => {
            let taker_proceeds = gross.checked_sub(fee).ok_or(Error::Underfunded)?;
            state.quote_custody = state
                .quote_custody
                .checked_sub(gross)
                .ok_or(Error::Underfunded)?;
            if state.quote_custody < active.quote_reserve_floor {
                return Err(Error::Underfunded);
            }
            state.sell_used[outcome] = used_after;
            state.sell_quote_paid[outcome] = due_after;
            plan.push(
                CustodyRole::DealerQuote,
                CustodyRole::TakerQuote,
                taker_proceeds,
            )?;
            plan.push(CustodyRole::DealerQuote, CustodyRole::FeeVault, fee)?;
        }
    }
    state.inventory[outcome] = inventory_after;
    state.fee_base = fee_base_after;
    state.fee_paid = fee_paid_after;
    state.fee_custody = state
        .fee_custody
        .checked_add(fee)
        .ok_or(Error::ArithmeticOverflow)?;
    state.active_work_remaining -= active.work_reward;
    state.liveness_custody = state
        .liveness_custody
        .checked_sub(active.work_reward)
        .ok_or(Error::Underfunded)?;
    plan.push(
        CustodyRole::LivenessVault,
        CustodyRole::Executor,
        active.work_reward,
    )?;
    bump_revision(&mut state)?;
    validate_state(policy, active, pending, state)?;
    Ok(Transition { post: state, plan })
}

fn enter_terminal(
    policy: Policy,
    active: CandidateView<'_>,
    mut state: State,
    request: Request,
) -> Result<Transition> {
    if state.phase != Phase::Open
        || request.actor_id != policy.market_id
        || usize::from(request.outcome) >= usize::from(policy.outcome_count)
    {
        return Err(Error::InvalidPhase);
    }
    let mut plan = Plan::empty();
    plan.push(
        CustodyRole::LivenessVault,
        CustodyRole::DealerOwner,
        state.pending_work_funding,
    )?;
    state.phase = Phase::Terminal;
    state.winner = request.outcome;
    state.pending_candidate_id = [0; 32];
    state.pending_revision = 0;
    state.pending_work_funding = 0;
    state.liveness_custody = state.active_work_remaining;
    bump_revision(&mut state)?;
    validate_state(policy, active, None, state)?;
    Ok(Transition { post: state, plan })
}

fn unwind(
    policy: Policy,
    active: CandidateView<'_>,
    mut state: State,
    request: Request,
) -> Result<Transition> {
    if state.phase != Phase::Terminal || state.active_work_remaining < active.work_reward {
        return Err(Error::InvalidPhase);
    }
    let outcome = usize::from(request.outcome);
    if outcome >= usize::from(policy.outcome_count) {
        return Err(Error::InvalidCurve);
    }
    state.inventory[outcome] = state.inventory[outcome]
        .checked_sub(request.quantity)
        .ok_or(Error::InventoryRisk)?;
    let payout = if request.outcome == state.winner {
        request.quantity
    } else {
        0
    };
    state.quote_custody = state
        .quote_custody
        .checked_add(payout)
        .ok_or(Error::ArithmeticOverflow)?;
    state.active_work_remaining -= active.work_reward;
    state.liveness_custody = state
        .liveness_custody
        .checked_sub(active.work_reward)
        .ok_or(Error::Underfunded)?;
    let mut plan = Plan::empty();
    plan.claim = ClaimAction::Redeem {
        outcome: request.outcome,
        quantity: request.quantity,
        payout,
    };
    plan.push(CustodyRole::MarketHoard, CustodyRole::DealerQuote, payout)?;
    plan.push(
        CustodyRole::LivenessVault,
        CustodyRole::Executor,
        active.work_reward,
    )?;
    bump_revision(&mut state)?;
    validate_state(policy, active, None, state)?;
    Ok(Transition { post: state, plan })
}

fn retire(policy: Policy, active: CandidateView<'_>, mut state: State) -> Result<Transition> {
    if state.phase != Phase::Terminal
        || state.inventory.iter().any(|quantity| *quantity != 0)
        || !is_zero(&state.pending_candidate_id)
    {
        return Err(Error::InvalidPhase);
    }
    let mut plan = Plan::empty();
    plan.push(
        CustodyRole::DealerQuote,
        CustodyRole::UnwindRecipient,
        state.quote_custody,
    )?;
    plan.push(
        CustodyRole::FeeVault,
        CustodyRole::FeeRecipient,
        state.fee_custody,
    )?;
    plan.push(
        CustodyRole::LivenessVault,
        CustodyRole::DealerOwner,
        state.active_work_remaining,
    )?;
    state.phase = Phase::Retired;
    state.winner = 0;
    state.quote_custody = 0;
    state.fee_custody = 0;
    state.liveness_custody = 0;
    state.active_work_remaining = 0;
    bump_revision(&mut state)?;
    validate_state(policy, active, None, state)?;
    Ok(Transition { post: state, plan })
}

fn adjust_liquidity(
    policy: Policy,
    active: CandidateView<'_>,
    pending: Option<CandidateView<'_>>,
    mut state: State,
    request: Request,
    add: bool,
) -> Result<Transition> {
    if state.phase != Phase::Open || request.actor_id != policy.dealer_id {
        return Err(Error::InvalidPhase);
    }
    let outcome = usize::from(request.outcome);
    let count = usize::from(policy.outcome_count);
    let mut plan = Plan::empty();
    if outcome == count {
        if add {
            state.quote_custody = state
                .quote_custody
                .checked_add(request.quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            plan.push(
                CustodyRole::DealerOwner,
                CustodyRole::DealerQuote,
                request.quantity,
            )?;
        } else {
            state.quote_custody = state
                .quote_custody
                .checked_sub(request.quantity)
                .ok_or(Error::Underfunded)?;
            if state.quote_custody < active.quote_reserve_floor {
                return Err(Error::Underfunded);
            }
            plan.push(
                CustodyRole::DealerQuote,
                CustodyRole::DealerOwner,
                request.quantity,
            )?;
        }
    } else if outcome < count {
        let current = state.inventory[outcome];
        let post = if add {
            current
                .checked_add(request.quantity)
                .ok_or(Error::ArithmeticOverflow)?
        } else {
            current
                .checked_sub(request.quantity)
                .ok_or(Error::InventoryRisk)?
        };
        if post < active.minimum_inventory(outcome)? || post > active.maximum_inventory(outcome)? {
            return Err(Error::InventoryRisk);
        }
        state.inventory[outcome] = post;
        plan.claim = ClaimAction::AdjustLiquidity {
            add,
            outcome: request.outcome,
            quantity: request.quantity,
        };
    } else {
        return Err(Error::InvalidCurve);
    }
    bump_revision(&mut state)?;
    validate_state(policy, active, pending, state)?;
    Ok(Transition { post: state, plan })
}

fn fee_due(policy: Policy, base: u64) -> Result<u64> {
    let numerator = u128::from(base) * u128::from(policy.fee_numerator);
    let denominator = u128::from(policy.fee_denominator);
    let rounded = numerator
        .checked_add(denominator - 1)
        .ok_or(Error::ArithmeticOverflow)?
        / denominator;
    u64_from(rounded)
}

fn bump_revision(state: &mut State) -> Result<()> {
    state.state_revision = state
        .state_revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(())
}

fn curve_offset(outcome: usize) -> usize {
    generated::CANDIDATE_CURVES_OFFSET + outcome * generated::OUTCOME_CURVE_BYTES
}

fn header(input: &[u8], width: usize, magic: &[u8; 8], version_offset: usize) -> Result<()> {
    exact_width(input, width)?;
    exact(input, 0, magic, Error::InvalidMagic)?;
    if u16_at(input, version_offset)? != generated::ABI_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

fn exact_width(input: &[u8], width: usize) -> Result<()> {
    if input.len() == width {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn checked_slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    input
        .get(offset..offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)
}

fn exact(input: &[u8], offset: usize, expected: &[u8], error: Error) -> Result<()> {
    if checked_slice(input, offset, expected.len())? == expected {
        Ok(())
    } else {
        Err(error)
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    if checked_slice(input, offset, width)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalPadding)
    }
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn bool_at(input: &[u8], offset: usize) -> Result<bool> {
    match byte_at(input, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::UnknownTag),
    }
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    let bytes: [u8; 2] = checked_slice(input, offset, 2)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u16::from_le_bytes(bytes))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    let bytes: [u8; 8] = checked_slice(input, offset, 8)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u64::from_le_bytes(bytes))
}

fn array_at(input: &[u8], offset: usize) -> Result<Identity> {
    checked_slice(input, offset, 32)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn u64_array_at(input: &[u8], offset: usize) -> Result<[u64; MAX_OUTCOMES]> {
    let mut values = [0_u64; MAX_OUTCOMES];
    for (index, value) in values.iter_mut().enumerate() {
        *value = u64_at(input, offset + index * 8)?;
    }
    Ok(values)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let target = output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::InvalidLength)?;
    target.copy_from_slice(value);
    Ok(())
}

fn put_header(output: &mut [u8], magic: &[u8; 8], version_offset: usize) -> Result<()> {
    put(output, 0, magic)?;
    put(
        output,
        version_offset,
        &generated::ABI_VERSION.to_le_bytes(),
    )
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    put(output, offset, &[value])
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) -> Result<()> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u64_array(output: &mut [u8], offset: usize, values: &[u64; MAX_OUTCOMES]) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        put_u64(output, offset + index * 8, *value)?;
    }
    Ok(())
}

fn is_zero(identity: &Identity) -> bool {
    identity.iter().all(|byte| *byte == 0)
}

fn u64_from(value: u128) -> Result<u64> {
    u64::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

#[cfg(test)]
mod tests;
