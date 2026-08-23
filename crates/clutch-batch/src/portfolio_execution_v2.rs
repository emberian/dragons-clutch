//! Account-ready authority for one exact RelationV2 portfolio pair.
//!
//! [`crate::relation_v2::EconomicOrderV2`] remains the sole coefficient owner.
//! This module never persists or accepts another coefficient vector. It binds
//! authenticated General SettlementRoot/retained-Feed/page membership to an exact RelationV2 order index,
//! then composes two private selection capabilities into one exclusive full
//! pair. The pair is executable only when all sixteen coefficient cells are
//! byte-for-byte equal, the selected fills consume both orders in full, and a
//! single exact scaled-integer valuation divides at the named receipt boundary.
//!
//! Account authentication stays in the Solana adapter trust boundary. The
//! adapter supplies a [`PortfolioAdapterV2`] which authenticates owner, PDA,
//! canonical body identity, generation, privileges, and exact codec
//! transitions. Successful checks mint private-field capabilities; detached
//! records and receipts are untrusted projections and grant no authority.
//!
//! This module moves no tokens and writes no accounts. Its prepared result is
//! an indivisible transition contract: both Reservations become canonical
//! `CONSUMED` postimages, both Position V3 bodies receive the exact cash/native
//! Egg effects without changing their stable incarnation generations, both
//! purpose Replay V3 accounts advance by one, and the canonical hash of one
//! replay-sensitive vector receipt preimage is exposed as a typed commitment
//! required from the complete counted active prefix of one through sixteen
//! 298-byte SettlementReceipt V5 siblings. Every typed kind-1 prestate is
//! pending with a zero commitment; delivery sets the same nonzero commitment
//! exactly once across the complete set. A live adapter must apply every named postimage and CPI
//! atomically or apply none of them.

use crate::relation_v1::MAX_OUTCOMES;
use crate::relation_v2::{
    validate_candidate_padding_v2, validate_live_order_fill_v2, EconomicBookV2,
    EconomicCandidateV2, EconomicDomainV2, EconomicErrorV2, PricePreconditionV2, Sha256V2,
};
use crate::{Side, MAX_ORDERS};

/// Exact semantic version of this account-ready portfolio authority.
pub const PORTFOLIO_EXECUTION_VERSION_V2: u8 = 2;
/// Canonical bytes of one selected-order membership record.
pub const SELECTED_PORTFOLIO_ORDER_V2_BYTES: usize = 560;
/// Canonical bytes of one exact-pair transition receipt preimage.
pub const PORTFOLIO_PAIR_RECEIPT_V2_BYTES: usize = 680;
/// Maximum scalar Receipt V5 siblings in one exact coefficient pair.
pub const PORTFOLIO_PAIR_MAX_RECEIPTS_V2: usize = MAX_OUTCOMES;

const SELECTED_ORDER_MAGIC_V2: [u8; 8] = *b"DCPSEL2\0";
const PAIR_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCPRCP2\0";
/// Byte-for-byte mirror of the domain owned by
/// `solana_layout::settlement_receipt_v5`.
///
/// `solana-layout` already depends on this kernel crate, so this crate cannot
/// import the layout constant without a dependency cycle. The layout adapter
/// must compare its owner constant with this exact fixed-width value before it
/// can implement [`PortfolioAdapterV2`].
pub const PORTFOLIO_PAIR_TRANSITION_COMMITMENT_DOMAIN_V2: &[u8; 44] =
    b"dragons-clutch/portfolio-pair-transition/v2\0";
const PAIR_EFFECTS_TRANSITION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/portfolio-pair-effects/v2\0";
/// Exact domain of the canonical pending Receipt V5 sibling-set digest.
///
/// Retirement adapters reconstruct the pre-delivery pending postimages from
/// the committed layout owner and must reproduce this domain byte-for-byte
/// before authenticating the immediate action-42 GEN1 delta.
pub const PORTFOLIO_PAIR_RECEIPT_SET_DOMAIN_V2: &[u8] =
    b"dragons-clutch/portfolio-pair-receipt-set/v2\0";
/// Canonical transcript byte for the V5 portfolio-pair transition kind.
pub const PORTFOLIO_PAIR_RECEIPT_TRANSITION_KIND_V2_BYTE: u8 = 1;

const SETTLEMENT_RECEIPT_DIRECT_END_MASK_V5: u8 = 0b0000_0011;

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);

/// Fixed-width identity used by every account and semantic binding.
pub type PortfolioIdentityV2 = [u8; 32];

/// Source layout authenticated by the General page adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PortfolioSourceOrderKindV2 {
    /// A canonical coefficient-vector Portfolio record, not a lowered single.
    Portfolio = 2,
}

/// The only valuation-to-collateral conversion admitted by this slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PortfolioValuationBoundaryV2 {
    /// Sum every `coefficient * price`, multiply once by exact filled units,
    /// then divide once by `price_scale`; any remainder is refused.
    ExactReceiptDivisionV1 = 1,
}

/// Typed optional transition commitment frozen in SettlementReceipt V5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementReceiptTransitionKindV2 {
    /// No specialized transition. The 32-byte field must remain zero forever.
    None = 0,
    /// Exact portfolio pair. A zero commitment is the authenticated pending
    /// prestate; a nonzero commitment is the immutable delivered poststate.
    PortfolioPairV2 = 1,
}

/// Canonical selected-order membership projected from an authenticated page.
///
/// This fixed-layout record deliberately omits coefficients, limits, expiry,
/// minimum-fill policy, and AON policy. Those facts have one semantic owner:
/// the exact [`crate::relation_v2::EconomicOrderV2`] at `order_index`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedPortfolioOrderRecordV2 {
    pub version: u8,
    pub outcome_count: u8,
    pub source_kind: PortfolioSourceOrderKindV2,
    pub side: Side,
    pub order_index: u8,
    pub page_slot: u8,
    /// Exact retained-Feed traversal/slice index selecting this pair.
    pub traversal_index: u16,
    pub page_index: u16,
    pub settlement_root_epoch_generation: u64,
    pub position_generation: u64,
    pub selected_fill_units: u64,
    pub market_semantics_digest: PortfolioIdentityV2,
    pub epoch_semantics_digest: PortfolioIdentityV2,
    pub economic_candidate_digest: PortfolioIdentityV2,
    pub order_set_digest: PortfolioIdentityV2,
    pub settlement_root_account_id: PortfolioIdentityV2,
    pub settlement_root_pre_semantic_id: PortfolioIdentityV2,
    pub settlement_candidate_id: PortfolioIdentityV2,
    pub retained_feed_account_id: PortfolioIdentityV2,
    pub retained_feed_semantic_id: PortfolioIdentityV2,
    pub settlement_witness_id: PortfolioIdentityV2,
    pub order_page_account_id: PortfolioIdentityV2,
    pub order_page_semantic_id: PortfolioIdentityV2,
    pub position_account_id: PortfolioIdentityV2,
    pub position_pre_semantic_id: PortfolioIdentityV2,
    pub order_id: PortfolioIdentityV2,
    pub owner_id: PortfolioIdentityV2,
}

impl SelectedPortfolioOrderRecordV2 {
    /// Encode the exact canonical fixed layout.
    pub fn encode_into(
        &self,
        output: &mut [u8; SELECTED_PORTFOLIO_ORDER_V2_BYTES],
    ) -> Result<(), PortfolioExecutionErrorV2> {
        self.validate_shape()?;
        *output = [0; SELECTED_PORTFOLIO_ORDER_V2_BYTES];
        output[0..8].copy_from_slice(&SELECTED_ORDER_MAGIC_V2);
        output[8] = self.version;
        output[9] = self.outcome_count;
        output[10] = source_order_kind_byte(self.source_kind);
        output[11] = side_byte(self.side);
        output[12] = self.order_index;
        output[13] = self.page_slot;
        output[14..16].copy_from_slice(&self.traversal_index.to_le_bytes());
        output[16..18].copy_from_slice(&self.page_index.to_le_bytes());
        output[24..32].copy_from_slice(&self.settlement_root_epoch_generation.to_le_bytes());
        output[32..40].copy_from_slice(&self.position_generation.to_le_bytes());
        output[40..48].copy_from_slice(&self.selected_fill_units.to_le_bytes());
        let identities = self.identities();
        let mut cursor = 48usize;
        let mut index = 0usize;
        while index < identities.len() {
            output[cursor..cursor + 32].copy_from_slice(identities[index]);
            cursor += 32;
            index += 1;
        }
        Ok(())
    }

    /// Decode and validate hostile fixed-layout bytes.
    pub fn decode(input: &[u8]) -> Result<Self, PortfolioExecutionErrorV2> {
        if input.len() != SELECTED_PORTFOLIO_ORDER_V2_BYTES
            || input[0..8] != SELECTED_ORDER_MAGIC_V2
        {
            return Err(PortfolioExecutionErrorV2::InvalidCodec);
        }
        if input[18..24].iter().any(|byte| *byte != 0) {
            return Err(PortfolioExecutionErrorV2::NonCanonicalPadding);
        }
        let source_kind = match input[10] {
            2 => PortfolioSourceOrderKindV2::Portfolio,
            _ => return Err(PortfolioExecutionErrorV2::UnknownSourceOrderKind),
        };
        let side = decode_side(input[11])?;
        let mut cursor = 48usize;
        let mut next_identity = || -> Result<PortfolioIdentityV2, PortfolioExecutionErrorV2> {
            let end = cursor
                .checked_add(32)
                .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
            let bytes = input
                .get(cursor..end)
                .ok_or(PortfolioExecutionErrorV2::InvalidCodec)?;
            let mut identity = [0u8; 32];
            identity.copy_from_slice(bytes);
            cursor = end;
            Ok(identity)
        };
        let value = Self {
            version: input[8],
            outcome_count: input[9],
            source_kind,
            side,
            order_index: input[12],
            page_slot: input[13],
            traversal_index: read_u16(input, 14)?,
            page_index: read_u16(input, 16)?,
            settlement_root_epoch_generation: read_u64(input, 24)?,
            position_generation: read_u64(input, 32)?,
            selected_fill_units: read_u64(input, 40)?,
            market_semantics_digest: next_identity()?,
            epoch_semantics_digest: next_identity()?,
            economic_candidate_digest: next_identity()?,
            order_set_digest: next_identity()?,
            settlement_root_account_id: next_identity()?,
            settlement_root_pre_semantic_id: next_identity()?,
            settlement_candidate_id: next_identity()?,
            retained_feed_account_id: next_identity()?,
            retained_feed_semantic_id: next_identity()?,
            settlement_witness_id: next_identity()?,
            order_page_account_id: next_identity()?,
            order_page_semantic_id: next_identity()?,
            position_account_id: next_identity()?,
            position_pre_semantic_id: next_identity()?,
            order_id: next_identity()?,
            owner_id: next_identity()?,
        };
        if cursor != SELECTED_PORTFOLIO_ORDER_V2_BYTES {
            return Err(PortfolioExecutionErrorV2::InvalidCodec);
        }
        value.validate_shape()?;
        Ok(value)
    }

    fn identities(&self) -> [&PortfolioIdentityV2; 16] {
        [
            &self.market_semantics_digest,
            &self.epoch_semantics_digest,
            &self.economic_candidate_digest,
            &self.order_set_digest,
            &self.settlement_root_account_id,
            &self.settlement_root_pre_semantic_id,
            &self.settlement_candidate_id,
            &self.retained_feed_account_id,
            &self.retained_feed_semantic_id,
            &self.settlement_witness_id,
            &self.order_page_account_id,
            &self.order_page_semantic_id,
            &self.position_account_id,
            &self.position_pre_semantic_id,
            &self.order_id,
            &self.owner_id,
        ]
    }

    fn validate_shape(&self) -> Result<(), PortfolioExecutionErrorV2> {
        if self.version != PORTFOLIO_EXECUTION_VERSION_V2 {
            return Err(PortfolioExecutionErrorV2::UnknownVersion);
        }
        if !(2..=MAX_OUTCOMES).contains(&usize::from(self.outcome_count)) {
            return Err(PortfolioExecutionErrorV2::InvalidOutcomeCount);
        }
        if self.selected_fill_units == 0
            || self.settlement_root_epoch_generation == 0
            || self.position_generation == 0
        {
            return Err(PortfolioExecutionErrorV2::InvalidGenerationOrUnits);
        }
        let identities = self.identities();
        let mut index = 0usize;
        while index < identities.len() {
            if is_zero_identity(identities[index]) {
                return Err(PortfolioExecutionErrorV2::ZeroIdentity);
            }
            index += 1;
        }
        if self.settlement_root_account_id == self.retained_feed_account_id
            || self.settlement_root_account_id == self.order_page_account_id
            || self.settlement_root_account_id == self.position_account_id
            || self.retained_feed_account_id == self.order_page_account_id
            || self.retained_feed_account_id == self.position_account_id
            || self.order_page_account_id == self.position_account_id
        {
            return Err(PortfolioExecutionErrorV2::AliasedAccount);
        }
        Ok(())
    }
}

/// Account role at the private adapter boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PortfolioAccountRoleV2 {
    SettlementRoot = 1,
    RetainedFeed = 2,
    OrderPage = 3,
    Position = 4,
    Reservation = 5,
    Replay = 6,
    SettlementReceipt = 7,
}

/// Exact account observation the outer adapter must authenticate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioAccountExpectationV2 {
    pub role: PortfolioAccountRoleV2,
    pub account_id: PortfolioIdentityV2,
    pub owner_program_id: PortfolioIdentityV2,
    pub data_semantic_id: PortfolioIdentityV2,
    /// Incarnation generation when the account schema owns one; otherwise
    /// `None`. Absence is not represented by a synthetic numeric value.
    pub generation: Option<u64>,
    pub writable: bool,
    pub must_exist: bool,
}

/// Exact join the adapter must prove between hostile page/selection bytes and
/// the existing RelationV2 semantic owners. No coefficient copy is present:
/// the adapter compares the decoded page slot directly with `relation_order`
/// supplied to [`PortfolioAdapterV2::authenticate_selection_membership`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioSelectionMembershipExpectationV2 {
    pub record: SelectedPortfolioOrderRecordV2,
}

/// One exact codec transition the adapter must reproduce before account writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioTransitionExpectationV2 {
    pub role: PortfolioAccountRoleV2,
    pub account_id: PortfolioIdentityV2,
    pub pre_semantic_id: PortfolioIdentityV2,
    pub post_semantic_id: PortfolioIdentityV2,
    pub stable_generation: Option<u64>,
    pub pre_replay_ordinal: u64,
    pub post_replay_ordinal: u64,
    pub cash_debit_atoms: u64,
    pub cash_credit_atoms: u64,
    pub reserved_cash_release_atoms: u64,
    pub claim_debits: [u64; MAX_OUTCOMES],
    pub claim_credits: [u64; MAX_OUTCOMES],
    pub reservation_consumed: bool,
}

/// Private authenticated-adapter seam.
///
/// Implementations live in the SBF adapter and must check actual owner/PDA,
/// exact canonical bytes, current generation, and privileges. Transition
/// methods additionally prove that each account owner's canonical codec maps
/// the exact preimage to the requested postimage and semantic identity.
pub trait PortfolioAdapterV2 {
    fn authenticate_account(&self, expected: &PortfolioAccountExpectationV2) -> bool;
    fn authenticate_selection_membership(
        &self,
        expected: &PortfolioSelectionMembershipExpectationV2,
        relation_order: &crate::relation_v2::EconomicOrderV2,
        candidate: &EconomicCandidateV2,
    ) -> bool;
    fn authenticate_transition(&self, expected: &PortfolioTransitionExpectationV2) -> bool;
    /// Decode every exact V5 sibling pre-data identity, reproduce canonical
    /// `commit_portfolio_pair_delivery` on the complete ordered set, and
    /// authenticate every resulting post-data identity and typed commitment.
    fn derive_settlement_receipt_v5_post_data_ids(
        &self,
        expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[PortfolioIdentityV2; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]>;
}

/// Capability proving selected page membership was joined to one RelationV2 row.
///
/// The private field prevents safe callers from promoting a decoded record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSelectedPortfolioOrderV2 {
    record: SelectedPortfolioOrderRecordV2,
    order: crate::relation_v2::EconomicOrderV2,
}

impl AuthenticatedSelectedPortfolioOrderV2 {
    pub const fn record(&self) -> &SelectedPortfolioOrderRecordV2 {
        &self.record
    }

    pub const fn order_id(&self) -> PortfolioIdentityV2 {
        self.record.order_id
    }

    pub const fn owner_id(&self) -> PortfolioIdentityV2 {
        self.record.owner_id
    }

    pub const fn order_index(&self) -> u8 {
        self.record.order_index
    }

    pub const fn position_account_id(&self) -> PortfolioIdentityV2 {
        self.record.position_account_id
    }

    /// Exact RelationV2 order authenticated at `record.order_index`.
    ///
    /// RelationV2 remains the sole coefficient owner.  This value is exposed
    /// only through the private membership capability; a detached order value
    /// cannot recreate that capability.
    pub const fn economic_order(&self) -> &crate::relation_v2::EconomicOrderV2 {
        &self.order
    }
}

/// Authenticate one fixed selected-order projection and bind it to RelationV2.
pub fn authenticate_selected_portfolio_order_v2<A: PortfolioAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioIdentityV2,
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    candidate: &EconomicCandidateV2,
    expected_economic_candidate_digest: PortfolioIdentityV2,
    record: SelectedPortfolioOrderRecordV2,
) -> Result<AuthenticatedSelectedPortfolioOrderV2, PortfolioExecutionErrorV2> {
    authenticate_selected_portfolio_order_with_access_v2(
        adapter,
        owner_program_id,
        domain,
        book,
        candidate,
        expected_economic_candidate_digest,
        record,
        SelectedAccountAccessV2::Delivery,
    )
}

/// Authenticate one materialization-time selected order under the atomic root
/// transition account contract: SettlementRoot writable and Position exact
/// read-only. This is deliberately a named, disjoint entrypoint; callers cannot
/// change either privilege through a boolean or public enum argument.
pub fn authenticate_selected_portfolio_order_for_materialization_v2<A: PortfolioAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioIdentityV2,
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    candidate: &EconomicCandidateV2,
    expected_economic_candidate_digest: PortfolioIdentityV2,
    record: SelectedPortfolioOrderRecordV2,
) -> Result<AuthenticatedSelectedPortfolioOrderV2, PortfolioExecutionErrorV2> {
    authenticate_selected_portfolio_order_with_access_v2(
        adapter,
        owner_program_id,
        domain,
        book,
        candidate,
        expected_economic_candidate_digest,
        record,
        SelectedAccountAccessV2::Materialization,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectedAccountAccessV2 {
    Delivery,
    Materialization,
}

#[allow(clippy::too_many_arguments)]
fn authenticate_selected_portfolio_order_with_access_v2<A: PortfolioAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioIdentityV2,
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    candidate: &EconomicCandidateV2,
    expected_economic_candidate_digest: PortfolioIdentityV2,
    record: SelectedPortfolioOrderRecordV2,
    account_access: SelectedAccountAccessV2,
) -> Result<AuthenticatedSelectedPortfolioOrderV2, PortfolioExecutionErrorV2> {
    record.validate_shape()?;
    domain.validate().map_err(PortfolioExecutionErrorV2::Economic)?;
    book.validate(domain).map_err(PortfolioExecutionErrorV2::Economic)?;
    validate_candidate_padding_v2(candidate, book.len)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    if is_zero_identity(&owner_program_id) {
        return Err(PortfolioExecutionErrorV2::ZeroIdentity);
    }
    if record.market_semantics_digest != domain.market_semantics_digest
        || record.epoch_semantics_digest != domain.epoch_semantics_digest
        || record.outcome_count != domain.outcome_count
    {
        return Err(PortfolioExecutionErrorV2::DomainMismatch);
    }
    if record.economic_candidate_digest != expected_economic_candidate_digest
        || is_zero_identity(&expected_economic_candidate_digest)
    {
        return Err(PortfolioExecutionErrorV2::CandidateMismatch);
    }
    let at = usize::from(record.order_index);
    if at >= usize::from(book.len) {
        return Err(PortfolioExecutionErrorV2::OrderMismatch);
    }
    let order = book.orders[at];
    if record.source_kind != PortfolioSourceOrderKindV2::Portfolio
        || record.order_id != order.order_id
        || record.side != order.side
        || record.selected_fill_units != candidate.fills[at]
        || record.selected_fill_units == 0
    {
        return Err(PortfolioExecutionErrorV2::OrderMismatch);
    }

    let expectations = [
        PortfolioAccountExpectationV2 {
            role: PortfolioAccountRoleV2::SettlementRoot,
            account_id: record.settlement_root_account_id,
            owner_program_id,
            data_semantic_id: record.settlement_root_pre_semantic_id,
            generation: Some(record.settlement_root_epoch_generation),
            writable: account_access == SelectedAccountAccessV2::Materialization,
            must_exist: true,
        },
        PortfolioAccountExpectationV2 {
            role: PortfolioAccountRoleV2::RetainedFeed,
            account_id: record.retained_feed_account_id,
            owner_program_id,
            data_semantic_id: record.retained_feed_semantic_id,
            generation: None,
            writable: false,
            must_exist: true,
        },
        PortfolioAccountExpectationV2 {
            role: PortfolioAccountRoleV2::OrderPage,
            account_id: record.order_page_account_id,
            owner_program_id,
            data_semantic_id: record.order_page_semantic_id,
            // OrderPage V5 owns no page-incarnation generation. Its exact PDA,
            // canonical body, and V5 page digest are the complete authority.
            generation: None,
            writable: false,
            must_exist: true,
        },
        PortfolioAccountExpectationV2 {
            role: PortfolioAccountRoleV2::Position,
            account_id: record.position_account_id,
            owner_program_id,
            data_semantic_id: record.position_pre_semantic_id,
            generation: Some(record.position_generation),
            writable: account_access == SelectedAccountAccessV2::Delivery,
            must_exist: true,
        },
    ];
    let mut index = 0usize;
    while index < expectations.len() {
        if !adapter.authenticate_account(&expectations[index]) {
            return Err(PortfolioExecutionErrorV2::AuthenticationFailed {
                role: expectations[index].role,
            });
        }
        index += 1;
    }
    if !adapter.authenticate_selection_membership(
        &PortfolioSelectionMembershipExpectationV2 { record },
        &order,
        candidate,
    ) {
        return Err(PortfolioExecutionErrorV2::SelectionMembershipAuthenticationFailed);
    }
    Ok(AuthenticatedSelectedPortfolioOrderV2 { record, order })
}

/// Private capability for one exact, exclusive, full coefficient-vector pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPortfolioPairV2 {
    buyer: AuthenticatedSelectedPortfolioOrderV2,
    seller: AuthenticatedSelectedPortfolioOrderV2,
    price_semantics_digest: PortfolioIdentityV2,
    boundary: PortfolioValuationBoundaryV2,
    pair_units: u64,
    unit_value_price_units: u128,
    total_value_price_units: u128,
    consideration_atoms: u64,
    prices: [u64; MAX_OUTCOMES],
    payoff: [u64; MAX_OUTCOMES],
}

impl AuthenticatedPortfolioPairV2 {
    pub const fn buyer(&self) -> &AuthenticatedSelectedPortfolioOrderV2 {
        &self.buyer
    }

    pub const fn seller(&self) -> &AuthenticatedSelectedPortfolioOrderV2 {
        &self.seller
    }

    pub const fn pair_units(&self) -> u64 {
        self.pair_units
    }

    pub const fn unit_value_price_units(&self) -> u128 {
        self.unit_value_price_units
    }

    pub const fn total_value_price_units(&self) -> u128 {
        self.total_value_price_units
    }

    pub const fn consideration_atoms(&self) -> u64 {
        self.consideration_atoms
    }

    pub const fn payoff(&self) -> &[u64; MAX_OUTCOMES] {
        &self.payoff
    }

    pub const fn prices(&self) -> &[u64; MAX_OUTCOMES] {
        &self.prices
    }
}

/// One hostile retained-Feed scalar row projected without account identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioReceiptSiblingTraversalV2 {
    pub slice_index: u16,
    pub sequence: u64,
    pub buy_order_index: u8,
    pub sell_order_index: u8,
    pub outcome: u8,
    pub quantity: u64,
    pub price: u64,
}

impl PortfolioReceiptSiblingTraversalV2 {
    /// Canonical inactive traversal padding.
    pub const EMPTY: Self = Self {
        slice_index: 0,
        sequence: 0,
        buy_order_index: 0,
        sell_order_index: 0,
        outcome: 0,
        quantity: 0,
        price: 0,
    };
}

/// Hostile bounded active-prefix projection of retained Feed traversal rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioReceiptSiblingTraversalSetV2 {
    pub sibling_count: u8,
    pub siblings: [PortfolioReceiptSiblingTraversalV2; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
}

/// Private capability proving the complete scalar sibling set for one pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPortfolioReceiptSiblingSetV2 {
    pair: AuthenticatedPortfolioPairV2,
    traversal: PortfolioReceiptSiblingTraversalSetV2,
}

impl AuthenticatedPortfolioReceiptSiblingSetV2 {
    pub const fn pair(&self) -> AuthenticatedPortfolioPairV2 {
        self.pair
    }

    pub const fn sibling_count(&self) -> u8 {
        self.traversal.sibling_count
    }

    pub fn sibling(&self, index: u8) -> Option<&PortfolioReceiptSiblingTraversalV2> {
        let at = usize::from(index);
        if at < usize::from(self.traversal.sibling_count) {
            self.traversal.siblings.get(at)
        } else {
            None
        }
    }
}

/// Authenticate the exhaustive, ordered retained-Feed scalar decomposition.
///
/// The count is derived from the nonzero exact payoff coordinates. The first
/// row must be the traversal coordinate already authenticated by both selected
/// endpoints, every row must name those same two dense order indices, and the
/// inactive tail must be canonical zero. Packet counts are never authority.
pub fn authenticate_portfolio_receipt_sibling_set_v2(
    pair: AuthenticatedPortfolioPairV2,
    traversal: PortfolioReceiptSiblingTraversalSetV2,
) -> Result<AuthenticatedPortfolioReceiptSiblingSetV2, PortfolioExecutionErrorV2> {
    let count = usize::from(traversal.sibling_count);
    if count == 0 || count > PORTFOLIO_PAIR_MAX_RECEIPTS_V2 {
        return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
    }
    let mut sibling_index = 0usize;
    let mut outcome = 0usize;
    while outcome < usize::from(pair.buyer.record.outcome_count) {
        if pair.payoff[outcome] != 0 {
            if sibling_index >= count {
                return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
            }
            let sibling = traversal.siblings[sibling_index];
            let expected_outcome = u8::try_from(outcome)
                .map_err(|_| PortfolioExecutionErrorV2::ArithmeticOverflow)?;
            if sibling.sequence != u64::from(sibling.slice_index) + 1
                || sibling.buy_order_index != pair.buyer.record.order_index
                || sibling.sell_order_index != pair.seller.record.order_index
                || sibling.outcome != expected_outcome
                || sibling.quantity != pair.payoff[outcome]
                || sibling.price != pair.prices[outcome]
                || (sibling_index == 0
                    && sibling.slice_index != pair.buyer.record.traversal_index)
                || (sibling_index != 0
                    && sibling.slice_index
                        <= traversal.siblings[sibling_index - 1].slice_index)
            {
                return Err(PortfolioExecutionErrorV2::FeedTraversalMismatch);
            }
            sibling_index += 1;
        }
        outcome += 1;
    }
    if sibling_index != count {
        return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
    }
    while sibling_index < PORTFOLIO_PAIR_MAX_RECEIPTS_V2 {
        if traversal.siblings[sibling_index] != PortfolioReceiptSiblingTraversalV2::EMPTY {
            return Err(PortfolioExecutionErrorV2::NonCanonicalPadding);
        }
        sibling_index += 1;
    }
    Ok(AuthenticatedPortfolioReceiptSiblingSetV2 { pair, traversal })
}

/// Compose two authenticated selections into the only atomic pair shape.
pub fn authenticate_exact_portfolio_pair_v2(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
    boundary: PortfolioValuationBoundaryV2,
    first: AuthenticatedSelectedPortfolioOrderV2,
    second: AuthenticatedSelectedPortfolioOrderV2,
) -> Result<AuthenticatedPortfolioPairV2, PortfolioExecutionErrorV2> {
    domain.validate().map_err(PortfolioExecutionErrorV2::Economic)?;
    book.validate(domain).map_err(PortfolioExecutionErrorV2::Economic)?;
    price.validate(domain).map_err(PortfolioExecutionErrorV2::Economic)?;
    validate_candidate_padding_v2(candidate, book.len)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    if boundary != PortfolioValuationBoundaryV2::ExactReceiptDivisionV1 {
        return Err(PortfolioExecutionErrorV2::UnsupportedRoundingBoundary);
    }
    if candidate.virtual_split != 0 || candidate.virtual_merge != 0 {
        return Err(PortfolioExecutionErrorV2::VirtualConversionNotPairable);
    }
    let (buyer, seller) = match (first.record.side, second.record.side) {
        (Side::Buy, Side::Sell) => (first, second),
        (Side::Sell, Side::Buy) => (second, first),
        _ => return Err(PortfolioExecutionErrorV2::SideMismatch),
    };
    if !shared_selection(&buyer.record, &seller.record) {
        return Err(PortfolioExecutionErrorV2::SelectionMismatch);
    }
    if buyer.record.order_id == seller.record.order_id
        || buyer.record.owner_id == seller.record.owner_id
        || buyer.record.position_account_id == seller.record.position_account_id
    {
        return Err(PortfolioExecutionErrorV2::AliasedPairEndpoint);
    }
    let buy_at = usize::from(buyer.record.order_index);
    let sell_at = usize::from(seller.record.order_index);
    if buy_at >= usize::from(book.len) || sell_at >= usize::from(book.len) || buy_at == sell_at {
        return Err(PortfolioExecutionErrorV2::OrderMismatch);
    }
    let buy = book.orders[buy_at];
    let sell = book.orders[sell_at];
    validate_live_order_fill_v2(domain, price, candidate, &buy, buyer.record.order_index)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    validate_live_order_fill_v2(domain, price, candidate, &sell, seller.record.order_index)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    if buy.coefficients != sell.coefficients {
        return Err(PortfolioExecutionErrorV2::CoefficientMismatch);
    }
    let units = buyer.record.selected_fill_units;
    if units != seller.record.selected_fill_units
        || units != buy.quantity
        || units != sell.quantity
    {
        return Err(PortfolioExecutionErrorV2::NotExactFullPair);
    }
    let mut index = 0usize;
    while index < usize::from(book.len) {
        if index != buy_at && index != sell_at && candidate.fills[index] != 0 {
            return Err(PortfolioExecutionErrorV2::NonExclusivePair);
        }
        index += 1;
    }
    let mut unit_value = 0u128;
    let mut payoff = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < usize::from(domain.outcome_count) {
        let value_term = u128::from(buy.coefficients[outcome])
            .checked_mul(u128::from(price.prices[outcome]))
            .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
        unit_value = unit_value
            .checked_add(value_term)
            .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
        payoff[outcome] = buy.coefficients[outcome]
            .checked_mul(units)
            .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
        outcome += 1;
    }
    while outcome < MAX_OUTCOMES {
        if buy.coefficients[outcome] != 0 || payoff[outcome] != 0 {
            return Err(PortfolioExecutionErrorV2::NonCanonicalClaimPadding);
        }
        outcome += 1;
    }
    let total = unit_value
        .checked_mul(u128::from(units))
        .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
    let scale = u128::from(domain.price_scale);
    if total % scale != 0 {
        return Err(PortfolioExecutionErrorV2::InexactValuation);
    }
    let consideration = u64::try_from(total / scale)
        .map_err(|_| PortfolioExecutionErrorV2::ConsiderationOverflow)?;
    Ok(AuthenticatedPortfolioPairV2 {
        buyer,
        seller,
        price_semantics_digest: price.semantic_price_digest,
        boundary,
        pair_units: units,
        unit_value_price_units: unit_value,
        total_value_price_units: total,
        consideration_atoms: consideration,
        prices: price.prices,
        payoff,
    })
}

/// Canonical Reservation lifecycle needed by exact pair execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PortfolioReservationLifecycleV2 {
    Entitled = 1,
    Consumed = 2,
}

/// Adapter-authenticated Reservation prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioReservationPrestateV2 {
    pub account_id: PortfolioIdentityV2,
    pub semantic_id: PortfolioIdentityV2,
    pub generation: u64,
    pub lifecycle: PortfolioReservationLifecycleV2,
    pub owner_id: PortfolioIdentityV2,
    pub order_id: PortfolioIdentityV2,
    pub position_account_id: PortfolioIdentityV2,
    pub position_generation: u64,
    /// Exact whole-order fill stamped by General entitlement.
    pub entitled_units: u64,
    /// Cumulative Egg-delivery ledger before this indivisible full-pair step.
    pub consumed_units: u64,
    /// Cumulative consideration-payment ledger before this step.
    pub paid_units: u64,
    pub remaining_cash_atoms: u64,
    pub remaining_claim_atoms: [u64; MAX_OUTCOMES],
    pub maximum_fee_atoms: u64,
}

/// Adapter-authenticated Position V3 prestate, projected without changing its owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPositionPrestateV2 {
    pub account_id: PortfolioIdentityV2,
    pub semantic_id: PortfolioIdentityV2,
    pub owner_id: PortfolioIdentityV2,
    pub generation: u64,
    pub cash_atoms: u64,
    pub reserved_cash_atoms: u64,
    pub native_eggs: [u64; MAX_OUTCOMES],
    pub outstanding_reservations: u64,
}

/// Adapter-authenticated purpose Replay V3 prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioReplayPrestateV2 {
    pub account_id: PortfolioIdentityV2,
    pub semantic_id: PortfolioIdentityV2,
    pub ordinal: u64,
}

/// Post-semantic identities produced by the account owners' canonical codecs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairPostSemanticIdsV2 {
    pub buyer_reservation: PortfolioIdentityV2,
    pub seller_reservation: PortfolioIdentityV2,
    pub buyer_position: PortfolioIdentityV2,
    pub seller_position: PortfolioIdentityV2,
    pub buyer_replay: PortfolioIdentityV2,
    pub seller_replay: PortfolioIdentityV2,
    /// Canonical active-prefix Receipt V5 postimage/data identities. Hostile
    /// execution input must leave every cell zero because layout derives them
    /// only after the shared portfolio commitment is known. Prepared output
    /// fills exactly the active sibling prefix and leaves the tail zero.
    pub settlement_receipts: [PortfolioIdentityV2; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
}

/// Exact authenticated SettlementReceipt V5 prestate needed by this action.
///
/// The persisted owner is the frozen 298-byte V5 layout: 217-byte V4 semantic
/// body, `transition_kind:u8`, `transition_commitment:[u8;32]`, and 48-byte
/// rent owner. The pair hash commits every transition field below and the V5
/// pre-data identity, excluding only the circular V5 post-data identity and the
/// commitment value being derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioSettlementReceiptV5Prestate {
    pub account_id: PortfolioIdentityV2,
    /// Exact current V5 account-data identity.
    pub pre_data_id: PortfolioIdentityV2,
    pub slice_index: u16,
    pub sequence: u64,
    pub outcome: u8,
    pub quantity: u64,
    pub price: u64,
    pub accounted_end_mask: u8,
    pub delivered_end_mask: u8,
    pub expected_end_mask: u8,
    pub transition_kind: SettlementReceiptTransitionKindV2,
    pub transition_commitment: PortfolioIdentityV2,
    pub rent_owner_id: PortfolioIdentityV2,
    pub rent_principal_lamports: u64,
    pub rent_donation_floor_lamports: u64,
}

impl PortfolioSettlementReceiptV5Prestate {
    /// Canonical inactive sibling padding.
    pub const EMPTY: Self = Self {
        account_id: [0; 32],
        pre_data_id: [0; 32],
        slice_index: 0,
        sequence: 0,
        outcome: 0,
        quantity: 0,
        price: 0,
        accounted_end_mask: 0,
        delivered_end_mask: 0,
        expected_end_mask: 0,
        transition_kind: SettlementReceiptTransitionKindV2::None,
        transition_commitment: [0; 32],
        rent_owner_id: [0; 32],
        rent_principal_lamports: 0,
        rent_donation_floor_lamports: 0,
    };
}

/// Complete canonical active-prefix set of scalar Receipt V5 siblings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioSettlementReceiptV5SetPrestate {
    pub receipt_count: u8,
    pub receipts: [PortfolioSettlementReceiptV5Prestate; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
}

/// Exact generationless Receipt V5 transition the General adapter must prove.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioSettlementReceiptV5TransitionExpectationV2 {
    pub prestate: PortfolioSettlementReceiptV5SetPrestate,
    pub post_transition_kind: SettlementReceiptTransitionKindV2,
    pub transition_commitment: PortfolioIdentityV2,
}

/// Complete hostile input for the atomic account transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairExecutionInputV2 {
    pub settlement_receipts: PortfolioSettlementReceiptV5SetPrestate,
    pub buyer_reservation: PortfolioReservationPrestateV2,
    pub seller_reservation: PortfolioReservationPrestateV2,
    pub buyer_position: PortfolioPositionPrestateV2,
    pub seller_position: PortfolioPositionPrestateV2,
    pub buyer_replay: PortfolioReplayPrestateV2,
    pub seller_replay: PortfolioReplayPrestateV2,
    pub post_semantic_ids: PortfolioPairPostSemanticIdsV2,
}

/// Canonical Position value poststate. Position generation is an incarnation
/// identity and therefore remains unchanged by ordinary settlement mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPositionPoststateV2 {
    generation: u64,
    cash_atoms: u64,
    reserved_cash_atoms: u64,
    native_eggs: [u64; MAX_OUTCOMES],
    outstanding_reservations: u64,
}

impl PortfolioPositionPoststateV2 {
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn cash_atoms(&self) -> u64 {
        self.cash_atoms
    }

    pub const fn reserved_cash_atoms(&self) -> u64 {
        self.reserved_cash_atoms
    }

    pub const fn native_eggs(&self) -> &[u64; MAX_OUTCOMES] {
        &self.native_eggs
    }

    pub const fn outstanding_reservations(&self) -> u64 {
        self.outstanding_reservations
    }
}

/// Exact account effects consumed as one indivisible adapter plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairEffectsV2 {
    buyer_cash_debit_atoms: u64,
    buyer_cash_refund_atoms: u64,
    seller_cash_credit_atoms: u64,
    claim_debits: [u64; MAX_OUTCOMES],
    claim_credits: [u64; MAX_OUTCOMES],
}

impl PortfolioPairEffectsV2 {
    pub const fn buyer_cash_debit_atoms(&self) -> u64 {
        self.buyer_cash_debit_atoms
    }

    pub const fn buyer_cash_refund_atoms(&self) -> u64 {
        self.buyer_cash_refund_atoms
    }

    pub const fn seller_cash_credit_atoms(&self) -> u64 {
        self.seller_cash_credit_atoms
    }

    pub const fn claim_debits(&self) -> &[u64; MAX_OUTCOMES] {
        &self.claim_debits
    }

    pub const fn claim_credits(&self) -> &[u64; MAX_OUTCOMES] {
        &self.claim_credits
    }
}

/// Canonical replay-sensitive receipt preimage.
///
/// This 680-byte value is not another account or counted liability. Its exact
/// V5-domain transition commitment is retained by the authenticated
/// SettlementReceipt V5 postimage and by the returned private capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairReceiptV2 {
    version: u8,
    outcome_count: u8,
    boundary: PortfolioValuationBoundaryV2,
    receipt_count: u8,
    slice_index: u16,
    sequence: u64,
    pair_units: u64,
    consideration_atoms: u64,
    unit_value_price_units: u128,
    total_value_price_units: u128,
    payoff: [u64; MAX_OUTCOMES],
    market_semantics_digest: PortfolioIdentityV2,
    epoch_semantics_digest: PortfolioIdentityV2,
    economic_candidate_digest: PortfolioIdentityV2,
    settlement_root_account_id: PortfolioIdentityV2,
    settlement_candidate_id: PortfolioIdentityV2,
    settlement_witness_id: PortfolioIdentityV2,
    price_semantics_digest: PortfolioIdentityV2,
    buy_order_id: PortfolioIdentityV2,
    sell_order_id: PortfolioIdentityV2,
    buyer_owner_id: PortfolioIdentityV2,
    seller_owner_id: PortfolioIdentityV2,
    entry_settlement_receipt_account_id: PortfolioIdentityV2,
    settlement_receipt_set_digest: PortfolioIdentityV2,
    retained_feed_semantic_id: PortfolioIdentityV2,
    transition_id: PortfolioIdentityV2,
}

impl PortfolioPairReceiptV2 {
    pub const fn slice_index(&self) -> u16 {
        self.slice_index
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn receipt_count(&self) -> u8 {
        self.receipt_count
    }

    pub const fn settlement_receipt_set_digest(&self) -> PortfolioIdentityV2 {
        self.settlement_receipt_set_digest
    }

    pub const fn consideration_atoms(&self) -> u64 {
        self.consideration_atoms
    }

    pub const fn payoff(&self) -> &[u64; MAX_OUTCOMES] {
        &self.payoff
    }

    pub const fn transition_id(&self) -> PortfolioIdentityV2 {
        self.transition_id
    }

    /// Exact receipt account body. Bytes `14..16` are canonical zero padding.
    pub fn encode_into(
        &self,
        output: &mut [u8; PORTFOLIO_PAIR_RECEIPT_V2_BYTES],
    ) -> Result<(), PortfolioExecutionErrorV2> {
        self.validate()?;
        *output = [0; PORTFOLIO_PAIR_RECEIPT_V2_BYTES];
        output[0..8].copy_from_slice(&PAIR_RECEIPT_MAGIC_V2);
        output[8] = self.version;
        output[9] = self.outcome_count;
        output[10] = valuation_boundary_byte(self.boundary);
        output[11] = self.receipt_count;
        output[12..14].copy_from_slice(&self.slice_index.to_le_bytes());
        output[16..24].copy_from_slice(&self.sequence.to_le_bytes());
        output[24..32].copy_from_slice(&self.pair_units.to_le_bytes());
        output[32..40].copy_from_slice(&self.consideration_atoms.to_le_bytes());
        output[40..56].copy_from_slice(&self.unit_value_price_units.to_le_bytes());
        output[56..72].copy_from_slice(&self.total_value_price_units.to_le_bytes());
        let mut cursor = 72usize;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            output[cursor..cursor + 8].copy_from_slice(&self.payoff[outcome].to_le_bytes());
            cursor += 8;
            outcome += 1;
        }
        let identities = self.identities();
        let mut index = 0usize;
        while index < identities.len() {
            output[cursor..cursor + 32].copy_from_slice(identities[index]);
            cursor += 32;
            index += 1;
        }
        if cursor != PORTFOLIO_PAIR_RECEIPT_V2_BYTES {
            return Err(PortfolioExecutionErrorV2::InvalidCodec);
        }
        Ok(())
    }

    /// Decode hostile receipt bytes as an untrusted projection.
    pub fn decode(input: &[u8]) -> Result<Self, PortfolioExecutionErrorV2> {
        if input.len() != PORTFOLIO_PAIR_RECEIPT_V2_BYTES
            || input[0..8] != PAIR_RECEIPT_MAGIC_V2
        {
            return Err(PortfolioExecutionErrorV2::InvalidCodec);
        }
        if input[14..16].iter().any(|byte| *byte != 0) {
            return Err(PortfolioExecutionErrorV2::NonCanonicalPadding);
        }
        let boundary = match input[10] {
            1 => PortfolioValuationBoundaryV2::ExactReceiptDivisionV1,
            _ => return Err(PortfolioExecutionErrorV2::UnsupportedRoundingBoundary),
        };
        let mut payoff = [0u64; MAX_OUTCOMES];
        let mut cursor = 72usize;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            payoff[outcome] = read_u64(input, cursor)?;
            cursor += 8;
            outcome += 1;
        }
        let mut next_identity = || -> Result<PortfolioIdentityV2, PortfolioExecutionErrorV2> {
            let end = cursor
                .checked_add(32)
                .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
            let bytes = input
                .get(cursor..end)
                .ok_or(PortfolioExecutionErrorV2::InvalidCodec)?;
            let mut identity = [0u8; 32];
            identity.copy_from_slice(bytes);
            cursor = end;
            Ok(identity)
        };
        let value = Self {
            version: input[8],
            outcome_count: input[9],
            boundary,
            receipt_count: input[11],
            slice_index: read_u16(input, 12)?,
            sequence: read_u64(input, 16)?,
            pair_units: read_u64(input, 24)?,
            consideration_atoms: read_u64(input, 32)?,
            unit_value_price_units: read_u128(input, 40)?,
            total_value_price_units: read_u128(input, 56)?,
            payoff,
            market_semantics_digest: next_identity()?,
            epoch_semantics_digest: next_identity()?,
            economic_candidate_digest: next_identity()?,
            settlement_root_account_id: next_identity()?,
            settlement_candidate_id: next_identity()?,
            settlement_witness_id: next_identity()?,
            price_semantics_digest: next_identity()?,
            buy_order_id: next_identity()?,
            sell_order_id: next_identity()?,
            buyer_owner_id: next_identity()?,
            seller_owner_id: next_identity()?,
            entry_settlement_receipt_account_id: next_identity()?,
            settlement_receipt_set_digest: next_identity()?,
            retained_feed_semantic_id: next_identity()?,
            transition_id: next_identity()?,
        };
        if cursor != PORTFOLIO_PAIR_RECEIPT_V2_BYTES {
            return Err(PortfolioExecutionErrorV2::InvalidCodec);
        }
        value.validate()?;
        Ok(value)
    }

    fn identities(&self) -> [&PortfolioIdentityV2; 15] {
        [
            &self.market_semantics_digest,
            &self.epoch_semantics_digest,
            &self.economic_candidate_digest,
            &self.settlement_root_account_id,
            &self.settlement_candidate_id,
            &self.settlement_witness_id,
            &self.price_semantics_digest,
            &self.buy_order_id,
            &self.sell_order_id,
            &self.buyer_owner_id,
            &self.seller_owner_id,
            &self.entry_settlement_receipt_account_id,
            &self.settlement_receipt_set_digest,
            &self.retained_feed_semantic_id,
            &self.transition_id,
        ]
    }

    fn validate(&self) -> Result<(), PortfolioExecutionErrorV2> {
        if self.version != PORTFOLIO_EXECUTION_VERSION_V2 {
            return Err(PortfolioExecutionErrorV2::UnknownVersion);
        }
        if !(2..=MAX_OUTCOMES).contains(&usize::from(self.outcome_count))
            || self.sequence != u64::from(self.slice_index) + 1
            || self.receipt_count == 0
            || usize::from(self.receipt_count) > usize::from(self.outcome_count)
            || self.pair_units == 0
        {
            return Err(PortfolioExecutionErrorV2::InvalidGenerationOrUnits);
        }
        let mut index = 0usize;
        let identities = self.identities();
        while index < identities.len() {
            if is_zero_identity(identities[index]) {
                return Err(PortfolioExecutionErrorV2::ZeroIdentity);
            }
            index += 1;
        }
        let mut nonzero_receipts = 0usize;
        let mut outcome = 0usize;
        while outcome < usize::from(self.outcome_count) {
            if self.payoff[outcome] != 0 {
                nonzero_receipts = nonzero_receipts
                    .checked_add(1)
                    .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
            }
            outcome += 1;
        }
        if nonzero_receipts != usize::from(self.receipt_count) {
            return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
        }
        outcome = usize::from(self.outcome_count);
        while outcome < MAX_OUTCOMES {
            if self.payoff[outcome] != 0 {
                return Err(PortfolioExecutionErrorV2::NonCanonicalClaimPadding);
            }
            outcome += 1;
        }
        Ok(())
    }
}

/// Private, indivisible execution capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedPortfolioPairExecutionV2 {
    receipt: PortfolioPairReceiptV2,
    transition_commitment: PortfolioIdentityV2,
    effects: PortfolioPairEffectsV2,
    buyer_position_after: PortfolioPositionPoststateV2,
    seller_position_after: PortfolioPositionPoststateV2,
    post_semantic_ids: PortfolioPairPostSemanticIdsV2,
}

impl PreparedPortfolioPairExecutionV2 {
    pub const fn receipt(&self) -> &PortfolioPairReceiptV2 {
        &self.receipt
    }

    pub const fn transition_commitment(&self) -> PortfolioIdentityV2 {
        self.transition_commitment
    }

    pub const fn effects(&self) -> &PortfolioPairEffectsV2 {
        &self.effects
    }

    pub const fn buyer_position_after(&self) -> &PortfolioPositionPoststateV2 {
        &self.buyer_position_after
    }

    pub const fn seller_position_after(&self) -> &PortfolioPositionPoststateV2 {
        &self.seller_position_after
    }

    pub const fn post_semantic_ids(&self) -> &PortfolioPairPostSemanticIdsV2 {
        &self.post_semantic_ids
    }
}

/// Prepare the authoritative Reservation/Position/Replay/receipt transition.
pub fn prepare_portfolio_pair_execution_v2<A: PortfolioAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioIdentityV2,
    pair: AuthenticatedPortfolioPairV2,
    input: PortfolioPairExecutionInputV2,
) -> Result<PreparedPortfolioPairExecutionV2, PortfolioExecutionErrorV2> {
    prepare_portfolio_pair_execution_borrowed_v2(adapter, owner_program_id, &pair, &input)
}

/// Frame-bounded form of [`prepare_portfolio_pair_execution_v2`].
///
/// Runtime adapters keep the maximum-width pair and hostile transition input
/// in bounded heap or account-backed storage and lend them here. The semantic
/// checks and resulting capability are byte-identical to the owned host API.
pub fn prepare_portfolio_pair_execution_borrowed_v2<A: PortfolioAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioIdentityV2,
    pair: &AuthenticatedPortfolioPairV2,
    input: &PortfolioPairExecutionInputV2,
) -> Result<PreparedPortfolioPairExecutionV2, PortfolioExecutionErrorV2> {
    if is_zero_identity(&owner_program_id) {
        return Err(PortfolioExecutionErrorV2::ZeroIdentity);
    }
    validate_settlement_receipt_v5_set_prestate(&input.settlement_receipts, &pair)?;
    let entry_receipt = input.settlement_receipts.receipts[0];
    if entry_receipt.slice_index != pair.buyer.record.traversal_index {
        return Err(PortfolioExecutionErrorV2::FeedTraversalMismatch);
    }
    validate_distinct_execution_accounts(&pair, &input)?;
    validate_reservation_prestate(&pair.buyer, &input.buyer_reservation, true, &pair.payoff)?;
    validate_reservation_prestate(&pair.seller, &input.seller_reservation, false, &pair.payoff)?;
    validate_position_prestate(&pair.buyer, &input.buyer_position)?;
    validate_position_prestate(&pair.seller, &input.seller_position)?;
    validate_replay_prestate(&input.buyer_replay)?;
    validate_replay_prestate(&input.seller_replay)?;
    validate_post_ids(&input)?;
    if input.buyer_reservation.remaining_cash_atoms < pair.consideration_atoms {
        return Err(PortfolioExecutionErrorV2::BuyerReservationUnderfunded);
    }
    if input.buyer_position.cash_atoms < pair.consideration_atoms
        || input.buyer_position.reserved_cash_atoms
            < input.buyer_reservation.remaining_cash_atoms
    {
        return Err(PortfolioExecutionErrorV2::PositionUnderfunded);
    }
    let buyer_cash = input
        .buyer_position
        .cash_atoms
        .checked_sub(pair.consideration_atoms)
        .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
    let buyer_reserved = input
        .buyer_position
        .reserved_cash_atoms
        .checked_sub(input.buyer_reservation.remaining_cash_atoms)
        .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
    if buyer_reserved > buyer_cash {
        return Err(PortfolioExecutionErrorV2::PositionUnderfunded);
    }
    let mut buyer_eggs = input.buyer_position.native_eggs;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        buyer_eggs[outcome] = buyer_eggs[outcome]
            .checked_add(pair.payoff[outcome])
            .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
        outcome += 1;
    }
    let seller_cash = input
        .seller_position
        .cash_atoms
        .checked_add(pair.consideration_atoms)
        .ok_or(PortfolioExecutionErrorV2::ArithmeticOverflow)?;
    let buyer_position_after = PortfolioPositionPoststateV2 {
        generation: input.buyer_position.generation,
        cash_atoms: buyer_cash,
        reserved_cash_atoms: buyer_reserved,
        native_eggs: buyer_eggs,
        outstanding_reservations: input.buyer_position.outstanding_reservations,
    };
    let seller_position_after = PortfolioPositionPoststateV2 {
        generation: input.seller_position.generation,
        cash_atoms: seller_cash,
        reserved_cash_atoms: input.seller_position.reserved_cash_atoms,
        native_eggs: input.seller_position.native_eggs,
        outstanding_reservations: input.seller_position.outstanding_reservations,
    };
    validate_position_post_id(
        &input.buyer_position,
        &buyer_position_after,
        input.post_semantic_ids.buyer_position,
    )?;
    validate_position_post_id(
        &input.seller_position,
        &seller_position_after,
        input.post_semantic_ids.seller_position,
    )?;
    let buyer_replay_post = input
        .buyer_replay
        .ordinal
        .checked_add(1)
        .ok_or(PortfolioExecutionErrorV2::ReplayOverflow)?;
    let seller_replay_post = input
        .seller_replay
        .ordinal
        .checked_add(1)
        .ok_or(PortfolioExecutionErrorV2::ReplayOverflow)?;
    let effects = PortfolioPairEffectsV2 {
        buyer_cash_debit_atoms: pair.consideration_atoms,
        buyer_cash_refund_atoms: input.buyer_reservation.remaining_cash_atoms
            - pair.consideration_atoms,
        seller_cash_credit_atoms: pair.consideration_atoms,
        claim_debits: pair.payoff,
        claim_credits: pair.payoff,
    };

    let accounts = execution_account_expectations(&pair, &input, owner_program_id);
    let mut account_index = 0usize;
    while account_index < accounts.len() {
        if !adapter.authenticate_account(&accounts[account_index]) {
            return Err(PortfolioExecutionErrorV2::AuthenticationFailed {
                role: accounts[account_index].role,
            });
        }
        account_index += 1;
    }
    let mut receipt_index = 0usize;
    while receipt_index < usize::from(input.settlement_receipts.receipt_count) {
        let receipt = input.settlement_receipts.receipts[receipt_index];
        let expected = account_expectation(
            PortfolioAccountRoleV2::SettlementReceipt,
            receipt.account_id,
            owner_program_id,
            receipt.pre_data_id,
            None,
            true,
            true,
        );
        if !adapter.authenticate_account(&expected) {
            return Err(PortfolioExecutionErrorV2::AuthenticationFailed {
                role: PortfolioAccountRoleV2::SettlementReceipt,
            });
        }
        receipt_index += 1;
    }
    let transition_id = portfolio_transition_id_v2(
        &pair,
        &input,
        &effects,
        &buyer_position_after,
        &seller_position_after,
        buyer_replay_post,
        seller_replay_post,
    )?;
    let buyer_record = pair.buyer.record;
    let seller_record = pair.seller.record;
    let receipt = PortfolioPairReceiptV2 {
        version: PORTFOLIO_EXECUTION_VERSION_V2,
        outcome_count: buyer_record.outcome_count,
        boundary: pair.boundary,
        receipt_count: input.settlement_receipts.receipt_count,
        slice_index: entry_receipt.slice_index,
        sequence: entry_receipt.sequence,
        pair_units: pair.pair_units,
        consideration_atoms: pair.consideration_atoms,
        unit_value_price_units: pair.unit_value_price_units,
        total_value_price_units: pair.total_value_price_units,
        payoff: pair.payoff,
        market_semantics_digest: buyer_record.market_semantics_digest,
        epoch_semantics_digest: buyer_record.epoch_semantics_digest,
        economic_candidate_digest: buyer_record.economic_candidate_digest,
        settlement_root_account_id: buyer_record.settlement_root_account_id,
        settlement_candidate_id: buyer_record.settlement_candidate_id,
        settlement_witness_id: buyer_record.settlement_witness_id,
        price_semantics_digest: pair.price_semantics_digest,
        buy_order_id: buyer_record.order_id,
        sell_order_id: seller_record.order_id,
        buyer_owner_id: buyer_record.owner_id,
        seller_owner_id: seller_record.owner_id,
        entry_settlement_receipt_account_id: entry_receipt.account_id,
        settlement_receipt_set_digest: portfolio_settlement_receipt_v5_set_digest_v2(
            &input.settlement_receipts,
        )?,
        retained_feed_semantic_id: buyer_record.retained_feed_semantic_id,
        transition_id,
    };
    let transition_commitment = portfolio_pair_transition_commitment_v2(&receipt)?;
    authenticate_execution_transitions(
        adapter,
        &input,
        &effects,
        buyer_replay_post,
        seller_replay_post,
    )?;
    let receipt_transition = PortfolioSettlementReceiptV5TransitionExpectationV2 {
        prestate: input.settlement_receipts,
        post_transition_kind: SettlementReceiptTransitionKindV2::PortfolioPairV2,
        transition_commitment,
    };
    let settlement_receipt_post_data_ids = adapter
        .derive_settlement_receipt_v5_post_data_ids(&receipt_transition)
        .ok_or(PortfolioExecutionErrorV2::TransitionAuthenticationFailed {
            role: PortfolioAccountRoleV2::SettlementReceipt,
        })?;
    let mut receipt_index = 0usize;
    while receipt_index < PORTFOLIO_PAIR_MAX_RECEIPTS_V2 {
        if receipt_index < usize::from(input.settlement_receipts.receipt_count) {
            if is_zero_identity(&settlement_receipt_post_data_ids[receipt_index])
                || settlement_receipt_post_data_ids[receipt_index]
                    == input.settlement_receipts.receipts[receipt_index].pre_data_id
            {
                return Err(PortfolioExecutionErrorV2::PostSemanticMismatch);
            }
        } else if !is_zero_identity(&settlement_receipt_post_data_ids[receipt_index]) {
            return Err(PortfolioExecutionErrorV2::PostSemanticMismatch);
        }
        receipt_index += 1;
    }
    let post_semantic_ids = PortfolioPairPostSemanticIdsV2 {
        settlement_receipts: settlement_receipt_post_data_ids,
        ..input.post_semantic_ids
    };
    Ok(PreparedPortfolioPairExecutionV2 {
        receipt,
        transition_commitment,
        effects,
        buyer_position_after,
        seller_position_after,
        post_semantic_ids,
    })
}

/// Derive the exact General-owned Receipt V5 transition commitment from the
/// canonical 680-byte preimage. Decoded bytes alone do not provide the private
/// execution capability.
pub fn portfolio_pair_transition_commitment_v2(
    receipt: &PortfolioPairReceiptV2,
) -> Result<PortfolioIdentityV2, PortfolioExecutionErrorV2> {
    let mut bytes = [0u8; PORTFOLIO_PAIR_RECEIPT_V2_BYTES];
    receipt.encode_into(&mut bytes)?;
    let mut hash = Sha256V2::new();
    hash.update(PORTFOLIO_PAIR_TRANSITION_COMMITMENT_DOMAIN_V2)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&bytes)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.finalize().map_err(PortfolioExecutionErrorV2::Economic)
}

/// Every deterministic refusal in this authority slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortfolioExecutionErrorV2 {
    Economic(EconomicErrorV2),
    UnknownVersion,
    UnknownSourceOrderKind,
    InvalidCodec,
    NonCanonicalPadding,
    InvalidOutcomeCount,
    InvalidGenerationOrUnits,
    ZeroIdentity,
    AliasedAccount,
    DomainMismatch,
    CandidateMismatch,
    OrderMismatch,
    AuthenticationFailed { role: PortfolioAccountRoleV2 },
    SelectionMembershipAuthenticationFailed,
    TransitionAuthenticationFailed { role: PortfolioAccountRoleV2 },
    UnsupportedRoundingBoundary,
    VirtualConversionNotPairable,
    SideMismatch,
    SelectionMismatch,
    AliasedPairEndpoint,
    CoefficientMismatch,
    NotExactFullPair,
    NonExclusivePair,
    NonCanonicalClaimPadding,
    InexactValuation,
    ConsiderationOverflow,
    ReservationMismatch,
    ReservationFeeUnsupported,
    BuyerReservationUnderfunded,
    PositionMismatch,
    PositionUnderfunded,
    ReplayMismatch,
    ReplayOverflow,
    SettlementReceiptMismatch,
    SettlementReceiptSetMismatch,
    FeedTraversalMismatch,
    PostSemanticMismatch,
    ArithmeticOverflow,
}

impl From<EconomicErrorV2> for PortfolioExecutionErrorV2 {
    fn from(value: EconomicErrorV2) -> Self {
        Self::Economic(value)
    }
}

fn shared_selection(
    left: &SelectedPortfolioOrderRecordV2,
    right: &SelectedPortfolioOrderRecordV2,
) -> bool {
    left.market_semantics_digest == right.market_semantics_digest
        && left.epoch_semantics_digest == right.epoch_semantics_digest
        && left.economic_candidate_digest == right.economic_candidate_digest
        && left.order_set_digest == right.order_set_digest
        && left.settlement_root_account_id == right.settlement_root_account_id
        && left.settlement_root_pre_semantic_id == right.settlement_root_pre_semantic_id
        && left.settlement_candidate_id == right.settlement_candidate_id
        && left.retained_feed_account_id == right.retained_feed_account_id
        && left.retained_feed_semantic_id == right.retained_feed_semantic_id
        && left.settlement_witness_id == right.settlement_witness_id
        && left.settlement_root_epoch_generation == right.settlement_root_epoch_generation
        && left.traversal_index == right.traversal_index
        && left.outcome_count == right.outcome_count
}

fn validate_reservation_prestate(
    selected: &AuthenticatedSelectedPortfolioOrderV2,
    reservation: &PortfolioReservationPrestateV2,
    buyer: bool,
    payoff: &[u64; MAX_OUTCOMES],
) -> Result<(), PortfolioExecutionErrorV2> {
    let record = selected.record;
    if is_zero_identity(&reservation.account_id)
        || is_zero_identity(&reservation.semantic_id)
        || reservation.generation == 0
        || reservation.lifecycle != PortfolioReservationLifecycleV2::Entitled
        || reservation.owner_id != record.owner_id
        || reservation.order_id != record.order_id
        || reservation.position_account_id != record.position_account_id
        || reservation.position_generation != record.position_generation
        || reservation.entitled_units != selected.record.selected_fill_units
        || reservation.consumed_units != 0
        || reservation.paid_units != 0
    {
        return Err(PortfolioExecutionErrorV2::ReservationMismatch);
    }
    if reservation.maximum_fee_atoms != 0 {
        return Err(PortfolioExecutionErrorV2::ReservationFeeUnsupported);
    }
    if buyer {
        if reservation
            .remaining_claim_atoms
            .iter()
            .any(|amount| *amount != 0)
        {
            return Err(PortfolioExecutionErrorV2::ReservationMismatch);
        }
    } else if reservation.remaining_cash_atoms != 0
        || reservation.remaining_claim_atoms != *payoff
    {
        return Err(PortfolioExecutionErrorV2::ReservationMismatch);
    }
    Ok(())
}

fn validate_position_prestate(
    selected: &AuthenticatedSelectedPortfolioOrderV2,
    position: &PortfolioPositionPrestateV2,
) -> Result<(), PortfolioExecutionErrorV2> {
    let record = selected.record;
    if is_zero_identity(&position.account_id)
        || is_zero_identity(&position.semantic_id)
        || position.account_id != record.position_account_id
        || position.semantic_id != record.position_pre_semantic_id
        || position.owner_id != record.owner_id
        || position.generation != record.position_generation
        || position.generation == 0
        || position.reserved_cash_atoms > position.cash_atoms
        || position.outstanding_reservations == 0
    {
        return Err(PortfolioExecutionErrorV2::PositionMismatch);
    }
    let mut outcome = usize::from(record.outcome_count);
    while outcome < MAX_OUTCOMES {
        if position.native_eggs[outcome] != 0 {
            return Err(PortfolioExecutionErrorV2::NonCanonicalClaimPadding);
        }
        outcome += 1;
    }
    Ok(())
}

fn validate_replay_prestate(
    replay: &PortfolioReplayPrestateV2,
) -> Result<(), PortfolioExecutionErrorV2> {
    if is_zero_identity(&replay.account_id) || is_zero_identity(&replay.semantic_id) {
        return Err(PortfolioExecutionErrorV2::ReplayMismatch);
    }
    Ok(())
}

fn validate_settlement_receipt_v5_prestate(
    receipt: &PortfolioSettlementReceiptV5Prestate,
) -> Result<(), PortfolioExecutionErrorV2> {
    if is_zero_identity(&receipt.account_id)
        || is_zero_identity(&receipt.pre_data_id)
        || is_zero_identity(&receipt.rent_owner_id)
        || receipt.rent_principal_lamports == 0
        || receipt.quantity == 0
        || usize::from(receipt.outcome) >= MAX_OUTCOMES
        || receipt.sequence != u64::from(receipt.slice_index) + 1
        || receipt.accounted_end_mask != receipt.expected_end_mask
        || receipt.delivered_end_mask != 0
        || receipt.expected_end_mask != SETTLEMENT_RECEIPT_DIRECT_END_MASK_V5
        || receipt.transition_kind != SettlementReceiptTransitionKindV2::PortfolioPairV2
        || !is_zero_identity(&receipt.transition_commitment)
    {
        return Err(PortfolioExecutionErrorV2::SettlementReceiptMismatch);
    }
    Ok(())
}

fn validate_settlement_receipt_v5_set_prestate(
    receipt_set: &PortfolioSettlementReceiptV5SetPrestate,
    pair: &AuthenticatedPortfolioPairV2,
) -> Result<(), PortfolioExecutionErrorV2> {
    let count = usize::from(receipt_set.receipt_count);
    if count == 0
        || count > PORTFOLIO_PAIR_MAX_RECEIPTS_V2
        || count > usize::from(pair.buyer.record.outcome_count)
    {
        return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
    }
    let mut receipt_index = 0usize;
    let mut expected_outcome = 0usize;
    while expected_outcome < usize::from(pair.buyer.record.outcome_count) {
        if pair.payoff[expected_outcome] != 0 {
            if receipt_index >= count {
                return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
            }
            let receipt = receipt_set.receipts[receipt_index];
            validate_settlement_receipt_v5_prestate(&receipt)?;
            if usize::from(receipt.outcome) != expected_outcome
                || receipt.quantity != pair.payoff[expected_outcome]
                || receipt.price != pair.prices[expected_outcome]
                || (receipt_index != 0
                    && receipt.slice_index
                        <= receipt_set.receipts[receipt_index - 1].slice_index)
            {
                return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
            }
            let mut earlier = 0usize;
            while earlier < receipt_index {
                if receipt_set.receipts[earlier].account_id == receipt.account_id
                    || receipt_set.receipts[earlier].pre_data_id == receipt.pre_data_id
                {
                    return Err(PortfolioExecutionErrorV2::AliasedAccount);
                }
                earlier += 1;
            }
            receipt_index += 1;
        }
        expected_outcome += 1;
    }
    if receipt_index != count {
        return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
    }
    while receipt_index < PORTFOLIO_PAIR_MAX_RECEIPTS_V2 {
        if receipt_set.receipts[receipt_index] != PortfolioSettlementReceiptV5Prestate::EMPTY {
            return Err(PortfolioExecutionErrorV2::NonCanonicalPadding);
        }
        receipt_index += 1;
    }
    Ok(())
}

/// Commit the complete ordered hostile Receipt V5 prestate set. The active
/// prefix is already validated against the exact pair payoff and simplex;
/// inactive cells are canonical zero and therefore omitted from the hash.
pub fn portfolio_settlement_receipt_v5_set_digest_v2(
    receipt_set: &PortfolioSettlementReceiptV5SetPrestate,
) -> Result<PortfolioIdentityV2, PortfolioExecutionErrorV2> {
    let count = usize::from(receipt_set.receipt_count);
    if count == 0 || count > PORTFOLIO_PAIR_MAX_RECEIPTS_V2 {
        return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
    }
    let mut hash = Sha256V2::new();
    hash.update(PORTFOLIO_PAIR_RECEIPT_SET_DOMAIN_V2)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&[PORTFOLIO_EXECUTION_VERSION_V2, receipt_set.receipt_count])
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    let mut index = 0usize;
    while index < count {
        let receipt = receipt_set.receipts[index];
        validate_settlement_receipt_v5_prestate(&receipt)?;
        if index != 0 {
            let prior = receipt_set.receipts[index - 1];
            if receipt.slice_index <= prior.slice_index || receipt.outcome <= prior.outcome {
                return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
            }
        }
        let mut earlier = 0usize;
        while earlier < index {
            if receipt_set.receipts[earlier].account_id == receipt.account_id
                || receipt_set.receipts[earlier].pre_data_id == receipt.pre_data_id
                || receipt_set.receipts[earlier].outcome == receipt.outcome
            {
                return Err(PortfolioExecutionErrorV2::SettlementReceiptSetMismatch);
            }
            earlier += 1;
        }
        hash.update(&receipt.account_id)
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.pre_data_id)
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.slice_index.to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.sequence.to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&[receipt.outcome])
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.quantity.to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.price.to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&[
            receipt.accounted_end_mask,
            receipt.delivered_end_mask,
            receipt.expected_end_mask,
            receipt_transition_kind_byte(receipt.transition_kind),
        ])
        .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.transition_commitment)
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.rent_owner_id)
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.rent_principal_lamports.to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&receipt.rent_donation_floor_lamports.to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        index += 1;
    }
    while index < PORTFOLIO_PAIR_MAX_RECEIPTS_V2 {
        if receipt_set.receipts[index] != PortfolioSettlementReceiptV5Prestate::EMPTY {
            return Err(PortfolioExecutionErrorV2::NonCanonicalPadding);
        }
        index += 1;
    }
    hash.finalize().map_err(PortfolioExecutionErrorV2::Economic)
}

fn validate_post_ids(
    input: &PortfolioPairExecutionInputV2,
) -> Result<(), PortfolioExecutionErrorV2> {
    let post = input.post_semantic_ids;
    let ids = [
        post.buyer_reservation,
        post.seller_reservation,
        post.buyer_position,
        post.seller_position,
        post.buyer_replay,
        post.seller_replay,
    ];
    if ids.iter().any(is_zero_identity)
        || post.buyer_reservation == input.buyer_reservation.semantic_id
        || post.seller_reservation == input.seller_reservation.semantic_id
        || post.buyer_replay == input.buyer_replay.semantic_id
        || post.seller_replay == input.seller_replay.semantic_id
        || post
            .settlement_receipts
            .iter()
            .any(|identity| !is_zero_identity(identity))
    {
        return Err(PortfolioExecutionErrorV2::PostSemanticMismatch);
    }
    Ok(())
}

fn validate_position_post_id(
    pre: &PortfolioPositionPrestateV2,
    post: &PortfolioPositionPoststateV2,
    post_semantic_id: PortfolioIdentityV2,
) -> Result<(), PortfolioExecutionErrorV2> {
    let value_changed = pre.generation != post.generation
        || pre.cash_atoms != post.cash_atoms
        || pre.reserved_cash_atoms != post.reserved_cash_atoms
        || pre.native_eggs != post.native_eggs
        || pre.outstanding_reservations != post.outstanding_reservations;
    if (value_changed && post_semantic_id == pre.semantic_id)
        || (!value_changed && post_semantic_id != pre.semantic_id)
    {
        return Err(PortfolioExecutionErrorV2::PostSemanticMismatch);
    }
    Ok(())
}

fn validate_distinct_execution_accounts(
    pair: &AuthenticatedPortfolioPairV2,
    input: &PortfolioPairExecutionInputV2,
) -> Result<(), PortfolioExecutionErrorV2> {
    let accounts = [
        pair.buyer.record.settlement_root_account_id,
        pair.buyer.record.retained_feed_account_id,
        pair.buyer.record.order_page_account_id,
        pair.seller.record.order_page_account_id,
        input.buyer_position.account_id,
        input.seller_position.account_id,
        input.buyer_reservation.account_id,
        input.seller_reservation.account_id,
        input.buyer_replay.account_id,
        input.seller_replay.account_id,
    ];
    let mut left = 0usize;
    while left < accounts.len() {
        if is_zero_identity(&accounts[left]) {
            return Err(PortfolioExecutionErrorV2::ZeroIdentity);
        }
        let mut right = left + 1;
        while right < accounts.len() {
            // Both selected orders may legitimately occupy the same frozen
            // OrderPage account. No other account alias is admissible.
            let shared_order_page = left == 2 && right == 3;
            if accounts[left] == accounts[right] && !shared_order_page {
                return Err(PortfolioExecutionErrorV2::AliasedAccount);
            }
            right += 1;
        }
        left += 1;
    }
    let mut receipt_index = 0usize;
    while receipt_index < usize::from(input.settlement_receipts.receipt_count) {
        let receipt_account = input.settlement_receipts.receipts[receipt_index].account_id;
        if is_zero_identity(&receipt_account) {
            return Err(PortfolioExecutionErrorV2::ZeroIdentity);
        }
        let mut account_index = 0usize;
        while account_index < accounts.len() {
            if receipt_account == accounts[account_index] {
                return Err(PortfolioExecutionErrorV2::AliasedAccount);
            }
            account_index += 1;
        }
        let mut earlier = 0usize;
        while earlier < receipt_index {
            if receipt_account == input.settlement_receipts.receipts[earlier].account_id {
                return Err(PortfolioExecutionErrorV2::AliasedAccount);
            }
            earlier += 1;
        }
        receipt_index += 1;
    }
    Ok(())
}

fn execution_account_expectations(
    pair: &AuthenticatedPortfolioPairV2,
    input: &PortfolioPairExecutionInputV2,
    owner_program_id: PortfolioIdentityV2,
) -> [PortfolioAccountExpectationV2; 7] {
    [
        account_expectation(
            PortfolioAccountRoleV2::SettlementRoot,
            pair.buyer.record.settlement_root_account_id,
            owner_program_id,
            pair.buyer.record.settlement_root_pre_semantic_id,
            Some(pair.buyer.record.settlement_root_epoch_generation),
            false,
            true,
        ),
        account_expectation(
            PortfolioAccountRoleV2::Reservation,
            input.buyer_reservation.account_id,
            owner_program_id,
            input.buyer_reservation.semantic_id,
            Some(input.buyer_reservation.generation),
            true,
            true,
        ),
        account_expectation(
            PortfolioAccountRoleV2::Reservation,
            input.seller_reservation.account_id,
            owner_program_id,
            input.seller_reservation.semantic_id,
            Some(input.seller_reservation.generation),
            true,
            true,
        ),
        account_expectation(
            PortfolioAccountRoleV2::Position,
            input.buyer_position.account_id,
            owner_program_id,
            input.buyer_position.semantic_id,
            Some(input.buyer_position.generation),
            true,
            true,
        ),
        account_expectation(
            PortfolioAccountRoleV2::Position,
            input.seller_position.account_id,
            owner_program_id,
            input.seller_position.semantic_id,
            Some(input.seller_position.generation),
            true,
            true,
        ),
        account_expectation(
            PortfolioAccountRoleV2::Replay,
            input.buyer_replay.account_id,
            owner_program_id,
            input.buyer_replay.semantic_id,
            None,
            true,
            true,
        ),
        account_expectation(
            PortfolioAccountRoleV2::Replay,
            input.seller_replay.account_id,
            owner_program_id,
            input.seller_replay.semantic_id,
            None,
            true,
            true,
        ),
    ]
}

fn account_expectation(
    role: PortfolioAccountRoleV2,
    account_id: PortfolioIdentityV2,
    owner_program_id: PortfolioIdentityV2,
    data_semantic_id: PortfolioIdentityV2,
    generation: Option<u64>,
    writable: bool,
    must_exist: bool,
) -> PortfolioAccountExpectationV2 {
    PortfolioAccountExpectationV2 {
        role,
        account_id,
        owner_program_id,
        data_semantic_id,
        generation,
        writable,
        must_exist,
    }
}

fn authenticate_execution_transition<A: PortfolioAdapterV2>(
    adapter: &A,
    expected: PortfolioTransitionExpectationV2,
) -> Result<(), PortfolioExecutionErrorV2> {
    if !adapter.authenticate_transition(&expected) {
        return Err(PortfolioExecutionErrorV2::TransitionAuthenticationFailed {
            role: expected.role,
        });
    }
    Ok(())
}

fn authenticate_execution_transitions<A: PortfolioAdapterV2>(
    adapter: &A,
    input: &PortfolioPairExecutionInputV2,
    effects: &PortfolioPairEffectsV2,
    buyer_replay_post: u64,
    seller_replay_post: u64,
) -> Result<(), PortfolioExecutionErrorV2> {
    authenticate_execution_transition(
        adapter,
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Reservation,
            account_id: input.buyer_reservation.account_id,
            pre_semantic_id: input.buyer_reservation.semantic_id,
            post_semantic_id: input.post_semantic_ids.buyer_reservation,
            stable_generation: Some(input.buyer_reservation.generation),
            pre_replay_ordinal: 0,
            post_replay_ordinal: 0,
            cash_debit_atoms: input.buyer_reservation.remaining_cash_atoms,
            cash_credit_atoms: 0,
            reserved_cash_release_atoms: 0,
            claim_debits: [0; MAX_OUTCOMES],
            claim_credits: [0; MAX_OUTCOMES],
            reservation_consumed: true,
        },
    )?;
    authenticate_execution_transition(
        adapter,
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Reservation,
            account_id: input.seller_reservation.account_id,
            pre_semantic_id: input.seller_reservation.semantic_id,
            post_semantic_id: input.post_semantic_ids.seller_reservation,
            stable_generation: Some(input.seller_reservation.generation),
            pre_replay_ordinal: 0,
            post_replay_ordinal: 0,
            cash_debit_atoms: 0,
            cash_credit_atoms: 0,
            reserved_cash_release_atoms: 0,
            claim_debits: effects.claim_debits,
            claim_credits: [0; MAX_OUTCOMES],
            reservation_consumed: true,
        },
    )?;
    authenticate_execution_transition(
        adapter,
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Position,
            account_id: input.buyer_position.account_id,
            pre_semantic_id: input.buyer_position.semantic_id,
            post_semantic_id: input.post_semantic_ids.buyer_position,
            stable_generation: Some(input.buyer_position.generation),
            pre_replay_ordinal: 0,
            post_replay_ordinal: 0,
            cash_debit_atoms: effects.buyer_cash_debit_atoms,
            cash_credit_atoms: 0,
            reserved_cash_release_atoms: input.buyer_reservation.remaining_cash_atoms,
            claim_debits: [0; MAX_OUTCOMES],
            claim_credits: effects.claim_credits,
            reservation_consumed: false,
        },
    )?;
    authenticate_execution_transition(
        adapter,
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Position,
            account_id: input.seller_position.account_id,
            pre_semantic_id: input.seller_position.semantic_id,
            post_semantic_id: input.post_semantic_ids.seller_position,
            stable_generation: Some(input.seller_position.generation),
            pre_replay_ordinal: 0,
            post_replay_ordinal: 0,
            cash_debit_atoms: 0,
            cash_credit_atoms: effects.seller_cash_credit_atoms,
            reserved_cash_release_atoms: 0,
            claim_debits: [0; MAX_OUTCOMES],
            claim_credits: [0; MAX_OUTCOMES],
            reservation_consumed: false,
        },
    )?;
    authenticate_execution_transition(
        adapter,
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Replay,
            account_id: input.buyer_replay.account_id,
            pre_semantic_id: input.buyer_replay.semantic_id,
            post_semantic_id: input.post_semantic_ids.buyer_replay,
            stable_generation: None,
            pre_replay_ordinal: input.buyer_replay.ordinal,
            post_replay_ordinal: buyer_replay_post,
            cash_debit_atoms: 0,
            cash_credit_atoms: 0,
            reserved_cash_release_atoms: 0,
            claim_debits: [0; MAX_OUTCOMES],
            claim_credits: [0; MAX_OUTCOMES],
            reservation_consumed: false,
        },
    )?;
    authenticate_execution_transition(
        adapter,
        PortfolioTransitionExpectationV2 {
            role: PortfolioAccountRoleV2::Replay,
            account_id: input.seller_replay.account_id,
            pre_semantic_id: input.seller_replay.semantic_id,
            post_semantic_id: input.post_semantic_ids.seller_replay,
            stable_generation: None,
            pre_replay_ordinal: input.seller_replay.ordinal,
            post_replay_ordinal: seller_replay_post,
            cash_debit_atoms: 0,
            cash_credit_atoms: 0,
            reserved_cash_release_atoms: 0,
            claim_debits: [0; MAX_OUTCOMES],
            claim_credits: [0; MAX_OUTCOMES],
            reservation_consumed: false,
        },
    )?;
    Ok(())
}

fn portfolio_transition_id_v2(
    pair: &AuthenticatedPortfolioPairV2,
    input: &PortfolioPairExecutionInputV2,
    effects: &PortfolioPairEffectsV2,
    buyer_position_after: &PortfolioPositionPoststateV2,
    seller_position_after: &PortfolioPositionPoststateV2,
    buyer_replay_post: u64,
    seller_replay_post: u64,
) -> Result<PortfolioIdentityV2, PortfolioExecutionErrorV2> {
    let mut hash = Sha256V2::new();
    hash.update(PAIR_EFFECTS_TRANSITION_DOMAIN_V2)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&[PORTFOLIO_EXECUTION_VERSION_V2])
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    let records = [pair.buyer.record, pair.seller.record];
    let mut record_index = 0usize;
    while record_index < records.len() {
        let mut bytes = [0u8; SELECTED_PORTFOLIO_ORDER_V2_BYTES];
        records[record_index].encode_into(&mut bytes)?;
        hash.update(&bytes)
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        record_index += 1;
    }
    let entry_receipt = input.settlement_receipts.receipts[0];
    let receipt_set_digest =
        portfolio_settlement_receipt_v5_set_digest_v2(&input.settlement_receipts)?;
    hash.update(&[input.settlement_receipts.receipt_count])
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&entry_receipt.slice_index.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&entry_receipt.sequence.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&entry_receipt.account_id)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&receipt_set_digest)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    // Every active sibling has the same canonical direct pre/post lifecycle;
    // the complete prestate of each is already transitively bound by the set
    // digest. These named post fields apply exhaustively to the active prefix.
    let receipt_lifecycle_transcript = [
        SETTLEMENT_RECEIPT_DIRECT_END_MASK_V5,
        SETTLEMENT_RECEIPT_DIRECT_END_MASK_V5,
        SETTLEMENT_RECEIPT_DIRECT_END_MASK_V5,
        receipt_transition_kind_byte(SettlementReceiptTransitionKindV2::PortfolioPairV2),
    ];
    hash.update(&receipt_lifecycle_transcript)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&pair.price_semantics_digest)
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&[valuation_boundary_byte(pair.boundary)])
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&pair.pair_units.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&pair.unit_value_price_units.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&pair.total_value_price_units.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&pair.consideration_atoms.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash_transition_side(
        &mut hash,
        &input.buyer_reservation,
        &input.buyer_position,
        &input.buyer_replay,
        input.post_semantic_ids.buyer_reservation,
        input.post_semantic_ids.buyer_position,
        input.post_semantic_ids.buyer_replay,
        buyer_replay_post,
        buyer_position_after,
    )?;
    hash_transition_side(
        &mut hash,
        &input.seller_reservation,
        &input.seller_position,
        &input.seller_replay,
        input.post_semantic_ids.seller_reservation,
        input.post_semantic_ids.seller_position,
        input.post_semantic_ids.seller_replay,
        seller_replay_post,
        seller_position_after,
    )?;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        hash.update(&effects.claim_debits[outcome].to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        hash.update(&effects.claim_credits[outcome].to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        outcome += 1;
    }
    hash.update(&effects.buyer_cash_debit_atoms.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&effects.buyer_cash_refund_atoms.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&effects.seller_cash_credit_atoms.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.finalize().map_err(PortfolioExecutionErrorV2::Economic)
}

#[allow(clippy::too_many_arguments)]
fn hash_transition_side(
    hash: &mut Sha256V2,
    reservation: &PortfolioReservationPrestateV2,
    position: &PortfolioPositionPrestateV2,
    replay: &PortfolioReplayPrestateV2,
    reservation_post: PortfolioIdentityV2,
    position_post: PortfolioIdentityV2,
    replay_post: PortfolioIdentityV2,
    replay_post_ordinal: u64,
    position_after: &PortfolioPositionPoststateV2,
) -> Result<(), PortfolioExecutionErrorV2> {
    let ids = [
        reservation.account_id,
        reservation.semantic_id,
        reservation_post,
        position.account_id,
        position.semantic_id,
        position_post,
        replay.account_id,
        replay.semantic_id,
        replay_post,
    ];
    let mut index = 0usize;
    while index < ids.len() {
        hash.update(&ids[index])
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        index += 1;
    }
    hash.update(&reservation.generation.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&reservation.entitled_units.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&reservation.consumed_units.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&reservation.paid_units.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&position.generation.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&replay.ordinal.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&replay_post_ordinal.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&position_after.cash_atoms.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&position_after.reserved_cash_atoms.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    hash.update(&position_after.outstanding_reservations.to_le_bytes())
        .map_err(PortfolioExecutionErrorV2::Economic)?;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        hash.update(&position_after.native_eggs[outcome].to_le_bytes())
            .map_err(PortfolioExecutionErrorV2::Economic)?;
        outcome += 1;
    }
    Ok(())
}

const fn side_byte(side: Side) -> u8 {
    match side {
        Side::Buy => 0,
        Side::Sell => 1,
    }
}

const fn source_order_kind_byte(kind: PortfolioSourceOrderKindV2) -> u8 {
    match kind {
        PortfolioSourceOrderKindV2::Portfolio => 2,
    }
}

const fn valuation_boundary_byte(boundary: PortfolioValuationBoundaryV2) -> u8 {
    match boundary {
        PortfolioValuationBoundaryV2::ExactReceiptDivisionV1 => 1,
    }
}

const fn receipt_transition_kind_byte(kind: SettlementReceiptTransitionKindV2) -> u8 {
    match kind {
        SettlementReceiptTransitionKindV2::None => 0,
        SettlementReceiptTransitionKindV2::PortfolioPairV2 => {
            PORTFOLIO_PAIR_RECEIPT_TRANSITION_KIND_V2_BYTE
        }
    }
}

fn decode_side(value: u8) -> Result<Side, PortfolioExecutionErrorV2> {
    match value {
        0 => Ok(Side::Buy),
        1 => Ok(Side::Sell),
        _ => Err(PortfolioExecutionErrorV2::InvalidCodec),
    }
}

fn is_zero_identity(identity: &PortfolioIdentityV2) -> bool {
    identity.iter().all(|byte| *byte == 0)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, PortfolioExecutionErrorV2> {
    let bytes = input
        .get(offset..offset + 2)
        .ok_or(PortfolioExecutionErrorV2::InvalidCodec)?;
    let mut array = [0u8; 2];
    array.copy_from_slice(bytes);
    Ok(u16::from_le_bytes(array))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, PortfolioExecutionErrorV2> {
    let bytes = input
        .get(offset..offset + 8)
        .ok_or(PortfolioExecutionErrorV2::InvalidCodec)?;
    let mut array = [0u8; 8];
    array.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(array))
}

fn read_u128(input: &[u8], offset: usize) -> Result<u128, PortfolioExecutionErrorV2> {
    let bytes = input
        .get(offset..offset + 16)
        .ok_or(PortfolioExecutionErrorV2::InvalidCodec)?;
    let mut array = [0u8; 16];
    array.copy_from_slice(bytes);
    Ok(u128::from_le_bytes(array))
}
