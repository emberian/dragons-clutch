//! The strand's Claims leg, built and hostile-decoded across the crate seam.
//!
//! `build_strand_packets_v1` is the only producer of `ClaimsAction::StrandResidual`,
//! and the Claims crate is the only thing that will accept or refuse it. This
//! file runs the two against each other: the builder's output must decode as a
//! canonical Claims plan, and every shape the strand must NOT take must be
//! refused by an exact named variant.
//!
//! The strand is the tree's first pre-terminal NON-UNIFORM supply move
//! (`EconomicKernel.strandPost`), so the assertion that most needs making is
//! that the Claims plan validator admits an uneven quantity vector here while
//! still refusing one for every complete-set action.

use dclutch_claims::{
    CallerRole, ClaimsAction, ClaimsPlanV1, Error as ClaimsError, NO_POSITION_REVISION,
};
use dclutch_trading::general::child_packets::{
    ChildPacketError, ClaimsResourcesV2, build_strand_packets_v1,
};
use dclutch_trading::general::{AggregateReplayContextV1, ExecutionContextV1};

const MARKET: [u8; 32] = [1; 32];
const RELEASE_SET: [u8; 32] = [2; 32];
const CANDIDATE: [u8; 32] = [3; 32];
const SETTLEMENT_OWNER: [u8; 32] = [4; 32];

fn context() -> AggregateReplayContextV1 {
    AggregateReplayContextV1 {
        execution: ExecutionContextV1 {
            market_id: MARKET,
            release_set_id: RELEASE_SET,
        },
        candidate_id: CANDIDATE,
        revision: 7,
    }
}

fn claims() -> ClaimsResourcesV2 {
    ClaimsResourcesV2 {
        settlement_owner: SETTLEMENT_OWNER,
        market_revision: 11,
        owner_position_revision: NO_POSITION_REVISION,
        settlement_position_revision: 5,
        escrow_position_revision: NO_POSITION_REVISION,
    }
}

#[test]
fn the_strand_builds_a_claims_only_packet_the_claims_crate_accepts() {
    let residual = [0_u64, 0, 8];
    let packets = build_strand_packets_v1(context(), 3, &residual, claims())
        .expect("a residual at one zero-priced outcome strands");
    assert!(
        packets.custody.is_none(),
        "the strand moves no collateral, so it carries no Custody leg",
    );
    let packet = packets.claims.expect("the strand IS a Claims leg");
    let plan = packet.plan().expect("the built request decodes");
    assert_eq!(plan.action(), ClaimsAction::StrandResidual);
    assert_eq!(plan.source_owner(), SETTLEMENT_OWNER);
    assert_eq!(
        plan.destination_owner(),
        [0; 32],
        "a burn pays nobody, so naming a destination would claim a movement \
         the burn does not make",
    );
    for (outcome, expected) in residual.iter().enumerate() {
        let index = u32::try_from(outcome).expect("outcome index");
        assert_eq!(plan.quantity(index).expect("quantity"), *expected);
    }
}

#[test]
fn a_close_that_strands_nothing_builds_no_claims_leg() {
    // The plan says `claims_active = false` for a clearing with no residual,
    // and the builder refuses to manufacture a leg for it: an all-zero burn
    // would spend a CPI and a Position revision to change nothing.
    assert_eq!(
        build_strand_packets_v1(context(), 3, &[0, 0, 0], claims()),
        Err(ChildPacketError::Coordinate),
    );
}

#[test]
fn a_residual_of_the_wrong_width_is_refused_by_name() {
    assert_eq!(
        build_strand_packets_v1(context(), 3, &[0, 8], claims()),
        Err(ChildPacketError::Coordinate),
    );
}

#[test]
fn the_claims_plan_admits_an_uneven_strand_and_still_refuses_an_uneven_merge() {
    // The exact pair that makes the strand's exception real. Both plans carry
    // the same uneven quantity vector out of one Position; the strand is the
    // one action for which non-uniformity is the point, and every complete-set
    // action still refuses it.
    let quantities: [u8; 24] = {
        let mut bytes = [0_u8; 24];
        bytes[16..24].copy_from_slice(&8_u64.to_le_bytes());
        bytes
    };
    ClaimsPlanV1::new(
        ClaimsAction::StrandResidual,
        CallerRole::Trading,
        RELEASE_SET,
        MARKET,
        CANDIDATE,
        SETTLEMENT_OWNER,
        [0; 32],
        11,
        5,
        NO_POSITION_REVISION,
        3,
        &quantities,
    )
    .expect("an uneven burn is exactly what the strand is");
    assert_eq!(
        ClaimsPlanV1::new(
            ClaimsAction::MergeCompleteSet,
            CallerRole::Trading,
            RELEASE_SET,
            MARKET,
            CANDIDATE,
            SETTLEMENT_OWNER,
            [0; 32],
            11,
            5,
            NO_POSITION_REVISION,
            3,
            &quantities,
        )
        .err(),
        Some(ClaimsError::InvalidQuantityVector),
    );
}
