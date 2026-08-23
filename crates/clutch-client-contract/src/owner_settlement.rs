//! Chain-derived General V2 owner-settlement projection.
//!
//! This contract consumes the semantic owner's fixed-capacity verified-order,
//! explicit-fee, candidate-total, and Position projections. It never equates
//! filled orders with owners: several filled orders may aggregate into one
//! lexicographically sorted 288-byte owner row.

use core::fmt;

use clutch_owner_settlement::{
    build_owner_settlement_book_v1, project_owner_disposition_v1, Error as SemanticError,
    OwnerSettlementAccumulatorV1,
};
pub use clutch_owner_settlement::{
    CandidateSettlementTotalsV1, OwnerSettlementDispositionV1, OwnerSettlementExpectationV1,
    SelectedOwnerFeeV1, SettlementSideV1, VerifiedSettlementOrderV1, MAX_ORDERS,
    OWNER_SETTLEMENT_BODY_V1_BYTES,
};

/// Public schema for the exact chain-derived owner projection.
pub const OWNER_SETTLEMENT_PROJECTION_SCHEMA: &str =
    "dragons-clutch/client/chain-derived-owner-settlement/v1";

/// One untrusted Position cash projection keyed by semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChainOwnerPositionV1 {
    /// Semantic Position owner.
    pub owner: [u8; 32],
    /// Current total Position cash atoms, including reserved cash.
    pub cash_atoms: u64,
    /// Current reserved cash atoms.
    pub reserved_cash_atoms: u64,
}

impl ChainOwnerPositionV1 {
    /// Canonical unused fixed-capacity row.
    pub const EMPTY: Self = Self {
        owner: [0; 32],
        cash_atoms: 0,
        reserved_cash_atoms: 0,
    };
}

/// Complete authenticated inputs needed to project General V2 owner rows.
#[derive(Clone, Copy, Debug)]
pub struct OwnerSettlementProjectionV1<'a> {
    /// Canonical Market identity.
    pub market: [u8; 32],
    /// Canonical Epoch identity.
    pub epoch: [u8; 32],
    /// Selected candidate identity.
    pub candidate: [u8; 32],
    /// Digest of the exact ordered owner/order membership rows.
    pub owner_order_set_digest: [u8; 32],
    /// Exact collateral conversion scale.
    pub price_scale: u64,
    /// Complete fixed-capacity verified filled-order rows.
    pub orders: &'a [VerifiedSettlementOrderV1; MAX_ORDERS],
    /// Active filled-order prefix.
    pub order_len: u8,
    /// One explicit fee row per participating owner, including zero fees.
    pub fees: &'a [SelectedOwnerFeeV1; MAX_ORDERS],
    /// Active fee prefix.
    pub fee_len: u8,
    /// Candidate totals independently decoded from selected state.
    pub expected: CandidateSettlementTotalsV1,
    /// Complete fixed-capacity current Position cash rows.
    pub positions: &'a [ChainOwnerPositionV1; MAX_ORDERS],
    /// Active Position prefix. This must equal owner count, not order count.
    pub position_len: u8,
}

/// One lexicographically sorted, open owner row and its projected terminal
/// cash disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedOwnerSettlementRowV1 {
    expectation: OwnerSettlementExpectationV1,
    body: [u8; OWNER_SETTLEMENT_BODY_V1_BYTES],
    disposition: OwnerSettlementDispositionV1,
}

impl ProjectedOwnerSettlementRowV1 {
    const EMPTY: Self = Self {
        expectation: OwnerSettlementExpectationV1::EMPTY,
        body: [0; OWNER_SETTLEMENT_BODY_V1_BYTES],
        disposition: OwnerSettlementDispositionV1 {
            debit_atoms: 0,
            credit_atoms: 0,
            selected_fee_atoms: 0,
            released_cash_atoms: 0,
            residue_price_units: 0,
            position_cash_atoms: 0,
            position_reserved_cash_atoms: 0,
        },
    };

    /// Immutable owner expectation encoded by this row.
    #[must_use]
    pub const fn expectation(&self) -> &OwnerSettlementExpectationV1 {
        &self.expectation
    }

    /// Exact 288-byte open semantic body.
    #[must_use]
    pub const fn body(&self) -> &[u8; OWNER_SETTLEMENT_BODY_V1_BYTES] {
        &self.body
    }

    /// Exact terminal cash fields if every receipt later authenticates and
    /// completes. This is not an execution receipt.
    #[must_use]
    pub const fn disposition(&self) -> OwnerSettlementDispositionV1 {
        self.disposition
    }
}

/// Canonical owner-row projection admitted by the shared client contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementProjectionPlanV1 {
    rows: [ProjectedOwnerSettlementRowV1; MAX_ORDERS],
    owner_count: u16,
    candidate_totals: CandidateSettlementTotalsV1,
    debit_atoms: u128,
    credit_atoms: u128,
    rounding_pot_price_units: u128,
}

impl OwnerSettlementProjectionPlanV1 {
    /// Exact public contract used to interpret this projection.
    #[must_use]
    pub const fn schema(&self) -> &'static str {
        OWNER_SETTLEMENT_PROJECTION_SCHEMA
    }

    /// Lexicographically owner-sorted active rows.
    #[must_use]
    pub fn rows(&self) -> &[ProjectedOwnerSettlementRowV1] {
        &self.rows[..usize::from(self.owner_count)]
    }

    /// Exact participating-owner count. It is deliberately independent of
    /// the filled-order count.
    #[must_use]
    pub const fn owner_count(&self) -> u16 {
        self.owner_count
    }

    /// Candidate totals this projection reproduced exactly.
    #[must_use]
    pub const fn candidate_totals(&self) -> CandidateSettlementTotalsV1 {
        self.candidate_totals
    }

    /// Sum of terminal owner debits in collateral atoms.
    #[must_use]
    pub const fn debit_atoms(&self) -> u128 {
        self.debit_atoms
    }

    /// Sum of terminal owner credits in collateral atoms.
    #[must_use]
    pub const fn credit_atoms(&self) -> u128 {
        self.credit_atoms
    }

    /// Exact non-fee terminal owner rounding slack.
    #[must_use]
    pub const fn rounding_pot_price_units(&self) -> u128 {
        self.rounding_pot_price_units
    }
}

/// Exact reason a chain-derived owner projection was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerSettlementProjectionRefusal {
    /// The semantic owner-row builder or codec refused an invariant.
    Semantic(SemanticError),
    /// Position cardinality differs from candidate owner count.
    PositionCardinality,
    /// No current Position row exists for a participating owner.
    MissingOwnerPosition,
    /// More than one Position row names the same participating owner.
    DuplicateOwnerPosition,
    /// A Position row names an owner absent from the canonical book.
    UnexpectedOwnerPosition,
}

impl From<SemanticError> for OwnerSettlementProjectionRefusal {
    fn from(value: SemanticError) -> Self {
        Self::Semantic(value)
    }
}

impl fmt::Display for OwnerSettlementProjectionRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Semantic(_) => "the owner-settlement semantic contract refused the projection",
            Self::PositionCardinality => {
                "Position row count differs from the selected candidate owner count"
            }
            Self::MissingOwnerPosition => "a participating owner has no projected Position row",
            Self::DuplicateOwnerPosition => {
                "a participating owner has more than one projected Position row"
            }
            Self::UnexpectedOwnerPosition => {
                "a projected Position row does not belong to a participating owner"
            }
        })
    }
}

/// Build exact canonical owner rows and prospective terminal dispositions.
///
/// Success is a client construction result, not evidence that accounts exist,
/// receipts completed, a transaction was admitted, or settlement executed.
///
/// # Errors
///
/// Refuses incomplete/duplicate order, fee, candidate-total, or Position joins
/// and every semantic owner-row invariant failure.
pub fn project_owner_settlement_v1(
    projection: &OwnerSettlementProjectionV1<'_>,
) -> Result<OwnerSettlementProjectionPlanV1, OwnerSettlementProjectionRefusal> {
    let book = build_owner_settlement_book_v1(
        projection.market,
        projection.epoch,
        projection.candidate,
        projection.owner_order_set_digest,
        projection.price_scale,
        projection.orders,
        projection.order_len,
        projection.fees,
        projection.fee_len,
        projection.expected,
    )?;
    if u16::from(projection.position_len) != book.owner_count {
        return Err(OwnerSettlementProjectionRefusal::PositionCardinality);
    }
    let positions = &projection.positions[..usize::from(projection.position_len)];
    let mut rows = [ProjectedOwnerSettlementRowV1::EMPTY; MAX_ORDERS];
    for (index, expectation) in book.rows[..usize::from(book.owner_count)]
        .iter()
        .copied()
        .enumerate()
    {
        let mut matches = positions
            .iter()
            .filter(|position| position.owner == expectation.owner);
        let position = matches
            .next()
            .ok_or(OwnerSettlementProjectionRefusal::MissingOwnerPosition)?;
        if matches.next().is_some() {
            return Err(OwnerSettlementProjectionRefusal::DuplicateOwnerPosition);
        }
        let open = OwnerSettlementAccumulatorV1::new(expectation)?;
        rows[index] = ProjectedOwnerSettlementRowV1 {
            expectation,
            body: open.encode_body()?,
            disposition: project_owner_disposition_v1(
                &expectation,
                position.cash_atoms,
                position.reserved_cash_atoms,
            )?,
        };
    }
    for position in positions {
        if !book.rows[..usize::from(book.owner_count)]
            .iter()
            .any(|row| row.owner == position.owner)
        {
            return Err(OwnerSettlementProjectionRefusal::UnexpectedOwnerPosition);
        }
    }
    Ok(OwnerSettlementProjectionPlanV1 {
        rows,
        owner_count: book.owner_count,
        candidate_totals: projection.expected,
        debit_atoms: book.debit_atoms,
        credit_atoms: book.credit_atoms,
        rounding_pot_price_units: book.rounding_pot_price_units,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection_fixture() -> (
        [VerifiedSettlementOrderV1; MAX_ORDERS],
        [SelectedOwnerFeeV1; MAX_ORDERS],
        [ChainOwnerPositionV1; MAX_ORDERS],
        CandidateSettlementTotalsV1,
    ) {
        let buyer = [0x40; 32];
        let seller = [0x20; 32];
        let mut orders = [VerifiedSettlementOrderV1 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: 0,
            slice_count: 0,
            reserved_cash_atoms: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV1 {
            owner: buyer,
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: 12_500,
            slice_count: 1,
            reserved_cash_atoms: 2,
        };
        orders[1] = VerifiedSettlementOrderV1 {
            owner: seller,
            order_index: 2,
            side: SettlementSideV1::Sell,
            consideration_price_units: 25_000,
            slice_count: 2,
            reserved_cash_atoms: 0,
        };
        orders[2] = VerifiedSettlementOrderV1 {
            owner: buyer,
            order_index: 1,
            side: SettlementSideV1::Buy,
            consideration_price_units: 12_500,
            slice_count: 1,
            reserved_cash_atoms: 2,
        };
        let mut fees = [SelectedOwnerFeeV1::EMPTY; MAX_ORDERS];
        fees[0] = SelectedOwnerFeeV1 {
            owner: buyer,
            fee_atoms: 0,
        };
        fees[1] = SelectedOwnerFeeV1 {
            owner: seller,
            fee_atoms: 0,
        };
        let mut positions = [ChainOwnerPositionV1::EMPTY; MAX_ORDERS];
        positions[0] = ChainOwnerPositionV1 {
            owner: buyer,
            cash_atoms: 10,
            reserved_cash_atoms: 4,
        };
        positions[1] = ChainOwnerPositionV1 {
            owner: seller,
            cash_atoms: 0,
            reserved_cash_atoms: 0,
        };
        let totals = CandidateSettlementTotalsV1 {
            owner_count: 2,
            buy_price_units: 25_000,
            sell_price_units: 25_000,
            selected_fee_atoms: 0,
            rounding_pot_price_units: 10_000,
            owner_slice_end_count: 4,
        };
        (orders, fees, positions, totals)
    }

    fn project<'a>(
        orders: &'a [VerifiedSettlementOrderV1; MAX_ORDERS],
        fees: &'a [SelectedOwnerFeeV1; MAX_ORDERS],
        positions: &'a [ChainOwnerPositionV1; MAX_ORDERS],
        expected: CandidateSettlementTotalsV1,
    ) -> OwnerSettlementProjectionV1<'a> {
        OwnerSettlementProjectionV1 {
            market: [1; 32],
            epoch: [2; 32],
            candidate: [3; 32],
            owner_order_set_digest: [4; 32],
            price_scale: 10_000,
            orders,
            order_len: 3,
            fees,
            fee_len: 2,
            expected,
            positions,
            position_len: 2,
        }
    }

    #[test]
    fn three_orders_project_to_two_sorted_owner_rows_and_exact_dispositions() {
        let (orders, fees, positions, totals) = projection_fixture();
        let plan =
            project_owner_settlement_v1(&project(&orders, &fees, &positions, totals)).unwrap();
        assert_eq!(plan.owner_count(), 2);
        assert_eq!(plan.schema(), OWNER_SETTLEMENT_PROJECTION_SCHEMA);
        assert_eq!(plan.rows().len(), 2);
        assert!(plan.rows()[0].expectation().owner < plan.rows()[1].expectation().owner);
        assert!(plan
            .rows()
            .iter()
            .all(|row| row.body().len() == OWNER_SETTLEMENT_BODY_V1_BYTES));
        assert_eq!(plan.rows()[0].disposition().credit_atoms, 2);
        assert_eq!(plan.rows()[1].disposition().debit_atoms, 3);
        assert_eq!(plan.rows()[1].disposition().released_cash_atoms, 1);
        assert_eq!(plan.rows()[1].disposition().position_cash_atoms, 7);
        assert_eq!(plan.candidate_totals().owner_slice_end_count, 4);
        assert_eq!(plan.rounding_pot_price_units(), 10_000);
    }

    #[test]
    fn explicit_zero_fees_and_exact_candidate_totals_are_mandatory() {
        let (orders, mut fees, positions, totals) = projection_fixture();
        fees[1] = SelectedOwnerFeeV1::EMPTY;
        assert!(matches!(
            project_owner_settlement_v1(&project(&orders, &fees, &positions, totals)),
            Err(OwnerSettlementProjectionRefusal::Semantic(_))
        ));

        let (orders, fees, positions, mut wrong_totals) = projection_fixture();
        wrong_totals.owner_count = 3;
        assert!(matches!(
            project_owner_settlement_v1(&project(&orders, &fees, &positions, wrong_totals,)),
            Err(OwnerSettlementProjectionRefusal::Semantic(_))
        ));
    }

    #[test]
    fn position_join_is_owner_exact_not_order_count_shaped() {
        let (orders, fees, mut positions, totals) = projection_fixture();
        positions[0] = positions[1];
        assert_eq!(
            project_owner_settlement_v1(&project(&orders, &fees, &positions, totals)),
            Err(OwnerSettlementProjectionRefusal::DuplicateOwnerPosition)
        );
    }

    #[test]
    fn oversized_position_prefix_refuses_before_indexing() {
        let (orders, fees, positions, totals) = projection_fixture();
        let mut projection = project(&orders, &fees, &positions, totals);
        projection.position_len = u8::MAX;
        assert_eq!(
            project_owner_settlement_v1(&projection),
            Err(OwnerSettlementProjectionRefusal::PositionCardinality)
        );
    }
}
