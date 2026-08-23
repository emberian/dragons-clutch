//! Fresh `0xb4/1` Direct Reservation semantic owner.

use clutch_batch::relation_v1::MAX_OUTCOMES;
use clutch_batch::relation_v2::EconomicOrderV2;
use clutch_batch::{PartialPolicy, Side};
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{PositionAccountV3, PositionPurposeV3, PositionV3Fields};

use crate::{
    require_fresh_child_account, require_live, DirectHashBackendV1, DirectMarketErrorV1,
    DirectMarketRootV1, DirectRentOwnerV1, DirectRootPhaseV1,
};

const RESERVATION_STATE_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/reservation-state/v1\0";

/// Exact current Reservation lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectReservationPhaseV1 {
    /// Funding is reserved and the order may enter the frozen Direct book.
    Active,
    /// Action 3 returned all funding and deleted the account.
    Cancelled,
    /// Action 9 consumed the exact selected fill; archive awaits action 13.
    Settled,
    /// Action 10..12 returned funding; archive awaits action 13.
    Lapsed,
}

impl DirectReservationPhaseV1 {
    /// Stable persisted byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Active => 1,
            Self::Cancelled => 2,
            Self::Settled => 3,
            Self::Lapsed => 4,
        }
    }
}

/// Private default-deny admission boundary.
pub trait AuthenticatedDirectReservationAdmissionV1 {
    /// Authenticate owner/controller authorization, exact root/Position/Replay
    /// accounts, fresh Reservation PDA absence, funding, and writable metas.
    fn authenticate_admission(
        &self,
        _root: DirectMarketRootV1,
        _position: AuthenticatedPositionV3,
        _existing_peer: Option<DirectReservationV1>,
        _reservation_account: [u8; 32],
        _order_id: [u8; 32],
        _side: Side,
        _outcome: u8,
        _quantity: u64,
        _minimum_fill: u64,
        _partial_policy: PartialPolicy,
        _expiry_epoch: u64,
        _limit_price_units_per_egg: u128,
        _rent: DirectRentOwnerV1,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing admission authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectReservationAdmissionAuthorityV1;

impl AuthenticatedDirectReservationAdmissionV1 for NoDirectReservationAdmissionAuthorityV1 {}

/// Sole semantic owner of one funded Direct order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationV1 {
    pub(crate) market_instance_id: [u8; 32],
    pub(crate) generation: u64,
    pub(crate) direct_root_account: [u8; 32],
    pub(crate) reservation_account: [u8; 32],
    pub(crate) general_market_runtime: [u8; 32],
    pub(crate) owner: [u8; 32],
    pub(crate) position_account: [u8; 32],
    pub(crate) position_replay_account: [u8; 32],
    pub(crate) position_generation: u64,
    pub(crate) order_id: [u8; 32],
    pub(crate) side: Side,
    pub(crate) outcome: u8,
    pub(crate) outcome_count: u8,
    pub(crate) quantity: u64,
    pub(crate) minimum_fill: u64,
    pub(crate) partial_policy: PartialPolicy,
    pub(crate) expiry_epoch: u64,
    pub(crate) limit_price_units_per_egg: u128,
    pub(crate) price_scale: u64,
    pub(crate) reserved_cash_atoms: u64,
    pub(crate) reserved_eggs: u64,
    pub(crate) rent: DirectRentOwnerV1,
    pub(crate) phase: DirectReservationPhaseV1,
    pub(crate) terminal_receipt_id: [u8; 32],
}

impl DirectReservationV1 {
    /// Exact Reservation account.
    pub const fn account(self) -> [u8; 32] { self.reservation_account }
    /// Exact order identity.
    pub const fn order_id(self) -> [u8; 32] { self.order_id }
    /// Semantic owner of the Position.
    pub const fn owner(self) -> [u8; 32] { self.owner }
    /// Buy or sell side.
    pub const fn side(self) -> Side { self.side }
    /// Selected scalar outcome.
    pub const fn outcome(self) -> u8 { self.outcome }
    /// Maximum native-Egg quantity.
    pub const fn quantity(self) -> u64 { self.quantity }
    /// Exact price-unit limit per Egg.
    pub const fn limit_price_units_per_egg(self) -> u128 { self.limit_price_units_per_egg }
    /// Exact reserved buyer cash aggregate.
    pub const fn reserved_cash_atoms(self) -> u64 { self.reserved_cash_atoms }
    /// Exact removed seller Eggs.
    pub const fn reserved_eggs(self) -> u64 { self.reserved_eggs }
    /// Current lifecycle.
    pub const fn phase(self) -> DirectReservationPhaseV1 { self.phase }
    /// Persisted rent ownership.
    pub const fn rent(self) -> DirectRentOwnerV1 { self.rent }
    /// Terminal transition receipt, zero only while active.
    pub const fn terminal_receipt_id(self) -> [u8; 32] { self.terminal_receipt_id }

    /// Validate the exact side/funding/lifecycle partition.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        for id in [
            self.market_instance_id,
            self.direct_root_account,
            self.reservation_account,
            self.general_market_runtime,
            self.owner,
            self.position_account,
            self.position_replay_account,
            self.order_id,
        ] {
            require_live(id)?;
        }
        self.rent.validate()?;
        if self.generation == 0
            || self.position_generation == 0
            || self.expiry_epoch < self.generation
            || !(2..=MAX_OUTCOMES).contains(&usize::from(self.outcome_count))
            || usize::from(self.outcome) >= usize::from(self.outcome_count)
            || self.quantity == 0
            || self.minimum_fill > self.quantity
            || (self.partial_policy == PartialPolicy::AllOrNone
                && self.minimum_fill != self.quantity)
            || self.price_scale == 0
        {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        match self.side {
            Side::Buy
                if self.reserved_cash_atoms
                    == exact_cash_atoms(
                        self.quantity,
                        self.limit_price_units_per_egg,
                        self.price_scale,
                    )?
                    && self.reserved_eggs == 0 => {}
            Side::Sell if self.reserved_cash_atoms == 0 && self.reserved_eggs == self.quantity => {}
            _ => return Err(DirectMarketErrorV1::MismatchedBinding),
        }
        match self.phase {
            DirectReservationPhaseV1::Active if self.terminal_receipt_id == [0; 32] => {}
            DirectReservationPhaseV1::Cancelled
            | DirectReservationPhaseV1::Settled
            | DirectReservationPhaseV1::Lapsed => require_live(self.terminal_receipt_id)?,
            _ => return Err(DirectMarketErrorV1::WrongPhase),
        }
        Ok(())
    }

    /// Validate immutable Market/Realm-owned binding against the exact root.
    pub fn validate_against_root(
        self,
        root: DirectMarketRootV1,
    ) -> Result<(), DirectMarketErrorV1> {
        self.validate()?;
        root.validate()?;
        let binding = root.binding();
        if self.market_instance_id != binding.market_instance_id
            || self.generation != binding.generation
            || self.direct_root_account != binding.direct_root_account
            || self.general_market_runtime != binding.general_market_runtime
            || self.outcome_count != binding.outcome_count
            || self.price_scale != binding.price_scale
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        Ok(())
    }

    /// Reconstruct the sole owner-blind RelationV2 order without an owner DTO.
    pub fn economic_order(self) -> Result<EconomicOrderV2, DirectMarketErrorV1> {
        self.validate()?;
        if self.phase != DirectReservationPhaseV1::Active {
            return Err(DirectMarketErrorV1::WrongPhase);
        }
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[usize::from(self.outcome)] = 1;
        Ok(EconomicOrderV2 {
            order_id: self.order_id,
            side: self.side,
            coefficients,
            quantity: self.quantity,
            minimum_fill: self.minimum_fill,
            partial_policy: self.partial_policy,
            expiry_epoch: self.expiry_epoch,
            limit_value_price_units_per_unit: self.limit_price_units_per_egg,
        })
    }

    /// Domain-separated identity of the complete Reservation state.
    pub fn semantic_id<B: DirectHashBackendV1>(
        self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let id = backend.sha256_parts(&[
            RESERVATION_STATE_DOMAIN_V1,
            &self.market_instance_id,
            &self.generation.to_le_bytes(),
            &self.direct_root_account,
            &self.reservation_account,
            &self.general_market_runtime,
            &self.owner,
            &self.position_account,
            &self.position_replay_account,
            &self.position_generation.to_le_bytes(),
            &self.order_id,
            &[side_byte(self.side)],
            &[self.outcome],
            &[self.outcome_count],
            &self.quantity.to_le_bytes(),
            &self.minimum_fill.to_le_bytes(),
            &[partial_policy_byte(self.partial_policy)],
            &self.expiry_epoch.to_le_bytes(),
            &self.limit_price_units_per_egg.to_le_bytes(),
            &self.price_scale.to_le_bytes(),
            &self.reserved_cash_atoms.to_le_bytes(),
            &self.reserved_eggs.to_le_bytes(),
            &self.rent.payer,
            &self.rent.principal_lamports.to_le_bytes(),
            &self.rent.donation_floor_lamports.to_le_bytes(),
            &[self.phase.byte()],
            &self.terminal_receipt_id,
        ]);
        require_live(id)?;
        Ok(id)
    }

    pub(crate) fn terminalize(
        mut self,
        phase: DirectReservationPhaseV1,
        receipt: [u8; 32],
    ) -> Result<Self, DirectMarketErrorV1> {
        if self.phase != DirectReservationPhaseV1::Active
            || phase == DirectReservationPhaseV1::Active
        {
            return Err(DirectMarketErrorV1::WrongPhase);
        }
        require_live(receipt)?;
        self.phase = phase;
        self.terminal_receipt_id = receipt;
        self.validate()?;
        Ok(self)
    }
}

/// Atomic Reservation and PositionV3 poststate for action 2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationAdmissionPlanV1 {
    /// Fresh exact Reservation state.
    pub reservation: DirectReservationV1,
    /// Position successor with exact reserve and child-count effect.
    pub position_poststate: PositionSettlementPoststateV3,
}

/// Admit one funded scalar Direct order from an authenticated PositionV3.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_reservation_admission_v1<
    A: AuthenticatedDirectReservationAdmissionV1 + ?Sized,
    B: DirectHashBackendV1,
>(
    authority: &A,
    root: DirectMarketRootV1,
    position: AuthenticatedPositionV3,
    existing_peer: Option<DirectReservationV1>,
    reservation_account: [u8; 32],
    order_id: [u8; 32],
    side: Side,
    outcome: u8,
    quantity: u64,
    minimum_fill: u64,
    partial_policy: PartialPolicy,
    expiry_epoch: u64,
    limit_price_units_per_egg: u128,
    rent: DirectRentOwnerV1,
    backend: &B,
) -> Result<DirectReservationAdmissionPlanV1, DirectMarketErrorV1> {
    root.validate()?;
    position
        .validate_writable()
        .map_err(|_| DirectMarketErrorV1::InvalidPosition)?;
    require_fresh_child_account(root.binding(), reservation_account)?;
    require_live(order_id)?;
    rent.validate()?;
    if root.phase() != DirectRootPhaseV1::Open
        || root.admitted_reservations() >= crate::MAX_DIRECT_RESERVATIONS_V1
    {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    let binding = root.binding();
    let fields = position.semantic.fields();
    if fields.purpose != PositionPurposeV3::General
        || fields.market_instance_id.bytes() != binding.market_instance_id
        || fields.realm_id.bytes() != binding.realm_id
        || fields.collateral_policy_id.bytes() != binding.collateral_policy_id
        || fields.collateral_release_id.bytes() != binding.collateral_release_id
        || fields.purpose_binding_id.bytes() != binding.general_market_runtime
        || position.general_market_runtime != binding.general_market_runtime
        || fields.owner.bytes() == reservation_account
        || position.account == reservation_account
        || fields.replay_account.bytes() == reservation_account
        || fields.outcome_count != binding.outcome_count
        || usize::from(outcome) >= usize::from(binding.outcome_count)
        || expiry_epoch < binding.generation
        || quantity == 0
        || minimum_fill > quantity
        || (partial_policy == PartialPolicy::AllOrNone && minimum_fill != quantity)
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    match (root.live_reservations(), existing_peer) {
        (0, None) => {}
        (1, Some(peer)) => {
            peer.validate_against_root(root)?;
            let peer_id = peer.semantic_id(backend)?;
            if peer.phase() != DirectReservationPhaseV1::Active
                || peer.account() != root.reservation_account(0)?
                || peer_id != root.reservation_semantic_id(0)?
                || peer.account() == reservation_account
                || peer.order_id() == order_id
                || peer.side() == side
                || peer.outcome() != outcome
            {
                return Err(DirectMarketErrorV1::MismatchedBinding);
            }
        }
        _ => return Err(DirectMarketErrorV1::MismatchedBinding),
    }
    let reserved_cash_atoms = match side {
        Side::Buy => exact_cash_atoms(quantity, limit_price_units_per_egg, binding.price_scale)?,
        Side::Sell => 0,
    };
    let reserved_eggs = match side { Side::Buy => 0, Side::Sell => quantity };
    authority.authenticate_admission(
        &root,
        position,
        existing_peer,
        reservation_account,
        order_id,
        side,
        outcome,
        quantity,
        minimum_fill,
        partial_policy,
        expiry_epoch,
        limit_price_units_per_egg,
        rent,
    )?;
    let mut native_eggs = fields.native_eggs;
    let reserved_cash_after = match side {
        Side::Buy => fields
            .reserved_cash_atoms
            .checked_add(reserved_cash_atoms)
            .ok_or(DirectMarketErrorV1::Arithmetic)?,
        Side::Sell => fields.reserved_cash_atoms,
    };
    if reserved_cash_after > fields.cash_atoms {
        return Err(DirectMarketErrorV1::InvalidPosition);
    }
    if side == Side::Sell {
        native_eggs[usize::from(outcome)] = native_eggs[usize::from(outcome)]
            .checked_sub(quantity)
            .ok_or(DirectMarketErrorV1::InvalidPosition)?;
    }
    let outstanding_reservations = fields
        .outstanding_reservations
        .checked_add(1)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let semantic = PositionAccountV3::new(PositionV3Fields {
        reserved_cash_atoms: reserved_cash_after,
        native_eggs,
        outstanding_reservations,
        ..fields
    })
    .map_err(|_| DirectMarketErrorV1::InvalidPosition)?;
    let reservation = DirectReservationV1 {
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        direct_root_account: binding.direct_root_account,
        reservation_account,
        general_market_runtime: binding.general_market_runtime,
        owner: fields.owner.bytes(),
        position_account: position.account,
        position_replay_account: fields.replay_account.bytes(),
        position_generation: fields.generation,
        order_id,
        side,
        outcome,
        outcome_count: binding.outcome_count,
        quantity,
        minimum_fill,
        partial_policy,
        expiry_epoch,
        limit_price_units_per_egg,
        price_scale: binding.price_scale,
        reserved_cash_atoms,
        reserved_eggs,
        rent,
        phase: DirectReservationPhaseV1::Active,
        terminal_receipt_id: [0; 32],
    };
    reservation.validate()?;
    Ok(DirectReservationAdmissionPlanV1 {
        reservation,
        position_poststate: PositionSettlementPoststateV3 {
            account: position.account,
            general_market_runtime: position.general_market_runtime,
            prestate_semantic_id: position.semantic_id,
            semantic,
        },
    })
}

fn exact_cash_atoms(
    quantity: u64,
    price_units_per_egg: u128,
    price_scale: u64,
) -> Result<u64, DirectMarketErrorV1> {
    if price_scale == 0 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let units = u128::from(quantity)
        .checked_mul(price_units_per_egg)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let scale = u128::from(price_scale);
    if units % scale != 0 {
        return Err(DirectMarketErrorV1::InexactCashConversion);
    }
    u64::try_from(units / scale).map_err(|_| DirectMarketErrorV1::Arithmetic)
}

const fn side_byte(side: Side) -> u8 {
    match side { Side::Buy => 1, Side::Sell => 2 }
}

const fn partial_policy_byte(policy: PartialPolicy) -> u8 {
    match policy { PartialPolicy::Allow => 1, PartialPolicy::AllOrNone => 2 }
}
