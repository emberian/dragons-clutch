//! Pure funded-order admission and release seam.
//!
//! The account-plane wrapper authenticates metadata and PDAs.  This module
//! stages the complete Position/reservation transition before any byte is
//! written, so tests can assert exact pre-state preservation on every refusal.

#![allow(dead_code)] // removed when the following account-plane commit calls this seam

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_kernel::Error as KernelError;
use clutch_solana_layout::{
    reservation::{ReservationAccount, ReservationPlan, RESERVATION_STATE_ACTIVE},
    EpochAccount, Hash32, OrderSlot, PositionAccount, EPOCH_PHASE_OPEN, MAX_OUTCOMES,
};

/// Immutable facts authenticated from the open Epoch and page header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReservationDomain {
    pub market: Hash32,
    pub epoch: Hash32,
    pub terms: Hash32,
    pub price_grid: Hash32,
    pub policy: Hash32,
    pub epoch_index: u64,
    pub price_scale: u64,
    pub outcome_count: u8,
    pub page_index: u16,
    pub phase: u8,
}

impl ReservationDomain {
    #[allow(dead_code)] // production account-plane integration follows this codec commit
    pub fn from_epoch(epoch: &EpochAccount, page_index: u16) -> Self {
        Self {
            market: epoch.market,
            epoch: epoch.epoch,
            terms: epoch.terms,
            price_grid: epoch.price_grid,
            policy: epoch.policy,
            epoch_index: epoch.epoch_index,
            price_scale: epoch.price_scale,
            outcome_count: epoch.outcome_count,
            page_index,
            phase: epoch.phase,
        }
    }
}

/// Inputs whose signatures/addresses the account plane has authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PlacementInput {
    pub actor: Hash32,
    pub domain: ReservationDomain,
    pub slot: OrderSlot,
    pub max_fee_atoms: u64,
    pub reservation_bump: u8,
}

/// Stage one exact funded placement.
pub(super) fn prepare_placement(
    position: &PositionAccount,
    input: &PlacementInput,
) -> Outcome<(PositionAccount, ReservationAccount)> {
    position.validate()?;
    require(
        input.domain.phase == EPOCH_PHASE_OPEN && position.close_state == 0,
        ClutchError::NotActive,
    )?;
    require(
        input.actor == input.slot.owner() && input.actor == position.owner,
        ClutchError::UnauthorizedActor,
    )?;
    require(
        input.domain.market == position.market,
        ClutchError::MismatchedState,
    )?;
    require(
        input.slot.generation() > 0 && slot_expiry(&input.slot)? >= input.domain.epoch_index,
        ClutchError::MismatchedState,
    )?;

    let plan = ReservationPlan::for_order(
        &input.slot,
        input.domain.outcome_count,
        input.domain.price_scale,
        input.max_fee_atoms,
    )?;
    let reservation = ReservationAccount::active(
        input.domain.market,
        input.domain.epoch,
        position.owner,
        input.slot.order_id(),
        input.domain.price_grid,
        input.domain.terms,
        input.domain.policy,
        position.generation,
        input.slot.generation(),
        input.domain.page_index,
        input.reservation_bump,
        plan,
    )?;

    let mut next = *position;
    let free_cash = next.free_cash_atoms()?;
    if plan.cash_atoms > free_cash {
        return Err(Refusal::Kernel(KernelError::InsufficientBalance));
    }
    next.reserved_cash_atoms = next
        .reserved_cash_atoms
        .checked_add(plan.cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        next.internal[i] = next.internal[i]
            .checked_sub(plan.internal[i])
            .ok_or(Refusal::Kernel(KernelError::InsufficientBalance))?;
        i += 1;
    }
    next.validate()?;
    Ok((next, reservation))
}

/// Inputs whose signatures, page slot, and account addresses are authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReleaseInput {
    pub actor: Hash32,
    pub domain: ReservationDomain,
    pub live_slot: OrderSlot,
    pub release_generation: u64,
}

/// Stage cancellation/lapse of one still-active reservation.
pub(super) fn prepare_release(
    position: &PositionAccount,
    reservation: &ReservationAccount,
    input: &ReleaseInput,
) -> Outcome<(PositionAccount, ReservationAccount)> {
    position.validate()?;
    reservation.validate()?;
    require(
        input.domain.phase == EPOCH_PHASE_OPEN && position.close_state == 0,
        ClutchError::NotActive,
    )?;
    require(
        input.actor == position.owner && input.actor == input.live_slot.owner(),
        ClutchError::UnauthorizedActor,
    )?;
    require(
        reservation.state == RESERVATION_STATE_ACTIVE
            && reservation.market == input.domain.market
            && reservation.epoch == input.domain.epoch
            && reservation.owner == position.owner
            && reservation.owner == input.live_slot.owner()
            && reservation.order_id == input.live_slot.order_id()
            && reservation.position_generation == position.generation
            && reservation.order_generation == input.live_slot.generation()
            && reservation.page_index == input.domain.page_index
            && reservation.terms == input.domain.terms
            && reservation.price_grid == input.domain.price_grid
            && reservation.policy == input.domain.policy
            && position.market == input.domain.market,
        ClutchError::MismatchedState,
    )?;
    let plan = ReservationPlan::for_order(
        &input.live_slot,
        input.domain.outcome_count,
        input.domain.price_scale,
        reservation.max_fee_atoms,
    )?;
    require(
        plan.cash_atoms == reservation.initial_cash_atoms
            && plan.internal == reservation.initial_internal
            && plan.outcome_count == reservation.outcome_count
            && plan.order_kind == reservation.order_kind
            && plan.side == reservation.side,
        ClutchError::MismatchedState,
    )?;

    let mut next_position = *position;
    next_position.reserved_cash_atoms = next_position
        .reserved_cash_atoms
        .checked_sub(reservation.remaining_cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        next_position.internal[i] = next_position.internal[i]
            .checked_add(reservation.remaining_internal[i])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        i += 1;
    }
    let next_reservation = reservation.released(input.release_generation)?;
    next_position.validate()?;
    Ok((next_position, next_reservation))
}

fn slot_expiry(slot: &OrderSlot) -> Outcome<u64> {
    match slot {
        OrderSlot::Single(order) => Ok(order.expiry_epoch),
        OrderSlot::Portfolio(order) => Ok(order.expiry_epoch),
        OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            Err(clutch_solana_layout::CodecError::InvalidEnum.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        canonical_order_id, OrderRecord, PortfolioRecord, EPOCH_PHASE_FROZEN,
    };

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn domain() -> ReservationDomain {
        ReservationDomain {
            market: h(1),
            epoch: h(2),
            terms: h(3),
            price_grid: h(4),
            policy: h(5),
            epoch_index: 9,
            price_scale: 10_000,
            outcome_count: 3,
            page_index: 0,
            phase: EPOCH_PHASE_OPEN,
        }
    }

    fn position() -> PositionAccount {
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[0] = 9;
        internal[1] = 9;
        PositionAccount {
            market: h(1),
            owner: h(7),
            generation: 6,
            internal,
            cash_atoms: 10,
            reserved_cash_atoms: 0,
            stored_bump: 8,
            close_state: 0,
        }
    }

    fn single(side: u8, quantity: u64, limit: u64, rank: u64) -> OrderSlot {
        OrderSlot::Single(OrderRecord {
            owner: h(7),
            order_id: canonical_order_id(rank),
            outcome: 0,
            side,
            quantity,
            limit,
            minimum_fill: 0,
            flags: 0,
            generation: rank,
            expiry_epoch: 9,
        })
    }

    fn place(
        position: &PositionAccount,
        slot: OrderSlot,
        fee: u64,
    ) -> Outcome<(PositionAccount, ReservationAccount)> {
        prepare_placement(
            position,
            &PlacementInput {
                actor: h(7),
                domain: domain(),
                slot,
                max_fee_atoms: fee,
                reservation_bump: 10,
            },
        )
    }

    #[test]
    fn cash_cannot_be_reserved_twice() {
        let first = place(&position(), single(0, 8, 5_000, 1), 1).unwrap();
        assert_eq!(first.0.cash_atoms, 10);
        assert_eq!(first.0.reserved_cash_atoms, 5);
        let before = first.0;
        assert_eq!(
            place(&before, single(0, 10, 5_000, 2), 1),
            Err(Refusal::Kernel(KernelError::InsufficientBalance))
        );
        assert_eq!(before.reserved_cash_atoms, 5);
    }

    #[test]
    fn claim_atoms_move_out_and_cannot_be_reserved_twice() {
        let first = place(&position(), single(1, 6, 2_500, 1), 0).unwrap();
        assert_eq!(first.0.internal[0], 3);
        assert_eq!(first.1.remaining_internal[0], 6);
        assert_eq!(
            place(&first.0, single(1, 4, 2_500, 2), 0),
            Err(Refusal::Kernel(KernelError::InsufficientBalance))
        );
    }

    #[test]
    fn cancellation_releases_exactly_once() {
        let slot = single(0, 8, 5_000, 1);
        let (reserved_position, reservation) = place(&position(), slot, 1).unwrap();
        let input = ReleaseInput {
            actor: h(7),
            domain: domain(),
            live_slot: slot,
            release_generation: 2,
        };
        let (released_position, released) =
            prepare_release(&reserved_position, &reservation, &input).unwrap();
        assert_eq!(released_position, position());
        assert!(released.remaining_is_zero());
        let before_position = released_position;
        let before_reservation = released;
        assert_eq!(
            prepare_release(&before_position, &before_reservation, &input),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(before_position, position());
        assert_eq!(before_reservation, released);
    }

    #[test]
    fn sealed_epoch_and_cross_order_release_refuse_atomically() {
        let slot = single(1, 6, 2_500, 1);
        let (reserved_position, reservation) = place(&position(), slot, 0).unwrap();
        let mut sealed = domain();
        sealed.phase = EPOCH_PHASE_FROZEN;
        let before_position = reserved_position;
        let before_reservation = reservation;
        assert_eq!(
            prepare_release(
                &before_position,
                &before_reservation,
                &ReleaseInput {
                    actor: h(7),
                    domain: sealed,
                    live_slot: slot,
                    release_generation: 2,
                }
            ),
            Err(Refusal::Adapter(ClutchError::NotActive))
        );
        assert_eq!(before_position, reserved_position);
        assert_eq!(before_reservation, reservation);

        assert_eq!(
            prepare_release(
                &before_position,
                &before_reservation,
                &ReleaseInput {
                    actor: h(7),
                    domain: domain(),
                    live_slot: single(1, 6, 2_500, 2),
                    release_generation: 3,
                }
            ),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn portfolio_products_and_basis_binding_are_exact() {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[0] = 2;
        coefficients[1] = 3;
        let slot = OrderSlot::Portfolio(PortfolioRecord {
            owner: h(7),
            order_id: canonical_order_id(1),
            side: 1,
            active_len: 2,
            flags: 0,
            coefficients,
            lots: 3,
            limit_collateral_per_lot: 8,
            minimum_fill_lots: 0,
            generation: 1,
            expiry_epoch: 9,
        });
        let (reserved_position, reservation) = place(&position(), slot, 2).unwrap();
        assert_eq!(&reservation.initial_internal[..3], &[6, 9, 0]);
        assert_eq!(&reserved_position.internal[..3], &[3, 0, 0]);

        let mut wrong_terms = domain();
        wrong_terms.terms = h(0xaa);
        assert_eq!(
            prepare_release(
                &reserved_position,
                &reservation,
                &ReleaseInput {
                    actor: h(7),
                    domain: wrong_terms,
                    live_slot: slot,
                    release_generation: 2,
                }
            ),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn hostile_owner_generation_fee_and_overflow_refuse() {
        let slot = single(0, 8, 5_000, 1);
        let mut wrong_actor = PlacementInput {
            actor: h(9),
            domain: domain(),
            slot,
            max_fee_atoms: 1,
            reservation_bump: 10,
        };
        assert_eq!(
            prepare_placement(&position(), &wrong_actor),
            Err(Refusal::Adapter(ClutchError::UnauthorizedActor))
        );
        wrong_actor.actor = h(7);
        wrong_actor.max_fee_atoms = u64::MAX;
        assert_eq!(
            prepare_placement(&position(), &wrong_actor),
            Err(Refusal::Codec(
                clutch_solana_layout::CodecError::ArithmeticOverflow
            ))
        );
    }
}
