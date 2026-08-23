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

use clutch_batch::{
    AuthenticatedPortfolioReceiptSiblingSetV2, Side, MAX_ORDERS,
    PORTFOLIO_PAIR_MAX_RECEIPTS_V2,
};
use clutch_collateral_adapter_v2::{BoundCollateralProfileV2, Error as CollateralError};
use clutch_fee_runtime_contract::allocation::{FeeEnvelopeV1, PayerAllocationV1};
use clutch_fee_runtime_contract::intent::OwnerFeeTransitionIntentV1;
use clutch_fee_runtime_contract::projection::{
    assemble_selected_owner_fee_book_v1, project_terminal_owner_fee_v1,
    AuthenticatedSelectedOwnerFeeV1, AuthenticatedSelectedOwnerFeeV4, SelectedOwnerFeeBookV1,
    VerifiedOwnerFeeFundingV1,
};
use clutch_fee_runtime_contract::selected::{
    OwnerFeeAssessmentV1, OwnerFeeCarryV1, SelectedCompositeFeeV1,
};
use clutch_fee_runtime_contract::terminal::OwnerFeeFinalizationOutcomeV2;
use clutch_fee_runtime_contract::{Error as FeeError, Id as FeeId, MAX_FEE_ROWS_V1};
use clutch_general_v2_contract::{
    candidate_bundle_digest_v1 as contract_candidate_bundle_digest_v1, complete_candidate_feed_v2,
    project_general_position_replay_prestate_v1, project_general_replay_transition_v1,
    verify_general_replay_last_transition_v1,
    settlement_witness_digest_v1 as contract_settlement_witness_digest_v1,
    owner_fee_finalization_account_data_id_v4, CandidateFeedHeaderV2,
    ConsumeDirectReceiptEggsPayloadV1, DeletableRentOwnerV1,
    EconomicDomainV2AccountV1, GeneralOrderPageSeedTupleV5, GeneralPositionReplayPrestateV1,
    GeneralReplayTransitionKindV1, GeneralReplayTransitionPlanV1,
    FinalizeMergeReceiptPaymentPayloadV1, GeneralReservationSeedTupleV9, Id32, MarketBindingV1,
    OwnerFeeFinalizationV4AccountV1,
    ReleaseUnfilledReservationPayloadV1,
    OwnerSettlementSeedTupleV5, OwnerSettlementV5AccountV1,
    SettlementCashPotV1AccountV1, SettlementReceiptSeedTupleV5,
    SettlementRootChildStateV1, SettlementRootPhaseV1, SettlementRootV1AccountV1,
    OWNER_SETTLEMENT_ACCOUNT_BYTES_V5, SETTLEMENT_ROOT_ACCOUNT_BYTES,
    MAX_OUTCOMES, MAX_SLICES, SETTLEMENT_SLICE_BYTES,
};
use clutch_owner_settlement::{
    build_owner_settlement_book_v2, build_owner_settlement_expectation_basis_book_v4,
    derive_settlement_receipt_data_id_v2, prepare_realize_owner_cash_semantic_v4,
    recover_owner_cash_position_prestate_v4,
    AuthenticatedOrderMembershipV2, AuthenticatedPositionV3,
    AuthenticatedSettlementReceiptEndV2,
    AuthenticatedSettlementReceiptV2, CandidateSettlementTotalsV2,
    Error as OwnerSettlementError, OrderKindV1, OwnerSettlementBookV2,
    PresentConsiderationV2, PresentPriceV2,
    OwnerSettlementExpectationBasisBookV4, OwnerSettlementExpectationBasisV4,
    OwnerCashRealizationSemanticPlanV4, OwnerSettlementAccumulatorV4,
    OwnerSettlementDispositionV4, OwnerSettlementExpectationV4,
    OwnerSettlementStateV4, PositionSettlementPoststateV3,
    SelectedOwnerFeeV1,
    SettlementCashPotExpectationV1, SettlementCashPotV1,
    SettlementReceiptDataHashV2, SettlementReceiptRouteV2, SettlementSideV1,
    VerifiedSettlementOrderV2, VerifiedSettlementOrderV4, VirtualCashDirectionV1,
};
use clutch_retirement::{
    project_general_position_v3, AdapterPositionMarketBindingV3, AdapterPositionPurposeBindingV3,
    Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Sha256Backend, POSITION_V3_BYTES, POSITION_V3_PDA_PREFIX,
    ReplayV3HashBackend, RetirementErrorV2,
};
use clutch_solana_layout::reservation::{
    canonical_reservation_id, ReservationAccount, ReservationPlan, RESERVATION_ACCOUNT_BYTES,
    RESERVATION_STATE_ACTIVE, RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED,
};
use clutch_solana_layout::reservation_v9::{
    canonical_reservation_id_v9, DeletableRentOwnerV1 as LayoutDeletableRentOwnerV1,
    ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9,
};
use clutch_solana_layout::settlement_receipt_v4::SettlementReceiptAccountV4;
use clutch_solana_layout::order_page_v5::{verify_page_v5, OrderSlotCursorV5};
use clutch_solana_layout::settlement_receipt_v5::{
    SettlementReceiptAccountV5, SettlementReceiptEvidenceV5,
    SettlementReceiptTransitionCommitmentV5, SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
};
use clutch_solana_layout::{
    CodecError as LayoutError, Hash32 as LayoutHash32, OrderSlot,
    RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
    RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_DIRECT, RECEIPT_LEG_MERGE,
    RECEIPT_LEG_SPLIT,
};
use sha2::{Digest, Sha256};

use crate::{
    BuiltDirectCandidateV1, CandidateBuilderErrorV1, CandidateSearchReportV1, CanonicalSha256,
    FrozenOrderKindV1, OwnerBlindBookProjectionV1, OwnerBlindBookProjectionV2, SettlementTailV1,
};

/// SHA-256 domain for the exact owner/order membership projection.
pub const OWNER_ORDER_SET_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/owner-order-set/v1\0";
/// Immutable V5 owner/order membership digest with no mutable account IDs.
pub const OWNER_ORDER_SET_DIGEST_DOMAIN_V2: &[u8] = b"dragons-clutch/owner-order-set/v2\0";
/// Maximum encoded settlement-tail width.
pub const MAX_SETTLEMENT_TAIL_BYTES_V1: usize = MAX_SLICES * SETTLEMENT_SLICE_BYTES;
/// Maximum real receipt ends: two for every direct slice.
pub const MAX_SETTLEMENT_RECEIPT_ENDS_V1: usize = MAX_SLICES * 2;

const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_FEE_ROWS_V1 == MAX_ORDERS);
/// Exact zero-fee action-38 evidence over the counted root, V5 row, and pot.
pub const ZERO_FEE_OWNER_FINALIZATION_EVIDENCE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/general-v2/zero-fee-owner-finalization-evidence/v5\0";
/// Stable per-owner evidence carried by every merge-payment Replay successor.
pub const MERGE_PAYMENT_OWNER_FINALIZATION_EVIDENCE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/general-v2/merge-payment-owner-finalization-evidence/v5\0";
/// Stable action-41 identity for one selected zero-fill Reservation release.
pub const UNFILLED_RESERVATION_RELEASE_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/unfilled-reservation-release-transition/v1\0";
/// Exact action-41 atomic poststate evidence committed into GEN1 Replay.
pub const UNFILLED_RESERVATION_RELEASE_EVIDENCE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/unfilled-reservation-release-evidence/v1\0";
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

/// One SBF-authenticated canonical Reservation account presented to the
/// resumable entitlement materializer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationReservationInputV3<'a> {
    /// Canonical Reservation PDA authenticated by the outer adapter.
    pub account: Id32,
    /// Exact hostile canonical Reservation bytes at the current cursor.
    pub encoded_body: &'a [u8],
}

/// One SBF-authenticated rent-owned Reservation V9 presented to the live V4
/// settlement materializer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationReservationInputV9<'a> {
    /// Canonical Reservation V9 PDA authenticated by the outer adapter.
    pub account: Id32,
    /// Exact hostile 666-byte Reservation V9 prestate.
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
    {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }
    let market_binding = settlement_position_market_binding_v3(projection, collateral)?;

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
        if position.replay_account().bytes() == input.account.bytes()
            || position.owner().bytes() == input.account.bytes()
        {
            return Err(SettlementAdapterErrorV1::PositionSetMismatch);
        }

        let mut owner_reservation_count = 0u64;
        let mut owner_reserved_cash = 0u64;
        let mut expected_generation = 0u64;
        order = 0;
        while order < usize::from(projection.book().len) {
            let membership = projection
                .order_membership(
                    u8::try_from(order)
                        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
                )
                .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
            if membership.owner() == expected_owner {
                let order_generation = reservations.position_generations[order];
                if order_generation == 0
                    || (expected_generation != 0 && expected_generation != order_generation)
                {
                    return Err(SettlementAdapterErrorV1::PositionSetMismatch);
                }
                expected_generation = order_generation;
                owner_reservation_count = owner_reservation_count
                    .checked_add(1)
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                owner_reserved_cash = owner_reserved_cash
                    .checked_add(reservations.reserved_cash_atoms[order])
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            }
            order += 1;
        }
        project_canonical_general_settlement_position_v3(
            position,
            expected_owner,
            expected_generation,
            projection.market(),
            market_binding,
        )?;
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

fn settlement_position_market_binding_v3(
    projection: &OwnerBlindBookProjectionV1,
    collateral: BoundCollateralProfileV2,
) -> Result<AdapterPositionMarketBindingV3, SettlementAdapterErrorV1> {
    if collateral.market().market.bytes()
        != projection.market_binding().market_instance_v2_id.bytes()
        || collateral.realm_bound().realm().realm.bytes() != projection.realm().bytes()
    {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }
    Ok(AdapterPositionMarketBindingV3 {
        market_instance_id: retirement_identity(projection.market_binding().market_instance_v2_id)?,
        outcome_count: projection.domain().outcome_count,
        realm_id: retirement_identity(projection.realm())?,
        collateral_policy_id: retirement_identity(Id32::new(collateral.policy_id().bytes())?)?,
        collateral_release_id: retirement_identity(Id32::new(collateral.release().id()?.bytes())?)?,
    })
}

fn retirement_identity(value: Id32) -> Result<Identity32V1, SettlementAdapterErrorV1> {
    Identity32V1::new(value.bytes()).map_err(|_| SettlementAdapterErrorV1::BindingMismatch)
}

/// Bind the one canonical ordinary-General Position purpose profile.
///
/// General settlement never accepts a caller-selected controller or purpose
/// binding. The semantic owner is the controller and the authenticated
/// MarketRuntime PDA is the purpose binding. This keeps Position and Replay
/// addresses derivable from frozen owner/generation facts while leaving
/// Dealer, Series, and StructuredClaim purpose profiles in their own owners.
fn project_canonical_general_settlement_position_v3(
    position: PositionAccountV3,
    expected_owner: Id32,
    expected_generation: u64,
    market_runtime: Id32,
    market_binding: AdapterPositionMarketBindingV3,
) -> Result<(), SettlementAdapterErrorV1> {
    if expected_generation == 0
        || market_runtime.is_zero()
        || position.lifecycle() != PositionLifecycleV3::Open
        || position.owner().bytes() != expected_owner.bytes()
        || position.controller().bytes() != expected_owner.bytes()
        || position.purpose_binding_id().bytes() != market_runtime.bytes()
        || position.generation() != expected_generation
    {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }
    let purpose_binding = AdapterPositionPurposeBindingV3 {
        owner: retirement_identity(expected_owner)?,
        controller: retirement_identity(expected_owner)?,
        purpose_binding_id: retirement_identity(market_runtime)?,
    };
    project_general_position_v3(position, market_binding, purpose_binding)?;
    Ok(())
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FeedSliceRecordV3 {
    buy_kind: u8,
    buy_index: u8,
    sell_kind: u8,
    sell_index: u8,
    outcome: u8,
    quantity: u64,
}

impl FeedSliceRecordV3 {
    fn route(self) -> Result<SettlementReceiptRouteV2, SettlementAdapterErrorV1> {
        match (
            self.buy_kind,
            self.buy_index,
            self.sell_kind,
            self.sell_index,
        ) {
            (0, _, 0, _) => Ok(SettlementReceiptRouteV2::Direct),
            (0, _, 1, 0) => Ok(SettlementReceiptRouteV2::SplitToBuy),
            (2, 0, 0, _) => Ok(SettlementReceiptRouteV2::SellToMerge),
            _ => Err(SettlementAdapterErrorV1::BindingMismatch),
        }
    }
}

fn read_feed_slice_v3(
    slices: &[u8],
    index: u16,
) -> Result<FeedSliceRecordV3, SettlementAdapterErrorV1> {
    let at = usize::from(index)
        .checked_mul(SETTLEMENT_SLICE_BYTES)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let record = slices
        .get(at..at + SETTLEMENT_SLICE_BYTES)
        .ok_or(SettlementAdapterErrorV1::OutputLengthMismatch)?;
    let mut quantity = [0u8; 8];
    quantity.copy_from_slice(&record[5..13]);
    let value = FeedSliceRecordV3 {
        buy_kind: record[0],
        buy_index: record[1],
        sell_kind: record[2],
        sell_index: record[3],
        outcome: record[4],
        quantity: u64::from_le_bytes(quantity),
    };
    value.route()?;
    if value.quantity == 0 {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(value)
}

fn read_feed_u64(bytes: &[u8], index: usize) -> Result<u64, SettlementAdapterErrorV1> {
    let at = index
        .checked_mul(8)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let value = bytes
        .get(at..at + 8)
        .ok_or(SettlementAdapterErrorV1::OutputLengthMismatch)?;
    let mut encoded = [0u8; 8];
    encoded.copy_from_slice(value);
    Ok(u64::from_le_bytes(encoded))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SettlementCoordinatesV4 {
    epoch: Id32,
    market: Id32,
    retained_feed: Id32,
    candidate_bundle_digest: Id32,
    market_binding: Id32,
    source_admission_node: Id32,
    order_set: Id32,
    settlement_candidate_id: Id32,
    settlement_witness_digest: Id32,
    epoch_generation: u64,
    slice_count: u16,
}

fn settlement_coordinates_v4(
    root: &SettlementRootV1AccountV1,
) -> Result<SettlementCoordinatesV4, SettlementAdapterErrorV1> {
    root.validate()?;
    Ok(SettlementCoordinatesV4 {
        epoch: root.epoch(),
        market: root.market(),
        retained_feed: root.retained_feed(),
        candidate_bundle_digest: root.candidate_bundle_digest(),
        market_binding: root.market_binding(),
        source_admission_node: root.source_admission_node(),
        order_set: root.order_set(),
        settlement_candidate_id: root.settlement_candidate_id(),
        settlement_witness_digest: root.settlement_witness_digest(),
        epoch_generation: root.epoch_generation(),
        slice_count: root.counts().expected_receipts,
    })
}

#[derive(Clone, Copy, Debug)]
struct FeedSettlementFactsV3 {
    prices: [u64; MAX_OUTCOMES],
    slices: [CanonicalSettlementSliceV1; MAX_SLICES],
    entitled_units: [u64; MAX_ORDERS],
    consideration_price_units: [u128; MAX_ORDERS],
    end_count: [u16; MAX_ORDERS],
    merge_delivery_count: [u16; MAX_ORDERS],
    first_slice: [u16; MAX_ORDERS],
    receipt_end_count: u16,
}


/// Exhaustive pre-root traversal over the sealed Feed and complete frozen V5 book.
///
/// This is a structural verifier output, not an execution capability. Action
/// 39 must join it to the authenticated winning Window/Node/Feed chain before
/// it may create a SettlementRoot. It deliberately contains no mutable
/// Reservation or Position body and no legacy SelectedCandidate projection.
#[derive(Clone, Copy, Debug)]
pub struct SettlementTraversalProjectionV4 {
    selected_feed_account: Id32,
    candidate_bundle_digest: Id32,
    feed: CandidateFeedHeaderV2,
    order_projection: OwnerBlindBookProjectionV2,
    owner_order_set_digest: Id32,
    terms: Id32,
    reservation_policy: Id32,
    position_market_binding: AdapterPositionMarketBindingV3,
    settlement_memberships: [Option<AuthenticatedOrderMembershipV2>; MAX_ORDERS],
    owner_basis: OwnerSettlementExpectationBasisBookV4,
    expected_reservation_count: u16,
    expected_filled_reservation_count: u16,
    expected_merge_payment_count: u16,
    virtual_cash_direction: VirtualCashDirectionV1,
    virtual_cash_atoms: u64,
    first_slice: [u16; MAX_ORDERS],
    slices: [CanonicalSettlementSliceV1; MAX_SLICES],
    prices: [u64; MAX_OUTCOMES],
}

impl SettlementTraversalProjectionV4 {
    /// Exact retained sealed Feed account presented to the traversal.
    pub const fn selected_feed_account(&self) -> Id32 {
        self.selected_feed_account
    }

    /// Exact digest of the full sealed Feed body.
    pub const fn candidate_bundle_digest(&self) -> Id32 {
        self.candidate_bundle_digest
    }

    /// Exact checked retained Feed header.
    pub const fn feed(&self) -> CandidateFeedHeaderV2 {
        self.feed
    }

    /// Complete generation-bearing V5 order projection.
    pub const fn order_projection(&self) -> &OwnerBlindBookProjectionV2 {
        &self.order_projection
    }

    /// Immutable candidate-wide membership digest excluding mutable bodies.
    pub const fn owner_order_set_digest(&self) -> Id32 {
        self.owner_order_set_digest
    }

    /// Exact immutable Reservation terms identity.
    pub const fn terms(&self) -> Id32 {
        self.terms
    }

    /// Exact immutable Reservation policy identity.
    pub const fn reservation_policy(&self) -> Id32 {
        self.reservation_policy
    }

    /// Full immutable MarketInstance/Realm/policy/release Position binding.
    pub const fn position_market_binding(&self) -> AdapterPositionMarketBindingV3 {
        self.position_market_binding
    }

    /// Canonical filled-order membership at one dense V5 order index.
    pub fn settlement_membership(
        &self,
        order_index: u8,
    ) -> Option<AuthenticatedOrderMembershipV2> {
        if order_index < self.feed.order_count {
            self.settlement_memberships[usize::from(order_index)]
        } else {
            None
        }
    }

    /// Exact owner-sorted, no-cash-summary pre-fee expectation basis.
    pub const fn owner_basis(&self) -> &OwnerSettlementExpectationBasisBookV4 {
        &self.owner_basis
    }

    /// Every frozen live order Reservation, including zero-fill orders.
    pub const fn expected_reservation_count(&self) -> u16 {
        self.expected_reservation_count
    }

    /// Distinct filled Reservations admitted later by action 24.
    pub const fn expected_filled_reservation_count(&self) -> u16 {
        self.expected_filled_reservation_count
    }

    /// Merge receipts requiring the later action-40 payment latch.
    pub const fn expected_merge_payment_count(&self) -> u16 {
        self.expected_merge_payment_count
    }

    /// Canonical direct/split/merge direction derived from the sealed Feed.
    pub const fn virtual_cash_direction(&self) -> VirtualCashDirectionV1 {
        self.virtual_cash_direction
    }

    /// Exact selected complete-set principal derived from the sealed Feed.
    pub const fn virtual_cash_atoms(&self) -> u64 {
        self.virtual_cash_atoms
    }

    /// First canonical selected slice containing one filled real order.
    pub fn first_slice(&self, order_index: u8) -> Option<u16> {
        if order_index < self.feed.order_count {
            let value = self.first_slice[usize::from(order_index)];
            if value == u16::MAX { None } else { Some(value) }
        } else {
            None
        }
    }

    /// Canonical retained-Feed settlement slice at one authenticated index.
    pub fn settlement_slice(&self, slice_index: u16) -> Option<CanonicalSettlementSliceV1> {
        if slice_index < self.feed.slice_count {
            Some(self.slices[usize::from(slice_index)])
        } else {
            None
        }
    }

    /// Exact alias used by the private Portfolio sibling constructor.
    pub fn slice(&self, slice_index: u16) -> Option<CanonicalSettlementSliceV1> {
        self.settlement_slice(slice_index)
    }

    /// Canonical selected price at one active outcome.
    pub fn outcome_price(&self, outcome: u8) -> Option<u64> {
        if outcome < self.feed.outcome_count {
            Some(self.prices[usize::from(outcome)])
        } else {
            None
        }
    }

    /// Exact alias used by the private Portfolio sibling constructor.
    pub fn price(&self, outcome: u8) -> Option<u64> {
        self.outcome_price(outcome)
    }
}

/// Root-bound next-slice projection used only after action 39.
#[derive(Clone, Copy, Debug)]
pub struct CandidateEntitlementProjectionV4 {
    settlement_root_account: Id32,
    settlement_root: SettlementRootV1AccountV1,
    traversal: SettlementTraversalProjectionV4,
    current_slice: CanonicalSettlementSliceV1,
    current_price: u64,
    current_buy_first_owner: bool,
    current_sell_first_owner: bool,
}

impl CandidateEntitlementProjectionV4 {
    /// Counted SettlementRoot account advanced atomically by action 24.
    pub const fn settlement_root_account(&self) -> Id32 {
        self.settlement_root_account
    }

    /// Exact structural SettlementRoot prestate.
    pub const fn settlement_root(&self) -> &SettlementRootV1AccountV1 {
        &self.settlement_root
    }

    /// Exhaustive immutable traversal joined to this root.
    pub const fn traversal(&self) -> &SettlementTraversalProjectionV4 {
        &self.traversal
    }

    /// Counted retained sealed Feed account.
    pub const fn selected_feed_account(&self) -> Id32 {
        self.traversal.selected_feed_account
    }

    /// Exact checked retained Feed header.
    pub const fn feed(&self) -> CandidateFeedHeaderV2 {
        self.traversal.feed
    }

    /// Complete generation-bearing V5 order projection.
    pub const fn order_projection(&self) -> &OwnerBlindBookProjectionV2 {
        &self.traversal.order_projection
    }

    /// Immutable candidate-wide membership digest excluding mutable bodies.
    pub const fn owner_order_set_digest(&self) -> Id32 {
        self.traversal.owner_order_set_digest
    }

    /// Exact immutable Reservation terms identity.
    pub const fn terms(&self) -> Id32 {
        self.traversal.terms
    }

    /// Exact immutable Reservation policy identity.
    pub const fn reservation_policy(&self) -> Id32 {
        self.traversal.reservation_policy
    }

    /// Full immutable MarketInstance/Realm/policy/release Position binding.
    pub const fn position_market_binding(&self) -> AdapterPositionMarketBindingV3 {
        self.traversal.position_market_binding
    }

    /// Canonical filled-order membership at one dense V5 order index.
    pub fn settlement_membership(
        &self,
        order_index: u8,
    ) -> Option<AuthenticatedOrderMembershipV2> {
        self.traversal.settlement_membership(order_index)
    }

    /// Exact owner-sorted, no-cash-summary pre-fee expectation basis.
    pub const fn owner_basis(&self) -> &OwnerSettlementExpectationBasisBookV4 {
        &self.traversal.owner_basis
    }

    /// Every frozen live order Reservation, including zero-fill orders.
    pub const fn expected_reservation_count(&self) -> u16 {
        self.traversal.expected_reservation_count
    }

    /// Distinct filled Reservations admitted by action 24.
    pub const fn expected_filled_reservation_count(&self) -> u16 {
        self.traversal.expected_filled_reservation_count
    }

    /// Merge receipts requiring the later action-40 payment latch.
    pub const fn expected_merge_payment_count(&self) -> u16 {
        self.traversal.expected_merge_payment_count
    }

    /// First canonical selected slice containing one filled real order.
    pub fn first_slice(&self, order_index: u8) -> Option<u16> {
        self.traversal.first_slice(order_index)
    }

    /// Exact canonical slice at the selected materialization cursor.
    pub const fn current_slice(&self) -> CanonicalSettlementSliceV1 {
        self.current_slice
    }

    /// Exact selected price for the current slice outcome, including zero.
    pub const fn current_price(&self) -> u64 {
        self.current_price
    }
}

/// Reconstruct the complete immutable pre-root basis without reading any
/// current Reservation or Position body.
#[allow(clippy::too_many_arguments)]
pub fn derive_settlement_traversal_projection_v4(
    selected_feed_account: Id32,
    selected_feed_body: &[u8],
    order_projection: &OwnerBlindBookProjectionV2,
    expected_terms: Id32,
    expected_reservation_policy: Id32,
    collateral: BoundCollateralProfileV2,
) -> Result<SettlementTraversalProjectionV4, SettlementAdapterErrorV1> {
    let projection = order_projection.base();
    let (feed, tail) = complete_candidate_feed_v2(selected_feed_body, true)?;
    let feed_bundle_id = derive_candidate_bundle_digest_v1(selected_feed_body)?;
    if selected_feed_account.is_zero()
        || feed_bundle_id.is_zero()
        || feed.slice_count == 0
        || projection.market() != feed.market
        || projection.epoch() != feed.epoch
        || projection.order_set() != feed.order_set
        || projection.economic_domain_digest() != feed.economic_domain_digest
        || projection.market_binding().relation_policy_id != feed.relation_policy_id
        || projection.market_binding().price_measure_policy_v1_id
            != feed.price_measure_policy_v1_id
        || projection.market_binding().native_claim_basis_id != feed.native_claim_basis_id
        || projection.domain().price_scale != feed.price_scale
        || projection.domain().outcome_count != feed.outcome_count
        || projection.book().len != feed.order_count
        || expected_terms.is_zero()
        || expected_reservation_policy.is_zero()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let position_market_binding = settlement_position_market_binding_v3(projection, collateral)?;
    let facts = derive_feed_settlement_facts_v3(
        feed,
        tail.prices_le(),
        tail.fills_le(),
        tail.slices_le(),
        projection,
    )?;
    let owner_order_set_digest = derive_owner_order_set_digest_v2(
        order_projection,
        expected_terms,
        expected_reservation_policy,
        position_market_binding,
    )?;
    let mut settlement_memberships = [None; MAX_ORDERS];
    let mut orders = [VerifiedSettlementOrderV4 {
        owner: [0; 32],
        order_index: 0,
        side: SettlementSideV1::Buy,
        consideration_price_units: PresentConsiderationV2::ABSENT,
        slice_count: 0,
        merge_delivery_count: 0,
    }; MAX_ORDERS];
    let mut order_len = 0usize;
    let mut order = 0usize;
    while order < usize::from(feed.order_count) {
        if facts.end_count[order] != 0 {
            let order_index =
                u8::try_from(order).map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
            let frozen = projection
                .order_membership(order_index)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
            let position_generation = order_projection
                .position_generation(order_index)
                .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
            let reservation = canonical_reservation_id_v9(
                LayoutHash32(frozen_identity(projection.market())),
                LayoutHash32(frozen_identity(projection.epoch())),
                LayoutHash32(frozen_identity(frozen.owner())),
                position_generation,
                LayoutHash32(frozen_identity(frozen.order_id())),
            );
            let side = match projection.book().orders[order].side {
                Side::Buy => SettlementSideV1::Buy,
                Side::Sell => SettlementSideV1::Sell,
            };
            let (order_kind, single_outcome) = match frozen.slot() {
                OrderSlot::Single(record) => (OrderKindV1::Single, record.outcome),
                OrderSlot::Portfolio(_) => (OrderKindV1::Portfolio, u8::MAX),
                OrderSlot::Empty | OrderSlot::Tombstone(_) => {
                    return Err(SettlementAdapterErrorV1::BindingMismatch)
                }
            };
            let membership = AuthenticatedOrderMembershipV2 {
                market: projection.market().bytes(),
                epoch: projection.epoch().bytes(),
                candidate: feed.settlement_candidate_id.bytes(),
                owner_order_set_digest: owner_order_set_digest.bytes(),
                order_id: frozen.order_id().bytes(),
                reservation: reservation.bytes(),
                owner: frozen.owner().bytes(),
                order_index,
                order_generation: frozen.generation(),
                position_generation,
                side,
                order_kind,
                outcome_count: feed.outcome_count,
                single_outcome,
                entitled_units: facts.entitled_units[order],
                entitled_consideration_price_units: PresentConsiderationV2::new(
                    facts.consideration_price_units[order],
                ),
            };
            membership.validate()?;
            settlement_memberships[order] = Some(membership);
            orders[order_len] = VerifiedSettlementOrderV4 {
                owner: frozen.owner().bytes(),
                order_index,
                side,
                consideration_price_units: PresentConsiderationV2::new(
                    facts.consideration_price_units[order],
                ),
                slice_count: facts.end_count[order],
                merge_delivery_count: facts.merge_delivery_count[order],
            };
            order_len += 1;
        }
        order += 1;
    }
    if order_len == 0 {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let owner_basis = build_owner_settlement_expectation_basis_book_v4(
        projection.market().bytes(),
        projection.epoch().bytes(),
        feed.settlement_candidate_id.bytes(),
        owner_order_set_digest.bytes(),
        feed.price_scale,
        &orders,
        u8::try_from(order_len).map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
    )?;
    let mut expected_merge_payment_count = 0u16;
    let mut owner_ordinal = 0u16;
    while owner_ordinal < owner_basis.owner_count() {
        let basis = owner_basis
            .row(owner_ordinal)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        expected_merge_payment_count = expected_merge_payment_count
            .checked_add(basis.expected_merge_delivery_count())
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        owner_ordinal = owner_ordinal
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    let mut feed_merge_payment_count = 0u16;
    order = 0;
    while order < usize::from(feed.order_count) {
        feed_merge_payment_count = feed_merge_payment_count
            .checked_add(facts.merge_delivery_count[order])
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        order += 1;
    }
    if expected_merge_payment_count != feed_merge_payment_count {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let (virtual_cash_direction, virtual_cash_atoms) = match (feed.virtual_split, feed.virtual_merge)
    {
        (0, 0) => (VirtualCashDirectionV1::None, 0),
        (split, 0) => (VirtualCashDirectionV1::Split, split),
        (0, merge) => (VirtualCashDirectionV1::Merge, merge),
        _ => return Err(SettlementAdapterErrorV1::BindingMismatch),
    };
    if (virtual_cash_direction == VirtualCashDirectionV1::Merge)
        != (expected_merge_payment_count != 0)
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(SettlementTraversalProjectionV4 {
        selected_feed_account,
        candidate_bundle_digest: feed_bundle_id,
        feed,
        order_projection: *order_projection,
        owner_order_set_digest,
        terms: expected_terms,
        reservation_policy: expected_reservation_policy,
        position_market_binding,
        settlement_memberships,
        owner_basis,
        expected_reservation_count: u16::from(feed.order_count),
        expected_filled_reservation_count: u16::try_from(order_len)
            .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
        expected_merge_payment_count,
        virtual_cash_direction,
        virtual_cash_atoms,
        first_slice: facts.first_slice,
        slices: facts.slices,
        prices: facts.prices,
    })
}

/// Bind the exhaustive traversal to the counted post-action-39 root and derive
/// the sole next action-24 slice. No legacy SelectedCandidate is accepted.
pub fn derive_candidate_entitlement_projection_v4(
    settlement_root_account: Id32,
    settlement_root: &SettlementRootV1AccountV1,
    traversal: &SettlementTraversalProjectionV4,
) -> Result<CandidateEntitlementProjectionV4, SettlementAdapterErrorV1> {
    require_root_traversal_binding_v4(settlement_root_account, settlement_root, traversal)?;
    let counts = settlement_root.counts();
    if settlement_root.phase() != SettlementRootPhaseV1::Materializing
        || counts.admitted_receipts >= counts.expected_receipts
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let cursor = counts.admitted_receipts;
    let current_slice = traversal.slices[usize::from(cursor)];
    let current_price = traversal.prices[usize::from(current_slice.outcome)];
    let projection = traversal.order_projection.base();
    let current_buy_first_owner = match current_slice.buy {
        SettlementLegV1::Order(order_index) => !owner_appeared_before_slice_v4(
            projection,
            &traversal.slices,
            cursor,
            projection
                .order_membership(order_index)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                .owner(),
        )?,
        SettlementLegV1::Split | SettlementLegV1::Merge => false,
    };
    let current_sell_first_owner = match current_slice.sell {
        SettlementLegV1::Order(order_index) => !owner_appeared_before_slice_v4(
            projection,
            &traversal.slices,
            cursor,
            projection
                .order_membership(order_index)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                .owner(),
        )?,
        SettlementLegV1::Split | SettlementLegV1::Merge => false,
    };
    Ok(CandidateEntitlementProjectionV4 {
        settlement_root_account,
        settlement_root: *settlement_root,
        traversal: *traversal,
        current_slice,
        current_price,
        current_buy_first_owner,
        current_sell_first_owner,
    })
}

/// Rebind one semantic owner to the exhaustive action-39 traversal after the
/// counted root exists. This returns immutable BasisV4 only; it grants no
/// cursor, fee, cash, replay, or account-write authority.
pub fn derive_root_owner_basis_v4(
    settlement_root_account: Id32,
    settlement_root: &SettlementRootV1AccountV1,
    traversal: &SettlementTraversalProjectionV4,
    owner: Id32,
) -> Result<OwnerSettlementExpectationBasisV4, SettlementAdapterErrorV1> {
    require_root_traversal_binding_v4(settlement_root_account, settlement_root, traversal)?;
    if !matches!(
        settlement_root.phase(),
        SettlementRootPhaseV1::Materializing | SettlementRootPhaseV1::Settling
    ) {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    traversal
        .owner_basis
        .row_for_owner(owner.bytes())
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)
}

/// Exact local facts required to allocate one future rent-owned settlement child.
///
/// The full rent minimum is always paid by `payer`; hostile prefunding never
/// discounts that principal and is persisted as the donation floor. This is a
/// structural pure input. The SBF adapter remains responsible for deriving the
/// account PDA and authenticating every account/meta fact before allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentOwnedSettlementCreateFundingV5 {
    /// Dragon's Clutch program identity assigned after allocation.
    pub program_id: Id32,
    /// Sole payer and eventual refundable-principal recipient.
    pub payer: Id32,
    /// Canonical System Program identity.
    pub system_program_id: Id32,
    /// Current payer lamports.
    pub payer_lamports: u64,
    /// Hostile prefund already parked at the canonical target.
    pub target_lamports_before: u64,
    /// Current target owner; fresh creation requires System Program.
    pub target_owner_before: Id32,
    /// Current target data length; fresh creation requires zero.
    pub target_data_len_before: u32,
    /// Whether the target meta is writable.
    pub target_writable: bool,
    /// Executable targets can never become settlement children.
    pub target_executable: bool,
    /// Exact rent-exempt minimum for the successor account width.
    pub rent_minimum: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RentOwnedSettlementCreatePlanV5 {
    payer_debit_lamports: u64,
    target_lamports_after: u64,
    contract_rent: DeletableRentOwnerV1,
    layout_rent: LayoutDeletableRentOwnerV1,
}

fn prepare_rent_owned_settlement_creation_v5(
    account: Id32,
    funding: RentOwnedSettlementCreateFundingV5,
) -> Result<RentOwnedSettlementCreatePlanV5, SettlementAdapterErrorV1> {
    if account.is_zero()
        || funding.program_id.is_zero()
        || funding.payer.is_zero()
        || funding.system_program_id.is_zero()
        || account == funding.program_id
        || account == funding.payer
        || account == funding.system_program_id
        || funding.program_id == funding.payer
        || funding.program_id == funding.system_program_id
        || funding.payer == funding.system_program_id
        || funding.target_owner_before != funding.system_program_id
        || funding.target_data_len_before != 0
        || !funding.target_writable
        || funding.target_executable
        || funding.rent_minimum == 0
        || funding.payer_lamports < funding.rent_minimum
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let target_lamports_after = funding
        .target_lamports_before
        .checked_add(funding.rent_minimum)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let contract_rent = DeletableRentOwnerV1 {
        payer: funding.payer,
        refundable_principal: funding.rent_minimum,
        donation_floor: funding.target_lamports_before,
    };
    contract_rent.validate()?;
    let layout_rent = LayoutDeletableRentOwnerV1 {
        payer: LayoutHash32(funding.payer.bytes()),
        refundable_principal: funding.rent_minimum,
        donation_floor: funding.target_lamports_before,
    };
    layout_rent.validate()?;
    Ok(RentOwnedSettlementCreatePlanV5 {
        payer_debit_lamports: funding.rent_minimum,
        target_lamports_after,
        contract_rent,
        layout_rent,
    })
}

/// Exact hostile rent-owned OwnerSettlement V5 account view.
///
/// This type deliberately does not claim account/PDA authority. A live adapter
/// must rederive the fresh V5 seed address and authenticate the program owner,
/// writable meta, bump, and exact bytes before using the returned projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccountViewV5<'a> {
    /// Presented owner-row account identity.
    pub account: Id32,
    /// Presented program owner.
    pub program_owner: Id32,
    /// Exact hostile 340-byte account body.
    pub exact_body: &'a [u8],
    /// Current account lamports.
    pub lamports: u64,
    /// Current rent-exempt minimum for 340 bytes.
    pub rent_minimum: u64,
    /// Canonical V5 PDA bump rederived by the adapter.
    pub canonical_bump: u8,
    /// Whether the account is writable for this action.
    pub writable: bool,
}

/// Structural exact projection of one canonical rent-owned owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccountProjectionV5 {
    account: Id32,
    program_owner: Id32,
    seed: OwnerSettlementSeedTupleV5,
    envelope: OwnerSettlementV5AccountV1,
    exact_body: [u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5],
    data_id: Id32,
    lamports: u64,
    rent_minimum: u64,
}

impl OwnerSettlementAccountProjectionV5 {
    /// Canonical V5 row account presented by the adapter.
    pub const fn account(&self) -> Id32 {
        self.account
    }

    /// Expected Dragon's Clutch program owner.
    pub const fn program_owner(&self) -> Id32 {
        self.program_owner
    }

    /// Fresh V5 seed tuple committed by the semantic row.
    pub const fn seed(&self) -> OwnerSettlementSeedTupleV5 {
        self.seed
    }

    /// Exact decoded rent-owned envelope.
    pub const fn envelope(&self) -> OwnerSettlementV5AccountV1 {
        self.envelope
    }

    /// Exact current 340-byte account body.
    pub const fn exact_body(&self) -> &[u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5] {
        &self.exact_body
    }

    /// Full V5 account-data identity, including rent owner and stored bump.
    pub const fn data_id(&self) -> Id32 {
        self.data_id
    }

    /// Current account lamports.
    pub const fn lamports(&self) -> u64 {
        self.lamports
    }

    /// Current rent-exempt minimum.
    pub const fn rent_minimum(&self) -> u64 {
        self.rent_minimum
    }
}

/// Decode and structurally bind one exact rent-owned OwnerSettlement V5 row.
pub fn project_owner_settlement_account_v5(
    view: OwnerSettlementAccountViewV5<'_>,
    expected_program_id: Id32,
    seed: OwnerSettlementSeedTupleV5,
) -> Result<OwnerSettlementAccountProjectionV5, SettlementAdapterErrorV1> {
    if !view.writable {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    project_owner_settlement_account_v5_meta_neutral(view, expected_program_id, seed)
}

/// Decode one exact read-only V5 owner row for delivery comparison.
///
/// Action 26 never mutates the row, so requiring a writable lock would create
/// needless contention. The live adapter must still authenticate the same PDA,
/// owner, bump, body, and read-only meta before calling this structural seam.
pub fn project_owner_settlement_account_v5_readonly(
    view: OwnerSettlementAccountViewV5<'_>,
    expected_program_id: Id32,
    seed: OwnerSettlementSeedTupleV5,
) -> Result<OwnerSettlementAccountProjectionV5, SettlementAdapterErrorV1> {
    if view.writable {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    project_owner_settlement_account_v5_meta_neutral(view, expected_program_id, seed)
}

fn project_owner_settlement_account_v5_meta_neutral(
    view: OwnerSettlementAccountViewV5<'_>,
    expected_program_id: Id32,
    seed: OwnerSettlementSeedTupleV5,
) -> Result<OwnerSettlementAccountProjectionV5, SettlementAdapterErrorV1> {
    if view.account.is_zero()
        || expected_program_id.is_zero()
        || view.account == expected_program_id
        || view.program_owner != expected_program_id
        || view.exact_body.len() != OWNER_SETTLEMENT_ACCOUNT_BYTES_V5
        || view.rent_minimum == 0
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let envelope = OwnerSettlementV5AccountV1::decode(view.exact_body)?;
    let expectation = envelope.semantic.expectation();
    if envelope.stored_bump != view.canonical_bump
        || seed.epoch() != &expectation.epoch()
        || seed.settlement_candidate() != &expectation.candidate()
        || seed.owner() != &expectation.owner()
        || view.lamports < view.rent_minimum
        || view.lamports
            < envelope
                .rent
                .refundable_principal
                .checked_add(envelope.rent.donation_floor)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let exact_body = envelope.encode_exact()?;
    if exact_body.as_slice() != view.exact_body {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let data_id = envelope.data_id(&CanonicalSha256)?;
    Ok(OwnerSettlementAccountProjectionV5 {
        account: view.account,
        program_owner: expected_program_id,
        seed,
        envelope,
        exact_body,
        data_id,
        lamports: view.lamports,
        rent_minimum: view.rent_minimum,
    })
}

/// Exact full-rent creation plan for one pristine OwnerSettlement V5 row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementCreatePlanV5 {
    account: Id32,
    program_id: Id32,
    seed: OwnerSettlementSeedTupleV5,
    bump: u8,
    payer: Id32,
    payer_debit_lamports: u64,
    target_lamports_after: u64,
    envelope: OwnerSettlementV5AccountV1,
    exact_body: [u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5],
    data_id: Id32,
}

impl OwnerSettlementCreatePlanV5 {
    pub const fn account(&self) -> Id32 { self.account }
    pub const fn program_id(&self) -> Id32 { self.program_id }
    pub const fn seed(&self) -> OwnerSettlementSeedTupleV5 { self.seed }
    pub const fn bump(&self) -> u8 { self.bump }
    pub const fn payer(&self) -> Id32 { self.payer }
    pub const fn payer_debit_lamports(&self) -> u64 { self.payer_debit_lamports }
    pub const fn target_lamports_after(&self) -> u64 { self.target_lamports_after }
    pub const fn envelope(&self) -> OwnerSettlementV5AccountV1 { self.envelope }
    pub const fn exact_body(&self) -> &[u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5] {
        &self.exact_body
    }
    pub const fn data_id(&self) -> Id32 { self.data_id }
}

/// Create the exact pristine V5 row from a verifier-derived owner expectation.
///
/// This is a structural constructor, not execution authority. Action 24 must
/// first derive `expectation` from the exhaustive root-bound traversal and the
/// authenticated fee snapshot, then atomically join this plan to the root
/// child-count successor.
pub fn prepare_create_owner_settlement_account_v5(
    account: Id32,
    seed: OwnerSettlementSeedTupleV5,
    bump: u8,
    expectation: OwnerSettlementExpectationV4,
    funding: RentOwnedSettlementCreateFundingV5,
) -> Result<OwnerSettlementCreatePlanV5, SettlementAdapterErrorV1> {
    expectation.validate()?;
    if seed.epoch() != &expectation.epoch()
        || seed.settlement_candidate() != &expectation.candidate()
        || seed.owner() != &expectation.owner()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let creation = prepare_rent_owned_settlement_creation_v5(account, funding)?;
    let envelope = OwnerSettlementV5AccountV1 {
        semantic: OwnerSettlementAccumulatorV4::new(expectation)?,
        rent: creation.contract_rent,
        stored_bump: bump,
        flags: 0,
    };
    let exact_body = envelope.encode_exact()?;
    let data_id = envelope.data_id(&CanonicalSha256)?;
    Ok(OwnerSettlementCreatePlanV5 {
        account,
        program_id: funding.program_id,
        seed,
        bump,
        payer: funding.payer,
        payer_debit_lamports: creation.payer_debit_lamports,
        target_lamports_after: creation.target_lamports_after,
        envelope,
        exact_body,
        data_id,
    })
}

/// Exact full-rent creation plan for one General-kind SettlementReceipt V5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptCreatePlanV5 {
    account: Id32,
    program_id: Id32,
    seed: SettlementReceiptSeedTupleV5,
    bump: u8,
    payer: Id32,
    payer_debit_lamports: u64,
    target_lamports_after: u64,
    receipt: SettlementReceiptAccountV5,
    evidence: SettlementReceiptEvidenceV5,
}

impl SettlementReceiptCreatePlanV5 {
    pub const fn account(&self) -> Id32 { self.account }
    pub const fn program_id(&self) -> Id32 { self.program_id }
    pub const fn seed(&self) -> SettlementReceiptSeedTupleV5 { self.seed }
    pub const fn bump(&self) -> u8 { self.bump }
    pub const fn payer(&self) -> Id32 { self.payer }
    pub const fn payer_debit_lamports(&self) -> u64 { self.payer_debit_lamports }
    pub const fn target_lamports_after(&self) -> u64 { self.target_lamports_after }
    pub const fn receipt(&self) -> SettlementReceiptAccountV5 { self.receipt }
    pub const fn evidence(&self) -> SettlementReceiptEvidenceV5 { self.evidence }
    pub const fn exact_body(&self) -> &[u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5] {
        self.evidence.exact_body()
    }
}

/// Wrap one pristine General receipt semantic state in the sole live V5 outer.
pub fn prepare_create_settlement_receipt_v5(
    account: Id32,
    seed: SettlementReceiptSeedTupleV5,
    bump: u8,
    semantic: SettlementReceiptAccountV4,
    funding: RentOwnedSettlementCreateFundingV5,
) -> Result<SettlementReceiptCreatePlanV5, SettlementAdapterErrorV1> {
    prepare_create_settlement_receipt_with_transition_v5(
        account,
        seed,
        bump,
        semantic,
        SettlementReceiptTransitionCommitmentV5::None,
        funding,
    )
}

fn prepare_create_settlement_receipt_with_transition_v5(
    account: Id32,
    seed: SettlementReceiptSeedTupleV5,
    bump: u8,
    semantic: SettlementReceiptAccountV4,
    transition: SettlementReceiptTransitionCommitmentV5,
    funding: RentOwnedSettlementCreateFundingV5,
) -> Result<SettlementReceiptCreatePlanV5, SettlementAdapterErrorV1> {
    semantic.validate()?;
    if semantic.stored_bump != bump
        || seed.epoch() != &semantic.epoch.bytes()
        || seed.settlement_candidate() != &semantic.candidate.bytes()
        || seed.slice_index_le() != &semantic.slice_index.to_le_bytes()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let creation = prepare_rent_owned_settlement_creation_v5(account, funding)?;
    let receipt = SettlementReceiptAccountV5::new(semantic, transition, creation.layout_rent)?;
    let evidence = receipt.evidence(LayoutHash32(account.bytes()))?;
    Ok(SettlementReceiptCreatePlanV5 {
        account,
        program_id: funding.program_id,
        seed,
        bump,
        payer: funding.payer,
        payer_debit_lamports: creation.payer_debit_lamports,
        target_lamports_after: creation.target_lamports_after,
        receipt,
        evidence,
    })
}

/// Structural V5 row/Position/cash-pot/root successor for action 38.
///
/// Fee-bearing composition must additionally authenticate the deleted payer
/// snapshot and durable rent-owned `0x83/4` receipt. Zero-fee composition must derive its
/// domain-separated row+pot evidence. Neither evidence source is caller data in
/// this arithmetic plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerCashRealizationPlanV5 {
    settlement_root_account: Id32,
    settlement_root_poststate: SettlementRootV1AccountV1,
    settlement_cash_pot_account: Id32,
    owner_settlement_account: Id32,
    owner_settlement_seed: OwnerSettlementSeedTupleV5,
    owner_settlement_prestate_data_id: Id32,
    owner_settlement_poststate: OwnerSettlementV5AccountV1,
    owner_settlement_poststate_body: [u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5],
    finalized_owner_row_data_id: Id32,
    pot_poststate_data_id: Id32,
    semantic: OwnerCashRealizationSemanticPlanV4,
    fee_finalization_required: bool,
}

impl OwnerCashRealizationPlanV5 {
    pub const fn settlement_root_account(&self) -> Id32 { self.settlement_root_account }
    pub const fn settlement_root_poststate(&self) -> &SettlementRootV1AccountV1 {
        &self.settlement_root_poststate
    }
    pub const fn settlement_cash_pot_account(&self) -> Id32 {
        self.settlement_cash_pot_account
    }
    pub const fn owner_settlement_account(&self) -> Id32 { self.owner_settlement_account }
    pub const fn owner_settlement_seed(&self) -> OwnerSettlementSeedTupleV5 {
        self.owner_settlement_seed
    }
    pub const fn owner_settlement_prestate_data_id(&self) -> Id32 {
        self.owner_settlement_prestate_data_id
    }
    pub const fn owner_settlement_poststate(&self) -> OwnerSettlementV5AccountV1 {
        self.owner_settlement_poststate
    }
    pub const fn owner_settlement_poststate_body(
        &self,
    ) -> &[u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5] {
        &self.owner_settlement_poststate_body
    }
    pub const fn finalized_owner_row_data_id(&self) -> Id32 {
        self.finalized_owner_row_data_id
    }
    pub const fn pot_poststate_data_id(&self) -> Id32 { self.pot_poststate_data_id }
    pub const fn semantic(&self) -> &OwnerCashRealizationSemanticPlanV4 { &self.semantic }
    pub const fn position(&self) -> PositionSettlementPoststateV3 { self.semantic.position() }
    pub const fn settlement_cash_pot(&self) -> SettlementCashPotV1 {
        self.semantic.settlement_cash_pot()
    }
    pub const fn disposition(&self) -> OwnerSettlementDispositionV4 {
        self.semantic.disposition()
    }
    pub const fn fee_finalization_required(&self) -> bool { self.fee_finalization_required }
}
/// Derive the sole zero-fee action-38 GEN1 evidence from an exact V5 plan.
///
/// The fee-bearing route must instead use the authenticated deleted payer-
/// allocation complete-data ID. Accepting free row/pot hashes here would let a
/// caller splice a different root or owner into the later action-40 chain.
pub fn derive_zero_fee_owner_finalization_evidence_v5(
    plan: &OwnerCashRealizationPlanV5,
) -> Result<Id32, SettlementAdapterErrorV1> {
    if plan.fee_finalization_required {
        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
    }
    let expectation = plan.semantic.expectation();
    derive_zero_fee_owner_finalization_evidence_fields_v5(
        plan.settlement_root_account,
        Id32::new(expectation.candidate())?,
        Id32::new(expectation.owner())?,
        plan.finalized_owner_row_data_id,
        plan.pot_poststate_data_id,
    )
}

fn derive_zero_fee_owner_finalization_evidence_fields_v5(
    settlement_root_account: Id32,
    candidate: Id32,
    owner: Id32,
    finalized_owner_row_data_id: Id32,
    pot_poststate_data_id: Id32,
) -> Result<Id32, SettlementAdapterErrorV1> {
    if settlement_root_account.is_zero()
        || candidate.is_zero()
        || owner.is_zero()
        || finalized_owner_row_data_id.is_zero()
        || pot_poststate_data_id.is_zero()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut hash = Sha256::new();
    hash.update(ZERO_FEE_OWNER_FINALIZATION_EVIDENCE_DOMAIN_V5);
    hash.update(settlement_root_account.bytes());
    hash.update(candidate.bytes());
    hash.update(owner.bytes());
    hash.update(finalized_owner_row_data_id.bytes());
    hash.update(pot_poststate_data_id.bytes());
    Id32::new(hash.finalize().into()).map_err(SettlementAdapterErrorV1::Contract)
}


/// Realize one exact AccountingComplete V5 row against Position/Replay and pot.
#[allow(clippy::too_many_arguments)]
pub fn prepare_realize_owner_cash_v5(
    settlement_root_account: Id32,
    settlement_root: &SettlementRootV1AccountV1,
    traversal: &SettlementTraversalProjectionV4,
    owner_settlement: &OwnerSettlementAccountProjectionV5,
    settlement_cash_pot_account: Id32,
    position_replay: GeneralPositionReplayPrestateV1,
    pot_before: SettlementCashPotV1,
) -> Result<OwnerCashRealizationPlanV5, SettlementAdapterErrorV1> {
    require_root_traversal_binding_v4(settlement_root_account, settlement_root, traversal)?;
    if settlement_root.phase() != SettlementRootPhaseV1::Settling
        || settlement_root.cash_pot_state() != SettlementRootChildStateV1::Live
        || settlement_cash_pot_account != settlement_root.settlement_cash_pot()
        || pot_before.expectation != settlement_root.cash_pot_expectation()?
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let envelope = owner_settlement.envelope;
    let expectation = envelope.semantic.expectation();
    let owner = Id32::new(expectation.owner())?;
    let basis = derive_root_owner_basis_v4(
        settlement_root_account,
        settlement_root,
        traversal,
        owner,
    )?;
    if !expectation_matches_basis_v5(expectation, basis)
        || owner_settlement.seed.epoch() != &settlement_root.epoch().bytes()
        || owner_settlement.seed.settlement_candidate()
            != &settlement_root.settlement_candidate_id().bytes()
        || owner_settlement.seed.owner() != &owner.bytes()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let position = position_replay.position();
    let semantic = prepare_realize_owner_cash_semantic_v4(
        owner_settlement.account.bytes(),
        envelope.semantic,
        position,
        pot_before,
    )?;
    let semantic_body = *semantic.owner_settlement_body();
    let finalized_semantic = OwnerSettlementAccumulatorV4::decode_body(
        clutch_owner_settlement::OWNER_SETTLEMENT_OUTER_TAG_V4,
        clutch_owner_settlement::OWNER_SETTLEMENT_OUTER_VERSION_V4,
        &semantic_body,
    )?;
    let owner_settlement_poststate = OwnerSettlementV5AccountV1 {
        semantic: finalized_semantic,
        rent: envelope.rent,
        stored_bump: envelope.stored_bump,
        flags: envelope.flags,
    };
    let owner_settlement_poststate_body = owner_settlement_poststate.encode_exact()?;
    let finalized_owner_row_data_id = owner_settlement_poststate.data_id(&CanonicalSha256)?;
    let pot_poststate_data_id = clutch_general_v2_contract::settlement_cash_pot_poststate_data_id_v1(
        semantic.settlement_cash_pot(),
        &CanonicalSha256,
    )?;
    let fee_finalization_required = !settlement_root.fee_record().is_zero();
    let settlement_root_poststate =
        settlement_root.complete_owner_finalization(fee_finalization_required)?;
    Ok(OwnerCashRealizationPlanV5 {
        settlement_root_account,
        settlement_root_poststate,
        settlement_cash_pot_account,
        owner_settlement_account: owner_settlement.account,
        owner_settlement_seed: owner_settlement.seed,
        owner_settlement_prestate_data_id: owner_settlement.data_id,
        owner_settlement_poststate,
        owner_settlement_poststate_body,
        finalized_owner_row_data_id,
        pot_poststate_data_id,
        semantic,
        fee_finalization_required,
    })
}

/// Structurally compare one counted SettlementRoot to an exhaustive V4
/// settlement traversal.
///
/// This authenticates no program owner, PDA, account body, meta privilege, or
/// traversal provenance. A live adapter must establish all of those facts
/// before promoting a successful equality bind into an execution seam.
pub fn bind_settlement_root_traversal_v4(
    settlement_root_account: Id32,
    settlement_root: &SettlementRootV1AccountV1,
    traversal: &SettlementTraversalProjectionV4,
) -> Result<(), SettlementAdapterErrorV1> {
    require_root_traversal_binding_v4(settlement_root_account, settlement_root, traversal)
}

fn require_root_traversal_binding_v4(
    settlement_root_account: Id32,
    settlement_root: &SettlementRootV1AccountV1,
    traversal: &SettlementTraversalProjectionV4,
) -> Result<(), SettlementAdapterErrorV1> {
    settlement_root.validate()?;
    let feed = traversal.feed;
    let counts = settlement_root.counts();
    if settlement_root_account.is_zero()
        || settlement_root.retained_feed() != traversal.selected_feed_account
        || settlement_root.candidate_bundle_digest() != traversal.candidate_bundle_digest
        || settlement_root.epoch() != feed.epoch
        || settlement_root.market() != feed.market
        || settlement_root.source_admission_node() != feed.node
        || settlement_root.order_set() != feed.order_set
        || settlement_root.settlement_candidate_id() != feed.settlement_candidate_id
        || settlement_root.settlement_witness_digest() != feed.settlement_witness_digest
        || settlement_root.epoch_generation() != feed.epoch_generation
        || settlement_root.outcome_count() != feed.outcome_count
        || settlement_root.order_count() != feed.order_count
        || settlement_root.owner_order_set_digest() != traversal.owner_order_set_digest
        || settlement_root.market_instance_v2_id().bytes()
            != traversal.position_market_binding.market_instance_id.bytes()
        || settlement_root.virtual_cash_direction() != traversal.virtual_cash_direction
        || settlement_root.virtual_cash_atoms() != traversal.virtual_cash_atoms
        || counts.expected_receipts != feed.slice_count
        || counts.expected_owner_rows != traversal.owner_basis.owner_count()
        || counts.expected_reservations != traversal.expected_reservation_count
        || counts.expected_filled_reservations != traversal.expected_filled_reservation_count
        || counts.expected_merge_payments != traversal.expected_merge_payment_count
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(())
}

const fn frozen_identity(value: Id32) -> [u8; 32] {
    value.bytes()
}

fn owner_appeared_before_slice_v4(
    projection: &OwnerBlindBookProjectionV1,
    slices: &[CanonicalSettlementSliceV1; MAX_SLICES],
    before: u16,
    owner: Id32,
) -> Result<bool, SettlementAdapterErrorV1> {
    let mut slice = 0u16;
    while slice < before {
        for leg in [
            slices[usize::from(slice)].buy,
            slices[usize::from(slice)].sell,
        ] {
            if let SettlementLegV1::Order(order_index) = leg {
                let membership = projection
                    .order_membership(order_index)
                    .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
                if membership.owner() == owner {
                    return Ok(true);
                }
            }
        }
        slice = slice
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    Ok(false)
}

/// Candidate-fee evidence used to seal one pristine rent-owned V5 owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerRowFeeEvidenceV5 {
    /// Exact authenticated candidate fee projection, including a zero-fee row.
    CandidateFee(AuthenticatedSelectedOwnerFeeV4),
    /// Candidate-wide fee record is absent and the row fee is exactly zero.
    NoFeeRecord,
}

/// One exact rent-owned owner row presented to action 24.
#[derive(Clone, Copy, Debug)]
pub enum OwnerRowMaterializationInputV5<'a> {
    /// Create the pristine row at its fresh V5 PDA.
    Create {
        /// Canonical V5 owner-row PDA authenticated by the outer adapter.
        account: Id32,
        /// Canonical fresh V5 bump.
        bump: u8,
        /// Exact full-rent creation facts.
        funding: RentOwnedSettlementCreateFundingV5,
        /// Exact disjoint fee/no-fee evidence.
        fee_evidence: OwnerRowFeeEvidenceV5,
    },
    /// Require the exact pristine V5 row created on an earlier owner end.
    Existing {
        /// Hostile exact V5 account view.
        view: OwnerSettlementAccountViewV5<'a>,
    },
}

/// One real endpoint supplied to the rent-owned action-24 materializer.
#[derive(Clone, Copy, Debug)]
pub struct EntitlementEndpointInputV5<'a> {
    pub order_index: u8,
    pub reservation: MaterializationReservationInputV9<'a>,
    pub position: PositionAccountInputV3<'a>,
    pub owner_row: OwnerRowMaterializationInputV5<'a>,
}

/// Complete account-local input for one V5 action-24 slice.
#[derive(Clone, Copy, Debug)]
pub struct MaterializeEntitlementSliceInputV5<'a> {
    pub entitlement: &'a CandidateEntitlementProjectionV4,
    pub receipt_account: Id32,
    pub receipt_bump: u8,
    pub receipt_funding: RentOwnedSettlementCreateFundingV5,
    pub endpoints: [Option<EntitlementEndpointInputV5<'a>>; 2],
}

/// Exact V5 owner-row treatment derived for one current real end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerRowMaterializationDispositionV5 {
    /// First occurrence creates one exact pristine rent-owned row.
    Create {
        fee_evidence: OwnerRowFeeEvidenceV5,
        plan: OwnerSettlementCreatePlanV5,
    },
    /// Earlier occurrence already created the exact pristine row.
    Existing {
        projection: OwnerSettlementAccountProjectionV5,
    },
}

impl OwnerRowMaterializationDispositionV5 {
    pub const fn account(&self) -> Id32 {
        match self {
            Self::Create { plan, .. } => plan.account(),
            Self::Existing { projection } => projection.account(),
        }
    }

    pub const fn payer_debit_lamports(&self) -> u64 {
        match self {
            Self::Create { plan, .. } => plan.payer_debit_lamports(),
            Self::Existing { .. } => 0,
        }
    }

    pub const fn seed(&self) -> OwnerSettlementSeedTupleV5 {
        match self {
            Self::Create { plan, .. } => plan.seed(),
            Self::Existing { projection } => projection.seed(),
        }
    }

    pub const fn exact_body(&self) -> &[u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5] {
        match self {
            Self::Create { plan, .. } => plan.exact_body(),
            Self::Existing { projection } => projection.exact_body(),
        }
    }
}

/// V9 Reservation comparison/stamp used by the sole live action-24 route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservationMaterializationPlanV5 {
    account: Id32,
    semantic_id: Id32,
    seed: GeneralReservationSeedTupleV9,
    order_index: u8,
    first_occurrence: bool,
    prestate_body: [u8; RESERVATION_ACCOUNT_BYTES_V9],
    poststate_body: [u8; RESERVATION_ACCOUNT_BYTES_V9],
}

impl ReservationMaterializationPlanV5 {
    pub const fn account(&self) -> Id32 { self.account }
    pub const fn semantic_id(&self) -> Id32 { self.semantic_id }
    pub const fn seed(&self) -> GeneralReservationSeedTupleV9 { self.seed }
    pub const fn order_index(&self) -> u8 { self.order_index }
    pub const fn first_occurrence(&self) -> bool { self.first_occurrence }
    pub const fn prestate_body(&self) -> &[u8; RESERVATION_ACCOUNT_BYTES_V9] {
        &self.prestate_body
    }
    pub const fn poststate_body(&self) -> &[u8; RESERVATION_ACCOUNT_BYTES_V9] {
        &self.poststate_body
    }
}

/// Read-only Position V3 fact used by the sole live action-24 route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionMaterializationFactV5 {
    account: Id32,
    semantic_id: Id32,
    position: PositionAccountV3,
}

impl PositionMaterializationFactV5 {
    pub const fn account(&self) -> Id32 { self.account }
    pub const fn semantic_id(&self) -> Id32 { self.semantic_id }
    pub const fn position(&self) -> PositionAccountV3 { self.position }
}

/// One fully derived real endpoint in the V5 action-24 atomic bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntitlementEndpointPlanV5 {
    membership: AuthenticatedOrderMembershipV2,
    reservation: ReservationMaterializationPlanV5,
    position: PositionMaterializationFactV5,
    owner_row: OwnerRowMaterializationDispositionV5,
}

impl EntitlementEndpointPlanV5 {
    pub const fn membership(&self) -> AuthenticatedOrderMembershipV2 { self.membership }
    pub const fn reservation(&self) -> &ReservationMaterializationPlanV5 { &self.reservation }
    pub const fn position(&self) -> PositionMaterializationFactV5 { self.position }
    pub const fn owner_row(&self) -> &OwnerRowMaterializationDispositionV5 { &self.owner_row }
}

/// Candidate-scoped child-count delta emitted by one V5 materialization slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateDependencyCreateDeltaV5 {
    receipts_created: u8,
    owner_rows_created: u8,
    filled_reservations_admitted: u8,
    merge_payment_admitted: bool,
}

impl CandidateDependencyCreateDeltaV5 {
    pub const fn receipts_created(&self) -> u8 { self.receipts_created }
    pub const fn owner_rows_created(&self) -> u8 { self.owner_rows_created }
    pub const fn filled_reservations_admitted(&self) -> u8 {
        self.filled_reservations_admitted
    }
    pub const fn merge_payment_admitted(&self) -> bool { self.merge_payment_admitted }
}

/// One atomic, resumable, rent-owned action-24 poststate bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializeEntitlementSlicePlanV5 {
    settlement_root_account: Id32,
    settlement_root_poststate: SettlementRootV1AccountV1,
    receipt: SettlementReceiptCreatePlanV5,
    endpoints: [Option<EntitlementEndpointPlanV5>; 2],
    endpoint_count: u8,
    dependency_delta: CandidateDependencyCreateDeltaV5,
}

impl MaterializeEntitlementSlicePlanV5 {
    pub const fn settlement_root_account(&self) -> Id32 { self.settlement_root_account }
    pub const fn settlement_root_poststate(&self) -> &SettlementRootV1AccountV1 {
        &self.settlement_root_poststate
    }
    pub const fn receipt(&self) -> &SettlementReceiptCreatePlanV5 { &self.receipt }
    pub const fn endpoint_count(&self) -> u8 { self.endpoint_count }
    pub fn endpoint(&self, ordinal: u8) -> Option<&EntitlementEndpointPlanV5> {
        if ordinal < self.endpoint_count {
            self.endpoints[usize::from(ordinal)].as_ref()
        } else {
            None
        }
    }
    pub const fn dependency_delta(&self) -> CandidateDependencyCreateDeltaV5 {
        self.dependency_delta
    }
}

/// One capability-selected portfolio sibling receipt allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairReceiptCreateInputV5 {
    /// Canonical V5 receipt PDA authenticated by the outer adapter.
    pub account: Id32,
    /// Canonical fresh V5 bump.
    pub bump: u8,
    /// Exact disjoint full-rent creation facts.
    pub funding: RentOwnedSettlementCreateFundingV5,
}

/// Complete input for the all-or-nothing portfolio action-24 successor.
///
/// `sibling_set` is the private exhaustive capability derived from the exact
/// RelationV2 pair and retained Feed. Its active prefix, never a packet count,
/// owns how many receipt creation rows are consumed.
#[derive(Clone, Copy, Debug)]
pub struct MaterializePortfolioPairInputV5<'a> {
    pub entitlement: &'a CandidateEntitlementProjectionV4,
    pub sibling_set: AuthenticatedPortfolioReceiptSiblingSetV2,
    pub receipts: [Option<PortfolioPairReceiptCreateInputV5>;
        PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
    pub endpoints: [EntitlementEndpointInputV5<'a>; 2],
}

/// Candidate-scoped count delta for one exhaustive portfolio pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioPairDependencyCreateDeltaV5 {
    receipts_created: u8,
    owner_rows_created: u8,
    filled_reservations_admitted: u8,
}

impl PortfolioPairDependencyCreateDeltaV5 {
    pub const fn receipts_created(&self) -> u8 { self.receipts_created }
    pub const fn owner_rows_created(&self) -> u8 { self.owner_rows_created }
    pub const fn filled_reservations_admitted(&self) -> u8 {
        self.filled_reservations_admitted
    }
}

/// Atomic poststate for every scalar sibling of one exclusive portfolio pair.
///
/// The fixed array is intentionally large. An SBF composer must place this
/// plan in caller-provided heap/out-parameter storage rather than a program
/// stack frame, and must create every active receipt and write the root and
/// two endpoint successors in one rollback domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializePortfolioPairPlanV5 {
    settlement_root_account: Id32,
    settlement_root_poststate: SettlementRootV1AccountV1,
    receipts: [Option<SettlementReceiptCreatePlanV5>; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
    receipt_count: u8,
    endpoints: [EntitlementEndpointPlanV5; 2],
    dependency_delta: PortfolioPairDependencyCreateDeltaV5,
}

impl MaterializePortfolioPairPlanV5 {
    pub const fn settlement_root_account(&self) -> Id32 { self.settlement_root_account }
    pub const fn settlement_root_poststate(&self) -> &SettlementRootV1AccountV1 {
        &self.settlement_root_poststate
    }
    pub const fn receipt_count(&self) -> u8 { self.receipt_count }
    pub fn receipt(&self, index: u8) -> Option<&SettlementReceiptCreatePlanV5> {
        if index < self.receipt_count {
            self.receipts[usize::from(index)].as_ref()
        } else {
            None
        }
    }
    pub fn endpoint(&self, ordinal: u8) -> Option<&EntitlementEndpointPlanV5> {
        self.endpoints.get(usize::from(ordinal))
    }
    pub const fn dependency_delta(&self) -> PortfolioPairDependencyCreateDeltaV5 {
        self.dependency_delta
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedEntitlementEndpointV5 {
    plan: EntitlementEndpointPlanV5,
    rent: Option<RentDebitFactV5>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RentDebitFactV5 {
    payer: Id32,
    balance_before: u64,
    debit: u64,
}

/// Materialize exactly one selected slice through only V9 and rent-owned V5.
pub fn prepare_materialize_entitlement_slice_v5(
    input: MaterializeEntitlementSliceInputV5<'_>,
) -> Result<MaterializeEntitlementSlicePlanV5, SettlementAdapterErrorV1> {
    let entitlement = input.entitlement;
    let selected = settlement_coordinates_v4(&entitlement.settlement_root)?;
    let cursor = entitlement.settlement_root.counts().admitted_receipts;
    let slice = entitlement.current_slice;
    let mut expected_orders = [0u8; 2];
    let mut expected_first_owner = [false; 2];
    let endpoint_count = match (slice.buy, slice.sell, slice.route) {
        (SettlementLegV1::Order(buy), SettlementLegV1::Order(sell), SettlementRouteV1::Direct) => {
            expected_orders = [buy, sell];
            expected_first_owner = [
                entitlement.current_buy_first_owner,
                entitlement.current_sell_first_owner,
            ];
            2u8
        }
        (SettlementLegV1::Order(buy), SettlementLegV1::Split, SettlementRouteV1::SplitToBuy) => {
            expected_orders[0] = buy;
            expected_first_owner[0] = entitlement.current_buy_first_owner;
            1u8
        }
        (SettlementLegV1::Merge, SettlementLegV1::Order(sell), SettlementRouteV1::SellToMerge) => {
            expected_orders[0] = sell;
            expected_first_owner[0] = entitlement.current_sell_first_owner;
            1u8
        }
        _ => return Err(SettlementAdapterErrorV1::BindingMismatch),
    };
    let mut ordinal = 0usize;
    while ordinal < input.endpoints.len() {
        if input.endpoints[ordinal].is_some() != (ordinal < usize::from(endpoint_count)) {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        ordinal += 1;
    }
    ordinal = 0;
    while ordinal < usize::from(endpoint_count) {
        let membership = entitlement
            .settlement_membership(expected_orders[ordinal])
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        if membership.order_kind != OrderKindV1::Single {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        ordinal += 1;
    }

    let receipt_seed = SettlementReceiptSeedTupleV5::new(
        selected.epoch,
        selected.settlement_candidate_id,
        cursor,
    )?;
    let receipt_semantic = SettlementReceiptAccountV4 {
        epoch: LayoutHash32(selected.epoch.bytes()),
        market: LayoutHash32(selected.market.bytes()),
        candidate: LayoutHash32(selected.settlement_candidate_id.bytes()),
        buy_order_id: match slice.buy {
            SettlementLegV1::Order(order_index) => LayoutHash32(
                entitlement
                    .settlement_membership(order_index)
                    .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                    .order_id,
            ),
            SettlementLegV1::Merge => LayoutHash32::ZERO,
            SettlementLegV1::Split => return Err(SettlementAdapterErrorV1::BindingMismatch),
        },
        sell_order_id: match slice.sell {
            SettlementLegV1::Order(order_index) => LayoutHash32(
                entitlement
                    .settlement_membership(order_index)
                    .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
                    .order_id,
            ),
            SettlementLegV1::Split => LayoutHash32::ZERO,
            SettlementLegV1::Merge => return Err(SettlementAdapterErrorV1::BindingMismatch),
        },
        consideration_price_units: u128::from(slice.quantity)
            .checked_mul(u128::from(entitlement.current_price))
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?,
        quantity: slice.quantity,
        settled_quantity: 0,
        price: entitlement.current_price,
        sequence: u64::from(cursor) + 1,
        slice_index: cursor,
        outcome: slice.outcome,
        leg_kind: match slice.route {
            SettlementRouteV1::Direct => RECEIPT_LEG_DIRECT,
            SettlementRouteV1::SplitToBuy => RECEIPT_LEG_SPLIT,
            SettlementRouteV1::SellToMerge => RECEIPT_LEG_MERGE,
        },
        consumed_flags: 0,
        stored_bump: input.receipt_bump,
        accounted_end_mask: 0,
    };
    let receipt = prepare_create_settlement_receipt_v5(
        input.receipt_account,
        receipt_seed,
        input.receipt_bump,
        receipt_semantic,
        input.receipt_funding,
    )?;
    let mut endpoints = [None; 2];
    let mut rent_debits = [None; 3];
    rent_debits[0] = Some(RentDebitFactV5 {
        payer: input.receipt_funding.payer,
        balance_before: input.receipt_funding.payer_lamports,
        debit: receipt.payer_debit_lamports(),
    });
    let mut owner_rows_created = 0u8;
    let mut filled_reservations_admitted = 0u8;
    ordinal = 0;
    while ordinal < usize::from(endpoint_count) {
        let endpoint_input = input.endpoints[ordinal]
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        if endpoint_input.order_index != expected_orders[ordinal] {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        let prepared = prepare_entitlement_endpoint_v5(
            entitlement,
            endpoint_input,
            expected_first_owner[ordinal],
            receipt.program_id(),
        )?;
        if let Some(rent) = prepared.rent {
            owner_rows_created = owner_rows_created
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            rent_debits[ordinal + 1] = Some(rent);
        }
        if prepared.plan.reservation.first_occurrence() {
            filled_reservations_admitted = filled_reservations_admitted
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        }
        endpoints[ordinal] = Some(prepared.plan);
        ordinal += 1;
    }
    validate_materialization_bundle_v5(
        entitlement,
        &receipt,
        &endpoints,
        endpoint_count,
        &rent_debits,
    )?;
    let merge_payment_admitted = slice.route == SettlementRouteV1::SellToMerge;
    let settlement_root_poststate = entitlement.settlement_root.admit_materialization_delta(
        owner_rows_created,
        filled_reservations_admitted,
        merge_payment_admitted,
    )?;
    Ok(MaterializeEntitlementSlicePlanV5 {
        settlement_root_account: entitlement.settlement_root_account,
        settlement_root_poststate,
        receipt,
        endpoints,
        endpoint_count,
        dependency_delta: CandidateDependencyCreateDeltaV5 {
            receipts_created: 1,
            owner_rows_created,
            filled_reservations_admitted,
            merge_payment_admitted,
        },
    })
}

/// Materialize every scalar Receipt V5 sibling of one exclusive portfolio pair.
///
/// This is the only action-24 pure route that accepts portfolio orders. The
/// root must still be at its initial receipt cursor, the private sibling
/// capability must cover the complete retained-Feed slice set, and both
/// endpoint rows and Reservations are created/adopted once. No anchor-only or
/// page-selected subset can produce `PortfolioPairPending` receipts.
pub fn prepare_materialize_portfolio_pair_v5(
    input: MaterializePortfolioPairInputV5<'_>,
) -> Result<MaterializePortfolioPairPlanV5, SettlementAdapterErrorV1> {
    let entitlement = input.entitlement;
    let root = entitlement.settlement_root;
    let counts = root.counts();
    let sibling_count = input.sibling_set.sibling_count();
    let sibling_len = usize::from(sibling_count);
    let pair = input.sibling_set.pair();
    let buyer_record = *pair.buyer().record();
    let seller_record = *pair.seller().record();
    let slice = entitlement.current_slice;
    let (buy_order_index, sell_order_index) = match (slice.buy, slice.sell, slice.route) {
        (
            SettlementLegV1::Order(buy),
            SettlementLegV1::Order(sell),
            SettlementRouteV1::Direct,
        ) => (buy, sell),
        _ => return Err(SettlementAdapterErrorV1::BindingMismatch),
    };
    if sibling_count == 0
        || sibling_len > PORTFOLIO_PAIR_MAX_RECEIPTS_V2
        || counts.admitted_receipts != 0
        || counts.live_receipts != 0
        || counts.expected_receipts != u16::from(sibling_count)
        || counts.expected_owner_rows != 2
        || counts.admitted_owner_rows != 0
        || counts.live_owner_rows != 0
        || counts.expected_filled_reservations != 2
        || counts.admitted_reservations != 0
        || counts.live_reservations != 0
        || counts.expected_merge_payments != 0
        || root.virtual_cash_direction() != VirtualCashDirectionV1::None
        || !entitlement.current_buy_first_owner
        || !entitlement.current_sell_first_owner
        || buy_order_index != buyer_record.order_index
        || sell_order_index != seller_record.order_index
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let root_data_id = root.data_id(&CanonicalSha256, entitlement.settlement_root_account)?;
    bind_portfolio_pair_record_v5(
        entitlement,
        buyer_record,
        buy_order_index,
        SettlementSideV1::Buy,
        root_data_id,
    )?;
    bind_portfolio_pair_record_v5(
        entitlement,
        seller_record,
        sell_order_index,
        SettlementSideV1::Sell,
        root_data_id,
    )?;
    let first_receipt_input = input.receipts[0]
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let mut prepared_endpoints = [None; 2];
    let mut rent_debits = [None; PORTFOLIO_PAIR_MAX_RECEIPTS_V2 + 2];
    let mut endpoint_ordinal = 0usize;
    while endpoint_ordinal < input.endpoints.len() {
        let expected_order_index = if endpoint_ordinal == 0 {
            buy_order_index
        } else {
            sell_order_index
        };
        let endpoint_input = input.endpoints[endpoint_ordinal];
        if endpoint_input.order_index != expected_order_index {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        let prepared = prepare_entitlement_endpoint_v5(
            entitlement,
            endpoint_input,
            true,
            first_receipt_input.funding.program_id,
        )?;
        if !prepared.plan.reservation.first_occurrence()
            || prepared.plan.membership.order_kind != OrderKindV1::Portfolio
            || !matches!(
                prepared.plan.owner_row,
                OwnerRowMaterializationDispositionV5::Create { .. }
            )
        {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        let record = if endpoint_ordinal == 0 {
            buyer_record
        } else {
            seller_record
        };
        if prepared.plan.position.account().bytes() != record.position_account_id
            || prepared.plan.position.semantic_id().bytes() != record.position_pre_semantic_id
        {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        if let Some(rent) = prepared.rent {
            rent_debits[PORTFOLIO_PAIR_MAX_RECEIPTS_V2 + endpoint_ordinal] = Some(rent);
        } else {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        prepared_endpoints[endpoint_ordinal] = Some(prepared.plan);
        endpoint_ordinal += 1;
    }
    let endpoints = [
        prepared_endpoints[0].ok_or(SettlementAdapterErrorV1::BindingMismatch)?,
        prepared_endpoints[1].ok_or(SettlementAdapterErrorV1::BindingMismatch)?,
    ];
    let buy_membership = endpoints[0].membership;
    let sell_membership = endpoints[1].membership;
    let mut receipts = [None; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
    let mut receipt_index = 0usize;
    while receipt_index < input.receipts.len() {
        if input.receipts[receipt_index].is_some() != (receipt_index < sibling_len) {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        if receipt_index < sibling_len {
            let sibling = input
                .sibling_set
                .sibling(
                    u8::try_from(receipt_index)
                        .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?,
                )
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
            let canonical_index = u16::try_from(receipt_index)
                .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
            let canonical_slice = entitlement.traversal.slices[receipt_index];
            let receipt_input = input.receipts[receipt_index]
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
            if sibling.outcome >= root.outcome_count()
                || sibling.slice_index != canonical_index
                || sibling.sequence
                    != u64::from(canonical_index)
                        .checked_add(1)
                        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?
                || sibling.buy_order_index != buy_order_index
                || sibling.sell_order_index != sell_order_index
                || canonical_slice.buy != SettlementLegV1::Order(buy_order_index)
                || canonical_slice.sell != SettlementLegV1::Order(sell_order_index)
                || canonical_slice.route != SettlementRouteV1::Direct
                || canonical_slice.outcome != sibling.outcome
                || canonical_slice.quantity != sibling.quantity
                || entitlement.traversal.prices[usize::from(sibling.outcome)] != sibling.price
                || receipt_input.funding.program_id != first_receipt_input.funding.program_id
            {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            let semantic = SettlementReceiptAccountV4 {
                epoch: LayoutHash32(root.epoch().bytes()),
                market: LayoutHash32(root.market().bytes()),
                candidate: LayoutHash32(root.settlement_candidate_id().bytes()),
                buy_order_id: LayoutHash32(buy_membership.order_id),
                sell_order_id: LayoutHash32(sell_membership.order_id),
                consideration_price_units: u128::from(sibling.quantity)
                    .checked_mul(u128::from(sibling.price))
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?,
                quantity: sibling.quantity,
                settled_quantity: 0,
                price: sibling.price,
                sequence: sibling.sequence,
                slice_index: sibling.slice_index,
                outcome: sibling.outcome,
                leg_kind: RECEIPT_LEG_DIRECT,
                consumed_flags: 0,
                stored_bump: receipt_input.bump,
                accounted_end_mask: 0,
            };
            let seed = SettlementReceiptSeedTupleV5::new(
                root.epoch(),
                root.settlement_candidate_id(),
                sibling.slice_index,
            )?;
            let plan = prepare_create_settlement_receipt_with_transition_v5(
                receipt_input.account,
                seed,
                receipt_input.bump,
                semantic,
                SettlementReceiptTransitionCommitmentV5::PortfolioPairPending,
                receipt_input.funding,
            )?;
            rent_debits[receipt_index] = Some(RentDebitFactV5 {
                payer: receipt_input.funding.payer,
                balance_before: receipt_input.funding.payer_lamports,
                debit: plan.payer_debit_lamports(),
            });
            receipts[receipt_index] = Some(plan);
        }
        receipt_index += 1;
    }
    validate_portfolio_pair_materialization_bundle_v5(
        entitlement,
        &receipts,
        sibling_count,
        &endpoints,
        &rent_debits,
    )?;
    let mut settlement_root_poststate = root;
    receipt_index = 0;
    while receipt_index < sibling_len {
        let first = receipt_index == 0;
        settlement_root_poststate = settlement_root_poststate.admit_materialization_delta(
            if first { 2 } else { 0 },
            if first { 2 } else { 0 },
            false,
        )?;
        receipt_index += 1;
    }
    Ok(MaterializePortfolioPairPlanV5 {
        settlement_root_account: entitlement.settlement_root_account,
        settlement_root_poststate,
        receipts,
        receipt_count: sibling_count,
        endpoints,
        dependency_delta: PortfolioPairDependencyCreateDeltaV5 {
            receipts_created: sibling_count,
            owner_rows_created: 2,
            filled_reservations_admitted: 2,
        },
    })
}

fn bind_portfolio_pair_record_v5(
    entitlement: &CandidateEntitlementProjectionV4,
    record: clutch_batch::SelectedPortfolioOrderRecordV2,
    order_index: u8,
    side: SettlementSideV1,
    root_data_id: Id32,
) -> Result<(), SettlementAdapterErrorV1> {
    let root = entitlement.settlement_root;
    let membership = entitlement
        .settlement_membership(order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let expected_page_account = entitlement
        .order_projection()
        .order_page_account(order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let expected_page_index = entitlement
        .order_projection()
        .order_page_index(order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    if record.order_index != order_index
        || record.side
            != match side {
                SettlementSideV1::Buy => Side::Buy,
                SettlementSideV1::Sell => Side::Sell,
            }
        || record.outcome_count != root.outcome_count()
        || record.traversal_index != 0
        || record.page_index != expected_page_index
        || record.settlement_root_epoch_generation != root.epoch_generation()
        || record.position_generation != membership.position_generation
        || record.economic_candidate_digest != entitlement.feed().base_relation_candidate_id.bytes()
        || record.order_set_digest != root.order_set().bytes()
        || record.settlement_root_account_id != entitlement.settlement_root_account.bytes()
        || record.settlement_root_pre_semantic_id != root_data_id.bytes()
        || record.settlement_candidate_id != root.settlement_candidate_id().bytes()
        || record.retained_feed_account_id != entitlement.selected_feed_account().bytes()
        || record.retained_feed_semantic_id
            != entitlement.traversal.candidate_bundle_digest().bytes()
        || record.settlement_witness_id != root.settlement_witness_digest().bytes()
        || record.order_page_account_id != expected_page_account.bytes()
        || record.order_id != membership.order_id
        || record.owner_id != membership.owner
        || membership.side != side
        || membership.order_kind != OrderKindV1::Portfolio
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(())
}

fn validate_portfolio_pair_materialization_bundle_v5(
    entitlement: &CandidateEntitlementProjectionV4,
    receipts: &[Option<SettlementReceiptCreatePlanV5>; PORTFOLIO_PAIR_MAX_RECEIPTS_V2],
    receipt_count: u8,
    endpoints: &[EntitlementEndpointPlanV5; 2],
    rent_debits: &[Option<RentDebitFactV5>; PORTFOLIO_PAIR_MAX_RECEIPTS_V2 + 2],
) -> Result<(), SettlementAdapterErrorV1> {
    let selected = settlement_coordinates_v4(&entitlement.settlement_root)?;
    let mut accounts = [Id32::ZERO; 40];
    let mut account_count = 0usize;
    for account in [
        entitlement.settlement_root_account,
        entitlement.selected_feed_account(),
        selected.market,
        selected.epoch,
        selected.market_binding,
    ] {
        insert_materialization_account_v5(&mut accounts, &mut account_count, account)?;
    }
    let mut page = 0u16;
    while usize::from(page) < usize::from(entitlement.order_projection().page_count()) {
        insert_materialization_account_v5(
            &mut accounts,
            &mut account_count,
            entitlement
                .order_projection()
                .page_account(page)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?,
        )?;
        page = page
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    let mut receipt_index = 0usize;
    while receipt_index < usize::from(receipt_count) {
        let receipt = receipts[receipt_index]
            .as_ref()
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        insert_materialization_account_v5(
            &mut accounts,
            &mut account_count,
            receipt.account(),
        )?;
        receipt_index += 1;
    }
    let expected_program_id = receipts[0]
        .as_ref()
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
        .program_id();
    for endpoint in endpoints {
        for account in [
            endpoint.reservation.account(),
            endpoint.position.account(),
            endpoint.owner_row.account(),
        ] {
            insert_materialization_account_v5(&mut accounts, &mut account_count, account)?;
        }
        let owner_row = match endpoint.owner_row {
            OwnerRowMaterializationDispositionV5::Create { plan, .. } => plan,
            OwnerRowMaterializationDispositionV5::Existing { .. } => {
                return Err(SettlementAdapterErrorV1::BindingMismatch)
            }
        };
        if owner_row.program_id() != expected_program_id {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
    }
    receipt_index = 0;
    while receipt_index < usize::from(receipt_count) {
        let receipt = receipts[receipt_index]
            .as_ref()
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        require_materialization_party_not_target_v5(
            receipt.program_id(),
            &accounts[..account_count],
        )?;
        require_materialization_party_not_target_v5(
            receipt.payer(),
            &accounts[..account_count],
        )?;
        receipt_index += 1;
    }
    for endpoint in endpoints {
        let owner_row = match endpoint.owner_row {
            OwnerRowMaterializationDispositionV5::Create { plan, .. } => plan,
            OwnerRowMaterializationDispositionV5::Existing { .. } => {
                return Err(SettlementAdapterErrorV1::BindingMismatch)
            }
        };
        require_materialization_party_not_target_v5(
            owner_row.payer(),
            &accounts[..account_count],
        )?;
    }
    validate_aggregate_rent_debits_v5(rent_debits)
}

fn prepare_entitlement_endpoint_v5(
    entitlement: &CandidateEntitlementProjectionV4,
    input: EntitlementEndpointInputV5<'_>,
    first_owner: bool,
    expected_program_id: Id32,
) -> Result<PreparedEntitlementEndpointV5, SettlementAdapterErrorV1> {
    let selected = settlement_coordinates_v4(&entitlement.settlement_root)?;
    let base = entitlement.order_projection().base();
    let feed = entitlement.feed();
    let cursor = entitlement.settlement_root.counts().admitted_receipts;
    let membership = entitlement
        .settlement_membership(input.order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let frozen = base
        .order_membership(input.order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let first_order = entitlement.first_slice(input.order_index) == Some(cursor);
    let expected_page_index = entitlement
        .order_projection
        .order_page_index(input.order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    if membership.owner != frozen.owner().bytes()
        || membership.order_id != frozen.order_id().bytes()
        || membership.order_generation != frozen.generation()
        || membership.position_generation
            != entitlement
                .order_projection
                .position_generation(input.order_index)
                .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let reservation_v9 = ReservationAccountV9::decode(input.reservation.encoded_body)?;
    let reservation = reservation_v9.body();
    let reservation_plan = ReservationPlan::for_order(
        frozen.slot(),
        feed.outcome_count,
        feed.price_scale,
        reservation.max_fee_atoms,
    )?;
    let expected_side = match membership.side {
        SettlementSideV1::Buy => 0,
        SettlementSideV1::Sell => 1,
    };
    let expected_kind = match membership.order_kind {
        OrderKindV1::Single => 1,
        OrderKindV1::Portfolio => 2,
    };
    if input.reservation.account.is_zero()
        || reservation.reservation.bytes() != membership.reservation
        || reservation.market.bytes() != selected.market.bytes()
        || reservation.epoch.bytes() != selected.epoch.bytes()
        || reservation.owner.bytes() != membership.owner
        || reservation.order_id.bytes() != membership.order_id
        || reservation.position_generation != membership.position_generation
        || reservation.order_generation != membership.order_generation
        || reservation.page_index != expected_page_index
        || reservation.price_grid.bytes() != base.price_grid_id().bytes()
        || reservation.terms.bytes() != entitlement.terms().bytes()
        || reservation.policy.bytes() != entitlement.reservation_policy().bytes()
        || reservation.outcome_count != feed.outcome_count
        || reservation.side != expected_side
        || reservation.order_kind != expected_kind
        || reservation.initial_cash_atoms != reservation_plan.cash_atoms
        || reservation.max_fee_atoms != reservation_plan.max_fee_atoms
        || reservation.initial_internal != reservation_plan.internal
        || reservation.remaining_cash_atoms != reservation.initial_cash_atoms
        || reservation.remaining_internal != reservation.initial_internal
        || reservation.release_generation != 0
        || reservation.consumed_units != 0
        || reservation.paid_units != 0
        || reservation.fee_debited_atoms != 0
        || reservation.fee_carry_numerator != 0
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    if first_order {
        if reservation.state != RESERVATION_STATE_ACTIVE || reservation.entitled_units != 0 {
            return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
        }
    } else if reservation.state != RESERVATION_STATE_ENTITLED
        || reservation.entitled_units != membership.entitled_units
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    let mut reservation_poststate = reservation;
    if first_order {
        reservation_poststate.state = RESERVATION_STATE_ENTITLED;
        reservation_poststate.entitled_units = membership.entitled_units;
    }
    let reservation_poststate_v9 =
        ReservationAccountV9::new(reservation_poststate, reservation_v9.rent())?;
    let mut reservation_prestate_body = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
    let mut reservation_poststate_body = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
    reservation_v9.encode(&mut reservation_prestate_body)?;
    reservation_poststate_v9.encode(&mut reservation_poststate_body)?;
    let reservation_semantic_id = Id32::new(reservation.reservation.bytes())?;
    let reservation_output = ReservationMaterializationPlanV5 {
        account: input.reservation.account,
        semantic_id: reservation_semantic_id,
        seed: GeneralReservationSeedTupleV9::new(reservation_semantic_id)?,
        order_index: input.order_index,
        first_occurrence: first_order,
        prestate_body: reservation_prestate_body,
        poststate_body: reservation_poststate_body,
    };
    let position = PositionAccountV3::decode(input.position.encoded_body)?;
    if input.position.account.is_zero()
        || position.outstanding_reservations() == 0
        || position.replay_account().bytes() == input.position.account.bytes()
        || position.owner().bytes() == input.position.account.bytes()
        || input.position.account == selected.market
        || input.position.account == selected.epoch
    {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }
    project_canonical_general_settlement_position_v3(
        position,
        Id32::new(membership.owner)?,
        membership.position_generation,
        selected.market,
        entitlement.position_market_binding(),
    )?;
    let position_output = PositionMaterializationFactV5 {
        account: input.position.account,
        semantic_id: Id32::new(position.semantic_id(&PositionBodySha256V3)?.bytes())?,
        position,
    };
    let basis = entitlement
        .owner_basis
        .row_for_owner(membership.owner)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let (owner_row, rent) = prepare_owner_row_materialization_v5(
        entitlement,
        basis,
        input.owner_row,
        first_owner,
        expected_program_id,
    )?;
    Ok(PreparedEntitlementEndpointV5 {
        plan: EntitlementEndpointPlanV5 {
            membership,
            reservation: reservation_output,
            position: position_output,
            owner_row,
        },
        rent,
    })
}

fn prepare_owner_row_materialization_v5(
    entitlement: &CandidateEntitlementProjectionV4,
    basis: OwnerSettlementExpectationBasisV4,
    input: OwnerRowMaterializationInputV5<'_>,
    first_owner: bool,
    expected_program_id: Id32,
) -> Result<
    (OwnerRowMaterializationDispositionV5, Option<RentDebitFactV5>),
    SettlementAdapterErrorV1,
> {
    let seed = OwnerSettlementSeedTupleV5::new(
        entitlement.settlement_root.epoch(),
        entitlement.settlement_root.settlement_candidate_id(),
        Id32::new(basis.owner())?,
    )?;
    match input {
        OwnerRowMaterializationInputV5::Create {
            account,
            bump,
            funding,
            fee_evidence,
        } => {
            if !first_owner || funding.program_id != expected_program_id {
                return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
            }
            let expectation = match fee_evidence {
                OwnerRowFeeEvidenceV5::CandidateFee(selected_fee) => {
                    let expectation = basis.with_selected_fee(selected_fee.row())?;
                    if entitlement.settlement_root.fee_record().is_zero()
                        || entitlement.settlement_root.fee_record_state()
                            != SettlementRootChildStateV1::Live
                        || selected_fee.fee_record().0
                            != entitlement.settlement_root.fee_record().bytes()
                        || selected_fee.owner_settlement_account().0 != account.bytes()
                        || selected_fee.settlement_candidate().0
                            != entitlement
                                .settlement_root
                                .settlement_candidate_id()
                                .bytes()
                        || selected_fee.row().owner != basis.owner()
                        || selected_fee.expectation() != expectation
                    {
                        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
                    }
                    expectation
                }
                OwnerRowFeeEvidenceV5::NoFeeRecord => {
                    if !entitlement.settlement_root.fee_record().is_zero()
                        || entitlement.settlement_root.fee_record_state()
                            != SettlementRootChildStateV1::Absent
                    {
                        return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
                    }
                    basis.with_selected_fee(SelectedOwnerFeeV1 {
                        owner: basis.owner(),
                        fee_atoms: 0,
                    })?
                }
            };
            let plan = prepare_create_owner_settlement_account_v5(
                account,
                seed,
                bump,
                expectation,
                funding,
            )?;
            Ok((
                OwnerRowMaterializationDispositionV5::Create { fee_evidence, plan },
                Some(RentDebitFactV5 {
                    payer: funding.payer,
                    balance_before: funding.payer_lamports,
                    debit: plan.payer_debit_lamports(),
                }),
            ))
        }
        OwnerRowMaterializationInputV5::Existing { view } => {
            if first_owner {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            let projection = project_owner_settlement_account_v5(view, expected_program_id, seed)?;
            let accumulator = projection.envelope().semantic;
            let expectation = accumulator.expectation();
            if accumulator.state() != OwnerSettlementStateV4::Accumulating
                || accumulator.buy_cash_handoff_atoms() != 0
                || accumulator.consumed_buy_price_units() != PresentConsiderationV2::ABSENT
                || accumulator.consumed_sell_price_units() != PresentConsiderationV2::ABSENT
                || accumulator.completed_buy_order_mask() != 0
                || accumulator.completed_sell_order_mask() != 0
                || accumulator.progress_count() != 0
                || !expectation_matches_basis_v5(expectation, basis)
                || (entitlement.settlement_root.fee_record().is_zero()
                    && expectation.selected_fee_atoms() != 0)
            {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            Ok((
                OwnerRowMaterializationDispositionV5::Existing { projection },
                None,
            ))
        }
    }
}

fn validate_materialization_bundle_v5(
    entitlement: &CandidateEntitlementProjectionV4,
    receipt: &SettlementReceiptCreatePlanV5,
    endpoints: &[Option<EntitlementEndpointPlanV5>; 2],
    endpoint_count: u8,
    rent_debits: &[Option<RentDebitFactV5>; 3],
) -> Result<(), SettlementAdapterErrorV1> {
    if !(1..=2).contains(&endpoint_count) {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let selected = settlement_coordinates_v4(&entitlement.settlement_root)?;
    let mut accounts = [Id32::ZERO; 16];
    let mut account_count = 0usize;
    for account in [
        entitlement.settlement_root_account,
        entitlement.selected_feed_account(),
        selected.market,
        selected.epoch,
        selected.market_binding,
        receipt.account(),
    ] {
        insert_materialization_account_v5(&mut accounts, &mut account_count, account)?;
    }
    let mut page = 0u16;
    while usize::from(page) < usize::from(entitlement.order_projection().page_count()) {
        insert_materialization_account_v5(
            &mut accounts,
            &mut account_count,
            entitlement
                .order_projection()
                .page_account(page)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?,
        )?;
        page = page
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    let mut ordinal = 0usize;
    while ordinal < usize::from(endpoint_count) {
        let endpoint = endpoints[ordinal]
            .as_ref()
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        for account in [
            endpoint.reservation.account(),
            endpoint.position.account(),
            endpoint.owner_row.account(),
        ] {
            insert_materialization_account_v5(&mut accounts, &mut account_count, account)?;
        }
        match endpoint.owner_row {
            OwnerRowMaterializationDispositionV5::Create { plan, .. }
                if plan.program_id() != receipt.program_id() =>
            {
                return Err(SettlementAdapterErrorV1::BindingMismatch)
            }
            OwnerRowMaterializationDispositionV5::Existing { projection }
                if projection.program_owner() != receipt.program_id() =>
            {
                return Err(SettlementAdapterErrorV1::BindingMismatch)
            }
            _ => {}
        }
        ordinal += 1;
    }
    while ordinal < endpoints.len() {
        if endpoints[ordinal].is_some() {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        ordinal += 1;
    }
    require_materialization_party_not_target_v5(receipt.program_id(), &accounts[..account_count])?;
    require_materialization_party_not_target_v5(receipt.payer(), &accounts[..account_count])?;
    ordinal = 0;
    while ordinal < usize::from(endpoint_count) {
        if let OwnerRowMaterializationDispositionV5::Create { plan, .. } = endpoints[ordinal]
            .as_ref()
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?
            .owner_row
        {
            require_materialization_party_not_target_v5(plan.payer(), &accounts[..account_count])?;
        }
        ordinal += 1;
    }
    validate_aggregate_rent_debits_v5(rent_debits)
}

fn expectation_matches_basis_v5(
    expectation: OwnerSettlementExpectationV4,
    basis: OwnerSettlementExpectationBasisV4,
) -> bool {
    expectation.market() == basis.market()
        && expectation.epoch() == basis.epoch()
        && expectation.candidate() == basis.candidate()
        && expectation.owner() == basis.owner()
        && expectation.owner_order_set_digest() == basis.owner_order_set_digest()
        && expectation.price_scale() == basis.price_scale()
        && expectation.expected_buy_order_mask() == basis.expected_buy_order_mask()
        && expectation.expected_sell_order_mask() == basis.expected_sell_order_mask()
        && expectation.expected_slice_count() == basis.expected_slice_count()
        && expectation.expected_merge_delivery_count()
            == basis.expected_merge_delivery_count()
        && expectation.expected_buy_price_units() == basis.expected_buy_price_units()
        && expectation.expected_sell_price_units() == basis.expected_sell_price_units()
}

fn validate_aggregate_rent_debits_v5<const N: usize>(
    rent_debits: &[Option<RentDebitFactV5>; N],
) -> Result<(), SettlementAdapterErrorV1> {
    let mut fact = 0usize;
    while fact < rent_debits.len() {
        if let Some(current) = rent_debits[fact] {
            if current.payer.is_zero() || current.debit > current.balance_before {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            let mut total = 0u64;
            let mut peer = 0usize;
            while peer < rent_debits.len() {
                if let Some(other) = rent_debits[peer] {
                    if other.payer == current.payer {
                        if other.balance_before != current.balance_before {
                            return Err(SettlementAdapterErrorV1::BindingMismatch);
                        }
                        total = total
                            .checked_add(other.debit)
                            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                    }
                }
                peer += 1;
            }
            if total > current.balance_before {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
        }
        fact += 1;
    }
    Ok(())
}

fn require_materialization_party_not_target_v5(
    party: Id32,
    targets: &[Id32],
) -> Result<(), SettlementAdapterErrorV1> {
    if party.is_zero() {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut index = 0usize;
    while index < targets.len() {
        if targets[index] == party {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        index += 1;
    }
    Ok(())
}

fn insert_materialization_account_v5<const N: usize>(
    accounts: &mut [Id32; N],
    len: &mut usize,
    account: Id32,
) -> Result<(), SettlementAdapterErrorV1> {
    if account.is_zero() || *len >= N {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut prior = 0usize;
    while prior < *len {
        if accounts[prior] == account {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
        prior += 1;
    }
    accounts[*len] = account;
    *len += 1;
    Ok(())
}

/// One exact real endpoint presented to the strict non-portfolio action 26.
///
/// All fields remain structural until the live adapter rederives account
/// ownership, PDAs, writable metas, and bumps. No caller boolean is accepted
/// as an authentication fact.
#[derive(Clone, Copy, Debug)]
pub struct DirectEggDeliveryEndpointInputV5<'a> {
    /// Exact finalized rent-owned owner row.
    pub owner_row: OwnerSettlementAccountProjectionV5,
    /// Presented frozen OrderPage V5 account.
    pub order_page_account: Id32,
    /// Exact hostile 4,140-byte OrderPage V5 body.
    pub order_page_body: &'a [u8],
    /// Presented canonical Reservation V9 account.
    pub reservation_account: Id32,
    /// Exact hostile 666-byte Reservation V9 body.
    pub reservation_body: &'a [u8],
    /// Presented canonical Position V3 account and body.
    pub position: PositionAccountInputV3<'a>,
    /// Presented purpose-owned Replay V3 account.
    pub replay_account: Id32,
    /// Canonical Replay PDA bump rederived by the adapter.
    pub replay_bump: u8,
    /// Exact next sequence required by the Replay prestate.
    pub replay_next_sequence: u64,
    /// Exact hostile purpose-owned Replay V3 body.
    pub replay_body: &'a [u8],
}

/// Complete SBF-neutral input for one atomic direct two-real-end delivery.
#[derive(Clone, Copy, Debug)]
pub struct ConsumeDirectReceiptEggsInputV5<'a> {
    /// Strict compact action-26 selector (`epoch || receipt`).
    pub payload: ConsumeDirectReceiptEggsPayloadV1,
    /// Counted SettlementRoot account.
    pub settlement_root_account: Id32,
    /// Exact counted SettlementRoot prestate.
    pub settlement_root: &'a SettlementRootV1AccountV1,
    /// Counted retained sealed Feed account.
    pub retained_feed_account: Id32,
    /// Exact hostile retained sealed Feed body.
    pub retained_feed_body: &'a [u8],
    /// Exact current rent-owned receipt.
    pub receipt: SettlementReceiptAccountV5,
    /// PDA-derived V5 receipt evidence from the same exact current bytes.
    pub receipt_evidence: SettlementReceiptEvidenceV5,
    /// Immutable MarketBinding account.
    pub market_binding_account: Id32,
    /// Exact decoded immutable MarketBinding.
    pub market_binding: &'a MarketBindingV1,
    /// Exact Realm-selected collateral policy/release join.
    pub collateral: BoundCollateralProfileV2,
    /// Real buyer endpoint.
    pub buyer: DirectEggDeliveryEndpointInputV5<'a>,
    /// Real seller endpoint.
    pub seller: DirectEggDeliveryEndpointInputV5<'a>,
}

/// One exact real endpoint in the indivisible V5 action-26 bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectEggDeliveryEndpointPlanV5 {
    owner_settlement_account: Id32,
    owner_settlement_data_id: Id32,
    owner_settlement_body: [u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5],
    order_page_account: Id32,
    order_page_seed: GeneralOrderPageSeedTupleV5,
    order_slot_index: u8,
    position_generation: u64,
    reservation_account: Id32,
    reservation_semantic_id: Id32,
    reservation_seed: GeneralReservationSeedTupleV9,
    reservation_prestate_data_id: Id32,
    reservation_poststate_data_id: Id32,
    reservation_poststate_body: [u8; RESERVATION_ACCOUNT_BYTES_V9],
    position_account: Id32,
    position_prestate_semantic_id: Id32,
    position_poststate_semantic_id: Id32,
    position_poststate_body: [u8; POSITION_V3_BYTES],
    position: PositionSettlementPoststateV3,
    replay: GeneralReplayTransitionPlanV1,
    completes_order: bool,
    returned_internal: [u64; MAX_OUTCOMES],
}

impl DirectEggDeliveryEndpointPlanV5 {
    pub const fn owner_settlement_account(&self) -> Id32 { self.owner_settlement_account }
    pub const fn owner_settlement_data_id(&self) -> Id32 { self.owner_settlement_data_id }
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5] {
        &self.owner_settlement_body
    }
    pub const fn order_page_account(&self) -> Id32 { self.order_page_account }
    pub const fn order_page_seed(&self) -> GeneralOrderPageSeedTupleV5 { self.order_page_seed }
    pub const fn order_slot_index(&self) -> u8 { self.order_slot_index }
    pub const fn position_generation(&self) -> u64 { self.position_generation }
    pub const fn reservation_account(&self) -> Id32 { self.reservation_account }
    pub const fn reservation_semantic_id(&self) -> Id32 { self.reservation_semantic_id }
    pub const fn reservation_seed(&self) -> GeneralReservationSeedTupleV9 { self.reservation_seed }
    pub const fn reservation_prestate_data_id(&self) -> Id32 {
        self.reservation_prestate_data_id
    }
    pub const fn reservation_poststate_data_id(&self) -> Id32 {
        self.reservation_poststate_data_id
    }
    pub const fn reservation_poststate_body(&self) -> &[u8; RESERVATION_ACCOUNT_BYTES_V9] {
        &self.reservation_poststate_body
    }
    pub const fn position_account(&self) -> Id32 { self.position_account }
    pub const fn position_prestate_semantic_id(&self) -> Id32 {
        self.position_prestate_semantic_id
    }
    pub const fn position_poststate_semantic_id(&self) -> Id32 {
        self.position_poststate_semantic_id
    }
    pub const fn position_poststate_body(&self) -> &[u8; POSITION_V3_BYTES] {
        &self.position_poststate_body
    }
    pub const fn position(&self) -> PositionSettlementPoststateV3 { self.position }
    pub const fn replay(&self) -> &GeneralReplayTransitionPlanV1 { &self.replay }
    pub const fn completes_order(&self) -> bool { self.completes_order }
    pub const fn returned_internal(&self) -> [u64; MAX_OUTCOMES] { self.returned_internal }
}

/// One atomic V5 action-26 poststate bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsumeDirectReceiptEggsPlanV5 {
    settlement_root_account: Id32,
    retained_feed_account: Id32,
    receipt_account: Id32,
    receipt_seed: SettlementReceiptSeedTupleV5,
    receipt_prestate_evidence: SettlementReceiptEvidenceV5,
    delivery_transition_id: Id32,
    receipt_poststate: SettlementReceiptAccountV5,
    receipt_poststate_evidence: SettlementReceiptEvidenceV5,
    receipt_poststate_body: [u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5],
    buyer: DirectEggDeliveryEndpointPlanV5,
    seller: DirectEggDeliveryEndpointPlanV5,
}

impl ConsumeDirectReceiptEggsPlanV5 {
    pub const fn settlement_root_account(&self) -> Id32 { self.settlement_root_account }
    pub const fn retained_feed_account(&self) -> Id32 { self.retained_feed_account }
    pub const fn receipt_account(&self) -> Id32 { self.receipt_account }
    pub const fn receipt_seed(&self) -> SettlementReceiptSeedTupleV5 { self.receipt_seed }
    pub const fn receipt_prestate_evidence(&self) -> SettlementReceiptEvidenceV5 {
        self.receipt_prestate_evidence
    }
    pub const fn delivery_transition_id(&self) -> Id32 { self.delivery_transition_id }
    pub const fn receipt_poststate(&self) -> SettlementReceiptAccountV5 {
        self.receipt_poststate
    }
    pub const fn receipt_poststate_evidence(&self) -> SettlementReceiptEvidenceV5 {
        self.receipt_poststate_evidence
    }
    pub const fn receipt_poststate_body(&self) -> &[u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5] {
        &self.receipt_poststate_body
    }
    pub const fn buyer(&self) -> &DirectEggDeliveryEndpointPlanV5 { &self.buyer }
    pub const fn seller(&self) -> &DirectEggDeliveryEndpointPlanV5 { &self.seller }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectOrderEndBindingV5 {
    slot: OrderSlot,
    order_index: u8,
    page_index: u16,
    slot_index: u8,
    position_generation: u64,
    entitled_units: u64,
    completes_order: bool,
}

/// Prepare the complete strict non-portfolio action-26 transition.
pub fn prepare_consume_direct_receipt_eggs_v5(
    input: ConsumeDirectReceiptEggsInputV5<'_>,
) -> Result<ConsumeDirectReceiptEggsPlanV5, SettlementAdapterErrorV1> {
    input.settlement_root.validate()?;
    input.market_binding.validate()?;
    input.receipt.validate()?;
    let root = input.settlement_root;
    let selected = settlement_coordinates_v4(root)?;
    let counts = root.counts();
    let (feed, tail) = complete_candidate_feed_v2(input.retained_feed_body, true)?;
    let feed_bundle_id = derive_candidate_bundle_digest_v1(input.retained_feed_body)?;
    let receipt_account = input.payload.receipt;
    let receipt_pda = LayoutHash32::new(receipt_account.bytes())?;
    let expected_receipt_evidence = input.receipt.evidence(receipt_pda)?;
    let mut receipt_semantic = input.receipt.semantic();
    if input.payload.epoch != selected.epoch
        || receipt_account.is_zero()
        || input.settlement_root_account.is_zero()
        || input.retained_feed_account != selected.retained_feed
        || feed_bundle_id != selected.candidate_bundle_digest
        || input.market_binding_account != selected.market_binding
        || input.market_binding.market != selected.market
        || root.phase() != SettlementRootPhaseV1::Settling
        || counts.admitted_receipts != counts.expected_receipts
        || counts.live_receipts != counts.admitted_receipts
        || feed.epoch != selected.epoch
        || feed.market != selected.market
        || feed.node != selected.source_admission_node
        || feed.order_set != selected.order_set
        || feed.settlement_candidate_id != selected.settlement_candidate_id
        || feed.settlement_witness_digest != selected.settlement_witness_digest
        || feed.epoch_generation != selected.epoch_generation
        || feed.slice_count != selected.slice_count
        || feed.relation_policy_id != input.market_binding.relation_policy_id
        || feed.price_measure_policy_v1_id != input.market_binding.price_measure_policy_v1_id
        || feed.native_claim_basis_id != input.market_binding.native_claim_basis_id
        || feed.price_scale != input.market_binding.price_scale
        || feed.outcome_count != input.market_binding.outcome_count
        || root.outcome_count() != feed.outcome_count
        || root.market_instance_v2_id() != input.market_binding.market_instance_v2_id
        || input.collateral.market().market.bytes()
            != input.market_binding.market_instance_v2_id.bytes()
        || input.receipt.transition() != SettlementReceiptTransitionCommitmentV5::None
        || expected_receipt_evidence != input.receipt_evidence
        || input.receipt_evidence.receipt().bytes() != receipt_account.bytes()
        || receipt_semantic.epoch.bytes() != selected.epoch.bytes()
        || receipt_semantic.market.bytes() != selected.market.bytes()
        || receipt_semantic.candidate.bytes() != selected.settlement_candidate_id.bytes()
        || receipt_semantic.leg_kind != RECEIPT_LEG_DIRECT
        || receipt_semantic.accounted_end_mask != receipt_semantic.expected_end_mask()
        || receipt_semantic.expected_end_mask()
            != RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED
        || receipt_semantic.delivered_end_mask() != 0
        || receipt_semantic.settled_quantity != 0
        || receipt_semantic.outcome >= feed.outcome_count
        || receipt_semantic.slice_index >= feed.slice_count
        || receipt_semantic.sequence != u64::from(receipt_semantic.slice_index) + 1
        || receipt_semantic.price
            != read_feed_u64(tail.prices_le(), usize::from(receipt_semantic.outcome))?
        || receipt_semantic.consideration_price_units
            != u128::from(receipt_semantic.quantity)
                .checked_mul(u128::from(receipt_semantic.price))
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let target_slice = read_feed_slice_v3(tail.slices_le(), receipt_semantic.slice_index)?;
    if target_slice.buy_kind != 0
        || target_slice.sell_kind != 0
        || target_slice.outcome != receipt_semantic.outcome
        || target_slice.quantity != receipt_semantic.quantity
    {
        return Err(SettlementAdapterErrorV1::ReceiptLatchMismatch);
    }
    require_direct_endpoint_account_partition_v5(
        input.settlement_root_account,
        input.retained_feed_account,
        receipt_account,
        input.market_binding_account,
        input.buyer,
        input.seller,
    )?;
    let delivery_transition_id = Id32::new(input.receipt_evidence.delivery_transition_id().bytes())?;
    let receipt_prestate_data_id = Id32::new(input.receipt_evidence.receipt_data_id().bytes())?;
    let buyer = stage_direct_egg_delivery_endpoint_v5(
        selected,
        feed,
        tail.slices_le(),
        receipt_semantic,
        target_slice.buy_index,
        delivery_transition_id,
        receipt_prestate_data_id,
        root.owner_order_set_digest(),
        input.market_binding,
        input.collateral,
        input.buyer,
        SettlementSideV1::Buy,
        GeneralReplayTransitionKindV1::DirectBuyer,
    )?;
    let seller = stage_direct_egg_delivery_endpoint_v5(
        selected,
        feed,
        tail.slices_le(),
        receipt_semantic,
        target_slice.sell_index,
        delivery_transition_id,
        receipt_prestate_data_id,
        root.owner_order_set_digest(),
        input.market_binding,
        input.collateral,
        input.seller,
        SettlementSideV1::Sell,
        GeneralReplayTransitionKindV1::DirectSeller,
    )?;
    if buyer.owner_settlement_account == seller.owner_settlement_account
        || buyer.reservation_account == seller.reservation_account
        || buyer.position_account == seller.position_account
        || buyer.replay.replay_account() == seller.replay.replay_account()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    receipt_semantic.settled_quantity = receipt_semantic.quantity;
    receipt_semantic.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED
        | RECEIPT_FLAG_SELL_CONSUMED
        | RECEIPT_FLAG_SLICE_EXHAUSTED;
    let receipt_poststate = SettlementReceiptAccountV5::new(
        receipt_semantic,
        SettlementReceiptTransitionCommitmentV5::None,
        input.receipt.rent(),
    )?;
    let receipt_poststate_body = receipt_poststate.encode_exact()?;
    let receipt_poststate_evidence = receipt_poststate.evidence(receipt_pda)?;
    let receipt_seed = SettlementReceiptSeedTupleV5::new(
        selected.epoch,
        selected.settlement_candidate_id,
        receipt_semantic.slice_index,
    )?;
    Ok(ConsumeDirectReceiptEggsPlanV5 {
        settlement_root_account: input.settlement_root_account,
        retained_feed_account: input.retained_feed_account,
        receipt_account,
        receipt_seed,
        receipt_prestate_evidence: input.receipt_evidence,
        delivery_transition_id,
        receipt_poststate,
        receipt_poststate_evidence,
        receipt_poststate_body,
        buyer,
        seller,
    })
}

#[allow(clippy::too_many_arguments)]
fn stage_direct_egg_delivery_endpoint_v5(
    selected: SettlementCoordinatesV4,
    feed: CandidateFeedHeaderV2,
    slices: &[u8],
    receipt: SettlementReceiptAccountV4,
    expected_order_index: u8,
    delivery_transition_id: Id32,
    receipt_prestate_data_id: Id32,
    owner_order_set_digest: Id32,
    market_binding: &MarketBindingV1,
    collateral: BoundCollateralProfileV2,
    input: DirectEggDeliveryEndpointInputV5<'_>,
    side: SettlementSideV1,
    replay_kind: GeneralReplayTransitionKindV1,
) -> Result<DirectEggDeliveryEndpointPlanV5, SettlementAdapterErrorV1> {
    if !matches!(
        (side, replay_kind),
        (SettlementSideV1::Buy, GeneralReplayTransitionKindV1::DirectBuyer)
            | (SettlementSideV1::Sell, GeneralReplayTransitionKindV1::DirectSeller)
    ) {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let envelope = input.owner_row.envelope();
    let terminal = envelope.terminal_projection()?;
    let expectation = terminal.semantic().expectation();
    let owner = Id32::new(expectation.owner())?;
    let order_end = bind_direct_single_order_end_v5(
        feed,
        slices,
        receipt,
        expected_order_index,
        owner,
        input.order_page_body,
        side,
    )?;
    let order_bit = 1u64
        .checked_shl(u32::from(order_end.order_index))
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let expected_mask = match side {
        SettlementSideV1::Buy => expectation.expected_buy_order_mask(),
        SettlementSideV1::Sell => expectation.expected_sell_order_mask(),
    };
    if expectation.market() != selected.market.bytes()
        || expectation.epoch() != selected.epoch.bytes()
        || expectation.candidate() != selected.settlement_candidate_id.bytes()
        || expectation.owner_order_set_digest() != owner_order_set_digest.bytes()
        || input.owner_row.seed().epoch() != &selected.epoch.bytes()
        || input.owner_row.seed().settlement_candidate()
            != &selected.settlement_candidate_id.bytes()
        || input.owner_row.seed().owner() != &owner.bytes()
        || expected_mask & order_bit == 0
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let reservation_prestate = ReservationAccountV9::decode(input.reservation_body)?;
    let mut reservation = reservation_prestate.body();
    let slot = order_end.slot;
    let order_id = slot.order_id();
    let order_generation = slot.generation();
    let expected_side = match side {
        SettlementSideV1::Buy => 0,
        SettlementSideV1::Sell => 1,
    };
    let single_outcome = match slot {
        OrderSlot::Single(order) => order.outcome,
        OrderSlot::Portfolio(_) | OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            return Err(SettlementAdapterErrorV1::BindingMismatch)
        }
    };
    let reservation_plan = ReservationPlan::for_order(
        &slot,
        feed.outcome_count,
        feed.price_scale,
        reservation.max_fee_atoms,
    )?;
    let expected_reservation = canonical_reservation_id_v9(
        LayoutHash32(selected.market.bytes()),
        LayoutHash32(selected.epoch.bytes()),
        LayoutHash32(owner.bytes()),
        order_end.position_generation,
        order_id,
    );
    if input.reservation_account.is_zero()
        || reservation.reservation != expected_reservation
        || reservation.market.bytes() != selected.market.bytes()
        || reservation.epoch.bytes() != selected.epoch.bytes()
        || reservation.owner.bytes() != owner.bytes()
        || reservation.order_id != order_id
        || reservation.order_generation != order_generation
        || reservation.position_generation != order_end.position_generation
        || reservation.page_index != order_end.page_index
        || reservation.outcome_count != feed.outcome_count
        || reservation.side != expected_side
        || reservation.order_kind != 1
        || single_outcome != receipt.outcome
        || reservation.state != RESERVATION_STATE_ENTITLED
        || reservation.entitled_units != order_end.entitled_units
        || reservation.consumed_units >= reservation.entitled_units
        || reservation.paid_units != reservation.consumed_units
        || reservation.initial_cash_atoms != reservation_plan.cash_atoms
        || reservation.max_fee_atoms != reservation_plan.max_fee_atoms
        || reservation.initial_internal != reservation_plan.internal
        || (expected_side == 0
            && (reservation.remaining_cash_atoms != 0
                || reservation.remaining_internal != [0; MAX_OUTCOMES]))
        || (expected_side == 1
            && (reservation.initial_cash_atoms != 0 || reservation.remaining_cash_atoms != 0))
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    let (completes_order, returned_internal) = advance_direct_reservation_v9(
        side,
        receipt.outcome,
        receipt.quantity,
        &mut reservation,
    )?;
    if completes_order != order_end.completes_order {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    let reservation_poststate = ReservationAccountV9::new(reservation, reservation_prestate.rent())?;
    let reservation_prestate_data_id = Id32::new(reservation_prestate.data_id()?.bytes())?;
    let reservation_poststate_data_id = Id32::new(reservation_poststate.data_id()?.bytes())?;
    let mut reservation_poststate_body = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
    reservation_poststate.encode(&mut reservation_poststate_body)?;

    let position_body = PositionAccountV3::decode(input.position.encoded_body)?;
    project_canonical_general_settlement_position_v3(
        position_body,
        owner,
        order_end.position_generation,
        selected.market,
        AdapterPositionMarketBindingV3 {
            market_instance_id: retirement_identity(market_binding.market_instance_v2_id)?,
            outcome_count: feed.outcome_count,
            realm_id: retirement_identity(Id32::new(collateral.realm_bound().realm().realm.bytes())?)?,
            collateral_policy_id: retirement_identity(Id32::new(collateral.policy_id().bytes())?)?,
            collateral_release_id: retirement_identity(Id32::new(collateral.release().id()?.bytes())?)?,
        },
    )?;
    let position_prestate_semantic_id =
        Id32::new(position_body.semantic_id(&PositionBodySha256V3)?.bytes())?;
    let position = AuthenticatedPositionV3 {
        account: input.position.account.bytes(),
        general_market_runtime: selected.market.bytes(),
        semantic: position_body,
        semantic_id: position_prestate_semantic_id.bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    position.validate_writable()?;
    let mut native_eggs = position_body.native_eggs();
    match side {
        SettlementSideV1::Buy => {
            let outcome = usize::from(receipt.outcome);
            native_eggs[outcome] = native_eggs[outcome]
                .checked_add(receipt.quantity)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
        }
        SettlementSideV1::Sell if completes_order => {
            let mut outcome = 0usize;
            while outcome < MAX_OUTCOMES {
                native_eggs[outcome] = native_eggs[outcome]
                    .checked_add(returned_internal[outcome])
                    .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
                outcome += 1;
            }
        }
        SettlementSideV1::Sell => {}
    }
    let position_poststate = position.settlement_poststate(
        position_body.cash_atoms(),
        position_body.reserved_cash_atoms(),
        native_eggs,
    )?;
    let position_poststate_semantic_id =
        Id32::new(position_poststate.semantic.semantic_id(&PositionBodySha256V3)?.bytes())?;
    let position_poststate_body = position_poststate.semantic.encode()?;
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
        position_poststate,
        replay_kind,
        delivery_transition_id,
        receipt_prestate_data_id,
        &PositionBodySha256V3,
    )?;
    if replay.position_prestate_semantic_id() != position_prestate_semantic_id
        || replay.position_poststate_semantic_id() != position_poststate_semantic_id
        || replay.transition_id() != delivery_transition_id
        || replay.transition_evidence_id() != receipt_prestate_data_id
        || replay.kind() != replay_kind
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let reservation_semantic_id = Id32::new(reservation.reservation.bytes())?;
    Ok(DirectEggDeliveryEndpointPlanV5 {
        owner_settlement_account: input.owner_row.account(),
        owner_settlement_data_id: input.owner_row.data_id(),
        owner_settlement_body: *input.owner_row.exact_body(),
        order_page_account: input.order_page_account,
        order_page_seed: GeneralOrderPageSeedTupleV5::new(selected.epoch, order_end.page_index)?,
        order_slot_index: order_end.slot_index,
        position_generation: order_end.position_generation,
        reservation_account: input.reservation_account,
        reservation_semantic_id,
        reservation_seed: GeneralReservationSeedTupleV9::new(reservation_semantic_id)?,
        reservation_prestate_data_id,
        reservation_poststate_data_id,
        reservation_poststate_body,
        position_account: input.position.account,
        position_prestate_semantic_id,
        position_poststate_semantic_id,
        position_poststate_body,
        position: position_poststate,
        replay,
        completes_order,
        returned_internal,
    })
}

fn bind_direct_single_order_end_v5(
    feed: CandidateFeedHeaderV2,
    slices: &[u8],
    receipt: SettlementReceiptAccountV4,
    order_index: u8,
    owner: Id32,
    order_page_body: &[u8],
    side: SettlementSideV1,
) -> Result<DirectOrderEndBindingV5, SettlementAdapterErrorV1> {
    if order_index >= feed.order_count {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let page = verify_page_v5(order_page_body)?;
    if page.market.bytes() != feed.market.bytes()
        || page.epoch.bytes() != feed.epoch.bytes()
        || page.order_set.bytes() != feed.order_set.bytes()
        || page.frozen != 1
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let expected_order_id = match side {
        SettlementSideV1::Buy => receipt.buy_order_id,
        SettlementSideV1::Sell => receipt.sell_order_id,
    };
    let mut cursor = OrderSlotCursorV5::new(order_page_body)?;
    let mut verified = None;
    while let Some(next) = cursor.next_slot() {
        let slot = next?;
        if slot.slot.is_live() && slot.slot.order_id() == expected_order_id {
            if verified.is_some() {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            verified = Some(slot);
        }
    }
    let verified = verified.ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let record = match verified.slot {
        OrderSlot::Single(record) => record,
        OrderSlot::Portfolio(_) | OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            return Err(SettlementAdapterErrorV1::BindingMismatch)
        }
    };
    let expected_side = match side {
        SettlementSideV1::Buy => 0,
        SettlementSideV1::Sell => 1,
    };
    if verified.slot.owner().bytes() != owner.bytes()
        || record.side != expected_side
        || record.outcome != receipt.outcome
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut entitled_units = 0u64;
    let mut completes_order = true;
    let mut occurrence_count = 0u16;
    let mut slice_index = 0u16;
    while slice_index < feed.slice_count {
        let slice = read_feed_slice_v3(slices, slice_index)?;
        let matches = match side {
            SettlementSideV1::Buy => slice.buy_kind == 0 && slice.buy_index == order_index,
            SettlementSideV1::Sell => slice.sell_kind == 0 && slice.sell_index == order_index,
        };
        if matches {
            entitled_units = entitled_units
                .checked_add(slice.quantity)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            occurrence_count = occurrence_count
                .checked_add(1)
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
            if slice_index > receipt.slice_index {
                completes_order = false;
            }
        }
        slice_index = slice_index
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    if occurrence_count == 0 {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(DirectOrderEndBindingV5 {
        slot: verified.slot,
        order_index,
        page_index: page.page_index,
        slot_index: verified.slot_index,
        position_generation: verified.position_generation,
        entitled_units,
        completes_order,
    })
}

fn advance_direct_reservation_v9(
    side: SettlementSideV1,
    outcome: u8,
    quantity: u64,
    reservation: &mut clutch_solana_layout::reservation::ReservationAccount,
) -> Result<(bool, [u64; MAX_OUTCOMES]), SettlementAdapterErrorV1> {
    if quantity == 0
        || outcome >= reservation.outcome_count
        || reservation.state != RESERVATION_STATE_ENTITLED
        || reservation.side
            != match side {
                SettlementSideV1::Buy => 0,
                SettlementSideV1::Sell => 1,
            }
        || reservation.paid_units != reservation.consumed_units
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    let next_units = reservation
        .consumed_units
        .checked_add(quantity)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    if next_units > reservation.entitled_units {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    reservation.consumed_units = next_units;
    reservation.paid_units = next_units;
    let completes_order = next_units == reservation.entitled_units;
    let mut returned_internal = [0u64; MAX_OUTCOMES];
    match side {
        SettlementSideV1::Buy => {
            if reservation.remaining_cash_atoms != 0
                || reservation.remaining_internal != [0; MAX_OUTCOMES]
            {
                return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
            }
        }
        SettlementSideV1::Sell => {
            let outcome = usize::from(outcome);
            reservation.remaining_internal[outcome] = reservation.remaining_internal[outcome]
                .checked_sub(quantity)
                .ok_or(SettlementAdapterErrorV1::ReservationSetMismatch)?;
            if completes_order {
                returned_internal = reservation.remaining_internal;
                reservation.remaining_internal = [0; MAX_OUTCOMES];
            }
        }
    }
    if completes_order {
        if !reservation.remaining_is_zero() {
            return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
        }
        reservation.state = RESERVATION_STATE_CONSUMED;
    }
    Ok((completes_order, returned_internal))
}

fn require_direct_endpoint_account_partition_v5(
    settlement_root: Id32,
    retained_feed: Id32,
    receipt: Id32,
    market_binding: Id32,
    buyer: DirectEggDeliveryEndpointInputV5<'_>,
    seller: DirectEggDeliveryEndpointInputV5<'_>,
) -> Result<(), SettlementAdapterErrorV1> {
    let base = [settlement_root, retained_feed, receipt, market_binding];
    let buyer_accounts = [
        buyer.owner_row.account(),
        buyer.reservation_account,
        buyer.position.account,
        buyer.replay_account,
    ];
    let seller_accounts = [
        seller.owner_row.account(),
        seller.reservation_account,
        seller.position.account,
        seller.replay_account,
    ];
    let mut all = [Id32::ZERO; 12];
    let mut len = 0usize;
    for account in base {
        insert_materialization_account_v5(&mut all, &mut len, account)?;
    }
    for account in buyer_accounts {
        insert_materialization_account_v5(&mut all, &mut len, account)?;
    }
    for account in seller_accounts {
        insert_materialization_account_v5(&mut all, &mut len, account)?;
    }
    for page in [buyer.order_page_account, seller.order_page_account] {
        if page.is_zero() || all[..len].contains(&page) {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
    }
    if buyer.owner_row.program_owner() != seller.owner_row.program_owner()
        || buyer.owner_row.program_owner().is_zero()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(())
}

/// Exact durable fee-finalization account presented to action 40.
///
/// The live adapter must authenticate the program owner, fee-carry PDA, bump,
/// and readonly meta before this pure contract decodes the exact outer bytes.
#[derive(Clone, Copy, Debug)]
pub struct MergePaymentFeeFinalizationInputV4<'a> {
    /// Existing owner carry PDA, now the immutable 0x83/v4 successor.
    pub account: Id32,
    /// Exact hostile 548-byte rent-owned finalization account.
    pub exact_body: &'a [u8],
}

/// One seller endpoint presented to action 40.
#[derive(Clone, Copy, Debug)]
pub struct MergeReceiptPaymentEndpointInputV5<'a> {
    /// Exact finalized rent-owned owner row.
    pub owner_row: OwnerSettlementAccountProjectionV5,
    /// Presented frozen OrderPage V5 account.
    pub order_page_account: Id32,
    /// Exact hostile 4,140-byte OrderPage V5 body.
    pub order_page_body: &'a [u8],
    /// Presented canonical Reservation V9 account.
    pub reservation_account: Id32,
    /// Exact hostile 666-byte Reservation V9 body.
    pub reservation_body: &'a [u8],
    /// Presented canonical Position V3 account and body.
    pub position: PositionAccountInputV3<'a>,
    /// Presented purpose-owned Replay V3 account.
    pub replay_account: Id32,
    /// Canonical Replay PDA bump rederived by the adapter.
    pub replay_bump: u8,
    /// Exact next sequence required by the Replay prestate.
    pub replay_next_sequence: u64,
    /// Exact hostile purpose-owned Replay V3 body.
    pub replay_body: &'a [u8],
}

/// Complete SBF-neutral action-40 input.
#[derive(Clone, Copy, Debug)]
pub struct FinalizeMergeReceiptPaymentInputV5<'a> {
    /// Strict action-40 selector coordinates.
    pub payload: FinalizeMergeReceiptPaymentPayloadV1,
    /// Counted SettlementRoot account.
    pub settlement_root_account: Id32,
    /// Exact counted SettlementRoot prestate.
    pub settlement_root: &'a SettlementRootV1AccountV1,
    /// Counted retained sealed Feed account.
    pub retained_feed_account: Id32,
    /// Exact hostile retained sealed Feed body.
    pub retained_feed_body: &'a [u8],
    /// Exact payment-pending rent-owned receipt.
    pub receipt: SettlementReceiptAccountV5,
    /// PDA-derived V5 receipt evidence from those exact bytes.
    pub receipt_evidence: SettlementReceiptEvidenceV5,
    /// Immutable MarketBinding account.
    pub market_binding_account: Id32,
    /// Exact decoded immutable MarketBinding.
    pub market_binding: &'a MarketBindingV1,
    /// Exact Realm-selected collateral policy/release join.
    pub collateral: BoundCollateralProfileV2,
    /// Counted candidate cash-pot account; action 40 never mutates it.
    pub settlement_cash_pot_account: Id32,
    /// Exact current candidate cash-pot outer; action 40 never mutates it.
    pub settlement_cash_pot: SettlementCashPotV1AccountV1,
    /// Durable fee finalization, present exactly on a fee-bearing root.
    pub fee_finalization: Option<MergePaymentFeeFinalizationInputV4<'a>>,
    /// Real seller endpoint whose paid Reservation cursor advances.
    pub seller: MergeReceiptPaymentEndpointInputV5<'a>,
}

/// Disjoint semantic source of the stable per-owner payment chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MergePaymentFinalizationSourceV5 {
    /// Current action-38 Replay evidence; no phantom fee account exists.
    ZeroFeeReplay = 1,
    /// Durable exact 0x83/v4 owner-fee finalization outer.
    FeeFinalizationV4 = 2,
}

impl MergePaymentFinalizationSourceV5 {
    const fn code(self) -> u8 {
        match self {
            Self::ZeroFeeReplay => 1,
            Self::FeeFinalizationV4 => 2,
        }
    }
}

/// One atomic V5 action-40 poststate bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizeMergeReceiptPaymentPlanV5 {
    settlement_root_account: Id32,
    settlement_root_poststate: SettlementRootV1AccountV1,
    retained_feed_account: Id32,
    receipt_account: Id32,
    receipt_seed: SettlementReceiptSeedTupleV5,
    receipt_prestate_evidence: SettlementReceiptEvidenceV5,
    payment_transition_id: Id32,
    receipt_poststate: SettlementReceiptAccountV5,
    receipt_poststate_evidence: SettlementReceiptEvidenceV5,
    receipt_poststate_body: [u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5],
    owner_settlement_account: Id32,
    finalized_owner_row_data_id: Id32,
    owner_settlement_body: [u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5],
    settlement_cash_pot_account: Id32,
    settlement_cash_pot_poststate_data_id: Option<Id32>,
    finalization_source: MergePaymentFinalizationSourceV5,
    finalization_source_data_id: Option<Id32>,
    stable_owner_finalization_evidence_id: Id32,
    order_page_account: Id32,
    order_page_seed: GeneralOrderPageSeedTupleV5,
    order_slot_index: u8,
    position_generation: u64,
    reservation_account: Id32,
    reservation_semantic_id: Id32,
    reservation_seed: GeneralReservationSeedTupleV9,
    reservation_prestate_data_id: Id32,
    reservation_poststate_data_id: Id32,
    reservation_poststate_body: [u8; RESERVATION_ACCOUNT_BYTES_V9],
    position_account: Id32,
    position_semantic_id: Id32,
    replay: GeneralReplayTransitionPlanV1,
    completes_order: bool,
}

impl FinalizeMergeReceiptPaymentPlanV5 {
    /// Counted SettlementRoot account to compare-and-write.
    pub const fn settlement_root_account(&self) -> Id32 { self.settlement_root_account }
    /// Exact structural counted-root successor.
    pub const fn settlement_root_poststate(&self) -> &SettlementRootV1AccountV1 {
        &self.settlement_root_poststate
    }
    /// Retained sealed Feed account authenticated by the composer.
    pub const fn retained_feed_account(&self) -> Id32 { self.retained_feed_account }
    /// Payment-pending Receipt V5 account to compare-and-write.
    pub const fn receipt_account(&self) -> Id32 { self.receipt_account }
    /// Fresh V5 seed tuple for that receipt.
    pub const fn receipt_seed(&self) -> SettlementReceiptSeedTupleV5 { self.receipt_seed }
    /// Exact payment-pending prestate evidence.
    pub const fn receipt_prestate_evidence(&self) -> SettlementReceiptEvidenceV5 {
        self.receipt_prestate_evidence
    }
    /// PDA-derived disjoint payment transition identity.
    pub const fn payment_transition_id(&self) -> Id32 { self.payment_transition_id }
    /// Exact exhausted receipt successor.
    pub const fn receipt_poststate(&self) -> SettlementReceiptAccountV5 {
        self.receipt_poststate
    }
    /// PDA-derived exact exhausted receipt evidence.
    pub const fn receipt_poststate_evidence(&self) -> SettlementReceiptEvidenceV5 {
        self.receipt_poststate_evidence
    }
    /// Exact 298-byte exhausted receipt successor.
    pub const fn receipt_poststate_body(&self) -> &[u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5] {
        &self.receipt_poststate_body
    }
    /// Finalized rent-owned owner row consumed readonly.
    pub const fn owner_settlement_account(&self) -> Id32 { self.owner_settlement_account }
    /// Full exact V5 owner-row data identity.
    pub const fn finalized_owner_row_data_id(&self) -> Id32 {
        self.finalized_owner_row_data_id
    }
    /// Exact unchanged 340-byte finalized owner row.
    pub const fn owner_settlement_body(&self) -> &[u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5] {
        &self.owner_settlement_body
    }
    /// Candidate cash-pot account consumed readonly.
    pub const fn settlement_cash_pot_account(&self) -> Id32 {
        self.settlement_cash_pot_account
    }
    /// Exact action-38 pot poststate when available from current/durable evidence.
    pub const fn settlement_cash_pot_poststate_data_id(&self) -> Option<Id32> {
        self.settlement_cash_pot_poststate_data_id
    }
    /// Disjoint zero-fee versus fee-bearing evidence source.
    pub const fn finalization_source(&self) -> MergePaymentFinalizationSourceV5 {
        self.finalization_source
    }
    /// Exact source data identity when current/durable evidence exposes it.
    pub const fn finalization_source_data_id(&self) -> Option<Id32> {
        self.finalization_source_data_id
    }
    /// Stable per-owner evidence carried by this and every later payment.
    pub const fn stable_owner_finalization_evidence_id(&self) -> Id32 {
        self.stable_owner_finalization_evidence_id
    }
    /// Frozen OrderPage V5 account containing the seller order.
    pub const fn order_page_account(&self) -> Id32 { self.order_page_account }
    /// Exact frozen OrderPage V5 seed tuple.
    pub const fn order_page_seed(&self) -> GeneralOrderPageSeedTupleV5 { self.order_page_seed }
    /// Physical slot index inside that page.
    pub const fn order_slot_index(&self) -> u8 { self.order_slot_index }
    /// Immutable Position generation frozen beside the order.
    pub const fn position_generation(&self) -> u64 { self.position_generation }
    /// Seller Reservation V9 account to compare-and-write.
    pub const fn reservation_account(&self) -> Id32 { self.reservation_account }
    /// Canonical Reservation V9 semantic identity.
    pub const fn reservation_semantic_id(&self) -> Id32 { self.reservation_semantic_id }
    /// Exact Reservation V9 seed tuple.
    pub const fn reservation_seed(&self) -> GeneralReservationSeedTupleV9 {
        self.reservation_seed
    }
    /// Exact Reservation V9 prestate data identity.
    pub const fn reservation_prestate_data_id(&self) -> Id32 {
        self.reservation_prestate_data_id
    }
    /// Exact Reservation V9 successor data identity.
    pub const fn reservation_poststate_data_id(&self) -> Id32 {
        self.reservation_poststate_data_id
    }
    /// Exact 666-byte Reservation V9 successor.
    pub const fn reservation_poststate_body(&self) -> &[u8; RESERVATION_ACCOUNT_BYTES_V9] {
        &self.reservation_poststate_body
    }
    /// Canonical seller Position V3 account consumed readonly.
    pub const fn position_account(&self) -> Id32 { self.position_account }
    /// Exact unchanged seller Position V3 semantic identity.
    pub const fn position_semantic_id(&self) -> Id32 { self.position_semantic_id }
    /// Exact purpose-owned GEN1 Replay successor.
    pub const fn replay(&self) -> &GeneralReplayTransitionPlanV1 { &self.replay }
    /// Whether this payment makes the Reservation fully consumed.
    pub const fn completes_order(&self) -> bool { self.completes_order }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MergePaymentOrderBindingV5 {
    slot: OrderSlot,
    page_index: u16,
    slot_index: u8,
    position_generation: u64,
}

fn derive_merge_payment_owner_finalization_evidence_v5(
    settlement_root_account: Id32,
    candidate: Id32,
    owner: Id32,
    finalized_owner_row_data_id: Id32,
    pot_poststate_data_id: Id32,
    source: MergePaymentFinalizationSourceV5,
    source_data_id: Id32,
) -> Result<Id32, SettlementAdapterErrorV1> {
    if settlement_root_account.is_zero()
        || candidate.is_zero()
        || owner.is_zero()
        || finalized_owner_row_data_id.is_zero()
        || pot_poststate_data_id.is_zero()
        || source_data_id.is_zero()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut hash = Sha256::new();
    hash.update(MERGE_PAYMENT_OWNER_FINALIZATION_EVIDENCE_DOMAIN_V5);
    hash.update(settlement_root_account.bytes());
    hash.update(candidate.bytes());
    hash.update(owner.bytes());
    hash.update(finalized_owner_row_data_id.bytes());
    hash.update(pot_poststate_data_id.bytes());
    hash.update([source.code()]);
    hash.update(source_data_id.bytes());
    Id32::new(hash.finalize().into()).map_err(SettlementAdapterErrorV1::Contract)
}

/// Prepare one exact merge-receipt payment after owner cash finalization.
///
/// The plan moves no cash and does not mutate Position. It atomically advances
/// only the Reservation paid cursor, the receipt exhausted latch, purpose-owned
/// Replay, and the counted SettlementRoot payment counter.
pub fn prepare_finalize_merge_receipt_payment_v5(
    input: FinalizeMergeReceiptPaymentInputV5<'_>,
) -> Result<FinalizeMergeReceiptPaymentPlanV5, SettlementAdapterErrorV1> {
    input.settlement_root.validate()?;
    input.market_binding.validate()?;
    input.receipt.validate()?;
    input.settlement_cash_pot.semantic.validate()?;
    let root = input.settlement_root;
    let selected = settlement_coordinates_v4(root)?;
    let counts = root.counts();
    let (feed, tail) = complete_candidate_feed_v2(input.retained_feed_body, true)?;
    let feed_bundle_id = derive_candidate_bundle_digest_v1(input.retained_feed_body)?;
    let receipt_account = input.payload.receipt;
    let receipt_pda = LayoutHash32::new(receipt_account.bytes())?;
    let expected_receipt_evidence = input.receipt.evidence(receipt_pda)?;
    let mut receipt_semantic = input.receipt.semantic();
    if input.payload.epoch != selected.epoch
        || receipt_account.is_zero()
        || input.settlement_root_account.is_zero()
        || input.retained_feed_account != selected.retained_feed
        || feed_bundle_id != selected.candidate_bundle_digest
        || input.market_binding_account != selected.market_binding
        || input.market_binding.market != selected.market
        || root.phase() != SettlementRootPhaseV1::Settling
        || root.virtual_cash_direction() != VirtualCashDirectionV1::Merge
        || root.cash_pot_state() != SettlementRootChildStateV1::Live
        || counts.admitted_receipts != counts.expected_receipts
        || counts.live_receipts != counts.admitted_receipts
        || counts.completed_owner_finalizations == 0
        || input.settlement_cash_pot_account != root.settlement_cash_pot()
        || input.settlement_cash_pot.semantic.expectation != root.cash_pot_expectation()?
        || input.settlement_cash_pot.stored_bump != root.cash_pot_bump()
        || input.settlement_cash_pot.flags != 0
        || feed.epoch != selected.epoch
        || feed.market != selected.market
        || feed.node != selected.source_admission_node
        || feed.order_set != selected.order_set
        || feed.settlement_candidate_id != selected.settlement_candidate_id
        || feed.settlement_witness_digest != selected.settlement_witness_digest
        || feed.epoch_generation != selected.epoch_generation
        || feed.slice_count != selected.slice_count
        || feed.relation_policy_id != input.market_binding.relation_policy_id
        || feed.price_measure_policy_v1_id != input.market_binding.price_measure_policy_v1_id
        || feed.native_claim_basis_id != input.market_binding.native_claim_basis_id
        || feed.price_scale != input.market_binding.price_scale
        || feed.outcome_count != input.market_binding.outcome_count
        || root.outcome_count() != feed.outcome_count
        || root.market_instance_v2_id() != input.market_binding.market_instance_v2_id
        || input.collateral.market().market.bytes()
            != input.market_binding.market_instance_v2_id.bytes()
        || input.receipt.transition() != SettlementReceiptTransitionCommitmentV5::None
        || expected_receipt_evidence != input.receipt_evidence
        || input.receipt_evidence.receipt().bytes() != receipt_account.bytes()
        || receipt_semantic.epoch.bytes() != selected.epoch.bytes()
        || receipt_semantic.market.bytes() != selected.market.bytes()
        || receipt_semantic.candidate.bytes() != selected.settlement_candidate_id.bytes()
        || receipt_semantic.leg_kind != RECEIPT_LEG_MERGE
        || receipt_semantic.accounted_end_mask != RECEIPT_FLAG_SELL_CONSUMED
        || receipt_semantic.expected_end_mask() != RECEIPT_FLAG_SELL_CONSUMED
        || receipt_semantic.delivered_end_mask() != RECEIPT_FLAG_SELL_CONSUMED
        || receipt_semantic.consumed_flags != RECEIPT_FLAG_SELL_CONSUMED
        || receipt_semantic.settled_quantity != receipt_semantic.quantity
        || receipt_semantic.quantity == 0
        || receipt_semantic.slice_index >= feed.slice_count
        || receipt_semantic.sequence != u64::from(receipt_semantic.slice_index) + 1
        || receipt_semantic.buy_order_id != LayoutHash32::ZERO
        || receipt_semantic.sell_order_id == LayoutHash32::ZERO
        || receipt_semantic.price
            != read_feed_u64(tail.prices_le(), usize::from(receipt_semantic.outcome))?
        || receipt_semantic.consideration_price_units
            != u128::from(receipt_semantic.quantity)
                .checked_mul(u128::from(receipt_semantic.price))
                .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    require_merge_payment_account_partition_v5(&input)?;
    let target_slice = read_feed_slice_v3(tail.slices_le(), receipt_semantic.slice_index)?;
    if target_slice.route()? != SettlementReceiptRouteV2::SellToMerge
        || target_slice.outcome != receipt_semantic.outcome
        || target_slice.quantity != receipt_semantic.quantity
    {
        return Err(SettlementAdapterErrorV1::ReceiptLatchMismatch);
    }
    let terminal = input.seller.owner_row.envelope().terminal_projection()?;
    let finalized = terminal.semantic();
    let expectation = finalized.expectation();
    let owner = Id32::new(expectation.owner())?;
    let order = bind_merge_payment_order_v5(
        feed,
        receipt_semantic,
        target_slice.sell_index,
        owner,
        input.seller.order_page_body,
    )?;
    let order_bit = 1u64
        .checked_shl(u32::from(target_slice.sell_index))
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    if expectation.market() != selected.market.bytes()
        || expectation.epoch() != selected.epoch.bytes()
        || expectation.candidate() != selected.settlement_candidate_id.bytes()
        || expectation.owner_order_set_digest() != root.owner_order_set_digest().bytes()
        || expectation.expected_sell_order_mask() & order_bit == 0
        || input.seller.owner_row.seed().epoch() != &selected.epoch.bytes()
        || input.seller.owner_row.seed().settlement_candidate()
            != &selected.settlement_candidate_id.bytes()
        || input.seller.owner_row.seed().owner() != &owner.bytes()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let reservation_prestate = ReservationAccountV9::decode(input.seller.reservation_body)?;
    let mut reservation = reservation_prestate.body();
    let reservation_plan = ReservationPlan::for_order(
        &order.slot,
        feed.outcome_count,
        feed.price_scale,
        reservation.max_fee_atoms,
    )?;
    let expected_reservation = canonical_reservation_id_v9(
        LayoutHash32(selected.market.bytes()),
        LayoutHash32(selected.epoch.bytes()),
        LayoutHash32(owner.bytes()),
        order.position_generation,
        order.slot.order_id(),
    );
    let next_paid_units = reservation
        .paid_units
        .checked_add(receipt_semantic.quantity)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    if input.seller.reservation_account.is_zero()
        || reservation.reservation != expected_reservation
        || reservation.market.bytes() != selected.market.bytes()
        || reservation.epoch.bytes() != selected.epoch.bytes()
        || reservation.owner.bytes() != owner.bytes()
        || reservation.order_id != order.slot.order_id()
        || reservation.order_generation != order.slot.generation()
        || reservation.position_generation != order.position_generation
        || reservation.page_index != order.page_index
        || reservation.outcome_count != feed.outcome_count
        || reservation.side != 1
        || reservation.order_kind != order.slot.kind()
        || reservation.state != RESERVATION_STATE_ENTITLED
        || reservation.initial_cash_atoms != reservation_plan.cash_atoms
        || reservation.max_fee_atoms != reservation_plan.max_fee_atoms
        || reservation.initial_internal != reservation_plan.internal
        || reservation.remaining_cash_atoms != 0
        || reservation.paid_units >= reservation.consumed_units
        || next_paid_units > reservation.consumed_units
        || reservation.consumed_units > reservation.entitled_units
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    reservation.paid_units = next_paid_units;
    let completes_order = reservation.paid_units == reservation.entitled_units;
    if completes_order {
        if reservation.consumed_units != reservation.entitled_units
            || !reservation.remaining_is_zero()
        {
            return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
        }
        reservation.state = RESERVATION_STATE_CONSUMED;
    }
    let reservation_poststate =
        ReservationAccountV9::new(reservation, reservation_prestate.rent())?;
    let reservation_prestate_data_id = Id32::new(reservation_prestate.data_id()?.bytes())?;
    let reservation_poststate_data_id = Id32::new(reservation_poststate.data_id()?.bytes())?;
    let mut reservation_poststate_body = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
    reservation_poststate.encode(&mut reservation_poststate_body)?;

    let position_body = PositionAccountV3::decode(input.seller.position.encoded_body)?;
    project_canonical_general_settlement_position_v3(
        position_body,
        owner,
        order.position_generation,
        selected.market,
        AdapterPositionMarketBindingV3 {
            market_instance_id: retirement_identity(input.market_binding.market_instance_v2_id)?,
            outcome_count: feed.outcome_count,
            realm_id: retirement_identity(Id32::new(input.collateral.realm_bound().realm().realm.bytes())?)?,
            collateral_policy_id: retirement_identity(Id32::new(input.collateral.policy_id().bytes())?)?,
            collateral_release_id: retirement_identity(Id32::new(input.collateral.release().id()?.bytes())?)?,
        },
    )?;
    let position_semantic_id =
        Id32::new(position_body.semantic_id(&PositionBodySha256V3)?.bytes())?;
    let position = AuthenticatedPositionV3 {
        account: input.seller.position.account.bytes(),
        general_market_runtime: selected.market.bytes(),
        semantic: position_body,
        semantic_id: position_semantic_id.bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: false,
    };
    position.validate()?;
    let position_poststate = position.unchanged_poststate()?;
    let replay_prestate = project_general_position_replay_prestate_v1(
        input.seller.replay_account,
        input.seller.replay_bump,
        input.seller.replay_next_sequence,
        input.seller.replay_body,
        position,
        &PositionBodySha256V3,
    )?;
    let finalized_owner_row_data_id = input.seller.owner_row.data_id();
    let current_pot_poststate_data_id =
        clutch_general_v2_contract::settlement_cash_pot_poststate_data_id_v1(
            input.settlement_cash_pot.semantic,
            &CanonicalSha256,
        )?;
    let (source, source_data_id, owner_pot_poststate_data_id, stable_evidence_id) =
        bind_merge_payment_finalization_evidence_v5(
            input.settlement_root_account,
            root,
            owner,
            input.seller.owner_row.account(),
            finalized_owner_row_data_id,
            input.seller.position.account,
            input.settlement_cash_pot_account,
            current_pot_poststate_data_id,
            input.payload.stable_zero_fee_finalization_evidence_id,
            input.fee_finalization,
            replay_prestate,
            finalized,
            position_body,
            position_semantic_id,
        )?;
    let prior_kind = replay_prestate.extension().last_kind();
    if prior_kind == Some(GeneralReplayTransitionKindV1::FinalizeMergeReceiptPayment) {
        verify_general_replay_last_transition_v1(
            replay_prestate,
            position_semantic_id,
            GeneralReplayTransitionKindV1::FinalizeMergeReceiptPayment,
            replay_prestate.extension().last_transition_id(),
            stable_evidence_id,
            &PositionBodySha256V3,
        )?;
    } else if prior_kind != Some(GeneralReplayTransitionKindV1::FinalizeOwnerSettlement) {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let payment_transition_id =
        Id32::new(input.receipt_evidence.payment_transition_id().bytes())?;
    let replay = project_general_replay_transition_v1(
        replay_prestate,
        position_poststate,
        GeneralReplayTransitionKindV1::FinalizeMergeReceiptPayment,
        payment_transition_id,
        stable_evidence_id,
        &PositionBodySha256V3,
    )?;
    if replay.position_prestate_semantic_id() != position_semantic_id
        || replay.position_poststate_semantic_id() != position_semantic_id
        || replay.transition_id() != payment_transition_id
        || replay.transition_evidence_id() != stable_evidence_id
        || replay.kind() != GeneralReplayTransitionKindV1::FinalizeMergeReceiptPayment
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    receipt_semantic.consumed_flags |= RECEIPT_FLAG_SLICE_EXHAUSTED;
    let receipt_poststate = SettlementReceiptAccountV5::new(
        receipt_semantic,
        SettlementReceiptTransitionCommitmentV5::None,
        input.receipt.rent(),
    )?;
    let receipt_poststate_body = receipt_poststate.encode_exact()?;
    let receipt_poststate_evidence = receipt_poststate.evidence(receipt_pda)?;
    let settlement_root_poststate = root.complete_merge_payment()?;
    let reservation_semantic_id = Id32::new(reservation.reservation.bytes())?;
    Ok(FinalizeMergeReceiptPaymentPlanV5 {
        settlement_root_account: input.settlement_root_account,
        settlement_root_poststate,
        retained_feed_account: input.retained_feed_account,
        receipt_account,
        receipt_seed: SettlementReceiptSeedTupleV5::new(
            selected.epoch,
            selected.settlement_candidate_id,
            receipt_semantic.slice_index,
        )?,
        receipt_prestate_evidence: input.receipt_evidence,
        payment_transition_id,
        receipt_poststate,
        receipt_poststate_evidence,
        receipt_poststate_body,
        owner_settlement_account: input.seller.owner_row.account(),
        finalized_owner_row_data_id,
        owner_settlement_body: *input.seller.owner_row.exact_body(),
        settlement_cash_pot_account: input.settlement_cash_pot_account,
        settlement_cash_pot_poststate_data_id: owner_pot_poststate_data_id,
        finalization_source: source,
        finalization_source_data_id: source_data_id,
        stable_owner_finalization_evidence_id: stable_evidence_id,
        order_page_account: input.seller.order_page_account,
        order_page_seed: GeneralOrderPageSeedTupleV5::new(selected.epoch, order.page_index)?,
        order_slot_index: order.slot_index,
        position_generation: order.position_generation,
        reservation_account: input.seller.reservation_account,
        reservation_semantic_id,
        reservation_seed: GeneralReservationSeedTupleV9::new(reservation_semantic_id)?,
        reservation_prestate_data_id,
        reservation_poststate_data_id,
        reservation_poststate_body,
        position_account: input.seller.position.account,
        position_semantic_id,
        replay,
        completes_order,
    })
}

fn require_merge_payment_account_partition_v5(
    input: &FinalizeMergeReceiptPaymentInputV5<'_>,
) -> Result<(), SettlementAdapterErrorV1> {
    let mut accounts = [Id32::ZERO; 11];
    let mut len = 0usize;
    for account in [
        input.settlement_root_account,
        input.retained_feed_account,
        input.payload.receipt,
        input.market_binding_account,
        input.settlement_cash_pot_account,
        input.seller.owner_row.account(),
        input.seller.reservation_account,
        input.seller.position.account,
        input.seller.replay_account,
    ] {
        insert_materialization_account_v5(&mut accounts, &mut len, account)?;
    }
    if input.seller.order_page_account.is_zero()
        || accounts[..len].contains(&input.seller.order_page_account)
        || input.seller.owner_row.program_owner().is_zero()
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    if let Some(fee) = input.fee_finalization {
        insert_materialization_account_v5(&mut accounts, &mut len, fee.account)?;
        if fee.account == input.seller.order_page_account {
            return Err(SettlementAdapterErrorV1::BindingMismatch);
        }
    }
    Ok(())
}

fn bind_merge_payment_order_v5(
    feed: CandidateFeedHeaderV2,
    receipt: SettlementReceiptAccountV4,
    order_index: u8,
    owner: Id32,
    order_page_body: &[u8],
) -> Result<MergePaymentOrderBindingV5, SettlementAdapterErrorV1> {
    if order_index >= feed.order_count {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let page = verify_page_v5(order_page_body)?;
    if page.market.bytes() != feed.market.bytes()
        || page.epoch.bytes() != feed.epoch.bytes()
        || page.order_set.bytes() != feed.order_set.bytes()
        || page.frozen != 1
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut cursor = OrderSlotCursorV5::new(order_page_body)?;
    let mut matched = None;
    while let Some(next) = cursor.next_slot() {
        let slot = next?;
        if slot.slot.is_live() && slot.slot.order_id() == receipt.sell_order_id {
            if matched.is_some() {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            matched = Some(slot);
        }
    }
    let matched = matched.ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let side = match matched.slot {
        OrderSlot::Single(record) => record.side,
        OrderSlot::Portfolio(record) => record.side,
        OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            return Err(SettlementAdapterErrorV1::BindingMismatch)
        }
    };
    if side != 1 || matched.slot.owner().bytes() != owner.bytes() {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    Ok(MergePaymentOrderBindingV5 {
        slot: matched.slot,
        page_index: page.page_index,
        slot_index: matched.slot_index,
        position_generation: matched.position_generation,
    })
}

#[allow(clippy::too_many_arguments)]
fn bind_merge_payment_finalization_evidence_v5(
    settlement_root_account: Id32,
    root: &SettlementRootV1AccountV1,
    owner: Id32,
    owner_settlement_account: Id32,
    finalized_owner_row_data_id: Id32,
    position_account: Id32,
    settlement_cash_pot_account: Id32,
    current_pot_poststate_data_id: Id32,
    zero_fee_stable_witness: Id32,
    fee_finalization: Option<MergePaymentFeeFinalizationInputV4<'_>>,
    replay_prestate: GeneralPositionReplayPrestateV1,
    finalized: clutch_owner_settlement::OwnerSettlementTerminalProjectionV4,
    position: PositionAccountV3,
    position_semantic_id: Id32,
) -> Result<
    (
        MergePaymentFinalizationSourceV5,
        Option<Id32>,
        Option<Id32>,
        Id32,
    ),
    SettlementAdapterErrorV1,
> {
    let candidate = root.settlement_candidate_id();
    match (root.fee_record().is_zero(), fee_finalization) {
        (false, Some(fee_input)) => {
            if !zero_fee_stable_witness.is_zero() {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            let outer = OwnerFeeFinalizationV4AccountV1::decode(fee_input.exact_body)?;
            let terminal = outer.terminal_projection(FeeId(fee_input.account.bytes()))?;
            let source_data_id = owner_fee_finalization_account_data_id_v4(
                fee_input.exact_body,
                &CanonicalSha256,
            )?;
            let pot_poststate_data_id =
                Id32::new(terminal.settlement_cash_pot_poststate_data_id.0)?;
            if terminal.outcome != OwnerFeeFinalizationOutcomeV2::Settled
                || terminal.carry_account.0 != fee_input.account.bytes()
                || terminal.fee_record.0 != root.fee_record().bytes()
                || terminal.settlement_candidate.0 != candidate.bytes()
                || terminal.owner.0 != owner.bytes()
                || terminal.owner_settlement_account.0 != owner_settlement_account.bytes()
                || terminal.owner_settlement_final_data_id.0
                    != finalized_owner_row_data_id.bytes()
                || terminal.position.0 != position_account.bytes()
                || terminal.position_poststate_semantic_id.0 != position_semantic_id.bytes()
                || terminal.settlement_cash_pot.0 != settlement_cash_pot_account.bytes()
                || terminal.authorized_fee_atoms != finalized.expectation().selected_fee_atoms()
            {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            let stable = derive_merge_payment_owner_finalization_evidence_v5(
                settlement_root_account,
                candidate,
                owner,
                finalized_owner_row_data_id,
                pot_poststate_data_id,
                MergePaymentFinalizationSourceV5::FeeFinalizationV4,
                source_data_id,
            )?;
            if replay_prestate.extension().last_kind()
                == Some(GeneralReplayTransitionKindV1::FinalizeOwnerSettlement)
                && (terminal.position_poststate_semantic_id.0 != position_semantic_id.bytes()
                    || terminal.replay_poststate_semantic_id.0
                        != replay_prestate.replay_semantic_id().bytes()
                    || terminal.replay_next_sequence != replay_prestate.next_sequence())
            {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            Ok((
                MergePaymentFinalizationSourceV5::FeeFinalizationV4,
                Some(source_data_id),
                Some(pot_poststate_data_id),
                stable,
            ))
        }
        (true, None) => {
            if zero_fee_stable_witness.is_zero() {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            if replay_prestate.extension().last_kind()
                == Some(GeneralReplayTransitionKindV1::FinalizeOwnerSettlement)
            {
                let zero_fee_action38_evidence =
                    derive_zero_fee_owner_finalization_evidence_fields_v5(
                        settlement_root_account,
                        candidate,
                        owner,
                        finalized_owner_row_data_id,
                        current_pot_poststate_data_id,
                    )?;
                let derived_stable = derive_merge_payment_owner_finalization_evidence_v5(
                    settlement_root_account,
                    candidate,
                    owner,
                    finalized_owner_row_data_id,
                    current_pot_poststate_data_id,
                    MergePaymentFinalizationSourceV5::ZeroFeeReplay,
                    zero_fee_action38_evidence,
                )?;
                if zero_fee_stable_witness != derived_stable {
                    return Err(SettlementAdapterErrorV1::BindingMismatch);
                }
                let finalized_accumulator = OwnerSettlementAccumulatorV4::decode_body(
                    clutch_owner_settlement::OWNER_SETTLEMENT_OUTER_TAG_V4,
                    clutch_owner_settlement::OWNER_SETTLEMENT_OUTER_VERSION_V4,
                    finalized.finalized_body(),
                )?;
                let prior_position = recover_owner_cash_position_prestate_v4(
                    finalized_accumulator,
                    position,
                )?;
                let prior_position_semantic_id = Id32::new(
                    prior_position
                        .semantic_id(&PositionBodySha256V3)?
                        .bytes(),
                )?;
                verify_general_replay_last_transition_v1(
                    replay_prestate,
                    prior_position_semantic_id,
                    GeneralReplayTransitionKindV1::FinalizeOwnerSettlement,
                    finalized_owner_row_data_id,
                    zero_fee_action38_evidence,
                    &PositionBodySha256V3,
                )?;
                return Ok((
                    MergePaymentFinalizationSourceV5::ZeroFeeReplay,
                    Some(zero_fee_action38_evidence),
                    Some(current_pot_poststate_data_id),
                    zero_fee_stable_witness,
                ));
            }
            Ok((
                MergePaymentFinalizationSourceV5::ZeroFeeReplay,
                None,
                None,
                zero_fee_stable_witness,
            ))
        }
        _ => Err(SettlementAdapterErrorV1::BindingMismatch),
    }
}

/// Exact hostile native-lamport balances for one Reservation V9 close.
///
/// Recipient identities are deliberately absent: the persisted Reservation
/// rent owner and traversal-authenticated MarketBinding own them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnfilledReservationRentBalancesV1 {
    /// Lamports currently held by the Reservation account being deleted.
    pub reservation_lamports: u64,
    /// Current lamports of the persisted refundable-principal payer.
    pub payer_lamports: u64,
    /// Current lamports of the immutable MarketBinding neutral sink.
    pub neutral_sink_lamports: u64,
}

/// Complete pure input for action 41.
///
/// The live adapter must reconstruct `traversal` from the authenticated
/// retained Feed and complete frozen V5 page set and authenticate every
/// supplied account owner, PDA, bump, and writable role.
#[derive(Clone, Copy, Debug)]
pub struct ReleaseUnfilledReservationInputV1<'a> {
    /// Strict `epoch || settlement_root` selector.
    pub payload: ReleaseUnfilledReservationPayloadV1,
    /// Exact counted SettlementRoot account identity.
    pub settlement_root_account: Id32,
    /// Structurally decoded writable SettlementRoot prestate.
    pub settlement_root: &'a SettlementRootV1AccountV1,
    /// Private exhaustive selected Feed/V5-page traversal.
    pub traversal: &'a SettlementTraversalProjectionV4,
    /// Exact frozen V5 page account containing the selected zero-fill order.
    pub order_page_account: Id32,
    /// Exact hostile frozen V5 page body.
    pub order_page_body: &'a [u8],
    /// Canonical rent-owned Reservation V9 PDA.
    pub reservation_account: Id32,
    /// Exact hostile 666-byte active Reservation V9 body.
    pub reservation_body: &'a [u8],
    /// Exact observed Reservation, payer, and sink balances.
    pub rent_balances: UnfilledReservationRentBalancesV1,
    /// Canonical writable Position V3 account and exact body.
    pub position: PositionAccountInputV3<'a>,
    /// Purpose-owned General Replay V3 account.
    pub replay_account: Id32,
    /// Canonical Replay PDA bump.
    pub replay_bump: u8,
    /// Exact ordinal required by the hostile Replay prestate.
    pub replay_next_sequence: u64,
    /// Exact hostile purpose Replay V3 body before action 41.
    pub replay_body: &'a [u8],
}

/// Exact V9 terminal release and deletion facts inside action 41.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnfilledReservationClosePlanV9 {
    account: Id32,
    semantic_id: Id32,
    seed: GeneralReservationSeedTupleV9,
    prestate_data_id: Id32,
    terminal_data_id: Id32,
    terminal_body: [u8; RESERVATION_ACCOUNT_BYTES_V9],
    release_generation: u64,
    released_reserved_cash_atoms: u64,
    released_internal: [u64; MAX_OUTCOMES],
    balance_before: u64,
    payer: Id32,
    payer_refund_lamports: u64,
    payer_balance_after: u64,
    neutral_sink: Id32,
    neutral_sink_credit_lamports: u64,
    neutral_sink_balance_after: u64,
}

impl UnfilledReservationClosePlanV9 {
    /// Canonical Reservation V9 PDA to delete.
    pub const fn account(&self) -> Id32 { self.account }
    /// Fresh V9 semantic identity retained by the deleted account.
    pub const fn semantic_id(&self) -> Id32 { self.semantic_id }
    /// Contract-owned V9 PDA seed tuple.
    pub const fn seed(&self) -> GeneralReservationSeedTupleV9 { self.seed }
    /// Exact active V9 prestate data ID.
    pub const fn prestate_data_id(&self) -> Id32 { self.prestate_data_id }
    /// Exact released V9 terminal data ID committed before deletion.
    pub const fn terminal_data_id(&self) -> Id32 { self.terminal_data_id }
    /// Canonical released V9 body staged in the close rollback domain.
    pub const fn terminal_body(&self) -> &[u8; RESERVATION_ACCOUNT_BYTES_V9] {
        &self.terminal_body
    }
    /// Sole canonical terminal generation: checked `order_generation + 1`.
    pub const fn release_generation(&self) -> u64 { self.release_generation }
    /// Reserved Position cash returned by deleting this zero-fill child.
    pub const fn released_reserved_cash_atoms(&self) -> u64 {
        self.released_reserved_cash_atoms
    }
    /// Position-owned native Eggs returned by deleting this child.
    pub const fn released_internal(&self) -> [u64; MAX_OUTCOMES] { self.released_internal }
    /// Exact observed Reservation lamports before deletion.
    pub const fn balance_before(&self) -> u64 { self.balance_before }
    /// Reservation lamports after the atomic close.
    pub const fn balance_after(&self) -> u64 { 0 }
    /// Persisted refundable-principal payer.
    pub const fn payer(&self) -> Id32 { self.payer }
    /// Exact persisted principal refunded to `payer`.
    pub const fn payer_refund_lamports(&self) -> u64 { self.payer_refund_lamports }
    /// Payer balance after the exact refund.
    pub const fn payer_balance_after(&self) -> u64 { self.payer_balance_after }
    /// Immutable MarketBinding neutral sink.
    pub const fn neutral_sink(&self) -> Id32 { self.neutral_sink }
    /// Donation floor plus unsolicited surplus routed to the neutral sink.
    pub const fn neutral_sink_credit_lamports(&self) -> u64 {
        self.neutral_sink_credit_lamports
    }
    /// Neutral-sink balance after the exact credit.
    pub const fn neutral_sink_balance_after(&self) -> u64 {
        self.neutral_sink_balance_after
    }
}

/// One indivisible action-41 root/Reservation/Position/Replay/rent bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseUnfilledReservationPlanV1 {
    settlement_root_account: Id32,
    settlement_root_poststate: SettlementRootV1AccountV1,
    retained_feed_account: Id32,
    order_page_account: Id32,
    order_page_index: u16,
    order_slot_index: u8,
    order_index: u8,
    position_generation: u64,
    transition_id: Id32,
    transition_evidence_id: Id32,
    reservation: UnfilledReservationClosePlanV9,
    position_account: Id32,
    position_prestate_semantic_id: Id32,
    position_poststate_semantic_id: Id32,
    position_poststate_body: [u8; POSITION_V3_BYTES],
    position: PositionSettlementPoststateV3,
    replay: GeneralReplayTransitionPlanV1,
}

impl ReleaseUnfilledReservationPlanV1 {
    /// Counted SettlementRoot account advanced once.
    pub const fn settlement_root_account(&self) -> Id32 { self.settlement_root_account }
    /// Exact root successor, potentially entering Settling on the last gap.
    pub const fn settlement_root_poststate(&self) -> &SettlementRootV1AccountV1 {
        &self.settlement_root_poststate
    }
    /// Exact retained sealed Feed account behind the zero-fill proof.
    pub const fn retained_feed_account(&self) -> Id32 { self.retained_feed_account }
    /// Exact frozen V5 page account.
    pub const fn order_page_account(&self) -> Id32 { self.order_page_account }
    /// Canonical page index.
    pub const fn order_page_index(&self) -> u16 { self.order_page_index }
    /// Canonical sparse slot index within the page.
    pub const fn order_slot_index(&self) -> u8 { self.order_slot_index }
    /// Dense order index inside the retained Feed/order projection.
    pub const fn order_index(&self) -> u8 { self.order_index }
    /// Immutable Position generation frozen by OrderPage V5.
    pub const fn position_generation(&self) -> u64 { self.position_generation }
    /// Stable action-41 identity derived without caller data.
    pub const fn transition_id(&self) -> Id32 { self.transition_id }
    /// Exact atomic-bundle digest committed into GEN1 Replay.
    pub const fn transition_evidence_id(&self) -> Id32 { self.transition_evidence_id }
    /// Exact terminal release, rent split, and Reservation deletion facts.
    pub const fn reservation(&self) -> &UnfilledReservationClosePlanV9 { &self.reservation }
    /// Canonical Position V3 PDA to write.
    pub const fn position_account(&self) -> Id32 { self.position_account }
    /// Exact Position semantic ID before release.
    pub const fn position_prestate_semantic_id(&self) -> Id32 {
        self.position_prestate_semantic_id
    }
    /// Exact Position semantic ID after value return and child decrement.
    pub const fn position_poststate_semantic_id(&self) -> Id32 {
        self.position_poststate_semantic_id
    }
    /// Exact canonical Position V3 successor body.
    pub const fn position_poststate_body(&self) -> &[u8; POSITION_V3_BYTES] {
        &self.position_poststate_body
    }
    /// Typed Position successor joined to Replay.
    pub const fn position(&self) -> PositionSettlementPoststateV3 { self.position }
    /// Exact purpose-owned GEN1 Replay successor.
    pub const fn replay(&self) -> &GeneralReplayTransitionPlanV1 { &self.replay }
}

/// Prepare one selected zero-fill Reservation release and co-close.
///
/// The exhaustive traversal, rather than a caller flag or order index, proves
/// that the order has no settlement end. Position value return, authoritative
/// child decrement, root count, Replay successor, terminal V9 transcript,
/// principal refund, neutral donation, and account deletion are returned as
/// one rollback-domain bundle with one evidence digest.
pub fn prepare_release_unfilled_reservation_v1(
    input: ReleaseUnfilledReservationInputV1<'_>,
) -> Result<ReleaseUnfilledReservationPlanV1, SettlementAdapterErrorV1> {
    require_root_traversal_binding_v4(
        input.settlement_root_account,
        input.settlement_root,
        input.traversal,
    )?;
    if input.payload.epoch != input.settlement_root.epoch()
        || input.payload.settlement_root != input.settlement_root_account
        || input.settlement_root.phase() != SettlementRootPhaseV1::Materializing
        || input.order_page_account.is_zero()
        || input.reservation_account.is_zero()
        || input.position.account.is_zero()
        || input.replay_account.is_zero()
        || input.order_page_account == input.settlement_root_account
        || input.order_page_account == input.traversal.selected_feed_account
        || input.reservation_account == input.settlement_root_account
        || input.reservation_account == input.traversal.selected_feed_account
        || input.reservation_account == input.order_page_account
        || input.reservation_account == input.position.account
        || input.reservation_account == input.replay_account
        || input.position.account == input.replay_account
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    let page = verify_page_v5(input.order_page_body)?;
    let base = input.traversal.order_projection.base();
    let feed = input.traversal.feed;
    if page.market.bytes() != feed.market.bytes()
        || page.epoch.bytes() != feed.epoch.bytes()
        || page.order_set.bytes() != feed.order_set.bytes()
        || page.frozen != 1
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    let reservation_v9 = ReservationAccountV9::decode(input.reservation_body)?;
    let reservation = reservation_v9.body();
    let reservation_semantic_id = Id32::new(reservation.reservation.bytes())?;
    let mut order_index = None;
    let mut dense = 0u8;
    while dense < feed.order_count {
        let frozen = base
            .order_membership(dense)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let position_generation = input
            .traversal
            .order_projection
            .position_generation(dense)
            .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
        let expected_reservation = canonical_reservation_id_v9(
            LayoutHash32(feed.market.bytes()),
            LayoutHash32(feed.epoch.bytes()),
            LayoutHash32(frozen.owner().bytes()),
            position_generation,
            LayoutHash32(frozen.order_id().bytes()),
        );
        if expected_reservation.bytes() == reservation.reservation.bytes() {
            if order_index.is_some() {
                return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
            }
            order_index = Some(dense);
        }
        dense = dense
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    }
    let order_index = order_index.ok_or(SettlementAdapterErrorV1::ReservationSetMismatch)?;
    if input.traversal.settlement_membership(order_index).is_some()
        || input.traversal.first_slice(order_index).is_some()
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }
    let frozen = base
        .order_membership(order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let expected_page_account = input
        .traversal
        .order_projection
        .order_page_account(order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let expected_page_index = input
        .traversal
        .order_projection
        .order_page_index(order_index)
        .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    let expected_position_generation = input
        .traversal
        .order_projection
        .position_generation(order_index)
        .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
    if input.order_page_account != expected_page_account || page.page_index != expected_page_index {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    let mut verified_slot = None;
    let mut cursor = OrderSlotCursorV5::new(input.order_page_body)?;
    while let Some(step) = cursor.next_slot() {
        let candidate = step?;
        if candidate.slot.order_id().bytes() == frozen.order_id().bytes() {
            if verified_slot.is_some() {
                return Err(SettlementAdapterErrorV1::BindingMismatch);
            }
            verified_slot = Some(candidate);
        }
    }
    let verified_slot = verified_slot.ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
    if verified_slot.slot != *frozen.slot()
        || verified_slot.position_generation != expected_position_generation
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    let expected_side = match base.book().orders[usize::from(order_index)].side {
        Side::Buy => 0,
        Side::Sell => 1,
    };
    let expected_kind = match verified_slot.slot {
        OrderSlot::Single(_) => 1,
        OrderSlot::Portfolio(_) => 2,
        OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            return Err(SettlementAdapterErrorV1::BindingMismatch)
        }
    };
    let reservation_plan = ReservationPlan::for_order(
        &verified_slot.slot,
        feed.outcome_count,
        feed.price_scale,
        reservation.max_fee_atoms,
    )?;
    if reservation.market.bytes() != feed.market.bytes()
        || reservation.epoch.bytes() != feed.epoch.bytes()
        || reservation.owner.bytes() != frozen.owner().bytes()
        || reservation.order_id.bytes() != frozen.order_id().bytes()
        || reservation.position_generation != expected_position_generation
        || reservation.order_generation != frozen.generation()
        || reservation.page_index != expected_page_index
        || reservation.price_grid.bytes() != base.price_grid_id().bytes()
        || reservation.terms.bytes() != input.traversal.terms.bytes()
        || reservation.policy.bytes() != input.traversal.reservation_policy.bytes()
        || reservation.outcome_count != feed.outcome_count
        || reservation.side != expected_side
        || reservation.order_kind != expected_kind
        || reservation.state != RESERVATION_STATE_ACTIVE
        || reservation.entitled_units != 0
        || reservation.consumed_units != 0
        || reservation.paid_units != 0
        || reservation.release_generation != 0
        || reservation.initial_cash_atoms != reservation_plan.cash_atoms
        || reservation.max_fee_atoms != reservation_plan.max_fee_atoms
        || reservation.initial_internal != reservation_plan.internal
        || reservation.remaining_cash_atoms != reservation.initial_cash_atoms
        || reservation.remaining_internal != reservation.initial_internal
        || reservation.fee_debited_atoms != 0
        || reservation.fee_carry_numerator != 0
    {
        return Err(SettlementAdapterErrorV1::ReservationSetMismatch);
    }

    let neutral_sink = base.market_binding().neutral_sink;
    let rent = reservation_v9.rent();
    let payer = Id32::new(rent.payer.bytes())?;
    let required_balance = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let protected_accounts = [
        input.settlement_root_account,
        input.traversal.selected_feed_account,
        input.order_page_account,
        input.reservation_account,
        input.position.account,
        input.replay_account,
    ];
    if payer == neutral_sink
        || protected_accounts.contains(&payer)
        || protected_accounts.contains(&neutral_sink)
        || input.rent_balances.reservation_lamports < required_balance
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let neutral_sink_credit_lamports = input
        .rent_balances
        .reservation_lamports
        .checked_sub(rent.refundable_principal)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let payer_balance_after = input
        .rent_balances
        .payer_lamports
        .checked_add(rent.refundable_principal)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let neutral_sink_balance_after = input
        .rent_balances
        .neutral_sink_lamports
        .checked_add(neutral_sink_credit_lamports)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;

    let position_body = PositionAccountV3::decode(input.position.encoded_body)?;
    project_canonical_general_settlement_position_v3(
        position_body,
        frozen.owner(),
        expected_position_generation,
        input.settlement_root.market(),
        input.traversal.position_market_binding,
    )?;
    let position_prestate_semantic_id =
        Id32::new(position_body.semantic_id(&PositionBodySha256V3)?.bytes())?;
    let position = AuthenticatedPositionV3 {
        account: input.position.account.bytes(),
        general_market_runtime: input.settlement_root.market().bytes(),
        semantic: position_body,
        semantic_id: position_prestate_semantic_id.bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    position.validate_writable()?;
    let position_poststate = position.release_reservation_poststate(
        reservation.remaining_cash_atoms,
        reservation.remaining_internal,
    )?;
    let position_poststate_semantic_id = Id32::new(
        position_poststate
            .semantic
            .semantic_id(&PositionBodySha256V3)?
            .bytes(),
    )?;
    let position_poststate_body = position_poststate.semantic.encode()?;

    let release_generation = reservation
        .order_generation
        .checked_add(1)
        .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?;
    let released_reservation = reservation_v9.released(release_generation)?;
    let reservation_prestate_data_id = Id32::new(reservation_v9.data_id()?.bytes())?;
    let reservation_terminal_data_id = Id32::new(released_reservation.data_id()?.bytes())?;
    let mut reservation_terminal_body = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
    released_reservation.encode(&mut reservation_terminal_body)?;
    let reservation_seed = GeneralReservationSeedTupleV9::new(reservation_semantic_id)?;
    let settlement_root_poststate = input.settlement_root.release_unfilled_reservation()?;

    let transition_id = derive_unfilled_reservation_release_transition_id_v1(
        input.settlement_root_account,
        input.settlement_root.settlement_candidate_id(),
        input.reservation_account,
        reservation_semantic_id,
        frozen.owner(),
        frozen.order_id(),
        order_index,
        expected_page_index,
        verified_slot.slot_index,
        expected_position_generation,
        reservation.order_generation,
        release_generation,
    )?;
    let root_prestate_data_id = settlement_root_release_data_id_v1(
        input.settlement_root,
        input.settlement_root_account,
    )?;
    let root_poststate_data_id = settlement_root_release_data_id_v1(
        &settlement_root_poststate,
        input.settlement_root_account,
    )?;
    let transition_evidence_id = derive_unfilled_reservation_release_evidence_id_v1(
        transition_id,
        input.settlement_root_account,
        root_prestate_data_id,
        root_poststate_data_id,
        input.traversal.selected_feed_account,
        input.order_page_account,
        input.reservation_account,
        reservation_prestate_data_id,
        reservation_terminal_data_id,
        input.position.account,
        position_prestate_semantic_id,
        position_poststate_semantic_id,
        input.replay_account,
        input.replay_next_sequence,
        payer,
        rent.refundable_principal,
        input.rent_balances.payer_lamports,
        payer_balance_after,
        neutral_sink,
        neutral_sink_credit_lamports,
        input.rent_balances.neutral_sink_lamports,
        neutral_sink_balance_after,
        input.rent_balances.reservation_lamports,
    )?;

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
        position_poststate,
        GeneralReplayTransitionKindV1::ReleaseUnfilledReservation,
        transition_id,
        transition_evidence_id,
        &PositionBodySha256V3,
    )?;
    if replay.position_prestate_semantic_id() != position_prestate_semantic_id
        || replay.position_poststate_semantic_id() != position_poststate_semantic_id
        || replay.transition_id() != transition_id
        || replay.transition_evidence_id() != transition_evidence_id
        || replay.kind() != GeneralReplayTransitionKindV1::ReleaseUnfilledReservation
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }

    Ok(ReleaseUnfilledReservationPlanV1 {
        settlement_root_account: input.settlement_root_account,
        settlement_root_poststate,
        retained_feed_account: input.traversal.selected_feed_account,
        order_page_account: input.order_page_account,
        order_page_index: expected_page_index,
        order_slot_index: verified_slot.slot_index,
        order_index,
        position_generation: expected_position_generation,
        transition_id,
        transition_evidence_id,
        reservation: UnfilledReservationClosePlanV9 {
            account: input.reservation_account,
            semantic_id: reservation_semantic_id,
            seed: reservation_seed,
            prestate_data_id: reservation_prestate_data_id,
            terminal_data_id: reservation_terminal_data_id,
            terminal_body: reservation_terminal_body,
            release_generation,
            released_reserved_cash_atoms: reservation.remaining_cash_atoms,
            released_internal: reservation.remaining_internal,
            balance_before: input.rent_balances.reservation_lamports,
            payer,
            payer_refund_lamports: rent.refundable_principal,
            payer_balance_after,
            neutral_sink,
            neutral_sink_credit_lamports,
            neutral_sink_balance_after,
        },
        position_account: input.position.account,
        position_prestate_semantic_id,
        position_poststate_semantic_id,
        position_poststate_body,
        position: position_poststate,
        replay,
    })
}

/// Derive one exact root data identity in an isolated frame so action 41 never
/// keeps two 980-byte encoded roots live in its planner frame.
#[inline(never)]
fn settlement_root_release_data_id_v1(
    root: &SettlementRootV1AccountV1,
    settlement_root_account: Id32,
) -> Result<Id32, SettlementAdapterErrorV1> {
    root.data_id(&CanonicalSha256, settlement_root_account)
        .map_err(SettlementAdapterErrorV1::Contract)
}

#[allow(clippy::too_many_arguments)]
fn derive_unfilled_reservation_release_transition_id_v1(
    settlement_root_account: Id32,
    candidate: Id32,
    reservation_account: Id32,
    reservation_semantic_id: Id32,
    owner: Id32,
    order_id: Id32,
    order_index: u8,
    page_index: u16,
    slot_index: u8,
    position_generation: u64,
    order_generation: u64,
    release_generation: u64,
) -> Result<Id32, SettlementAdapterErrorV1> {
    if release_generation
        != order_generation
            .checked_add(1)
            .ok_or(SettlementAdapterErrorV1::ArithmeticOverflow)?
    {
        return Err(SettlementAdapterErrorV1::BindingMismatch);
    }
    let mut hash = Sha256::new();
    hash.update(UNFILLED_RESERVATION_RELEASE_TRANSITION_DOMAIN_V1);
    hash.update(settlement_root_account.bytes());
    hash.update(candidate.bytes());
    hash.update(reservation_account.bytes());
    hash.update(reservation_semantic_id.bytes());
    hash.update(owner.bytes());
    hash.update(order_id.bytes());
    hash.update([order_index]);
    hash.update(page_index.to_le_bytes());
    hash.update([slot_index]);
    hash.update(position_generation.to_le_bytes());
    hash.update(order_generation.to_le_bytes());
    hash.update(release_generation.to_le_bytes());
    Id32::new(hash.finalize().into()).map_err(SettlementAdapterErrorV1::Contract)
}

#[allow(clippy::too_many_arguments)]
fn derive_unfilled_reservation_release_evidence_id_v1(
    transition_id: Id32,
    settlement_root_account: Id32,
    root_prestate_data_id: Id32,
    root_poststate_data_id: Id32,
    retained_feed_account: Id32,
    order_page_account: Id32,
    reservation_account: Id32,
    reservation_prestate_data_id: Id32,
    reservation_terminal_data_id: Id32,
    position_account: Id32,
    position_prestate_semantic_id: Id32,
    position_poststate_semantic_id: Id32,
    replay_account: Id32,
    replay_next_sequence: u64,
    payer: Id32,
    payer_refund_lamports: u64,
    payer_balance_before: u64,
    payer_balance_after: u64,
    neutral_sink: Id32,
    neutral_sink_credit_lamports: u64,
    neutral_sink_balance_before: u64,
    neutral_sink_balance_after: u64,
    reservation_balance_before: u64,
) -> Result<Id32, SettlementAdapterErrorV1> {
    let mut hash = Sha256::new();
    hash.update(UNFILLED_RESERVATION_RELEASE_EVIDENCE_DOMAIN_V1);
    hash.update(transition_id.bytes());
    hash.update(settlement_root_account.bytes());
    hash.update(root_prestate_data_id.bytes());
    hash.update(root_poststate_data_id.bytes());
    hash.update(retained_feed_account.bytes());
    hash.update(order_page_account.bytes());
    hash.update(reservation_account.bytes());
    hash.update(reservation_prestate_data_id.bytes());
    hash.update(reservation_terminal_data_id.bytes());
    hash.update(position_account.bytes());
    hash.update(position_prestate_semantic_id.bytes());
    hash.update(position_poststate_semantic_id.bytes());
    hash.update(replay_account.bytes());
    hash.update(replay_next_sequence.to_le_bytes());
    hash.update(payer.bytes());
    hash.update(payer_refund_lamports.to_le_bytes());
    hash.update(payer_balance_before.to_le_bytes());
    hash.update(payer_balance_after.to_le_bytes());
    hash.update(neutral_sink.bytes());
    hash.update(neutral_sink_credit_lamports.to_le_bytes());
    hash.update(neutral_sink_balance_before.to_le_bytes());
    hash.update(neutral_sink_balance_after.to_le_bytes());
    hash.update(reservation_balance_before.to_le_bytes());
    Id32::new(hash.finalize().into()).map_err(SettlementAdapterErrorV1::Contract)
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
///
/// V2 deliberately excludes current Position bodies and account data IDs.
/// Every hashed fact is immutable after admission: the V5 page/order set,
/// order and Position generations, canonical Reservation identity, canonical
/// General Position seed tuple, and full Market/Realm/policy/release binding.
pub fn derive_owner_order_set_digest_v2(
    projection: &OwnerBlindBookProjectionV2,
    terms: Id32,
    reservation_policy: Id32,
    position_market: AdapterPositionMarketBindingV3,
) -> Result<Id32, SettlementAdapterErrorV1> {
    let base = projection.base();
    if terms.is_zero()
        || reservation_policy.is_zero()
        || position_market.market_instance_id.bytes()
            != base.market_binding().market_instance_v2_id.bytes()
        || position_market.realm_id.bytes() != base.realm().bytes()
        || position_market.outcome_count != base.domain().outcome_count
    {
        return Err(SettlementAdapterErrorV1::PositionSetMismatch);
    }
    let mut hash = Sha256::new();
    hash.update(OWNER_ORDER_SET_DIGEST_DOMAIN_V2);
    hash.update(base.market().bytes());
    hash.update(base.epoch().bytes());
    hash.update(base.order_set().bytes());
    hash.update(base.economic_domain_digest().bytes());
    hash.update(base.price_grid_id().bytes());
    hash.update(terms.bytes());
    hash.update(reservation_policy.bytes());
    hash.update(base.market_binding().settlement_policy_id.bytes());
    hash.update(position_market.market_instance_id.bytes());
    hash.update(position_market.realm_id.bytes());
    hash.update(position_market.collateral_policy_id.bytes());
    hash.update(position_market.collateral_release_id.bytes());
    hash.update([position_market.outcome_count]);
    hash.update(POSITION_V3_PDA_PREFIX);
    hash.update([u8::from(PositionPurposeV3::General)]);
    hash.update(base.market().bytes());
    hash.update([base.book().len]);
    let mut order = 0usize;
    while order < usize::from(base.book().len) {
        let index =
            u8::try_from(order).map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
        let membership = base
            .order_membership(index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let position_generation = projection
            .position_generation(index)
            .ok_or(SettlementAdapterErrorV1::PositionSetMismatch)?;
        let mut prior = 0usize;
        while prior < order {
            let prior_index = u8::try_from(prior)
                .map_err(|_| SettlementAdapterErrorV1::ArithmeticOverflow)?;
            let prior_membership = base
                .order_membership(prior_index)
                .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
            if prior_membership.owner() == membership.owner()
                && projection.position_generation(prior_index) != Some(position_generation)
            {
                return Err(SettlementAdapterErrorV1::PositionSetMismatch);
            }
            prior += 1;
        }
        let page_index = projection
            .order_page_index(index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let page_seed = projection
            .page_seed(page_index)
            .ok_or(SettlementAdapterErrorV1::BindingMismatch)?;
        let reservation = canonical_reservation_id(
            LayoutHash32(base.market().bytes()),
            LayoutHash32(base.epoch().bytes()),
            LayoutHash32(membership.owner().bytes()),
            position_generation,
            LayoutHash32(membership.order_id().bytes()),
        );
        hash.update([index]);
        hash.update(membership.order_id().bytes());
        hash.update(membership.owner().bytes());
        hash.update(membership.generation().to_le_bytes());
        hash.update(position_generation.to_le_bytes());
        hash.update(reservation.bytes());
        hash.update(page_seed.domain());
        hash.update(page_seed.epoch());
        hash.update(page_seed.page_index_le());
        hash.update([membership.kind().code()]);
        hash.update([match base.book().orders[order].side {
            Side::Buy => 0,
            Side::Sell => 1,
        }]);
        hash.update(POSITION_V3_PDA_PREFIX);
        hash.update(position_market.market_instance_id.bytes());
        hash.update(membership.owner().bytes());
        hash.update([u8::from(PositionPurposeV3::General)]);
        hash.update(base.market().bytes());
        order += 1;
    }
    Id32::new(hash.finalize().into()).map_err(SettlementAdapterErrorV1::Contract)
}

/// Historical whole-account membership digest retained only for V1/V2
/// construction paths.
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
    /// Position V4 rows did not exactly cover or bind the frozen owner set.
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

#[cfg(test)]
mod scalable_receipt_end_tests {
    use super::*;

    fn id(byte: u8) -> Id32 {
        Id32::new([byte; 32]).unwrap()
    }

    fn slice_bytes(buy_kind: u8, buy: u8, sell_kind: u8, sell: u8, quantity: u64) -> [u8; 13] {
        let mut value = [0u8; 13];
        value[..5].copy_from_slice(&[buy_kind, buy, sell_kind, sell, 1]);
        value[5..].copy_from_slice(&quantity.to_le_bytes());
        value
    }

    #[test]
    fn compact_slice_decoder_accepts_only_canonical_routes() {
        let direct = read_feed_slice_v3(&slice_bytes(0, 3, 0, 7, 11), 0).unwrap();
        assert_eq!(direct.route(), Ok(SettlementReceiptRouteV2::Direct));

        let split = read_feed_slice_v3(&slice_bytes(0, 3, 1, 0, 11), 0).unwrap();
        assert_eq!(split.route(), Ok(SettlementReceiptRouteV2::SplitToBuy));

        let merge = read_feed_slice_v3(&slice_bytes(2, 0, 0, 7, 11), 0).unwrap();
        assert_eq!(merge.route(), Ok(SettlementReceiptRouteV2::SellToMerge));

        assert_eq!(
            read_feed_slice_v3(&slice_bytes(0, 3, 1, 9, 11), 0),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );
        assert_eq!(
            read_feed_slice_v3(&slice_bytes(2, 9, 0, 7, 11), 0),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn compact_slice_decoder_refuses_zero_and_wrong_geometry() {
        assert_eq!(
            read_feed_slice_v3(&slice_bytes(0, 3, 0, 7, 0), 0),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );
        assert_eq!(
            read_feed_slice_v3(&slice_bytes(0, 3, 0, 7, 11)[..12], 0),
            Err(SettlementAdapterErrorV1::OutputLengthMismatch)
        );
    }

    #[test]
    fn materialization_rent_checks_aggregate_same_payer_debits() {
        let facts = [
            Some(RentDebitFactV5 {
                payer: id(1),
                balance_before: 10,
                debit: 6,
            }),
            Some(RentDebitFactV5 {
                payer: id(1),
                balance_before: 10,
                debit: 5,
            }),
            None,
        ];
        assert_eq!(
            validate_aggregate_rent_debits_v5(&facts),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );

        let inconsistent_snapshot = [
            facts[0],
            Some(RentDebitFactV5 {
                payer: id(1),
                balance_before: 11,
                debit: 4,
            }),
            None,
        ];
        assert_eq!(
            validate_aggregate_rent_debits_v5(&inconsistent_snapshot),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );

        let funded = [
            facts[0],
            Some(RentDebitFactV5 {
                payer: id(2),
                balance_before: 5,
                debit: 5,
            }),
            None,
        ];
        assert_eq!(validate_aggregate_rent_debits_v5(&funded), Ok(()));
    }

    #[test]
    fn materialization_account_set_refuses_aliases_and_overflow() {
        let mut accounts = [Id32::ZERO; 2];
        let mut len = 0usize;
        assert_eq!(
            insert_materialization_account_v5(&mut accounts, &mut len, id(1)),
            Ok(())
        );
        assert_eq!(
            insert_materialization_account_v5(&mut accounts, &mut len, id(1)),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );
        assert_eq!(
            insert_materialization_account_v5(&mut accounts, &mut len, id(2)),
            Ok(())
        );
        assert_eq!(
            insert_materialization_account_v5(&mut accounts, &mut len, id(3)),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );
    }

    fn fresh_receipt_semantic_v5() -> SettlementReceiptAccountV4 {
        SettlementReceiptAccountV4 {
            epoch: LayoutHash32([1; 32]),
            market: LayoutHash32([2; 32]),
            candidate: LayoutHash32([3; 32]),
            buy_order_id: LayoutHash32([4; 32]),
            sell_order_id: LayoutHash32([5; 32]),
            consideration_price_units: 21,
            quantity: 7,
            settled_quantity: 0,
            price: 3,
            sequence: 1,
            slice_index: 0,
            outcome: 1,
            leg_kind: RECEIPT_LEG_DIRECT,
            consumed_flags: 0,
            stored_bump: 9,
            accounted_end_mask: 0,
        }
    }

    #[test]
    fn v5_receipt_creation_never_discounts_full_rent_for_hostile_prefund() {
        let semantic = fresh_receipt_semantic_v5();
        let seed = SettlementReceiptSeedTupleV5::new(id(1), id(3), 0).unwrap();
        let plan = prepare_create_settlement_receipt_v5(
            id(6),
            seed,
            9,
            semantic,
            RentOwnedSettlementCreateFundingV5 {
                program_id: id(7),
                payer: id(8),
                system_program_id: id(9),
                payer_lamports: 1_000,
                target_lamports_before: 41,
                target_owner_before: id(9),
                target_data_len_before: 0,
                target_writable: true,
                target_executable: false,
                rent_minimum: 100,
            },
        )
        .unwrap();
        assert_eq!(plan.payer_debit_lamports(), 100);
        assert_eq!(plan.target_lamports_after(), 141);
        assert_eq!(plan.receipt().rent().refundable_principal, 100);
        assert_eq!(plan.receipt().rent().donation_floor, 41);
        assert_eq!(
            plan.receipt().transition(),
            SettlementReceiptTransitionCommitmentV5::None
        );
        assert_ne!(plan.evidence().receipt_data_id(), LayoutHash32::ZERO);
    }

    #[test]
    fn portfolio_pending_receipt_requires_the_typed_direct_transition() {
        let semantic = fresh_receipt_semantic_v5();
        let seed = SettlementReceiptSeedTupleV5::new(id(1), id(3), 0).unwrap();
        let funding = RentOwnedSettlementCreateFundingV5 {
            program_id: id(7),
            payer: id(8),
            system_program_id: id(9),
            payer_lamports: 1_000,
            target_lamports_before: 0,
            target_owner_before: id(9),
            target_data_len_before: 0,
            target_writable: true,
            target_executable: false,
            rent_minimum: 100,
        };
        let pending = prepare_create_settlement_receipt_with_transition_v5(
            id(6),
            seed,
            9,
            semantic,
            SettlementReceiptTransitionCommitmentV5::PortfolioPairPending,
            funding,
        )
        .unwrap();
        assert_eq!(
            pending.receipt().transition(),
            SettlementReceiptTransitionCommitmentV5::PortfolioPairPending
        );
        let mut merge = semantic;
        merge.leg_kind = RECEIPT_LEG_MERGE;
        assert_eq!(
            prepare_create_settlement_receipt_with_transition_v5(
                id(6),
                seed,
                9,
                merge,
                SettlementReceiptTransitionCommitmentV5::PortfolioPairPending,
                funding,
            ),
            Err(SettlementAdapterErrorV1::Layout(LayoutError::InvalidEnum))
        );
    }

    #[test]
    fn v5_receipt_creation_refuses_short_payer_and_non_system_target() {
        let semantic = fresh_receipt_semantic_v5();
        let seed = SettlementReceiptSeedTupleV5::new(id(1), id(3), 0).unwrap();
        let mut funding = RentOwnedSettlementCreateFundingV5 {
            program_id: id(7),
            payer: id(8),
            system_program_id: id(9),
            payer_lamports: 99,
            target_lamports_before: 500,
            target_owner_before: id(9),
            target_data_len_before: 0,
            target_writable: true,
            target_executable: false,
            rent_minimum: 100,
        };
        assert_eq!(
            prepare_create_settlement_receipt_v5(id(6), seed, 9, semantic, funding),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );
        funding.payer_lamports = 100;
        funding.target_owner_before = funding.program_id;
        assert_eq!(
            prepare_create_settlement_receipt_v5(id(6), seed, 9, semantic, funding),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn merge_payment_stable_evidence_refuses_cross_chain_splices() {
        let baseline = derive_merge_payment_owner_finalization_evidence_v5(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            MergePaymentFinalizationSourceV5::ZeroFeeReplay,
            id(6),
        )
        .unwrap();
        for changed in [
            derive_merge_payment_owner_finalization_evidence_v5(
                id(7), id(2), id(3), id(4), id(5),
                MergePaymentFinalizationSourceV5::ZeroFeeReplay, id(6),
            )
            .unwrap(),
            derive_merge_payment_owner_finalization_evidence_v5(
                id(1), id(7), id(3), id(4), id(5),
                MergePaymentFinalizationSourceV5::ZeroFeeReplay, id(6),
            )
            .unwrap(),
            derive_merge_payment_owner_finalization_evidence_v5(
                id(1), id(2), id(7), id(4), id(5),
                MergePaymentFinalizationSourceV5::ZeroFeeReplay, id(6),
            )
            .unwrap(),
            derive_merge_payment_owner_finalization_evidence_v5(
                id(1), id(2), id(3), id(7), id(5),
                MergePaymentFinalizationSourceV5::ZeroFeeReplay, id(6),
            )
            .unwrap(),
            derive_merge_payment_owner_finalization_evidence_v5(
                id(1), id(2), id(3), id(4), id(7),
                MergePaymentFinalizationSourceV5::ZeroFeeReplay, id(6),
            )
            .unwrap(),
            derive_merge_payment_owner_finalization_evidence_v5(
                id(1), id(2), id(3), id(4), id(5),
                MergePaymentFinalizationSourceV5::FeeFinalizationV4, id(6),
            )
            .unwrap(),
            derive_merge_payment_owner_finalization_evidence_v5(
                id(1), id(2), id(3), id(4), id(5),
                MergePaymentFinalizationSourceV5::ZeroFeeReplay, id(7),
            )
            .unwrap(),
        ] {
            assert_ne!(baseline, changed);
        }
    }

    #[test]
    fn action41_transition_identity_owns_the_terminal_generation() {
        let canonical = derive_unfilled_reservation_release_transition_id_v1(
            id(1), id(2), id(3), id(4), id(5), id(6), 7, 8, 9, 10, 11, 12,
        )
        .unwrap();
        assert!(!canonical.is_zero());
        assert_eq!(
            derive_unfilled_reservation_release_transition_id_v1(
                id(1), id(2), id(3), id(4), id(5), id(6), 7, 8, 9, 10, 11, 13,
            ),
            Err(SettlementAdapterErrorV1::BindingMismatch)
        );
        assert_eq!(
            derive_unfilled_reservation_release_transition_id_v1(
                id(1), id(2), id(3), id(4), id(5), id(6), 7, 8, 9, 10, u64::MAX, 0,
            ),
            Err(SettlementAdapterErrorV1::ArithmeticOverflow)
        );
    }

    #[test]
    fn action41_evidence_commits_root_and_exact_rent_poststates() {
        let root_pre = id(21);
        let root_post = id(22);
        let derive = |post: Id32, payer_after| {
            derive_unfilled_reservation_release_evidence_id_v1(
                id(1), id(2), root_pre, post, id(3), id(4), id(5), id(6), id(7), id(8),
                id(9), id(10), id(11), 12, id(13), 14, 15, payer_after, id(16), 17, 18,
                35, 31,
            )
            .unwrap()
        };
        let canonical = derive(root_post, 29);
        assert_ne!(canonical, derive(root_pre, 29));
        assert_ne!(canonical, derive(root_post, 30));
    }
}
