//! A pure, fixed-layout policy for resolving categorical price claims.
//!
//! This module deliberately represents only the small data surface needed by
//! the policy.  An adapter is responsible for authenticating and decoding a
//! Pyth V1 observation before constructing [`PythV1Observation`].

/// Maximum number of ordered price cells in this measured profile.
pub const MAX_PRICE_CELLS: usize = 15;

/// Error returned by Pyth V1 categorical policy validation or evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythV1Error {
    /// A required release or feed-profile identifier was all zero bytes.
    ZeroIdentifier,
    /// The normalized decimal precision exceeded the fixed profile bound.
    InvalidNormalizedDecimals,
    /// The requested number of price cells was outside one through fifteen.
    InvalidPriceCellCount,
    /// The failure outcome was not exactly the price-cell count.
    InvalidFailureOutcome,
    /// Active upper edges were not nonzero and strictly increasing.
    InvalidActiveEdges,
    /// Inactive upper-edge storage was not all zero.
    NonzeroEdgeTail,
    /// The maximum confidence basis-points bound was outside one through 10,000.
    InvalidConfidenceBps,
    /// The confidence multiplier was zero and would ignore provider confidence.
    ZeroConfidenceMultiplier,
    /// Checked arithmetic or conversion left the policy's integer domain.
    ArithmeticOverflow,
    /// The supplied prior and current publications did not advance in time.
    EqualOrReversedPublishTime,
    /// The supplied publication does not cross the target time as required.
    DoesNotCrossTarget,
    /// The crossing publication arrived too far after the target time.
    CrossingLagExceeded,
    /// The evaluation clock was outside the inclusive price-resolution window.
    OutsideResolutionWindow,
    /// The publication was older than the policy permits at the evaluation clock.
    ObservationTooOld,
    /// The publication was too far in the future at the evaluation clock.
    ObservationTooFarInFuture,
    /// The raw lower endpoint of the confidence interval was nonpositive.
    NonpositiveLowerEndpoint,
    /// Confidence exceeded the relative or normalized-atom policy cap.
    ConfidenceExceeded,
    /// The normalized interval crossed, touched, or lay outside every price cell.
    IntervalDoesNotFitCell,
    /// Permissionless failure may only resolve strictly after the price window.
    FailureTooEarly,
}

/// Result alias for this policy.
pub type PythV1Result<T> = core::result::Result<T, PythV1Error>;

/// Validated inputs used to construct [`CategoricalPythV1Policy`].
///
/// The identifiers are opaque adapter-profile commitments, not Pyth SDK types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalPythV1PolicyInput {
    /// Nonzero immutable adapter release identifier.
    pub pyth_release_id: [u8; 32],
    /// Nonzero immutable feed-profile identifier.
    pub feed_profile_id: [u8; 32],
    /// Target Unix timestamp that the observation must cross.
    pub target_time: i64,
    /// Delay after target time before price resolution begins, in seconds.
    pub grace: u32,
    /// Inclusive price-resolution duration after grace, in seconds.
    pub window: u32,
    /// Maximum allowed `publish_time - target_time`, in seconds.
    pub max_crossing_lag: u32,
    /// Maximum allowed clock age for a past publication, in seconds.
    pub max_age: u32,
    /// Maximum allowed publication lead over the clock, in seconds.
    pub max_future_skew: u32,
    /// Multiplier applied to the raw Pyth confidence value.
    pub confidence_multiplier: u16,
    /// Maximum normalized half-width as basis points of normalized price.
    pub max_confidence_bps: u16,
    /// Maximum normalized half-width in normalized price atoms.
    pub max_normalized_confidence_atoms: u128,
    /// Decimal precision of normalized price atoms.
    pub normalized_decimals: u8,
    /// Number of price cells before the one explicit failure outcome.
    pub price_cell_count: u16,
    /// Upper edges for bounded cells; inactive tail entries must be zero.
    pub upper_edges: [u128; MAX_PRICE_CELLS],
    /// The explicit failure-outcome index, which must equal `price_cell_count`.
    pub failure_outcome_index: u16,
}

/// Immutable validated policy for the first categorical price slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalPythV1Policy {
    pyth_release_id: [u8; 32],
    feed_profile_id: [u8; 32],
    target_time: i64,
    grace: u32,
    window: u32,
    max_crossing_lag: u32,
    max_age: u32,
    max_future_skew: u32,
    confidence_multiplier: u16,
    max_confidence_bps: u16,
    max_normalized_confidence_atoms: u128,
    normalized_decimals: u8,
    price_cell_count: u16,
    upper_edges: [u128; MAX_PRICE_CELLS],
    failure_outcome_index: u16,
}

impl CategoricalPythV1Policy {
    /// Validate and construct a policy with one explicit failure outcome.
    pub fn new(input: CategoricalPythV1PolicyInput) -> PythV1Result<Self> {
        if is_zero_identifier(&input.pyth_release_id) || is_zero_identifier(&input.feed_profile_id)
        {
            return Err(PythV1Error::ZeroIdentifier);
        }
        if input.normalized_decimals > 18 {
            return Err(PythV1Error::InvalidNormalizedDecimals);
        }
        if input.price_cell_count == 0 || usize::from(input.price_cell_count) > MAX_PRICE_CELLS {
            return Err(PythV1Error::InvalidPriceCellCount);
        }
        if input.failure_outcome_index != input.price_cell_count {
            return Err(PythV1Error::InvalidFailureOutcome);
        }
        if input.max_confidence_bps == 0 || input.max_confidence_bps > 10_000 {
            return Err(PythV1Error::InvalidConfidenceBps);
        }
        if input.confidence_multiplier == 0 {
            return Err(PythV1Error::ZeroConfidenceMultiplier);
        }
        validate_edges(input.price_cell_count, &input.upper_edges)?;

        Ok(Self {
            pyth_release_id: input.pyth_release_id,
            feed_profile_id: input.feed_profile_id,
            target_time: input.target_time,
            grace: input.grace,
            window: input.window,
            max_crossing_lag: input.max_crossing_lag,
            max_age: input.max_age,
            max_future_skew: input.max_future_skew,
            confidence_multiplier: input.confidence_multiplier,
            max_confidence_bps: input.max_confidence_bps,
            max_normalized_confidence_atoms: input.max_normalized_confidence_atoms,
            normalized_decimals: input.normalized_decimals,
            price_cell_count: input.price_cell_count,
            upper_edges: input.upper_edges,
            failure_outcome_index: input.failure_outcome_index,
        })
    }

    /// Return the opaque immutable Pyth adapter release identifier.
    pub const fn pyth_release_id(&self) -> [u8; 32] {
        self.pyth_release_id
    }

    /// Return the opaque immutable feed-profile identifier.
    pub const fn feed_profile_id(&self) -> [u8; 32] {
        self.feed_profile_id
    }

    /// Return the target Unix timestamp.
    pub const fn target_time(&self) -> i64 {
        self.target_time
    }

    /// Return the normalized price decimal precision.
    pub const fn normalized_decimals(&self) -> u8 {
        self.normalized_decimals
    }

    /// Return the number of price cells.
    pub const fn price_cell_count(&self) -> u16 {
        self.price_cell_count
    }

    /// Return all fixed-layout upper edges, including the validated zero tail.
    pub const fn upper_edges(&self) -> &[u128; MAX_PRICE_CELLS] {
        &self.upper_edges
    }

    /// Return the one explicit failure outcome index.
    pub const fn failure_outcome_index(&self) -> u16 {
        self.failure_outcome_index
    }

    /// Return the inclusive `(start, end)` price-resolution timestamps.
    pub fn resolution_window(&self) -> PythV1Result<(i64, i64)> {
        let start = self
            .target_time
            .checked_add(i64::from(self.grace))
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        let end = start
            .checked_add(i64::from(self.window))
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        Ok((start, end))
    }

    /// Resolve a valid crossing observation to exactly one price-cell outcome.
    ///
    /// The adapter supplies `clock_time`; this pure fold neither reads a clock
    /// nor changes ledger state.
    pub fn resolve_price(
        &self,
        clock_time: i64,
        observation: PythV1Observation,
    ) -> PythV1Result<CategoricalPythV1Resolution> {
        self.require_price_window(clock_time)?;
        self.validate_observation_time(clock_time, observation)?;

        let raw_half_width = i64::from(self.confidence_multiplier)
            .checked_mul(
                i64::try_from(observation.confidence)
                    .map_err(|_| PythV1Error::ArithmeticOverflow)?,
            )
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        let raw_lower = observation
            .price
            .checked_sub(raw_half_width)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        if raw_lower <= 0 {
            return Err(PythV1Error::NonpositiveLowerEndpoint);
        }
        let raw_upper = observation
            .price
            .checked_add(raw_half_width)
            .ok_or(PythV1Error::ArithmeticOverflow)?;

        let (normalized_lower, normalized_upper) = normalize_outward(
            raw_lower,
            raw_upper,
            observation.exponent,
            self.normalized_decimals,
        )?;
        let normalized_center = normalize_floor(
            observation.price,
            observation.exponent,
            self.normalized_decimals,
        )?;
        self.validate_confidence(
            observation.price,
            raw_half_width,
            normalized_lower,
            normalized_center,
            normalized_upper,
        )?;

        self.select_price_cell(normalized_lower, normalized_upper)
    }

    /// Resolve the explicit failure outcome strictly after the price window ends.
    pub fn resolve_failure(&self, clock_time: i64) -> PythV1Result<CategoricalPythV1Resolution> {
        let (_, end) = self.resolution_window()?;
        if clock_time <= end {
            return Err(PythV1Error::FailureTooEarly);
        }
        Ok(CategoricalPythV1Resolution {
            winner: self.failure_outcome_index,
        })
    }

    fn require_price_window(&self, clock_time: i64) -> PythV1Result<()> {
        let (start, end) = self.resolution_window()?;
        if clock_time < start || clock_time > end {
            return Err(PythV1Error::OutsideResolutionWindow);
        }
        Ok(())
    }

    fn validate_observation_time(
        &self,
        clock_time: i64,
        observation: PythV1Observation,
    ) -> PythV1Result<()> {
        if observation.prev_publish_time >= observation.publish_time {
            return Err(PythV1Error::EqualOrReversedPublishTime);
        }
        if observation.prev_publish_time >= self.target_time
            || observation.publish_time < self.target_time
        {
            return Err(PythV1Error::DoesNotCrossTarget);
        }
        let crossing_lag = observation
            .publish_time
            .checked_sub(self.target_time)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        if crossing_lag > i64::from(self.max_crossing_lag) {
            return Err(PythV1Error::CrossingLagExceeded);
        }

        let age = clock_time
            .checked_sub(observation.publish_time)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        let future_skew = observation
            .publish_time
            .checked_sub(clock_time)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        if age > i64::from(self.max_age) {
            return Err(PythV1Error::ObservationTooOld);
        }
        if future_skew > i64::from(self.max_future_skew) {
            return Err(PythV1Error::ObservationTooFarInFuture);
        }
        Ok(())
    }

    fn validate_confidence(
        &self,
        raw_price: i64,
        raw_half_width: i64,
        normalized_lower: u128,
        normalized_center: u128,
        normalized_upper: u128,
    ) -> PythV1Result<()> {
        let scaled_raw_half_width = u128::try_from(raw_half_width)
            .map_err(|_| PythV1Error::ArithmeticOverflow)?
            .checked_mul(10_000)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        let scaled_raw_price = u128::try_from(raw_price)
            .map_err(|_| PythV1Error::ArithmeticOverflow)?
            .checked_mul(u128::from(self.max_confidence_bps))
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        if scaled_raw_half_width > scaled_raw_price {
            return Err(PythV1Error::ConfidenceExceeded);
        }
        let lower_half_width = normalized_center
            .checked_sub(normalized_lower)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        let upper_half_width = normalized_upper
            .checked_sub(normalized_center)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        let normalized_half_width = if lower_half_width > upper_half_width {
            lower_half_width
        } else {
            upper_half_width
        };
        if normalized_half_width > self.max_normalized_confidence_atoms {
            return Err(PythV1Error::ConfidenceExceeded);
        }
        Ok(())
    }

    fn select_price_cell(
        &self,
        normalized_lower: u128,
        normalized_upper: u128,
    ) -> PythV1Result<CategoricalPythV1Resolution> {
        let cell_count = usize::from(self.price_cell_count);
        let mut lower_edge = 0u128;
        let mut index = 0usize;
        while index < cell_count {
            let upper_edge = self
                .upper_edges
                .get(index)
                .copied()
                .ok_or(PythV1Error::ArithmeticOverflow)?;
            let is_last = index
                .checked_add(1)
                .ok_or(PythV1Error::ArithmeticOverflow)?
                == cell_count;
            let contained =
                normalized_lower >= lower_edge && (is_last || normalized_upper < upper_edge);
            if contained {
                return Ok(CategoricalPythV1Resolution {
                    winner: u16::try_from(index).map_err(|_| PythV1Error::ArithmeticOverflow)?,
                });
            }
            if !is_last && normalized_lower < upper_edge && normalized_upper >= upper_edge {
                return Err(PythV1Error::IntervalDoesNotFitCell);
            }
            lower_edge = upper_edge;
            index = index
                .checked_add(1)
                .ok_or(PythV1Error::ArithmeticOverflow)?;
        }
        Err(PythV1Error::IntervalDoesNotFitCell)
    }
}

/// A raw Pyth V1-shaped observation, decoded by an untrusted adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythV1Observation {
    /// Previous publication Unix timestamp.
    pub prev_publish_time: i64,
    /// Current publication Unix timestamp.
    pub publish_time: i64,
    /// Signed raw price mantissa.
    pub price: i64,
    /// Unsigned raw confidence half-width before policy multiplication.
    pub confidence: u64,
    /// Base-ten exponent shared by price and confidence.
    pub exponent: i32,
}

/// A categorical winner selected by this policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalPythV1Resolution {
    winner: u16,
}

impl CategoricalPythV1Resolution {
    /// Return the selected price-cell or explicit failure outcome index.
    pub const fn winner(&self) -> u16 {
        self.winner
    }
}

fn is_zero_identifier(identifier: &[u8; 32]) -> bool {
    for byte in identifier {
        if *byte != 0 {
            return false;
        }
    }
    true
}

fn validate_edges(
    price_cell_count: u16,
    upper_edges: &[u128; MAX_PRICE_CELLS],
) -> PythV1Result<()> {
    let active_edge_count = usize::from(price_cell_count)
        .checked_sub(1)
        .ok_or(PythV1Error::InvalidPriceCellCount)?;
    let mut previous = 0u128;
    let mut index = 0usize;
    while index < MAX_PRICE_CELLS {
        let edge = upper_edges
            .get(index)
            .copied()
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        if index < active_edge_count {
            if edge == 0 || edge <= previous {
                return Err(PythV1Error::InvalidActiveEdges);
            }
            previous = edge;
        } else if edge != 0 {
            return Err(PythV1Error::NonzeroEdgeTail);
        }
        index = index
            .checked_add(1)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
    }
    Ok(())
}

/// Normalize a positive raw interval using one outward rounding boundary.
///
/// For a negative decimal shift, the lower endpoint is rounded down and the
/// upper endpoint is rounded up.  This is the policy's only decimal rounding
/// boundary; all later comparisons use exact integer atoms.
fn normalize_outward(
    raw_lower: i64,
    raw_upper: i64,
    exponent: i32,
    normalized_decimals: u8,
) -> PythV1Result<(u128, u128)> {
    let shift = exponent
        .checked_add(i32::from(normalized_decimals))
        .ok_or(PythV1Error::ArithmeticOverflow)?;
    let lower = u128::try_from(raw_lower).map_err(|_| PythV1Error::ArithmeticOverflow)?;
    let upper = u128::try_from(raw_upper).map_err(|_| PythV1Error::ArithmeticOverflow)?;
    if shift >= 0 {
        let factor =
            ten_to_the(u32::try_from(shift).map_err(|_| PythV1Error::ArithmeticOverflow)?)?;
        return Ok((
            lower
                .checked_mul(factor)
                .ok_or(PythV1Error::ArithmeticOverflow)?,
            upper
                .checked_mul(factor)
                .ok_or(PythV1Error::ArithmeticOverflow)?,
        ));
    }
    let divisor = ten_to_the(
        u32::try_from(shift.checked_neg().ok_or(PythV1Error::ArithmeticOverflow)?)
            .map_err(|_| PythV1Error::ArithmeticOverflow)?,
    )?;
    let lower_normalized = lower / divisor;
    let upper_quotient = upper / divisor;
    let upper_remainder = upper % divisor;
    let upper_normalized = if upper_remainder == 0 {
        upper_quotient
    } else {
        upper_quotient
            .checked_add(1)
            .ok_or(PythV1Error::ArithmeticOverflow)?
    };
    Ok((lower_normalized, upper_normalized))
}

fn normalize_floor(raw_value: i64, exponent: i32, normalized_decimals: u8) -> PythV1Result<u128> {
    let shift = exponent
        .checked_add(i32::from(normalized_decimals))
        .ok_or(PythV1Error::ArithmeticOverflow)?;
    let value = u128::try_from(raw_value).map_err(|_| PythV1Error::ArithmeticOverflow)?;
    if shift >= 0 {
        return value
            .checked_mul(ten_to_the(
                u32::try_from(shift).map_err(|_| PythV1Error::ArithmeticOverflow)?,
            )?)
            .ok_or(PythV1Error::ArithmeticOverflow);
    }
    Ok(value
        / ten_to_the(
            u32::try_from(shift.checked_neg().ok_or(PythV1Error::ArithmeticOverflow)?)
                .map_err(|_| PythV1Error::ArithmeticOverflow)?,
        )?)
}

fn ten_to_the(exponent: u32) -> PythV1Result<u128> {
    let mut result = 1u128;
    let mut remaining = exponent;
    while remaining != 0 {
        result = result
            .checked_mul(10)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
        remaining = remaining
            .checked_sub(1)
            .ok_or(PythV1Error::ArithmeticOverflow)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> CategoricalPythV1PolicyInput {
        let mut upper_edges = [0u128; MAX_PRICE_CELLS];
        set_edge(&mut upper_edges, 0, 100);
        set_edge(&mut upper_edges, 1, 200);
        CategoricalPythV1PolicyInput {
            pyth_release_id: [1; 32],
            feed_profile_id: [2; 32],
            target_time: 100,
            grace: 5,
            window: 10,
            max_crossing_lag: 5,
            max_age: 20,
            max_future_skew: 5,
            confidence_multiplier: 1,
            max_confidence_bps: 10_000,
            max_normalized_confidence_atoms: 1_000,
            normalized_decimals: 0,
            price_cell_count: 3,
            upper_edges,
            failure_outcome_index: 3,
        }
    }

    fn observation(price: i64, confidence: u64, exponent: i32) -> PythV1Observation {
        PythV1Observation {
            prev_publish_time: 99,
            publish_time: 100,
            price,
            confidence,
            exponent,
        }
    }

    fn set_edge(edges: &mut [u128; MAX_PRICE_CELLS], index: usize, value: u128) {
        if let Some(edge) = edges.get_mut(index) {
            *edge = value;
        }
    }

    #[test]
    fn policy_rejects_invalid_shape_identifiers_and_bounds() {
        let mut candidate = input();
        candidate.pyth_release_id = [0; 32];
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::ZeroIdentifier)
        );
        candidate = input();
        candidate.feed_profile_id = [0; 32];
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::ZeroIdentifier)
        );
        candidate = input();
        candidate.normalized_decimals = 19;
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::InvalidNormalizedDecimals)
        );
        candidate = input();
        candidate.price_cell_count = 0;
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::InvalidPriceCellCount)
        );
        candidate = input();
        candidate.price_cell_count = 16;
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::InvalidPriceCellCount)
        );
        candidate = input();
        candidate.failure_outcome_index = 2;
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::InvalidFailureOutcome)
        );
        candidate = input();
        candidate.max_confidence_bps = 0;
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::InvalidConfidenceBps)
        );
        candidate = input();
        candidate.max_confidence_bps = 10_001;
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::InvalidConfidenceBps)
        );
        candidate = input();
        candidate.confidence_multiplier = 0;
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::ZeroConfidenceMultiplier)
        );
    }

    #[test]
    fn policy_rejects_noncanonical_edge_shapes() {
        let mut candidate = input();
        set_edge(&mut candidate.upper_edges, 0, 0);
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::InvalidActiveEdges)
        );
        candidate = input();
        set_edge(&mut candidate.upper_edges, 1, 100);
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::InvalidActiveEdges)
        );
        candidate = input();
        set_edge(&mut candidate.upper_edges, 2, 1);
        assert_eq!(
            CategoricalPythV1Policy::new(candidate),
            Err(PythV1Error::NonzeroEdgeTail)
        );
    }

    #[test]
    fn crossing_endpoints_and_equal_timestamps_refuse() -> PythV1Result<()> {
        let policy = CategoricalPythV1Policy::new(input())?;
        let mut candidate = observation(50, 0, 0);
        candidate.prev_publish_time = 100;
        assert_eq!(
            policy.resolve_price(105, candidate),
            Err(PythV1Error::EqualOrReversedPublishTime)
        );
        candidate = observation(50, 0, 0);
        candidate.prev_publish_time = 98;
        candidate.publish_time = 99;
        assert_eq!(
            policy.resolve_price(105, candidate),
            Err(PythV1Error::DoesNotCrossTarget)
        );
        candidate = observation(50, 0, 0);
        candidate.prev_publish_time = 100;
        candidate.publish_time = 100;
        assert_eq!(
            policy.resolve_price(105, candidate),
            Err(PythV1Error::EqualOrReversedPublishTime)
        );
        candidate = observation(50, 0, 0);
        candidate.publish_time = 106;
        assert_eq!(
            policy.resolve_price(105, candidate),
            Err(PythV1Error::CrossingLagExceeded)
        );
        Ok(())
    }

    #[test]
    fn time_window_freshness_and_overflow_are_total() -> PythV1Result<()> {
        let policy = CategoricalPythV1Policy::new(input())?;
        assert_eq!(
            policy.resolve_price(104, observation(50, 0, 0)),
            Err(PythV1Error::OutsideResolutionWindow)
        );
        assert_eq!(
            policy.resolve_price(105, observation(50, 0, 0))?.winner(),
            0
        );
        assert_eq!(
            policy.resolve_price(115, observation(50, 0, 0))?.winner(),
            0
        );
        assert_eq!(
            policy.resolve_price(116, observation(50, 0, 0)),
            Err(PythV1Error::OutsideResolutionWindow)
        );
        assert_eq!(
            policy.resolve_price(121, observation(50, 0, 0)),
            Err(PythV1Error::OutsideResolutionWindow)
        );

        let mut stale = observation(50, 0, 0);
        stale.publish_time = 100;
        assert_eq!(policy.resolve_price(115, stale)?.winner(), 0);
        assert_eq!(
            policy.resolve_price(116, stale),
            Err(PythV1Error::OutsideResolutionWindow)
        );
        let mut future = observation(50, 0, 0);
        future.publish_time = 104;
        assert_eq!(policy.resolve_price(105, future)?.winner(), 0);
        future.publish_time = 111;
        assert_eq!(
            policy.resolve_price(105, future),
            Err(PythV1Error::CrossingLagExceeded)
        );

        let mut overflowing = input();
        overflowing.target_time = i64::MAX;
        assert_eq!(
            CategoricalPythV1Policy::new(overflowing)?.resolution_window(),
            Err(PythV1Error::ArithmeticOverflow)
        );
        let mut end_overflowing = input();
        end_overflowing.target_time = i64::MAX;
        end_overflowing.grace = 0;
        end_overflowing.window = 1;
        assert_eq!(
            CategoricalPythV1Policy::new(end_overflowing)?.resolution_window(),
            Err(PythV1Error::ArithmeticOverflow)
        );
        let mut underflowing = input();
        underflowing.target_time = 0;
        let underflow_policy = CategoricalPythV1Policy::new(underflowing)?;
        let mut extreme = observation(50, 0, 0);
        extreme.prev_publish_time = -1;
        extreme.publish_time = 0;
        assert_eq!(
            underflow_policy.validate_observation_time(i64::MIN, extreme),
            Err(PythV1Error::ArithmeticOverflow)
        );
        extreme.prev_publish_time = i64::MIN;
        extreme.publish_time = 0;
        assert_eq!(
            underflow_policy.validate_observation_time(i64::MAX, extreme),
            Err(PythV1Error::ObservationTooOld)
        );
        let mut stale_policy_input = input();
        stale_policy_input.max_age = 4;
        let stale_policy = CategoricalPythV1Policy::new(stale_policy_input)?;
        assert_eq!(
            stale_policy.resolve_price(105, observation(50, 0, 0)),
            Err(PythV1Error::ObservationTooOld)
        );
        let mut future_policy_input = input();
        future_policy_input.grace = 0;
        future_policy_input.max_future_skew = 3;
        let future_policy = CategoricalPythV1Policy::new(future_policy_input)?;
        let mut too_future = observation(50, 0, 0);
        too_future.publish_time = 104;
        assert_eq!(
            future_policy.resolve_price(100, too_future),
            Err(PythV1Error::ObservationTooFarInFuture)
        );
        Ok(())
    }

    #[test]
    fn outward_rounding_handles_both_directions_and_bad_exponents() -> PythV1Result<()> {
        let mut candidate = input();
        candidate.normalized_decimals = 2;
        set_edge(&mut candidate.upper_edges, 0, 99);
        set_edge(&mut candidate.upper_edges, 1, 199);
        let policy = CategoricalPythV1Policy::new(candidate)?;
        assert_eq!(policy.resolve_price(105, observation(1, 0, 0))?.winner(), 1);
        assert_eq!(
            policy.resolve_price(105, observation(150, 0, -2))?.winner(),
            1
        );
        assert_eq!(normalize_outward(101, 109, -2, 0)?, (1, 2));
        assert_eq!(normalize_outward(2, 3, 1, 0)?, (20, 30));
        assert_eq!(
            normalize_outward(1, 2, i32::MIN, 0),
            Err(PythV1Error::ArithmeticOverflow)
        );
        assert_eq!(
            normalize_outward(i64::MAX, i64::MAX, 2, 18),
            Err(PythV1Error::ArithmeticOverflow)
        );
        Ok(())
    }

    #[test]
    fn nonpositive_interval_and_confidence_caps_refuse() -> PythV1Result<()> {
        let policy = CategoricalPythV1Policy::new(input())?;
        assert_eq!(
            policy.resolve_price(105, observation(2, 2, 0)),
            Err(PythV1Error::NonpositiveLowerEndpoint)
        );
        assert_eq!(
            policy.resolve_price(105, observation(2, u64::MAX, 0)),
            Err(PythV1Error::ArithmeticOverflow)
        );
        let mut bounded = input();
        bounded.max_normalized_confidence_atoms = 4;
        let bounded = CategoricalPythV1Policy::new(bounded)?;
        assert_eq!(
            bounded.resolve_price(105, observation(50, 5, 0)),
            Err(PythV1Error::ConfidenceExceeded)
        );
        let mut bps = input();
        bps.max_confidence_bps = 100;
        let bps = CategoricalPythV1Policy::new(bps)?;
        assert_eq!(
            bps.resolve_price(105, observation(50, 1, 0)),
            Err(PythV1Error::ConfidenceExceeded)
        );
        Ok(())
    }

    #[test]
    fn raw_bps_cap_does_not_reject_coarse_outward_normalization() -> PythV1Result<()> {
        let mut candidate = input();
        candidate.max_confidence_bps = 100;
        let policy = CategoricalPythV1Policy::new(candidate)?;
        assert_eq!(
            policy.resolve_price(105, observation(101, 1, -2))?.winner(),
            0
        );
        Ok(())
    }

    #[test]
    fn exact_edges_straddles_and_every_cell_are_canonical() -> PythV1Result<()> {
        let policy = CategoricalPythV1Policy::new(input())?;
        assert_eq!(
            policy.resolve_price(105, observation(99, 0, 0))?.winner(),
            0
        );
        assert_eq!(
            policy.resolve_price(105, observation(100, 0, 0))?.winner(),
            1
        );
        assert_eq!(
            policy.resolve_price(105, observation(150, 0, 0))?.winner(),
            1
        );
        assert_eq!(
            policy.resolve_price(105, observation(200, 0, 0))?.winner(),
            2
        );
        assert_eq!(
            policy.resolve_price(105, observation(201, 0, 0))?.winner(),
            2
        );
        assert_eq!(
            policy.resolve_price(105, observation(99, 1, 0)),
            Err(PythV1Error::IntervalDoesNotFitCell)
        );
        Ok(())
    }

    #[test]
    fn failure_is_permissionless_only_after_inclusive_window() -> PythV1Result<()> {
        let policy = CategoricalPythV1Policy::new(input())?;
        assert_eq!(
            policy.resolve_failure(114),
            Err(PythV1Error::FailureTooEarly)
        );
        assert_eq!(
            policy.resolve_failure(115),
            Err(PythV1Error::FailureTooEarly)
        );
        assert_eq!(policy.resolve_failure(116)?.winner(), 3);
        Ok(())
    }
}
