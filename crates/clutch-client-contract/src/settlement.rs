//! Fail-closed classification of the settlement shape a client can construct.

use core::fmt;

use clutch_batch::relation_v1::{
    BookV1, CandidateV1, LegRefV1, OrderV1, PairingWitnessV1, MAX_OUTCOMES, MAX_SLICES,
};
use clutch_batch::{Side, MAX_ORDERS};
use clutch_solana_layout::{canonical_order_id, stream::OrderPageHeader, Hash32};

/// Borrowed, untrusted projection presented to the current settlement client.
///
/// The fields use authoritative layout and relation types. Constructing this
/// value does not authenticate account ownership, a PDA, an epoch binding, a
/// candidate, or a witness; the onchain program remains authoritative.
#[derive(Clone, Copy, Debug)]
pub struct SettlementProjection<'a> {
    /// Page count recorded by the observed epoch.
    pub epoch_page_count: u16,
    /// Order count recorded by the observed epoch.
    pub epoch_order_count: u16,
    /// Outcome count recorded by the observed epoch.
    pub outcome_count: u8,
    /// Exact collateral conversion scale recorded by the observed epoch.
    pub price_scale: u64,
    /// Header of the only page projected by the current client.
    pub page: &'a OrderPageHeader,
    /// Relation projection of the page's live records.
    pub book: &'a BookV1,
    /// Persisted identities of live records, in relation-walk order.
    pub identities: &'a [Hash32],
    /// Selected candidate projected through the relation type.
    pub candidate: &'a CandidateV1,
    /// Selected executable decomposition projected through the relation type.
    pub witness: &'a PairingWitnessV1,
}

/// One direct entitlement/settlement instruction admitted by the current
/// client capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSettlementGroup {
    buy: u8,
    sell: u8,
    slice: u16,
}

impl DirectSettlementGroup {
    const EMPTY: Self = Self {
        buy: 0,
        sell: 0,
        slice: 0,
    };

    /// Zero-based live relation rank of the buying order.
    #[must_use]
    pub const fn buy(self) -> u8 {
        self.buy
    }

    /// Zero-based live relation rank of the selling order.
    #[must_use]
    pub const fn sell(self) -> u8 {
        self.sell
    }

    /// Zero-based witness slice and receipt coordinate.
    #[must_use]
    pub const fn slice(self) -> u16 {
        self.slice
    }
}

/// Complete ephemeral settlement plan supported by the current client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSettlementPlan {
    groups: [DirectSettlementGroup; MAX_SLICES],
    len: u16,
}

impl DirectSettlementPlan {
    fn empty() -> Self {
        Self {
            groups: [DirectSettlementGroup::EMPTY; MAX_SLICES],
            len: 0,
        }
    }

    fn push(&mut self, group: DirectSettlementGroup) -> Result<(), SettlementRefusal> {
        let index = usize::from(self.len);
        let slot = self
            .groups
            .get_mut(index)
            .ok_or(SettlementRefusal::WitnessCountOutOfRange)?;
        *slot = group;
        self.len = self
            .len
            .checked_add(1)
            .ok_or(SettlementRefusal::WitnessCountOutOfRange)?;
        Ok(())
    }

    /// Admitted direct groups in canonical witness order.
    #[must_use]
    pub fn groups(&self) -> &[DirectSettlementGroup] {
        &self.groups[..usize::from(self.len)]
    }
}

/// Exact reason the current client refused to claim it could settle a shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementRefusal {
    /// The current client projects exactly page zero of a one-page epoch.
    ExtraPages,
    /// The page is not frozen or its counts do not exactly bind the epoch.
    PageDoesNotBindEpoch,
    /// A tombstone requires a physical-slot/stored-id/live-rank mapping the
    /// current client does not carry.
    ChurnedPage,
    /// Page, live-book, identity, candidate, or outcome cardinalities disagree.
    ProjectionCardinalityMismatch,
    /// A live projected order cannot be tied to the same canonical stored id.
    LiveRankIdentityMismatch,
    /// The settlement conversion scale is zero.
    ZeroPriceScale,
    /// Candidate imbalance requires the two-phase potted virtual-leg shape.
    VirtualBalanceRequiresPot,
    /// No receipt exists to consume, so the client cannot claim settlement.
    EmptyWitness,
    /// Witness cardinality exceeds the authoritative fixed bound.
    WitnessCountOutOfRange,
    /// A witness names a virtual split or merge leg.
    VirtualLeg,
    /// A witness names an order outside the live book.
    OrderIndexOutOfRange,
    /// A portfolio pair shares an endpoint with another pair.
    NonexclusivePortfolioPair,
    /// An otherwise exclusive portfolio pair requires atomic portfolio accounts.
    ExclusivePortfolioPair,
    /// One endpoint is single-Egg and the other is portfolio.
    MixedSinglePortfolioPair,
    /// A direct slice is self-referential, empty, or outside the outcome set.
    InvalidDirectLeg,
    /// Direct endpoints do not bind the named outcome and buy/sell roles.
    DirectOrderRoleMismatch,
    /// More than one receipt would be required for the same direct pair.
    DuplicateDirectPair,
    /// Summing witness coverage overflowed an exact integer.
    CoverageOverflow,
    /// Direct groups do not exhaust every selected fill exactly.
    IncompleteFillCoverage,
    /// A filled single-Egg order names no candidate price.
    PriceIndexOutOfRange,
    /// Whole-order value multiplication overflowed.
    SettlementValueOverflow,
    /// Exact whole-order conversion requires a pot account the current client
    /// does not include.
    PotRequired,
}

impl fmt::Display for SettlementRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExtraPages => "Operator trade settlement supports exactly frozen page zero of a one-page epoch; additional pages are not projected",
            Self::PageDoesNotBindEpoch => "the projected page does not exactly bind the frozen epoch order set",
            Self::ChurnedPage => "Operator trade settlement does not yet carry the physical-slot/order-id/live-rank mapping required by a churned page",
            Self::ProjectionCardinalityMismatch => "the projected live book, identities, candidate, outcome count, and frozen order count disagree",
            Self::LiveRankIdentityMismatch => "Operator trade settlement cannot prove the live relation rank is the frozen physical order identity",
            Self::ZeroPriceScale => "the frozen settlement price scale is zero",
            Self::VirtualBalanceRequiresPot => "Operator trade settlement does not yet build the potted two-phase virtual-leg account shape",
            Self::EmptyWitness => "Operator trade settlement has no receipt to consume and cannot claim a terminal settlement",
            Self::WitnessCountOutOfRange => "the settlement witness exceeds its authoritative fixed bound",
            Self::VirtualLeg => "Operator trade settlement does not yet build virtual-leg entitlement and settlement accounts",
            Self::OrderIndexOutOfRange => "a witness slice names an order outside the frozen live book",
            Self::NonexclusivePortfolioPair => "Operator trade settlement does not yet build per-slice accounts for a nonexclusive portfolio pair",
            Self::ExclusivePortfolioPair => "Operator trade settlement does not yet build the atomic exclusive-portfolio account shape",
            Self::MixedSinglePortfolioPair => "Operator trade settlement does not yet build the per-slice mixed single/portfolio account shape",
            Self::InvalidDirectLeg => "a witness slice is not an executable direct settlement leg",
            Self::DirectOrderRoleMismatch => "a direct witness slice does not bind a buy and sell order on its named outcome",
            Self::DuplicateDirectPair => "Operator trade settlement does not yet track more than one receipt for a direct order pair",
            Self::CoverageOverflow => "witness coverage overflowed",
            Self::IncompleteFillCoverage => "the direct settlement groups do not exhaust the selected candidate's fills",
            Self::PriceIndexOutOfRange => "a filled single order names a price outside the candidate",
            Self::SettlementValueOverflow => "whole-order settlement value overflow",
            Self::PotRequired => "Operator trade settlement omits the pot account required by an inexact whole-order collateral conversion",
        })
    }
}

/// Classify an untrusted projection against the exact settlement shape the
/// current Operator can construct completely.
///
/// Classification is deliberately stricter than the onchain program. Success
/// means only that the client has every account coordinate needed to attempt
/// the direct settlement. It is not evidence that the candidate is valid or
/// that any transaction executed.
///
/// # Errors
///
/// Refuses every projection outside the exact direct, one-page, no-pot,
/// single-Egg capability described by [`SettlementRefusal`].
pub fn classify_direct_settlement(
    shape: &SettlementProjection<'_>,
) -> Result<DirectSettlementPlan, SettlementRefusal> {
    require_supported_page(shape)?;
    let book_len = usize::from(shape.book.len);
    let witness_len = usize::from(shape.witness.len);
    if witness_len > MAX_SLICES {
        return Err(SettlementRefusal::WitnessCountOutOfRange);
    }

    let mut plan = DirectSettlementPlan::empty();
    let mut covered = [0_u64; MAX_ORDERS];
    for index in 0..witness_len {
        let slice = shape
            .witness
            .slices
            .get(index)
            .ok_or(SettlementRefusal::WitnessCountOutOfRange)?;
        let (LegRefV1::Order(buy), LegRefV1::Order(sell)) = (slice.buy_ref, slice.sell_ref) else {
            return Err(SettlementRefusal::VirtualLeg);
        };
        let buy_index = usize::from(buy);
        let sell_index = usize::from(sell);
        let buy_order = shape
            .book
            .orders
            .get(buy_index)
            .filter(|_| buy_index < book_len)
            .ok_or(SettlementRefusal::OrderIndexOutOfRange)?;
        let sell_order = shape
            .book
            .orders
            .get(sell_index)
            .filter(|_| sell_index < book_len)
            .ok_or(SettlementRefusal::OrderIndexOutOfRange)?;
        let (buy_single, sell_single) = match (buy_order, sell_order) {
            (OrderV1::SingleEgg(buy_single), OrderV1::SingleEgg(sell_single)) => {
                (buy_single, sell_single)
            }
            (OrderV1::Portfolio(_), OrderV1::Portfolio(_)) => {
                if !pair_is_exclusive(shape.witness, witness_len, buy, sell) {
                    return Err(SettlementRefusal::NonexclusivePortfolioPair);
                }
                return Err(SettlementRefusal::ExclusivePortfolioPair);
            }
            _ => return Err(SettlementRefusal::MixedSinglePortfolioPair),
        };
        if buy == sell || slice.quantity == 0 || slice.outcome >= shape.outcome_count {
            return Err(SettlementRefusal::InvalidDirectLeg);
        }
        if buy_single.side != Side::Buy
            || sell_single.side != Side::Sell
            || buy_single.outcome != slice.outcome
            || sell_single.outcome != slice.outcome
        {
            return Err(SettlementRefusal::DirectOrderRoleMismatch);
        }
        if plan
            .groups()
            .iter()
            .any(|group| group.buy == buy && group.sell == sell)
        {
            return Err(SettlementRefusal::DuplicateDirectPair);
        }
        covered[buy_index] = covered[buy_index]
            .checked_add(slice.quantity)
            .ok_or(SettlementRefusal::CoverageOverflow)?;
        covered[sell_index] = covered[sell_index]
            .checked_add(slice.quantity)
            .ok_or(SettlementRefusal::CoverageOverflow)?;
        plan.push(DirectSettlementGroup {
            buy,
            sell,
            slice: u16::try_from(index).map_err(|_| SettlementRefusal::WitnessCountOutOfRange)?,
        })?;
    }

    let scale = u128::from(shape.price_scale);
    for (index, covered_quantity) in covered.iter().enumerate().take(book_len) {
        if *covered_quantity != shape.candidate.fills[index] {
            return Err(SettlementRefusal::IncompleteFillCoverage);
        }
        let OrderV1::SingleEgg(single) = shape.book.orders[index] else {
            if *covered_quantity != 0 {
                return Err(SettlementRefusal::IncompleteFillCoverage);
            }
            continue;
        };
        let price = shape
            .candidate
            .prices
            .get(usize::from(single.outcome))
            .ok_or(SettlementRefusal::PriceIndexOutOfRange)?;
        let value = u128::from(shape.candidate.fills[index])
            .checked_mul(u128::from(*price))
            .ok_or(SettlementRefusal::SettlementValueOverflow)?;
        if !value.is_multiple_of(scale) {
            return Err(SettlementRefusal::PotRequired);
        }
    }
    Ok(plan)
}

fn require_supported_page(shape: &SettlementProjection<'_>) -> Result<(), SettlementRefusal> {
    if shape.epoch_page_count != 1 || shape.page.page_index != 0 || shape.page.page_count != 1 {
        return Err(SettlementRefusal::ExtraPages);
    }
    if shape.page.frozen != 1
        || shape.page.set_order_count != shape.epoch_order_count
        || u16::from(shape.page.order_count) != shape.epoch_order_count
    {
        return Err(SettlementRefusal::PageDoesNotBindEpoch);
    }
    if shape.page.tombstone_count != 0 {
        return Err(SettlementRefusal::ChurnedPage);
    }
    let book_len = usize::from(shape.book.len);
    if book_len > MAX_ORDERS
        || book_len != shape.identities.len()
        || u16::from(shape.book.len) != shape.epoch_order_count
        || shape.candidate.order_len != shape.book.len
        || shape.outcome_count < 2
        || usize::from(shape.outcome_count) > MAX_OUTCOMES
    {
        return Err(SettlementRefusal::ProjectionCardinalityMismatch);
    }
    for (index, identity) in shape.identities.iter().enumerate() {
        let rank = u64::try_from(index)
            .map_err(|_| SettlementRefusal::ProjectionCardinalityMismatch)?
            .checked_add(1)
            .ok_or(SettlementRefusal::ProjectionCardinalityMismatch)?;
        if *identity != canonical_order_id(rank) || shape.book.orders[index].id() != rank {
            return Err(SettlementRefusal::LiveRankIdentityMismatch);
        }
        if let OrderV1::SingleEgg(single) = shape.book.orders[index] {
            if single.outcome >= shape.outcome_count {
                return Err(SettlementRefusal::ProjectionCardinalityMismatch);
            }
        }
    }
    if shape.price_scale == 0 {
        return Err(SettlementRefusal::ZeroPriceScale);
    }
    if shape.candidate.virtual_split != 0 || shape.candidate.virtual_merge != 0 {
        return Err(SettlementRefusal::VirtualBalanceRequiresPot);
    }
    if shape.witness.len == 0 {
        return Err(SettlementRefusal::EmptyWitness);
    }
    Ok(())
}

fn pair_is_exclusive(witness: &PairingWitnessV1, witness_len: usize, buy: u8, sell: u8) -> bool {
    witness.slices[..witness_len].iter().all(|slice| {
        let exact_pair =
            slice.buy_ref == LegRefV1::Order(buy) && slice.sell_ref == LegRefV1::Order(sell);
        let touches_buy =
            slice.buy_ref == LegRefV1::Order(buy) || slice.sell_ref == LegRefV1::Order(buy);
        let touches_sell =
            slice.buy_ref == LegRefV1::Order(sell) || slice.sell_ref == LegRefV1::Order(sell);
        exact_pair || (!touches_buy && !touches_sell)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v1::{PairingSliceV1, PortfolioOrderV1, SingleEggOrderV1};
    use clutch_batch::{PartialPolicy, Side};
    use clutch_solana_layout::{registry::LEGACY_INTENT_VERSION, Hash32};

    const OUTCOMES: u8 = 2;
    const PRICE_SCALE: u64 = 10_000;

    fn single(rank: u64, outcome: u8, side: Side) -> OrderV1 {
        OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: rank,
            owner: u16::try_from(rank).expect("small fixture owner"),
            outcome,
            side,
            quantity: 500,
            limit_price: PRICE_SCALE,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        })
    }

    fn portfolio(rank: u64, side: Side) -> OrderV1 {
        let mut coefficients = [0_u64; MAX_OUTCOMES];
        coefficients[0] = 1;
        OrderV1::Portfolio(PortfolioOrderV1 {
            canonical_order_id: rank,
            owner: u16::try_from(rank).expect("small fixture owner"),
            side,
            coefficients,
            active_len: OUTCOMES,
            lots: 500,
            limit_collateral_per_lot: PRICE_SCALE,
            minimum_fill_lots: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        })
    }

    fn book_of(orders: &[OrderV1]) -> BookV1 {
        let mut book = BookV1::empty();
        book.orders[..orders.len()].copy_from_slice(orders);
        book.len = u8::try_from(orders.len()).expect("small fixture book");
        book
    }

    fn identities(count: usize) -> [Hash32; MAX_ORDERS] {
        let mut values = [Hash32::ZERO; MAX_ORDERS];
        for (index, value) in values.iter_mut().enumerate().take(count) {
            *value = canonical_order_id(u64::try_from(index).expect("small fixture rank") + 1);
        }
        values
    }

    fn header(order_count: u8) -> OrderPageHeader {
        OrderPageHeader {
            market: Hash32::ZERO,
            epoch: Hash32::ZERO,
            order_set: Hash32::ZERO,
            page_digest: Hash32::ZERO,
            first_order_id: canonical_order_id(1),
            last_order_id: canonical_order_id(u64::from(order_count)),
            prev_page_last_order_id: Hash32::ZERO,
            page_index: 0,
            page_count: 1,
            set_order_count: u16::from(order_count),
            order_count,
            tombstone_count: 0,
            frozen: 1,
            stored_bump: 0,
        }
    }

    fn candidate(fills: &[u64], price: u64) -> CandidateV1 {
        let mut prices = [0_u64; MAX_OUTCOMES];
        prices[0] = price;
        let mut candidate = CandidateV1::empty(
            u8::try_from(fills.len()).expect("small fixture candidate"),
            prices,
        );
        candidate.fills[..fills.len()].copy_from_slice(fills);
        candidate
    }

    fn direct(buy: u8, sell: u8, outcome: u8, quantity: u64) -> PairingSliceV1 {
        PairingSliceV1 {
            buy_ref: LegRefV1::Order(buy),
            sell_ref: LegRefV1::Order(sell),
            outcome,
            quantity,
        }
    }

    fn witness(slices: &[PairingSliceV1]) -> PairingWitnessV1 {
        let mut witness = PairingWitnessV1::empty();
        witness.slices[..slices.len()].copy_from_slice(slices);
        witness.len = u16::try_from(slices.len()).expect("small fixture witness");
        witness
    }

    fn classify<'a>(
        page: &'a OrderPageHeader,
        book: &'a BookV1,
        identities: &'a [Hash32],
        candidate: &'a CandidateV1,
        witness: &'a PairingWitnessV1,
    ) -> Result<DirectSettlementPlan, SettlementRefusal> {
        classify_direct_settlement(&SettlementProjection {
            epoch_page_count: page.page_count,
            epoch_order_count: page.set_order_count,
            outcome_count: OUTCOMES,
            price_scale: PRICE_SCALE,
            page,
            book,
            identities,
            candidate,
            witness,
        })
    }

    fn direct_fixture() -> (
        OrderPageHeader,
        BookV1,
        [Hash32; MAX_ORDERS],
        CandidateV1,
        PairingWitnessV1,
    ) {
        let page = header(2);
        let book = book_of(&[single(1, 0, Side::Buy), single(2, 0, Side::Sell)]);
        let ids = identities(2);
        let candidate = candidate(&[500, 500], 200);
        let witness = witness(&[direct(0, 1, 0, 500)]);
        (page, book, ids, candidate, witness)
    }

    #[test]
    fn direct_exact_single_pair_is_the_only_admitted_profile() {
        let (page, book, ids, candidate, witness) = direct_fixture();
        let plan = classify(&page, &book, &ids[..2], &candidate, &witness)
            .expect("the exact current direct shape is supported");
        assert_eq!(
            plan.groups(),
            &[DirectSettlementGroup {
                buy: 0,
                sell: 1,
                slice: 0,
            }]
        );
    }

    #[test]
    fn direct_roles_and_outcomes_bind_the_account_plan() {
        let page = header(2);
        let ids = identities(2);
        let candidate = candidate(&[500, 500], 200);
        let outcome_zero = witness(&[direct(0, 1, 0, 500)]);

        let swapped = book_of(&[single(1, 0, Side::Sell), single(2, 0, Side::Buy)]);
        assert_eq!(
            classify(&page, &swapped, &ids[..2], &candidate, &outcome_zero),
            Err(SettlementRefusal::DirectOrderRoleMismatch)
        );

        let cross_outcome = book_of(&[single(1, 0, Side::Buy), single(2, 1, Side::Sell)]);
        assert_eq!(
            classify(&page, &cross_outcome, &ids[..2], &candidate, &outcome_zero,),
            Err(SettlementRefusal::DirectOrderRoleMismatch)
        );

        let direct_book = book_of(&[single(1, 0, Side::Buy), single(2, 0, Side::Sell)]);
        let wrong_slice_outcome = witness(&[direct(0, 1, 1, 500)]);
        assert_eq!(
            classify(
                &page,
                &direct_book,
                &ids[..2],
                &candidate,
                &wrong_slice_outcome,
            ),
            Err(SettlementRefusal::DirectOrderRoleMismatch)
        );

        let out_of_range = book_of(&[
            single(1, OUTCOMES, Side::Buy),
            single(2, OUTCOMES, Side::Sell),
        ]);
        assert_eq!(
            classify(&page, &out_of_range, &ids[..2], &candidate, &outcome_zero),
            Err(SettlementRefusal::ProjectionCardinalityMismatch)
        );
    }

    #[test]
    fn extra_pages_churn_and_identity_renumbering_refuse() {
        let (mut page, book, mut ids, candidate, witness) = direct_fixture();
        page.page_count = 2;
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &witness),
            Err(SettlementRefusal::ExtraPages)
        );
        page.page_count = 1;
        page.tombstone_count = 1;
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &witness),
            Err(SettlementRefusal::ChurnedPage)
        );
        page.tombstone_count = 0;
        ids[1] = canonical_order_id(9);
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &witness),
            Err(SettlementRefusal::LiveRankIdentityMismatch)
        );
    }

    #[test]
    fn virtual_legs_and_virtual_balance_refuse() {
        let (page, book, ids, mut candidate, mut witness) = direct_fixture();
        witness.slices[0].sell_ref = LegRefV1::Split;
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &witness),
            Err(SettlementRefusal::VirtualLeg)
        );
        candidate.virtual_split = 1;
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &witness),
            Err(SettlementRefusal::VirtualBalanceRequiresPot)
        );
    }

    #[test]
    fn mixed_exclusive_and_nonexclusive_portfolios_refuse_distinctly() {
        let ids = identities(3);
        let page2 = header(2);
        let candidate2 = candidate(&[500, 500], 200);
        let one = witness(&[direct(0, 1, 0, 500)]);
        let mixed = book_of(&[single(1, 0, Side::Buy), portfolio(2, Side::Sell)]);
        assert_eq!(
            classify(&page2, &mixed, &ids[..2], &candidate2, &one),
            Err(SettlementRefusal::MixedSinglePortfolioPair)
        );
        let pair = book_of(&[portfolio(1, Side::Buy), portfolio(2, Side::Sell)]);
        assert_eq!(
            classify(&page2, &pair, &ids[..2], &candidate2, &one),
            Err(SettlementRefusal::ExclusivePortfolioPair)
        );

        let page3 = header(3);
        let three = book_of(&[
            portfolio(1, Side::Buy),
            portfolio(2, Side::Sell),
            portfolio(3, Side::Sell),
        ]);
        let candidate3 = candidate(&[500, 250, 250], 200);
        let shared = witness(&[direct(0, 1, 0, 250), direct(0, 2, 0, 250)]);
        assert_eq!(
            classify(&page3, &three, &ids[..3], &candidate3, &shared),
            Err(SettlementRefusal::NonexclusivePortfolioPair)
        );
    }

    #[test]
    fn duplicate_receipts_incomplete_coverage_and_required_pot_refuse() {
        let (page, book, ids, _, _) = direct_fixture();
        let exact = candidate(&[2, 2], PRICE_SCALE);
        let duplicate = witness(&[direct(0, 1, 0, 1), direct(0, 1, 0, 1)]);
        assert_eq!(
            classify(&page, &book, &ids[..2], &exact, &duplicate),
            Err(SettlementRefusal::DuplicateDirectPair)
        );

        let incomplete = candidate(&[500, 499], 200);
        let one = witness(&[direct(0, 1, 0, 500)]);
        assert_eq!(
            classify(&page, &book, &ids[..2], &incomplete, &one),
            Err(SettlementRefusal::IncompleteFillCoverage)
        );

        let inexact = candidate(&[1, 1], 1_250);
        let one_atom = witness(&[direct(0, 1, 0, 1)]);
        assert_eq!(
            classify(&page, &book, &ids[..2], &inexact, &one_atom),
            Err(SettlementRefusal::PotRequired)
        );
    }

    #[test]
    fn malformed_cardinalities_and_indices_refuse_without_panicking() {
        let (mut page, mut book, ids, mut candidate, mut observed_witness) = direct_fixture();
        observed_witness.len = u16::MAX;
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &observed_witness),
            Err(SettlementRefusal::WitnessCountOutOfRange)
        );

        observed_witness = witness(&[direct(0, 9, 0, 500)]);
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &observed_witness),
            Err(SettlementRefusal::OrderIndexOutOfRange)
        );

        book.len = u8::MAX;
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &observed_witness),
            Err(SettlementRefusal::ProjectionCardinalityMismatch)
        );
        book.len = 2;
        candidate.order_len = 1;
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &observed_witness),
            Err(SettlementRefusal::ProjectionCardinalityMismatch)
        );
        candidate.order_len = 2;
        page.frozen = 0;
        assert_eq!(
            classify(&page, &book, &ids[..2], &candidate, &observed_witness),
            Err(SettlementRefusal::PageDoesNotBindEpoch)
        );
    }

    #[test]
    fn empty_invalid_and_overflowing_direct_witnesses_refuse() {
        let (page, book, ids, direct_candidate, _) = direct_fixture();
        assert_eq!(
            classify(
                &page,
                &book,
                &ids[..2],
                &direct_candidate,
                &PairingWitnessV1::empty(),
            ),
            Err(SettlementRefusal::EmptyWitness)
        );
        let self_pair = witness(&[direct(0, 0, 0, 1)]);
        assert_eq!(
            classify(&page, &book, &ids[..2], &direct_candidate, &self_pair),
            Err(SettlementRefusal::InvalidDirectLeg)
        );

        let page3 = header(3);
        let book3 = book_of(&[
            single(1, 0, Side::Buy),
            single(2, 0, Side::Sell),
            single(3, 0, Side::Sell),
        ]);
        let ids3 = identities(3);
        let candidate3 = candidate(&[u64::MAX, u64::MAX, 1], PRICE_SCALE);
        let overflowing = witness(&[direct(0, 1, 0, u64::MAX), direct(0, 2, 0, 1)]);
        assert_eq!(
            classify(&page3, &book3, &ids3[..3], &candidate3, &overflowing),
            Err(SettlementRefusal::CoverageOverflow)
        );
    }

    #[test]
    fn invalid_scale_and_outcome_width_refuse_before_arithmetic() {
        let (page, book, ids, candidate, witness) = direct_fixture();
        let mut shape = SettlementProjection {
            epoch_page_count: page.page_count,
            epoch_order_count: page.set_order_count,
            outcome_count: OUTCOMES,
            price_scale: 0,
            page: &page,
            book: &book,
            identities: &ids[..2],
            candidate: &candidate,
            witness: &witness,
        };
        assert_eq!(
            classify_direct_settlement(&shape),
            Err(SettlementRefusal::ZeroPriceScale)
        );
        shape.price_scale = PRICE_SCALE;
        shape.outcome_count = 1;
        assert_eq!(
            classify_direct_settlement(&shape),
            Err(SettlementRefusal::ProjectionCardinalityMismatch)
        );
    }

    #[test]
    fn evidence_and_registry_vocabulary_are_not_settlement_state() {
        assert_eq!(LEGACY_INTENT_VERSION, 3);
        let descriptor = crate::evidence::EvidenceDescriptor::new(
            crate::evidence::EvidenceProvenance::ChainDerived,
            crate::evidence::EvidenceScope::CurrentSnapshot,
        );
        assert!(descriptor.retained_historical_source().is_err());
    }
}
