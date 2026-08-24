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
use clutch_fee_runtime_contract::allocation::{FeeEnvelopeFundingV1, FeeEnvelopeV1};
pub use clutch_fee_runtime_contract::selected::SelectedCompositeFeeV2;
use clutch_fee_runtime_contract::selected::{
    AssessmentBoundaryV1, OwnerFeeAssessmentV1, OwnerFeeCarryV1,
};
pub use clutch_fee_runtime_contract::weight_v2::{
    CompositeFeeWeightRowV2, CompositeFeeWeightTranscriptV2,
};
use clutch_fee_runtime_contract::weight_v2::{
    composite_fee_hamilton_share_v2, composite_fee_weight_transcript_from_indexed_rows_v2,
    composite_fee_weight_transcript_v2,
};
use clutch_fee_runtime_contract::{Error as FeeError, Id as FeeId};
use clutch_general_v2_contract::Id32;
use clutch_solana_layout::reservation::{
    ReservationPlan, ORDER_KIND_PORTFOLIO, ORDER_KIND_SINGLE, RESERVATION_STATE_ACTIVE,
};
use clutch_solana_layout::reservation_v9::{canonical_reservation_id_v9, ReservationAccountV9};
use clutch_solana_layout::Hash32 as LayoutHash32;

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
    selected: &'a SelectedCompositeFeeV2,
    market: Id32,
    epoch: Id32,
    settlement_candidate: Id32,
    owner_order_set_digest: Id32,
    batch_policy_id: Id32,
    expected_executed_owner_count: u16,
    transcript: CompositeFeeWeightTranscriptV2,
}

/// Compact facts returned after one owner-ordered traversal of exact weights.
///
/// This summary is deliberately not an authenticated row cache. The private
/// SBF adapter must store every callback row, sort by Position, and pass that
/// exact compact cache through
/// `composite_fee_weight_transcript_from_indexed_rows_v2` before it can mint
/// the V3 recipient-allocation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioFeeWeightVisitSummaryV2 {
    market: Id32,
    epoch: Id32,
    settlement_candidate: Id32,
    owner_order_set_digest: Id32,
    batch_policy_id: Id32,
    common_denominator: u128,
    traversed_owner_count: u16,
    nonzero_weight_row_count: u8,
    total_weight: u128,
    collected_fee_atoms: u64,
}

impl PortfolioFeeWeightVisitSummaryV2 {
    /// Canonical Market identity from the retained Feed.
    pub const fn market(self) -> Id32 { self.market }
    /// Canonical Epoch identity from the retained Feed.
    pub const fn epoch(self) -> Id32 { self.epoch }
    /// Exact selected settlement-candidate identity.
    pub const fn settlement_candidate(self) -> Id32 { self.settlement_candidate }
    /// Complete immutable owner/order-set digest from the traversal.
    pub const fn owner_order_set_digest(self) -> Id32 { self.owner_order_set_digest }
    /// Batch-policy identity rebound to the selected-fee semantic.
    pub const fn batch_policy_id(self) -> Id32 { self.batch_policy_id }
    /// Exact common fee denominator used by every owner quote.
    pub const fn common_denominator(self) -> u128 { self.common_denominator }
    /// Distinct owners with nonzero selected execution before zero omission.
    pub const fn traversed_owner_count(self) -> u16 { self.traversed_owner_count }
    /// Exact number of nonzero callback rows requiring Position sorting.
    pub const fn nonzero_weight_row_count(self) -> u8 { self.nonzero_weight_row_count }
    /// Exact sum of all nonzero unrounded numerators.
    pub const fn total_weight(self) -> u128 { self.total_weight }
    /// Sum of exact per-owner terminal fee ceilings.
    pub const fn collected_fee_atoms(self) -> u64 { self.collected_fee_atoms }
}

/// One exact owner-netted row emitted by the traversal visitor.
///
/// Fields are private and there is no structural constructor: only the
/// traversal-backed visitor can associate an owner with its ordinary Position,
/// existing composite numerator, and terminal ceiling. The compact cache used
/// for recipient allocation retains only [`Self::weight_row`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisitedPortfolioFeeWeightRowV2 {
    owner: Id32,
    weight_row: CompositeFeeWeightRowV2,
    terminal_fee_atoms: u64,
}

/// Exact whole-owner fee certificate shared by action-24 charging and the
/// certified maker-weight plane.
///
/// The numerator is the selected CompositeDispersionFloor numerator before
/// carry or atom rounding.  It is derived beside the terminal assessment from
/// the same owner-netted payoff and selected prices, so the persisted
/// assessment work cannot substitute an order-local or consideration weight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerTerminalCompositeFeeCertificateV2 {
    carry: OwnerFeeCarryV1,
    assessment: OwnerFeeAssessmentV1,
    exact_weight_numerator: u128,
}

impl OwnerTerminalCompositeFeeCertificateV2 {
    /// Closed terminal carry for the exact owner.
    pub const fn carry(self) -> OwnerFeeCarryV1 { self.carry }
    /// Sole terminal-ceil charge for the exact owner.
    pub const fn assessment(self) -> OwnerFeeAssessmentV1 { self.assessment }
    /// Unrounded owner-netted CompositeDispersionFloor numerator.
    pub const fn exact_weight_numerator(self) -> u128 { self.exact_weight_numerator }
}

impl VisitedPortfolioFeeWeightRowV2 {
    /// Traversal-authenticated order owner before Position projection.
    pub const fn owner(self) -> Id32 { self.owner }
    /// Exact nonzero `(Position, numerator)` row to cache and sort.
    pub const fn weight_row(self) -> CompositeFeeWeightRowV2 { self.weight_row }
    /// Exact terminal owner fee ceiling under the common denominator.
    pub const fn terminal_fee_atoms(self) -> u64 { self.terminal_fee_atoms }
}

const _: () = assert!(
    core::mem::size_of::<DerivedPortfolioFeeWeightStreamV2<'static>>() <= 512
);

impl DerivedPortfolioFeeWeightStreamV2<'_> {
    /// Exact selected fee semantic used for every row quote.
    pub const fn selected(&self) -> &SelectedCompositeFeeV2 { self.selected }
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
    selected: &'a SelectedCompositeFeeV2,
    batch_policy: &FrozenPolicyV1,
) -> Result<DerivedPortfolioFeeWeightStreamV2<'a>, SettlementAdapterErrorV1> {
    let projection = traversal.projection();
    let feed = projection.feed();
    let (batch_policy_id, executed_owner_count, prices, common_denominator) =
        portfolio_fee_weight_preflight_v2(traversal, positions, selected, batch_policy)?;
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

/// Visit each traversal-derived nonzero owner weight exactly once.
///
/// Rows arrive in canonical owner order because owner netting is performed
/// once per executed owner. Position order is intentionally left to the
/// private adapter's compact heap cache. After sorting, the adapter must call
/// `composite_fee_weight_transcript_from_indexed_rows_v2`; that pure helper
/// detects duplicate Positions, omissions, extras, zero rows, and order
/// changes before the cache can feed Hamilton or the V3 encoder. No caller
/// owner, Position, weight, consideration, posted size, or count enters here.
#[inline(never)]
pub fn visit_portfolio_fee_weight_rows_v2<F>(
    traversal: &dyn SettlementTraversalAccessV5,
    positions: &dyn PortfolioFeeWeightPositionAccessV2,
    selected: &SelectedCompositeFeeV2,
    batch_policy: &FrozenPolicyV1,
    mut visit: F,
) -> Result<PortfolioFeeWeightVisitSummaryV2, SettlementAdapterErrorV1>
where
    F: FnMut(VisitedPortfolioFeeWeightRowV2) -> Result<(), SettlementAdapterErrorV1>,
{
    let projection = traversal.projection();
    let feed = projection.feed();
    let (batch_policy_id, traversed_owner_count, prices, common_denominator) =
        portfolio_fee_weight_preflight_v2(traversal, positions, selected, batch_policy)?;
    let mut nonzero_weight_row_count = 0u8;
    let mut total_weight = 0u128;
    let mut collected_fee_atoms = 0u64;
    let mut prior_owner = None;
    while let Some(owner) = next_executed_owner_after(traversal, prior_owner)? {
        if let Some(row) = owner_weight_row(
            traversal,
            positions,
            selected,
            &prices,
            common_denominator,
            owner,
        )? {
            let terminal_fee_atoms = terminal_fee_atoms_v2(row, common_denominator)?;
            visit(VisitedPortfolioFeeWeightRowV2 {
                owner,
                weight_row: row,
                terminal_fee_atoms,
            })?;
            nonzero_weight_row_count = nonzero_weight_row_count
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            total_weight = total_weight
                .checked_add(row.exact_numerator())
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            collected_fee_atoms = collected_fee_atoms
                .checked_add(terminal_fee_atoms)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        }
        prior_owner = Some(owner);
    }
    if usize::from(nonzero_weight_row_count) > MAX_ORDERS
        || (nonzero_weight_row_count == 0) != (total_weight == 0)
        || (nonzero_weight_row_count == 0) != (collected_fee_atoms == 0)
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    Ok(PortfolioFeeWeightVisitSummaryV2 {
        market: feed.market,
        epoch: feed.epoch,
        settlement_candidate: feed.settlement_candidate_id,
        owner_order_set_digest: projection.owner_order_set_digest(),
        batch_policy_id,
        common_denominator,
        traversed_owner_count,
        nonzero_weight_row_count,
        total_weight,
        collected_fee_atoms,
    })
}

/// Derive one owner's exact terminal charge from the same owner-netted
/// selected payoff used by the certified maker-weight plane.
///
/// This is the charging-side projection of the existing weight semantic
/// owner: it streams the retained Feed and frozen pages, aggregates the whole
/// executed buy payoff once, reads the same selected prices, and applies the
/// fee runtime's sole terminal-ceil boundary. No order-local quote, caller
/// payoff, consideration surrogate, or caller fee amount enters this path.
pub fn derive_owner_terminal_composite_fee_v2(
    traversal: &dyn SettlementTraversalAccessV5,
    selected: &SelectedCompositeFeeV2,
    owner: Id32,
) -> Result<(OwnerFeeCarryV1, OwnerFeeAssessmentV1), SettlementAdapterErrorV1> {
    let certificate = certify_owner_terminal_composite_fee_v2(traversal, selected, owner)?;
    Ok((certificate.carry(), certificate.assessment()))
}

/// Certify the charging result and its pre-rounding maker-weight numerator in
/// one traversal-owned derivation.
pub fn certify_owner_terminal_composite_fee_v2(
    traversal: &dyn SettlementTraversalAccessV5,
    selected: &SelectedCompositeFeeV2,
    owner: Id32,
) -> Result<OwnerTerminalCompositeFeeCertificateV2, SettlementAdapterErrorV1> {
    if owner.is_zero()
        || selected.realm().0 != traversal.projection().realm().bytes()
        || selected.market().0 != traversal.projection().feed().market.bytes()
        || selected.epoch().0 != traversal.projection().feed().epoch.bytes()
        || selected.selected_candidate().0
            != traversal
                .projection()
                .feed()
                .settlement_candidate_id
                .bytes()
        || selected.price_scale() != traversal.projection().feed().price_scale
        || selected.outcome_count() != traversal.projection().feed().outcome_count
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let (payoff, has_buy, has_sell) = owner_executed_payoff(traversal, owner)?;
    if !has_buy && !has_sell {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let prices = traversal_prices(traversal)?;
    let quote = selected.quote_owner(&payoff, &prices, 0)?;
    if quote.exact_numerator != quote.base_numerator
        || quote.exact_denominator != quote.base_denominator
        || quote.base_denominator != selected.carry_denominator()
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let carry = OwnerFeeCarryV1::admit(selected, FeeId(owner.bytes()))?;
    let (carry, assessment) = carry.assess(
        selected,
        &payoff,
        &prices,
        AssessmentBoundaryV1::TerminalCeil,
    )?;
    if !carry.is_closed()
        || carry.remainder() != 0
        || assessment.next_carry() != 0
        || assessment.boundary() != AssessmentBoundaryV1::TerminalCeil
        || assessment.charged_atoms() != carry.paid_atoms()
        || u128::from(assessment.charged_atoms()) != quote.terminal_ceil_atoms
        || (quote.base_numerator != 0 && !has_buy)
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    Ok(OwnerTerminalCompositeFeeCertificateV2 {
        carry,
        assessment,
        exact_weight_numerator: quote.base_numerator,
    })
}

fn pristine_fee_envelope_matches_plan(
    initial_cash_atoms: u64,
    remaining_cash_atoms: u64,
    max_fee_atoms: u64,
    initial_internal: &[u64; MAX_OUTCOMES],
    remaining_internal: &[u64; MAX_OUTCOMES],
    expected: ReservationPlan,
) -> bool {
    initial_cash_atoms == expected.cash_atoms
        && remaining_cash_atoms == expected.cash_atoms
        && max_fee_atoms == expected.max_fee_atoms
        && *initial_internal == expected.internal
        && *remaining_internal == expected.internal
}

fn owner_fee_order_and_reservation_id_v2(
    traversal: &dyn SettlementTraversalAccessV5,
    owner: Id32,
    order_index: u8,
) -> Result<(crate::StreamedOwnerBlindOrderV5, Id32), SettlementAdapterErrorV1> {
    if owner.is_zero() || traversal.selected_fill(order_index)? == 0 {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let row = traversal
        .order(order_index)?
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    if row.membership().owner() != owner {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let feed = traversal.projection().feed();
    let reservation = canonical_reservation_id_v9(
        LayoutHash32(feed.market.bytes()),
        LayoutHash32(feed.epoch.bytes()),
        LayoutHash32(owner.bytes()),
        row.position_generation(),
        LayoutHash32(row.membership().order_id().bytes()),
    );
    Ok((row, Id32::new(reservation.bytes())?))
}

/// Derive the canonical signed-Reservation identity for one filled order in
/// an authenticated owner basis.
///
/// This bounded locator reads only the frozen order row and selected fill; it
/// does not reconstruct settlement membership or scan the slice tail.
pub fn derive_owner_fee_reservation_id_v2(
    traversal: &dyn SettlementTraversalAccessV5,
    owner: Id32,
    order_index: u8,
) -> Result<Id32, SettlementAdapterErrorV1> {
    owner_fee_order_and_reservation_id_v2(traversal, owner, order_index)
        .map(|(_, reservation)| reservation)
}

/// Recompute one signed fee envelope from the same frozen order row that owns
/// its selected fill.
///
/// The Reservation is not accepted as an independent statement of its cash
/// or Egg capacity. Its complete pristine envelope is rederived from the
/// authenticated page slot, retained Feed width/scale, and signed fee cap.
/// The live adapter remains responsible for the account owner, canonical PDA,
/// stored bump, and meta permissions.
pub fn derive_owner_fee_envelope_v2(
    traversal: &dyn SettlementTraversalAccessV5,
    owner: Id32,
    order_index: u8,
    reservation: ReservationAccountV9,
) -> Result<FeeEnvelopeV1, SettlementAdapterErrorV1> {
    let (row, expected_reservation) =
        owner_fee_order_and_reservation_id_v2(traversal, owner, order_index)?;
    let feed = traversal.projection().feed();
    derive_owner_fee_envelope_from_page_v2(
        feed,
        traversal.projection().price_grid_id(),
        traversal.projection().terms(),
        traversal.projection().reservation_policy(),
        owner,
        order_index,
        traversal.selected_fill(order_index)?,
        row,
        reservation,
    )
    .and_then(|envelope| {
        if envelope.intent.0 != expected_reservation.bytes() {
            Err(SettlementAdapterErrorV1::ReservationSetMismatch)
        } else {
            Ok(envelope)
        }
    })
}

/// Recompute one exact signed fee envelope from a single root-bound retained
/// Feed and one hostile-authenticated frozen page row.
///
/// This is the bounded continuation counterpart to
/// [`derive_owner_fee_envelope_v2`]. It accepts no payoff, fee, weight, or
/// Reservation identity from the caller. The adapter supplies the already
/// authenticated current authority scalars and must rejoin the accumulated
/// transcript to the complete traversal before minting any liability.
#[allow(clippy::too_many_arguments)]
pub fn derive_owner_fee_envelope_from_page_v2(
    feed: CandidateFeedHeaderV2,
    price_grid_id: Id32,
    terms: Id32,
    reservation_policy: Id32,
    owner: Id32,
    order_index: u8,
    selected_fill: u64,
    row: crate::StreamedOwnerBlindOrderV5,
    reservation: ReservationAccountV9,
) -> Result<FeeEnvelopeV1, SettlementAdapterErrorV1> {
    if owner.is_zero()
        || price_grid_id.is_zero()
        || terms.is_zero()
        || reservation_policy.is_zero()
        || order_index >= feed.order_count
        || selected_fill == 0
        || row.membership().owner() != owner
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    reservation.validate()?;
    let body = reservation.body();
    let expected_reservation = canonical_reservation_id_v9(
        LayoutHash32(feed.market.bytes()),
        LayoutHash32(feed.epoch.bytes()),
        LayoutHash32(owner.bytes()),
        row.position_generation(),
        LayoutHash32(row.membership().order_id().bytes()),
    );
    let expected_plan = ReservationPlan::for_order(
        row.membership().slot(),
        feed.outcome_count,
        feed.price_scale,
        body.max_fee_atoms,
    )?;
    let (side, order_kind, funding) = match (
        row.economic_order().side,
        row.membership().kind(),
    ) {
        (Side::Buy, crate::FrozenOrderKindV1::Single) => (
            0u8,
            ORDER_KIND_SINGLE,
            FeeEnvelopeFundingV1::BuyCashReservation,
        ),
        (Side::Buy, crate::FrozenOrderKindV1::Portfolio) => (
            0u8,
            ORDER_KIND_PORTFOLIO,
            FeeEnvelopeFundingV1::BuyCashReservation,
        ),
        (Side::Sell, crate::FrozenOrderKindV1::Single) => (
            1u8,
            ORDER_KIND_SINGLE,
            FeeEnvelopeFundingV1::NoCashReservation,
        ),
        (Side::Sell, crate::FrozenOrderKindV1::Portfolio) => (
            1u8,
            ORDER_KIND_PORTFOLIO,
            FeeEnvelopeFundingV1::NoCashReservation,
        ),
    };
    if body.reservation.bytes() != expected_reservation.bytes()
        || body.market.bytes() != feed.market.bytes()
        || body.epoch.bytes() != feed.epoch.bytes()
        || body.owner.bytes() != owner.bytes()
        || body.order_id.bytes() != row.membership().order_id().bytes()
        || body.price_grid.bytes() != price_grid_id.bytes()
        || body.terms.bytes() != terms.bytes()
        || body.policy.bytes() != reservation_policy.bytes()
        || body.position_generation != row.position_generation()
        || body.order_generation != row.membership().generation()
        || body.page_index != row.page_index()
        || body.outcome_count != feed.outcome_count
        || body.side != side
        || body.order_kind != order_kind
        || body.state != RESERVATION_STATE_ACTIVE
        || !pristine_fee_envelope_matches_plan(
            body.initial_cash_atoms,
            body.remaining_cash_atoms,
            body.max_fee_atoms,
            &body.initial_internal,
            &body.remaining_internal,
            expected_plan,
        )
        || body.release_generation != 0
        || body.entitled_units != 0
        || body.consumed_units != 0
        || body.paid_units != 0
        || body.fee_debited_atoms != 0
        || body.fee_carry_numerator != 0
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    Ok(FeeEnvelopeV1 {
        owner: FeeId(owner.bytes()),
        intent: FeeId(expected_reservation.bytes()),
        funding,
        max_fee_atoms: body.max_fee_atoms,
        debited_atoms: 0,
    })
}

/// Commit the private adapter's Position-sorted compact callback cache.
///
/// `summary` can only be produced by the one-pass traversal visitor. This
/// helper equality-binds its exact row count, common denominator, and total
/// unrounded weight to the canonical indexed transcript. A live SBF composer
/// must call it over the same private heap entries populated directly by the
/// visitor callback; packet rows or public implementations are not authority.
pub fn commit_visited_portfolio_fee_weight_cache_v2<F>(
    summary: PortfolioFeeWeightVisitSummaryV2,
    selected: &SelectedCompositeFeeV2,
    mut row_at: F,
) -> Result<CompositeFeeWeightTranscriptV2, SettlementAdapterErrorV1>
where
    F: FnMut(u8) -> Result<Option<CompositeFeeWeightRowV2>, SettlementAdapterErrorV1>,
{
    if selected.carry_denominator() != summary.common_denominator {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let mut stream_error = None;
    let transcript_result = composite_fee_weight_transcript_from_indexed_rows_v2(
        selected.fee_record(),
        summary.common_denominator,
        summary.nonzero_weight_row_count,
        |index| match row_at(index) {
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
    if transcript.len() != summary.nonzero_weight_row_count
        || transcript.total_weight() != summary.total_weight
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    Ok(transcript)
}

fn terminal_fee_atoms_v2(
    row: CompositeFeeWeightRowV2,
    common_denominator: u128,
) -> Result<u64, SettlementAdapterErrorV1> {
    if common_denominator == 0 {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let quotient = row.exact_numerator() / common_denominator;
    let remainder = row.exact_numerator() % common_denominator;
    let floor = u64::try_from(quotient)
        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
    if remainder == 0 {
        Ok(floor)
    } else {
        floor
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)
    }
}

fn portfolio_fee_weight_preflight_v2(
    traversal: &dyn SettlementTraversalAccessV5,
    positions: &dyn PortfolioFeeWeightPositionAccessV2,
    selected: &SelectedCompositeFeeV2,
    batch_policy: &FrozenPolicyV1,
) -> Result<
    (Id32, u16, [u64; MAX_OUTCOMES], u128),
    SettlementAdapterErrorV1,
> {
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
    let traversed_owner_count = count_executed_owners(traversal)?;
    if traversed_owner_count != projection.expected_owner_count() {
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
    Ok((
        batch_policy_id,
        traversed_owner_count,
        prices,
        zero_quote.base_denominator,
    ))
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
    selected: &SelectedCompositeFeeV2,
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
    selected: &SelectedCompositeFeeV2,
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
    selected: &SelectedCompositeFeeV2,
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

    #[test]
    fn signed_fee_envelope_cannot_replace_the_frozen_order_plan() {
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[1] = 17;
        let expected = ReservationPlan {
            cash_atoms: 31,
            internal,
            max_fee_atoms: 7,
            outcome_count: 2,
            order_kind: ORDER_KIND_PORTFOLIO,
            side: 0,
        };
        assert!(pristine_fee_envelope_matches_plan(
            31,
            31,
            7,
            &internal,
            &internal,
            expected,
        ));
        assert!(!pristine_fee_envelope_matches_plan(
            30,
            30,
            7,
            &internal,
            &internal,
            expected,
        ));
        let mut substituted = internal;
        substituted[1] = 18;
        assert!(!pristine_fee_envelope_matches_plan(
            31,
            31,
            7,
            &substituted,
            &substituted,
            expected,
        ));
    }
}
