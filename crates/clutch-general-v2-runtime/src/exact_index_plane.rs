// SPDX-License-Identifier: AGPL-3.0-or-later

//! Disabled exact index plane for frozen General V2 settlement inputs.
//!
//! The counted indexed Root now owns a centrally reserved account version and
//! the canonical in-place Root PDA, but this module deliberately owns no live
//! instruction or capability bit. It defines the active-width account pair and
//! pure construction/closure postwrites before a later SBF adapter makes it
//! reachable. Construction accepts only complete
//! hostile-decoded V5 pages and the selected sealed CandidateFeed; no caller
//! supplies a location, adjacency row, aggregate, count, or semantic identity.
//!
//! The locator maps dense frozen-order rank to its authenticated `(page,slot)`
//! placement.  The adjacency account groups every real end of every selected
//! settlement slice by order and carries the exact selected entitlement
//! aggregate.  Together they are sufficient to replace both the complete-book
//! lookup and complete-witness rescan in pair entitlement materialization.

use clutch_batch::Side;
use clutch_general_v2_contract::{
    DeletableRentOwnerV1, EconomicDomainV2AccountV1, Id32, MarketBindingV2,
    ExactIndexChildrenStateV1, IndexedSettlementRootRentPreparationV1,
    IndexedSettlementRootV1AccountV1, SettlementRootPhaseV1, SettlementRootV1AccountV1,
    INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG, INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION,
    INDEXED_SETTLEMENT_ROOT_BYTES_V1, MARKET_BINDING_ACCOUNT_BYTES_V2, MAX_ORDERS,
    MAX_OUTCOMES, MAX_SLICES,
};
use clutch_product_series::MarketGenesisProfileV2;
use clutch_solana_layout::{
    order_page_v5::verify_page_v5,
    registry::{
        GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_TAG,
        GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_VERSION,
        GENERAL_V2_CANDIDATE_ADJACENCY_MAX_ACCOUNT_BYTES,
        GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_TAG,
        GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_VERSION,
        GENERAL_V2_FROZEN_ORDER_LOCATOR_MAX_ACCOUNT_BYTES,
        GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_BYTES,
        GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION,
        GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG,
    },
    PriceGridAccount, MAX_ORDERS_PER_PAGE, MAX_ORDER_PAGES,
};
use sha2::{Digest, Sha256};

use clutch_collateral_adapter_v2::BoundCollateralProfileV2;

use crate::{
    bind_settlement_root_traversal_v4, derive_settlement_traversal_projection_v4,
    project_owner_blind_book_costed_v1, GeneralOrderPageInputV5, SettlementLegV1,
    SettlementRouteV1,
};

/// Schema version accepted by both exact index accounts.
pub const EXACT_INDEX_PLANE_VERSION_V1: u8 = 1;
/// Sealed immutable state; no partially built account is a valid index.
pub const EXACT_INDEX_PLANE_STATE_SEALED_V1: u8 = 1;
/// Exact locator account tag/version plus domain suffix.
pub const FROZEN_ORDER_LOCATOR_MAGIC_V1: [u8; 8] = [
    GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_TAG,
    GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_VERSION,
    b'D',
    b'C',
    b'I',
    b'X',
    b'L',
    b'1',
];
/// Exact candidate adjacency tag/version plus domain suffix.
pub const CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1: [u8; 8] = [
    GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_TAG,
    GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_VERSION,
    b'D',
    b'C',
    b'I',
    b'X',
    b'A',
    b'1',
];
/// Domain for the exact MarketBinding V2 account-data digest.
pub const EXACT_INDEX_MARKET_BINDING_DIGEST_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/exact-index-market-binding/v1\0";
/// Domain for the ordered page-account/page-body set digest.
pub const EXACT_INDEX_PAGE_SET_DIGEST_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/exact-index-page-set/v1\0";
/// Domain for the semantic identity shared by the account pair.
pub const EXACT_INDEX_PLANE_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/exact-index-plane/v1\0";
/// Domain for one exact locator account body.
pub const FROZEN_ORDER_LOCATOR_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/frozen-order-locator-data/v1\0";
/// Domain for one exact adjacency account body.
pub const CANDIDATE_ORDER_SLICE_INDEX_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/candidate-order-slice-index-data/v1\0";

const ENVELOPE_BYTES: usize = 16;
const COMMON_ID_COUNT: usize = 18;
const COMMON_IDS_BYTES: usize = COMMON_ID_COUNT * 32;
const COMMON_COUNTER_BYTES: usize = 24;
const RENT_OWNER_BYTES: usize = 48;
/// Exact fixed header before either active row tail.
pub const EXACT_INDEX_COMMON_HEADER_BYTES_V1: usize =
    ENVELOPE_BYTES + COMMON_IDS_BYTES + COMMON_COUNTER_BYTES + RENT_OWNER_BYTES;
/// Exact width of one dense-order locator row.
pub const FROZEN_ORDER_LOCATOR_ROW_BYTES_V1: usize = 4;
/// Exact width of one order aggregate/adjacency directory row.
pub const CANDIDATE_ORDER_AGGREGATE_ROW_BYTES_V1: usize = 32;
/// Exact width of one real order end of a selected settlement slice.
pub const CANDIDATE_ORDER_SLICE_EDGE_BYTES_V1: usize = 16;
/// Maximum real slice ends: two for every direct slice.
pub const MAX_EXACT_INDEX_EDGES_V1: usize = MAX_SLICES * 2;
/// The account plane has no live instruction/capability in the current profile.
/// The reserved Root successor owns its exact counts; promotion still requires
/// the complete SBF transition and close family. Dealer or receipt counters
/// must never be repurposed for this pair.
pub const EXACT_INDEX_PLANE_LIVE_ENABLED_V1: bool = false;
/// Largest active locator body.
pub const FROZEN_ORDER_LOCATOR_MAX_BYTES_V1: usize =
    EXACT_INDEX_COMMON_HEADER_BYTES_V1 + MAX_ORDERS * FROZEN_ORDER_LOCATOR_ROW_BYTES_V1;
/// Largest active adjacency body.
pub const CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1: usize = EXACT_INDEX_COMMON_HEADER_BYTES_V1
    + MAX_ORDERS * CANDIDATE_ORDER_AGGREGATE_ROW_BYTES_V1
    + MAX_EXACT_INDEX_EDGES_V1 * CANDIDATE_ORDER_SLICE_EDGE_BYTES_V1;

const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_SLICES == 416);
const _: () = assert!(MAX_ORDER_PAGES == 4);
const _: () = assert!(MAX_ORDERS_PER_PAGE == 16);
const _: () = assert!(EXACT_INDEX_COMMON_HEADER_BYTES_V1 == 664);
const _: () = assert!(FROZEN_ORDER_LOCATOR_MAX_BYTES_V1 == 920);
const _: () = assert!(CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1 == 16_024);
const _: () = assert!(
    FROZEN_ORDER_LOCATOR_MAX_BYTES_V1 == GENERAL_V2_FROZEN_ORDER_LOCATOR_MAX_ACCOUNT_BYTES
);
const _: () = assert!(
    CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1
        == GENERAL_V2_CANDIDATE_ADJACENCY_MAX_ACCOUNT_BYTES
);
const _: () = assert!(
    INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG == GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG
);
const _: () = assert!(
    INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION
        == GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION
);
const _: () = assert!(
    INDEXED_SETTLEMENT_ROOT_BYTES_V1 == GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_BYTES
);

/// Fail-closed refusal set for the disabled exact index plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactIndexPlaneErrorV1 {
    /// A complete V5 page set failed hostile decoding or canonical projection.
    FrozenBook,
    /// The selected CandidateFeed failed hostile decoding or traversal checks.
    CandidateTraversal,
    /// The counted root was not the root for this exact selected traversal.
    RootBinding,
    /// Genesis/profile, MarketBinding, grid, or root identities disagreed.
    BindingMismatch,
    /// An identity required to be live was zero.
    ZeroIdentity,
    /// An active width or count was outside the fixed protocol bounds.
    InvalidCount,
    /// A version, state, flag, enum, or reserved byte was noncanonical.
    InvalidState,
    /// An offset, total, balance, or conversion overflowed.
    ArithmeticOverflow,
    /// A caller buffer was not the one exact active width.
    WrongLength,
    /// A locator row was duplicate, unordered, or outside its page.
    InvalidLocator,
    /// An adjacency row or edge was incomplete, asymmetric, or unordered.
    InvalidAdjacency,
    /// A persisted selected aggregate disagreed with the candidate traversal.
    AggregateMismatch,
    /// Account creation metadata did not describe a fresh writable system account.
    InvalidCreateAccount,
    /// Refund principal/donation ownership or close balance geometry was invalid.
    InvalidRent,
    /// Closure was attempted before the counted settlement root became terminal.
    NonTerminalRoot,
}

/// One dense frozen-order location derived from the complete V5 page set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrozenOrderLocatorRowV1 {
    page_index: u16,
    page_slot: u8,
}

impl FrozenOrderLocatorRowV1 {
    /// Canonical V5 page index.
    pub const fn page_index(self) -> u16 {
        self.page_index
    }

    /// Canonical physical slot, including any preceding tombstone gaps.
    pub const fn page_slot(self) -> u8 {
        self.page_slot
    }
}

const EMPTY_LOCATOR: FrozenOrderLocatorRowV1 = FrozenOrderLocatorRowV1 {
    page_index: 0,
    page_slot: 0,
};

/// Buy/sell role of one real order in the selected candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExactIndexOrderSideV1 {
    /// Buyer receives native Eggs.
    Buy = 1,
    /// Seller supplies native Eggs.
    Sell = 2,
}

impl ExactIndexOrderSideV1 {
    const fn code(self) -> u8 {
        match self {
            Self::Buy => 1,
            Self::Sell => 2,
        }
    }

    fn decode(value: u8) -> Result<Self, ExactIndexPlaneErrorV1> {
        match value {
            1 => Ok(Self::Buy),
            2 => Ok(Self::Sell),
            _ => Err(ExactIndexPlaneErrorV1::InvalidState),
        }
    }
}

/// Counterparty class stored on one real order end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExactIndexCounterpartyV1 {
    /// Another real order; `counterparty_order` is live.
    Order = 0,
    /// A virtual complete-set split supplying one buy.
    Split = 1,
    /// A virtual complete-set merge absorbing one sell.
    Merge = 2,
}

impl ExactIndexCounterpartyV1 {
    const fn code(self) -> u8 {
        match self {
            Self::Order => 0,
            Self::Split => 1,
            Self::Merge => 2,
        }
    }

    fn decode(value: u8) -> Result<Self, ExactIndexPlaneErrorV1> {
        match value {
            0 => Ok(Self::Order),
            1 => Ok(Self::Split),
            2 => Ok(Self::Merge),
            _ => Err(ExactIndexPlaneErrorV1::InvalidState),
        }
    }
}

/// Directory and aggregate row for one dense frozen order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateOrderAggregateRowV1 {
    first_edge: u16,
    edge_count: u16,
    distinct_real_counterparties: u16,
    virtual_edge_count: u16,
    total_quantity: u64,
    entitled_quantity: u64,
    side: ExactIndexOrderSideV1,
}

impl CandidateOrderAggregateRowV1 {
    /// First edge in the account-wide canonical edge tail.
    pub const fn first_edge(self) -> u16 {
        self.first_edge
    }

    /// Number of active edges in this order's group.
    pub const fn edge_count(self) -> u16 {
        self.edge_count
    }

    /// Number of distinct real counterparties in the group.
    pub const fn distinct_real_counterparties(self) -> u16 {
        self.distinct_real_counterparties
    }

    /// Number of virtual split/merge edges in the group.
    pub const fn virtual_edge_count(self) -> u16 {
        self.virtual_edge_count
    }

    /// Exact Egg quantity across every selected edge of this order.
    pub const fn total_quantity(self) -> u64 {
        self.total_quantity
    }

    /// Candidate-derived coefficient-weighted entitled Egg quantity.
    pub const fn entitled_quantity(self) -> u64 {
        self.entitled_quantity
    }

    /// Authenticated frozen order side.
    pub const fn side(self) -> ExactIndexOrderSideV1 {
        self.side
    }
}

const EMPTY_AGGREGATE: CandidateOrderAggregateRowV1 = CandidateOrderAggregateRowV1 {
    first_edge: 0,
    edge_count: 0,
    distinct_real_counterparties: 0,
    virtual_edge_count: 0,
    total_quantity: 0,
    entitled_quantity: 0,
    side: ExactIndexOrderSideV1::Buy,
};

/// One canonical selected-slice adjacency edge belonging to a real order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateOrderSliceEdgeV1 {
    slice_index: u16,
    counterparty_kind: ExactIndexCounterpartyV1,
    counterparty_order: u8,
    outcome: u8,
    side: ExactIndexOrderSideV1,
    quantity: u64,
}

impl CandidateOrderSliceEdgeV1 {
    /// Canonical CandidateFeed slice index.
    pub const fn slice_index(self) -> u16 {
        self.slice_index
    }

    /// Real-order, split, or merge counterparty classification.
    pub const fn counterparty_kind(self) -> ExactIndexCounterpartyV1 {
        self.counterparty_kind
    }

    /// Dense counterparty order index, zero for a virtual edge.
    pub const fn counterparty_order(self) -> u8 {
        self.counterparty_order
    }

    /// Exact native outcome transferred by the selected slice.
    pub const fn outcome(self) -> u8 {
        self.outcome
    }

    /// Side of the order owning this edge.
    pub const fn side(self) -> ExactIndexOrderSideV1 {
        self.side
    }

    /// Positive Egg quantity carried by the selected slice.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
}

const EMPTY_EDGE: CandidateOrderSliceEdgeV1 = CandidateOrderSliceEdgeV1 {
    slice_index: 0,
    counterparty_kind: ExactIndexCounterpartyV1::Order,
    counterparty_order: 0,
    outcome: 0,
    side: ExactIndexOrderSideV1::Buy,
    quantity: 0,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactIndexCommonV1 {
    market: Id32,
    epoch: Id32,
    order_set: Id32,
    settlement_candidate: Id32,
    selected_feed: Id32,
    candidate_bundle_digest: Id32,
    realm: Id32,
    profile: Id32,
    capability_profile: Id32,
    market_genesis_profile: Id32,
    market_binding_account: Id32,
    market_binding_digest: Id32,
    economic_domain_digest: Id32,
    page_set_digest: Id32,
    plane_id: Id32,
    sibling_account: Id32,
    settlement_root_account: Id32,
    owner_order_set_digest: Id32,
    epoch_generation: u64,
    page_count: u8,
    order_count: u8,
    outcome_count: u8,
    slice_count: u16,
    edge_count: u16,
    page_slot_counts: [u8; MAX_ORDER_PAGES],
    rent: DeletableRentOwnerV1,
    stored_bump: u8,
}

impl ExactIndexCommonV1 {
    fn validate(&self) -> Result<(), ExactIndexPlaneErrorV1> {
        for id in [
            self.market,
            self.epoch,
            self.order_set,
            self.settlement_candidate,
            self.selected_feed,
            self.candidate_bundle_digest,
            self.realm,
            self.profile,
            self.capability_profile,
            self.market_genesis_profile,
            self.market_binding_account,
            self.market_binding_digest,
            self.economic_domain_digest,
            self.page_set_digest,
            self.plane_id,
            self.sibling_account,
            self.settlement_root_account,
            self.owner_order_set_digest,
        ] {
            if id.is_zero() {
                return Err(ExactIndexPlaneErrorV1::ZeroIdentity);
            }
        }
        if self.epoch_generation == 0
            || self.page_count == 0
            || usize::from(self.page_count) > MAX_ORDER_PAGES
            || self.order_count == 0
            || usize::from(self.order_count) > MAX_ORDERS
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.slice_count == 0
            || usize::from(self.slice_count) > MAX_SLICES
            || self.edge_count < self.slice_count
            || usize::from(self.edge_count) > MAX_EXACT_INDEX_EDGES_V1
        {
            return Err(ExactIndexPlaneErrorV1::InvalidCount);
        }
        let mut page = 0usize;
        while page < MAX_ORDER_PAGES {
            if page < usize::from(self.page_count) {
                if self.page_slot_counts[page] == 0
                    || usize::from(self.page_slot_counts[page]) > MAX_ORDERS_PER_PAGE
                {
                    return Err(ExactIndexPlaneErrorV1::InvalidCount);
                }
            } else if self.page_slot_counts[page] != 0 {
                return Err(ExactIndexPlaneErrorV1::InvalidState);
            }
            page += 1;
        }
        self.rent
            .validate()
            .map_err(|_| ExactIndexPlaneErrorV1::InvalidRent)?;
        if self.rent.payer == self.sibling_account
            || self.rent.payer == self.settlement_root_account
            || self.rent.payer == self.market
        {
            return Err(ExactIndexPlaneErrorV1::BindingMismatch);
        }
        Ok(())
    }

    fn semantic_eq(&self, other: &Self) -> bool {
        self.market == other.market
            && self.epoch == other.epoch
            && self.order_set == other.order_set
            && self.settlement_candidate == other.settlement_candidate
            && self.selected_feed == other.selected_feed
            && self.candidate_bundle_digest == other.candidate_bundle_digest
            && self.realm == other.realm
            && self.profile == other.profile
            && self.capability_profile == other.capability_profile
            && self.market_genesis_profile == other.market_genesis_profile
            && self.market_binding_account == other.market_binding_account
            && self.market_binding_digest == other.market_binding_digest
            && self.economic_domain_digest == other.economic_domain_digest
            && self.page_set_digest == other.page_set_digest
            && self.plane_id == other.plane_id
            && self.settlement_root_account == other.settlement_root_account
            && self.owner_order_set_digest == other.owner_order_set_digest
            && self.epoch_generation == other.epoch_generation
            && self.page_count == other.page_count
            && self.order_count == other.order_count
            && self.outcome_count == other.outcome_count
            && self.slice_count == other.slice_count
            && self.edge_count == other.edge_count
            && self.page_slot_counts == other.page_slot_counts
    }
}

/// Exact active-width frozen-order locator account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrozenOrderLocatorV1 {
    common: ExactIndexCommonV1,
    rows: [FrozenOrderLocatorRowV1; MAX_ORDERS],
}

impl FrozenOrderLocatorV1 {
    /// Exact active byte width; no inactive fixed-capacity tail is persisted.
    pub fn encoded_len(&self) -> Result<usize, ExactIndexPlaneErrorV1> {
        locator_encoded_len(self.common.order_count)
    }

    /// Shared semantic plane identity.
    pub const fn plane_id(&self) -> Id32 {
        self.common.plane_id
    }

    /// Exact sibling adjacency account identity.
    pub const fn sibling_account(&self) -> Id32 {
        self.common.sibling_account
    }

    /// Exact persisted rent/refund owner.
    pub const fn rent_owner(&self) -> DeletableRentOwnerV1 {
        self.common.rent
    }

    /// Dense frozen-order count.
    pub const fn order_count(&self) -> u8 {
        self.common.order_count
    }

    /// Checked locator for one dense frozen-order rank.
    pub fn locate(&self, order_index: u8) -> Option<FrozenOrderLocatorRowV1> {
        if order_index < self.common.order_count {
            Some(self.rows[usize::from(order_index)])
        } else {
            None
        }
    }

    /// Encode exactly the active header and locator prefix.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), ExactIndexPlaneErrorV1> {
        self.validate()?;
        if output.len() != self.encoded_len()? {
            return Err(ExactIndexPlaneErrorV1::WrongLength);
        }
        let mut writer = ExactWriter::new(output);
        encode_common(&mut writer, FROZEN_ORDER_LOCATOR_MAGIC_V1, &self.common)?;
        let mut order = 0usize;
        while order < usize::from(self.common.order_count) {
            writer.u16(self.rows[order].page_index)?;
            writer.u8(self.rows[order].page_slot)?;
            writer.u8(0)?;
            order += 1;
        }
        writer.finish()
    }

    /// Decode only version 1 and reject every trailing or inactive byte.
    pub fn decode(input: &[u8]) -> Result<Self, ExactIndexPlaneErrorV1> {
        if input.len() < EXACT_INDEX_COMMON_HEADER_BYTES_V1 {
            return Err(ExactIndexPlaneErrorV1::WrongLength);
        }
        let mut reader = ExactReader::new(input);
        let common = decode_common(&mut reader, FROZEN_ORDER_LOCATOR_MAGIC_V1)?;
        if input.len() != locator_encoded_len(common.order_count)? {
            return Err(ExactIndexPlaneErrorV1::WrongLength);
        }
        let mut rows = [EMPTY_LOCATOR; MAX_ORDERS];
        let mut order = 0usize;
        while order < usize::from(common.order_count) {
            rows[order] = FrozenOrderLocatorRowV1 {
                page_index: reader.u16()?,
                page_slot: reader.u8()?,
            };
            reader.reserved(1)?;
            order += 1;
        }
        reader.finish()?;
        let value = Self { common, rows };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), ExactIndexPlaneErrorV1> {
        self.common.validate()?;
        let mut order = 0usize;
        let mut prior_page = 0u16;
        let mut prior_slot = 0u8;
        while order < usize::from(self.common.order_count) {
            let row = self.rows[order];
            if usize::from(row.page_index) >= usize::from(self.common.page_count)
                || row.page_slot
                    >= self.common.page_slot_counts[usize::from(row.page_index)]
                || (order != 0
                    && (row.page_index < prior_page
                        || (row.page_index == prior_page && row.page_slot <= prior_slot)))
            {
                return Err(ExactIndexPlaneErrorV1::InvalidLocator);
            }
            prior_page = row.page_index;
            prior_slot = row.page_slot;
            order += 1;
        }
        while order < MAX_ORDERS {
            if self.rows[order] != EMPTY_LOCATOR {
                return Err(ExactIndexPlaneErrorV1::InvalidLocator);
            }
            order += 1;
        }
        Ok(())
    }
}

/// Exact active-width candidate-bound order adjacency/aggregate account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateOrderSliceIndexV1 {
    common: ExactIndexCommonV1,
    aggregates: [CandidateOrderAggregateRowV1; MAX_ORDERS],
    edges: [CandidateOrderSliceEdgeV1; MAX_EXACT_INDEX_EDGES_V1],
}

impl CandidateOrderSliceIndexV1 {
    /// Exact active byte width; inactive aggregate and edge capacity is absent.
    pub fn encoded_len(&self) -> Result<usize, ExactIndexPlaneErrorV1> {
        adjacency_encoded_len(self.common.order_count, self.common.edge_count)
    }

    /// Shared semantic plane identity.
    pub const fn plane_id(&self) -> Id32 {
        self.common.plane_id
    }

    /// Exact sibling locator account identity.
    pub const fn sibling_account(&self) -> Id32 {
        self.common.sibling_account
    }

    /// Exact persisted rent/refund owner.
    pub const fn rent_owner(&self) -> DeletableRentOwnerV1 {
        self.common.rent
    }

    /// Checked aggregate row for one dense frozen-order rank.
    pub fn aggregate(&self, order_index: u8) -> Option<CandidateOrderAggregateRowV1> {
        if order_index < self.common.order_count {
            Some(self.aggregates[usize::from(order_index)])
        } else {
            None
        }
    }

    /// Checked edge at an account-wide active edge index.
    pub fn edge(&self, edge_index: u16) -> Option<CandidateOrderSliceEdgeV1> {
        if edge_index < self.common.edge_count {
            Some(self.edges[usize::from(edge_index)])
        } else {
            None
        }
    }

    /// Encode exactly the active header, aggregate prefix, and edge prefix.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), ExactIndexPlaneErrorV1> {
        self.validate()?;
        if output.len() != self.encoded_len()? {
            return Err(ExactIndexPlaneErrorV1::WrongLength);
        }
        let mut writer = ExactWriter::new(output);
        encode_common(
            &mut writer,
            CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1,
            &self.common,
        )?;
        let mut order = 0usize;
        while order < usize::from(self.common.order_count) {
            encode_aggregate(&mut writer, self.aggregates[order])?;
            order += 1;
        }
        let mut edge = 0usize;
        while edge < usize::from(self.common.edge_count) {
            encode_edge(&mut writer, self.edges[edge])?;
            edge += 1;
        }
        writer.finish()
    }

    /// Decode only version 1 and reject every trailing or inactive byte.
    pub fn decode(input: &[u8]) -> Result<Self, ExactIndexPlaneErrorV1> {
        if input.len() < EXACT_INDEX_COMMON_HEADER_BYTES_V1 {
            return Err(ExactIndexPlaneErrorV1::WrongLength);
        }
        let mut reader = ExactReader::new(input);
        let common = decode_common(&mut reader, CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1)?;
        if input.len() != adjacency_encoded_len(common.order_count, common.edge_count)? {
            return Err(ExactIndexPlaneErrorV1::WrongLength);
        }
        let mut aggregates = [EMPTY_AGGREGATE; MAX_ORDERS];
        let mut order = 0usize;
        while order < usize::from(common.order_count) {
            aggregates[order] = decode_aggregate(&mut reader)?;
            order += 1;
        }
        let mut edges = [EMPTY_EDGE; MAX_EXACT_INDEX_EDGES_V1];
        let mut edge = 0usize;
        while edge < usize::from(common.edge_count) {
            edges[edge] = decode_edge(&mut reader)?;
            edge += 1;
        }
        reader.finish()?;
        let value = Self {
            common,
            aggregates,
            edges,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), ExactIndexPlaneErrorV1> {
        self.common.validate()?;
        let mut expected_first = 0u16;
        let mut order = 0usize;
        let mut observed_edges = 0usize;
        while order < usize::from(self.common.order_count) {
            let aggregate = self.aggregates[order];
            if aggregate.first_edge != expected_first
                || aggregate.entitled_quantity != aggregate.total_quantity
            {
                return Err(ExactIndexPlaneErrorV1::AggregateMismatch);
            }
            let end = expected_first
                .checked_add(aggregate.edge_count)
                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
            if end > self.common.edge_count {
                return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
            }
            let mut cursor = aggregate.first_edge;
            let mut prior_slice = None;
            let mut total = 0u64;
            let mut virtual_count = 0u16;
            let mut distinct_real = 0u16;
            while cursor < end {
                let edge = self.edges[usize::from(cursor)];
                if edge.quantity == 0
                    || edge.outcome >= self.common.outcome_count
                    || edge.side != aggregate.side
                    || prior_slice.is_some_and(|prior| edge.slice_index <= prior)
                    || edge.slice_index >= self.common.slice_count
                {
                    return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
                }
                match (aggregate.side, edge.counterparty_kind) {
                    (ExactIndexOrderSideV1::Buy, ExactIndexCounterpartyV1::Order)
                    | (ExactIndexOrderSideV1::Sell, ExactIndexCounterpartyV1::Order) => {
                        if usize::from(edge.counterparty_order) >= usize::from(self.common.order_count)
                            || usize::from(edge.counterparty_order) == order
                        {
                            return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
                        }
                        let mut first = true;
                        let mut earlier = aggregate.first_edge;
                        while earlier < cursor {
                            let prior = self.edges[usize::from(earlier)];
                            if prior.counterparty_kind == ExactIndexCounterpartyV1::Order
                                && prior.counterparty_order == edge.counterparty_order
                            {
                                first = false;
                            }
                            earlier = earlier
                                .checked_add(1)
                                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                        }
                        if first {
                            distinct_real = distinct_real
                                .checked_add(1)
                                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                        }
                    }
                    (ExactIndexOrderSideV1::Buy, ExactIndexCounterpartyV1::Split)
                    | (ExactIndexOrderSideV1::Sell, ExactIndexCounterpartyV1::Merge) => {
                        if edge.counterparty_order != 0 {
                            return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
                        }
                        virtual_count = virtual_count
                            .checked_add(1)
                            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                    }
                    _ => return Err(ExactIndexPlaneErrorV1::InvalidAdjacency),
                }
                total = total
                    .checked_add(edge.quantity)
                    .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                prior_slice = Some(edge.slice_index);
                cursor = cursor
                    .checked_add(1)
                    .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                observed_edges += 1;
            }
            if total != aggregate.total_quantity
                || virtual_count != aggregate.virtual_edge_count
                || distinct_real != aggregate.distinct_real_counterparties
            {
                return Err(ExactIndexPlaneErrorV1::AggregateMismatch);
            }
            expected_first = end;
            order += 1;
        }
        if expected_first != self.common.edge_count
            || observed_edges != usize::from(self.common.edge_count)
        {
            return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
        }
        while order < MAX_ORDERS {
            if self.aggregates[order] != EMPTY_AGGREGATE {
                return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
            }
            order += 1;
        }
        let mut edge = usize::from(self.common.edge_count);
        while edge < MAX_EXACT_INDEX_EDGES_V1 {
            if self.edges[edge] != EMPTY_EDGE {
                return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
            }
            edge += 1;
        }
        validate_symmetric_edges(self)
    }

}

/// Adapter-authenticated metadata for one fresh index PDA creation.
///
/// These are account-meta facts, not semantic index rows.  The later SBF
/// adapter must authenticate them before passing them to this pure seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexCreateAccountInputV1 {
    /// Fresh target PDA account.
    pub account: Id32,
    /// Dragon's Clutch program owner installed after creation.
    pub program_id: Id32,
    /// System Program identity currently owning the unallocated target.
    pub system_program: Id32,
    /// Exact rent payer and eventual principal refund recipient.
    pub payer: Id32,
    /// Authenticated payer lamports before both atomic creates.
    pub payer_lamports: u64,
    /// Hostile prefund already present on the target and routed as donation.
    pub target_lamports: u64,
    /// Current target owner, which must equal `system_program`.
    pub target_owner: Id32,
    /// Current target data width, which must be zero.
    pub target_data_len: usize,
    /// Runtime writable bit for the target.
    pub target_writable: bool,
    /// Runtime executable bit for the target.
    pub target_executable: bool,
    /// Full rent-exempt principal for the exact active width.
    pub rent_exempt_minimum: u64,
    /// PDA bump derived by the later adapter from its versioned seed tuple.
    pub stored_bump: u8,
}

/// Unforgeable placeholder for a future counted-root admission capability.
///
/// There is intentionally no public or crate-private constructor. The sole
/// internal mint is the higher-level counted-root creation plan, which returns
/// the exact indexed-root write atomically with both child writes. No SBF route
/// currently consumes that plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactIndexAdmissionV1 {
    _private: (),
}

/// Unforgeable placeholder for a future counted-root retirement capability.
///
/// The sole internal mint is the higher-level counted-root retirement plan,
/// which atomically advances both sibling children from live to retired.
/// Terminality of the nested Root V1 is necessary but deliberately not
/// sufficient authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactIndexRetirementV1 {
    _private: (),
}

impl ExactIndexCreateAccountInputV1 {
    fn validate(
        self,
        expected_len: usize,
        semantic_accounts: &[Id32],
    ) -> Result<DeletableRentOwnerV1, ExactIndexPlaneErrorV1> {
        for id in [
            self.account,
            self.program_id,
            self.system_program,
            self.payer,
            self.target_owner,
        ] {
            if id.is_zero() {
                return Err(ExactIndexPlaneErrorV1::ZeroIdentity);
            }
        }
        if self.program_id == self.system_program
            || self.account == self.payer
            || self.account == self.program_id
            || self.account == self.system_program
            || self.payer == self.program_id
            || self.payer == self.system_program
            || self.target_owner != self.system_program
            || self.target_data_len != 0
            || !self.target_writable
            || self.target_executable
            || self.rent_exempt_minimum == 0
            || expected_len < EXACT_INDEX_COMMON_HEADER_BYTES_V1
            || semantic_accounts.iter().any(|account| *account == self.account)
        {
            return Err(ExactIndexPlaneErrorV1::InvalidCreateAccount);
        }
        if self.payer_lamports < self.rent_exempt_minimum {
            return Err(ExactIndexPlaneErrorV1::InvalidRent);
        }
        let rent = DeletableRentOwnerV1 {
            payer: self.payer,
            refundable_principal: self.rent_exempt_minimum,
            donation_floor: self.target_lamports,
        };
        rent.validate()
            .map_err(|_| ExactIndexPlaneErrorV1::InvalidRent)?;
        Ok(rent)
    }
}

fn validate_create_pair_identities(
    locator: ExactIndexCreateAccountInputV1,
    adjacency: ExactIndexCreateAccountInputV1,
    neutral_sink: Id32,
) -> Result<(), ExactIndexPlaneErrorV1> {
    if locator.account == adjacency.account
        || locator.program_id != adjacency.program_id
        || locator.system_program != adjacency.system_program
        || locator.payer == neutral_sink
        || adjacency.payer == neutral_sink
        || locator.payer == adjacency.account
        || adjacency.payer == locator.account
    {
        return Err(ExactIndexPlaneErrorV1::InvalidCreateAccount);
    }
    Ok(())
}

/// Complete hostile construction input for the disabled exact index pair.
///
/// `pages`, `selected_feed_body`, and every immutable artifact must be account
/// owner/PDA authenticated by a future adapter.  This pure constructor then
/// decodes and derives every persisted semantic field from them.  There are no
/// caller-authored order locations, slice edges, aggregates, counts, or IDs.
#[derive(Clone, Copy, Debug)]
pub struct ConstructExactIndexPlaneInputV1<'a> {
    /// Complete canonical V5 page set in page-index order.
    pub pages: &'a [GeneralOrderPageInputV5<'a>],
    /// Exact EconomicDomain V2 account body projection.
    pub economic_domain: &'a EconomicDomainV2AccountV1,
    /// Exact immutable MarketBinding V2 account identity.
    pub market_binding_account: Id32,
    /// Exact hostile-decoded immutable MarketBinding V2 body.
    pub market_binding: &'a MarketBindingV2,
    /// Exact hostile-decoded PriceGrid account.
    pub price_grid: &'a PriceGridAccount,
    /// Exact hostile-decoded Product MarketGenesisProfile V2 body.
    pub market_genesis_profile: &'a MarketGenesisProfileV2,
    /// Fully checked collateral/profile/release chain used by settlement.
    pub collateral: BoundCollateralProfileV2,
    /// Retained sealed CandidateFeed V2 account identity.
    pub selected_feed_account: Id32,
    /// Exact retained sealed CandidateFeed V2 bytes.
    pub selected_feed_body: &'a [u8],
    /// Adapter-authenticated immutable Reservation terms identity.
    pub reservation_terms: Id32,
    /// Adapter-authenticated immutable Reservation policy identity.
    pub reservation_policy: Id32,
    /// Counted selected SettlementRoot account identity.
    pub settlement_root_account: Id32,
    /// Exact hostile-decoded selected SettlementRoot body.
    pub settlement_root: &'a SettlementRootV1AccountV1,
    /// Fresh locator PDA metadata.
    pub locator_create: ExactIndexCreateAccountInputV1,
    /// Fresh adjacency PDA metadata.
    pub adjacency_create: ExactIndexCreateAccountInputV1,
}

/// Private typed atomic postwrites for a later SBF creation instruction.
///
/// The fields are intentionally private: an adapter can encode the two exact
/// bodies and apply the checked balance deltas, but cannot edit derived rows or
/// semantic bindings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexPlaneCreatePostwritesV1 {
    locator_account: Id32,
    adjacency_account: Id32,
    locator: FrozenOrderLocatorV1,
    adjacency: CandidateOrderSliceIndexV1,
    locator_data_id: Id32,
    adjacency_data_id: Id32,
    locator_payer_debit: u64,
    adjacency_payer_debit: u64,
    locator_post_lamports: u64,
    adjacency_post_lamports: u64,
}

impl ExactIndexPlaneCreatePostwritesV1 {
    /// Fresh locator PDA authenticated by the later adapter.
    pub const fn locator_account(&self) -> Id32 {
        self.locator_account
    }

    /// Fresh candidate adjacency PDA authenticated by the later adapter.
    pub const fn adjacency_account(&self) -> Id32 {
        self.adjacency_account
    }

    /// Shared content identity of the exact account pair.
    pub const fn plane_id(&self) -> Id32 {
        self.locator.common.plane_id
    }

    /// Contract digest of the exact active locator body.
    pub const fn locator_data_id(&self) -> Id32 {
        self.locator_data_id
    }

    /// Contract digest of the exact active adjacency body.
    pub const fn adjacency_data_id(&self) -> Id32 {
        self.adjacency_data_id
    }

    /// Exact active locator account data width.
    pub fn locator_data_len(&self) -> Result<usize, ExactIndexPlaneErrorV1> {
        self.locator.encoded_len()
    }

    /// Exact active adjacency account data width.
    pub fn adjacency_data_len(&self) -> Result<usize, ExactIndexPlaneErrorV1> {
        self.adjacency.encoded_len()
    }

    /// Full rent principal debited from the locator payer.
    pub const fn locator_payer_debit(&self) -> u64 {
        self.locator_payer_debit
    }

    /// Full rent principal debited from the adjacency payer.
    pub const fn adjacency_payer_debit(&self) -> u64 {
        self.adjacency_payer_debit
    }

    /// Target locator balance after preserving hostile prefund as donation.
    pub const fn locator_post_lamports(&self) -> u64 {
        self.locator_post_lamports
    }

    /// Target adjacency balance after preserving hostile prefund as donation.
    pub const fn adjacency_post_lamports(&self) -> u64 {
        self.adjacency_post_lamports
    }

    /// Encode the exact active locator poststate into the caller's account data.
    pub fn encode_locator(&self, output: &mut [u8]) -> Result<(), ExactIndexPlaneErrorV1> {
        self.locator.encode(output)
    }

    /// Encode the exact active adjacency poststate into the caller's account data.
    pub fn encode_adjacency(&self, output: &mut [u8]) -> Result<(), ExactIndexPlaneErrorV1> {
        self.adjacency.encode(output)
    }

    /// Query through the just-derived immutable pair.
    pub fn pair_coverage(
        &self,
        buy_order: u8,
        sell_order: u8,
    ) -> Result<IndexedPairCoverageV1, ExactIndexPlaneErrorV1> {
        indexed_pair_coverage_v1(&self.locator, &self.adjacency, buy_order, sell_order)
    }
}

/// Atomic counted-root plus two-child creation postwrites.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactIndexRootCreatePostwritesV1 {
    indexed_root: IndexedSettlementRootV1AccountV1,
    indexed_root_data_id: Id32,
    root_rent: IndexedSettlementRootRentPreparationV1,
    indexes: ExactIndexPlaneCreatePostwritesV1,
}

impl CountedExactIndexRootCreatePostwritesV1 {
    /// Breaking root poststate that counts exactly both live siblings.
    pub const fn indexed_root(&self) -> &IndexedSettlementRootV1AccountV1 {
        &self.indexed_root
    }

    /// Account-key-bound exact indexed-root poststate identity.
    pub const fn indexed_root_data_id(&self) -> Id32 {
        self.indexed_root_data_id
    }

    /// Exact fresh-allocation or in-place-upgrade root rent postwrites.
    pub const fn root_rent_preparation(&self) -> &IndexedSettlementRootRentPreparationV1 {
        &self.root_rent
    }

    /// Private exact child creation postwrites in the same rollback domain.
    pub const fn index_postwrites(&self) -> &ExactIndexPlaneCreatePostwritesV1 {
        &self.indexes
    }

    /// Encode the exact reserved-disabled indexed-root successor body.
    pub fn encode_indexed_root(
        &self,
        output: &mut [u8],
    ) -> Result<(), ExactIndexPlaneErrorV1> {
        self.indexed_root
            .encode(output)
            .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)
    }
}

/// Indexed replacement for the old complete-witness pair rescan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedPairCoverageV1 {
    pair_slice_indices: [u16; MAX_OUTCOMES],
    pair_slice_count: u8,
    buy_total: u64,
    sell_total: u64,
    buy_elsewhere: bool,
    sell_elsewhere: bool,
}

/// Unforgeable placeholder for a future counted-root immutable-read authority.
///
/// Its private fields are populated only after the adapter projection checks
/// all three canonical PDAs/owners and the root-held full child body IDs. This
/// keeps the cheap local-row reader unavailable to unauthenticated callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactIndexReadAuthorityV1 {
    plane_id: Id32,
    locator_account: Id32,
    adjacency_account: Id32,
    _private: (),
}

/// Exact sealed account bodies consumed by the bounded pair reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedExactIndexPairInputV1<'a> {
    /// Unforgeable root + account-owner/PDA read authority.
    pub authority: CountedExactIndexReadAuthorityV1,
    /// Exact active locator account body.
    pub locator_body: &'a [u8],
    /// Exact active adjacency account body.
    pub adjacency_body: &'a [u8],
}

/// One adapter-authenticated read-only canonical PDA account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexReadAccountInputV1<'a> {
    /// Actual account key.
    pub account: Id32,
    /// Complete hostile account bytes.
    pub body: &'a [u8],
    /// Actual account owner.
    pub owner: Id32,
    /// Canonical PDA independently derived by the SBF adapter.
    pub canonical_account: Id32,
    /// Canonical bump independently derived by the SBF adapter.
    pub canonical_bump: u8,
    /// Runtime writable privilege, which must be false for a sealed read.
    pub writable: bool,
    /// Runtime executable bit, which must be false.
    pub executable: bool,
}

/// Complete authenticated Root/locator/adjacency read join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticateCountedExactIndexReadInputV1<'a> {
    /// Expected Dragon's Clutch program owner.
    pub program_id: Id32,
    /// Reserved indexed SettlementRoot V2 PDA.
    pub root: ExactIndexReadAccountInputV1<'a>,
    /// Sealed frozen-order locator PDA.
    pub locator: ExactIndexReadAccountInputV1<'a>,
    /// Sealed candidate adjacency PDA.
    pub adjacency: ExactIndexReadAccountInputV1<'a>,
}

impl IndexedPairCoverageV1 {
    /// Active exact pair-slice prefix followed by zero padding.
    pub const fn pair_slice_indices(&self) -> &[u16; MAX_OUTCOMES] {
        &self.pair_slice_indices
    }

    /// Number of exact pair slices, bounded by the outcome width.
    pub const fn pair_slice_count(&self) -> u8 {
        self.pair_slice_count
    }

    /// Buy order's candidate-wide entitled Egg total.
    pub const fn buy_total(&self) -> u64 {
        self.buy_total
    }

    /// Sell order's candidate-wide entitled Egg total.
    pub const fn sell_total(&self) -> u64 {
        self.sell_total
    }

    /// Whether the buy has any real or virtual edge outside this pair.
    pub const fn buy_elsewhere(&self) -> bool {
        self.buy_elsewhere
    }

    /// Whether the sell has any real or virtual edge outside this pair.
    pub const fn sell_elsewhere(&self) -> bool {
        self.sell_elsewhere
    }
}

/// Construct both sealed indexes from complete hostile inputs and the selected root.
pub fn construct_exact_index_plane_v1(
    counted_root_admission: CountedExactIndexAdmissionV1,
    input: ConstructExactIndexPlaneInputV1<'_>,
) -> Result<ExactIndexPlaneCreatePostwritesV1, ExactIndexPlaneErrorV1> {
    let _counted_root_admission = counted_root_admission;
    input
        .market_binding
        .validate()
        .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    input
        .price_grid
        .validate()
        .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    input
        .market_genesis_profile
        .validate_shape()
        .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    input
        .settlement_root
        .validate()
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    let binding = input.market_binding.base();
    let genesis_id = Id32::new(
        input
            .market_genesis_profile
            .id()
            .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?
            .bytes(),
    )
    .map_err(|_| ExactIndexPlaneErrorV1::ZeroIdentity)?;
    let realm = Id32::new(input.market_genesis_profile.realm_id.bytes())
        .map_err(|_| ExactIndexPlaneErrorV1::ZeroIdentity)?;
    let profile = Id32::new(input.market_genesis_profile.profile_id.bytes())
        .map_err(|_| ExactIndexPlaneErrorV1::ZeroIdentity)?;
    let capability_profile = Id32::new(input.market_genesis_profile.capability_profile_id.bytes())
        .map_err(|_| ExactIndexPlaneErrorV1::ZeroIdentity)?;
    let collateral_market = input.collateral.market();
    if input.market_binding_account.is_zero()
        || input.selected_feed_account.is_zero()
        || input.settlement_root_account.is_zero()
        || genesis_id != binding.market_genesis_profile_v2_id
        || realm.bytes() != input.price_grid.realm.bytes()
        || input.market_genesis_profile.price_grid_id.bytes() != input.price_grid.grid.bytes()
        || input.market_genesis_profile.price_measure_policy_id.bytes()
            != binding.price_measure_policy_v1_id.bytes()
        || input.market_genesis_profile.relation_policy_id.bytes()
            != binding.relation_policy_id.bytes()
        || input.market_genesis_profile.score_policy_id.bytes() != binding.score_policy_id.bytes()
        || collateral_market.market.bytes() != binding.market.bytes()
        || collateral_market.realm.bytes() != realm.bytes()
        || collateral_market.profile.bytes() != profile.bytes()
        || input.settlement_root.market_binding() != input.market_binding_account
        || input.settlement_root.market() != binding.market
        || input.settlement_root.order_set().is_zero()
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }

    let projection = project_owner_blind_book_costed_v1(
        input.pages,
        input.settlement_root.order_set(),
        input.economic_domain,
        input.market_binding,
        input.price_grid,
    )
    .map_err(|_| ExactIndexPlaneErrorV1::FrozenBook)?;
    let traversal = derive_settlement_traversal_projection_v4(
        input.selected_feed_account,
        input.selected_feed_body,
        &projection,
        input.reservation_terms,
        input.reservation_policy,
        input.collateral,
    )
    .map_err(|_| ExactIndexPlaneErrorV1::CandidateTraversal)?;
    bind_settlement_root_traversal_v4(
        input.settlement_root_account,
        input.settlement_root,
        &traversal,
    )
    .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    let feed = traversal.feed();
    if feed.order_count == 0 || feed.slice_count == 0 {
        return Err(ExactIndexPlaneErrorV1::InvalidCount);
    }

    let page_count = projection.page_count();
    let mut page_slot_counts = [0u8; MAX_ORDER_PAGES];
    let mut page_set_hasher = Sha256::new();
    page_set_hasher.update(EXACT_INDEX_PAGE_SET_DIGEST_DOMAIN_V1);
    page_set_hasher.update(binding.market.bytes());
    page_set_hasher.update(input.settlement_root.epoch().bytes());
    page_set_hasher.update(input.settlement_root.order_set().bytes());
    page_set_hasher.update([page_count]);
    let mut page = 0usize;
    while page < input.pages.len() {
        let header = verify_page_v5(input.pages[page].body)
            .map_err(|_| ExactIndexPlaneErrorV1::FrozenBook)?;
        let expected_page = u16::try_from(page)
            .map_err(|_| ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        if header.page_index != expected_page
            || header.page_count != u16::from(page_count)
            || input.pages[page].account
                != projection
                    .page_account(expected_page)
                    .ok_or(ExactIndexPlaneErrorV1::FrozenBook)?
        {
            return Err(ExactIndexPlaneErrorV1::FrozenBook);
        }
        page_slot_counts[page] = header.order_count;
        page_set_hasher.update(input.pages[page].account.bytes());
        page_set_hasher.update(header.page_digest.bytes());
        page_set_hasher.update(header.page_index.to_le_bytes());
        page_set_hasher.update([header.order_count, header.tombstone_count]);
        page += 1;
    }
    if page != usize::from(page_count) {
        return Err(ExactIndexPlaneErrorV1::FrozenBook);
    }
    let page_set_digest = finish_id(page_set_hasher)?;

    let mut market_binding_bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
    input
        .market_binding
        .encode(&mut market_binding_bytes)
        .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    let market_binding_digest = hash_id(
        EXACT_INDEX_MARKET_BINDING_DIGEST_DOMAIN_V1,
        &[&input.market_binding_account.bytes(), &market_binding_bytes],
    )?;

    let mut locators = [EMPTY_LOCATOR; MAX_ORDERS];
    let mut order = 0usize;
    while order < usize::from(feed.order_count) {
        let order_index = u8::try_from(order)
            .map_err(|_| ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        locators[order] = FrozenOrderLocatorRowV1 {
            page_index: projection
                .order_page_index(order_index)
                .ok_or(ExactIndexPlaneErrorV1::FrozenBook)?,
            page_slot: projection
                .order_page_slot(order_index)
                .ok_or(ExactIndexPlaneErrorV1::FrozenBook)?,
        };
        order += 1;
    }

    let mut aggregates = [EMPTY_AGGREGATE; MAX_ORDERS];
    order = 0;
    while order < usize::from(feed.order_count) {
        let side = match projection.base().book().orders[order].side {
            Side::Buy => ExactIndexOrderSideV1::Buy,
            Side::Sell => ExactIndexOrderSideV1::Sell,
        };
        let order_index = u8::try_from(order)
            .map_err(|_| ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let entitled_quantity = traversal
            .settlement_membership(order_index)
            .map_or(0, |membership| membership.entitled_units);
        aggregates[order] = CandidateOrderAggregateRowV1 {
            side,
            entitled_quantity,
            ..EMPTY_AGGREGATE
        };
        order += 1;
    }
    let mut expected_edge_count = 0u16;
    let mut slice_index = 0u16;
    while slice_index < feed.slice_count {
        let slice = traversal
            .settlement_slice(slice_index)
            .ok_or(ExactIndexPlaneErrorV1::CandidateTraversal)?;
        match (slice.buy(), slice.sell(), slice.route()) {
            (SettlementLegV1::Order(buy), SettlementLegV1::Order(sell), SettlementRouteV1::Direct) => {
                count_order_edge(&mut aggregates, buy, slice.quantity())?;
                count_order_edge(&mut aggregates, sell, slice.quantity())?;
                expected_edge_count = expected_edge_count
                    .checked_add(2)
                    .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
            }
            (SettlementLegV1::Order(buy), SettlementLegV1::Split, SettlementRouteV1::SplitToBuy) => {
                count_order_edge(&mut aggregates, buy, slice.quantity())?;
                expected_edge_count = expected_edge_count
                    .checked_add(1)
                    .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
            }
            (SettlementLegV1::Merge, SettlementLegV1::Order(sell), SettlementRouteV1::SellToMerge) => {
                count_order_edge(&mut aggregates, sell, slice.quantity())?;
                expected_edge_count = expected_edge_count
                    .checked_add(1)
                    .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
            }
            _ => return Err(ExactIndexPlaneErrorV1::CandidateTraversal),
        }
        slice_index = slice_index
            .checked_add(1)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    }
    let mut next_edge = 0u16;
    order = 0;
    while order < usize::from(feed.order_count) {
        aggregates[order].first_edge = next_edge;
        if aggregates[order].total_quantity != aggregates[order].entitled_quantity {
            return Err(ExactIndexPlaneErrorV1::AggregateMismatch);
        }
        next_edge = next_edge
            .checked_add(aggregates[order].edge_count)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        order += 1;
    }
    if next_edge != expected_edge_count {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }

    let mut edges = [EMPTY_EDGE; MAX_EXACT_INDEX_EDGES_V1];
    let mut cursors = [0u16; MAX_ORDERS];
    order = 0;
    while order < usize::from(feed.order_count) {
        cursors[order] = aggregates[order].first_edge;
        order += 1;
    }
    slice_index = 0;
    while slice_index < feed.slice_count {
        let slice = traversal
            .settlement_slice(slice_index)
            .ok_or(ExactIndexPlaneErrorV1::CandidateTraversal)?;
        match (slice.buy(), slice.sell(), slice.route()) {
            (SettlementLegV1::Order(buy), SettlementLegV1::Order(sell), SettlementRouteV1::Direct) => {
                push_order_edge(
                    &mut edges,
                    &mut cursors,
                    buy,
                    CandidateOrderSliceEdgeV1 {
                        slice_index,
                        counterparty_kind: ExactIndexCounterpartyV1::Order,
                        counterparty_order: sell,
                        outcome: slice.outcome(),
                        side: ExactIndexOrderSideV1::Buy,
                        quantity: slice.quantity(),
                    },
                )?;
                push_order_edge(
                    &mut edges,
                    &mut cursors,
                    sell,
                    CandidateOrderSliceEdgeV1 {
                        slice_index,
                        counterparty_kind: ExactIndexCounterpartyV1::Order,
                        counterparty_order: buy,
                        outcome: slice.outcome(),
                        side: ExactIndexOrderSideV1::Sell,
                        quantity: slice.quantity(),
                    },
                )?;
            }
            (SettlementLegV1::Order(buy), SettlementLegV1::Split, SettlementRouteV1::SplitToBuy) => {
                push_order_edge(
                    &mut edges,
                    &mut cursors,
                    buy,
                    CandidateOrderSliceEdgeV1 {
                        slice_index,
                        counterparty_kind: ExactIndexCounterpartyV1::Split,
                        counterparty_order: 0,
                        outcome: slice.outcome(),
                        side: ExactIndexOrderSideV1::Buy,
                        quantity: slice.quantity(),
                    },
                )?;
            }
            (SettlementLegV1::Merge, SettlementLegV1::Order(sell), SettlementRouteV1::SellToMerge) => {
                push_order_edge(
                    &mut edges,
                    &mut cursors,
                    sell,
                    CandidateOrderSliceEdgeV1 {
                        slice_index,
                        counterparty_kind: ExactIndexCounterpartyV1::Merge,
                        counterparty_order: 0,
                        outcome: slice.outcome(),
                        side: ExactIndexOrderSideV1::Sell,
                        quantity: slice.quantity(),
                    },
                )?;
            }
            _ => return Err(ExactIndexPlaneErrorV1::CandidateTraversal),
        }
        slice_index = slice_index
            .checked_add(1)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    }
    derive_aggregate_counterparty_counts(&mut aggregates, &edges, feed.order_count)?;

    let semantic_accounts = [
        binding.market,
        input.settlement_root.epoch(),
        input.settlement_root_account,
        input.selected_feed_account,
        input.market_binding_account,
        binding.neutral_sink,
    ];
    validate_create_pair_identities(
        input.locator_create,
        input.adjacency_create,
        binding.neutral_sink,
    )?;
    let locator_len = locator_encoded_len(feed.order_count)?;
    let adjacency_len = adjacency_encoded_len(feed.order_count, expected_edge_count)?;
    let locator_rent = input
        .locator_create
        .validate(locator_len, &semantic_accounts)?;
    let adjacency_rent = input
        .adjacency_create
        .validate(adjacency_len, &semantic_accounts)?;
    if input.locator_create.payer == input.adjacency_create.payer {
        let combined = input
            .locator_create
            .rent_exempt_minimum
            .checked_add(input.adjacency_create.rent_exempt_minimum)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        if combined > input.locator_create.payer_lamports
            || input.locator_create.payer_lamports != input.adjacency_create.payer_lamports
        {
            return Err(ExactIndexPlaneErrorV1::InvalidRent);
        }
    }

    let owner_order_set_digest = traversal.owner_order_set_digest();
    let plane_id = derive_plane_id(
        input.settlement_root,
        input.settlement_root_account,
        input.market_binding_account,
        market_binding_digest,
        projection.base().economic_domain_digest(),
        page_set_digest,
        realm,
        profile,
        capability_profile,
        genesis_id,
        owner_order_set_digest,
        input.selected_feed_account,
        traversal.candidate_bundle_digest(),
        page_count,
        page_slot_counts,
        feed.order_count,
        feed.outcome_count,
        feed.slice_count,
        expected_edge_count,
        input.locator_create.account,
        input.adjacency_create.account,
        locator_rent,
        adjacency_rent,
        &locators,
        &aggregates,
        &edges,
    )?;
    let base_common = ExactIndexCommonV1 {
        market: binding.market,
        epoch: input.settlement_root.epoch(),
        order_set: input.settlement_root.order_set(),
        settlement_candidate: feed.settlement_candidate_id,
        selected_feed: input.selected_feed_account,
        candidate_bundle_digest: traversal.candidate_bundle_digest(),
        realm,
        profile,
        capability_profile,
        market_genesis_profile: genesis_id,
        market_binding_account: input.market_binding_account,
        market_binding_digest,
        economic_domain_digest: projection.base().economic_domain_digest(),
        page_set_digest,
        plane_id,
        sibling_account: input.adjacency_create.account,
        settlement_root_account: input.settlement_root_account,
        owner_order_set_digest,
        epoch_generation: feed.epoch_generation,
        page_count,
        order_count: feed.order_count,
        outcome_count: feed.outcome_count,
        slice_count: feed.slice_count,
        edge_count: expected_edge_count,
        page_slot_counts,
        rent: locator_rent,
        stored_bump: input.locator_create.stored_bump,
    };
    let locator = FrozenOrderLocatorV1 {
        common: base_common,
        rows: locators,
    };
    let adjacency = CandidateOrderSliceIndexV1 {
        common: ExactIndexCommonV1 {
            sibling_account: input.locator_create.account,
            rent: adjacency_rent,
            stored_bump: input.adjacency_create.stored_bump,
            ..base_common
        },
        aggregates,
        edges,
    };
    validate_exact_index_pair(
        input.locator_create.account,
        &locator,
        input.adjacency_create.account,
        &adjacency,
    )?;
    let locator_data_id = locator_data_id(&locator)?;
    let adjacency_data_id = adjacency_data_id(&adjacency)?;
    let locator_post_lamports = input
        .locator_create
        .target_lamports
        .checked_add(input.locator_create.rent_exempt_minimum)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let adjacency_post_lamports = input
        .adjacency_create
        .target_lamports
        .checked_add(input.adjacency_create.rent_exempt_minimum)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok(ExactIndexPlaneCreatePostwritesV1 {
        locator_account: input.locator_create.account,
        adjacency_account: input.adjacency_create.account,
        locator,
        adjacency,
        locator_data_id,
        adjacency_data_id,
        locator_payer_debit: input.locator_create.rent_exempt_minimum,
        adjacency_payer_debit: input.adjacency_create.rent_exempt_minimum,
        locator_post_lamports,
        adjacency_post_lamports,
    })
}

/// Derive the exact two-child plane and its counted breaking root atomically.
///
/// Unlike the raw child constructor, this function can create its private
/// admission capability because the returned root poststate owns exact
/// expected/admitted/live counts of two. The discriminator/version/PDA are
/// centrally reserved, but no action, dispatch entry, or profile capability
/// consumes this plan today.
pub fn construct_counted_exact_index_root_v1(
    root_rent: IndexedSettlementRootRentPreparationV1,
    input: ConstructExactIndexPlaneInputV1<'_>,
) -> Result<CountedExactIndexRootCreatePostwritesV1, ExactIndexPlaneErrorV1> {
    let root_account = input.settlement_root_account;
    if root_rent.root_account() != root_account
        || root_rent.base_before() != input.settlement_root
        || root_rent.neutral_sink() != input.market_binding.base().neutral_sink
    {
        return Err(ExactIndexPlaneErrorV1::RootBinding);
    }
    let indexes = construct_exact_index_plane_v1(
        CountedExactIndexAdmissionV1 { _private: () },
        input,
    )?;
    validate_root_and_index_rent_funding(&root_rent, &input)?;
    let indexed_root = IndexedSettlementRootV1AccountV1::new_live(
        *root_rent.base_after(),
        indexes.locator_account,
        indexes.adjacency_account,
        indexes.locator.common.plane_id,
        indexes.locator_data_id,
        indexes.adjacency_data_id,
        indexes.locator.common.capability_profile,
    )
    .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    let indexed_root_data_id = indexed_root
        .data_id(&crate::CanonicalSha256, root_account)
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    Ok(CountedExactIndexRootCreatePostwritesV1 {
        indexed_root,
        indexed_root_data_id,
        root_rent,
        indexes,
    })
}

fn validate_root_and_index_rent_funding(
    root_rent: &IndexedSettlementRootRentPreparationV1,
    input: &ConstructExactIndexPlaneInputV1<'_>,
) -> Result<(), ExactIndexPlaneErrorV1> {
    let root_payer = root_rent.rent_after().payer;
    if root_payer == input.locator_create.account
        || root_payer == input.adjacency_create.account
        || input.locator_create.payer == input.settlement_root_account
        || input.adjacency_create.payer == input.settlement_root_account
    {
        return Err(ExactIndexPlaneErrorV1::InvalidRent);
    }
    let mut combined_root_payer_debit = root_rent.payer_debit_lamports();
    for create in [input.locator_create, input.adjacency_create] {
        if create.payer == root_payer {
            if create.payer_lamports != root_rent.payer_balance_before_lamports() {
                return Err(ExactIndexPlaneErrorV1::InvalidRent);
            }
            combined_root_payer_debit = combined_root_payer_debit
                .checked_add(create.rent_exempt_minimum)
                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        }
    }
    if combined_root_payer_debit > root_rent.payer_balance_before_lamports() {
        return Err(ExactIndexPlaneErrorV1::InvalidRent);
    }
    Ok(())
}

/// Validate exact cross-links and shared semantic bindings of a decoded pair.
pub fn validate_exact_index_pair(
    locator_account: Id32,
    locator: &FrozenOrderLocatorV1,
    adjacency_account: Id32,
    adjacency: &CandidateOrderSliceIndexV1,
) -> Result<(), ExactIndexPlaneErrorV1> {
    locator.validate()?;
    adjacency.validate()?;
    if locator_account.is_zero()
        || adjacency_account.is_zero()
        || locator_account == adjacency_account
        || locator.common.sibling_account != adjacency_account
        || adjacency.common.sibling_account != locator_account
        || !locator.common.semantic_eq(&adjacency.common)
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    Ok(())
}

/// Produce exact pair coverage without rereading pages or scanning unrelated slices.
pub fn indexed_pair_coverage_v1(
    locator: &FrozenOrderLocatorV1,
    adjacency: &CandidateOrderSliceIndexV1,
    buy_order: u8,
    sell_order: u8,
) -> Result<IndexedPairCoverageV1, ExactIndexPlaneErrorV1> {
    locator.validate()?;
    adjacency.validate()?;
    if !locator.common.semantic_eq(&adjacency.common)
        || buy_order >= adjacency.common.order_count
        || sell_order >= adjacency.common.order_count
        || buy_order == sell_order
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let buy = adjacency.aggregates[usize::from(buy_order)];
    let sell = adjacency.aggregates[usize::from(sell_order)];
    if buy.side != ExactIndexOrderSideV1::Buy || sell.side != ExactIndexOrderSideV1::Sell {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    let mut pair_slice_indices = [0u16; MAX_OUTCOMES];
    let mut pair_count = 0usize;
    let buy_end = buy
        .first_edge
        .checked_add(buy.edge_count)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let mut cursor = buy.first_edge;
    while cursor < buy_end {
        let edge = adjacency.edges[usize::from(cursor)];
        if edge.counterparty_kind == ExactIndexCounterpartyV1::Order
            && edge.counterparty_order == sell_order
        {
            if pair_count >= usize::from(adjacency.common.outcome_count)
                || pair_count >= MAX_OUTCOMES
            {
                return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
            }
            pair_slice_indices[pair_count] = edge.slice_index;
            pair_count += 1;
        }
        cursor = cursor
            .checked_add(1)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    }
    if pair_count == 0 {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    let pair_slice_count = u8::try_from(pair_count)
        .map_err(|_| ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok(IndexedPairCoverageV1 {
        pair_slice_indices,
        pair_slice_count,
        buy_total: buy.total_quantity,
        sell_total: sell.total_quantity,
        buy_elsewhere: buy.virtual_edge_count != 0 || buy.distinct_real_counterparties != 1,
        sell_elsewhere: sell.virtual_edge_count != 0 || sell.distinct_real_counterparties != 1,
    })
}

/// Read only two locator rows and two grouped adjacency ranges from sealed PDAs.
///
/// Full account validation belongs to one-time construction. This read still
/// hostile-decodes both constant headers and every selected local edge, but it
/// does not walk unrelated page locations, order directories, or slice edges.
/// The unforgeable authority represents the adapter projection's proof that
/// both immutable accounts are program-owned canonical PDAs named by the exact
/// counted root successor and match its full body IDs.
pub fn indexed_pair_coverage_from_sealed_accounts_v1(
    input: SealedExactIndexPairInputV1<'_>,
    buy_order: u8,
    sell_order: u8,
) -> Result<IndexedPairCoverageV1, ExactIndexPlaneErrorV1> {
    if input.locator_body.len() < EXACT_INDEX_COMMON_HEADER_BYTES_V1
        || input.adjacency_body.len() < EXACT_INDEX_COMMON_HEADER_BYTES_V1
    {
        return Err(ExactIndexPlaneErrorV1::WrongLength);
    }
    let mut locator_reader = ExactReader::new(input.locator_body);
    let locator_common = decode_common(&mut locator_reader, FROZEN_ORDER_LOCATOR_MAGIC_V1)?;
    let mut adjacency_reader = ExactReader::new(input.adjacency_body);
    let adjacency_common = decode_common(
        &mut adjacency_reader,
        CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1,
    )?;
    if input.locator_body.len() != locator_encoded_len(locator_common.order_count)?
        || input.adjacency_body.len()
            != adjacency_encoded_len(adjacency_common.order_count, adjacency_common.edge_count)?
        || !locator_common.semantic_eq(&adjacency_common)
        || locator_common.plane_id != input.authority.plane_id
        || locator_common.sibling_account != input.authority.adjacency_account
        || adjacency_common.sibling_account != input.authority.locator_account
        || buy_order >= locator_common.order_count
        || sell_order >= locator_common.order_count
        || buy_order == sell_order
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    read_one_locator(input.locator_body, &locator_common, buy_order)?;
    read_one_locator(input.locator_body, &locator_common, sell_order)?;
    let buy = read_one_aggregate(input.adjacency_body, &adjacency_common, buy_order)?;
    let sell = read_one_aggregate(input.adjacency_body, &adjacency_common, sell_order)?;
    if buy.side != ExactIndexOrderSideV1::Buy || sell.side != ExactIndexOrderSideV1::Sell {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    let mut pair_slice_indices = [0u16; MAX_OUTCOMES];
    let mut pair_outcomes = [0u8; MAX_OUTCOMES];
    let mut pair_quantities = [0u64; MAX_OUTCOMES];
    let (buy_pair_count, buy_elsewhere) = scan_local_edge_group(
        input.adjacency_body,
        &adjacency_common,
        buy_order,
        sell_order,
        buy,
        &mut pair_slice_indices,
        &mut pair_outcomes,
        &mut pair_quantities,
    )?;
    if buy_pair_count == 0 {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    let mut sell_pair_slices = [0u16; MAX_OUTCOMES];
    let mut sell_pair_outcomes = [0u8; MAX_OUTCOMES];
    let mut sell_pair_quantities = [0u64; MAX_OUTCOMES];
    let (sell_pair_count, sell_elsewhere) = scan_local_edge_group(
        input.adjacency_body,
        &adjacency_common,
        sell_order,
        buy_order,
        sell,
        &mut sell_pair_slices,
        &mut sell_pair_outcomes,
        &mut sell_pair_quantities,
    )?;
    if buy_pair_count != sell_pair_count {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    let mut pair = 0usize;
    while pair < usize::from(buy_pair_count) {
        if pair_slice_indices[pair] != sell_pair_slices[pair]
            || pair_outcomes[pair] != sell_pair_outcomes[pair]
            || pair_quantities[pair] != sell_pair_quantities[pair]
        {
            return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
        }
        pair += 1;
    }
    Ok(IndexedPairCoverageV1 {
        pair_slice_indices,
        pair_slice_count: buy_pair_count,
        buy_total: buy.total_quantity,
        sell_total: sell.total_quantity,
        buy_elsewhere,
        sell_elsewhere,
    })
}

fn read_one_locator(
    body: &[u8],
    common: &ExactIndexCommonV1,
    order: u8,
) -> Result<FrozenOrderLocatorRowV1, ExactIndexPlaneErrorV1> {
    let relative = usize::from(order)
        .checked_mul(FROZEN_ORDER_LOCATOR_ROW_BYTES_V1)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let at = EXACT_INDEX_COMMON_HEADER_BYTES_V1
        .checked_add(relative)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let mut reader = ExactReader { input: body, at };
    let row = FrozenOrderLocatorRowV1 {
        page_index: reader.u16()?,
        page_slot: reader.u8()?,
    };
    reader.reserved(1)?;
    if usize::from(row.page_index) >= usize::from(common.page_count)
        || row.page_slot >= common.page_slot_counts[usize::from(row.page_index)]
    {
        return Err(ExactIndexPlaneErrorV1::InvalidLocator);
    }
    Ok(row)
}

fn read_one_aggregate(
    body: &[u8],
    common: &ExactIndexCommonV1,
    order: u8,
) -> Result<CandidateOrderAggregateRowV1, ExactIndexPlaneErrorV1> {
    let relative = usize::from(order)
        .checked_mul(CANDIDATE_ORDER_AGGREGATE_ROW_BYTES_V1)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let at = EXACT_INDEX_COMMON_HEADER_BYTES_V1
        .checked_add(relative)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let mut reader = ExactReader { input: body, at };
    let row = decode_aggregate(&mut reader)?;
    let end = row
        .first_edge
        .checked_add(row.edge_count)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    if end > common.edge_count || row.entitled_quantity != row.total_quantity {
        return Err(ExactIndexPlaneErrorV1::AggregateMismatch);
    }
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
fn scan_local_edge_group(
    body: &[u8],
    common: &ExactIndexCommonV1,
    owner_order: u8,
    selected_counterparty: u8,
    aggregate: CandidateOrderAggregateRowV1,
    pair_slices: &mut [u16; MAX_OUTCOMES],
    pair_outcomes: &mut [u8; MAX_OUTCOMES],
    pair_quantities: &mut [u64; MAX_OUTCOMES],
) -> Result<(u8, bool), ExactIndexPlaneErrorV1> {
    let aggregate_tail = usize::from(common.order_count)
        .checked_mul(CANDIDATE_ORDER_AGGREGATE_ROW_BYTES_V1)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let edges_at = EXACT_INDEX_COMMON_HEADER_BYTES_V1
        .checked_add(aggregate_tail)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let end = aggregate
        .first_edge
        .checked_add(aggregate.edge_count)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let mut cursor = aggregate.first_edge;
    let mut prior_slice = None;
    let mut total = 0u64;
    let mut pair_count = 0usize;
    let mut elsewhere = false;
    let mut distinct_seen = [false; MAX_ORDERS];
    let mut distinct_real = 0u16;
    let mut virtual_count = 0u16;
    while cursor < end {
        let relative = usize::from(cursor)
            .checked_mul(CANDIDATE_ORDER_SLICE_EDGE_BYTES_V1)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let at = edges_at
            .checked_add(relative)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let mut reader = ExactReader { input: body, at };
        let edge = decode_edge(&mut reader)?;
        if edge.quantity == 0
            || edge.side != aggregate.side
            || edge.outcome >= common.outcome_count
            || edge.slice_index >= common.slice_count
            || prior_slice.is_some_and(|prior| edge.slice_index <= prior)
        {
            return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
        }
        match (aggregate.side, edge.counterparty_kind) {
            (ExactIndexOrderSideV1::Buy, ExactIndexCounterpartyV1::Order)
            | (ExactIndexOrderSideV1::Sell, ExactIndexCounterpartyV1::Order) => {
                if edge.counterparty_order >= common.order_count
                    || edge.counterparty_order == owner_order
                {
                    return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
                }
                let counterparty = usize::from(edge.counterparty_order);
                if !distinct_seen[counterparty] {
                    distinct_seen[counterparty] = true;
                    distinct_real = distinct_real
                        .checked_add(1)
                        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                }
                if edge.counterparty_order == selected_counterparty {
                    if pair_count >= usize::from(common.outcome_count)
                        || pair_count >= MAX_OUTCOMES
                    {
                        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
                    }
                    pair_slices[pair_count] = edge.slice_index;
                    pair_outcomes[pair_count] = edge.outcome;
                    pair_quantities[pair_count] = edge.quantity;
                    pair_count += 1;
                } else {
                    elsewhere = true;
                }
            }
            (ExactIndexOrderSideV1::Buy, ExactIndexCounterpartyV1::Split)
            | (ExactIndexOrderSideV1::Sell, ExactIndexCounterpartyV1::Merge) => {
                if edge.counterparty_order != 0 {
                    return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
                }
                virtual_count = virtual_count
                    .checked_add(1)
                    .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                elsewhere = true;
            }
            _ => return Err(ExactIndexPlaneErrorV1::InvalidAdjacency),
        }
        total = total
            .checked_add(edge.quantity)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        prior_slice = Some(edge.slice_index);
        cursor = cursor
            .checked_add(1)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    }
    if total != aggregate.total_quantity
        || distinct_real != aggregate.distinct_real_counterparties
        || virtual_count != aggregate.virtual_edge_count
    {
        return Err(ExactIndexPlaneErrorV1::AggregateMismatch);
    }
    let pair_count = u8::try_from(pair_count)
        .map_err(|_| ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok((pair_count, elsewhere))
}

fn count_order_edge(
    aggregates: &mut [CandidateOrderAggregateRowV1; MAX_ORDERS],
    order: u8,
    quantity: u64,
) -> Result<(), ExactIndexPlaneErrorV1> {
    let row = aggregates
        .get_mut(usize::from(order))
        .ok_or(ExactIndexPlaneErrorV1::InvalidAdjacency)?;
    if quantity == 0 {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    row.edge_count = row
        .edge_count
        .checked_add(1)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    row.total_quantity = row
        .total_quantity
        .checked_add(quantity)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok(())
}

fn push_order_edge(
    edges: &mut [CandidateOrderSliceEdgeV1; MAX_EXACT_INDEX_EDGES_V1],
    cursors: &mut [u16; MAX_ORDERS],
    order: u8,
    edge: CandidateOrderSliceEdgeV1,
) -> Result<(), ExactIndexPlaneErrorV1> {
    let cursor = cursors
        .get_mut(usize::from(order))
        .ok_or(ExactIndexPlaneErrorV1::InvalidAdjacency)?;
    let target = edges
        .get_mut(usize::from(*cursor))
        .ok_or(ExactIndexPlaneErrorV1::InvalidAdjacency)?;
    if *target != EMPTY_EDGE {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    *target = edge;
    *cursor = cursor
        .checked_add(1)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok(())
}

fn derive_aggregate_counterparty_counts(
    aggregates: &mut [CandidateOrderAggregateRowV1; MAX_ORDERS],
    edges: &[CandidateOrderSliceEdgeV1; MAX_EXACT_INDEX_EDGES_V1],
    order_count: u8,
) -> Result<(), ExactIndexPlaneErrorV1> {
    let mut order = 0usize;
    while order < usize::from(order_count) {
        let row = aggregates[order];
        let end = row
            .first_edge
            .checked_add(row.edge_count)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let mut cursor = row.first_edge;
        let mut distinct_real = 0u16;
        let mut virtual_count = 0u16;
        while cursor < end {
            let edge = edges[usize::from(cursor)];
            if edge.counterparty_kind == ExactIndexCounterpartyV1::Order {
                let mut prior = row.first_edge;
                let mut first = true;
                while prior < cursor {
                    let observed = edges[usize::from(prior)];
                    if observed.counterparty_kind == ExactIndexCounterpartyV1::Order
                        && observed.counterparty_order == edge.counterparty_order
                    {
                        first = false;
                    }
                    prior = prior
                        .checked_add(1)
                        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                }
                if first {
                    distinct_real = distinct_real
                        .checked_add(1)
                        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
                }
            } else {
                virtual_count = virtual_count
                    .checked_add(1)
                    .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
            }
            cursor = cursor
                .checked_add(1)
                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        }
        aggregates[order].distinct_real_counterparties = distinct_real;
        aggregates[order].virtual_edge_count = virtual_count;
        order += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn derive_plane_id(
    root: &SettlementRootV1AccountV1,
    root_account: Id32,
    market_binding_account: Id32,
    market_binding_digest: Id32,
    economic_domain_digest: Id32,
    page_set_digest: Id32,
    realm: Id32,
    profile: Id32,
    capability_profile: Id32,
    market_genesis_profile: Id32,
    owner_order_set_digest: Id32,
    selected_feed: Id32,
    candidate_bundle_digest: Id32,
    page_count: u8,
    page_slot_counts: [u8; MAX_ORDER_PAGES],
    order_count: u8,
    outcome_count: u8,
    slice_count: u16,
    edge_count: u16,
    locator_account: Id32,
    adjacency_account: Id32,
    locator_rent: DeletableRentOwnerV1,
    adjacency_rent: DeletableRentOwnerV1,
    locators: &[FrozenOrderLocatorRowV1; MAX_ORDERS],
    aggregates: &[CandidateOrderAggregateRowV1; MAX_ORDERS],
    edges: &[CandidateOrderSliceEdgeV1; MAX_EXACT_INDEX_EDGES_V1],
) -> Result<Id32, ExactIndexPlaneErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(EXACT_INDEX_PLANE_ID_DOMAIN_V1);
    for id in [
        root.market(),
        root.epoch(),
        root.order_set(),
        root.settlement_candidate_id(),
        selected_feed,
        candidate_bundle_digest,
        realm,
        profile,
        capability_profile,
        market_genesis_profile,
        market_binding_account,
        market_binding_digest,
        economic_domain_digest,
        page_set_digest,
        root_account,
        owner_order_set_digest,
        locator_account,
        adjacency_account,
    ] {
        hasher.update(id.bytes());
    }
    hasher.update(root.epoch_generation().to_le_bytes());
    hasher.update([page_count, order_count, outcome_count]);
    hasher.update(slice_count.to_le_bytes());
    hasher.update(edge_count.to_le_bytes());
    hasher.update(page_slot_counts);
    update_rent_hash(&mut hasher, locator_rent);
    update_rent_hash(&mut hasher, adjacency_rent);
    let mut order = 0usize;
    while order < usize::from(order_count) {
        update_locator_hash(&mut hasher, locators[order]);
        update_aggregate_hash(&mut hasher, aggregates[order]);
        order += 1;
    }
    let mut edge = 0usize;
    while edge < usize::from(edge_count) {
        update_edge_hash(&mut hasher, edges[edge]);
        edge += 1;
    }
    finish_id(hasher)
}

fn locator_data_id(locator: &FrozenOrderLocatorV1) -> Result<Id32, ExactIndexPlaneErrorV1> {
    locator.validate()?;
    let mut hasher = Sha256::new();
    hasher.update(FROZEN_ORDER_LOCATOR_DATA_ID_DOMAIN_V1);
    update_common_hash(&mut hasher, FROZEN_ORDER_LOCATOR_MAGIC_V1, &locator.common);
    let mut order = 0usize;
    while order < usize::from(locator.common.order_count) {
        update_locator_hash(&mut hasher, locator.rows[order]);
        order += 1;
    }
    finish_id(hasher)
}

fn adjacency_data_id(
    adjacency: &CandidateOrderSliceIndexV1,
) -> Result<Id32, ExactIndexPlaneErrorV1> {
    adjacency.validate()?;
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_ORDER_SLICE_INDEX_DATA_ID_DOMAIN_V1);
    update_common_hash(
        &mut hasher,
        CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1,
        &adjacency.common,
    );
    let mut order = 0usize;
    while order < usize::from(adjacency.common.order_count) {
        update_aggregate_hash(&mut hasher, adjacency.aggregates[order]);
        order += 1;
    }
    let mut edge = 0usize;
    while edge < usize::from(adjacency.common.edge_count) {
        update_edge_hash(&mut hasher, adjacency.edges[edge]);
        edge += 1;
    }
    finish_id(hasher)
}

fn update_common_hash(hasher: &mut Sha256, magic: [u8; 8], common: &ExactIndexCommonV1) {
    hasher.update(magic);
    hasher.update([
        EXACT_INDEX_PLANE_VERSION_V1,
        EXACT_INDEX_PLANE_STATE_SEALED_V1,
        common.stored_bump,
        0,
    ]);
    hasher.update([0; 4]);
    for id in common_ids(common) {
        hasher.update(id.bytes());
    }
    hasher.update(common.epoch_generation.to_le_bytes());
    hasher.update([common.page_count, common.order_count]);
    hasher.update(common.slice_count.to_le_bytes());
    hasher.update(common.edge_count.to_le_bytes());
    hasher.update([common.outcome_count, 0]);
    hasher.update(common.page_slot_counts);
    hasher.update([0; 4]);
    update_rent_hash(hasher, common.rent);
}

fn update_rent_hash(hasher: &mut Sha256, rent: DeletableRentOwnerV1) {
    hasher.update(rent.payer.bytes());
    hasher.update(rent.refundable_principal.to_le_bytes());
    hasher.update(rent.donation_floor.to_le_bytes());
}

fn update_locator_hash(hasher: &mut Sha256, row: FrozenOrderLocatorRowV1) {
    hasher.update(row.page_index.to_le_bytes());
    hasher.update([row.page_slot, 0]);
}

fn update_aggregate_hash(hasher: &mut Sha256, row: CandidateOrderAggregateRowV1) {
    hasher.update(row.first_edge.to_le_bytes());
    hasher.update(row.edge_count.to_le_bytes());
    hasher.update(row.distinct_real_counterparties.to_le_bytes());
    hasher.update(row.virtual_edge_count.to_le_bytes());
    hasher.update(row.total_quantity.to_le_bytes());
    hasher.update(row.entitled_quantity.to_le_bytes());
    hasher.update([row.side.code(), 1]);
    hasher.update([0; 6]);
}

fn update_edge_hash(hasher: &mut Sha256, edge: CandidateOrderSliceEdgeV1) {
    hasher.update(edge.slice_index.to_le_bytes());
    hasher.update([
        edge.counterparty_kind.code(),
        edge.counterparty_order,
        edge.outcome,
        edge.side.code(),
    ]);
    hasher.update([0; 2]);
    hasher.update(edge.quantity.to_le_bytes());
}

fn hash_id(domain: &[u8], parts: &[&[u8]]) -> Result<Id32, ExactIndexPlaneErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    let mut index = 0usize;
    while index < parts.len() {
        hasher.update(parts[index]);
        index += 1;
    }
    finish_id(hasher)
}

fn finish_id(hasher: Sha256) -> Result<Id32, ExactIndexPlaneErrorV1> {
    let digest: [u8; 32] = hasher.finalize().into();
    Id32::new(digest).map_err(|_| ExactIndexPlaneErrorV1::ZeroIdentity)
}

/// One adapter-authenticated live index account presented for atomic closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexCloseAccountInputV1<'a> {
    /// Exact index PDA account.
    pub account: Id32,
    /// Exact active account body.
    pub body: &'a [u8],
    /// Current complete lamport balance.
    pub lamports: u64,
    /// Current account owner, which must equal `program_id`.
    pub owner: Id32,
    /// Expected Dragon's Clutch program owner.
    pub program_id: Id32,
    /// Runtime writable bit.
    pub writable: bool,
    /// Runtime executable bit.
    pub executable: bool,
}

/// Complete terminal-root input for atomic retirement of both index siblings.
#[derive(Clone, Copy, Debug)]
pub struct CloseExactIndexPlaneInputV1<'a> {
    /// Counted terminal SettlementRoot account identity.
    pub settlement_root_account: Id32,
    /// Exact hostile-decoded terminal SettlementRoot body.
    pub settlement_root: &'a SettlementRootV1AccountV1,
    /// Immutable MarketBinding V2 account identity.
    pub market_binding_account: Id32,
    /// Exact hostile-decoded MarketBinding V2 body, used to recover the one
    /// canonical neutral sink for all nonprincipal lamports.
    pub market_binding: &'a MarketBindingV2,
    /// Locator sibling and balance.
    pub locator: ExactIndexCloseAccountInputV1<'a>,
    /// Adjacency sibling and balance.
    pub adjacency: ExactIndexCloseAccountInputV1<'a>,
}

/// One exact balance credit in the private terminal close postwrites.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexCloseCreditV1 {
    recipient: Id32,
    amount: u64,
}

impl ExactIndexCloseCreditV1 {
    /// Exact authenticated principal owner or immutable neutral sink.
    pub const fn recipient(self) -> Id32 {
        self.recipient
    }

    /// Exact lamports credited atomically with zeroing both siblings.
    pub const fn amount(self) -> u64 {
        self.amount
    }
}

/// Private typed postwrites for atomic terminal retirement of the index pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexPlaneClosePostwritesV1 {
    locator_account: Id32,
    adjacency_account: Id32,
    locator_principal: ExactIndexCloseCreditV1,
    locator_donation: ExactIndexCloseCreditV1,
    adjacency_principal: ExactIndexCloseCreditV1,
    adjacency_donation: ExactIndexCloseCreditV1,
    plane_id: Id32,
}

/// Atomic close postwrites plus counted indexed-root retirement poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactIndexRootRetirementPostwritesV1 {
    indexed_root_poststate: IndexedSettlementRootV1AccountV1,
    indexed_root_poststate_data_id: Id32,
    close: ExactIndexPlaneClosePostwritesV1,
}

impl CountedExactIndexRootRetirementPostwritesV1 {
    /// Root poststate with admitted two, live zero, retired two.
    pub const fn indexed_root_poststate(&self) -> &IndexedSettlementRootV1AccountV1 {
        &self.indexed_root_poststate
    }

    /// Account-key-bound exact retired indexed-root identity.
    pub const fn indexed_root_poststate_data_id(&self) -> Id32 {
        self.indexed_root_poststate_data_id
    }

    /// Exact atomic sibling close and rent-credit postwrites.
    pub const fn close_postwrites(&self) -> &ExactIndexPlaneClosePostwritesV1 {
        &self.close
    }
}

impl ExactIndexPlaneClosePostwritesV1 {
    /// Locator account whose lamports and data become zero.
    pub const fn locator_account(&self) -> Id32 {
        self.locator_account
    }

    /// Adjacency account whose lamports and data become zero.
    pub const fn adjacency_account(&self) -> Id32 {
        self.adjacency_account
    }

    /// Locator refundable-principal credit.
    pub const fn locator_principal_credit(&self) -> ExactIndexCloseCreditV1 {
        self.locator_principal
    }

    /// Locator donation/excess credit to the immutable neutral sink.
    pub const fn locator_donation_credit(&self) -> ExactIndexCloseCreditV1 {
        self.locator_donation
    }

    /// Adjacency refundable-principal credit.
    pub const fn adjacency_principal_credit(&self) -> ExactIndexCloseCreditV1 {
        self.adjacency_principal
    }

    /// Adjacency donation/excess credit to the immutable neutral sink.
    pub const fn adjacency_donation_credit(&self) -> ExactIndexCloseCreditV1 {
        self.adjacency_donation
    }

    /// Shared retired index-plane identity.
    pub const fn plane_id(&self) -> Id32 {
        self.plane_id
    }
}

/// Prepare an atomic close only after the counted root proves exhaustive terminality.
pub fn close_exact_index_plane_v1(
    counted_root_retirement: CountedExactIndexRetirementV1,
    input: CloseExactIndexPlaneInputV1<'_>,
) -> Result<ExactIndexPlaneClosePostwritesV1, ExactIndexPlaneErrorV1> {
    let _counted_root_retirement = counted_root_retirement;
    input
        .settlement_root
        .validate()
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    input
        .market_binding
        .validate()
        .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    if input.settlement_root.phase() != SettlementRootPhaseV1::Terminal {
        return Err(ExactIndexPlaneErrorV1::NonTerminalRoot);
    }
    if input.locator.account.is_zero()
        || input.adjacency.account.is_zero()
        || input.locator.account == input.adjacency.account
        || input.locator.program_id.is_zero()
        || input.locator.program_id != input.adjacency.program_id
        || input.locator.owner != input.locator.program_id
        || input.adjacency.owner != input.adjacency.program_id
        || !input.locator.writable
        || !input.adjacency.writable
        || input.locator.executable
        || input.adjacency.executable
        || input.market_binding_account != input.settlement_root.market_binding()
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let locator = FrozenOrderLocatorV1::decode(input.locator.body)?;
    let adjacency = CandidateOrderSliceIndexV1::decode(input.adjacency.body)?;
    validate_exact_index_pair(
        input.locator.account,
        &locator,
        input.adjacency.account,
        &adjacency,
    )?;
    let binding_digest = {
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
        input
            .market_binding
            .encode(&mut bytes)
            .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
        hash_id(
            EXACT_INDEX_MARKET_BINDING_DIGEST_DOMAIN_V1,
            &[&input.market_binding_account.bytes(), &bytes],
        )?
    };
    let common = locator.common;
    let counts = input.settlement_root.counts();
    if common.settlement_root_account != input.settlement_root_account
        || common.market != input.settlement_root.market()
        || common.epoch != input.settlement_root.epoch()
        || common.order_set != input.settlement_root.order_set()
        || common.settlement_candidate != input.settlement_root.settlement_candidate_id()
        || common.selected_feed != input.settlement_root.retained_feed()
        || common.candidate_bundle_digest != input.settlement_root.candidate_bundle_digest()
        || common.owner_order_set_digest != input.settlement_root.owner_order_set_digest()
        || common.market_binding_account != input.market_binding_account
        || common.market_binding_digest != binding_digest
        || common.epoch_generation != input.settlement_root.epoch_generation()
        || common.order_count != input.settlement_root.order_count()
        || common.outcome_count != input.settlement_root.outcome_count()
        || common.slice_count != counts.expected_receipts
    {
        return Err(ExactIndexPlaneErrorV1::RootBinding);
    }
    let neutral_sink = input.market_binding.base().neutral_sink;
    let (locator_principal, locator_donation) = close_credits(
        common.rent,
        input.locator.lamports,
        neutral_sink,
    )?;
    let (adjacency_principal, adjacency_donation) = close_credits(
        adjacency.common.rent,
        input.adjacency.lamports,
        neutral_sink,
    )?;
    Ok(ExactIndexPlaneClosePostwritesV1 {
        locator_account: input.locator.account,
        adjacency_account: input.adjacency.account,
        locator_principal,
        locator_donation,
        adjacency_principal,
        adjacency_donation,
        plane_id: common.plane_id,
    })
}

/// Atomically close both exact children and advance their counted root partition.
pub fn retire_counted_exact_index_root_v1(
    indexed_root: &IndexedSettlementRootV1AccountV1,
    input: CloseExactIndexPlaneInputV1<'_>,
) -> Result<CountedExactIndexRootRetirementPostwritesV1, ExactIndexPlaneErrorV1> {
    indexed_root
        .validate()
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    if indexed_root.index_state() != ExactIndexChildrenStateV1::Live
        || indexed_root.base() != input.settlement_root
        || indexed_root.locator_account() != input.locator.account
        || indexed_root.adjacency_account() != input.adjacency.account
        || input.settlement_root_account.is_zero()
    {
        return Err(ExactIndexPlaneErrorV1::RootBinding);
    }
    let locator = FrozenOrderLocatorV1::decode(input.locator.body)?;
    let adjacency = CandidateOrderSliceIndexV1::decode(input.adjacency.body)?;
    if locator.plane_id() != indexed_root.plane_id()
        || locator_data_id(&locator)? != indexed_root.locator_data_id()
        || adjacency_data_id(&adjacency)? != indexed_root.adjacency_data_id()
        || locator.common.capability_profile != indexed_root.capability_profile_id()
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let root_account = input.settlement_root_account;
    let close = close_exact_index_plane_v1(
        CountedExactIndexRetirementV1 { _private: () },
        input,
    )?;
    let indexed_root_poststate = indexed_root
        .retire_index_children()
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    let indexed_root_poststate_data_id = indexed_root_poststate
        .data_id(&crate::CanonicalSha256, root_account)
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    Ok(CountedExactIndexRootRetirementPostwritesV1 {
        indexed_root_poststate,
        indexed_root_poststate_data_id,
        close,
    })
}

/// Authenticate the reserved indexed root and both complete sealed child bodies.
///
/// The SBF adapter must independently derive the three canonical PDAs and
/// bumps before calling this pure join. This function then hostile-decodes all
/// three complete bodies and verifies the root-held full locator and adjacency
/// data IDs before minting the bounded local-row read capability. The returned
/// reader still touches only the requested locator rows and edge groups.
pub fn authenticate_counted_exact_index_read_v1<'a>(
    input: AuthenticateCountedExactIndexReadInputV1<'a>,
) -> Result<SealedExactIndexPairInputV1<'a>, ExactIndexPlaneErrorV1> {
    if input.program_id.is_zero() {
        return Err(ExactIndexPlaneErrorV1::ZeroIdentity);
    }
    for account in [input.root, input.locator, input.adjacency] {
        if account.account.is_zero()
            || account.owner != input.program_id
            || account.account != account.canonical_account
            || account.writable
            || account.executable
        {
            return Err(ExactIndexPlaneErrorV1::BindingMismatch);
        }
    }
    if input.root.account == input.locator.account
        || input.root.account == input.adjacency.account
        || input.locator.account == input.adjacency.account
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let root = IndexedSettlementRootV1AccountV1::decode(input.root.body)
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    if root.index_state() != ExactIndexChildrenStateV1::Live
        || root.base().stored_bump() != input.root.canonical_bump
        || root.locator_account() != input.locator.account
        || root.adjacency_account() != input.adjacency.account
    {
        return Err(ExactIndexPlaneErrorV1::RootBinding);
    }
    let locator = FrozenOrderLocatorV1::decode(input.locator.body)?;
    let adjacency = CandidateOrderSliceIndexV1::decode(input.adjacency.body)?;
    validate_exact_index_pair(
        input.locator.account,
        &locator,
        input.adjacency.account,
        &adjacency,
    )?;
    if locator.common.stored_bump != input.locator.canonical_bump
        || adjacency.common.stored_bump != input.adjacency.canonical_bump
        || locator_data_id(&locator)? != root.locator_data_id()
        || adjacency_data_id(&adjacency)? != root.adjacency_data_id()
        || locator.common.plane_id != root.plane_id()
        || locator.common.capability_profile != root.capability_profile_id()
        || locator.common.settlement_root_account != input.root.account
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let base = root.base();
    if locator.common.market != base.market()
        || locator.common.epoch != base.epoch()
        || locator.common.order_set != base.order_set()
        || locator.common.settlement_candidate != base.settlement_candidate_id()
        || locator.common.selected_feed != base.retained_feed()
        || locator.common.candidate_bundle_digest != base.candidate_bundle_digest()
        || locator.common.owner_order_set_digest != base.owner_order_set_digest()
        || locator.common.market_binding_account != base.market_binding()
        || locator.common.epoch_generation != base.epoch_generation()
        || locator.common.order_count != base.order_count()
        || locator.common.outcome_count != base.outcome_count()
        || locator.common.slice_count != base.counts().expected_receipts
    {
        return Err(ExactIndexPlaneErrorV1::RootBinding);
    }
    Ok(SealedExactIndexPairInputV1 {
        authority: CountedExactIndexReadAuthorityV1 {
            plane_id: root.plane_id(),
            locator_account: input.locator.account,
            adjacency_account: input.adjacency.account,
            _private: (),
        },
        locator_body: input.locator.body,
        adjacency_body: input.adjacency.body,
    })
}

fn close_credits(
    rent: DeletableRentOwnerV1,
    lamports: u64,
    neutral_sink: Id32,
) -> Result<(ExactIndexCloseCreditV1, ExactIndexCloseCreditV1), ExactIndexPlaneErrorV1> {
    rent.validate()
        .map_err(|_| ExactIndexPlaneErrorV1::InvalidRent)?;
    if neutral_sink.is_zero() || neutral_sink == rent.payer {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let minimum = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    if lamports < minimum {
        return Err(ExactIndexPlaneErrorV1::InvalidRent);
    }
    let donation = lamports
        .checked_sub(rent.refundable_principal)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok((
        ExactIndexCloseCreditV1 {
            recipient: rent.payer,
            amount: rent.refundable_principal,
        },
        ExactIndexCloseCreditV1 {
            recipient: neutral_sink,
            amount: donation,
        },
    ))
}

fn locator_encoded_len(order_count: u8) -> Result<usize, ExactIndexPlaneErrorV1> {
    if order_count == 0 || usize::from(order_count) > MAX_ORDERS {
        return Err(ExactIndexPlaneErrorV1::InvalidCount);
    }
    EXACT_INDEX_COMMON_HEADER_BYTES_V1
        .checked_add(
            usize::from(order_count)
                .checked_mul(FROZEN_ORDER_LOCATOR_ROW_BYTES_V1)
                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?,
        )
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)
}

fn adjacency_encoded_len(
    order_count: u8,
    edge_count: u16,
) -> Result<usize, ExactIndexPlaneErrorV1> {
    if order_count == 0
        || usize::from(order_count) > MAX_ORDERS
        || usize::from(edge_count) > MAX_EXACT_INDEX_EDGES_V1
    {
        return Err(ExactIndexPlaneErrorV1::InvalidCount);
    }
    let aggregate_bytes = usize::from(order_count)
        .checked_mul(CANDIDATE_ORDER_AGGREGATE_ROW_BYTES_V1)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let edge_bytes = usize::from(edge_count)
        .checked_mul(CANDIDATE_ORDER_SLICE_EDGE_BYTES_V1)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    EXACT_INDEX_COMMON_HEADER_BYTES_V1
        .checked_add(aggregate_bytes)
        .and_then(|value| value.checked_add(edge_bytes))
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)
}

fn encode_common(
    writer: &mut ExactWriter<'_>,
    magic: [u8; 8],
    common: &ExactIndexCommonV1,
) -> Result<(), ExactIndexPlaneErrorV1> {
    common.validate()?;
    writer.bytes(&magic)?;
    writer.u8(EXACT_INDEX_PLANE_VERSION_V1)?;
    writer.u8(EXACT_INDEX_PLANE_STATE_SEALED_V1)?;
    writer.u8(common.stored_bump)?;
    writer.u8(0)?;
    writer.bytes(&[0; 4])?;
    for id in common_ids(common) {
        writer.id(id)?;
    }
    writer.u64(common.epoch_generation)?;
    writer.u8(common.page_count)?;
    writer.u8(common.order_count)?;
    writer.u16(common.slice_count)?;
    writer.u16(common.edge_count)?;
    writer.u8(common.outcome_count)?;
    writer.u8(0)?;
    writer.bytes(&common.page_slot_counts)?;
    writer.bytes(&[0; 4])?;
    writer.id(common.rent.payer)?;
    writer.u64(common.rent.refundable_principal)?;
    writer.u64(common.rent.donation_floor)
}

fn decode_common(
    reader: &mut ExactReader<'_>,
    expected_magic: [u8; 8],
) -> Result<ExactIndexCommonV1, ExactIndexPlaneErrorV1> {
    if reader.array::<8>()? != expected_magic
        || reader.u8()? != EXACT_INDEX_PLANE_VERSION_V1
        || reader.u8()? != EXACT_INDEX_PLANE_STATE_SEALED_V1
    {
        return Err(ExactIndexPlaneErrorV1::InvalidState);
    }
    let stored_bump = reader.u8()?;
    if reader.u8()? != 0 {
        return Err(ExactIndexPlaneErrorV1::InvalidState);
    }
    reader.reserved(4)?;
    let market = reader.id()?;
    let epoch = reader.id()?;
    let order_set = reader.id()?;
    let settlement_candidate = reader.id()?;
    let selected_feed = reader.id()?;
    let candidate_bundle_digest = reader.id()?;
    let realm = reader.id()?;
    let profile = reader.id()?;
    let capability_profile = reader.id()?;
    let market_genesis_profile = reader.id()?;
    let market_binding_account = reader.id()?;
    let market_binding_digest = reader.id()?;
    let economic_domain_digest = reader.id()?;
    let page_set_digest = reader.id()?;
    let plane_id = reader.id()?;
    let sibling_account = reader.id()?;
    let settlement_root_account = reader.id()?;
    let owner_order_set_digest = reader.id()?;
    let epoch_generation = reader.u64()?;
    let page_count = reader.u8()?;
    let order_count = reader.u8()?;
    let slice_count = reader.u16()?;
    let edge_count = reader.u16()?;
    let outcome_count = reader.u8()?;
    reader.reserved(1)?;
    let page_slot_counts = reader.array::<MAX_ORDER_PAGES>()?;
    reader.reserved(4)?;
    let rent = DeletableRentOwnerV1 {
        payer: reader.id()?,
        refundable_principal: reader.u64()?,
        donation_floor: reader.u64()?,
    };
    let common = ExactIndexCommonV1 {
        market,
        epoch,
        order_set,
        settlement_candidate,
        selected_feed,
        candidate_bundle_digest,
        realm,
        profile,
        capability_profile,
        market_genesis_profile,
        market_binding_account,
        market_binding_digest,
        economic_domain_digest,
        page_set_digest,
        plane_id,
        sibling_account,
        settlement_root_account,
        owner_order_set_digest,
        epoch_generation,
        page_count,
        order_count,
        outcome_count,
        slice_count,
        edge_count,
        page_slot_counts,
        rent,
        stored_bump,
    };
    common.validate()?;
    Ok(common)
}

fn common_ids(common: &ExactIndexCommonV1) -> [Id32; COMMON_ID_COUNT] {
    [
        common.market,
        common.epoch,
        common.order_set,
        common.settlement_candidate,
        common.selected_feed,
        common.candidate_bundle_digest,
        common.realm,
        common.profile,
        common.capability_profile,
        common.market_genesis_profile,
        common.market_binding_account,
        common.market_binding_digest,
        common.economic_domain_digest,
        common.page_set_digest,
        common.plane_id,
        common.sibling_account,
        common.settlement_root_account,
        common.owner_order_set_digest,
    ]
}

fn encode_aggregate(
    writer: &mut ExactWriter<'_>,
    aggregate: CandidateOrderAggregateRowV1,
) -> Result<(), ExactIndexPlaneErrorV1> {
    writer.u16(aggregate.first_edge)?;
    writer.u16(aggregate.edge_count)?;
    writer.u16(aggregate.distinct_real_counterparties)?;
    writer.u16(aggregate.virtual_edge_count)?;
    writer.u64(aggregate.total_quantity)?;
    writer.u64(aggregate.entitled_quantity)?;
    writer.u8(aggregate.side.code())?;
    writer.u8(1)?;
    writer.bytes(&[0; 6])
}

fn decode_aggregate(
    reader: &mut ExactReader<'_>,
) -> Result<CandidateOrderAggregateRowV1, ExactIndexPlaneErrorV1> {
    let value = CandidateOrderAggregateRowV1 {
        first_edge: reader.u16()?,
        edge_count: reader.u16()?,
        distinct_real_counterparties: reader.u16()?,
        virtual_edge_count: reader.u16()?,
        total_quantity: reader.u64()?,
        entitled_quantity: reader.u64()?,
        side: ExactIndexOrderSideV1::decode(reader.u8()?)?,
    };
    if reader.u8()? != 1 {
        return Err(ExactIndexPlaneErrorV1::InvalidState);
    }
    reader.reserved(6)?;
    Ok(value)
}

fn encode_edge(
    writer: &mut ExactWriter<'_>,
    edge: CandidateOrderSliceEdgeV1,
) -> Result<(), ExactIndexPlaneErrorV1> {
    writer.u16(edge.slice_index)?;
    writer.u8(edge.counterparty_kind.code())?;
    writer.u8(edge.counterparty_order)?;
    writer.u8(edge.outcome)?;
    writer.u8(edge.side.code())?;
    writer.bytes(&[0; 2])?;
    writer.u64(edge.quantity)
}

fn decode_edge(
    reader: &mut ExactReader<'_>,
) -> Result<CandidateOrderSliceEdgeV1, ExactIndexPlaneErrorV1> {
    let value = CandidateOrderSliceEdgeV1 {
        slice_index: reader.u16()?,
        counterparty_kind: ExactIndexCounterpartyV1::decode(reader.u8()?)?,
        counterparty_order: reader.u8()?,
        outcome: reader.u8()?,
        side: ExactIndexOrderSideV1::decode(reader.u8()?)?,
        quantity: {
            reader.reserved(2)?;
            reader.u64()?
        },
    };
    Ok(value)
}

fn validate_symmetric_edges(
    index: &CandidateOrderSliceIndexV1,
) -> Result<(), ExactIndexPlaneErrorV1> {
    let mut edge_index = 0usize;
    let mut slice_end_counts = [0u8; MAX_SLICES];
    let mut slice_virtual_counts = [0u8; MAX_SLICES];
    let mut slice_buy_counts = [0u8; MAX_SLICES];
    let mut slice_sell_counts = [0u8; MAX_SLICES];
    while edge_index < usize::from(index.common.edge_count) {
        let edge = index.edges[edge_index];
        let slice = usize::from(edge.slice_index);
        let slot = &mut slice_end_counts[slice];
        *slot = slot
            .checked_add(1)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let side_count = match edge.side {
            ExactIndexOrderSideV1::Buy => &mut slice_buy_counts[slice],
            ExactIndexOrderSideV1::Sell => &mut slice_sell_counts[slice],
        };
        *side_count = side_count
            .checked_add(1)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        if edge.counterparty_kind == ExactIndexCounterpartyV1::Order {
            let owner = owner_for_edge(index, edge_index)?;
            let counterparty = usize::from(edge.counterparty_order);
            let aggregate = index.aggregates[counterparty];
            let end = aggregate
                .first_edge
                .checked_add(aggregate.edge_count)
                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
            let mut cursor = aggregate.first_edge;
            let mut found = false;
            while cursor < end {
                let reciprocal = index.edges[usize::from(cursor)];
                if reciprocal.slice_index == edge.slice_index
                    && reciprocal.counterparty_kind == ExactIndexCounterpartyV1::Order
                    && usize::from(reciprocal.counterparty_order) == owner
                    && reciprocal.side != edge.side
                    && reciprocal.outcome == edge.outcome
                    && reciprocal.quantity == edge.quantity
                {
                    if found {
                        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
                    }
                    found = true;
                }
                cursor = cursor
                    .checked_add(1)
                    .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
            }
            if !found {
                return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
            }
        } else {
            slice_virtual_counts[slice] = slice_virtual_counts[slice]
                .checked_add(1)
                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        }
        edge_index += 1;
    }
    let mut slice = 0usize;
    while slice < usize::from(index.common.slice_count) {
        let direct = slice_end_counts[slice] == 2
            && slice_virtual_counts[slice] == 0
            && slice_buy_counts[slice] == 1
            && slice_sell_counts[slice] == 1;
        let virtual_route = slice_end_counts[slice] == 1
            && slice_virtual_counts[slice] == 1
            && ((slice_buy_counts[slice] == 1 && slice_sell_counts[slice] == 0)
                || (slice_buy_counts[slice] == 0 && slice_sell_counts[slice] == 1));
        if !direct && !virtual_route {
            return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
        }
        slice += 1;
    }
    while slice < MAX_SLICES {
        if slice_end_counts[slice] != 0 {
            return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
        }
        slice += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    fn id(byte: u8) -> Id32 {
        Id32::new([byte; 32]).expect("fixture identity")
    }

    fn common(sibling: Id32, payer: Id32, bump: u8) -> ExactIndexCommonV1 {
        ExactIndexCommonV1 {
            market: id(1),
            epoch: id(2),
            order_set: id(3),
            settlement_candidate: id(4),
            selected_feed: id(5),
            candidate_bundle_digest: id(6),
            realm: id(7),
            profile: id(8),
            capability_profile: id(9),
            market_genesis_profile: id(10),
            market_binding_account: id(11),
            market_binding_digest: id(12),
            economic_domain_digest: id(13),
            page_set_digest: id(14),
            plane_id: id(15),
            sibling_account: sibling,
            settlement_root_account: id(16),
            owner_order_set_digest: id(17),
            epoch_generation: 3,
            page_count: 1,
            order_count: 2,
            outcome_count: 2,
            slice_count: 1,
            edge_count: 2,
            page_slot_counts: [2, 0, 0, 0],
            rent: DeletableRentOwnerV1 {
                payer,
                refundable_principal: 100,
                donation_floor: 4,
            },
            stored_bump: bump,
        }
    }

    fn pair() -> (Id32, FrozenOrderLocatorV1, Id32, CandidateOrderSliceIndexV1) {
        let locator_account = id(30);
        let adjacency_account = id(31);
        let mut rows = [EMPTY_LOCATOR; MAX_ORDERS];
        rows[0] = FrozenOrderLocatorRowV1 {
            page_index: 0,
            page_slot: 0,
        };
        rows[1] = FrozenOrderLocatorRowV1 {
            page_index: 0,
            page_slot: 1,
        };
        let locator = FrozenOrderLocatorV1 {
            common: common(adjacency_account, id(32), 8),
            rows,
        };
        let mut aggregates = [EMPTY_AGGREGATE; MAX_ORDERS];
        aggregates[0] = CandidateOrderAggregateRowV1 {
            first_edge: 0,
            edge_count: 1,
            distinct_real_counterparties: 1,
            virtual_edge_count: 0,
            total_quantity: 7,
            entitled_quantity: 7,
            side: ExactIndexOrderSideV1::Buy,
        };
        aggregates[1] = CandidateOrderAggregateRowV1 {
            first_edge: 1,
            edge_count: 1,
            distinct_real_counterparties: 1,
            virtual_edge_count: 0,
            total_quantity: 7,
            entitled_quantity: 7,
            side: ExactIndexOrderSideV1::Sell,
        };
        let mut edges = [EMPTY_EDGE; MAX_EXACT_INDEX_EDGES_V1];
        edges[0] = CandidateOrderSliceEdgeV1 {
            slice_index: 0,
            counterparty_kind: ExactIndexCounterpartyV1::Order,
            counterparty_order: 1,
            outcome: 0,
            side: ExactIndexOrderSideV1::Buy,
            quantity: 7,
        };
        edges[1] = CandidateOrderSliceEdgeV1 {
            slice_index: 0,
            counterparty_kind: ExactIndexCounterpartyV1::Order,
            counterparty_order: 0,
            outcome: 0,
            side: ExactIndexOrderSideV1::Sell,
            quantity: 7,
        };
        let adjacency = CandidateOrderSliceIndexV1 {
            common: common(locator_account, id(33), 9),
            aggregates,
            edges,
        };
        (locator_account, locator, adjacency_account, adjacency)
    }

    #[test]
    fn exact_active_width_round_trip_and_pair_query() {
        let (locator_account, locator, adjacency_account, adjacency) = pair();
        validate_exact_index_pair(
            locator_account,
            &locator,
            adjacency_account,
            &adjacency,
        )
        .expect("valid pair");
        let mut locator_bytes = vec![0; locator.encoded_len().expect("locator width")];
        locator.encode(&mut locator_bytes).expect("locator encode");
        let mut adjacency_bytes = vec![0; adjacency.encoded_len().expect("adjacency width")];
        adjacency
            .encode(&mut adjacency_bytes)
            .expect("adjacency encode");
        assert_eq!(FrozenOrderLocatorV1::decode(&locator_bytes), Ok(locator));
        assert_eq!(
            CandidateOrderSliceIndexV1::decode(&adjacency_bytes),
            Ok(adjacency)
        );
        let coverage = indexed_pair_coverage_v1(&locator, &adjacency, 0, 1)
            .expect("indexed pair coverage");
        assert_eq!(coverage.pair_slice_count(), 1);
        assert_eq!(coverage.pair_slice_indices()[0], 0);
        assert_eq!(coverage.buy_total(), 7);
        assert_eq!(coverage.sell_total(), 7);
        assert!(!coverage.buy_elsewhere());
        assert!(!coverage.sell_elsewhere());
        let sealed = indexed_pair_coverage_from_sealed_accounts_v1(
            SealedExactIndexPairInputV1 {
                authority: CountedExactIndexReadAuthorityV1 {
                    plane_id: locator.plane_id(),
                    locator_account,
                    adjacency_account,
                    _private: (),
                },
                locator_body: &locator_bytes,
                adjacency_body: &adjacency_bytes,
            },
            0,
            1,
        )
        .expect("bounded sealed read");
        assert_eq!(sealed, coverage);
    }

    #[test]
    fn decoders_refuse_version_trailing_reserved_and_inactive_capacity() {
        let (_, locator, _, adjacency) = pair();
        let mut locator_bytes = vec![0; locator.encoded_len().expect("locator width")];
        locator.encode(&mut locator_bytes).expect("locator encode");
        locator_bytes[8] = 2;
        assert_eq!(
            FrozenOrderLocatorV1::decode(&locator_bytes),
            Err(ExactIndexPlaneErrorV1::InvalidState)
        );
        locator_bytes[8] = EXACT_INDEX_PLANE_VERSION_V1;
        locator_bytes[12] = 1;
        assert_eq!(
            FrozenOrderLocatorV1::decode(&locator_bytes),
            Err(ExactIndexPlaneErrorV1::InvalidState)
        );
        locator_bytes[12] = 0;
        locator_bytes.push(0);
        assert_eq!(
            FrozenOrderLocatorV1::decode(&locator_bytes),
            Err(ExactIndexPlaneErrorV1::WrongLength)
        );

        let mut adjacency_bytes = vec![0; adjacency.encoded_len().expect("adjacency width")];
        adjacency
            .encode(&mut adjacency_bytes)
            .expect("adjacency encode");
        adjacency_bytes.push(0);
        assert_eq!(
            CandidateOrderSliceIndexV1::decode(&adjacency_bytes),
            Err(ExactIndexPlaneErrorV1::WrongLength)
        );
    }

    #[test]
    fn locator_refuses_duplicate_and_out_of_page_rows() {
        let (_, mut locator, _, _) = pair();
        locator.rows[1] = locator.rows[0];
        assert_eq!(locator.validate(), Err(ExactIndexPlaneErrorV1::InvalidLocator));
        locator.rows[1] = FrozenOrderLocatorRowV1 {
            page_index: 0,
            page_slot: 2,
        };
        assert_eq!(locator.validate(), Err(ExactIndexPlaneErrorV1::InvalidLocator));
    }

    #[test]
    fn adjacency_refuses_asymmetry_wrong_virtual_route_and_aggregate_drift() {
        let (_, _, _, mut adjacency) = pair();
        adjacency.edges[1].quantity = 8;
        assert_eq!(
            adjacency.validate(),
            Err(ExactIndexPlaneErrorV1::AggregateMismatch)
        );

        let (_, _, _, mut adjacency) = pair();
        adjacency.edges[0].counterparty_kind = ExactIndexCounterpartyV1::Merge;
        adjacency.edges[0].counterparty_order = 0;
        assert_eq!(
            adjacency.validate(),
            Err(ExactIndexPlaneErrorV1::InvalidAdjacency)
        );

        let (_, _, _, mut adjacency) = pair();
        adjacency.aggregates[0].entitled_quantity = 6;
        assert_eq!(
            adjacency.validate(),
            Err(ExactIndexPlaneErrorV1::AggregateMismatch)
        );
    }

    #[test]
    fn bounded_sealed_read_refuses_local_edge_tampering_and_wrong_root_authority() {
        let (locator_account, locator, adjacency_account, adjacency) = pair();
        let mut locator_bytes = vec![0; locator.encoded_len().expect("locator width")];
        locator.encode(&mut locator_bytes).expect("locator encode");
        let mut adjacency_bytes = vec![0; adjacency.encoded_len().expect("adjacency width")];
        adjacency
            .encode(&mut adjacency_bytes)
            .expect("adjacency encode");
        let authority = CountedExactIndexReadAuthorityV1 {
            plane_id: locator.plane_id(),
            locator_account,
            adjacency_account,
            _private: (),
        };
        let first_edge_quantity = EXACT_INDEX_COMMON_HEADER_BYTES_V1
            + (2 * CANDIDATE_ORDER_AGGREGATE_ROW_BYTES_V1)
            + 8;
        adjacency_bytes[first_edge_quantity] = 8;
        assert_eq!(
            indexed_pair_coverage_from_sealed_accounts_v1(
                SealedExactIndexPairInputV1 {
                    authority,
                    locator_body: &locator_bytes,
                    adjacency_body: &adjacency_bytes,
                },
                0,
                1,
            ),
            Err(ExactIndexPlaneErrorV1::AggregateMismatch)
        );

        adjacency
            .encode(&mut adjacency_bytes)
            .expect("restore adjacency");
        assert_eq!(
            indexed_pair_coverage_from_sealed_accounts_v1(
                SealedExactIndexPairInputV1 {
                    authority: CountedExactIndexReadAuthorityV1 {
                        plane_id: id(60),
                        ..authority
                    },
                    locator_body: &locator_bytes,
                    adjacency_body: &adjacency_bytes,
                },
                0,
                1,
            ),
            Err(ExactIndexPlaneErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn pair_binding_and_data_ids_cover_siblings_rent_and_rows() {
        let (locator_account, locator, adjacency_account, adjacency) = pair();
        let original_locator_id = locator_data_id(&locator).expect("locator id");
        let original_adjacency_id = adjacency_data_id(&adjacency).expect("adjacency id");
        let mut other_locator = locator;
        other_locator.common.rent.donation_floor = 5;
        assert_ne!(
            locator_data_id(&other_locator).expect("changed locator id"),
            original_locator_id
        );
        let mut other_adjacency = adjacency;
        other_adjacency.common.stored_bump = 10;
        assert_ne!(
            adjacency_data_id(&other_adjacency).expect("changed adjacency id"),
            original_adjacency_id
        );
        other_adjacency.common.sibling_account = id(44);
        assert_eq!(
            validate_exact_index_pair(
                locator_account,
                &locator,
                adjacency_account,
                &other_adjacency,
            ),
            Err(ExactIndexPlaneErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn rent_close_credits_preserve_principal_and_route_all_excess() {
        let rent = DeletableRentOwnerV1 {
            payer: id(50),
            refundable_principal: 100,
            donation_floor: 4,
        };
        let (principal, donation) = close_credits(rent, 111, id(51)).expect("credits");
        assert_eq!(principal.recipient(), id(50));
        assert_eq!(principal.amount(), 100);
        assert_eq!(donation.recipient(), id(51));
        assert_eq!(donation.amount(), 11);
        assert_eq!(
            close_credits(rent, 103, id(51)),
            Err(ExactIndexPlaneErrorV1::InvalidRent)
        );
        assert_eq!(
            close_credits(rent, 111, id(50)),
            Err(ExactIndexPlaneErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn create_pair_refuses_closing_account_and_neutral_sink_as_rent_payers() {
        let account = |account, payer| ExactIndexCreateAccountInputV1 {
            account,
            program_id: id(60),
            system_program: id(61),
            payer,
            payer_lamports: 100,
            target_lamports: 0,
            target_owner: id(61),
            target_data_len: 0,
            target_writable: true,
            target_executable: false,
            rent_exempt_minimum: 100,
            stored_bump: 1,
        };
        let locator = account(id(62), id(64));
        let adjacency = account(id(63), id(65));
        assert_eq!(
            validate_create_pair_identities(
                account(id(62), id(63)),
                adjacency,
                id(66),
            ),
            Err(ExactIndexPlaneErrorV1::InvalidCreateAccount)
        );
        assert_eq!(
            validate_create_pair_identities(locator, account(id(63), id(66)), id(66)),
            Err(ExactIndexPlaneErrorV1::InvalidCreateAccount)
        );
        assert_eq!(
            validate_create_pair_identities(locator, adjacency, id(66)),
            Ok(())
        );
    }
}

fn owner_for_edge(
    index: &CandidateOrderSliceIndexV1,
    edge_index: usize,
) -> Result<usize, ExactIndexPlaneErrorV1> {
    let mut order = 0usize;
    while order < usize::from(index.common.order_count) {
        let row = index.aggregates[order];
        let start = usize::from(row.first_edge);
        let end = start
            .checked_add(usize::from(row.edge_count))
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        if edge_index >= start && edge_index < end {
            return Ok(order);
        }
        order += 1;
    }
    Err(ExactIndexPlaneErrorV1::InvalidAdjacency)
}

struct ExactWriter<'a> {
    output: &'a mut [u8],
    at: usize,
}

impl<'a> ExactWriter<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, at: 0 }
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), ExactIndexPlaneErrorV1> {
        let end = self
            .at
            .checked_add(value.len())
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let target = self
            .output
            .get_mut(self.at..end)
            .ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
        target.copy_from_slice(value);
        self.at = end;
        Ok(())
    }

    fn id(&mut self, value: Id32) -> Result<(), ExactIndexPlaneErrorV1> {
        self.bytes(&value.bytes())
    }

    fn u8(&mut self, value: u8) -> Result<(), ExactIndexPlaneErrorV1> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), ExactIndexPlaneErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), ExactIndexPlaneErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn finish(self) -> Result<(), ExactIndexPlaneErrorV1> {
        if self.at == self.output.len() {
            Ok(())
        } else {
            Err(ExactIndexPlaneErrorV1::WrongLength)
        }
    }
}

struct ExactReader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> ExactReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, at: 0 }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ExactIndexPlaneErrorV1> {
        let end = self
            .at
            .checked_add(N)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let source = self
            .input
            .get(self.at..end)
            .ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
        let mut value = [0u8; N];
        value.copy_from_slice(source);
        self.at = end;
        Ok(value)
    }

    fn id(&mut self) -> Result<Id32, ExactIndexPlaneErrorV1> {
        Id32::new(self.array::<32>()?).map_err(|_| ExactIndexPlaneErrorV1::ZeroIdentity)
    }

    fn u8(&mut self) -> Result<u8, ExactIndexPlaneErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ExactIndexPlaneErrorV1> {
        Ok(u16::from_le_bytes(self.array::<2>()?))
    }

    fn u64(&mut self) -> Result<u64, ExactIndexPlaneErrorV1> {
        Ok(u64::from_le_bytes(self.array::<8>()?))
    }

    fn reserved(&mut self, len: usize) -> Result<(), ExactIndexPlaneErrorV1> {
        let end = self
            .at
            .checked_add(len)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let bytes = self
            .input
            .get(self.at..end)
            .ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
        if bytes.iter().any(|byte| *byte != 0) {
            return Err(ExactIndexPlaneErrorV1::InvalidState);
        }
        self.at = end;
        Ok(())
    }

    fn finish(self) -> Result<(), ExactIndexPlaneErrorV1> {
        if self.at == self.input.len() {
            Ok(())
        } else {
            Err(ExactIndexPlaneErrorV1::WrongLength)
        }
    }
}
