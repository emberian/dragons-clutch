//! Borrowed traversal-derived selected-execution fee-weight authority.
//!
//! The retained V5 Feed and complete page stream remain the sole source of
//! selected fills, ownership, sides, and coefficient vectors. This module
//! aggregates each owner's complete executed buy payoff, asks the existing
//! composite-fee owner for its exact zero-carry base numerator, omits zero,
//! and maps the result to the already-authenticated ordinary Position V3.
//! It never derives a weight from consideration, limit, posted quantity, or a
//! caller-provided owner row, and it never materializes a maximum-width book
//! inside an SBF capability.

use clutch_batch::relation_v1::{FrozenPolicyV1, SelfCrossPolicyV1, MAX_OUTCOMES};
use clutch_batch::{Side, MAX_ORDERS};
use clutch_batch_policy_identity::batch_policy_digest;
pub use clutch_fee_runtime_contract::selected::SelectedCompositeFeeV1;
pub use clutch_fee_runtime_contract::weight_v2::{
    CompositeFeeWeightRowV2, CompositeFeeWeightTranscriptV2,
};
use clutch_fee_runtime_contract::weight_v2::{
    composite_fee_hamilton_share_v2, composite_fee_weight_transcript_v2,
};
use clutch_fee_runtime_contract::{Error as FeeError, Id as FeeId};
use clutch_general_v2_contract::Id32;

use crate::{
    AdapterPositionMarketBindingV3, AuthenticatedSettlementPositionBookV3,
    AuthenticatedSettlementPositionV3, SettlementAdapterErrorV1, SettlementTraversalAccessV5,
};

/// Narrow Position identity source for exact fee-weight rows.
///
/// The existing complete Position-book authenticator implements this trait.
/// A live adapter may instead derive the canonical Position V3 PDA from the
/// already-authenticated owner/Market binding when an action neither reads nor
/// mutates Position state. No caller-supplied Position ID may implement the
/// private SBF capability used by recipient creation.
pub trait PortfolioFeeWeightPositionAccessV2 {
    /// Exact full-width Position market binding.
    fn market_binding(&self) -> AdapterPositionMarketBindingV3;
    /// Canonical ordinary Position account for one traversal-owned owner.
    fn position_account(
        &self,
        owner: Id32,
    ) -> Result<Id32, SettlementAdapterErrorV1>;
}

impl PortfolioFeeWeightPositionAccessV2 for AuthenticatedSettlementPositionBookV3 {
    fn market_binding(&self) -> AdapterPositionMarketBindingV3 {
        AuthenticatedSettlementPositionBookV3::market_binding(self)
    }

    fn position_account(
        &self,
        owner: Id32,
    ) -> Result<Id32, SettlementAdapterErrorV1> {
        self.position_for_owner(owner)
            .map(AuthenticatedSettlementPositionV3::account)
            .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)
    }
}

/// Compact borrowed V5 fee-weight stream awaiting the SBF adapter's exact
/// MarketBinding batch-policy authentication.
///
/// This value is deliberately not named authenticated. It retains the exact
/// traversal, Position book, and selected-fee borrows and reproduces each row
/// on demand. Its size is independent of the maximum owner count.
#[derive(Clone, Copy, Debug)]
pub struct DerivedPortfolioFeeWeightStreamV2<'a> {
    traversal: &'a dyn SettlementTraversalAccessV5,
    positions: &'a dyn PortfolioFeeWeightPositionAccessV2,
    selected: &'a SelectedCompositeFeeV1,
    market: Id32,
    epoch: Id32,
    settlement_candidate: Id32,
    owner_order_set_digest: Id32,
    batch_policy_id: Id32,
    expected_executed_owner_count: u16,
    transcript: CompositeFeeWeightTranscriptV2,
}

const _: () = assert!(
    core::mem::size_of::<DerivedPortfolioFeeWeightStreamV2<'static>>() <= 512
);

impl DerivedPortfolioFeeWeightStreamV2<'_> {
    /// Exact selected fee semantic used for every row quote.
    pub const fn selected(&self) -> &SelectedCompositeFeeV1 { self.selected }
    /// Canonical Market identity from the retained Feed.
    pub const fn market(&self) -> Id32 { self.market }
    /// Canonical Epoch identity from the retained Feed.
    pub const fn epoch(&self) -> Id32 { self.epoch }
    /// Exact selected settlement-candidate identity.
    pub const fn settlement_candidate(&self) -> Id32 { self.settlement_candidate }
    /// Complete immutable owner/order-set digest from the traversal.
    pub const fn owner_order_set_digest(&self) -> Id32 { self.owner_order_set_digest }
    /// Batch-policy identity that the live adapter must bind to MarketBinding.
    pub const fn batch_policy_id(&self) -> Id32 { self.batch_policy_id }
    /// Number of distinct owners with nonzero selected execution.
    pub const fn expected_executed_owner_count(&self) -> u16 {
        self.expected_executed_owner_count
    }
    /// Exact compact commitment to every Position-sorted nonzero row.
    pub const fn transcript(&self) -> CompositeFeeWeightTranscriptV2 { self.transcript }

    /// Reproduce one canonical Position-sorted row by dense stream index.
    ///
    /// An index at or beyond the committed row count returns `None`. No owner,
    /// Position, weight, or row count is accepted from the caller.
    pub fn row(
        &self,
        index: u8,
    ) -> Result<Option<CompositeFeeWeightRowV2>, SettlementAdapterErrorV1> {
        if index >= self.transcript.len() {
            return Ok(None);
        }
        let prices = traversal_prices(self.traversal)?;
        let mut prior = None;
        let mut cursor = 0u8;
        while cursor <= index {
            let row = next_position_weight_row(
                self.traversal,
                self.positions,
                self.selected,
                &prices,
                self.transcript.common_denominator(),
                prior,
            )?
            .ok_or(SettlementAdapterErrorV1::FeeOwnerMismatch)?;
            if cursor == index {
                return Ok(Some(row));
            }
            prior = Some(row.position());
            cursor = cursor
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        }
        Err(SettlementAdapterErrorV1::FeeOwnerMismatch)
    }

    /// Allocate one row at the sole Hamilton final-collateral-atom boundary.
    ///
    /// `total_atoms` remains structural here; a value-moving composer must
    /// bind it to the authenticated recipient pool before using the result.
    /// Every floor and remainder is recomputed from the certified stream, and
    /// equal remainders break by ascending Position identity.
    pub fn hamilton_atoms(
        &self,
        index: u8,
        total_atoms: u64,
    ) -> Result<Option<u64>, SettlementAdapterErrorV1> {
        let Some(target) = self.row(index)? else { return Ok(None) };
        let target_share = composite_fee_hamilton_share_v2(
            total_atoms,
            target.exact_numerator(),
            self.transcript.total_weight(),
        )?;
        let mut assigned = 0u64;
        let mut higher_ranked = 0u64;
        let mut cursor = 0u8;
        while cursor < self.transcript.len() {
            let row = self
                .row(cursor)?
                .ok_or(SettlementAdapterErrorV1::FeeOwnerMismatch)?;
            let share = composite_fee_hamilton_share_v2(
                total_atoms,
                row.exact_numerator(),
                self.transcript.total_weight(),
            )?;
            assigned = assigned
                .checked_add(share.floor_atoms())
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            if share.remainder() > target_share.remainder()
                || (share.remainder() == target_share.remainder()
                    && row.position() < target.position())
            {
                higher_ranked = higher_ranked
                    .checked_add(1)
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            }
            cursor = cursor
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        }
        let dust = total_atoms
            .checked_sub(assigned)
            .ok_or(SettlementAdapterErrorV1::FeeOwnerMismatch)?;
        if dust > u64::from(self.transcript.len()) {
            return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
        }
        let extra = if higher_ranked < dust { 1u64 } else { 0u64 };
        Ok(Some(
            target_share
                .floor_atoms()
                .checked_add(extra)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?,
        ))
    }
}

/// Derive the complete exact borrowed weight stream from one authenticated
/// traversal and its complete ordinary Position V3 set.
///
/// The selected fee record supplies the same composite rates and denominator
/// used for charging. The complete batch-policy preimage is required so the
/// traversal can reproduce its exact per-outcome owner-overlap refusal rather
/// than inventing a different selected-fill self-cross rule.
pub fn derive_portfolio_fee_weight_stream_v2<'a>(
    traversal: &'a dyn SettlementTraversalAccessV5,
    positions: &'a dyn PortfolioFeeWeightPositionAccessV2,
    selected: &'a SelectedCompositeFeeV1,
    batch_policy: &FrozenPolicyV1,
) -> Result<DerivedPortfolioFeeWeightStreamV2<'a>, SettlementAdapterErrorV1> {
    let projection = traversal.projection();
    let feed = projection.feed();
    batch_policy
        .validate()
        .map_err(|_| SettlementAdapterErrorV1::BindingMismatch)?;
    let batch_policy_id = Id32::new(
        batch_policy_digest(batch_policy)
            .map_err(|_| SettlementAdapterErrorV1::BindingMismatch)?
            .0,
    )?;
    if batch_policy.self_cross != SelfCrossPolicyV1::RefuseOverlap
        || selected.batch_policy().0 != batch_policy_id.bytes()
        || selected.realm().0 != projection.realm().bytes()
        || selected.market().0 != feed.market.bytes()
        || selected.epoch().0 != feed.epoch.bytes()
        || selected.selected_candidate().0 != feed.settlement_candidate_id.bytes()
        || selected.price_scale() != feed.price_scale
        || selected.outcome_count() != feed.outcome_count
        || positions.market_binding() != projection.position_market_binding()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    require_posted_owner_overlap_refusal(traversal)?;

    let executed_owner_count = count_executed_owners(traversal)?;
    if executed_owner_count != projection.expected_owner_count() {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }

    let prices = traversal_prices(traversal)?;
    let zero_quote = selected.quote_owner(&[0u64; MAX_OUTCOMES], &prices, 0)?;
    if zero_quote.base_numerator != 0
        || zero_quote.exact_numerator != 0
        || zero_quote.base_denominator != zero_quote.exact_denominator
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let common_denominator = zero_quote.base_denominator;
    require_unique_weight_positions(
        traversal,
        positions,
        selected,
        &prices,
        common_denominator,
    )?;

    let mut stream_error = None;
    let transcript_result = composite_fee_weight_transcript_v2(
        selected.fee_record(),
        common_denominator,
        |prior| match next_position_weight_row(
            traversal,
            positions,
            selected,
            &prices,
            common_denominator,
            prior,
        ) {
            Ok(row) => Ok(row),
            Err(error) => {
                stream_error = Some(error);
                Err(FeeError::MismatchedBinding)
            }
        },
    );
    if let Some(error) = stream_error {
        return Err(error);
    }
    let transcript = transcript_result?;
    Ok(DerivedPortfolioFeeWeightStreamV2 {
        traversal,
        positions,
        selected,
        market: feed.market,
        epoch: feed.epoch,
        settlement_candidate: feed.settlement_candidate_id,
        owner_order_set_digest: projection.owner_order_set_digest(),
        batch_policy_id,
        expected_executed_owner_count: executed_owner_count,
        transcript,
    })
}

fn traversal_prices(
    traversal: &dyn SettlementTraversalAccessV5,
) -> Result<[u64; MAX_OUTCOMES], SettlementAdapterErrorV1> {
    let feed = traversal.projection().feed();
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut outcome = 0u8;
    while outcome < feed.outcome_count {
        prices[usize::from(outcome)] = traversal.outcome_price(outcome)?;
        outcome = outcome
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    Ok(prices)
}

fn next_executed_owner_after(
    traversal: &dyn SettlementTraversalAccessV5,
    prior: Option<Id32>,
) -> Result<Option<Id32>, SettlementAdapterErrorV1> {
    if prior.is_some_and(Id32::is_zero) {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let feed = traversal.projection().feed();
    let mut best = None;
    let mut order_index = 0u8;
    while order_index < feed.order_count {
        if traversal.selected_fill(order_index)? != 0 {
            let owner = traversal
                .order(order_index)?
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                .membership()
                .owner();
            if owner.is_zero() {
                return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
            }
            if prior.is_none_or(|previous| owner > previous)
                && best.is_none_or(|current| owner < current)
            {
                best = Some(owner);
            }
        }
        order_index = order_index
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    Ok(best)
}

fn count_executed_owners(
    traversal: &dyn SettlementTraversalAccessV5,
) -> Result<u16, SettlementAdapterErrorV1> {
    let mut count = 0u16;
    let mut prior = None;
    while let Some(owner) = next_executed_owner_after(traversal, prior)? {
        count = count
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        prior = Some(owner);
    }
    Ok(count)
}

fn owner_weight_row(
    traversal: &dyn SettlementTraversalAccessV5,
    positions: &dyn PortfolioFeeWeightPositionAccessV2,
    selected: &SelectedCompositeFeeV1,
    prices: &[u64; MAX_OUTCOMES],
    common_denominator: u128,
    owner: Id32,
) -> Result<Option<CompositeFeeWeightRowV2>, SettlementAdapterErrorV1> {
    let (payoff, has_buy, _) = owner_executed_payoff(traversal, owner)?;
    let quote = selected.quote_owner(&payoff, prices, 0)?;
    if quote.exact_numerator != quote.base_numerator
        || quote.exact_denominator != quote.base_denominator
        || quote.base_denominator != common_denominator
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    if quote.base_numerator == 0 {
        return Ok(None);
    }
    if !has_buy {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let position = positions.position_account(owner)?;
    Ok(Some(CompositeFeeWeightRowV2::structural(
        FeeId(position.bytes()),
        quote.base_numerator,
    )?))
}

fn next_position_weight_row(
    traversal: &dyn SettlementTraversalAccessV5,
    positions: &dyn PortfolioFeeWeightPositionAccessV2,
    selected: &SelectedCompositeFeeV1,
    prices: &[u64; MAX_OUTCOMES],
    common_denominator: u128,
    prior_position: Option<FeeId>,
) -> Result<Option<CompositeFeeWeightRowV2>, SettlementAdapterErrorV1> {
    if prior_position.is_some_and(FeeId::is_zero) {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }
    let mut owner_prior = None;
    let mut best = None;
    while let Some(owner) = next_executed_owner_after(traversal, owner_prior)? {
        if let Some(row) = owner_weight_row(
            traversal,
            positions,
            selected,
            prices,
            common_denominator,
            owner,
        )? {
            if prior_position.is_none_or(|previous| row.position() > previous)
                && best.is_none_or(|current: CompositeFeeWeightRowV2| {
                    row.position() < current.position()
                })
            {
                best = Some(row);
            }
        }
        owner_prior = Some(owner);
    }
    Ok(best)
}

fn require_unique_weight_positions(
    traversal: &dyn SettlementTraversalAccessV5,
    positions: &dyn PortfolioFeeWeightPositionAccessV2,
    selected: &SelectedCompositeFeeV1,
    prices: &[u64; MAX_OUTCOMES],
    common_denominator: u128,
) -> Result<(), SettlementAdapterErrorV1> {
    let mut owner_prior = None;
    while let Some(owner) = next_executed_owner_after(traversal, owner_prior)? {
        if let Some(row) = owner_weight_row(
            traversal,
            positions,
            selected,
            prices,
            common_denominator,
            owner,
        )? {
            let mut other_prior = Some(owner);
            while let Some(other_owner) = next_executed_owner_after(traversal, other_prior)? {
                if owner_weight_row(
                    traversal,
                    positions,
                    selected,
                    prices,
                    common_denominator,
                    other_owner,
                )?
                .is_some_and(|other| other.position() == row.position())
                {
                    return Err(SettlementAdapterErrorV1::PositionSetMismatch);
                }
                other_prior = Some(other_owner);
            }
        }
        owner_prior = Some(owner);
    }
    Ok(())
}

fn owner_executed_payoff(
    traversal: &dyn SettlementTraversalAccessV5,
    owner: Id32,
) -> Result<([u64; MAX_OUTCOMES], bool, bool), SettlementAdapterErrorV1> {
    let feed = traversal.projection().feed();
    let mut payoff = [0u64; MAX_OUTCOMES];
    let mut has_buy = false;
    let mut has_sell = false;
    let mut order_index = 0u8;
    while order_index < feed.order_count {
        let fill = traversal.selected_fill(order_index)?;
        let row = traversal
            .order(order_index)?
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        if fill != 0 && row.membership().owner() == owner {
            accumulate_selected_owner_order(
                row.economic_order(),
                fill,
                feed.outcome_count,
                &mut payoff,
                &mut has_buy,
                &mut has_sell,
            )?;
        }
        order_index = order_index
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    Ok((payoff, has_buy, has_sell))
}

fn require_posted_owner_overlap_refusal(
    traversal: &dyn SettlementTraversalAccessV5,
) -> Result<(), SettlementAdapterErrorV1> {
    let feed = traversal.projection().feed();
    let mut order = 0u8;
    while order < feed.order_count {
        let row = traversal
            .order(order)?
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let mut later = order
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        while later < feed.order_count {
            let other = traversal
                .order(later)?
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
            if row.membership().owner() == other.membership().owner()
                && row.economic_order().side != other.economic_order().side
            {
                let mut outcome = 0usize;
                while outcome < usize::from(feed.outcome_count) {
                    if row.economic_order().coefficients[outcome] != 0
                        && other.economic_order().coefficients[outcome] != 0
                    {
                        return Err(SettlementAdapterErrorV1::OwnerPairingInfeasible);
                    }
                    outcome += 1;
                }
            }
            later = later
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        }
        order = order
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    Ok(())
}

fn accumulate_selected_owner_order(
    order: &clutch_batch::relation_v2::EconomicOrderV2,
    fill: u64,
    outcome_count: u8,
    payoff: &mut [u64; MAX_OUTCOMES],
    has_buy: &mut bool,
    has_sell: &mut bool,
) -> Result<(), SettlementAdapterErrorV1> {
    if fill == 0 {
        return Ok(());
    }
    match order.side {
        Side::Buy => {
            *has_buy = true;
            let mut outcome = 0usize;
            while outcome < usize::from(outcome_count) {
                payoff[outcome] = payoff[outcome]
                    .checked_add(
                        order.coefficients[outcome]
                            .checked_mul(fill)
                            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?,
                    )
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                outcome += 1;
            }
        }
        Side::Sell => *has_sell = true,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v2::EconomicOrderV2;
    use clutch_batch::PartialPolicy;

    fn order(side: Side, coefficients: [u64; MAX_OUTCOMES]) -> EconomicOrderV2 {
        EconomicOrderV2 {
            order_id: [7; 32],
            side,
            coefficients,
            quantity: 9,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 1,
            limit_value_price_units_per_unit: 1,
        }
    }

    #[test]
    fn same_owner_buy_payoffs_net_before_the_existing_quote() {
        let mut first = [0u64; MAX_OUTCOMES];
        first[0] = 2;
        let mut second = [0u64; MAX_OUTCOMES];
        second[1] = 3;
        let mut payoff = [0u64; MAX_OUTCOMES];
        let mut has_buy = false;
        let mut has_sell = false;
        accumulate_selected_owner_order(
            &order(Side::Buy, first), 4, 2, &mut payoff, &mut has_buy, &mut has_sell,
        ).unwrap();
        accumulate_selected_owner_order(
            &order(Side::Buy, second), 5, 2, &mut payoff, &mut has_buy, &mut has_sell,
        ).unwrap();
        assert_eq!(&payoff[..2], &[8, 15]);
        assert!(has_buy);
        assert!(!has_sell);
    }

    #[test]
    fn seller_only_is_exact_zero_not_a_surrogate_weight() {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[0] = 1;
        let mut payoff = [0u64; MAX_OUTCOMES];
        let mut has_buy = false;
        let mut has_sell = false;
        accumulate_selected_owner_order(
            &order(Side::Sell, coefficients),
            9,
            2,
            &mut payoff,
            &mut has_buy,
            &mut has_sell,
        ).unwrap();
        assert_eq!(payoff, [0u64; MAX_OUTCOMES]);
        assert!(!has_buy);
        assert!(has_sell);
    }

    #[test]
    fn zero_fill_cannot_create_buy_or_sell_weight_input() {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[0] = u64::MAX;
        let mut payoff = [0u64; MAX_OUTCOMES];
        let mut has_buy = false;
        let mut has_sell = false;
        accumulate_selected_owner_order(
            &order(Side::Buy, coefficients),
            0,
            2,
            &mut payoff,
            &mut has_buy,
            &mut has_sell,
        ).unwrap();
        assert_eq!(payoff, [0u64; MAX_OUTCOMES]);
        assert!(!has_buy);
        assert!(!has_sell);
    }
}
