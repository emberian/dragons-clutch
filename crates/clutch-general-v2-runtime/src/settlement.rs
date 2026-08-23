// SPDX-License-Identifier: AGPL-3.0-or-later

//! SBF-neutral construction of the settlement half of a General V2 feed.
//!
//! This module owns no account address, outer discriminator, capability, or
//! value movement. It derives canonical settlement slices and owner rows from
//! the exact frozen page projection, checked RelationV2 candidate, exact
//! reservation bodies, and private fee-runtime outputs. The eventual SBF
//! adapter must still authenticate account owners/PDAs and commit all returned
//! transitions atomically.

use core::cmp::min;

use clutch_batch::{Side, MAX_ORDERS};
use clutch_collateral_adapter_v2::{BoundCollateralProfileV2, Error as CollateralError};
use clutch_fee_runtime_contract::allocation::{FeeEnvelopeV1, PayerAllocationV1};
use clutch_fee_runtime_contract::intent::OwnerFeeTransitionIntentV1;
use clutch_fee_runtime_contract::projection::{
    assemble_selected_owner_fee_book_v1, project_terminal_owner_fee_v1,
    AuthenticatedSelectedOwnerFeeV1, SelectedOwnerFeeBookV1, VerifiedOwnerFeeFundingV1,
};
use clutch_fee_runtime_contract::selected::{
    OwnerFeeAssessmentV1, OwnerFeeCarryV1, SelectedCompositeFeeV1,
};
use clutch_fee_runtime_contract::{Error as FeeError, Id as FeeId, MAX_FEE_ROWS_V1};
use clutch_general_v2_contract::{
    candidate_bundle_digest_v1 as contract_candidate_bundle_digest_v1,
    project_general_position_replay_prestate_v1, project_general_replay_transition_v1,
    settlement_witness_digest_v1 as contract_settlement_witness_digest_v1, CandidateFeedHeaderV2,
    AccountReceiptEndPayloadV1, AuthenticatedSelectedCandidateV1, DeletableRentOwnerV1,
    EconomicDomainV2AccountV1, GeneralReplayTransitionKindV1, GeneralReplayTransitionPlanV1,
    Id32, MarketBindingV1, MAX_OUTCOMES, MAX_SLICES, SETTLEMENT_SLICE_BYTES,
};
use clutch_owner_settlement::{
    build_owner_settlement_book_v2, derive_settlement_receipt_data_id_v2,
    owner_rounding_residue_price_units, project_owner_receipt_end_v2,
    AuthenticatedOrderMembershipV2, AuthenticatedPositionV3,
    AuthenticatedSettlementReceiptEndV2,
    AuthenticatedSettlementReceiptV2, CandidateSettlementTotalsV2,
    Error as OwnerSettlementError, OrderKindV1, OwnerSettlementAccumulatorV2,
    OwnerSettlementAccountProjectionV2, OwnerSettlementBookV2, PresentConsiderationV2,
    PresentPriceV2, SelectedOwnerFeeV1, SettlementCashPotExpectationV1,
    SettlementReceiptDataHashV2, SettlementReceiptRouteV2, SettlementSideV1,
    VerifiedSettlementOrderV2, VirtualCashDirectionV1,
    OWNER_SETTLEMENT_BODY_V2_BYTES,
};
use clutch_retirement::{
    project_general_position_v3, AdapterPositionMarketBindingV3, AdapterPositionPurposeBindingV3,
    Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionV3Sha256Backend,
    ReplayV3HashBackend, RetirementErrorV2,
};
use clutch_solana_layout::reservation::{
    ReservationAccount, ReservationPlan, RESERVATION_ACCOUNT_BYTES, RESERVATION_STATE_ACTIVE,
    RESERVATION_STATE_ENTITLED,
};
use clutch_solana_layout::CodecError as LayoutError;
use sha2::{Digest, Sha256};

use crate::{
    BuiltDirectCandidateV1, CandidateBuilderErrorV1, CandidateSearchReportV1, CanonicalSha256,
    FrozenOrderKindV1, OwnerBlindBookProjectionV1, SettlementTailV1,
};

/// SHA-256 domain for the exact owner/order membership projection.
pub const OWNER_ORDER_SET_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/owner-order-set/v1\0";
/// Maximum encoded settlement-tail width.
pub const MAX_SETTLEMENT_TAIL_BYTES_V1: usize = MAX_SLICES * SETTLEMENT_SLICE_BYTES;
/// Maximum real receipt ends: two for every direct slice.
pub const MAX_SETTLEMENT_RECEIPT_ENDS_V1: usize = MAX_SLICES * 2;

const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_FEE_ROWS_V1 == MAX_ORDERS);
const _: () = assert!(SETTLEMENT_SLICE_BYTES == 13);

/// Exact reservation funding authenticated for every live frozen order.
///
/// Construction decodes the full reservation body and recomputes its envelope
/// from the page record. The outer account owner/PDA remains an explicit SBF
/// adapter obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedReservationBookV1 {
    market: Id32,
    epoch: Id32,
    order_set: Id32,
    price_grid_id: Id32,
    terms: Id32,
    reservation_policy: Id32,
    reservation_ids: [Id32; MAX_ORDERS],
    position_generations: [u64; MAX_ORDERS],
    reserved_cash_atoms: [u64; MAX_ORDERS],
    maximum_fee_atoms: [u64; MAX_ORDERS],
    order_count: u8,
}

/// Exact Position V3 account body presented by the SBF adapter.
///
/// The pure runtime derives every semantic field and the canonical data ID
/// from `encoded_body`. The adapter remains responsible for authenticating
/// `account` as the Position V3 PDA owned by the program before calling this
/// module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionAccountInputV3<'a> {
    /// SBF-authenticated Position V3 account identity.
    pub account: Id32,
    /// Exact canonical `PositionAccountV3` bytes.
    pub encoded_body: &'a [u8],
}

/// One private checked Position V3 row keyed by semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSettlementPositionV3 {
    account: Id32,
    data_id: Id32,
    position: PositionAccountV3,
}

impl AuthenticatedSettlementPositionV3 {
    /// SBF-authenticated Position V3 account identity.
    pub const fn account(&self) -> Id32 {
        self.account
    }

    /// Contract-owned digest of the exact canonical Position V3 body.
    pub const fn data_id(&self) -> Id32 {
        self.data_id
    }

    /// Exact decoded canonical Position V3 body.
    pub const fn position(&self) -> PositionAccountV3 {
        self.position
    }
}

/// Complete Position V3 set for every distinct owner in a frozen order set.
///
/// The private rows bind full MarketInstanceV2, Realm, collateral policy,
/// collateral release, General purpose, controller, replay, purpose binding,
/// generation, cash, and native-Egg state. No lowered legacy MarketId or
/// caller-authored balance summary enters the projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSettlementPositionBookV3 {
    market_runtime: Id32,
    market_binding: AdapterPositionMarketBindingV3,
    rows: [Option<AuthenticatedSettlementPositionV3>; MAX_ORDERS],
    owner_count: u8,
}

impl AuthenticatedSettlementPositionBookV3 {
    /// General V2 MarketRuntime identity kept separate from MarketInstanceV2.
    pub const fn market_runtime(&self) -> Id32 {
        self.market_runtime
    }

    /// Exact full-width MarketInstanceV2/Realm/policy/release binding.
    pub const fn market_binding(&self) -> AdapterPositionMarketBindingV3 {
        self.market_binding
    }

    /// Number of distinct Position owners in the complete frozen order set.
    pub const fn owner_count(&self) -> u8 {
        self.owner_count
    }

    /// Checked Position V3 row for one semantic owner.
    pub fn position_for_owner(&self, owner: Id32) -> Option<&AuthenticatedSettlementPositionV3> {
        let mut index = 0usize;
        while index < usize::from(self.owner_count) {
            if let Some(row) = &self.rows[index] {
                if row.position.owner().bytes() == owner.bytes() {
                    return Some(row);
                }
            }
            index += 1;
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PositionBodySha256V3;

impl PositionV3Sha256Backend for PositionBodySha256V3 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(domain);
        hash.update(body);
        hash.finalize().into()
    }
}

impl ReplayV3HashBackend for PositionBodySha256V3 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        let mut index = 0usize;
        while index < parts.len() {
            hash.update(parts[index]);
            index += 1;
        }
        hash.finalize().into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReceiptDataSha256V2;

impl SettlementReceiptDataHashV2 for ReceiptDataSha256V2 {
    fn sha256(&self, domain: &[u8], transcript: &[u8]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(domain);
        hash.update(transcript);
        hash.finalize().into()
    }
}

impl AuthenticatedReservationBookV1 {
    /// Exact frozen order-set identity.
    pub const fn order_set(&self) -> Id32 {
        self.order_set
    }

    /// Active live-order width.
    pub const fn order_count(&self) -> u8 {
        self.order_count
    }

    /// Exact authenticated placement/settlement terms identity.
    pub const fn terms(&self) -> Id32 {
        self.terms
    }

    /// Exact authenticated reservation-policy identity.
    pub const fn reservation_policy(&self) -> Id32 {
        self.reservation_policy
    }

    /// Canonical reservation identity at one live-order index.
    pub fn reservation_id(&self, order_index: u8) -> Option<Id32> {
        if order_index < self.order_count {
            Some(self.reservation_ids[usize::from(order_index)])
        } else {
            None
        }
    }

    /// Position generation authenticated by the reservation body.
    ///
    /// The SBF adapter must still match this value to the writable Position
    /// account whose owner was retained by the frozen order projection.
    pub fn position_generation(&self, order_index: u8) -> Option<u64> {
        if order_index < self.order_count {
            Some(self.position_generations[usize::from(order_index)])
        } else {
            None
        }
    }

    /// Exact initial buy-cash envelope; zero for a sell order.
    pub fn reserved_cash_atoms(&self, order_index: u8) -> Option<u64> {
        if order_index < self.order_count {
            Some(self.reserved_cash_atoms[usize::from(order_index)])
        } else {
            None
        }
    }

    /// Signed fee ceiling retained in the exact reservation body.
    pub fn maximum_fee_atoms(&self, order_index: u8) -> Option<u64> {
        if order_index < self.order_count {
            Some(self.maximum_fee_atoms[usize::from(order_index)])
        } else {
            None
        }
    }
}

/// Decode and bind exactly one active reservation for every live order.
///
/// Ordering is the dense live-order order used by RelationV2 and CandidateFeed
/// fills. Missing, duplicate, reordered, released, or already-consumed
/// reservation bodies are refused. `expected_terms` and
/// `expected_reservation_policy` must be projected from the adapter-
/// authenticated Market/Epoch parents; neither is an economic summary.
pub fn authenticate_reservation_book_v1(
    projection: &OwnerBlindBookProjectionV1,
    expected_terms: Id32,
    expected_reservation_policy: Id32,
    encoded_reservations: &[&[u8]],
) -> Result<AuthenticatedReservationBookV1, SettlementAdapterErrorV1> {
    let order_count = projection.book().len;
    if expected_terms.is_zero()
        || expected_reservation_policy.is_zero()
        || encoded_reservations.len() != usize::from(order_count)
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    let mut reservation_ids = [Id32::ZERO; MAX_ORDERS];
    let mut position_generations = [0u64; MAX_ORDERS];
    let mut reserved_cash_atoms = [0u64; MAX_ORDERS];
    let mut maximum_fee_atoms = [0u64; MAX_ORDERS];
    let mut index = 0usize;
    while index < usize::from(order_count) {
        let reservation = ReservationAccount::decode(encoded_reservations[index])?;
        let order_index =
            u8::try_from(index).map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
        let membership = projection
            .order_membership(order_index)
            .ok_or(SettlementAdapterErrorV1::ReservationSetMismatch)?;
        let economic_order = projection.book().orders[index];
        let expected_side = match economic_order.side {
            Side::Buy => 0,
            Side::Sell => 1,
        };
        let expected_kind = match membership.kind() {
            FrozenOrderKindV1::Single => 1,
            FrozenOrderKindV1::Portfolio => 2,
        };
        let expected_plan = ReservationPlan::for_order(
            membership.slot(),
            projection.domain().outcome_count,
            projection.domain().price_scale,
            reservation.max_fee_atoms,
        )?;
        if reservation.market.bytes() != projection.market().bytes()
            || reservation.epoch.bytes() != projection.epoch().bytes()
            || reservation.owner.bytes() != membership.owner().bytes()
            || reservation.order_id.bytes() != membership.order_id().bytes()
            || reservation.order_generation != membership.generation()
            || reservation.price_grid.bytes() != projection.price_grid_id().bytes()
            || reservation.terms.bytes() != expected_terms.bytes()
            || reservation.policy.bytes() != expected_reservation_policy.bytes()
            || reservation.outcome_count != projection.domain().outcome_count
            || reservation.side != expected_side
            || reservation.order_kind != expected_kind
            || reservation.state != RESERVATION_STATE_ACTIVE
            || reservation.initial_cash_atoms != expected_plan.cash_atoms
            || reservation.remaining_cash_atoms != expected_plan.cash_atoms
            || reservation.max_fee_atoms != expected_plan.max_fee_atoms
            || reservation.initial_internal != expected_plan.internal
            || reservation.remaining_internal != expected_plan.internal
        {
            return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
        }
        reservation_ids[index] = Id32::new(reservation.reservation.bytes())?;
        position_generations[index] = reservation.position_generation;
        reserved_cash_atoms[index] = reservation.initial_cash_atoms;
        maximum_fee_atoms[index] = reservation.max_fee_atoms;
        index += 1;
    }
    Ok(AuthenticatedReservationBookV1 {
        market: projection.market(),
        epoch: projection.epoch(),
        order_set: projection.order_set(),
        price_grid_id: projection.price_grid_id(),
        terms: expected_terms,
        reservation_policy: expected_reservation_policy,
        reservation_ids,
        position_generations,
        reserved_cash_atoms,
        maximum_fee_atoms,
        order_count,
    })
}

/// Decode and bind the exact Position V3 set for every distinct frozen owner.
///
/// `collateral` must be the private checked result of the canonical V2
/// Realm/Profile/policy/release join. The full MarketInstanceV2 and Realm are
/// rejoined to the General book; policy and release IDs are derived from that
/// checked capability. Position rows must appear in ascending owner order and
/// exactly cover the frozen order set. Their generations must agree with every
/// corresponding reservation.
pub fn authenticate_settlement_position_book_v3(
    projection: &OwnerBlindBookProjectionV1,
    reservations: &AuthenticatedReservationBookV1,
    collateral: BoundCollateralProfileV2,
    inputs: &[PositionAccountInputV3<'_>],
) -> Result<AuthenticatedSettlementPositionBookV3, SettlementAdapterErrorV1> {
    if reservations.market != projection.market()
        || reservations.epoch != projection.epoch()
        || reservations.order_set != projection.order_set()
        || reservations.order_count != projection.book().len
        || collateral.market().market.bytes()
            != projection.market_binding().market_instance_v2_id.bytes()
        || collateral.realm_bound().realm().realm.bytes() != projection.realm().bytes()
    {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }
    let market_binding = AdapterPositionMarketBindingV3 {
        market_instance_id: retirement_identity(projection.market_binding().market_instance_v2_id)?,
        outcome_count: projection.domain().outcome_count,
        realm_id: retirement_identity(projection.realm())?,
        collateral_policy_id: retirement_identity(Id32::new(collateral.policy_id().bytes())?)?,
        collateral_release_id: retirement_identity(Id32::new(collateral.release().id()?.bytes())?)?,
    };

    let mut expected_owners = [Id32::ZERO; MAX_ORDERS];
    let mut expected_owner_count = 0usize;
    let mut order = 0usize;
    while order < usize::from(projection.book().len) {
        let membership = projection
            .order_membership(
                u8::try_from(order).map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
            )
            .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
        insert_owner(
            &mut expected_owners,
            &mut expected_owner_count,
            membership.owner(),
        )?;
        order += 1;
    }
    if inputs.len() != expected_owner_count {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }

    let mut rows = [None; MAX_ORDERS];
    let mut owner_index = 0usize;
    while owner_index < expected_owner_count {
        let input = inputs[owner_index];
        let expected_owner = expected_owners[owner_index];
        if input.account.is_zero() {
            return Err(SettlementAdapterErrorV1::PositionSetMismatch);
        }
        let position = PositionAccountV3::decode(input.encoded_body)?;
        if position.lifecycle() != PositionLifecycleV3::Open
            || position.owner().bytes() != expected_owner.bytes()
            || position.replay_account().bytes() == input.account.bytes()
            || position.owner().bytes() == input.account.bytes()
        {
            return Err(SettlementAdapterErrorV1::PositionSetMismatch);
        }
        let purpose_binding = AdapterPositionPurposeBindingV3 {
            owner: retirement_identity(expected_owner)?,
            controller: position.controller(),
            purpose_binding_id: position.purpose_binding_id(),
        };
        project_general_position_v3(position, market_binding, purpose_binding)?;

        let mut owner_reservation_count = 0u64;
        let mut owner_reserved_cash = 0u64;
        order = 0;
        while order < usize::from(projection.book().len) {
            let membership = projection
                .order_membership(
                    u8::try_from(order)
                        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
                )
                .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
            if membership.owner() == expected_owner {
                if reservations.position_generations[order] != position.generation() {
                    return Err(SettlementAdapterErrorV1::PositionSetMismatch);
                }
                owner_reservation_count = owner_reservation_count
                    .checked_add(1)
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                owner_reserved_cash = owner_reserved_cash
                    .checked_add(reservations.reserved_cash_atoms[order])
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            }
            order += 1;
        }
        if position.outstanding_reservations() < owner_reservation_count
            || position.reserved_cash_atoms() < owner_reserved_cash
        {
            return Err(SettlementAdapterErrorV1::PositionSetMismatch);
        }
        let data_id = Id32::new(position.semantic_id(&PositionBodySha256V3)?.bytes())?;
        rows[owner_index] = Some(AuthenticatedSettlementPositionV3 {
            account: input.account,
            data_id,
            position,
        });
        owner_index += 1;
    }
    Ok(AuthenticatedSettlementPositionBookV3 {
        market_runtime: projection.market(),
        market_binding,
        rows,
        owner_count: u8::try_from(expected_owner_count)
            .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
    })
}

fn retirement_identity(value: Id32) -> Result<Identity32V1, SettlementAdapterErrorV1> {
    Identity32V1::new(value.bytes()).map_err(|_| SettlementAdapterErrorV1::BindingMismatch)
}

/// One canonical settlement leg.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementLegV1 {
    /// A real filled order at its dense live-order index.
    Order(u8),
    /// Virtual complete-set split supplying a real buy leg.
    Split,
    /// Virtual complete-set merge absorbing a real sell leg.
    Merge,
}

impl SettlementLegV1 {
    const fn kind(self) -> u8 {
        match self {
            Self::Order(_) => 0,
            Self::Split => 1,
            Self::Merge => 2,
        }
    }

    const fn index(self) -> u8 {
        match self {
            Self::Order(index) => index,
            Self::Split | Self::Merge => 0,
        }
    }
}

/// Direct order-to-order, split-to-buy, or sell-to-merge route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementRouteV1 {
    /// Two real orders owned by distinct Position owners.
    Direct = 0,
    /// A virtual split supplies the real buy order.
    SplitToBuy = 1,
    /// A real sell order supplies the virtual merge.
    SellToMerge = 2,
}

/// One exact canonical pairing slice retained behind a checked projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalSettlementSliceV1 {
    buy: SettlementLegV1,
    sell: SettlementLegV1,
    route: SettlementRouteV1,
    outcome: u8,
    quantity: u64,
}

impl CanonicalSettlementSliceV1 {
    const EMPTY: Self = Self {
        buy: SettlementLegV1::Order(0),
        sell: SettlementLegV1::Order(0),
        route: SettlementRouteV1::Direct,
        outcome: 0,
        quantity: 0,
    };

    /// Buy order or virtual merge leg.
    pub const fn buy(&self) -> SettlementLegV1 {
        self.buy
    }

    /// Sell order or virtual split leg.
    pub const fn sell(&self) -> SettlementLegV1 {
        self.sell
    }

    /// Direct, split, or merge classification.
    pub const fn route(&self) -> SettlementRouteV1 {
        self.route
    }

    /// Outcome transferred by both ends.
    pub const fn outcome(&self) -> u8 {
        self.outcome
    }

    /// Exact Egg atoms transferred.
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }

    fn encode(self, output: &mut [u8]) -> Result<(), SettlementAdapterErrorV1> {
        if output.len() != SETTLEMENT_SLICE_BYTES || self.quantity == 0 {
            return Err(SettlementAdapterErrorV1::OutputLengthMismatch);
        }
        output[0] = self.buy.kind();
        output[1] = self.buy.index();
        output[2] = self.sell.kind();
        output[3] = self.sell.index();
        output[4] = self.outcome;
        output[5..13].copy_from_slice(&self.quantity.to_le_bytes());
        Ok(())
    }
}

/// One real receipt end derived from a canonical slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedSettlementReceiptEndV1 {
    slice_index: u16,
    order_index: u8,
    owner: Id32,
    side: SettlementSideV1,
    route: SettlementRouteV1,
    outcome: u8,
    quantity: u64,
    consideration_price_units: u128,
    completes_order: bool,
    expected_end_mask: u8,
}

impl DerivedSettlementReceiptEndV1 {
    /// Zero-based canonical slice index.
    pub const fn slice_index(&self) -> u16 {
        self.slice_index
    }

    /// Dense live-order index used by the feed fill vector.
    pub const fn order_index(&self) -> u8 {
        self.order_index
    }

    /// Semantic Position owner from the frozen page.
    pub const fn owner(&self) -> Id32 {
        self.owner
    }

    /// Buy debit or sell credit end.
    pub const fn side(&self) -> SettlementSideV1 {
        self.side
    }

    /// Direct, split, or merge classification inherited from the slice.
    pub const fn route(&self) -> SettlementRouteV1 {
        self.route
    }

    /// Bound outcome.
    pub const fn outcome(&self) -> u8 {
        self.outcome
    }

    /// Exact Egg atoms moved by this end.
    pub const fn quantity(&self) -> u64 {
        self.quantity
    }

    /// Exact `quantity * price[outcome]` with no rounding.
    pub const fn consideration_price_units(&self) -> u128 {
        self.consideration_price_units
    }

    /// Whether this is the final canonical receipt end for the order.
    pub const fn completes_order(&self) -> bool {
        self.completes_order
    }

    /// Real receipt ends present on the slice: bit zero buy, bit one sell.
    pub const fn expected_end_mask(&self) -> u8 {
        self.expected_end_mask
    }
}

/// Complete canonical settlement projection for one checked Direct candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateSettlementProjectionV1 {
    market: Id32,
    epoch: Id32,
    order_set: Id32,
    candidate: Id32,
    owner_order_set_digest: Id32,
    position_book: AuthenticatedSettlementPositionBookV3,
    price_scale: u64,
    prices: [u64; MAX_OUTCOMES],
    slices: [CanonicalSettlementSliceV1; MAX_SLICES],
    slice_count: u16,
    receipt_end_count: u16,
    settlement_orders: [VerifiedSettlementOrderV2; MAX_ORDERS],
    settlement_memberships: [Option<AuthenticatedOrderMembershipV2>; MAX_ORDERS],
    settlement_order_count: u8,
    participating_owners: [Id32; MAX_ORDERS],
    owner_count: u8,
    buy_price_units: u128,
    sell_price_units: u128,
    buy_present: bool,
    sell_present: bool,
    rounding_pot_price_units: u128,
    virtual_split_price_units: u128,
    virtual_merge_price_units: u128,
}

impl CandidateSettlementProjectionV1 {
    /// Canonical owner/order membership digest.
    pub const fn owner_order_set_digest(&self) -> Id32 {
        self.owner_order_set_digest
    }

    /// Complete full-width Position V3 set bound into owner membership.
    pub const fn position_book(&self) -> &AuthenticatedSettlementPositionBookV3 {
        &self.position_book
    }

    /// Checked RelationV2 candidate identity.
    pub const fn candidate(&self) -> Id32 {
        self.candidate
    }

    /// Number of canonical settlement slices.
    pub const fn slice_count(&self) -> u16 {
        self.slice_count
    }

    /// Canonical slice at one active index.
    pub fn slice(&self, index: u16) -> Option<&CanonicalSettlementSliceV1> {
        if index < self.slice_count {
            Some(&self.slices[usize::from(index)])
        } else {
            None
        }
    }

    /// Bind fresh account identities to one derived two-real-end receipt.
    pub fn bind_fresh_direct_receipt(
        &self,
        slice_index: u16,
        receipt: Id32,
        receipt_accounting_id: Id32,
        delivery_transition_id: Id32,
    ) -> Result<AuthenticatedSettlementReceiptV2, SettlementAdapterErrorV1> {
        let slice = *self
            .slice(slice_index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let (buy_order, sell_order) = match (slice.route, slice.buy, slice.sell) {
            (
                SettlementRouteV1::Direct,
                SettlementLegV1::Order(buy),
                SettlementLegV1::Order(sell),
            ) => (buy, sell),
            _ => return Err(SettlementAdapterErrorV1::BindingMismatch),
        };
        let mut value = AuthenticatedSettlementReceiptV2 {
            receipt: live_adapter_id(receipt)?,
            receipt_data_id: [0; 32],
            receipt_accounting_id: live_adapter_id(receipt_accounting_id)?,
            delivery_transition_id: live_adapter_id(delivery_transition_id)?,
            market: self.market.bytes(),
            epoch: self.epoch.bytes(),
            candidate: self.candidate.bytes(),
            owner_order_set_digest: self.owner_order_set_digest.bytes(),
            buy_order_id: self
                .settlement_order_membership(buy_order)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                .order_id,
            sell_order_id: self
                .settlement_order_membership(sell_order)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                .order_id,
            route: SettlementReceiptRouteV2::Direct,
            outcome: slice.outcome,
            quantity: slice.quantity,
            price: PresentPriceV2::new(self.prices[usize::from(slice.outcome)]),
            consideration_price_units: PresentConsiderationV2::new(slice_consideration(
                &self.prices,
                slice,
            )?),
            slice_index,
            sequence: u64::from(slice_index) + 1,
            settled_quantity: 0,
            accounted_end_mask: 0,
            delivered_end_mask: 0,
        };
        value.receipt_data_id = derive_projection_receipt_data_id_v2(value, self)?;
        value.validate(self.position_book.market_binding.outcome_count)?;
        Ok(value)
    }

    /// Bind fresh account identities to one derived virtual-split receipt.
    pub fn bind_fresh_virtual_split_receipt(
        &self,
        slice_index: u16,
        receipt: Id32,
        receipt_accounting_id: Id32,
        delivery_transition_id: Id32,
    ) -> Result<AuthenticatedSettlementReceiptV2, SettlementAdapterErrorV1> {
        let slice = *self
            .slice(slice_index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let buy_order = match (slice.route, slice.buy, slice.sell) {
            (
                SettlementRouteV1::SplitToBuy,
                SettlementLegV1::Order(buy),
                SettlementLegV1::Split,
            ) => buy,
            _ => return Err(SettlementAdapterErrorV1::BindingMismatch),
        };
        let mut value = AuthenticatedSettlementReceiptV2 {
            receipt: live_adapter_id(receipt)?,
            receipt_data_id: [0; 32],
            receipt_accounting_id: live_adapter_id(receipt_accounting_id)?,
            delivery_transition_id: live_adapter_id(delivery_transition_id)?,
            market: self.market.bytes(),
            epoch: self.epoch.bytes(),
            candidate: self.candidate.bytes(),
            owner_order_set_digest: self.owner_order_set_digest.bytes(),
            buy_order_id: self
                .settlement_order_membership(buy_order)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                .order_id,
            sell_order_id: [0; 32],
            route: SettlementReceiptRouteV2::SplitToBuy,
            outcome: slice.outcome,
            quantity: slice.quantity,
            price: PresentPriceV2::new(self.prices[usize::from(slice.outcome)]),
            consideration_price_units: PresentConsiderationV2::new(slice_consideration(
                &self.prices,
                slice,
            )?),
            slice_index,
            sequence: u64::from(slice_index) + 1,
            settled_quantity: 0,
            accounted_end_mask: 0,
            delivered_end_mask: 0,
        };
        value.receipt_data_id = derive_projection_receipt_data_id_v2(value, self)?;
        value.validate(self.position_book.market_binding.outcome_count)?;
        Ok(value)
    }

    /// Bind fresh account identities to one derived virtual-merge receipt.
    pub fn bind_fresh_virtual_merge_receipt(
        &self,
        slice_index: u16,
        receipt: Id32,
        receipt_accounting_id: Id32,
        delivery_transition_id: Id32,
    ) -> Result<AuthenticatedSettlementReceiptV2, SettlementAdapterErrorV1> {
        let slice = *self
            .slice(slice_index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let sell_order = match (slice.route, slice.buy, slice.sell) {
            (
                SettlementRouteV1::SellToMerge,
                SettlementLegV1::Merge,
                SettlementLegV1::Order(sell),
            ) => sell,
            _ => return Err(SettlementAdapterErrorV1::BindingMismatch),
        };
        let mut value = AuthenticatedSettlementReceiptV2 {
            receipt: live_adapter_id(receipt)?,
            receipt_data_id: [0; 32],
            receipt_accounting_id: live_adapter_id(receipt_accounting_id)?,
            delivery_transition_id: live_adapter_id(delivery_transition_id)?,
            market: self.market.bytes(),
            epoch: self.epoch.bytes(),
            candidate: self.candidate.bytes(),
            owner_order_set_digest: self.owner_order_set_digest.bytes(),
            buy_order_id: [0; 32],
            sell_order_id: self
                .settlement_order_membership(sell_order)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                .order_id,
            route: SettlementReceiptRouteV2::SellToMerge,
            outcome: slice.outcome,
            quantity: slice.quantity,
            price: PresentPriceV2::new(self.prices[usize::from(slice.outcome)]),
            consideration_price_units: PresentConsiderationV2::new(slice_consideration(
                &self.prices,
                slice,
            )?),
            slice_index,
            sequence: u64::from(slice_index) + 1,
            settled_quantity: 0,
            accounted_end_mask: 0,
            delivered_end_mask: 0,
        };
        value.receipt_data_id = derive_projection_receipt_data_id_v2(value, self)?;
        value.validate(self.position_book.market_binding.outcome_count)?;
        Ok(value)
    }

    /// Number of real receipt ends across all slices.
    pub const fn receipt_end_count(&self) -> u16 {
        self.receipt_end_count
    }

    /// Derived real receipt end at one active index.
    pub fn receipt_end(&self, index: u16) -> Option<DerivedSettlementReceiptEndV1> {
        if index >= self.receipt_end_count {
            return None;
        }
        let mut observed = 0u16;
        let mut slice_index = 0u16;
        while slice_index < self.slice_count {
            let slice = self.slices[usize::from(slice_index)];
            if let SettlementLegV1::Order(order_index) = slice.buy {
                if observed == index {
                    return self.derive_receipt_end(
                        slice_index,
                        slice,
                        order_index,
                        SettlementSideV1::Buy,
                    );
                }
                observed = observed.checked_add(1)?;
            }
            if let SettlementLegV1::Order(order_index) = slice.sell {
                if observed == index {
                    return self.derive_receipt_end(
                        slice_index,
                        slice,
                        order_index,
                        SettlementSideV1::Sell,
                    );
                }
                observed = observed.checked_add(1)?;
            }
            slice_index = slice_index.checked_add(1)?;
        }
        None
    }

    /// Join SBF-authenticated receipt/transition identities to one exact
    /// derived accounting row consumed by `clutch-owner-settlement`.
    ///
    /// Receipt PDA derivation, account ownership, the existing latch bitmap,
    /// and the complete Egg/reservation transition remain adapter-owned facts;
    /// no economic amount, owner, order, sequence, or completion flag is
    /// accepted from the caller.
    pub fn bind_receipt_end_account(
        &self,
        receipt_end_index: u16,
        receipt: AuthenticatedSettlementReceiptV2,
    ) -> Result<AuthenticatedSettlementReceiptEndV2, SettlementAdapterErrorV1> {
        receipt.validate(self.position_book.market_binding.outcome_count)?;
        if derive_projection_receipt_data_id_v2(receipt, self)? != receipt.receipt_data_id {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        let end = self
            .receipt_end(receipt_end_index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let slice = *self
            .slice(end.slice_index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let side_mask = match end.side {
            SettlementSideV1::Buy => 0b01,
            SettlementSideV1::Sell => 0b10,
        };
        let route = match end.route {
            SettlementRouteV1::Direct => SettlementReceiptRouteV2::Direct,
            SettlementRouteV1::SplitToBuy => SettlementReceiptRouteV2::SplitToBuy,
            SettlementRouteV1::SellToMerge => SettlementReceiptRouteV2::SellToMerge,
        };
        let expected_buy_order = match slice.buy {
            SettlementLegV1::Order(index) => {
                self.settlement_order_membership(index)
                    .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                    .order_id
            }
            SettlementLegV1::Merge => [0; 32],
            SettlementLegV1::Split => return Err(SettlementAdapterErrorV1::BindingMismatch),
        };
        let expected_sell_order = match slice.sell {
            SettlementLegV1::Order(index) => {
                self.settlement_order_membership(index)
                    .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                    .order_id
            }
            SettlementLegV1::Split => [0; 32],
            SettlementLegV1::Merge => return Err(SettlementAdapterErrorV1::BindingMismatch),
        };
        if receipt.market != self.market.bytes()
            || receipt.epoch != self.epoch.bytes()
            || receipt.candidate != self.candidate.bytes()
            || receipt.owner_order_set_digest != self.owner_order_set_digest.bytes()
            || receipt.buy_order_id != expected_buy_order
            || receipt.sell_order_id != expected_sell_order
            || receipt.route != route
            || receipt.outcome != end.outcome
            || receipt.quantity != end.quantity
            || receipt.price != PresentPriceV2::new(self.prices[usize::from(end.outcome)])
            || receipt.consideration_price_units
                != PresentConsiderationV2::new(end.consideration_price_units)
            || receipt.slice_index != end.slice_index
            || receipt.sequence != u64::from(end.slice_index) + 1
            || receipt.settled_quantity != 0
            || receipt.delivered_end_mask != 0
            || receipt.accounted_end_mask & !end.expected_end_mask != 0
            || receipt.accounted_end_mask & side_mask != 0
        {
            return Err(SettlementAdapterErrorV1::ReceiptLatchMismatch);
        }
        let value = AuthenticatedSettlementReceiptEndV2 {
            receipt: receipt.receipt,
            receipt_data_id: receipt.receipt_data_id,
            receipt_accounting_id: receipt.receipt_accounting_id,
            market: self.market.bytes(),
            epoch: self.epoch.bytes(),
            candidate: self.candidate.bytes(),
            owner_order_set_digest: self.owner_order_set_digest.bytes(),
            owner: end.owner.bytes(),
            order_index: end.order_index,
            side: end.side,
            route,
            consideration_price_units: PresentConsiderationV2::new(end.consideration_price_units),
            completes_order: end.completes_order,
            slice_index: end.slice_index,
            sequence: u64::from(end.slice_index) + 1,
            accounted_end_mask: receipt.accounted_end_mask,
            expected_end_mask: end.expected_end_mask,
        };
        value.validate()?;
        Ok(value)
    }

    fn derive_receipt_end(
        &self,
        slice_index: u16,
        slice: CanonicalSettlementSliceV1,
        order_index: u8,
        side: SettlementSideV1,
    ) -> Option<DerivedSettlementReceiptEndV1> {
        let mut owner = None;
        let mut row = 0usize;
        while row < usize::from(self.settlement_order_count) {
            if self.settlement_orders[row].order_index == order_index {
                owner = Id32::new(self.settlement_orders[row].owner).ok();
                break;
            }
            row += 1;
        }
        let expected_end_mask = match slice.route {
            SettlementRouteV1::Direct => 0b11,
            SettlementRouteV1::SplitToBuy => 0b01,
            SettlementRouteV1::SellToMerge => 0b10,
        };
        let mut completes_order = true;
        let mut later = usize::from(slice_index) + 1;
        while later < usize::from(self.slice_count) {
            if self.slices[later].buy == SettlementLegV1::Order(order_index)
                || self.slices[later].sell == SettlementLegV1::Order(order_index)
            {
                completes_order = false;
                break;
            }
            later += 1;
        }
        Some(DerivedSettlementReceiptEndV1 {
            slice_index,
            order_index,
            owner: owner?,
            side,
            route: slice.route,
            outcome: slice.outcome,
            quantity: slice.quantity,
            consideration_price_units: u128::from(slice.quantity)
                .checked_mul(u128::from(self.prices[usize::from(slice.outcome)]))?,
            completes_order,
            expected_end_mask,
        })
    }

    /// Number of participating owners, in lexicographic owner order.
    pub const fn owner_count(&self) -> u8 {
        self.owner_count
    }

    /// Participating owner at one canonical ordinal.
    pub fn participating_owner(&self, index: u8) -> Option<Id32> {
        if index < self.owner_count {
            Some(self.participating_owners[usize::from(index)])
        } else {
            None
        }
    }

    /// Presence-explicit owner-settlement membership for one filled order.
    ///
    /// This is absent for an unfilled order. The SBF adapter must still decode
    /// the post-selection entitled Reservation and authenticate its account
    /// address/owner plus the named Position generation before value movement.
    pub fn settlement_order_membership(
        &self,
        order_index: u8,
    ) -> Option<AuthenticatedOrderMembershipV2> {
        self.settlement_memberships
            .get(usize::from(order_index))
            .copied()
            .flatten()
    }

    /// Exact owner buy consideration before collateral rounding.
    pub const fn buy_price_units(&self) -> u128 {
        self.buy_price_units
    }

    /// Exact owner sell consideration before collateral rounding.
    pub const fn sell_price_units(&self) -> u128 {
        self.sell_price_units
    }

    /// Exact owner-level terminal-rounding residue in price units.
    pub const fn rounding_pot_price_units(&self) -> u128 {
        self.rounding_pot_price_units
    }

    /// Exact virtual-split complete-set value in price units.
    pub const fn virtual_split_price_units(&self) -> u128 {
        self.virtual_split_price_units
    }

    /// Exact virtual-merge complete-set value in price units.
    pub const fn virtual_merge_price_units(&self) -> u128 {
        self.virtual_merge_price_units
    }

    /// Exact active settlement-tail width in bytes.
    pub fn encoded_tail_len(&self) -> usize {
        usize::from(self.slice_count) * SETTLEMENT_SLICE_BYTES
    }

    /// Encode only the canonical active CandidateFeedV2 settlement tail.
    pub fn encode_tail(&self, output: &mut [u8]) -> Result<(), SettlementAdapterErrorV1> {
        if output.len() != self.encoded_tail_len() {
            return Err(SettlementAdapterErrorV1::OutputLengthMismatch);
        }
        let mut slice = 0usize;
        while slice < usize::from(self.slice_count) {
            let at = slice * SETTLEMENT_SLICE_BYTES;
            self.slices[slice].encode(&mut output[at..at + SETTLEMENT_SLICE_BYTES])?;
            slice += 1;
        }
        Ok(())
    }

    /// Exact fee-funding facts for one canonical participating owner.
    pub fn owner_fee_funding(&self, owner_ordinal: u8) -> Option<VerifiedOwnerFeeFundingV1> {
        if owner_ordinal >= self.owner_count {
            return None;
        }
        let owner = self.participating_owners[usize::from(owner_ordinal)];
        let mut buy_price_units = 0u128;
        let mut reserved_buy_cash_atoms = 0u64;
        let mut has_buy = false;
        let mut has_sell = false;
        let mut order = 0usize;
        while order < usize::from(self.settlement_order_count) {
            let row = self.settlement_orders[order];
            if row.owner == owner.bytes() {
                match row.side {
                    SettlementSideV1::Buy => {
                        has_buy = true;
                        buy_price_units =
                            buy_price_units.checked_add(row.consideration_price_units.value)?;
                        reserved_buy_cash_atoms =
                            reserved_buy_cash_atoms.checked_add(row.reserved_cash_atoms)?;
                    }
                    SettlementSideV1::Sell => has_sell = true,
                }
            }
            order += 1;
        }
        Some(VerifiedOwnerFeeFundingV1 {
            owner: FeeId(owner.bytes()),
            price_scale: self.price_scale,
            buy_price_units,
            reserved_buy_cash_atoms,
            has_buy,
            has_sell,
        })
    }

    fn totals(&self, selected_fee_atoms: u128) -> CandidateSettlementTotalsV2 {
        CandidateSettlementTotalsV2 {
            owner_count: u16::from(self.owner_count),
            buy_price_units: if self.buy_present {
                PresentConsiderationV2::new(self.buy_price_units)
            } else {
                PresentConsiderationV2::ABSENT
            },
            sell_price_units: if self.sell_present {
                PresentConsiderationV2::new(self.sell_price_units)
            } else {
                PresentConsiderationV2::ABSENT
            },
            selected_fee_atoms,
            rounding_pot_price_units: self.rounding_pot_price_units,
            owner_slice_end_count: self.receipt_end_count,
        }
    }
}

/// Exact semantic inputs for one accounting-only action 25 transition.
///
/// The live adapter must authenticate the SelectedCandidate, owner-row,
/// receipt, Reservation, Position, and Replay accounts before calling this
/// structural composer. The composer then rederives every economic claim from
/// the selected candidate and returns one indivisible write set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountReceiptEndTransitionInputV2<'a> {
    /// Strict action-25 selector decoded by the General contract.
    pub payload: AccountReceiptEndPayloadV1,
    /// Authenticated terminal SelectedCandidate account and identity.
    pub selected_candidate: AuthenticatedSelectedCandidateV1<'a>,
    /// Complete checked settlement projection owned by that candidate.
    pub settlement: &'a CandidateSettlementProjectionV1,
    /// Initial Reservation bindings from the same frozen order set.
    pub reservation_book: &'a AuthenticatedReservationBookV1,
    /// Canonical real-end ordinal derived from the settlement witness.
    pub receipt_end_index: u16,
    /// Exact authenticated receipt prestate.
    pub receipt: AuthenticatedSettlementReceiptV2,
    /// Exact authenticated V2 owner-row prestate.
    pub owner_row: OwnerSettlementAccountProjectionV2,
    /// SBF-authenticated canonical Reservation account identity.
    pub reservation_account: Id32,
    /// Exact hostile Reservation body before this accounting end.
    pub reservation_body: &'a [u8],
    /// SBF-authenticated purpose Replay V3 account identity.
    pub replay_account: Id32,
    /// Canonical Replay PDA bump.
    pub replay_bump: u8,
    /// Ordinal required by the authenticated Replay prestate.
    pub replay_next_sequence: u64,
    /// Exact hostile purpose Replay V3 body before this action.
    pub replay_body: &'a [u8],
}

/// One atomic action-25 poststate bundle.
///
/// Position is deliberately read-only and unchanged. The only writes are the
/// receipt accounting latch, V2 owner row, conditional buyer Reservation cash
/// handoff, and purpose-owned Replay V3 successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountReceiptEndTransitionPlanV2 {
    selected_candidate_account: Id32,
    receipt_prestate_data_id: Id32,
    receipt_poststate: AuthenticatedSettlementReceiptV2,
    owner_settlement_account: Id32,
    owner_settlement_poststate_body: [u8; OWNER_SETTLEMENT_BODY_V2_BYTES],
    reservation_account: Id32,
    reservation_poststate_body: [u8; RESERVATION_ACCOUNT_BYTES],
    reservation_body_changed: bool,
    reserved_cash_handoff_atoms: u64,
    position_account: Id32,
    position_semantic_id: Id32,
    replay: GeneralReplayTransitionPlanV1,
}

impl AccountReceiptEndTransitionPlanV2 {
    /// SelectedCandidate account that authorized this exact projection.
    pub const fn selected_candidate_account(&self) -> Id32 {
        self.selected_candidate_account
    }

    /// Exact receipt prestate digest committed as GEN1 evidence.
    pub const fn receipt_prestate_data_id(&self) -> Id32 {
        self.receipt_prestate_data_id
    }

    /// Canonical receipt successor with one additional accounting latch.
    pub const fn receipt_poststate(&self) -> AuthenticatedSettlementReceiptV2 {
        self.receipt_poststate
    }

    /// V2 owner-settlement account to compare-and-write.
    pub const fn owner_settlement_account(&self) -> Id32 {
        self.owner_settlement_account
    }

    /// Exact canonical 288-byte V2 owner-row successor.
    pub const fn owner_settlement_poststate_body(
        &self,
    ) -> &[u8; OWNER_SETTLEMENT_BODY_V2_BYTES] {
        &self.owner_settlement_poststate_body
    }

    /// Canonical Reservation account participating in the atomic comparison.
    pub const fn reservation_account(&self) -> Id32 {
        self.reservation_account
    }

    /// Exact canonical Reservation successor body.
    pub const fn reservation_poststate_body(&self) -> &[u8; RESERVATION_ACCOUNT_BYTES] {
        &self.reservation_poststate_body
    }

    /// Whether terminal buy accounting moves cash ownership out of Reservation.
    pub const fn reservation_body_changed(&self) -> bool {
        self.reservation_body_changed
    }

    /// Exact buy cash envelope handed off once; zero otherwise.
    pub const fn reserved_cash_handoff_atoms(&self) -> u64 {
        self.reserved_cash_handoff_atoms
    }

    /// Read-only Position account whose owner and generation were bound.
    pub const fn position_account(&self) -> Id32 {
        self.position_account
    }

    /// Unchanged exact Position semantic ID.
    pub const fn position_semantic_id(&self) -> Id32 {
        self.position_semantic_id
    }

    /// Exact purpose Replay V3 successor joined to every other poststate.
    pub const fn replay(&self) -> &GeneralReplayTransitionPlanV1 {
        &self.replay
    }
}

/// Prepare one complete accounting-only action 25 transition.
///
/// The canonical receipt owns per-end replay, the V2 owner row owns cumulative
/// price units and completed-order masks, and the canonical Reservation owns
/// only its asset envelope. No parallel Reservation accounting ledger exists.
pub fn prepare_account_receipt_end_transition_v2(
    input: AccountReceiptEndTransitionInputV2<'_>,
) -> Result<AccountReceiptEndTransitionPlanV2, SettlementAdapterErrorV1> {
    input.selected_candidate.account.validate()?;
    let selected = input.selected_candidate.account;
    let settlement = input.settlement;
    if input.selected_candidate.artifact != input.payload.selected_candidate
        || input.payload.epoch != settlement.epoch
        || input.payload.owner_settlement.bytes() != input.owner_row.address
        || input.payload.receipt.bytes() != input.receipt.receipt()
        || input.payload.receipt_accounting_id.bytes()
            != input.receipt.receipt_accounting_id()
        || selected.epoch != settlement.epoch
        || selected.market != settlement.market
        || selected.order_set != settlement.order_set
        || selected.base_relation_candidate_id != settlement.candidate
        || selected.slice_count != settlement.slice_count
        || selected.entitlement_state != 2
        || selected.next_slice_index != selected.slice_count
        || input.reservation_book.market != settlement.market
        || input.reservation_book.epoch != settlement.epoch
        || input.reservation_book.order_set != settlement.order_set
        || input.reservation_book.order_count == 0
        || input.reservation_account.is_zero()
        || input.replay_account.is_zero()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    let receipt_end = settlement.bind_receipt_end_account(
        input.receipt_end_index,
        input.receipt,
    )?;
    let derived_end = settlement
        .receipt_end(input.receipt_end_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let order = settlement
        .settlement_order_membership(derived_end.order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    order.validate()?;
    let position_row = settlement
        .position_book
        .position_for_owner(derived_end.owner)
        .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
    let position_body = position_row.position();
    let position_fields = position_body.fields();
    let position = AuthenticatedPositionV3 {
        account: position_row.account().bytes(),
        general_market_runtime: settlement.market.bytes(),
        semantic: position_body,
        semantic_id: position_row.data_id().bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: false,
    };
    position.validate()?;

    let order_index = usize::from(order.order_index);
    let expected_reservation_id = input
        .reservation_book
        .reservation_id(order.order_index)
        .ok_or(SettlementAdapterErrorV1::ReservationSetMismatch)?;
    let expected_position_generation = input
        .reservation_book
        .position_generation(order.order_index)
        .ok_or(SettlementAdapterErrorV1::ReservationSetMismatch)?;
    let expected_initial_cash = input
        .reservation_book
        .reserved_cash_atoms(order.order_index)
        .ok_or(SettlementAdapterErrorV1::ReservationSetMismatch)?;
    let expected_maximum_fee = input
        .reservation_book
        .maximum_fee_atoms(order.order_index)
        .ok_or(SettlementAdapterErrorV1::ReservationSetMismatch)?;
    let mut reservation = ReservationAccount::decode(input.reservation_body)?;
    let expected_side = match order.side {
        SettlementSideV1::Buy => 0,
        SettlementSideV1::Sell => 1,
    };
    let expected_kind = match order.order_kind {
        OrderKindV1::Single => 1,
        OrderKindV1::Portfolio => 2,
    };
    if order_index >= usize::from(input.reservation_book.order_count)
        || reservation.reservation.bytes() != expected_reservation_id.bytes()
        || reservation.market.bytes() != settlement.market.bytes()
        || reservation.epoch.bytes() != settlement.epoch.bytes()
        || reservation.owner.bytes() != order.owner
        || reservation.order_id.bytes() != order.order_id
        || reservation.position_generation != expected_position_generation
        || reservation.position_generation != order.position_generation
        || reservation.position_generation != position_fields.generation
        || reservation.order_generation != order.order_generation
        || reservation.price_grid.bytes() != input.reservation_book.price_grid_id.bytes()
        || reservation.terms.bytes() != input.reservation_book.terms.bytes()
        || reservation.policy.bytes() != input.reservation_book.reservation_policy.bytes()
        || reservation.outcome_count != order.outcome_count
        || reservation.order_kind != expected_kind
        || reservation.side != expected_side
        || reservation.state != RESERVATION_STATE_ENTITLED
        || reservation.entitled_units != order.entitled_units
        || reservation.consumed_units != 0
        || reservation.paid_units != 0
        || reservation.initial_cash_atoms != expected_initial_cash
        || reservation.max_fee_atoms != expected_maximum_fee
        || (order.order_kind == OrderKindV1::Single
            && order.single_outcome != input.receipt.outcome)
        || input.reservation_account.bytes() == input.receipt.receipt()
        || input.reservation_account.bytes() == input.owner_row.address
        || input.reservation_account == input.selected_candidate.artifact
        || input.reservation_account.bytes() == position.account
        || input.reservation_account == input.replay_account
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }

    let owner_projection = project_owner_receipt_end_v2(input.owner_row, receipt_end)?;
    let owner_poststate = OwnerSettlementAccumulatorV2::decode_body(
        &owner_projection.owner_settlement_body,
    )?;
    let order_bit = 1u64
        .checked_shl(u32::from(order.order_index))
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let (pre_completed, post_completed) = match order.side {
        SettlementSideV1::Buy => (
            input.owner_row.accumulator.completed_buy_order_mask & order_bit != 0,
            owner_poststate.completed_buy_order_mask & order_bit != 0,
        ),
        SettlementSideV1::Sell => (
            input.owner_row.accumulator.completed_sell_order_mask & order_bit != 0,
            owner_poststate.completed_sell_order_mask & order_bit != 0,
        ),
    };
    if pre_completed
        || post_completed != derived_end.completes_order
        || owner_projection.receipt_data_id != input.receipt.receipt_data_id()
        || owner_projection.receipt_accounting_id != input.receipt.receipt_accounting_id()
        || owner_projection.receipt_accounted_end_mask
            != (input.receipt.accounted_end_mask
                | match order.side {
                    SettlementSideV1::Buy => 1,
                    SettlementSideV1::Sell => 2,
                })
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    let reserved_cash_handoff_atoms = match order.side {
        SettlementSideV1::Buy => {
            if reservation.remaining_internal != [0; MAX_OUTCOMES]
                || reservation.remaining_cash_atoms != reservation.initial_cash_atoms
                || position_fields.reserved_cash_atoms
                    < input.owner_row.accumulator.expectation.reserved_cash_atoms
            {
                return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
            }
            if derived_end.completes_order {
                let handoff = reservation.remaining_cash_atoms;
                reservation.remaining_cash_atoms = 0;
                handoff
            } else {
                0
            }
        }
        SettlementSideV1::Sell => {
            if reservation.initial_cash_atoms != 0 || reservation.remaining_cash_atoms != 0 {
                return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
            }
            0
        }
    };
    reservation.validate()?;
    let mut reservation_poststate_body = [0u8; RESERVATION_ACCOUNT_BYTES];
    let written = reservation.encode(&mut reservation_poststate_body)?;
    if written != RESERVATION_ACCOUNT_BYTES {
        return Err(SettlementAdapterErrorV1::OutputLengthMismatch);
    }
    let reservation_body_changed = reservation_poststate_body != input.reservation_body;
    if reservation_body_changed
        != (order.side == SettlementSideV1::Buy
            && derived_end.completes_order
            && reserved_cash_handoff_atoms != 0)
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    let mut receipt_poststate = input.receipt;
    receipt_poststate.accounted_end_mask = owner_projection.receipt_accounted_end_mask;
    receipt_poststate.receipt_data_id = derive_projection_receipt_data_id_v2(
        receipt_poststate,
        settlement,
    )?;
    receipt_poststate.validate(settlement.position_book.market_binding.outcome_count)?;

    let replay_prestate = project_general_position_replay_prestate_v1(
        input.replay_account,
        input.replay_bump,
        input.replay_next_sequence,
        input.replay_body,
        position,
        &PositionBodySha256V3,
    )?;
    let replay = project_general_replay_transition_v1(
        replay_prestate,
        position.unchanged_poststate()?,
        GeneralReplayTransitionKindV1::AccountReceiptEnd,
        input.payload.receipt_accounting_id,
        Id32::new(input.receipt.receipt_data_id())?,
        &PositionBodySha256V3,
    )?;
    if replay.position_prestate_semantic_id() != position_row.data_id()
        || replay.position_poststate_semantic_id() != position_row.data_id()
        || replay.transition_id() != input.payload.receipt_accounting_id
        || replay.transition_evidence_id().bytes() != input.receipt.receipt_data_id()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    Ok(AccountReceiptEndTransitionPlanV2 {
        selected_candidate_account: input.selected_candidate.artifact,
        receipt_prestate_data_id: Id32::new(input.receipt.receipt_data_id())?,
        receipt_poststate,
        owner_settlement_account: Id32::new(owner_projection.owner_settlement_account)?,
        owner_settlement_poststate_body: owner_projection.owner_settlement_body,
        reservation_account: input.reservation_account,
        reservation_poststate_body,
        reservation_body_changed,
        reserved_cash_handoff_atoms,
        position_account: position_row.account(),
        position_semantic_id: position_row.data_id(),
        replay,
    })
}

/// Derive the canonical owner-aware settlement decomposition.
///
/// RelationV2 remains owner-blind. This constructor therefore performs the
/// separate executable-pairing check and may refuse a RelationV2-valid
/// candidate whose ownership distribution cannot settle without self-pairing.
pub fn derive_candidate_settlement_v1(
    candidate: &BuiltDirectCandidateV1,
    projection: &OwnerBlindBookProjectionV1,
    reservations: &AuthenticatedReservationBookV1,
    positions: &AuthenticatedSettlementPositionBookV3,
) -> Result<CandidateSettlementProjectionV1, SettlementAdapterErrorV1> {
    let candidate_id = candidate.base_relation_candidate_id()?;
    if candidate.market() != projection.market()
        || candidate.epoch() != projection.epoch()
        || candidate.order_set() != projection.order_set()
        || reservations.market != projection.market()
        || reservations.epoch != projection.epoch()
        || reservations.order_set != projection.order_set()
        || reservations.price_grid_id != projection.price_grid_id()
        || reservations.order_count != projection.book().len
        || positions.market_runtime != projection.market()
        || positions.market_binding.market_instance_id.bytes()
            != projection.market_binding().market_instance_v2_id.bytes()
        || positions.market_binding.realm_id.bytes() != projection.realm().bytes()
        || positions.market_binding.outcome_count != projection.domain().outcome_count
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let owner_order_set_digest =
        derive_owner_order_set_digest_v1(projection, reservations, positions)?;
    let mut slices = [CanonicalSettlementSliceV1::EMPTY; MAX_SLICES];
    let slice_count = derive_pairing_slices(candidate, projection, &mut slices)?;
    let mut per_order_end_count = [0u16; MAX_ORDERS];
    let mut receipt_end_count = 0u16;
    let mut slice = 0usize;
    while slice < usize::from(slice_count) {
        if let SettlementLegV1::Order(order) = slices[slice].buy {
            per_order_end_count[usize::from(order)] = per_order_end_count[usize::from(order)]
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            receipt_end_count = receipt_end_count
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        }
        if let SettlementLegV1::Order(order) = slices[slice].sell {
            per_order_end_count[usize::from(order)] = per_order_end_count[usize::from(order)]
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            receipt_end_count = receipt_end_count
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        }
        slice += 1;
    }

    let mut settlement_orders = [EMPTY_VERIFIED_SETTLEMENT_ORDER_V2; MAX_ORDERS];
    let mut settlement_memberships = [None; MAX_ORDERS];
    let mut settlement_order_count = 0usize;
    let mut participating_owners = [Id32::ZERO; MAX_ORDERS];
    let mut owner_count = 0usize;
    let mut buy_price_units = 0u128;
    let mut sell_price_units = 0u128;
    let mut buy_present = false;
    let mut sell_present = false;
    let mut order = 0usize;
    while order < usize::from(projection.book().len) {
        let fill = candidate.economic_candidate().fills[order];
        if fill != 0 {
            let membership = projection
                .order_membership(
                    u8::try_from(order)
                        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
                )
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
            let position = positions
                .position_for_owner(membership.owner())
                .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
            let value = order_consideration_price_units(candidate, projection, order, fill)?;
            if per_order_end_count[order] == 0 {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            let side = match projection.book().orders[order].side {
                Side::Buy => SettlementSideV1::Buy,
                Side::Sell => SettlementSideV1::Sell,
            };
            let reserved_cash = match side {
                SettlementSideV1::Buy => reservations.reserved_cash_atoms[order],
                SettlementSideV1::Sell => 0,
            };
            settlement_orders[settlement_order_count] = VerifiedSettlementOrderV2 {
                owner: membership.owner().bytes(),
                order_index: u8::try_from(order)
                    .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
                side,
                consideration_price_units: PresentConsiderationV2::new(value),
                slice_count: per_order_end_count[order],
                reserved_cash_atoms: reserved_cash,
            };
            let position_generation = reservations.position_generations[order];
            if membership.generation() == 0
                || position_generation == 0
                || position.position().generation() != position_generation
            {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            let entitled_units = order_entitled_units(candidate, projection, order, fill)?;
            let (order_kind, single_outcome) = match membership.slot() {
                clutch_solana_layout::OrderSlot::Single(record) => {
                    (OrderKindV1::Single, record.outcome)
                }
                clutch_solana_layout::OrderSlot::Portfolio(_) => (OrderKindV1::Portfolio, u8::MAX),
                clutch_solana_layout::OrderSlot::Empty
                | clutch_solana_layout::OrderSlot::Tombstone(_) => {
                    return Err(SettlementAdapterErrorV1::BindingMismatch);
                }
            };
            let settlement_membership = AuthenticatedOrderMembershipV2 {
                market: projection.market().bytes(),
                epoch: projection.epoch().bytes(),
                candidate: candidate_id.bytes(),
                owner_order_set_digest: owner_order_set_digest.bytes(),
                order_id: membership.order_id().bytes(),
                reservation: reservations.reservation_ids[order].bytes(),
                owner: membership.owner().bytes(),
                order_index: u8::try_from(order)
                    .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
                order_generation: membership.generation(),
                position_generation,
                side,
                order_kind,
                outcome_count: projection.domain().outcome_count,
                single_outcome,
                entitled_units,
                entitled_consideration_price_units: PresentConsiderationV2::new(value),
            };
            settlement_membership.validate()?;
            settlement_memberships[order] = Some(settlement_membership);
            settlement_order_count += 1;
            match side {
                SettlementSideV1::Buy => {
                    buy_present = true;
                    buy_price_units = buy_price_units
                        .checked_add(value)
                        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                }
                SettlementSideV1::Sell => {
                    sell_present = true;
                    sell_price_units = sell_price_units
                        .checked_add(value)
                        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                }
            }
            insert_owner(
                &mut participating_owners,
                &mut owner_count,
                membership.owner(),
            )?;
        }
        order += 1;
    }

    let mut rounding_pot_price_units = 0u128;
    let mut owner = 0usize;
    while owner < owner_count {
        let owner_id = participating_owners[owner];
        let mut owner_buy = 0u128;
        let mut owner_sell = 0u128;
        order = 0;
        while order < settlement_order_count {
            let row = settlement_orders[order];
            if row.owner == owner_id.bytes() {
                match row.side {
                    SettlementSideV1::Buy => {
                        owner_buy = owner_buy
                            .checked_add(row.consideration_price_units.value)
                            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                    }
                    SettlementSideV1::Sell => {
                        owner_sell = owner_sell
                            .checked_add(row.consideration_price_units.value)
                            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                    }
                }
            }
            order += 1;
        }
        rounding_pot_price_units = rounding_pot_price_units
            .checked_add(owner_rounding_residue_price_units(
                owner_buy,
                owner_sell,
                candidate.price().price_scale,
            )?)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        owner += 1;
    }
    let virtual_split_price_units = u128::from(candidate.economic_candidate().virtual_split)
        .checked_mul(u128::from(candidate.price().price_scale))
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let virtual_merge_price_units = u128::from(candidate.economic_candidate().virtual_merge)
        .checked_mul(u128::from(candidate.price().price_scale))
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    Ok(CandidateSettlementProjectionV1 {
        market: projection.market(),
        epoch: projection.epoch(),
        order_set: projection.order_set(),
        candidate: candidate_id,
        owner_order_set_digest,
        position_book: *positions,
        price_scale: candidate.price().price_scale,
        prices: candidate.price().prices,
        slices,
        slice_count,
        receipt_end_count,
        settlement_orders,
        settlement_memberships,
        settlement_order_count: u8::try_from(settlement_order_count)
            .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
        participating_owners,
        owner_count: u8::try_from(owner_count)
            .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
        buy_price_units,
        sell_price_units,
        buy_present,
        sell_present,
        rounding_pot_price_units,
        virtual_split_price_units,
        virtual_merge_price_units,
    })
}

/// Project one terminal owner fee using funding derived from the exact
/// candidate settlement projection rather than caller-supplied owner totals.
/// The presented mutable envelope prefix must be the exhaustive filled-order
/// set for this owner, strictly ordered by authenticated Reservation identity;
/// its identities, side funding, and signed maximums are rejoined here. The
/// SBF adapter remains responsible for authenticating each envelope account's
/// current debit before this pure projection is called.
#[allow(clippy::too_many_arguments)]
pub fn project_candidate_owner_fee_v1(
    settlement: &CandidateSettlementProjectionV1,
    reservations: &AuthenticatedReservationBookV1,
    owner_ordinal: u8,
    selected: &SelectedCompositeFeeV1,
    transition: &OwnerFeeTransitionIntentV1,
    carry: &OwnerFeeCarryV1,
    assessment: &OwnerFeeAssessmentV1,
    payer: &PayerAllocationV1,
    envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    envelope_len: u8,
) -> Result<AuthenticatedSelectedOwnerFeeV1, SettlementAdapterErrorV1> {
    let funding = settlement
        .owner_fee_funding(owner_ordinal)
        .ok_or(SettlementAdapterErrorV1::FeeOwnerMismatch)?;
    authenticate_owner_fee_envelopes(
        settlement,
        reservations,
        owner_ordinal,
        envelopes,
        envelope_len,
    )?;
    if selected.selected_candidate().0 != settlement.candidate.bytes() {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(project_terminal_owner_fee_v1(
        selected,
        transition,
        carry,
        assessment,
        payer,
        funding,
        envelopes,
        envelope_len,
    )?)
}

fn authenticate_owner_fee_envelopes(
    settlement: &CandidateSettlementProjectionV1,
    reservations: &AuthenticatedReservationBookV1,
    owner_ordinal: u8,
    envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    envelope_len: u8,
) -> Result<(), SettlementAdapterErrorV1> {
    if reservations.market != settlement.market
        || reservations.epoch != settlement.epoch
        || reservations.order_set != settlement.order_set
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let owner = settlement
        .participating_owner(owner_ordinal)
        .ok_or(SettlementAdapterErrorV1::FeeOwnerMismatch)?;
    let mut expected_order_indices = [0u8; MAX_ORDERS];
    let mut expected_len = 0usize;
    let mut order = 0usize;
    while order < usize::from(reservations.order_count) {
        if let Some(membership) = settlement.settlement_memberships[order] {
            if membership.owner == owner.bytes() {
                let mut insert = expected_len;
                while insert > 0
                    && reservations.reservation_ids[order]
                        < reservations.reservation_ids
                            [usize::from(expected_order_indices[insert - 1])]
                {
                    expected_order_indices[insert] = expected_order_indices[insert - 1];
                    insert -= 1;
                }
                expected_order_indices[insert] = u8::try_from(order)
                    .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
                expected_len += 1;
            }
        }
        order += 1;
    }
    if usize::from(envelope_len) != expected_len {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let mut envelope = 0usize;
    while envelope < expected_len {
        let order_index = usize::from(expected_order_indices[envelope]);
        let expected = settlement.settlement_memberships[order_index]
            .ok_or(SettlementAdapterErrorV1::FeeOwnerMismatch)?;
        let observed = envelopes[envelope];
        let (funding, maximum_fee_atoms) = match expected.side {
            SettlementSideV1::Buy => (
                clutch_fee_runtime_contract::allocation::FeeEnvelopeFundingV1::BuyCashReservation,
                reservations.maximum_fee_atoms[order_index],
            ),
            SettlementSideV1::Sell => (
                clutch_fee_runtime_contract::allocation::FeeEnvelopeFundingV1::NoCashReservation,
                0,
            ),
        };
        if observed.owner.0 != owner.bytes()
            || observed.intent.0 != reservations.reservation_ids[order_index].bytes()
            || observed.funding != funding
            || observed.max_fee_atoms != maximum_fee_atoms
        {
            return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
        }
        envelope += 1;
    }
    while envelope < MAX_FEE_ROWS_V1 {
        let padding = envelopes[envelope];
        if padding.owner.0 != [0; 32]
            || padding.intent.0 != [0; 32]
            || padding.funding
                != clutch_fee_runtime_contract::allocation::FeeEnvelopeFundingV1::NoCashReservation
            || padding.max_fee_atoms != 0
            || padding.debited_atoms != 0
        {
            return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
        }
        envelope += 1;
    }
    Ok(())
}

/// Assemble the complete fee book against independently derived participants
/// and candidate totals. No caller-supplied `CandidateSettlementTotalsV2` is
/// accepted by this bridge.
pub fn assemble_candidate_owner_fee_book_v1(
    settlement: &CandidateSettlementProjectionV1,
    selected: &SelectedCompositeFeeV1,
    projections: &[AuthenticatedSelectedOwnerFeeV1; MAX_ORDERS],
    projection_len: u8,
) -> Result<SelectedOwnerFeeBookV1, SettlementAdapterErrorV1> {
    if projection_len != settlement.owner_count
        || selected.selected_candidate().0 != settlement.candidate.bytes()
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let mut owners = [FeeId([0; 32]); MAX_ORDERS];
    let mut selected_fee_atoms = 0u128;
    let mut owner = 0usize;
    while owner < usize::from(settlement.owner_count) {
        owners[owner] = FeeId(settlement.participating_owners[owner].bytes());
        let row = projections[owner].row();
        if row.owner != settlement.participating_owners[owner].bytes() {
            return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
        }
        selected_fee_atoms = selected_fee_atoms
            .checked_add(u128::from(row.fee_atoms))
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        owner += 1;
    }
    let expected = settlement.totals(selected_fee_atoms);
    Ok(assemble_selected_owner_fee_book_v1(
        selected,
        &owners,
        projections,
        projection_len,
        expected,
    )?)
}

/// Private checked bridge into the owner-settlement builder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementBridgeV2 {
    totals: CandidateSettlementTotalsV2,
    book: Option<OwnerSettlementBookV2>,
    cash_pot_expectation: Option<SettlementCashPotExpectationV1>,
}

impl OwnerSettlementBridgeV2 {
    /// Candidate-wide totals recomputed from exact rows and terminal fees.
    pub const fn totals(&self) -> CandidateSettlementTotalsV2 {
        self.totals
    }

    /// Canonical owner book, absent only for a zero-fill candidate.
    pub fn book(&self) -> Option<&OwnerSettlementBookV2> {
        self.book.as_ref()
    }

    /// Candidate-wide cash-pot expectation derived from the same owner rows.
    ///
    /// It is absent only for a zero-fill candidate. Split funding and opening
    /// merge proceeds remain distinct principal directions in the owner-owned
    /// expectation.
    pub fn cash_pot_expectation(&self) -> Option<SettlementCashPotExpectationV1> {
        self.cash_pot_expectation
    }
}

/// Join a complete private fee book into canonical owner settlement rows.
pub fn bridge_owner_settlement_v2(
    settlement: &CandidateSettlementProjectionV1,
    fee_book: Option<&SelectedOwnerFeeBookV1>,
) -> Result<OwnerSettlementBridgeV2, SettlementAdapterErrorV1> {
    if settlement.owner_count == 0 {
        if fee_book.is_some()
            || settlement.settlement_order_count != 0
            || settlement.receipt_end_count != 0
            || settlement.buy_present
            || settlement.sell_present
        {
            return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
        }
        return Ok(OwnerSettlementBridgeV2 {
            totals: settlement.totals(0),
            book: None,
            cash_pot_expectation: None,
        });
    }
    let fees = fee_book.ok_or(SettlementAdapterErrorV1::FeeOwnerMismatch)?;
    if fees.settlement_candidate().0 != settlement.candidate.bytes()
        || fees.owner_count() != settlement.owner_count
    {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let mut owner = 0usize;
    while owner < usize::from(settlement.owner_count) {
        if fees.rows()[owner].owner != settlement.participating_owners[owner].bytes() {
            return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
        }
        owner += 1;
    }
    let totals = settlement.totals(fees.selected_fee_atoms());
    let book = build_owner_settlement_book_v2(
        settlement.market.bytes(),
        settlement.epoch.bytes(),
        settlement.candidate.bytes(),
        settlement.owner_order_set_digest.bytes(),
        settlement.price_scale,
        &settlement.settlement_orders,
        settlement.settlement_order_count,
        fees.rows(),
        fees.owner_count(),
        totals,
    )?;
    let selected_fee_atoms = u64::try_from(totals.selected_fee_atoms)
        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let consideration_debit_atoms = book
        .debit_atoms
        .checked_sub(totals.selected_fee_atoms)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let virtual_price_units = settlement
        .virtual_split_price_units
        .checked_add(settlement.virtual_merge_price_units)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let virtual_cash_atoms = virtual_price_units
        .checked_div(u128::from(settlement.price_scale))
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let virtual_cash_direction = if settlement.virtual_split_price_units != 0 {
        VirtualCashDirectionV1::Split
    } else if settlement.virtual_merge_price_units != 0 {
        VirtualCashDirectionV1::Merge
    } else {
        VirtualCashDirectionV1::None
    };
    let cash_pot_expectation = SettlementCashPotExpectationV1 {
        market: settlement.market.bytes(),
        epoch: settlement.epoch.bytes(),
        candidate: settlement.candidate.bytes(),
        owner_order_set_digest: settlement.owner_order_set_digest.bytes(),
        fee_record: if selected_fee_atoms == 0 {
            [0; 32]
        } else {
            fees.fee_record().0
        },
        price_scale: settlement.price_scale,
        owner_count: totals.owner_count,
        consideration_debit_atoms: u64::try_from(consideration_debit_atoms)
            .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
        seller_credit_atoms: u64::try_from(book.credit_atoms)
            .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
        selected_fee_atoms,
        rounding_pot_price_units: totals.rounding_pot_price_units,
        virtual_cash_direction,
        virtual_cash_atoms: u64::try_from(virtual_cash_atoms)
            .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
    };
    cash_pot_expectation.validate()?;
    Ok(OwnerSettlementBridgeV2 {
        totals,
        book: Some(book),
        cash_pot_expectation: Some(cash_pot_expectation),
    })
}

/// Full checked output of canonical feed and settlement construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReadyDirectCandidateV1 {
    candidate: BuiltDirectCandidateV1,
    settlement: CandidateSettlementProjectionV1,
    owner_settlement: OwnerSettlementBridgeV2,
    header: CandidateFeedHeaderV2,
    settlement_witness_digest: Id32,
    candidate_bundle_digest: Id32,
}

impl SettlementReadyDirectCandidateV1 {
    /// Checked bounded-search candidate; its report is not an optimality claim.
    pub const fn candidate(&self) -> &BuiltDirectCandidateV1 {
        &self.candidate
    }

    /// Factual complete/incomplete status for the named search family.
    pub const fn search_report(&self) -> &CandidateSearchReportV1 {
        self.candidate.search_report()
    }

    /// Canonical owner-aware settlement projection.
    pub const fn settlement(&self) -> &CandidateSettlementProjectionV1 {
        &self.settlement
    }

    /// Canonical owner-settlement bridge and exact totals.
    pub const fn owner_settlement(&self) -> &OwnerSettlementBridgeV2 {
        &self.owner_settlement
    }

    /// Canonical sealed CandidateFeedV2 header encoded into caller storage.
    pub const fn header(&self) -> CandidateFeedHeaderV2 {
        self.header
    }

    /// Contract-owned digest of the base candidate and exact active slices.
    pub const fn settlement_witness_digest(&self) -> Id32 {
        self.settlement_witness_digest
    }

    /// Contract-owned typed digest of the complete sealed candidate bundle.
    pub const fn candidate_bundle_digest(&self) -> Id32 {
        self.candidate_bundle_digest
    }
}

/// Serialize and authenticate one complete Direct feed after fee and owner
/// settlement projection.
///
/// `settlement_tail_output` and `candidate_feed_output` must have their exact
/// active widths. The revealed node must already carry the independently
/// derived settlement-witness and candidate-bundle digests; both are
/// recomputed here before success.
#[allow(clippy::too_many_arguments)]
pub fn encode_settlement_ready_direct_candidate_v1(
    candidate: BuiltDirectCandidateV1,
    settlement: CandidateSettlementProjectionV1,
    fee_book: Option<&SelectedOwnerFeeBookV1>,
    settlement_tail_output: &mut [u8],
    candidate_feed_output: &mut [u8],
    candidate_feed_identity: Id32,
    admission_node: &clutch_general_v2_contract::AdmissionNodeV3AccountV1,
    market_binding: &MarketBindingV1,
    economic_domain_account: &EconomicDomainV2AccountV1,
    rent: DeletableRentOwnerV1,
    stored_bump: u8,
) -> Result<SettlementReadyDirectCandidateV1, SettlementAdapterErrorV1> {
    let candidate_id = candidate.base_relation_candidate_id()?;
    if settlement.candidate != candidate_id
        || settlement.market != candidate.market()
        || settlement.epoch != candidate.epoch()
        || settlement.order_set != candidate.order_set()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    settlement.encode_tail(settlement_tail_output)?;
    let settlement_witness_digest = derive_settlement_witness_digest_v1(
        candidate_id,
        settlement_tail_output,
        settlement.slice_count,
    )?;
    if admission_node.settlement_witness_digest != settlement_witness_digest {
        return Err(SettlementAdapterErrorV1::SettlementWitnessMismatch);
    }
    let owner_settlement = bridge_owner_settlement_v2(&settlement, fee_book)?;
    let header = candidate.encode_sealed_candidate_feed_v2(
        candidate_feed_output,
        candidate_feed_identity,
        admission_node,
        market_binding,
        economic_domain_account,
        SettlementTailV1 {
            slice_count: settlement.slice_count,
            encoded_slices: settlement_tail_output,
        },
        rent,
        stored_bump,
    )?;
    let candidate_bundle_digest = derive_candidate_bundle_digest_v1(candidate_feed_output)?;
    if admission_node.candidate_bundle_digest != candidate_bundle_digest {
        return Err(SettlementAdapterErrorV1::CandidateBundleMismatch);
    }
    Ok(SettlementReadyDirectCandidateV1 {
        candidate,
        settlement,
        owner_settlement,
        header,
        settlement_witness_digest,
        candidate_bundle_digest,
    })
}

/// Derive the exact owner/order membership digest from the complete private
/// frozen-page projection.
pub fn derive_owner_order_set_digest_v1(
    projection: &OwnerBlindBookProjectionV1,
    reservations: &AuthenticatedReservationBookV1,
    positions: &AuthenticatedSettlementPositionBookV3,
) -> Result<Id32, SettlementAdapterErrorV1> {
    if reservations.market != projection.market()
        || reservations.epoch != projection.epoch()
        || reservations.order_set != projection.order_set()
        || reservations.price_grid_id != projection.price_grid_id()
        || reservations.order_count != projection.book().len
        || positions.market_runtime != projection.market()
        || positions.market_binding.market_instance_id.bytes()
            != projection.market_binding().market_instance_v2_id.bytes()
        || positions.market_binding.realm_id.bytes() != projection.realm().bytes()
        || positions.market_binding.outcome_count != projection.domain().outcome_count
    {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }
    let mut hash = Sha256::new();
    hash.update(OWNER_ORDER_SET_DIGEST_DOMAIN_V1);
    hash.update(projection.market().bytes());
    hash.update(projection.epoch().bytes());
    hash.update(projection.order_set().bytes());
    hash.update(projection.economic_domain_digest().bytes());
    hash.update(projection.price_grid_id().bytes());
    hash.update(reservations.terms.bytes());
    hash.update(reservations.reservation_policy.bytes());
    hash.update(projection.market_binding().settlement_policy_id.bytes());
    hash.update(positions.market_binding.market_instance_id.bytes());
    hash.update(positions.market_binding.realm_id.bytes());
    hash.update(positions.market_binding.collateral_policy_id.bytes());
    hash.update(positions.market_binding.collateral_release_id.bytes());
    hash.update([positions.market_binding.outcome_count]);
    hash.update([projection.book().len]);
    let mut order = 0usize;
    while order < usize::from(projection.book().len) {
        let index =
            u8::try_from(order).map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
        let membership = projection
            .order_membership(index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let position = positions
            .position_for_owner(membership.owner())
            .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
        hash.update([index]);
        hash.update(membership.order_id().bytes());
        hash.update(membership.owner().bytes());
        hash.update(membership.generation().to_le_bytes());
        hash.update(reservations.reservation_ids[order].bytes());
        hash.update(reservations.position_generations[order].to_le_bytes());
        hash.update([membership.kind().code()]);
        hash.update([match projection.book().orders[order].side {
            Side::Buy => 0,
            Side::Sell => 1,
        }]);
        hash.update(position.account.bytes());
        hash.update(position.data_id.bytes());
        order += 1;
    }
    Id32::new(hash.finalize().into()).map_err(SettlementAdapterErrorV1::Contract)
}

/// Derive the exact settlement-witness identity, including an empty active
/// slice list when the candidate has no fills.
pub fn derive_settlement_witness_digest_v1(
    base_relation_candidate_id: Id32,
    encoded_slices: &[u8],
    slice_count: u16,
) -> Result<Id32, SettlementAdapterErrorV1> {
    let expected = usize::from(slice_count)
        .checked_mul(SETTLEMENT_SLICE_BYTES)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    if base_relation_candidate_id.is_zero() || encoded_slices.len() != expected {
        return Err(SettlementAdapterErrorV1::OutputLengthMismatch);
    }
    contract_settlement_witness_digest_v1(
        &CanonicalSha256,
        base_relation_candidate_id,
        slice_count,
        encoded_slices,
    )
    .map_err(SettlementAdapterErrorV1::Contract)
}

/// Derive the contract-owned bundle identity from a complete sealed
/// `CandidateFeedV2` account.
pub fn derive_candidate_bundle_digest_v1(
    sealed_candidate_feed: &[u8],
) -> Result<Id32, SettlementAdapterErrorV1> {
    if sealed_candidate_feed.is_empty() {
        return Err(SettlementAdapterErrorV1::OutputLengthMismatch);
    }
    contract_candidate_bundle_digest_v1(&CanonicalSha256, sealed_candidate_feed, true)
        .map_err(SettlementAdapterErrorV1::Contract)
}

/// Deterministic refusal set for settlement construction and pure bridges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementAdapterErrorV1 {
    /// Candidate builder or canonical feed encoder refused the join.
    Candidate(CandidateBuilderErrorV1),
    /// General contract codec refused an identity or body.
    Contract(clutch_general_v2_contract::CodecError),
    /// Frozen layout or reservation codec refused hostile bytes.
    Layout(LayoutError),
    /// Canonical Realm-selected collateral binding refused the join.
    Collateral(CollateralError),
    /// Canonical Position V3 codec or purpose projection refused the body.
    Position(RetirementErrorV2),
    /// Owner-settlement contract refused derived rows or totals.
    OwnerSettlement(OwnerSettlementError),
    /// Fee runtime refused a terminal owner or complete fee book.
    Fee(FeeError),
    /// Exact market, epoch, order-set, candidate, or grid binding differed.
    BindingMismatch,
    /// Reservation list was incomplete or disagreed with page semantics.
    ReservationSetMismatch,
    /// Position V3 rows did not exactly cover or bind the frozen owner set.
    PositionSetMismatch,
    /// Owner-aware pairing cannot avoid a real self-pair.
    OwnerPairingInfeasible,
    /// Canonical construction exceeded the frozen 416-slice capacity.
    SliceCapacityExceeded,
    /// Fee rows did not exactly cover canonical participating owners.
    FeeOwnerMismatch,
    /// Authenticated receipt latch was not canonical for the derived end.
    ReceiptLatchMismatch,
    /// Revealed AdmissionNode carried a different settlement-witness digest.
    SettlementWitnessMismatch,
    /// Revealed AdmissionNode carried a different exact candidate-bundle digest.
    CandidateBundleMismatch,
    /// Checked exact arithmetic overflowed.
    ArithmeticOverflow,
    /// Caller storage did not have the exact active width.
    OutputLengthMismatch,
}

impl From<CandidateBuilderErrorV1> for SettlementAdapterErrorV1 {
    fn from(value: CandidateBuilderErrorV1) -> Self {
        Self::Candidate(value)
    }
}

impl From<clutch_general_v2_contract::CodecError> for SettlementAdapterErrorV1 {
    fn from(value: clutch_general_v2_contract::CodecError) -> Self {
        Self::Contract(value)
    }
}

impl From<LayoutError> for SettlementAdapterErrorV1 {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<CollateralError> for SettlementAdapterErrorV1 {
    fn from(value: CollateralError) -> Self {
        Self::Collateral(value)
    }
}

impl From<RetirementErrorV2> for SettlementAdapterErrorV1 {
    fn from(value: RetirementErrorV2) -> Self {
        Self::Position(value)
    }
}

impl From<OwnerSettlementError> for SettlementAdapterErrorV1 {
    fn from(value: OwnerSettlementError) -> Self {
        Self::OwnerSettlement(value)
    }
}

impl From<FeeError> for SettlementAdapterErrorV1 {
    fn from(value: FeeError) -> Self {
        Self::Fee(value)
    }
}

const EMPTY_VERIFIED_SETTLEMENT_ORDER_V2: VerifiedSettlementOrderV2 = VerifiedSettlementOrderV2 {
    owner: [0; 32],
    order_index: 0,
    side: SettlementSideV1::Buy,
    consideration_price_units: PresentConsiderationV2::ABSENT,
    slice_count: 0,
    reserved_cash_atoms: 0,
};

#[derive(Clone, Copy, Debug)]
struct OutcomePairingStateV1 {
    owners: [Id32; MAX_ORDERS],
    owner_count: usize,
    order_index: [u8; MAX_ORDERS],
    owner_slot: [u8; MAX_ORDERS],
    side: [Side; MAX_ORDERS],
    remaining: [u64; MAX_ORDERS],
    leg_count: usize,
    buy_remaining: [u64; MAX_ORDERS],
    sell_remaining: [u64; MAX_ORDERS],
}

impl OutcomePairingStateV1 {
    fn participation(&self, slot: usize) -> Result<u64, SettlementAdapterErrorV1> {
        self.buy_remaining[slot]
            .checked_add(self.sell_remaining[slot])
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)
    }

    const fn side_remaining(&self, slot: usize, side: Side) -> u64 {
        match side {
            Side::Buy => self.buy_remaining[slot],
            Side::Sell => self.sell_remaining[slot],
        }
    }

    fn pick_leg(&self, slot: usize, side: Side) -> Option<usize> {
        let mut best: Option<usize> = None;
        let mut leg = 0usize;
        while leg < self.leg_count {
            if self.remaining[leg] != 0
                && self.side[leg] == side
                && usize::from(self.owner_slot[leg]) == slot
            {
                best = Some(match best {
                    None => leg,
                    Some(current) => {
                        if self.order_index[leg] < self.order_index[current] {
                            leg
                        } else {
                            current
                        }
                    }
                });
            }
            leg += 1;
        }
        best
    }

    fn pick_owner(
        &self,
        side: Side,
        forbidden: Option<usize>,
    ) -> Result<Option<usize>, SettlementAdapterErrorV1> {
        let mut best: Option<usize> = None;
        let mut slot = 0usize;
        while slot < self.owner_count {
            if Some(slot) != forbidden && self.side_remaining(slot, side) != 0 {
                best = Some(match best {
                    None => slot,
                    Some(current) => {
                        let challenger = self.participation(slot)?;
                        let incumbent = self.participation(current)?;
                        if challenger > incumbent
                            || (challenger == incumbent && self.owners[slot] < self.owners[current])
                        {
                            slot
                        } else {
                            current
                        }
                    }
                });
            }
            slot += 1;
        }
        Ok(best)
    }

    fn max_participation_excluding(
        &self,
        first: usize,
        second: Option<usize>,
    ) -> Result<u64, SettlementAdapterErrorV1> {
        let mut maximum = 0u64;
        let mut slot = 0usize;
        while slot < self.owner_count {
            if slot != first && Some(slot) != second {
                maximum = maximum.max(self.participation(slot)?);
            }
            slot += 1;
        }
        Ok(maximum)
    }
}

fn derive_pairing_slices(
    candidate: &BuiltDirectCandidateV1,
    projection: &OwnerBlindBookProjectionV1,
    slices: &mut [CanonicalSettlementSliceV1; MAX_SLICES],
) -> Result<u16, SettlementAdapterErrorV1> {
    let mut emitted = 0usize;
    let mut outcome = 0usize;
    while outcome < usize::from(projection.domain().outcome_count) {
        let mut state = OutcomePairingStateV1 {
            owners: [Id32::ZERO; MAX_ORDERS],
            owner_count: 0,
            order_index: [0; MAX_ORDERS],
            owner_slot: [0; MAX_ORDERS],
            side: [Side::Buy; MAX_ORDERS],
            remaining: [0; MAX_ORDERS],
            leg_count: 0,
            buy_remaining: [0; MAX_ORDERS],
            sell_remaining: [0; MAX_ORDERS],
        };
        let mut order = 0usize;
        while order < usize::from(projection.book().len) {
            let fill = candidate.economic_candidate().fills[order];
            let quantity = projection.book().orders[order].coefficients[outcome]
                .checked_mul(fill)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            if quantity != 0 {
                let order_index = u8::try_from(order)
                    .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
                let owner = projection
                    .order_membership(order_index)
                    .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                    .owner();
                let owner_slot = append_owner(&mut state.owners, &mut state.owner_count, owner)?;
                let leg = state.leg_count;
                state.order_index[leg] = order_index;
                state.owner_slot[leg] = u8::try_from(owner_slot)
                    .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
                state.side[leg] = projection.book().orders[order].side;
                state.remaining[leg] = quantity;
                state.leg_count += 1;
                match projection.book().orders[order].side {
                    Side::Buy => {
                        state.buy_remaining[owner_slot] = state.buy_remaining[owner_slot]
                            .checked_add(quantity)
                            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                    }
                    Side::Sell => {
                        state.sell_remaining[owner_slot] = state.sell_remaining[owner_slot]
                            .checked_add(quantity)
                            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                    }
                }
            }
            order += 1;
        }
        let mut buy_total = 0u64;
        let mut sell_total = 0u64;
        let mut owner = 0usize;
        while owner < state.owner_count {
            buy_total = buy_total
                .checked_add(state.buy_remaining[owner])
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            sell_total = sell_total
                .checked_add(state.sell_remaining[owner])
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            owner += 1;
        }
        let mut split_remaining = candidate.economic_candidate().virtual_split;
        let mut merge_remaining = candidate.economic_candidate().virtual_merge;
        let mut flow_remaining = buy_total
            .checked_add(merge_remaining)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        if flow_remaining
            != sell_total
                .checked_add(split_remaining)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?
        {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        owner = 0;
        while owner < state.owner_count {
            if state.participation(owner)? > flow_remaining {
                return Err(SettlementAdapterErrorV1::OwnerPairingInfeasible);
            }
            owner += 1;
        }
        while flow_remaining != 0 {
            let buy_owner = state.pick_owner(Side::Buy, None)?;
            let sell_owner = state.pick_owner(Side::Sell, None)?;
            let chosen_side = match (buy_owner, sell_owner) {
                (None, None) => return Err(SettlementAdapterErrorV1::OwnerPairingInfeasible),
                (Some(_), None) => Side::Buy,
                (None, Some(_)) => Side::Sell,
                (Some(buy), Some(sell)) => {
                    if state.participation(buy)? >= state.participation(sell)? {
                        Side::Buy
                    } else {
                        Side::Sell
                    }
                }
            };
            let chosen_slot = match chosen_side {
                Side::Buy => buy_owner,
                Side::Sell => sell_owner,
            }
            .ok_or(SettlementAdapterErrorV1::OwnerPairingInfeasible)?;
            let chosen_leg = state
                .pick_leg(chosen_slot, chosen_side)
                .ok_or(SettlementAdapterErrorV1::OwnerPairingInfeasible)?;
            let opposite = match chosen_side {
                Side::Buy => Side::Sell,
                Side::Sell => Side::Buy,
            };
            let virtual_capacity = match chosen_side {
                Side::Buy => split_remaining,
                Side::Sell => merge_remaining,
            };
            let counterparty_slot = state.pick_owner(opposite, Some(chosen_slot))?;
            let use_virtual = match counterparty_slot {
                None => virtual_capacity != 0,
                Some(slot) => state.participation(slot)? < virtual_capacity,
            };
            let (counterparty_leg, counterparty_remaining, counterparty_owner) = if use_virtual {
                (None, virtual_capacity, None)
            } else {
                let slot =
                    counterparty_slot.ok_or(SettlementAdapterErrorV1::OwnerPairingInfeasible)?;
                let leg = state
                    .pick_leg(slot, opposite)
                    .ok_or(SettlementAdapterErrorV1::OwnerPairingInfeasible)?;
                (Some(leg), state.remaining[leg], Some(slot))
            };
            if counterparty_remaining == 0 {
                return Err(SettlementAdapterErrorV1::OwnerPairingInfeasible);
            }
            let blocking = state.max_participation_excluding(chosen_slot, counterparty_owner)?;
            let slack = flow_remaining
                .checked_sub(blocking)
                .ok_or(SettlementAdapterErrorV1::OwnerPairingInfeasible)?;
            if slack == 0 {
                return Err(SettlementAdapterErrorV1::OwnerPairingInfeasible);
            }
            let quantity = min(
                min(state.remaining[chosen_leg], counterparty_remaining),
                slack,
            );
            if quantity == 0 || emitted >= MAX_SLICES {
                return Err(if emitted >= MAX_SLICES {
                    SettlementAdapterErrorV1::SliceCapacityExceeded
                } else {
                    SettlementAdapterErrorV1::OwnerPairingInfeasible
                });
            }
            let chosen_ref = SettlementLegV1::Order(state.order_index[chosen_leg]);
            let counter_ref = match counterparty_leg {
                Some(leg) => SettlementLegV1::Order(state.order_index[leg]),
                None => match chosen_side {
                    Side::Buy => SettlementLegV1::Split,
                    Side::Sell => SettlementLegV1::Merge,
                },
            };
            slices[emitted] = match chosen_side {
                Side::Buy => CanonicalSettlementSliceV1 {
                    buy: chosen_ref,
                    sell: counter_ref,
                    route: if counterparty_leg.is_some() {
                        SettlementRouteV1::Direct
                    } else {
                        SettlementRouteV1::SplitToBuy
                    },
                    outcome: u8::try_from(outcome)
                        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
                    quantity,
                },
                Side::Sell => CanonicalSettlementSliceV1 {
                    buy: counter_ref,
                    sell: chosen_ref,
                    route: if counterparty_leg.is_some() {
                        SettlementRouteV1::Direct
                    } else {
                        SettlementRouteV1::SellToMerge
                    },
                    outcome: u8::try_from(outcome)
                        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
                    quantity,
                },
            };
            emitted += 1;
            state.remaining[chosen_leg] -= quantity;
            match chosen_side {
                Side::Buy => state.buy_remaining[chosen_slot] -= quantity,
                Side::Sell => state.sell_remaining[chosen_slot] -= quantity,
            }
            match (counterparty_leg, counterparty_owner) {
                (Some(leg), Some(slot)) => {
                    state.remaining[leg] -= quantity;
                    match opposite {
                        Side::Buy => state.buy_remaining[slot] -= quantity,
                        Side::Sell => state.sell_remaining[slot] -= quantity,
                    }
                }
                _ => match chosen_side {
                    Side::Buy => split_remaining -= quantity,
                    Side::Sell => merge_remaining -= quantity,
                },
            }
            flow_remaining -= quantity;
        }
        if split_remaining != 0 || merge_remaining != 0 {
            return Err(SettlementAdapterErrorV1::OwnerPairingInfeasible);
        }
        let mut leg = 0usize;
        while leg < state.leg_count {
            if state.remaining[leg] != 0 {
                return Err(SettlementAdapterErrorV1::OwnerPairingInfeasible);
            }
            leg += 1;
        }
        outcome += 1;
    }
    u16::try_from(emitted).map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)
}

fn slice_consideration(
    prices: &[u64; MAX_OUTCOMES],
    slice: CanonicalSettlementSliceV1,
) -> Result<u128, SettlementAdapterErrorV1> {
    u128::from(slice.quantity)
        .checked_mul(u128::from(prices[usize::from(slice.outcome)]))
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)
}

fn derive_projection_receipt_data_id_v2(
    receipt: AuthenticatedSettlementReceiptV2,
    settlement: &CandidateSettlementProjectionV1,
) -> Result<[u8; 32], SettlementAdapterErrorV1> {
    derive_settlement_receipt_data_id_v2(
        receipt,
        settlement.position_book.market_binding.outcome_count,
        &ReceiptDataSha256V2,
    )
    .map_err(SettlementAdapterErrorV1::OwnerSettlement)
}

fn live_adapter_id(id: Id32) -> Result<[u8; 32], SettlementAdapterErrorV1> {
    if id.is_zero() {
        Err(SettlementAdapterErrorV1::BindingMismatch)
    } else {
        Ok(id.bytes())
    }
}

fn order_consideration_price_units(
    candidate: &BuiltDirectCandidateV1,
    projection: &OwnerBlindBookProjectionV1,
    order: usize,
    fill: u64,
) -> Result<u128, SettlementAdapterErrorV1> {
    let mut unit = 0u128;
    let mut outcome = 0usize;
    while outcome < usize::from(candidate.price().native_outcome_count) {
        unit = unit
            .checked_add(
                u128::from(projection.book().orders[order].coefficients[outcome])
                    .checked_mul(u128::from(candidate.price().prices[outcome]))
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?,
            )
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        outcome += 1;
    }
    unit.checked_mul(u128::from(fill))
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)
}

fn order_entitled_units(
    candidate: &BuiltDirectCandidateV1,
    projection: &OwnerBlindBookProjectionV1,
    order: usize,
    fill: u64,
) -> Result<u64, SettlementAdapterErrorV1> {
    let mut units = 0u64;
    let mut outcome = 0usize;
    while outcome < usize::from(candidate.price().native_outcome_count) {
        let leg = projection.book().orders[order].coefficients[outcome]
            .checked_mul(fill)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        units = units
            .checked_add(leg)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        outcome += 1;
    }
    if units == 0 {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(units)
}

fn insert_owner(
    owners: &mut [Id32; MAX_ORDERS],
    owner_count: &mut usize,
    owner: Id32,
) -> Result<usize, SettlementAdapterErrorV1> {
    if owner.is_zero() {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut at = 0usize;
    while at < *owner_count && owners[at] < owner {
        at += 1;
    }
    if at < *owner_count && owners[at] == owner {
        return Ok(at);
    }
    if *owner_count >= MAX_ORDERS {
        return Err(SettlementAdapterErrorV1::ArithmeticOverflow);
    }
    let mut shift = *owner_count;
    while shift > at {
        owners[shift] = owners[shift - 1];
        shift -= 1;
    }
    owners[at] = owner;
    *owner_count += 1;
    Ok(at)
}

/// Stable owner slots for pairing state. Unlike the sorted participant list,
/// these slots are referenced by already-populated legs and therefore must
/// never shift when a later owner is discovered.
fn append_owner(
    owners: &mut [Id32; MAX_ORDERS],
    owner_count: &mut usize,
    owner: Id32,
) -> Result<usize, SettlementAdapterErrorV1> {
    if owner.is_zero() {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut at = 0usize;
    while at < *owner_count {
        if owners[at] == owner {
            return Ok(at);
        }
        at += 1;
    }
    if *owner_count >= MAX_ORDERS {
        return Err(SettlementAdapterErrorV1::ArithmeticOverflow);
    }
    owners[*owner_count] = owner;
    *owner_count += 1;
    Ok(*owner_count - 1)
}
