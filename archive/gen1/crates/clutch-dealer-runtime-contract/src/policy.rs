// SPDX-License-Identifier: AGPL-3.0-or-later

use core::convert::TryFrom;

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    mul, validate_padding_u64, Error, FixedCodec, Id, Result, DEALER_POLICY_CONTENT_DOMAIN_V1,
    MAX_ATOMS, MAX_LP_PAGES, MAX_OUTCOMES, MAX_PRICE_DENOMINATOR,
};

/// Local semantic-body magic; this is not a global account discriminator.
pub const DEALER_POLICY_MAGIC_V1: [u8; 8] = *b"DCDPOLV1";
/// Exact local semantic-body version.
pub const DEALER_POLICY_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical `DealerPolicyV1` body.
pub const DEALER_POLICY_BYTES_V1: usize =
    HEADER_BYTES + (16 * 32) + 8 + (12 * 8) + (4 * MAX_OUTCOMES * 8) + 8;

/// Immutable covered-dealer policy and external semantic bindings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPolicyV1 {
    /// Immutable Realm identity selecting collateral semantics.
    pub realm_id: Id,
    /// Immutable capability Profile identity.
    pub profile_id: Id,
    /// Full successor `MarketInstanceV2Id`, never a lowered legacy market key.
    pub market_instance_v2_id: Id,
    /// Native claim-basis identity.
    pub claim_basis_id: Id,
    /// Exact collateral mint identity.
    pub collateral_mint: Id,
    /// Exact token-program identity for the admitted collateral profile.
    pub token_program: Id,
    /// Reviewed Hoard custody-semantics identity.
    pub hoard_custody_semantics_id: Id,
    /// Exact RelationV2 semantic identity.
    pub relation_v2_id: Id,
    /// Exact quantized price-measure policy identity.
    pub price_measure_policy_id: Id,
    /// Exact signed quadratic curve policy identity.
    pub curve_policy_id: Id,
    /// Checker/policy identity governing generation-specific curve-price certificates.
    pub curve_price_certificate_policy_id: Id,
    /// Exact fee-policy identity.
    pub fee_policy_id: Id,
    /// Exact prepaid-liveness-policy identity.
    pub liveness_policy_id: Id,
    /// Exact counted-retirement-policy identity.
    pub retirement_policy_id: Id,
    /// Immutable Realm/Profile sink for hostile prefunds and account surplus.
    pub neutral_sink: Id,
    /// Authority whose authenticated quote artifact may be leased.
    pub quote_authority: Id,
    /// Active native Egg width.
    pub outcome_count: u8,
    /// Exact native payout denominator and conservative lot.
    pub payout_denominator: u64,
    /// Cash atoms contributed by one immutable LP share unit.
    pub capital_unit_cash_atoms: u64,
    /// Existing, already-backed Eggs contributed by one LP share unit.
    pub capital_unit_eggs: [u64; MAX_OUTCOMES],
    /// Denominator shared by the immutable initial-price weights.
    pub initial_price_denominator: u64,
    /// Initial-price weights followed by canonical zero padding.
    pub initial_price_weights: [u64; MAX_OUTCOMES],
    /// Immutable quadratic depth in raw Egg atoms.
    pub depth_atoms: u64,
    /// Maximum net Eggs bought by the dealer per outcome.
    pub max_net_buy: [u64; MAX_OUTCOMES],
    /// Maximum net Eggs sold by the dealer per outcome.
    pub max_net_sell: [u64; MAX_OUTCOMES],
    /// Minimum exact share units required for activation.
    pub minimum_lp_shares: u64,
    /// Maximum exact share units admitted by the policy.
    pub maximum_lp_shares: u64,
    /// First slot at which funding closes.
    pub funding_deadline_slot: u64,
    /// First slot at which ordinary two-sided trading is admitted.
    pub trading_open_slot: u64,
    /// First slot at which timed unwind-only mode is available.
    pub trading_close_slot: u64,
    /// First slot at which authenticated resolution is admitted.
    pub maturity_slot: u64,
    /// Queued-share numerator required for unwind-only mode.
    pub shutdown_queue_numerator: u64,
    /// Queued-share denominator required for unwind-only mode.
    pub shutdown_queue_denominator: u64,
    /// Maximum counted LP pages in this dealer graph.
    pub maximum_lp_pages: u32,
}

impl DealerPolicyV1 {
    /// Validate identities, canonical padding, exact simplex, signed box,
    /// capitalization geometry, page bound, and immutable schedule.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.realm_id,
            self.profile_id,
            self.market_instance_v2_id,
            self.claim_basis_id,
            self.collateral_mint,
            self.token_program,
            self.hoard_custody_semantics_id,
            self.relation_v2_id,
            self.price_measure_policy_id,
            self.curve_policy_id,
            self.curve_price_certificate_policy_id,
            self.fee_policy_id,
            self.liveness_policy_id,
            self.retirement_policy_id,
            self.neutral_sink,
            self.quote_authority,
        ];
        let mut identity_index = 0usize;
        while identity_index < identities.len() {
            identities[identity_index].validate_live()?;
            identity_index += 1;
        }

        let n = usize::from(self.outcome_count);
        if self.outcome_count < 2 || n > MAX_OUTCOMES {
            return Err(Error::InvalidParameter);
        }
        if self.payout_denominator == 0
            || self.initial_price_denominator == 0
            || self.depth_atoms == 0
            || self.minimum_lp_shares == 0
            || self.maximum_lp_shares == 0
            || self.shutdown_queue_numerator == 0
            || self.shutdown_queue_denominator == 0
            || self.maximum_lp_pages == 0
        {
            return Err(Error::InvalidParameter);
        }
        if self.payout_denominator > MAX_ATOMS
            || self.initial_price_denominator > MAX_PRICE_DENOMINATOR
            || self.depth_atoms > MAX_ATOMS
            || self.capital_unit_cash_atoms > MAX_ATOMS
            || self.maximum_lp_shares > MAX_ATOMS
            || self.minimum_lp_shares > self.maximum_lp_shares
            || self.shutdown_queue_numerator > self.shutdown_queue_denominator
            || self.shutdown_queue_denominator > MAX_ATOMS
            || self.maximum_lp_pages > MAX_LP_PAGES
        {
            return Err(Error::InvalidParameter);
        }
        if self.funding_deadline_slot == 0
            || self.funding_deadline_slot > self.trading_open_slot
            || self.trading_open_slot >= self.trading_close_slot
            || self.trading_close_slot >= self.maturity_slot
        {
            return Err(Error::InvalidSchedule);
        }

        validate_padding_u64(self.outcome_count, &self.capital_unit_eggs)?;
        validate_padding_u64(self.outcome_count, &self.initial_price_weights)?;
        validate_padding_u64(self.outcome_count, &self.max_net_buy)?;
        validate_padding_u64(self.outcome_count, &self.max_net_sell)?;

        let mut price_sum = 0u128;
        let mut has_capital = self.capital_unit_cash_atoms != 0;
        let mut has_flow = false;
        let mut index = 0usize;
        while index < n {
            price_sum = price_sum
                .checked_add(u128::from(self.initial_price_weights[index]))
                .ok_or(Error::ArithmeticOverflow)?;
            let values = [
                self.capital_unit_eggs[index],
                self.max_net_buy[index],
                self.max_net_sell[index],
            ];
            let mut value_index = 0usize;
            while value_index < values.len() {
                let value = values[value_index];
                if value > MAX_ATOMS || !value.is_multiple_of(self.payout_denominator) {
                    return Err(Error::InvalidParameter);
                }
                value_index += 1;
            }
            has_capital |= self.capital_unit_eggs[index] != 0;
            has_flow |= self.max_net_buy[index] != 0 || self.max_net_sell[index] != 0;

            let minimum_eggs = mul(self.capital_unit_eggs[index], self.minimum_lp_shares)?;
            let maximum_eggs = mul(self.capital_unit_eggs[index], self.maximum_lp_shares)?;
            if minimum_eggs < self.max_net_sell[index]
                || maximum_eggs
                    .checked_add(self.max_net_buy[index])
                    .ok_or(Error::ArithmeticOverflow)?
                    > MAX_ATOMS
            {
                return Err(Error::InvalidParameter);
            }
            index += 1;
        }
        if price_sum != u128::from(self.initial_price_denominator) || !has_capital || !has_flow {
            return Err(Error::InvalidParameter);
        }
        if mul(self.capital_unit_cash_atoms, self.maximum_lp_shares)? > MAX_ATOMS {
            return Err(Error::InvalidParameter);
        }
        self.validate_full_price_box()
    }

    /// Validate one signed inventory against the exact box, lot, padding, and
    /// nonnegative-price domain frozen by this policy.
    pub fn validate_net_sold(&self, net_sold: &[i64; MAX_OUTCOMES]) -> Result<()> {
        self.validate()?;
        crate::validate_padding_i64(self.outcome_count, net_sold)?;
        let n = i128::from(self.outcome_count);
        let depth = i128::from(self.depth_atoms);
        let price_denominator = i128::from(self.initial_price_denominator);
        let mut sum = 0i128;
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            let value = i128::from(net_sold[index]);
            if value < -i128::from(self.max_net_buy[index])
                || value > i128::from(self.max_net_sell[index])
                || value % i128::from(self.payout_denominator) != 0
            {
                return Err(Error::InvalidParameter);
            }
            sum = sum.checked_add(value).ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        index = 0;
        while index < usize::from(self.outcome_count) {
            let initial = i128::from(self.initial_price_weights[index])
                .checked_mul(depth)
                .and_then(|value| value.checked_mul(n))
                .ok_or(Error::ArithmeticOverflow)?;
            let displacement = price_denominator
                .checked_mul(
                    n.checked_mul(i128::from(net_sold[index]))
                        .and_then(|value| value.checked_sub(sum))
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            if initial
                .checked_add(displacement)
                .ok_or(Error::ArithmeticOverflow)?
                < 0
            {
                return Err(Error::InvalidParameter);
            }
            index += 1;
        }
        Ok(())
    }

    /// Whether an exact queued-share total reaches the immutable shutdown
    /// quorum. A zero-share facility never meets the threshold.
    pub fn shutdown_queue_threshold_met(&self, queued: u64, total: u64) -> Result<bool> {
        self.validate()?;
        if queued > total || total > self.maximum_lp_shares {
            return Err(Error::InvalidParameter);
        }
        self.shutdown_queue_threshold_met_validated(queued, total)
    }

    pub(crate) fn shutdown_queue_threshold_met_validated(
        &self,
        queued: u64,
        total: u64,
    ) -> Result<bool> {
        if total == 0 {
            return Ok(false);
        }
        let left = u128::from(queued)
            .checked_mul(u128::from(self.shutdown_queue_denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        let right = u128::from(total)
            .checked_mul(u128::from(self.shutdown_queue_numerator))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(left >= right)
    }

    /// Exact present subsidy covering the quadratic curve's vertex loss bound.
    pub fn minimum_sponsor_subsidy(&self) -> Result<u64> {
        self.validate()?;
        let denominator = u128::from(self.initial_price_denominator);
        let denominator_squared = denominator
            .checked_mul(denominator)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut sum_squares = 0u128;
        let mut maximum_distance_numerator = 0u128;
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            let weight = u128::from(self.initial_price_weights[index]);
            sum_squares = sum_squares
                .checked_add(
                    weight
                        .checked_mul(weight)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        index = 0;
        while index < usize::from(self.outcome_count) {
            let twice_vertex_dot = denominator
                .checked_mul(u128::from(self.initial_price_weights[index]))
                .and_then(|value| value.checked_mul(2))
                .ok_or(Error::ArithmeticOverflow)?;
            let distance_numerator = denominator_squared
                .checked_add(sum_squares)
                .and_then(|value| value.checked_sub(twice_vertex_dot))
                .ok_or(Error::ConservationFailure)?;
            maximum_distance_numerator =
                core::cmp::max(maximum_distance_numerator, distance_numerator);
            index += 1;
        }
        let numerator = u128::from(self.depth_atoms)
            .checked_mul(maximum_distance_numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        let divisor = denominator_squared
            .checked_mul(2)
            .ok_or(Error::ArithmeticOverflow)?;
        u64::try_from(ceil_div_u128(numerator, divisor)?).map_err(|_| Error::ArithmeticOverflow)
    }

    /// Least sponsor capital satisfying both loss and lower-corner bid financing.
    pub fn minimum_sponsor_capital(&self) -> Result<u64> {
        self.validate()?;
        let loss_subsidy = self.minimum_sponsor_subsidy()?;
        let minimum_lp_cash = mul(self.capital_unit_cash_atoms, self.minimum_lp_shares)?;
        let mut lower_corner = [0i64; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            lower_corner[index] =
                -i64::try_from(self.max_net_buy[index]).map_err(|_| Error::ArithmeticOverflow)?;
            index += 1;
        }
        let lower_potential = self.signed_rounded_potential(&lower_corner)?;
        let without_sponsor = i128::from(minimum_lp_cash)
            .checked_add(i128::from(lower_potential))
            .ok_or(Error::ArithmeticOverflow)?;
        let financing = if without_sponsor >= 0 {
            0
        } else {
            u64::try_from(
                without_sponsor
                    .checked_neg()
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .map_err(|_| Error::ArithmeticOverflow)?
        };
        Ok(core::cmp::max(loss_subsidy, financing))
    }

    /// Canonical signed integer endpoint potential `ceil(C(q))` for one
    /// already lot-constrained inventory vector.
    pub fn signed_rounded_potential(&self, net_sold: &[i64; MAX_OUTCOMES]) -> Result<i64> {
        self.validate_net_sold(net_sold)?;
        let n = i128::from(self.outcome_count);
        let depth = i128::from(self.depth_atoms);
        let price_denominator = i128::from(self.initial_price_denominator);
        let mut sum = 0i128;
        let mut sum_squares = 0i128;
        let mut initial_dot = 0i128;
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            let value = i128::from(net_sold[index]);
            sum = sum.checked_add(value).ok_or(Error::ArithmeticOverflow)?;
            sum_squares = sum_squares
                .checked_add(value.checked_mul(value).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            initial_dot = initial_dot
                .checked_add(
                    value
                        .checked_mul(i128::from(self.initial_price_weights[index]))
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        let variance = n
            .checked_mul(sum_squares)
            .and_then(|value| value.checked_sub(sum.checked_mul(sum)?))
            .ok_or(Error::ConservationFailure)?;
        let linear = depth
            .checked_mul(2)
            .and_then(|value| value.checked_mul(n))
            .and_then(|value| value.checked_mul(initial_dot))
            .ok_or(Error::ArithmeticOverflow)?;
        let quadratic = variance
            .checked_mul(price_denominator)
            .ok_or(Error::ArithmeticOverflow)?;
        let numerator = linear
            .checked_add(quadratic)
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = depth
            .checked_mul(2)
            .and_then(|value| value.checked_mul(n))
            .and_then(|value| value.checked_mul(price_denominator))
            .ok_or(Error::ArithmeticOverflow)?;
        i64::try_from(ceil_div_i128(numerator, denominator)?).map_err(|_| Error::ArithmeticOverflow)
    }

    fn validate_full_price_box(&self) -> Result<()> {
        let n = u128::from(self.outcome_count);
        let denominator = u128::from(self.initial_price_denominator);
        let depth = u128::from(self.depth_atoms);
        let mut total_sell = 0u128;
        let mut index = 0usize;
        while index < usize::from(self.outcome_count) {
            total_sell = total_sell
                .checked_add(u128::from(self.max_net_sell[index]))
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        index = 0;
        while index < usize::from(self.outcome_count) {
            let initial = u128::from(self.initial_price_weights[index])
                .checked_mul(depth)
                .and_then(|value| value.checked_mul(n))
                .ok_or(Error::ArithmeticOverflow)?;
            let own_buy = u128::from(self.max_net_buy[index])
                .checked_mul(n - 1)
                .ok_or(Error::ArithmeticOverflow)?;
            let other_sell = total_sell
                .checked_sub(u128::from(self.max_net_sell[index]))
                .ok_or(Error::ConservationFailure)?;
            let displacement = denominator
                .checked_mul(
                    own_buy
                        .checked_add(other_sell)
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            if initial < displacement {
                return Err(Error::InvalidParameter);
            }
            index += 1;
        }
        Ok(())
    }

    /// Canonical policy identity, `SHA256(policy_domain || body)`.
    pub fn policy_id(&self) -> Result<Id> {
        self.content_id(DEALER_POLICY_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for DealerPolicyV1 {
    const ENCODED_LEN: usize = DEALER_POLICY_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_POLICY_MAGIC_V1, DEALER_POLICY_VERSION_V1);
        let identities = [
            self.realm_id,
            self.profile_id,
            self.market_instance_v2_id,
            self.claim_basis_id,
            self.collateral_mint,
            self.token_program,
            self.hoard_custody_semantics_id,
            self.relation_v2_id,
            self.price_measure_policy_id,
            self.curve_policy_id,
            self.curve_price_certificate_policy_id,
            self.fee_policy_id,
            self.liveness_policy_id,
            self.retirement_policy_id,
            self.neutral_sink,
            self.quote_authority,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            writer.id(identities[index]);
            index += 1;
        }
        writer.u8(self.outcome_count);
        writer.reserved(7);
        writer.u64(self.payout_denominator);
        writer.u64(self.capital_unit_cash_atoms);
        write_u64_array(&mut writer, &self.capital_unit_eggs);
        writer.u64(self.initial_price_denominator);
        write_u64_array(&mut writer, &self.initial_price_weights);
        writer.u64(self.depth_atoms);
        write_u64_array(&mut writer, &self.max_net_buy);
        write_u64_array(&mut writer, &self.max_net_sell);
        writer.u64(self.minimum_lp_shares);
        writer.u64(self.maximum_lp_shares);
        writer.u64(self.funding_deadline_slot);
        writer.u64(self.trading_open_slot);
        writer.u64(self.trading_close_slot);
        writer.u64(self.maturity_slot);
        writer.u64(self.shutdown_queue_numerator);
        writer.u64(self.shutdown_queue_denominator);
        writer.u32(self.maximum_lp_pages);
        writer.reserved(4);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&DEALER_POLICY_MAGIC_V1, DEALER_POLICY_VERSION_V1)?;
        let value = Self {
            realm_id: reader.id(),
            profile_id: reader.id(),
            market_instance_v2_id: reader.id(),
            claim_basis_id: reader.id(),
            collateral_mint: reader.id(),
            token_program: reader.id(),
            hoard_custody_semantics_id: reader.id(),
            relation_v2_id: reader.id(),
            price_measure_policy_id: reader.id(),
            curve_policy_id: reader.id(),
            curve_price_certificate_policy_id: reader.id(),
            fee_policy_id: reader.id(),
            liveness_policy_id: reader.id(),
            retirement_policy_id: reader.id(),
            neutral_sink: reader.id(),
            quote_authority: reader.id(),
            outcome_count: reader.u8(),
            payout_denominator: {
                reader.reserved(7)?;
                reader.u64()
            },
            capital_unit_cash_atoms: reader.u64(),
            capital_unit_eggs: read_u64_array(&mut reader),
            initial_price_denominator: reader.u64(),
            initial_price_weights: read_u64_array(&mut reader),
            depth_atoms: reader.u64(),
            max_net_buy: read_u64_array(&mut reader),
            max_net_sell: read_u64_array(&mut reader),
            minimum_lp_shares: reader.u64(),
            maximum_lp_shares: reader.u64(),
            funding_deadline_slot: reader.u64(),
            trading_open_slot: reader.u64(),
            trading_close_slot: reader.u64(),
            maturity_slot: reader.u64(),
            shutdown_queue_numerator: reader.u64(),
            shutdown_queue_denominator: reader.u64(),
            maximum_lp_pages: reader.u32(),
        };
        reader.reserved(4)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

fn write_u64_array(writer: &mut Writer<'_>, values: &[u64; MAX_OUTCOMES]) {
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        writer.u64(values[index]);
        index += 1;
    }
}

fn read_u64_array(reader: &mut Reader<'_>) -> [u64; MAX_OUTCOMES] {
    let mut values = [0u64; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        values[index] = reader.u64();
        index += 1;
    }
    values
}

const _: () = assert!(DEALER_POLICY_BYTES_V1 == 1_148);
const _: () = assert!(DEALER_POLICY_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);

fn ceil_div_u128(numerator: u128, denominator: u128) -> Result<u128> {
    if denominator == 0 {
        return Err(Error::InvalidParameter);
    }
    if numerator == 0 {
        return Ok(0);
    }
    numerator
        .checked_sub(1)
        .and_then(|value| value.checked_div(denominator))
        .and_then(|value| value.checked_add(1))
        .ok_or(Error::ArithmeticOverflow)
}

fn ceil_div_i128(numerator: i128, denominator: i128) -> Result<i128> {
    if denominator <= 0 {
        return Err(Error::InvalidParameter);
    }
    if numerator >= 0 {
        numerator
            .checked_add(denominator - 1)
            .and_then(|value| value.checked_div(denominator))
            .ok_or(Error::ArithmeticOverflow)
    } else {
        numerator
            .checked_div(denominator)
            .ok_or(Error::ArithmeticOverflow)
    }
}
