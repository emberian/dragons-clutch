// SPDX-License-Identifier: AGPL-3.0-or-later

//! Compact, capability-disabled exact indexes for General V2 settlement.
//!
//! CandidateFeed remains the sole persisted owner of slice leg, counterparty,
//! outcome, quantity, and route facts. The adjacency child stores only a
//! per-order directory and grouped canonical `u16` Feed slice indices. Both
//! maximum-width children fit below 4 KiB and are streamed directly into
//! account memory; no fixed-capacity adjacency value crosses the SBF stack.

use clutch_batch::Side;
use clutch_general_v2_contract::{
    complete_candidate_feed_v2, encode_retire_indexed_settlement_root_v1,
    CandidateWindowV5AccountV1, DeletableRentOwnerV1, GeneralEpochV6AccountV1,
    AuthenticatedIndexedSettlementRootRentV1, ExactIndexChildrenStateV1, Id32,
    IndexedSettlementRootCloseProjectionV1, IndexedSettlementRootV1AccountV1,
    MarketBindingV2, SettlementRootV1AccountV1,
    SettlementSliceLegKindV1, SettlementSliceV1, INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG,
    INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION, INDEXED_SETTLEMENT_ROOT_BYTES_V1, MAX_ORDERS,
    MAX_OUTCOMES, MAX_SLICES, SETTLEMENT_SLICE_BYTES,
};
use clutch_solana_layout::registry::{
    GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_TAG,
    GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_VERSION,
    GENERAL_V2_CANDIDATE_ADJACENCY_MAX_ACCOUNT_BYTES,
    GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_TAG,
    GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_VERSION,
    GENERAL_V2_FROZEN_ORDER_LOCATOR_MAX_ACCOUNT_BYTES,
    GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_BYTES,
    GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION,
    GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG,
};
use clutch_solana_layout::MAX_ORDERS_PER_PAGE;
use sha2::{Digest, Sha256};

use crate::{
    authenticate_settlement_feed_view_v5, bind_settlement_root_traversal_v5, CanonicalSha256,
    SettlementLegV1, SettlementRouteV1, SettlementTraversalAccessV5,
};

pub const EXACT_INDEX_PLANE_VERSION_V1: u8 = 1;
pub const EXACT_INDEX_PLANE_STATE_SEALED_V1: u8 = 1;
pub const EXACT_INDEX_PLANE_LIVE_ENABLED_V1: bool = false;
pub const FROZEN_ORDER_LOCATOR_MAGIC_V1: [u8; 8] = [
    GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_TAG,
    GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_VERSION,
    b'D', b'C', b'I', b'X', b'L', b'1',
];
pub const CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1: [u8; 8] = [
    GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_TAG,
    GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_VERSION,
    b'D', b'C', b'I', b'X', b'A', b'1',
];
pub const EXACT_INDEX_PLANE_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/compact-exact-index-plane/v1\0";
pub const FROZEN_ORDER_LOCATOR_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/compact-order-locator-data/v1\0";
pub const CANDIDATE_ORDER_SLICE_INDEX_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/compact-order-slice-index-data/v1\0";
const ENVELOPE_BYTES: usize = 16;
const COMPACT_IDS_BYTES: usize = 6 * 32;
const COMPACT_COUNTER_BYTES: usize = 16;
const RENT_OWNER_BYTES: usize = 48;
pub const EXACT_INDEX_COMMON_HEADER_BYTES_V1: usize =
    ENVELOPE_BYTES + COMPACT_IDS_BYTES + COMPACT_COUNTER_BYTES + RENT_OWNER_BYTES;
pub const FROZEN_ORDER_LOCATOR_ROW_BYTES_V1: usize = 4;
pub const CANDIDATE_ORDER_DIRECTORY_ROW_BYTES_V1: usize = 8;
pub const CANDIDATE_ORDER_SLICE_REFERENCE_BYTES_V1: usize = 2;
pub const MAX_EXACT_INDEX_SLICE_REFERENCES_V1: usize = MAX_SLICES * 2;
pub const FROZEN_ORDER_LOCATOR_MAX_BYTES_V1: usize =
    EXACT_INDEX_COMMON_HEADER_BYTES_V1 + MAX_ORDERS * FROZEN_ORDER_LOCATOR_ROW_BYTES_V1;
pub const CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1: usize =
    EXACT_INDEX_COMMON_HEADER_BYTES_V1
        + MAX_ORDERS * CANDIDATE_ORDER_DIRECTORY_ROW_BYTES_V1
        + MAX_EXACT_INDEX_SLICE_REFERENCES_V1 * CANDIDATE_ORDER_SLICE_REFERENCE_BYTES_V1;

const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_SLICES == 416);
const _: () = assert!(EXACT_INDEX_COMMON_HEADER_BYTES_V1 == 272);
const _: () = assert!(FROZEN_ORDER_LOCATOR_MAX_BYTES_V1 == 528);
const _: () = assert!(CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1 == 2_448);
const _: () = assert!(FROZEN_ORDER_LOCATOR_MAX_BYTES_V1 <= 4_096);
const _: () = assert!(CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1 <= 4_096);
const _: () = assert!(FROZEN_ORDER_LOCATOR_MAX_BYTES_V1
    == GENERAL_V2_FROZEN_ORDER_LOCATOR_MAX_ACCOUNT_BYTES);
const _: () = assert!(CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1
    == GENERAL_V2_CANDIDATE_ADJACENCY_MAX_ACCOUNT_BYTES);
const _: () = assert!(INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG
    == GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG);
const _: () = assert!(INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION
    == GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION);
const _: () = assert!(INDEXED_SETTLEMENT_ROOT_BYTES_V1
    == GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_BYTES);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactIndexPlaneErrorV1 {
    CandidateTraversal,
    RootBinding,
    BindingMismatch,
    ZeroIdentity,
    InvalidCount,
    InvalidState,
    ArithmeticOverflow,
    WrongLength,
    InvalidLocator,
    InvalidAdjacency,
    InvalidCreateAccount,
    InvalidRent,
    NonTerminalRoot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExactIndexOrderSideV1 { Buy = 1, Sell = 2 }
impl ExactIndexOrderSideV1 {
    fn decode(value: u8) -> Result<Self, ExactIndexPlaneErrorV1> {
        match value { 1 => Ok(Self::Buy), 2 => Ok(Self::Sell), _ => Err(ExactIndexPlaneErrorV1::InvalidState) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrozenOrderLocatorRowV1 { page_index: u16, page_slot: u8 }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateOrderDirectoryRowV1 {
    first_slice_ref: u16,
    slice_ref_count: u16,
    side: ExactIndexOrderSideV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactIndexCommonV1 {
    settlement_root_account: Id32,
    sibling_account: Id32,
    plane_id: Id32,
    selected_feed_account: Id32,
    selected_feed_data_id: Id32,
    traversal_binding_id: Id32,
    order_count: u8,
    outcome_count: u8,
    slice_count: u16,
    slice_reference_count: u16,
    page_count: u8,
    page_physical_slot_counts: [u8; 4],
    rent: DeletableRentOwnerV1,
    stored_bump: u8,
}
impl ExactIndexCommonV1 {
    fn validate(&self) -> Result<(), ExactIndexPlaneErrorV1> {
        for id in [self.settlement_root_account, self.sibling_account, self.plane_id,
            self.selected_feed_account, self.selected_feed_data_id, self.traversal_binding_id]
        {
            if id.is_zero() { return Err(ExactIndexPlaneErrorV1::ZeroIdentity); }
        }
        let maximum_references = self.slice_count.checked_mul(2)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        if self.order_count == 0 || usize::from(self.order_count) > MAX_ORDERS
            || self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.slice_count == 0 || usize::from(self.slice_count) > MAX_SLICES
            || self.slice_reference_count < self.slice_count
            || self.slice_reference_count > maximum_references
            || usize::from(self.slice_reference_count) > MAX_EXACT_INDEX_SLICE_REFERENCES_V1
            || self.page_count == 0 || usize::from(self.page_count) > 4
        { return Err(ExactIndexPlaneErrorV1::InvalidCount); }
        let mut page = 0usize;
        while page < 4 {
            if (page < usize::from(self.page_count))
                != (self.page_physical_slot_counts[page] != 0)
                || usize::from(self.page_physical_slot_counts[page]) > MAX_ORDERS_PER_PAGE
            {
                return Err(ExactIndexPlaneErrorV1::InvalidCount);
            }
            page += 1;
        }
        self.rent.validate().map_err(|_| ExactIndexPlaneErrorV1::InvalidRent)
    }
    fn semantic_eq(&self, other: &Self) -> bool {
        self.settlement_root_account == other.settlement_root_account
            && self.plane_id == other.plane_id
            && self.selected_feed_account == other.selected_feed_account
            && self.selected_feed_data_id == other.selected_feed_data_id
            && self.traversal_binding_id == other.traversal_binding_id
            && self.order_count == other.order_count
            && self.outcome_count == other.outcome_count
            && self.slice_count == other.slice_count
            && self.slice_reference_count == other.slice_reference_count
            && self.page_count == other.page_count
            && self.page_physical_slot_counts == other.page_physical_slot_counts
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexCreateAccountInputV1 {
    pub account: Id32, pub program_id: Id32, pub payer: Id32,
    pub payer_lamports: u64, pub target_lamports: u64, pub target_owner: Id32,
    pub target_data_len: usize, pub target_writable: bool, pub target_executable: bool,
    pub rent_exempt_minimum: u64, pub stored_bump: u8,
}
impl ExactIndexCreateAccountInputV1 {
    fn validate(self, expected_len: usize, forbidden: &[Id32])
        -> Result<DeletableRentOwnerV1, ExactIndexPlaneErrorV1>
    {
        if self.account.is_zero() || self.program_id.is_zero() || self.payer.is_zero()
            || self.payer == self.account
            || self.account == self.program_id || self.payer == self.program_id
            || forbidden.contains(&self.account) || forbidden.contains(&self.payer)
            || !self.target_owner.is_zero() || self.target_data_len != 0
            || !self.target_writable || self.target_executable
            || self.rent_exempt_minimum == 0 || self.payer_lamports < self.rent_exempt_minimum
            || expected_len > 4_096
        { return Err(ExactIndexPlaneErrorV1::InvalidCreateAccount); }
        let rent = DeletableRentOwnerV1 { payer: self.payer,
            refundable_principal: self.rent_exempt_minimum, donation_floor: self.target_lamports };
        rent.validate().map_err(|_| ExactIndexPlaneErrorV1::InvalidRent)?;
        Ok(rent)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ConstructExactIndexStreamingInputV1<'a> {
    pub traversal: &'a dyn SettlementTraversalAccessV5,
    pub settlement_root_account: Id32,
    pub settlement_root: &'a SettlementRootV1AccountV1,
    pub capability_profile_id: Id32,
    pub locator_create: ExactIndexCreateAccountInputV1,
    pub adjacency_create: ExactIndexCreateAccountInputV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactIndexRootStreamResultV1 {
    indexed_root_data_id: Id32, locator_data_id: Id32, adjacency_data_id: Id32, plane_id: Id32,
}
impl CountedExactIndexRootStreamResultV1 {
    pub const fn indexed_root_data_id(&self) -> Id32 { self.indexed_root_data_id }
    pub const fn locator_data_id(&self) -> Id32 { self.locator_data_id }
    pub const fn adjacency_data_id(&self) -> Id32 { self.adjacency_data_id }
    pub const fn plane_id(&self) -> Id32 { self.plane_id }
}

pub fn locator_data_len_v1(order_count: u8) -> Result<usize, ExactIndexPlaneErrorV1> {
    if order_count == 0 || usize::from(order_count) > MAX_ORDERS { return Err(ExactIndexPlaneErrorV1::InvalidCount); }
    EXACT_INDEX_COMMON_HEADER_BYTES_V1.checked_add(usize::from(order_count) * FROZEN_ORDER_LOCATOR_ROW_BYTES_V1)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)
}
pub fn adjacency_data_len_v1(order_count: u8, references: u16) -> Result<usize, ExactIndexPlaneErrorV1> {
    if order_count == 0 || usize::from(order_count) > MAX_ORDERS
        || usize::from(references) > MAX_EXACT_INDEX_SLICE_REFERENCES_V1
    { return Err(ExactIndexPlaneErrorV1::InvalidCount); }
    EXACT_INDEX_COMMON_HEADER_BYTES_V1
        .checked_add(usize::from(order_count) * CANDIDATE_ORDER_DIRECTORY_ROW_BYTES_V1)
        .and_then(|v| v.checked_add(usize::from(references) * CANDIDATE_ORDER_SLICE_REFERENCE_BYTES_V1))
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)
}

pub fn stream_counted_exact_index_root_v1(
    root_rent: AuthenticatedIndexedSettlementRootRentV1<'_>,
    input: &ConstructExactIndexStreamingInputV1<'_>,
    root_output: &mut [u8], locator_output: &mut [u8], adjacency_output: &mut [u8],
) -> Result<CountedExactIndexRootStreamResultV1, ExactIndexPlaneErrorV1> {
    input.settlement_root.validate().map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    let projection = input.traversal.projection();
    bind_settlement_root_traversal_v5(input.settlement_root_account, input.settlement_root, projection)
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    if root_rent.root_account() != input.settlement_root_account
        || root_rent.base_before() != input.settlement_root || input.capability_profile_id.is_zero()
        || projection.selected_feed_account() != input.settlement_root.retained_feed()
        || projection.candidate_bundle_digest() != input.settlement_root.candidate_bundle_digest()
        || projection.feed_data_id().is_zero()
    { return Err(ExactIndexPlaneErrorV1::RootBinding); }
    let feed = projection.feed();
    let references = projection.exact_slice_reference_count();
    let maximum_references = feed
        .slice_count
        .checked_mul(2)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    if references < feed.slice_count || references > maximum_references {
        return Err(ExactIndexPlaneErrorV1::InvalidCount);
    }
    let locator_len = locator_data_len_v1(feed.order_count)?;
    let adjacency_len = adjacency_data_len_v1(feed.order_count, references)?;
    if root_output.len() != INDEXED_SETTLEMENT_ROOT_BYTES_V1 || locator_output.len() != locator_len
        || adjacency_output.len() != adjacency_len { return Err(ExactIndexPlaneErrorV1::WrongLength); }
    let forbidden = [input.settlement_root_account, projection.selected_feed_account(),
        input.settlement_root.market(), input.settlement_root.market_binding()];
    let locator_rent = input.locator_create.validate(locator_len, &forbidden)?;
    let adjacency_rent = input.adjacency_create.validate(adjacency_len, &forbidden)?;
    if input.locator_create.account == input.adjacency_create.account
        || input.locator_create.program_id != input.adjacency_create.program_id
        || input.locator_create.payer != input.adjacency_create.payer
        || input.locator_create.payer != root_rent.rent_after().payer
        || input.locator_create.payer_lamports != input.adjacency_create.payer_lamports
    { return Err(ExactIndexPlaneErrorV1::InvalidCreateAccount); }
    let mut combined = root_rent.payer_debit_lamports();
    for create in [input.locator_create, input.adjacency_create] {
        if create.payer == root_rent.rent_after().payer {
            if create.payer_lamports != root_rent.payer_balance_before_lamports() {
                return Err(ExactIndexPlaneErrorV1::InvalidRent);
            }
            combined = combined.checked_add(create.rent_exempt_minimum)
                .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        }
    }
    if combined > root_rent.payer_balance_before_lamports() { return Err(ExactIndexPlaneErrorV1::InvalidRent); }

    locator_output.fill(0); adjacency_output.fill(0);
    let physical_slot_counts = projection.page_physical_slot_counts();
    let mut next = 0u16;
    let mut cursors = [0u16; MAX_ORDERS];
    let mut order = 0usize;
    while order < usize::from(feed.order_count) {
        let index = u8::try_from(order).map_err(|_| ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let order_row = input.traversal.order(index)
            .map_err(|_| ExactIndexPlaneErrorV1::CandidateTraversal)?
            .ok_or(ExactIndexPlaneErrorV1::InvalidLocator)?;
        let location = FrozenOrderLocatorRowV1 {
            page_index: order_row.page_index(),
            page_slot: order_row.page_slot(),
        };
        if usize::from(location.page_index) >= usize::from(projection.page_count())
            || location.page_slot
                >= physical_slot_counts[usize::from(location.page_index)]
        {
            return Err(ExactIndexPlaneErrorV1::InvalidLocator);
        }
        let mut prior = 0usize;
        while prior < order {
            if read_locator(locator_output, prior)? == location {
                return Err(ExactIndexPlaneErrorV1::InvalidLocator);
            }
            prior += 1;
        }
        write_locator(locator_output, order, location)?;
        let side = match order_row.economic_order().side {
            Side::Buy => ExactIndexOrderSideV1::Buy, Side::Sell => ExactIndexOrderSideV1::Sell,
        };
        let slice_ref_count = projection
            .order_slice_reference_count(index)
            .ok_or(ExactIndexPlaneErrorV1::InvalidAdjacency)?;
        write_directory(adjacency_output, order, CandidateOrderDirectoryRowV1 {
            first_slice_ref: next, slice_ref_count, side,
        })?;
        cursors[order] = next;
        next = next
            .checked_add(slice_ref_count)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        order += 1;
    }
    if next != references {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    let mut slice = 0u16;
    while slice < feed.slice_count {
        let value = input.traversal.settlement_slice(slice)
            .map_err(|_| ExactIndexPlaneErrorV1::CandidateTraversal)?;
        if value.quantity() == 0 {
            return Err(ExactIndexPlaneErrorV1::CandidateTraversal);
        }
        match (value.buy(), value.sell(), value.route()) {
            (SettlementLegV1::Order(buy), SettlementLegV1::Order(sell), SettlementRouteV1::Direct) => {
                write_reference_for_side(adjacency_output, feed.order_count, &mut cursors, buy,
                    ExactIndexOrderSideV1::Buy, slice)?;
                write_reference_for_side(adjacency_output, feed.order_count, &mut cursors, sell,
                    ExactIndexOrderSideV1::Sell, slice)?;
            }
            (SettlementLegV1::Order(buy), SettlementLegV1::Split, SettlementRouteV1::SplitToBuy) =>
                write_reference_for_side(adjacency_output, feed.order_count, &mut cursors, buy,
                    ExactIndexOrderSideV1::Buy, slice)?,
            (SettlementLegV1::Merge, SettlementLegV1::Order(sell), SettlementRouteV1::SellToMerge) =>
                write_reference_for_side(adjacency_output, feed.order_count, &mut cursors, sell,
                    ExactIndexOrderSideV1::Sell, slice)?,
            _ => return Err(ExactIndexPlaneErrorV1::CandidateTraversal),
        }
        slice = slice.checked_add(1).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    }
    order = 0;
    while order < usize::from(feed.order_count) {
        let row = read_directory(adjacency_output, order)?;
        let end = row.first_slice_ref.checked_add(row.slice_ref_count)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        if cursors[order] != end { return Err(ExactIndexPlaneErrorV1::InvalidAdjacency); }
        order += 1;
    }
    let plane_id = derive_plane_id(input.settlement_root, input.settlement_root_account,
        projection.selected_feed_account(), projection.feed_data_id(),
        projection.owner_order_set_digest(), input.locator_create.account,
        input.adjacency_create.account, locator_rent, adjacency_rent, feed.order_count,
        references, &locator_output[EXACT_INDEX_COMMON_HEADER_BYTES_V1..],
        &adjacency_output[EXACT_INDEX_COMMON_HEADER_BYTES_V1..])?;
    let common = ExactIndexCommonV1 {
        settlement_root_account: input.settlement_root_account,
        sibling_account: input.adjacency_create.account, plane_id,
        selected_feed_account: projection.selected_feed_account(),
        selected_feed_data_id: projection.feed_data_id(),
        traversal_binding_id: projection.owner_order_set_digest(), order_count: feed.order_count,
        outcome_count: feed.outcome_count, slice_count: feed.slice_count,
        slice_reference_count: references, page_count: projection.page_count(),
        page_physical_slot_counts: physical_slot_counts, rent: locator_rent,
        stored_bump: input.locator_create.stored_bump,
    };
    encode_common(locator_output, FROZEN_ORDER_LOCATOR_MAGIC_V1, common)?;
    encode_common(adjacency_output, CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1,
        ExactIndexCommonV1 { sibling_account: input.locator_create.account,
            rent: adjacency_rent, stored_bump: input.adjacency_create.stored_bump, ..common })?;
    let locator_data_id = sealed_locator_data_id_from_raw_v1(locator_output)?;
    let adjacency_data_id = sealed_adjacency_data_id_from_raw_v1(adjacency_output)?;
    let indexed_root_data_id = root_rent.encode_new_live_and_data_id(
        input.locator_create.account,
        input.adjacency_create.account,
        plane_id,
        locator_data_id,
        adjacency_data_id,
        projection.feed_data_id(),
        input.capability_profile_id,
        &CanonicalSha256,
        root_output,
    )
    .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    Ok(CountedExactIndexRootStreamResultV1 { indexed_root_data_id, locator_data_id,
        adjacency_data_id, plane_id })
}

fn write_reference_for_side(
    body: &mut [u8], order_count: u8, cursors: &mut [u16; MAX_ORDERS],
    order: u8, expected_side: ExactIndexOrderSideV1, slice: u16,
) -> Result<(), ExactIndexPlaneErrorV1>
{
    let index = usize::from(order);
    if order >= order_count { return Err(ExactIndexPlaneErrorV1::InvalidAdjacency); }
    let row = read_directory(body, index)?;
    if row.side != expected_side {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    let cursor = cursors[index];
    let end = row.first_slice_ref.checked_add(row.slice_ref_count).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    if cursor < row.first_slice_ref || cursor >= end { return Err(ExactIndexPlaneErrorV1::InvalidAdjacency); }
    body.get_mut(reference_range(order_count, cursor)?).ok_or(ExactIndexPlaneErrorV1::WrongLength)?
        .copy_from_slice(&slice.to_le_bytes());
    cursors[index] = cursor.checked_add(1).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn derive_plane_id(root: &SettlementRootV1AccountV1, root_account: Id32, feed: Id32,
    feed_data_id: Id32, traversal: Id32, locator: Id32, adjacency: Id32,
    locator_rent: DeletableRentOwnerV1, adjacency_rent: DeletableRentOwnerV1,
    order_count: u8, references: u16, locator_rows: &[u8], adjacency_rows: &[u8])
    -> Result<Id32, ExactIndexPlaneErrorV1>
{
    let mut hasher = Sha256::new(); hasher.update(EXACT_INDEX_PLANE_ID_DOMAIN_V1);
    for id in [root_account, root.market(), root.epoch(), root.order_set(),
        root.settlement_candidate_id(), feed, feed_data_id, traversal, locator, adjacency]
    { hasher.update(id.bytes()); }
    hasher.update([order_count, root.outcome_count()]);
    hasher.update(root.counts().expected_receipts.to_le_bytes());
    hasher.update(references.to_le_bytes());
    update_rent_hash(&mut hasher, locator_rent); update_rent_hash(&mut hasher, adjacency_rent);
    hasher.update(locator_rows); hasher.update(adjacency_rows); finish_id(hasher)
}
fn update_rent_hash(hasher: &mut Sha256, rent: DeletableRentOwnerV1) {
    hasher.update(rent.payer.bytes()); hasher.update(rent.refundable_principal.to_le_bytes());
    hasher.update(rent.donation_floor.to_le_bytes());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexReadAccountInputV1<'a> {
    pub account: Id32, pub body: &'a [u8], pub owner: Id32, pub canonical_account: Id32,
    pub canonical_bump: u8, pub writable: bool, pub executable: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticateCountedExactIndexReadInputV1<'a> {
    pub program_id: Id32, pub root: ExactIndexReadAccountInputV1<'a>,
    pub locator: ExactIndexReadAccountInputV1<'a>, pub adjacency: ExactIndexReadAccountInputV1<'a>,
    pub feed: ExactIndexReadAccountInputV1<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactIndexReadAuthorityV1 {
    program_id: Id32, root_account: Id32, plane_id: Id32,
    locator_account: Id32, adjacency_account: Id32,
    feed_account: Id32, feed_full_data_id: Id32, _private: (),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedExactIndexPairInputV1<'a> {
    authority: CountedExactIndexReadAuthorityV1,
    locator_body: &'a [u8],
    adjacency_body: &'a [u8],
    feed_body: &'a [u8],
}
pub fn authenticate_counted_exact_index_read_v1<'a>(input: AuthenticateCountedExactIndexReadInputV1<'a>)
    -> Result<SealedExactIndexPairInputV1<'a>, ExactIndexPlaneErrorV1>
{ let (_, sealed) = authenticate_join(input, false, false)?; Ok(sealed) }

fn authenticate_join<'a>(input: AuthenticateCountedExactIndexReadInputV1<'a>, root_writable: bool,
    children_writable: bool) -> Result<(IndexedSettlementRootV1AccountV1,
    SealedExactIndexPairInputV1<'a>), ExactIndexPlaneErrorV1>
{
    if input.program_id.is_zero() { return Err(ExactIndexPlaneErrorV1::ZeroIdentity); }
    let physical = [input.root.account, input.locator.account, input.adjacency.account,
        input.feed.account];
    let mut left = 0usize;
    while left < physical.len() {
        let mut right = left + 1;
        while right < physical.len() {
            if physical[left] == physical[right] {
                return Err(ExactIndexPlaneErrorV1::BindingMismatch);
            }
            right += 1;
        }
        left += 1;
    }
    for (account, writable) in [(input.root, root_writable), (input.locator, children_writable),
        (input.adjacency, children_writable), (input.feed, false)]
    {
        if account.account.is_zero() || account.owner != input.program_id
            || account.account != account.canonical_account || account.writable != writable
            || account.executable { return Err(ExactIndexPlaneErrorV1::BindingMismatch); }
    }
    let root = IndexedSettlementRootV1AccountV1::decode(input.root.body)
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    if root.index_state() != ExactIndexChildrenStateV1::Live
        || root.base().stored_bump() != input.root.canonical_bump
        || root.locator_account() != input.locator.account
        || root.adjacency_account() != input.adjacency.account
        || root.base().retained_feed() != input.feed.account
    { return Err(ExactIndexPlaneErrorV1::RootBinding); }
    let locator = decode_common(input.locator.body, FROZEN_ORDER_LOCATOR_MAGIC_V1)?;
    let adjacency = decode_common(input.adjacency.body, CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1)?;
    let feed_view = authenticate_settlement_feed_view_v5(input.feed.account, input.feed.body)
        .map_err(|_| ExactIndexPlaneErrorV1::CandidateTraversal)?;
    let feed_bundle_id = feed_view.candidate_bundle_digest();
    let feed_full_data_id = feed_view.data_id();
    if !locator.semantic_eq(&adjacency) || locator.sibling_account != input.adjacency.account
        || adjacency.sibling_account != input.locator.account
        || locator.stored_bump != input.locator.canonical_bump
        || adjacency.stored_bump != input.adjacency.canonical_bump
        || locator.settlement_root_account != input.root.account
        || locator.selected_feed_account != input.feed.account
        || locator.selected_feed_data_id != feed_full_data_id
        || root.selected_feed_data_id() != feed_full_data_id
        || feed_bundle_id != root.base().candidate_bundle_digest()
        || locator.traversal_binding_id != root.base().owner_order_set_digest()
        || locator.plane_id != root.plane_id()
        || sealed_locator_data_id_from_raw_v1(input.locator.body)? != root.locator_data_id()
        || sealed_adjacency_data_id_from_raw_v1(input.adjacency.body)? != root.adjacency_data_id()
        || locator.order_count != root.base().order_count()
        || locator.outcome_count != root.base().outcome_count()
        || locator.slice_count != root.base().counts().expected_receipts
    { return Err(ExactIndexPlaneErrorV1::BindingMismatch); }
    Ok((root, SealedExactIndexPairInputV1 {
        authority: CountedExactIndexReadAuthorityV1 { program_id: input.program_id,
            root_account: input.root.account,
            plane_id: root.plane_id(),
            locator_account: input.locator.account, adjacency_account: input.adjacency.account,
            feed_account: input.feed.account, feed_full_data_id, _private: () },
        locator_body: input.locator.body, adjacency_body: input.adjacency.body,
        feed_body: input.feed.body,
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedPairCoverageV1 {
    pair_slice_indices: [u16; MAX_SLICES], pair_slice_count: u16,
    buy_total: u64, sell_total: u64, buy_elsewhere: bool, sell_elsewhere: bool,
}
impl IndexedPairCoverageV1 {
    pub const fn pair_slice_indices(&self) -> &[u16; MAX_SLICES] { &self.pair_slice_indices }
    pub const fn pair_slice_count(&self) -> u16 { self.pair_slice_count }
    pub const fn buy_total(&self) -> u64 { self.buy_total }
    pub const fn sell_total(&self) -> u64 { self.sell_total }
    pub const fn buy_elsewhere(&self) -> bool { self.buy_elsewhere }
    pub const fn sell_elsewhere(&self) -> bool { self.sell_elsewhere }
}
pub fn indexed_pair_coverage_from_sealed_accounts_v1(input: SealedExactIndexPairInputV1<'_>,
    buy_order: u8, sell_order: u8) -> Result<IndexedPairCoverageV1, ExactIndexPlaneErrorV1>
{
    let locator = decode_common(input.locator_body, FROZEN_ORDER_LOCATOR_MAGIC_V1)?;
    let adjacency = decode_common(input.adjacency_body, CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1)?;
    if !locator.semantic_eq(&adjacency) || locator.plane_id != input.authority.plane_id
        || locator.sibling_account != input.authority.adjacency_account
        || adjacency.sibling_account != input.authority.locator_account
        || locator.selected_feed_account != input.authority.feed_account
        || buy_order >= locator.order_count || sell_order >= locator.order_count || buy_order == sell_order
    { return Err(ExactIndexPlaneErrorV1::BindingMismatch); }
    let buy_location = read_locator(input.locator_body, usize::from(buy_order))?;
    let sell_location = read_locator(input.locator_body, usize::from(sell_order))?;
    for location in [buy_location, sell_location] {
        if usize::from(location.page_index) >= usize::from(locator.page_count)
            || location.page_slot
                >= locator.page_physical_slot_counts[usize::from(location.page_index)]
        {
            return Err(ExactIndexPlaneErrorV1::InvalidLocator);
        }
    }
    let buy = read_directory(input.adjacency_body, usize::from(buy_order))?;
    let sell = read_directory(input.adjacency_body, usize::from(sell_order))?;
    if buy.side != ExactIndexOrderSideV1::Buy || sell.side != ExactIndexOrderSideV1::Sell {
        return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
    }
    let (header, tail) = complete_candidate_feed_v2(input.feed_body, true)
        .map_err(|_| ExactIndexPlaneErrorV1::CandidateTraversal)?;
    if header.order_count != locator.order_count || header.outcome_count != locator.outcome_count
        || header.slice_count != locator.slice_count { return Err(ExactIndexPlaneErrorV1::BindingMismatch); }
    let mut pair_slices = [0u16; MAX_SLICES];
    let (buy_count, buy_total, buy_elsewhere) = scan_group(input.adjacency_body, &adjacency,
        tail.slices_le(), buy_order, sell_order, buy, true, &mut pair_slices)?;
    let mut sell_slices = [0u16; MAX_SLICES];
    let (sell_count, sell_total, sell_elsewhere) = scan_group(input.adjacency_body, &adjacency,
        tail.slices_le(), sell_order, buy_order, sell, false, &mut sell_slices)?;
    if buy_count == 0 || buy_count != sell_count { return Err(ExactIndexPlaneErrorV1::InvalidAdjacency); }
    let mut pair = 0usize;
    while pair < usize::from(buy_count) {
        if pair_slices[pair] != sell_slices[pair] { return Err(ExactIndexPlaneErrorV1::InvalidAdjacency); }
        pair += 1;
    }
    Ok(IndexedPairCoverageV1 { pair_slice_indices: pair_slices, pair_slice_count: buy_count,
        buy_total, sell_total, buy_elsewhere, sell_elsewhere })
}
#[allow(clippy::too_many_arguments)]
fn scan_group(body: &[u8], common: &ExactIndexCommonV1, slices: &[u8], order: u8,
    counterparty: u8, directory: CandidateOrderDirectoryRowV1, buy_side: bool,
    pair_slices: &mut [u16; MAX_SLICES]) -> Result<(u16, u64, bool), ExactIndexPlaneErrorV1>
{
    let end = directory.first_slice_ref.checked_add(directory.slice_ref_count)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    let mut cursor = directory.first_slice_ref; let mut previous = None;
    let mut pair_count = 0u16; let mut total = 0u64; let mut elsewhere = false;
    while cursor < end {
        let slice_index = read_reference(body, common.order_count, cursor)?;
        if slice_index >= common.slice_count || previous.is_some_and(|v| slice_index <= v) {
            return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
        }
        previous = Some(slice_index);
        let at = usize::from(slice_index).checked_mul(SETTLEMENT_SLICE_BYTES)
            .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let record = SettlementSliceV1::decode(slices.get(at..at + SETTLEMENT_SLICE_BYTES)
            .ok_or(ExactIndexPlaneErrorV1::WrongLength)?, common.order_count, common.outcome_count)
            .map_err(|_| ExactIndexPlaneErrorV1::CandidateTraversal)?;
        total = total.checked_add(record.quantity).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let matches = if buy_side {
            if record.buy_kind != SettlementSliceLegKindV1::Order || record.buy_index != order {
                return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
            }
            record.sell_kind == SettlementSliceLegKindV1::Order && record.sell_index == counterparty
        } else {
            if record.sell_kind != SettlementSliceLegKindV1::Order || record.sell_index != order {
                return Err(ExactIndexPlaneErrorV1::InvalidAdjacency);
            }
            record.buy_kind == SettlementSliceLegKindV1::Order && record.buy_index == counterparty
        };
        if matches {
            let at = usize::from(pair_count);
            if at >= pair_slices.len() { return Err(ExactIndexPlaneErrorV1::InvalidAdjacency); }
            pair_slices[at] = slice_index;
            pair_count = pair_count.checked_add(1).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        } else { elsewhere = true; }
        cursor = cursor.checked_add(1).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    }
    Ok((pair_count, total, elsewhere))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexCloseAccountInputV1 {
    pub account: Id32, pub lamports: u64, pub owner: Id32,
    pub program_id: Id32, pub writable: bool, pub executable: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct CloseExactIndexPlaneInputV1<'a> {
    pub market_binding_account: Id32, pub market_binding: &'a MarketBindingV2,
    pub locator: ExactIndexCloseAccountInputV1,
    pub adjacency: ExactIndexCloseAccountInputV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexCloseCreditV1 { recipient: Id32, amount: u64 }
impl ExactIndexCloseCreditV1 {
    pub const fn recipient(&self) -> Id32 { self.recipient }
    pub const fn amount(&self) -> u64 { self.amount }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexPlaneClosePostwritesV1 {
    locator_principal: ExactIndexCloseCreditV1, locator_donation: ExactIndexCloseCreditV1,
    adjacency_principal: ExactIndexCloseCreditV1, adjacency_donation: ExactIndexCloseCreditV1,
}
impl ExactIndexPlaneClosePostwritesV1 {
    pub const fn locator_principal_credit(&self) -> ExactIndexCloseCreditV1 { self.locator_principal }
    pub const fn locator_donation_credit(&self) -> ExactIndexCloseCreditV1 { self.locator_donation }
    pub const fn adjacency_principal_credit(&self) -> ExactIndexCloseCreditV1 { self.adjacency_principal }
    pub const fn adjacency_donation_credit(&self) -> ExactIndexCloseCreditV1 { self.adjacency_donation }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactIndexRootRetirementPostwritesV1 {
    indexed_root_poststate_data_id: Id32, close: ExactIndexPlaneClosePostwritesV1,
}
impl CountedExactIndexRootRetirementPostwritesV1 {
    pub const fn indexed_root_poststate_data_id(&self) -> Id32 { self.indexed_root_poststate_data_id }
    pub const fn close_postwrites(&self) -> &ExactIndexPlaneClosePostwritesV1 { &self.close }
}
/// Authenticate and close both exact-index children while streaming the root
/// successor in the same action-specific composition.
///
/// The hostile indexed-root body is decoded only inside this call. Neither its
/// 1,228-byte value nor a second full poststate crosses the SBF caller boundary.
pub fn stream_retire_counted_exact_index_root_v1(
    authentication: AuthenticateCountedExactIndexReadInputV1<'_>,
    input: CloseExactIndexPlaneInputV1<'_>,
    root_output: &mut [u8],
)
    -> Result<CountedExactIndexRootRetirementPostwritesV1, ExactIndexPlaneErrorV1>
{
    let (indexed, sealed) = authenticate_join(authentication, true, true)?;
    let indexed = &indexed;
    indexed.validate().map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    input.market_binding.validate().map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    if indexed.index_state() != ExactIndexChildrenStateV1::Live
        || indexed.locator_account() != input.locator.account || indexed.adjacency_account() != input.adjacency.account
        || input.market_binding_account != indexed.base().market_binding()
        || sealed.authority.root_account == input.market_binding_account
        || sealed.authority.feed_account == input.market_binding_account
        || sealed.authority.locator_account == input.market_binding_account
        || sealed.authority.adjacency_account == input.market_binding_account
        || sealed.authority.locator_account != input.locator.account
        || sealed.authority.adjacency_account != input.adjacency.account
        || sealed.authority.feed_account != indexed.base().retained_feed()
        || input.locator.program_id != sealed.authority.program_id
        || input.locator.owner != input.locator.program_id || input.adjacency.owner != input.adjacency.program_id
        || input.locator.program_id != input.adjacency.program_id || !input.locator.writable
        || !input.adjacency.writable || input.locator.executable || input.adjacency.executable
        || input.market_binding.base().market != indexed.base().market()
        || input.market_binding.base().market_instance_v2_id
            != indexed.base().market_instance_v2_id()
        || input.market_binding.batch_policy_id() != indexed.base().batch_policy_id()
    { return Err(ExactIndexPlaneErrorV1::BindingMismatch); }
    let locator = decode_common(sealed.locator_body, FROZEN_ORDER_LOCATOR_MAGIC_V1)?;
    let adjacency = decode_common(sealed.adjacency_body, CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1)?;
    let sink = input.market_binding.base().neutral_sink;
    if !locator.semantic_eq(&adjacency)
        || locator.settlement_root_account != sealed.authority.root_account
        || locator.selected_feed_account != sealed.authority.feed_account
        || locator.selected_feed_data_id != sealed.authority.feed_full_data_id
        || indexed.selected_feed_data_id() != sealed.authority.feed_full_data_id
        || locator.traversal_binding_id != indexed.base().owner_order_set_digest()
        || locator.sibling_account != input.adjacency.account
        || adjacency.sibling_account != input.locator.account
        || locator.plane_id != indexed.plane_id()
        || sink == sealed.authority.root_account || sink == sealed.authority.feed_account
        || sink == input.locator.account || sink == input.adjacency.account
        || sink == input.market_binding_account
        || locator.rent.payer == sealed.authority.root_account
        || locator.rent.payer == sealed.authority.feed_account
        || locator.rent.payer == input.locator.account
        || locator.rent.payer == input.adjacency.account
        || locator.rent.payer == input.market_binding_account
        || adjacency.rent.payer == sealed.authority.root_account
        || adjacency.rent.payer == sealed.authority.feed_account
        || adjacency.rent.payer == input.locator.account
        || adjacency.rent.payer == input.adjacency.account
        || adjacency.rent.payer == input.market_binding_account
    { return Err(ExactIndexPlaneErrorV1::BindingMismatch); }
    let (locator_principal, locator_donation) = close_credits(locator.rent, input.locator.lamports, sink)?;
    let (adjacency_principal, adjacency_donation) = close_credits(adjacency.rent, input.adjacency.lamports, sink)?;
    let data_id = indexed
        .encode_retire_index_children_and_data_id(
            &CanonicalSha256,
            sealed.authority.root_account,
            root_output,
        )
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    Ok(CountedExactIndexRootRetirementPostwritesV1 {
        indexed_root_poststate_data_id: data_id,
        close: ExactIndexPlaneClosePostwritesV1 { locator_principal, locator_donation,
            adjacency_principal, adjacency_donation } })
}
fn close_credits(rent: DeletableRentOwnerV1, lamports: u64, sink: Id32)
    -> Result<(ExactIndexCloseCreditV1, ExactIndexCloseCreditV1), ExactIndexPlaneErrorV1>
{
    rent.validate().map_err(|_| ExactIndexPlaneErrorV1::InvalidRent)?;
    let minimum = rent.refundable_principal.checked_add(rent.donation_floor)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    if sink.is_zero() || sink == rent.payer || lamports < minimum { return Err(ExactIndexPlaneErrorV1::InvalidRent); }
    Ok((ExactIndexCloseCreditV1 { recipient: rent.payer, amount: rent.refundable_principal },
        ExactIndexCloseCreditV1 { recipient: sink,
            amount: lamports.checked_sub(rent.refundable_principal).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)? }))
}

#[derive(Clone, Copy, Debug)]
/// Exact accounts and bytes consumed by retained-Feed retirement.
pub struct RetireCountedExactFeedInputV1<'a> {
    /// Executing Dragon's Clutch program identity.
    pub program_id: Id32,
    /// Canonical indexed-root PDA.
    pub root_account: Id32,
    /// Exact hostile indexed-root body currently stored at `root_account`.
    pub root_body: &'a [u8],
    /// Canonical MarketBinding PDA.
    pub market_binding_account: Id32,
    /// Already authenticated MarketBinding value.
    pub market_binding: &'a MarketBindingV2,
    /// Retained Feed account selected by the root.
    pub feed_account: Id32,
    /// Exact hostile Feed body.
    pub feed_body: &'a [u8],
    /// Observed Feed lamports before close.
    pub feed_lamports: u64,
    /// Observed Feed owner.
    pub feed_owner: Id32,
    /// Whether the Feed account is writable.
    pub feed_writable: bool,
    /// Whether the Feed account is executable.
    pub feed_executable: bool,
    /// Keeper receiving exactly the prepaid Feed close reward.
    pub keeper_destination: Id32,
}

/// Compact root-write identity and exact Feed-close credits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactFeedRetirementPostwritesV1 {
    indexed_root_poststate_data_id: Id32,
    feed_principal: ExactIndexCloseCreditV1,
    feed_donation: ExactIndexCloseCreditV1,
    feed_keeper_reward: ExactIndexCloseCreditV1,
}

impl CountedExactFeedRetirementPostwritesV1 {
    /// Account-bound ID of the streamed terminal indexed root.
    pub const fn indexed_root_poststate_data_id(&self) -> Id32 {
        self.indexed_root_poststate_data_id
    }
    /// Exact refundable Feed rent principal credit.
    pub const fn feed_principal_credit(&self) -> ExactIndexCloseCreditV1 {
        self.feed_principal
    }
    /// Neutral-sink credit containing donation floor and late donations.
    pub const fn feed_donation_credit(&self) -> ExactIndexCloseCreditV1 {
        self.feed_donation
    }
    /// Exact immutable prepaid keeper reward credit.
    pub const fn feed_keeper_reward_credit(&self) -> ExactIndexCloseCreditV1 {
        self.feed_keeper_reward
    }
}

/// Authenticate and close the retained Feed only after both compact indexes
/// retired, while streaming the base-`Terminal` indexed-root successor.
///
/// This action-specific composer keeps later hostile donations out of keeper
/// rewards and rent principal: the exact prepaid reward goes to the caller,
/// the exact principal returns to its payer, and every other lamport goes to
/// the immutable MarketBinding neutral sink.
pub fn stream_retire_counted_exact_feed_v1(
    input: RetireCountedExactFeedInputV1<'_>,
    root_output: &mut [u8],
) -> Result<CountedExactFeedRetirementPostwritesV1, ExactIndexPlaneErrorV1> {
    let indexed_root = IndexedSettlementRootV1AccountV1::decode(input.root_body)
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    indexed_root.validate().map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    input.market_binding.validate().map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    if input.program_id.is_zero()
        || input.root_account.is_zero()
        || input.market_binding_account.is_zero()
        || input.feed_account.is_zero()
        || input.keeper_destination.is_zero()
        || indexed_root.index_state() != ExactIndexChildrenStateV1::Retired
        || indexed_root.base().phase()
            != clutch_general_v2_contract::SettlementRootPhaseV1::Retiring
        || indexed_root.base().retained_feed_state()
            != clutch_general_v2_contract::SettlementRootChildStateV1::Live
        || indexed_root.base().retained_feed() != input.feed_account
        || indexed_root.base().market_binding() != input.market_binding_account
        || input.market_binding.base().market != indexed_root.base().market()
        || input.market_binding.base().market_instance_v2_id
            != indexed_root.base().market_instance_v2_id()
        || input.market_binding.batch_policy_id() != indexed_root.base().batch_policy_id()
        || input.feed_owner != input.program_id
        || !input.feed_writable
        || input.feed_executable
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let physical = [input.root_account, input.market_binding_account, input.feed_account];
    let mut left = 0usize;
    while left < physical.len() {
        let mut right = left + 1;
        while right < physical.len() {
            if physical[left] == physical[right] {
                return Err(ExactIndexPlaneErrorV1::BindingMismatch);
            }
            right += 1;
        }
        left += 1;
    }
    let feed = authenticate_settlement_feed_view_v5(input.feed_account, input.feed_body)
        .map_err(|_| ExactIndexPlaneErrorV1::CandidateTraversal)?;
    let header = feed.header();
    if feed.data_id() != indexed_root.selected_feed_data_id()
        || feed.candidate_bundle_digest()
            != indexed_root.base().candidate_bundle_digest()
        || header.epoch != indexed_root.base().epoch()
        || header.node != indexed_root.base().source_admission_node()
        || header.market != indexed_root.base().market()
        || header.order_set != indexed_root.base().order_set()
        || header.settlement_candidate_id
            != indexed_root.base().settlement_candidate_id()
        || header.settlement_witness_digest
            != indexed_root.base().settlement_witness_digest()
        || header.epoch_generation != indexed_root.base().epoch_generation()
        || header.outcome_count != indexed_root.base().outcome_count()
        || header.order_count != indexed_root.base().order_count()
        || header.slice_count != indexed_root.base().counts().expected_receipts
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let sink = input.market_binding.base().neutral_sink;
    let rent = header.rent;
    let forbidden = [input.root_account, input.market_binding_account, input.feed_account];
    if forbidden.contains(&sink)
        || forbidden.contains(&rent.payer)
        || forbidden.contains(&input.keeper_destination)
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let (feed_principal, feed_donation, feed_keeper_reward) = feed_close_credits(
        rent,
        header.close_reward_lamports,
        input.feed_lamports,
        sink,
        input.keeper_destination,
    )?;
    let indexed_root_poststate_data_id = indexed_root
        .encode_retire_feed_and_finish_and_data_id(
            &CanonicalSha256,
            input.root_account,
            root_output,
        )
        .map_err(|_| ExactIndexPlaneErrorV1::RootBinding)?;
    Ok(CountedExactFeedRetirementPostwritesV1 {
        indexed_root_poststate_data_id,
        feed_principal,
        feed_donation,
        feed_keeper_reward,
    })
}

fn feed_close_credits(
    rent: DeletableRentOwnerV1,
    close_reward_lamports: u64,
    lamports: u64,
    sink: Id32,
    keeper: Id32,
) -> Result<
    (ExactIndexCloseCreditV1, ExactIndexCloseCreditV1, ExactIndexCloseCreditV1),
    ExactIndexPlaneErrorV1,
> {
    rent.validate().map_err(|_| ExactIndexPlaneErrorV1::InvalidRent)?;
    if close_reward_lamports == 0
        || sink.is_zero()
        || keeper.is_zero()
        || sink == rent.payer
    {
        return Err(ExactIndexPlaneErrorV1::InvalidRent);
    }
    let required = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .and_then(|value| value.checked_add(close_reward_lamports))
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    if lamports < required {
        return Err(ExactIndexPlaneErrorV1::InvalidRent);
    }
    let donation = lamports
        .checked_sub(rent.refundable_principal)
        .and_then(|value| value.checked_sub(close_reward_lamports))
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok((
        ExactIndexCloseCreditV1 { recipient: rent.payer, amount: rent.refundable_principal },
        ExactIndexCloseCreditV1 { recipient: sink, amount: donation },
        ExactIndexCloseCreditV1 { recipient: keeper, amount: close_reward_lamports },
    ))
}

#[derive(Clone, Copy, Debug)]
/// Exact hostile terminal indexed-root close inputs.
pub struct CloseCountedExactRootInputV1<'a> {
    /// Executing Dragon's Clutch program identity.
    pub program_id: Id32,
    /// Indexed-root account being closed.
    pub root_account: Id32,
    /// Exact hostile terminal indexed-root body.
    pub root_body: &'a [u8],
    /// Observed root lamports before close.
    pub root_lamports: u64,
    /// Observed root owner.
    pub root_owner: Id32,
    /// Whether the root account is writable.
    pub root_writable: bool,
    /// Whether the root account is executable.
    pub root_executable: bool,
    /// Canonical writable parent Epoch account.
    pub epoch_account: Id32,
    /// Exact hostile parent Epoch body.
    pub epoch_body: &'a [u8],
    /// Observed parent Epoch owner.
    pub epoch_owner: Id32,
    /// Whether the parent Epoch is writable.
    pub epoch_writable: bool,
    /// Whether the parent Epoch is executable.
    pub epoch_executable: bool,
    /// Canonical read-only finalized Window account.
    pub window_account: Id32,
    /// Exact hostile finalized Window body.
    pub window_body: &'a [u8],
    /// Observed finalized Window owner.
    pub window_owner: Id32,
    /// Whether the finalized Window is writable.
    pub window_writable: bool,
    /// Whether the finalized Window is executable.
    pub window_executable: bool,
    /// Canonical MarketBinding PDA.
    pub market_binding_account: Id32,
    /// Already authenticated MarketBinding value.
    pub market_binding: &'a MarketBindingV2,
}

/// Compact terminal handoff and exact indexed-root close credits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedExactRootClosePostwritesV1 {
    terminal: IndexedSettlementRootCloseProjectionV1,
    root_principal: ExactIndexCloseCreditV1,
    root_donation: ExactIndexCloseCreditV1,
}

impl CountedExactRootClosePostwritesV1 {
    /// Structural terminal receipt decoded from the exact root bytes.
    pub const fn terminal(&self) -> &IndexedSettlementRootCloseProjectionV1 {
        &self.terminal
    }
    /// Exact refundable root rent principal credit.
    pub const fn root_principal_credit(&self) -> ExactIndexCloseCreditV1 {
        self.root_principal
    }
    /// Neutral-sink credit containing donation floor and late donations.
    pub const fn root_donation_credit(&self) -> ExactIndexCloseCreditV1 {
        self.root_donation
    }
}

/// Hostile-decode the terminal indexed root, stream the parent Epoch's unique
/// selected-root decrement, and plan the root's exact final close.
///
/// The full 1,228-byte body is decoded and hashed inside the contract boundary;
/// only the compact terminal projection and exact credits cross back to SBF.
/// The finalized Window remains read-only and proves its historical selected
/// artifact is this exact root.
pub fn close_counted_exact_root_v1(
    input: CloseCountedExactRootInputV1<'_>,
    epoch_output: &mut [u8],
) -> Result<CountedExactRootClosePostwritesV1, ExactIndexPlaneErrorV1> {
    input.market_binding.validate().map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    if input.program_id.is_zero()
        || input.root_account.is_zero()
        || input.market_binding_account.is_zero()
        || input.epoch_account.is_zero()
        || input.window_account.is_zero()
        || input.root_owner != input.program_id
        || input.epoch_owner != input.program_id
        || input.window_owner != input.program_id
        || !input.root_writable
        || !input.epoch_writable
        || input.window_writable
        || input.root_executable
        || input.epoch_executable
        || input.window_executable
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let physical = [
        input.root_account,
        input.epoch_account,
        input.window_account,
        input.market_binding_account,
    ];
    let mut left = 0usize;
    while left < physical.len() {
        let mut right = left + 1;
        while right < physical.len() {
            if physical[left] == physical[right] {
                return Err(ExactIndexPlaneErrorV1::BindingMismatch);
            }
            right += 1;
        }
        left += 1;
    }
    let terminal = IndexedSettlementRootV1AccountV1::decode_terminal_close_projection(
        &CanonicalSha256,
        input.root_account,
        input.root_body,
    )
    .map_err(|_| ExactIndexPlaneErrorV1::NonTerminalRoot)?;
    if terminal.terminal().base().root_account() != input.root_account
        || terminal.market_binding() != input.market_binding_account
        || terminal.terminal().base().epoch() != input.epoch_account
        || terminal.terminal().base().market() != input.market_binding.base().market
        || terminal.terminal().base().market_instance_v2_id()
            != input.market_binding.base().market_instance_v2_id
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let sink = input.market_binding.base().neutral_sink;
    let rent = terminal.root_rent();
    if sink == input.root_account
        || sink == input.epoch_account
        || sink == input.window_account
        || sink == input.market_binding_account
        || rent.payer == input.root_account
        || rent.payer == input.epoch_account
        || rent.payer == input.window_account
        || rent.payer == input.market_binding_account
    {
        return Err(ExactIndexPlaneErrorV1::BindingMismatch);
    }
    let (root_principal, root_donation) =
        close_credits(rent, input.root_lamports, sink)?;
    let epoch = GeneralEpochV6AccountV1::decode(input.epoch_body)
        .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    let window = CandidateWindowV5AccountV1::decode(input.window_body)
        .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    encode_retire_indexed_settlement_root_v1(
        &terminal,
        input.epoch_account,
        &epoch,
        input.window_account,
        &window,
        epoch_output,
    )
    .map_err(|_| ExactIndexPlaneErrorV1::BindingMismatch)?;
    Ok(CountedExactRootClosePostwritesV1 {
        terminal,
        root_principal,
        root_donation,
    })
}

pub fn sealed_locator_data_id_from_raw_v1(body: &[u8]) -> Result<Id32, ExactIndexPlaneErrorV1> {
    let common = decode_common(body, FROZEN_ORDER_LOCATOR_MAGIC_V1)?;
    if body.len() != locator_data_len_v1(common.order_count)? { return Err(ExactIndexPlaneErrorV1::WrongLength); }
    hash_id(FROZEN_ORDER_LOCATOR_DATA_ID_DOMAIN_V1, body)
}
pub fn sealed_adjacency_data_id_from_raw_v1(body: &[u8]) -> Result<Id32, ExactIndexPlaneErrorV1> {
    let common = decode_common(body, CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1)?;
    if body.len() != adjacency_data_len_v1(common.order_count, common.slice_reference_count)? {
        return Err(ExactIndexPlaneErrorV1::WrongLength);
    }
    hash_id(CANDIDATE_ORDER_SLICE_INDEX_DATA_ID_DOMAIN_V1, body)
}
fn hash_id(domain: &[u8], body: &[u8]) -> Result<Id32, ExactIndexPlaneErrorV1> {
    let mut h = Sha256::new(); h.update(domain); h.update(body); finish_id(h)
}
fn finish_id(h: Sha256) -> Result<Id32, ExactIndexPlaneErrorV1> {
    Id32::new(h.finalize().into()).map_err(|_| ExactIndexPlaneErrorV1::ZeroIdentity)
}

fn encode_common(body: &mut [u8], magic: [u8; 8], common: ExactIndexCommonV1)
    -> Result<(), ExactIndexPlaneErrorV1>
{
    common.validate()?;
    let mut w = Writer::new(body.get_mut(..EXACT_INDEX_COMMON_HEADER_BYTES_V1)
        .ok_or(ExactIndexPlaneErrorV1::WrongLength)?);
    w.bytes(&magic)?; w.u8(EXACT_INDEX_PLANE_VERSION_V1)?; w.u8(EXACT_INDEX_PLANE_STATE_SEALED_V1)?;
    w.u8(common.stored_bump)?; w.u8(0)?; w.bytes(&[0; 4])?;
    for id in [common.settlement_root_account, common.sibling_account, common.plane_id,
        common.selected_feed_account, common.selected_feed_data_id, common.traversal_binding_id]
    { w.id(id)?; }
    w.u8(common.order_count)?; w.u8(common.outcome_count)?; w.u16(common.slice_count)?;
    w.u16(common.slice_reference_count)?; w.u8(common.page_count)?; w.u8(0)?;
    w.bytes(&common.page_physical_slot_counts)?; w.bytes(&[0; 4])?; w.id(common.rent.payer)?;
    w.u64(common.rent.refundable_principal)?; w.u64(common.rent.donation_floor)?; w.finish()
}
fn decode_common(body: &[u8], magic: [u8; 8]) -> Result<ExactIndexCommonV1, ExactIndexPlaneErrorV1> {
    let mut r = Reader::new(body.get(..EXACT_INDEX_COMMON_HEADER_BYTES_V1)
        .ok_or(ExactIndexPlaneErrorV1::WrongLength)?);
    if r.array::<8>()? != magic || r.u8()? != EXACT_INDEX_PLANE_VERSION_V1
        || r.u8()? != EXACT_INDEX_PLANE_STATE_SEALED_V1 { return Err(ExactIndexPlaneErrorV1::InvalidState); }
    let stored_bump = r.u8()?; r.reserved(5)?;
    let settlement_root_account = r.id()?; let sibling_account = r.id()?; let plane_id = r.id()?;
    let selected_feed_account = r.id()?; let selected_feed_data_id = r.id()?;
    let traversal_binding_id = r.id()?; let order_count = r.u8()?; let outcome_count = r.u8()?;
    let slice_count = r.u16()?; let slice_reference_count = r.u16()?; let page_count = r.u8()?;
    r.reserved(1)?; let page_physical_slot_counts = r.array::<4>()?; r.reserved(4)?;
    let rent = DeletableRentOwnerV1 { payer: r.id()?, refundable_principal: r.u64()?, donation_floor: r.u64()? };
    r.finish()?;
    let value = ExactIndexCommonV1 { settlement_root_account, sibling_account, plane_id,
        selected_feed_account, selected_feed_data_id, traversal_binding_id, order_count,
        outcome_count, slice_count, slice_reference_count, page_count,
        page_physical_slot_counts,
        rent, stored_bump };
    value.validate()?; Ok(value)
}

fn locator_range(order: usize) -> Result<core::ops::Range<usize>, ExactIndexPlaneErrorV1> {
    let start = EXACT_INDEX_COMMON_HEADER_BYTES_V1.checked_add(order.checked_mul(4)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok(start..start + 4)
}
fn directory_range(order: usize) -> Result<core::ops::Range<usize>, ExactIndexPlaneErrorV1> {
    let start = EXACT_INDEX_COMMON_HEADER_BYTES_V1.checked_add(order.checked_mul(8)
        .ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok(start..start + 8)
}
fn reference_range(order_count: u8, reference: u16) -> Result<core::ops::Range<usize>, ExactIndexPlaneErrorV1> {
    let start = EXACT_INDEX_COMMON_HEADER_BYTES_V1.checked_add(usize::from(order_count) * 8)
        .and_then(|v| v.checked_add(usize::from(reference) * 2)).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
    Ok(start..start + 2)
}
fn write_locator(body: &mut [u8], order: usize, row: FrozenOrderLocatorRowV1) -> Result<(), ExactIndexPlaneErrorV1> {
    let out = body.get_mut(locator_range(order)?).ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
    out[0..2].copy_from_slice(&row.page_index.to_le_bytes()); out[2] = row.page_slot; out[3] = 0; Ok(())
}
fn read_locator(body: &[u8], order: usize) -> Result<FrozenOrderLocatorRowV1, ExactIndexPlaneErrorV1> {
    let i = body.get(locator_range(order)?).ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
    if i[3] != 0 { return Err(ExactIndexPlaneErrorV1::InvalidLocator); }
    Ok(FrozenOrderLocatorRowV1 { page_index: u16::from_le_bytes([i[0], i[1]]), page_slot: i[2] })
}
fn write_directory(body: &mut [u8], order: usize, row: CandidateOrderDirectoryRowV1) -> Result<(), ExactIndexPlaneErrorV1> {
    let out = body.get_mut(directory_range(order)?).ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
    out[0..2].copy_from_slice(&row.first_slice_ref.to_le_bytes());
    out[2..4].copy_from_slice(&row.slice_ref_count.to_le_bytes()); out[4] = row.side as u8;
    out[5..8].fill(0); Ok(())
}
fn read_directory(body: &[u8], order: usize) -> Result<CandidateOrderDirectoryRowV1, ExactIndexPlaneErrorV1> {
    let i = body.get(directory_range(order)?).ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
    if i[5..8].iter().any(|b| *b != 0) { return Err(ExactIndexPlaneErrorV1::InvalidAdjacency); }
    Ok(CandidateOrderDirectoryRowV1 { first_slice_ref: u16::from_le_bytes([i[0], i[1]]),
        slice_ref_count: u16::from_le_bytes([i[2], i[3]]), side: ExactIndexOrderSideV1::decode(i[4])? })
}
fn read_reference(body: &[u8], order_count: u8, reference: u16) -> Result<u16, ExactIndexPlaneErrorV1> {
    let i = body.get(reference_range(order_count, reference)?).ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
    Ok(u16::from_le_bytes([i[0], i[1]]))
}

struct Writer<'a> { output: &'a mut [u8], at: usize }
impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self { Self { output, at: 0 } }
    fn bytes(&mut self, value: &[u8]) -> Result<(), ExactIndexPlaneErrorV1> {
        let end = self.at.checked_add(value.len()).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        self.output.get_mut(self.at..end).ok_or(ExactIndexPlaneErrorV1::WrongLength)?.copy_from_slice(value);
        self.at = end; Ok(())
    }
    fn id(&mut self, v: Id32) -> Result<(), ExactIndexPlaneErrorV1> { self.bytes(&v.bytes()) }
    fn u8(&mut self, v: u8) -> Result<(), ExactIndexPlaneErrorV1> { self.bytes(&[v]) }
    fn u16(&mut self, v: u16) -> Result<(), ExactIndexPlaneErrorV1> { self.bytes(&v.to_le_bytes()) }
    fn u64(&mut self, v: u64) -> Result<(), ExactIndexPlaneErrorV1> { self.bytes(&v.to_le_bytes()) }
    fn finish(self) -> Result<(), ExactIndexPlaneErrorV1> {
        if self.at == self.output.len() { Ok(()) } else { Err(ExactIndexPlaneErrorV1::WrongLength) }
    }
}
struct Reader<'a> { input: &'a [u8], at: usize }
impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Self { Self { input, at: 0 } }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], ExactIndexPlaneErrorV1> {
        let end = self.at.checked_add(N).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        let i = self.input.get(self.at..end).ok_or(ExactIndexPlaneErrorV1::WrongLength)?;
        let mut out = [0u8; N]; out.copy_from_slice(i); self.at = end; Ok(out)
    }
    fn id(&mut self) -> Result<Id32, ExactIndexPlaneErrorV1> {
        Id32::new(self.array::<32>()?).map_err(|_| ExactIndexPlaneErrorV1::ZeroIdentity)
    }
    fn u8(&mut self) -> Result<u8, ExactIndexPlaneErrorV1> { Ok(self.array::<1>()?[0]) }
    fn u16(&mut self) -> Result<u16, ExactIndexPlaneErrorV1> { Ok(u16::from_le_bytes(self.array::<2>()?)) }
    fn u64(&mut self) -> Result<u64, ExactIndexPlaneErrorV1> { Ok(u64::from_le_bytes(self.array::<8>()?)) }
    fn reserved(&mut self, len: usize) -> Result<(), ExactIndexPlaneErrorV1> {
        let end = self.at.checked_add(len).ok_or(ExactIndexPlaneErrorV1::ArithmeticOverflow)?;
        if self.input.get(self.at..end).ok_or(ExactIndexPlaneErrorV1::WrongLength)?.iter().any(|b| *b != 0) {
            return Err(ExactIndexPlaneErrorV1::InvalidState);
        }
        self.at = end; Ok(())
    }
    fn finish(self) -> Result<(), ExactIndexPlaneErrorV1> {
        if self.at == self.input.len() { Ok(()) } else { Err(ExactIndexPlaneErrorV1::WrongLength) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> Id32 {
        Id32::new([value; 32]).expect("nonzero test identity")
    }

    fn common() -> ExactIndexCommonV1 {
        ExactIndexCommonV1 {
            settlement_root_account: id(1),
            sibling_account: id(2),
            plane_id: id(3),
            selected_feed_account: id(4),
            selected_feed_data_id: id(5),
            traversal_binding_id: id(6),
            order_count: 2,
            outcome_count: 2,
            slice_count: 2,
            slice_reference_count: 3,
            page_count: 1,
            // Three physical slots contain two live orders and one tombstone.
            page_physical_slot_counts: [3, 0, 0, 0],
            rent: DeletableRentOwnerV1 {
                payer: id(7),
                refundable_principal: 11,
                donation_floor: 5,
            },
            stored_bump: 9,
        }
    }

    #[test]
    fn maximum_compact_account_widths_remain_below_four_kibibytes() {
        assert_eq!(locator_data_len_v1(MAX_ORDERS as u8), Ok(528));
        assert_eq!(
            adjacency_data_len_v1(MAX_ORDERS as u8, MAX_EXACT_INDEX_SLICE_REFERENCES_V1 as u16),
            Ok(2_448),
        );
        assert!(FROZEN_ORDER_LOCATOR_MAX_BYTES_V1 <= 4_096);
        assert!(CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1 <= 4_096);
    }

    #[test]
    fn physical_page_width_is_not_dense_live_order_count() {
        let value = common();
        assert_eq!(value.order_count, 2);
        assert_eq!(value.page_physical_slot_counts[0], 3);
        assert_eq!(value.validate(), Ok(()));
        assert_eq!(
            ExactIndexCommonV1 {
                page_physical_slot_counts: [17, 0, 0, 0],
                ..value
            }
            .validate(),
            Err(ExactIndexPlaneErrorV1::InvalidCount),
        );
        assert_eq!(
            ExactIndexCommonV1 {
                slice_reference_count: 5,
                ..value
            }
            .validate(),
            Err(ExactIndexPlaneErrorV1::InvalidCount),
        );
    }

    #[test]
    fn grouped_references_are_compact_and_full_body_id_detects_tamper() {
        let value = common();
        let mut body = vec![0u8; adjacency_data_len_v1(2, 3).expect("length")];
        encode_common(&mut body, CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1, value)
            .expect("header");
        write_directory(&mut body, 0, CandidateOrderDirectoryRowV1 {
            first_slice_ref: 0,
            slice_ref_count: 2,
            side: ExactIndexOrderSideV1::Buy,
        }).expect("buy directory");
        write_directory(&mut body, 1, CandidateOrderDirectoryRowV1 {
            first_slice_ref: 2,
            slice_ref_count: 1,
            side: ExactIndexOrderSideV1::Sell,
        }).expect("sell directory");
        for (at, reference) in [0u16, 1, 1].into_iter().enumerate() {
            body[reference_range(2, at as u16).expect("reference range")]
                .copy_from_slice(&reference.to_le_bytes());
        }
        assert_eq!(read_reference(&body, 2, 0), Ok(0));
        assert_eq!(read_reference(&body, 2, 1), Ok(1));
        assert_eq!(read_reference(&body, 2, 2), Ok(1));
        let before = sealed_adjacency_data_id_from_raw_v1(&body).expect("sealed ID");
        let last = body.len() - 1;
        body[last] ^= 1;
        let after = sealed_adjacency_data_id_from_raw_v1(&body).expect("tampered ID");
        assert_ne!(before, after);
    }

    #[test]
    fn fresh_child_principal_never_uses_prefund_as_discount() {
        let create = ExactIndexCreateAccountInputV1 {
            account: id(8),
            program_id: id(9),
            payer: id(10),
            payer_lamports: 100,
            target_lamports: 17,
            target_owner: Id32::ZERO,
            target_data_len: 0,
            target_writable: true,
            target_executable: false,
            rent_exempt_minimum: 31,
            stored_bump: 4,
        };
        assert_eq!(
            create.validate(528, &[]),
            Ok(DeletableRentOwnerV1 {
                payer: id(10),
                refundable_principal: 31,
                donation_floor: 17,
            }),
        );
        assert_eq!(
            ExactIndexCreateAccountInputV1 { payer_lamports: 30, ..create }
                .validate(528, &[]),
            Err(ExactIndexPlaneErrorV1::InvalidCreateAccount),
        );
        assert_eq!(
            create.validate(528, &[id(8)]),
            Err(ExactIndexPlaneErrorV1::InvalidCreateAccount),
        );
    }

    #[test]
    fn repeated_same_pair_outcome_decomposition_exceeds_outcome_width() {
        const REPEATS: u16 = 17;
        let mut value = common();
        value.slice_count = REPEATS;
        value.slice_reference_count = REPEATS * 2;
        let mut body = vec![0u8; adjacency_data_len_v1(2, REPEATS * 2).expect("length")];
        encode_common(&mut body, CANDIDATE_ORDER_SLICE_INDEX_MAGIC_V1, value)
            .expect("header");
        let buy = CandidateOrderDirectoryRowV1 {
            first_slice_ref: 0,
            slice_ref_count: REPEATS,
            side: ExactIndexOrderSideV1::Buy,
        };
        let sell = CandidateOrderDirectoryRowV1 {
            first_slice_ref: REPEATS,
            slice_ref_count: REPEATS,
            side: ExactIndexOrderSideV1::Sell,
        };
        write_directory(&mut body, 0, buy).expect("buy directory");
        write_directory(&mut body, 1, sell).expect("sell directory");
        let mut index = 0u16;
        while index < REPEATS {
            for at in [index, REPEATS + index] {
                body[reference_range(2, at).expect("reference range")]
                    .copy_from_slice(&index.to_le_bytes());
            }
            index += 1;
        }
        let mut slices = vec![0u8; usize::from(REPEATS) * SETTLEMENT_SLICE_BYTES];
        index = 0;
        while index < REPEATS {
            let at = usize::from(index) * SETTLEMENT_SLICE_BYTES;
            slices[at] = SettlementSliceLegKindV1::Order as u8;
            slices[at + 1] = 0;
            slices[at + 2] = SettlementSliceLegKindV1::Order as u8;
            slices[at + 3] = 1;
            slices[at + 4] = 0;
            slices[at + 5..at + 13].copy_from_slice(&1u64.to_le_bytes());
            index += 1;
        }
        let mut pair = [0u16; MAX_SLICES];
        let (count, total, elsewhere) = scan_group(
            &body, &value, &slices, 0, 1, buy, true, &mut pair,
        ).expect("all repeated slices are valid");
        assert_eq!(count, REPEATS);
        assert_eq!(total, u64::from(REPEATS));
        assert!(!elsewhere);
    }

    #[test]
    fn close_splits_exact_principal_and_all_nonprincipal_lamports() {
        let rent = DeletableRentOwnerV1 {
            payer: id(11),
            refundable_principal: 31,
            donation_floor: 17,
        };
        let (principal, donation) = close_credits(rent, 53, id(12)).expect("close credits");
        assert_eq!(principal.recipient(), id(11));
        assert_eq!(principal.amount(), 31);
        assert_eq!(donation.recipient(), id(12));
        assert_eq!(donation.amount(), 22);
        assert_eq!(
            close_credits(rent, 47, id(12)),
            Err(ExactIndexPlaneErrorV1::InvalidRent),
        );
        assert_eq!(
            close_credits(rent, 53, id(11)),
            Err(ExactIndexPlaneErrorV1::InvalidRent),
        );
    }

    #[test]
    fn feed_close_preserves_reward_and_routes_late_donations() {
        let rent = DeletableRentOwnerV1 {
            payer: id(11),
            refundable_principal: 31,
            donation_floor: 7,
        };
        let (principal, donation, keeper) =
            feed_close_credits(rent, 5, 53, id(12), id(13)).expect("feed credits");
        assert_eq!((principal.recipient(), principal.amount()), (id(11), 31));
        assert_eq!((donation.recipient(), donation.amount()), (id(12), 17));
        assert_eq!((keeper.recipient(), keeper.amount()), (id(13), 5));
        assert_eq!(principal.amount() + donation.amount() + keeper.amount(), 53);
        assert_eq!(
            feed_close_credits(rent, 5, 42, id(12), id(13)),
            Err(ExactIndexPlaneErrorV1::InvalidRent),
        );
        // Credit roles may intentionally coalesce; only source-account aliases
        // are forbidden by the action composer.
        assert!(feed_close_credits(rent, 5, 43, id(11), id(11)).is_ok());
    }
}
