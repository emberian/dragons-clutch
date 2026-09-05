//! Exact sparse Claims transition for Dealer V3 junior equity.
//!
//! The Dealer request carries this canonical SignedDeltaV3 packet as a signed
//! trailing witness. Dealer recomputes the complete poststate from its equity
//! semantics and compares every header coordinate, Position, aggregate delta,
//! and nonzero Position/outcome row. There is no second persisted inventory or
//! family-specific Claims writer.

#[cfg(not(target_os = "solana"))]
extern crate alloc;

#[cfg(not(target_os = "solana"))]
use alloc::{vec, vec::Vec};

use dclutch_claims_svm::{
    CallerRole,
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
        SignedDeltaPlanV3, SignedDeltaPositionV3, SignedDeltaV3, plan_bytes,
    },
};

/// Stable refusal from Dealer-to-Claims signed-delta composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquityClaimsErrorV3 {
    /// Runtime inventory slices were empty or had different widths.
    WidthMismatch,
    /// A required identity was zero or owners were not distinct.
    InvalidIdentity,
    /// An exact delta/count/revision computation overflowed.
    Arithmetic,
    /// SignedDeltaV3 construction or hostile decoding refused.
    ClaimsPacket,
    /// Packet header, table, or row semantics differed from the equity plan.
    PacketMismatch,
}

/// Immutable Claims-owned coordinates authenticated from chain state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EquityClaimsContextV3 {
    /// Immutable execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Request identity derived from the exact fixed Dealer request header.
    pub request_id: [u8; 32],
    /// Exact finalized Product-record digest.
    pub product_record_digest: [u8; 32],
    /// Stable semantic LiabilityBasisV2 identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact finalized linked-basis raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Current Claims aggregate revision.
    pub expected_market_revision: u64,
    /// Canonical Dealer Claims Position owner.
    pub dealer_owner: [u8; 32],
    /// Current Dealer Position revision.
    pub dealer_revision: u64,
    /// Canonical LP Claims Position owner.
    pub lp_owner: [u8; 32],
    /// Current LP Position revision.
    pub lp_revision: u64,
}

/// Borrowed exact pre/post Claims resources for one equity action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EquityClaimsTransitionV3<'a> {
    /// Current canonical Dealer inventory.
    pub dealer_before: &'a [u64],
    /// Candidate canonical Dealer inventory.
    pub dealer_after: &'a [u64],
    /// Current canonical LP inventory.
    pub lp_before: &'a [u64],
    /// Candidate canonical LP inventory.
    pub lp_after: &'a [u64],
    /// Complete sets minted before Position movement.
    pub minimum_complete_sets_to_split: u64,
    /// Complete sets burned after Position movement.
    pub maximum_complete_sets_to_merge: u64,
}

/// Exact sparse packet geometry; zero means the Claims route is disabled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EquityClaimsGeometryV3 {
    /// Exact SignedDeltaV3 packet bytes, or zero for a Claims no-op.
    pub packet_bytes: usize,
    /// Number of actually changed Position owners.
    pub position_count: u32,
    /// Number of nonzero Position/outcome rows.
    pub position_delta_count: u32,
}

/// Compute exact sparse packet geometry from canonical pre/post resources.
pub fn equity_claims_geometry_v3(
    context: EquityClaimsContextV3,
    transition: EquityClaimsTransitionV3<'_>,
) -> Result<EquityClaimsGeometryV3, EquityClaimsErrorV3> {
    validate(context, transition)?;
    let width = transition.dealer_before.len();
    let dealer_used = any_position_delta(transition.dealer_before, transition.dealer_after)?;
    let lp_used = any_position_delta(transition.lp_before, transition.lp_after)?;
    let position_count = u32::from(dealer_used) + u32::from(lp_used);
    let mut position_delta_count = 0_u32;
    for (before, after) in transition
        .dealer_before
        .iter()
        .zip(transition.dealer_after.iter())
        .chain(transition.lp_before.iter().zip(transition.lp_after.iter()))
    {
        if before != after {
            position_delta_count = position_delta_count
                .checked_add(1)
                .ok_or(EquityClaimsErrorV3::Arithmetic)?;
        }
    }
    if position_delta_count == 0 {
        if transition.minimum_complete_sets_to_split != transition.maximum_complete_sets_to_merge {
            return Err(EquityClaimsErrorV3::PacketMismatch);
        }
        return Ok(EquityClaimsGeometryV3 {
            packet_bytes: 0,
            position_count: 0,
            position_delta_count: 0,
        });
    }
    let claim_count = u32::try_from(width).map_err(|_| EquityClaimsErrorV3::Arithmetic)?;
    let packet_bytes = plan_bytes(claim_count, position_count, position_delta_count)
        .map_err(|_| EquityClaimsErrorV3::ClaimsPacket)?;
    Ok(EquityClaimsGeometryV3 {
        packet_bytes,
        position_count,
        position_delta_count,
    })
}

/// Encode the exact sparse packet into caller-owned bytes.
///
/// Host construction may allocate the bounded transient row vectors. The SBF
/// adapter uses [`validate_equity_claims_packet_v3`] and never allocates or
/// fabricates a child packet.
#[cfg(not(target_os = "solana"))]
pub fn encode_equity_claims_packet_v3(
    context: EquityClaimsContextV3,
    transition: EquityClaimsTransitionV3<'_>,
    output: &mut [u8],
) -> Result<EquityClaimsGeometryV3, EquityClaimsErrorV3> {
    let geometry = equity_claims_geometry_v3(context, transition)?;
    if output.len() != geometry.packet_bytes {
        return Err(EquityClaimsErrorV3::WidthMismatch);
    }
    if geometry.packet_bytes == 0 {
        return Ok(geometry);
    }
    let mut owners = [
        (context.dealer_owner, context.dealer_revision, true),
        (context.lp_owner, context.lp_revision, false),
    ];
    if owners[0].0 > owners[1].0 {
        owners.swap(0, 1);
    }
    let mut positions = Vec::with_capacity(usize::try_from(geometry.position_count).unwrap_or(0));
    let mut rows = Vec::with_capacity(usize::try_from(geometry.position_delta_count).unwrap_or(0));
    for (owner, revision, dealer) in owners {
        let (before, after) = if dealer {
            (transition.dealer_before, transition.dealer_after)
        } else {
            (transition.lp_before, transition.lp_after)
        };
        if !any_position_delta(before, after)? {
            continue;
        }
        let position_index =
            u32::try_from(positions.len()).map_err(|_| EquityClaimsErrorV3::Arithmetic)?;
        positions.push(
            SignedDeltaPositionV3::new(owner, revision)
                .map_err(|_| EquityClaimsErrorV3::ClaimsPacket)?,
        );
        for (outcome, (pre, post)) in before.iter().zip(after.iter()).enumerate() {
            let delta = signed_difference(*pre, *post)?;
            if delta.direction() == DeltaDirectionV3::Neutral {
                continue;
            }
            rows.push(
                PositionDeltaV3::new(
                    PositionDeltaInputV3 {
                        position_index,
                        outcome: u32::try_from(outcome)
                            .map_err(|_| EquityClaimsErrorV3::Arithmetic)?,
                        delta,
                    },
                    geometry.position_count,
                    u32::try_from(before.len()).map_err(|_| EquityClaimsErrorV3::Arithmetic)?,
                )
                .map_err(|_| EquityClaimsErrorV3::ClaimsPacket)?,
            );
        }
    }
    let aggregate = aggregate_delta(transition)?;
    let aggregate_deltas = vec![aggregate; transition.dealer_before.len()];
    SignedDeltaPlanV3::encode_into(
        SignedDeltaPlanInputV3 {
            caller_role: CallerRole::Trading,
            release_set: context.release_set,
            market: context.market,
            request_id: context.request_id,
            product_record_digest: context.product_record_digest,
            semantic_basis_id: context.semantic_basis_id,
            linked_basis_record_digest: context.linked_basis_record_digest,
            expected_market_revision: context.expected_market_revision,
            claim_count: u32::try_from(transition.dealer_before.len())
                .map_err(|_| EquityClaimsErrorV3::Arithmetic)?,
        },
        &positions,
        &aggregate_deltas,
        &rows,
        output,
    )
    .map_err(|_| EquityClaimsErrorV3::ClaimsPacket)?;
    Ok(geometry)
}

/// Validate a signed borrowed packet against the recomputed equity poststate.
///
/// An empty packet is canonical only when every Position and aggregate delta
/// is zero. Nonempty packets are hostile-decoded and compared in their exact
/// canonical order without allocation.
pub fn validate_equity_claims_packet_v3(
    context: EquityClaimsContextV3,
    transition: EquityClaimsTransitionV3<'_>,
    packet: &[u8],
) -> Result<EquityClaimsGeometryV3, EquityClaimsErrorV3> {
    let geometry = equity_claims_geometry_v3(context, transition)?;
    if packet.len() != geometry.packet_bytes {
        return Err(EquityClaimsErrorV3::PacketMismatch);
    }
    if packet.is_empty() {
        return Ok(geometry);
    }
    let plan = SignedDeltaPlanV3::decode(packet).map_err(|_| EquityClaimsErrorV3::ClaimsPacket)?;
    if plan.caller_role() != CallerRole::Trading
        || plan.release_set() != context.release_set
        || plan.market() != context.market
        || plan.request_id() != context.request_id
        || plan.product_record_digest() != context.product_record_digest
        || plan.semantic_basis_id() != context.semantic_basis_id
        || plan.linked_basis_record_digest() != context.linked_basis_record_digest
        || plan.expected_market_revision() != context.expected_market_revision
        || usize::try_from(plan.claim_count()).ok() != Some(transition.dealer_before.len())
        || plan.position_count() != geometry.position_count
        || plan.position_delta_count() != geometry.position_delta_count
    {
        return Err(EquityClaimsErrorV3::PacketMismatch);
    }
    let aggregate = aggregate_delta(transition)?;
    for outcome in 0..plan.claim_count() {
        if plan
            .aggregate_delta(outcome)
            .map_err(|_| EquityClaimsErrorV3::ClaimsPacket)?
            != aggregate
        {
            return Err(EquityClaimsErrorV3::PacketMismatch);
        }
    }

    let mut expected_position = 0_u32;
    let mut expected_row = 0_u32;
    let ordered = if context.dealer_owner < context.lp_owner {
        [true, false]
    } else {
        [false, true]
    };
    for dealer in ordered {
        let (owner, revision, before, after) = if dealer {
            (
                context.dealer_owner,
                context.dealer_revision,
                transition.dealer_before,
                transition.dealer_after,
            )
        } else {
            (
                context.lp_owner,
                context.lp_revision,
                transition.lp_before,
                transition.lp_after,
            )
        };
        if !any_position_delta(before, after)? {
            continue;
        }
        let position = plan
            .position(expected_position)
            .map_err(|_| EquityClaimsErrorV3::ClaimsPacket)?;
        if position.owner() != owner || position.expected_revision() != revision {
            return Err(EquityClaimsErrorV3::PacketMismatch);
        }
        for (outcome, (pre, post)) in before.iter().zip(after.iter()).enumerate() {
            let delta = signed_difference(*pre, *post)?;
            if delta.direction() == DeltaDirectionV3::Neutral {
                continue;
            }
            let row = plan
                .position_delta(expected_row)
                .map_err(|_| EquityClaimsErrorV3::ClaimsPacket)?;
            if row.position_index() != expected_position
                || usize::try_from(row.outcome()).ok() != Some(outcome)
                || row.delta() != delta
            {
                return Err(EquityClaimsErrorV3::PacketMismatch);
            }
            expected_row = expected_row
                .checked_add(1)
                .ok_or(EquityClaimsErrorV3::Arithmetic)?;
        }
        expected_position = expected_position
            .checked_add(1)
            .ok_or(EquityClaimsErrorV3::Arithmetic)?;
    }
    if expected_position != geometry.position_count || expected_row != geometry.position_delta_count
    {
        return Err(EquityClaimsErrorV3::PacketMismatch);
    }
    Ok(geometry)
}

fn validate(
    context: EquityClaimsContextV3,
    transition: EquityClaimsTransitionV3<'_>,
) -> Result<(), EquityClaimsErrorV3> {
    let width = transition.dealer_before.len();
    if width == 0
        || transition.dealer_after.len() != width
        || transition.lp_before.len() != width
        || transition.lp_after.len() != width
    {
        return Err(EquityClaimsErrorV3::WidthMismatch);
    }
    for identity in [
        context.release_set,
        context.market,
        context.request_id,
        context.product_record_digest,
        context.semantic_basis_id,
        context.linked_basis_record_digest,
        context.dealer_owner,
        context.lp_owner,
    ] {
        if identity == [0; 32] {
            return Err(EquityClaimsErrorV3::InvalidIdentity);
        }
    }
    if context.dealer_owner == context.lp_owner
        || context.dealer_revision == u64::MAX
        || context.lp_revision == u64::MAX
        || context.expected_market_revision == u64::MAX
    {
        return Err(EquityClaimsErrorV3::InvalidIdentity);
    }
    Ok(())
}

fn any_position_delta(before: &[u64], after: &[u64]) -> Result<bool, EquityClaimsErrorV3> {
    if before.len() != after.len() {
        return Err(EquityClaimsErrorV3::WidthMismatch);
    }
    Ok(before
        .iter()
        .zip(after.iter())
        .any(|(pre, post)| pre != post))
}

fn signed_difference(before: u64, after: u64) -> Result<SignedDeltaV3, EquityClaimsErrorV3> {
    let (direction, magnitude) = match after.cmp(&before) {
        core::cmp::Ordering::Equal => (DeltaDirectionV3::Neutral, 0),
        core::cmp::Ordering::Greater => (
            DeltaDirectionV3::Credit,
            after
                .checked_sub(before)
                .ok_or(EquityClaimsErrorV3::Arithmetic)?,
        ),
        core::cmp::Ordering::Less => (
            DeltaDirectionV3::Debit,
            before
                .checked_sub(after)
                .ok_or(EquityClaimsErrorV3::Arithmetic)?,
        ),
    };
    SignedDeltaV3::new(direction, magnitude).map_err(|_| EquityClaimsErrorV3::ClaimsPacket)
}

fn aggregate_delta(
    transition: EquityClaimsTransitionV3<'_>,
) -> Result<SignedDeltaV3, EquityClaimsErrorV3> {
    signed_difference(
        transition.maximum_complete_sets_to_merge,
        transition.minimum_complete_sets_to_split,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> EquityClaimsContextV3 {
        EquityClaimsContextV3 {
            release_set: [1; 32],
            market: [2; 32],
            request_id: [3; 32],
            product_record_digest: [4; 32],
            semantic_basis_id: [5; 32],
            linked_basis_record_digest: [6; 32],
            expected_market_revision: 7,
            dealer_owner: [7; 32],
            dealer_revision: 8,
            lp_owner: [8; 32],
            lp_revision: 9,
        }
    }

    #[test]
    fn sparse_packet_round_trips_and_substitution_refuses() {
        let transition = EquityClaimsTransitionV3 {
            dealer_before: &[0, 10, 20],
            dealer_after: &[0, 15, 30],
            lp_before: &[10, 10, 10],
            lp_after: &[10, 5, 0],
            minimum_complete_sets_to_split: 0,
            maximum_complete_sets_to_merge: 0,
        };
        let geometry = equity_claims_geometry_v3(context(), transition).expect("geometry");
        assert_eq!(geometry.position_count, 2);
        assert_eq!(geometry.position_delta_count, 4);
        let mut packet = alloc::vec![0; geometry.packet_bytes];
        encode_equity_claims_packet_v3(context(), transition, &mut packet).expect("packet");
        assert_eq!(
            validate_equity_claims_packet_v3(context(), transition, &packet),
            Ok(geometry)
        );
        let last = packet.len().checked_sub(1).expect("packet");
        *packet.get_mut(last).expect("last") ^= 1;
        assert!(validate_equity_claims_packet_v3(context(), transition, &packet).is_err());
    }

    #[test]
    fn true_noop_uses_no_claims_route() {
        let transition = EquityClaimsTransitionV3 {
            dealer_before: &[0, 0],
            dealer_after: &[0, 0],
            lp_before: &[0, 0],
            lp_after: &[0, 0],
            minimum_complete_sets_to_split: 0,
            maximum_complete_sets_to_merge: 0,
        };
        assert_eq!(
            validate_equity_claims_packet_v3(context(), transition, &[]),
            Ok(EquityClaimsGeometryV3 {
                packet_bytes: 0,
                position_count: 0,
                position_delta_count: 0,
            })
        );
    }
}
