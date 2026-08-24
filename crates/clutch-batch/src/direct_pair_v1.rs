//! Exact owner-blind Direct pair selection over RelationV2.
//!
//! Direct is the scalar-Egg specialization of the coefficient-vector
//! relation, not a second order model. This module therefore accepts only an
//! exact two-order [`EconomicBookV2`], re-executes RelationV2, and proves that
//! the accepted candidate is one nonzero buy/sell pair for one active outcome.
//! The returned capability has private fields and also requires a private
//! traversal authority, so decoded page rows or a caller-built pair cannot be
//! promoted to settlement authority.
//!
//! Cash conversion has one named boundary: [`DirectCashBoundaryV1::ExactOnly`].
//! A price-unit consideration which is not exactly divisible by the immutable
//! price scale is refused; this module never floors, rounds, or creates dust.

use crate::relation_v1::MAX_OUTCOMES;
use crate::relation_v2::{
    validate_two_order_prefix_v2, verify_economic_candidate_v2,
    verify_two_order_economic_candidate_v2, EconomicBookV2, EconomicCandidateV2,
    EconomicDomainV2, EconomicErrorV2, EconomicOrderV2, PricePreconditionV2,
    VerifiedEconomicsV2,
};
use crate::{Side, MAX_ORDERS};

const DIRECT_PAIR_SELECTION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct-pair-selection/v1\0";

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);

/// The only Direct cash conversion boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCashBoundaryV1 {
    /// Require `quantity * price` to be exactly divisible by `price_scale`.
    ExactOnly,
}

impl DirectCashBoundaryV1 {
    /// Stable transcript byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::ExactOnly => 1,
        }
    }
}

/// Every deterministic refusal owned by Direct pair selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectPairErrorV1 {
    /// The underlying owner-blind RelationV2 candidate refused.
    Economic(EconomicErrorV2),
    /// The book was not the exact two-order Direct shape.
    WrongBookShape,
    /// The two live orders were not one buy and one sell.
    WrongSidePartition,
    /// A live order was not a unit coefficient for exactly one active outcome.
    NotSingleEgg,
    /// The two orders selected different outcomes.
    OutcomeMismatch,
    /// The candidate did not fill both orders by the same nonzero quantity.
    FillMismatch,
    /// Direct forbids virtual split and merge legs.
    VirtualConversion,
    /// Recomputed aggregate economics did not equal the exact scalar pair.
    EconomicMismatch,
    /// Exact price-unit arithmetic overflowed.
    ArithmeticOverflow,
    /// Exact-only conversion would require rounding.
    InexactCashConversion,
    /// A required selected-traversal identity was zero.
    ZeroSelectionIdentity,
    /// The private traversal authority refused this exact candidate.
    UnauthenticatedSelection,
}

impl From<EconomicErrorV2> for DirectPairErrorV1 {
    fn from(value: EconomicErrorV2) -> Self {
        Self::Economic(value)
    }
}

/// Default-deny boundary for the persisted Direct selection transcript.
///
/// Implementing this trait is not account authentication. A live adapter must
/// keep its implementation private and construct it only after checking the
/// complete page set, page order, selected traversal, account owners, PDAs,
/// exact bodies, immutable Market/epoch identities, and selection replay.
pub trait AuthenticatedDirectSelectionAuthorityV1 {
    /// Authenticate that the named persisted traversal selected exactly this
    /// recomputed RelationV2 candidate and no alternate pair.
    fn authenticate_selected_pair(
        &self,
        _selection_transcript_id: [u8; 32],
        _domain: &EconomicDomainV2,
        _book: &EconomicBookV2,
        _price: &PricePreconditionV2,
        _candidate: &EconomicCandidateV2,
        _economics: &VerifiedEconomicsV2,
    ) -> Result<(), DirectPairErrorV1> {
        Err(DirectPairErrorV1::UnauthenticatedSelection)
    }

    /// Authenticate the frame-safe exact two-row projection and its canonical
    /// general-width RelationV2 digest.
    fn authenticate_compact_selected_pair(
        &self,
        _selection_transcript_id: [u8; 32],
        _domain: &EconomicDomainV2,
        _orders: &[EconomicOrderV2; 2],
        _price: &PricePreconditionV2,
        _candidate: DirectEconomicCandidateV1,
        _economics: &VerifiedEconomicsV2,
    ) -> Result<(), DirectPairErrorV1> {
        Err(DirectPairErrorV1::UnauthenticatedSelection)
    }
}

/// Explicit default-deny authority for callers without persisted traversal
/// authentication.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectSelectionAuthorityV1;

impl AuthenticatedDirectSelectionAuthorityV1 for NoDirectSelectionAuthorityV1 {}

/// Exact two-order book used by the Direct specialization.
///
/// The two rows remain ordered by canonical RelationV2 order identity. Its
/// verifier emits the same digest as a 64-row [`EconomicBookV2`] with exact
/// empty padding but never materializes that larger value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectEconomicBookV1 {
    /// Exact sorted active rows followed by canonical empty padding.
    pub orders: [EconomicOrderV2; 2],
    /// Active prefix in `0..=2`.
    pub len: u8,
}

impl DirectEconomicBookV1 {
    /// Validate the exact fixed-capacity RelationV2 prefix.
    pub fn validate(&self, domain: &EconomicDomainV2) -> Result<(), DirectPairErrorV1> {
        validate_two_order_prefix_v2(domain, &self.orders, self.len)
            .map_err(DirectPairErrorV1::Economic)
    }
}

/// Compact coordinates of one Direct candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectEconomicCandidateV1 {
    /// Fill units for the exact two sorted rows.
    pub fills: [u64; 2],
    /// Exact AON bits for those two rows; upper bits are forbidden.
    pub honored_aon_mask: u8,
}

impl DirectEconomicCandidateV1 {
    /// Canonical zero-fill witness.
    pub const EMPTY: Self = Self {
        fills: [0; 2],
        honored_aon_mask: 0,
    };
}

/// Private exact Direct pair selected by one authenticated traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedDirectPairV1 {
    selection_transcript_id: [u8; 32],
    economic_candidate_digest: [u8; 32],
    semantic_price_digest: [u8; 32],
    buy_order_id: [u8; 32],
    sell_order_id: [u8; 32],
    buy_order_index: u8,
    sell_order_index: u8,
    outcome: u8,
    outcome_count: u8,
    quantity: u64,
    price_units_per_egg: u64,
    price_scale: u64,
    consideration_price_units: u128,
    consideration_cash_atoms: u64,
    boundary: DirectCashBoundaryV1,
}

impl SelectedDirectPairV1 {
    /// Persisted selected-traversal identity authenticated by the adapter.
    pub const fn selection_transcript_id(&self) -> [u8; 32] {
        self.selection_transcript_id
    }

    /// Independently recomputed RelationV2 candidate identity.
    pub const fn economic_candidate_digest(&self) -> [u8; 32] {
        self.economic_candidate_digest
    }

    /// Canonical semantic price-vector identity.
    pub const fn semantic_price_digest(&self) -> [u8; 32] {
        self.semantic_price_digest
    }

    /// Exact buy order identity selected from the complete book.
    pub const fn buy_order_id(&self) -> [u8; 32] {
        self.buy_order_id
    }

    /// Exact sell order identity selected from the complete book.
    pub const fn sell_order_id(&self) -> [u8; 32] {
        self.sell_order_id
    }

    /// Dense buy-order index in the authenticated complete book.
    pub const fn buy_order_index(&self) -> u8 {
        self.buy_order_index
    }

    /// Dense sell-order index in the authenticated complete book.
    pub const fn sell_order_index(&self) -> u8 {
        self.sell_order_index
    }

    /// Selected active outcome.
    pub const fn outcome(&self) -> u8 {
        self.outcome
    }

    /// Full immutable active outcome width.
    pub const fn outcome_count(&self) -> u8 {
        self.outcome_count
    }

    /// Exact matched native-Egg quantity.
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }

    /// Exact price units per native Egg.
    pub const fn price_units_per_egg(&self) -> u64 {
        self.price_units_per_egg
    }

    /// Immutable exact price scale.
    pub const fn price_scale(&self) -> u64 {
        self.price_scale
    }

    /// Exact pre-division consideration.
    pub const fn consideration_price_units(&self) -> u128 {
        self.consideration_price_units
    }

    /// Exactly converted collateral cash atoms.
    pub const fn consideration_cash_atoms(&self) -> u64 {
        self.consideration_cash_atoms
    }

    /// Sole named cash conversion boundary.
    pub const fn boundary(&self) -> DirectCashBoundaryV1 {
        self.boundary
    }

    /// Canonical fixed-width semantic transcript for downstream receipt owners.
    ///
    /// This byte string is not itself persisted authority. A Direct account
    /// codec may hash it into its own receipt only after authenticating this
    /// private capability.
    pub fn canonical_transcript(&self) -> [u8; 253] {
        let mut output = [0u8; 253];
        output[0..40].copy_from_slice(DIRECT_PAIR_SELECTION_DOMAIN_V1);
        output[40..72].copy_from_slice(&self.selection_transcript_id);
        output[72..104].copy_from_slice(&self.economic_candidate_digest);
        output[104..136].copy_from_slice(&self.semantic_price_digest);
        output[136..168].copy_from_slice(&self.buy_order_id);
        output[168..200].copy_from_slice(&self.sell_order_id);
        output[200] = self.buy_order_index;
        output[201] = self.sell_order_index;
        output[202] = self.outcome;
        output[203] = self.outcome_count;
        output[204..212].copy_from_slice(&self.quantity.to_le_bytes());
        output[212..220].copy_from_slice(&self.price_units_per_egg.to_le_bytes());
        output[220..228].copy_from_slice(&self.price_scale.to_le_bytes());
        output[228..244].copy_from_slice(&self.consideration_price_units.to_le_bytes());
        output[244..252].copy_from_slice(&self.consideration_cash_atoms.to_le_bytes());
        output[252] = self.boundary.byte();
        output
    }
}

/// Re-execute RelationV2 and mint the sole Direct settlement capability.
pub fn authenticate_selected_direct_pair_v1<A: AuthenticatedDirectSelectionAuthorityV1 + ?Sized>(
    authority: &A,
    selection_transcript_id: [u8; 32],
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
) -> Result<SelectedDirectPairV1, DirectPairErrorV1> {
    if all_zero(&selection_transcript_id) {
        return Err(DirectPairErrorV1::ZeroSelectionIdentity);
    }
    if usize::from(book.len) != 2 {
        return Err(DirectPairErrorV1::WrongBookShape);
    }
    let economics = verify_economic_candidate_v2(domain, book, price, candidate)?;
    if candidate.virtual_split != 0 || candidate.virtual_merge != 0 {
        return Err(DirectPairErrorV1::VirtualConversion);
    }

    let first = book.orders[0];
    let second = book.orders[1];
    let (buy, buy_index, sell, sell_index) = match (first.side, second.side) {
        (Side::Buy, Side::Sell) => (first, 0u8, second, 1u8),
        (Side::Sell, Side::Buy) => (second, 1u8, first, 0u8),
        _ => return Err(DirectPairErrorV1::WrongSidePartition),
    };
    let buy_outcome = single_egg_outcome(domain.outcome_count, &buy.coefficients)?;
    let sell_outcome = single_egg_outcome(domain.outcome_count, &sell.coefficients)?;
    if buy_outcome != sell_outcome {
        return Err(DirectPairErrorV1::OutcomeMismatch);
    }
    let buy_fill = candidate.fills[usize::from(buy_index)];
    let sell_fill = candidate.fills[usize::from(sell_index)];
    if buy_fill == 0 || buy_fill != sell_fill {
        return Err(DirectPairErrorV1::FillMismatch);
    }

    require_exact_economics(&economics, buy_outcome, buy_fill)?;
    let price_units_per_egg = price.prices[usize::from(buy_outcome)];
    let consideration_price_units = u128::from(buy_fill)
        .checked_mul(u128::from(price_units_per_egg))
        .ok_or(DirectPairErrorV1::ArithmeticOverflow)?;
    let scale = u128::from(domain.price_scale);
    if consideration_price_units % scale != 0 {
        return Err(DirectPairErrorV1::InexactCashConversion);
    }
    let consideration_cash_atoms = u64::try_from(consideration_price_units / scale)
        .map_err(|_| DirectPairErrorV1::ArithmeticOverflow)?;

    authority.authenticate_selected_pair(
        selection_transcript_id,
        domain,
        book,
        price,
        candidate,
        &economics,
    )?;

    Ok(SelectedDirectPairV1 {
        selection_transcript_id,
        economic_candidate_digest: economics.economic_candidate_digest,
        semantic_price_digest: price.semantic_price_digest,
        buy_order_id: buy.order_id,
        sell_order_id: sell.order_id,
        buy_order_index: buy_index,
        sell_order_index: sell_index,
        outcome: buy_outcome,
        outcome_count: domain.outcome_count,
        quantity: buy_fill,
        price_units_per_egg,
        price_scale: domain.price_scale,
        consideration_price_units,
        consideration_cash_atoms,
        boundary: DirectCashBoundaryV1::ExactOnly,
    })
}

/// Frame-safe Direct specialization of the exact RelationV2 two-row book.
///
/// Candidate identity and ScoreV2-Q are byte-for-byte the canonical
/// general-width RelationV2 result with 62 empty rows and 62 zero fills.
pub fn authenticate_compact_selected_direct_pair_v1<
    A: AuthenticatedDirectSelectionAuthorityV1 + ?Sized,
>(
    authority: &A,
    selection_transcript_id: [u8; 32],
    domain: &EconomicDomainV2,
    book: &DirectEconomicBookV1,
    price: &PricePreconditionV2,
    candidate: DirectEconomicCandidateV1,
) -> Result<SelectedDirectPairV1, DirectPairErrorV1> {
    if all_zero(&selection_transcript_id) {
        return Err(DirectPairErrorV1::ZeroSelectionIdentity);
    }
    if book.len != 2 {
        return Err(DirectPairErrorV1::WrongBookShape);
    }
    let economics = verify_two_order_economic_candidate_v2(
        domain,
        &book.orders,
        price,
        candidate.fills,
        candidate.honored_aon_mask,
    )?;
    let first = book.orders[0];
    let second = book.orders[1];
    let (buy, buy_index, sell, sell_index) = match (first.side, second.side) {
        (Side::Buy, Side::Sell) => (first, 0u8, second, 1u8),
        (Side::Sell, Side::Buy) => (second, 1u8, first, 0u8),
        _ => return Err(DirectPairErrorV1::WrongSidePartition),
    };
    let buy_outcome = single_egg_outcome(domain.outcome_count, &buy.coefficients)?;
    let sell_outcome = single_egg_outcome(domain.outcome_count, &sell.coefficients)?;
    if buy_outcome != sell_outcome {
        return Err(DirectPairErrorV1::OutcomeMismatch);
    }
    let buy_fill = candidate.fills[usize::from(buy_index)];
    let sell_fill = candidate.fills[usize::from(sell_index)];
    if buy_fill == 0 || buy_fill != sell_fill {
        return Err(DirectPairErrorV1::FillMismatch);
    }
    require_exact_economics(&economics, buy_outcome, buy_fill)?;
    let price_units_per_egg = price.prices[usize::from(buy_outcome)];
    let consideration_price_units = u128::from(buy_fill)
        .checked_mul(u128::from(price_units_per_egg))
        .ok_or(DirectPairErrorV1::ArithmeticOverflow)?;
    let scale = u128::from(domain.price_scale);
    if consideration_price_units % scale != 0 {
        return Err(DirectPairErrorV1::InexactCashConversion);
    }
    let consideration_cash_atoms = u64::try_from(consideration_price_units / scale)
        .map_err(|_| DirectPairErrorV1::ArithmeticOverflow)?;
    authority.authenticate_compact_selected_pair(
        selection_transcript_id,
        domain,
        &book.orders,
        price,
        candidate,
        &economics,
    )?;
    Ok(SelectedDirectPairV1 {
        selection_transcript_id,
        economic_candidate_digest: economics.economic_candidate_digest,
        semantic_price_digest: price.semantic_price_digest,
        buy_order_id: buy.order_id,
        sell_order_id: sell.order_id,
        buy_order_index: buy_index,
        sell_order_index: sell_index,
        outcome: buy_outcome,
        outcome_count: domain.outcome_count,
        quantity: buy_fill,
        price_units_per_egg,
        price_scale: domain.price_scale,
        consideration_price_units,
        consideration_cash_atoms,
        boundary: DirectCashBoundaryV1::ExactOnly,
    })
}

/// Verify one compact Direct candidate through the RelationV2-owned projection.
pub fn verify_compact_direct_candidate_v1(
    domain: &EconomicDomainV2,
    book: &DirectEconomicBookV1,
    price: &PricePreconditionV2,
    candidate: DirectEconomicCandidateV1,
) -> Result<VerifiedEconomicsV2, DirectPairErrorV1> {
    if book.len != 2 {
        return Err(DirectPairErrorV1::WrongBookShape);
    }
    verify_two_order_economic_candidate_v2(
        domain,
        &book.orders,
        price,
        candidate.fills,
        candidate.honored_aon_mask,
    )
    .map_err(DirectPairErrorV1::Economic)
}

fn single_egg_outcome(
    outcome_count: u8,
    coefficients: &[u64; MAX_OUTCOMES],
) -> Result<u8, DirectPairErrorV1> {
    let active = usize::from(outcome_count);
    let mut selected = None;
    let mut outcome = 0usize;
    while outcome < active {
        match coefficients[outcome] {
            0 => {}
            1 if selected.is_none() => {
                selected = Some(
                    u8::try_from(outcome).map_err(|_| DirectPairErrorV1::ArithmeticOverflow)?,
                );
            }
            _ => return Err(DirectPairErrorV1::NotSingleEgg),
        }
        outcome += 1;
    }
    while outcome < MAX_OUTCOMES {
        if coefficients[outcome] != 0 {
            return Err(DirectPairErrorV1::NotSingleEgg);
        }
        outcome += 1;
    }
    selected.ok_or(DirectPairErrorV1::NotSingleEgg)
}

fn require_exact_economics(
    economics: &VerifiedEconomicsV2,
    selected_outcome: u8,
    quantity: u64,
) -> Result<(), DirectPairErrorV1> {
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        let expected = if outcome == usize::from(selected_outcome) {
            quantity
        } else {
            0
        };
        if economics.aggregate_buy_flow[outcome] != expected
            || economics.aggregate_sell_flow[outcome] != expected
            || economics.direct_flow[outcome] != expected
        {
            return Err(DirectPairErrorV1::EconomicMismatch);
        }
        outcome += 1;
    }
    Ok(())
}

fn all_zero(value: &[u8; 32]) -> bool {
    let mut index = 0usize;
    while index < value.len() {
        if value[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}
