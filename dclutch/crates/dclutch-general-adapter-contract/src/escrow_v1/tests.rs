//! Hostile coverage for the escrow's compartment authority and its balances.
//!
//! The first two tests are the ones that would have caught the defect this
//! module exists to retire: they read the compartments out of the EMITTED
//! artifact bytes and compare them against the table the packet builder reads.
//! Neither side is restated; both are executed.

use std::vec;
use std::vec::Vec;

use dclutch_custody_contract::{CustodyRequestV1, OperationV1};
use dclutch_effect_kernel::v3::ProgramV3;
use dclutch_general_config_contract::root::GeneralRootV2;

use super::*;
use crate::candidate_v1::{GeneralCandidateStatusV1, GeneralCandidateV1};
use crate::collection_v1::{
    GeneralBatchOpeningV1, GeneralOrderHeaderV1, GeneralOrderStateV1, MakerFundingV1,
    general_order_len_v1,
};
use crate::effect_artifacts_v3::{
    GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3, encode_general_effect_program_v3_atomic,
    general_effect_instruction_count_v3, general_effect_program_bytes_v3,
    general_effect_template_bytes_v3,
};
use crate::runtime_width::{CandidateHeaderV2, CandidateV2, candidate_len};

const WIDTH: u32 = 3;
const PRICE_SCALE: u64 = 100;
const COLLECTION_CLOSE: u64 = 1_000;
const SETTLEMENT_CLOSE: u64 = 2_000;
const ADMISSION_SLOT: u64 = 10;
const SUBMISSION_SLOT: u64 = 1_100;
const PAGE_REVISION: u64 = 9;
const REWARD_RATE: u64 = 5_000;
const ROW_COUNT: u32 = 2;
const RENT_FLOOR: u64 = 2_282_880;
const MAX_LOTS: u64 = 10;
const MAX_QUOTE_DEBIT_PER_LOT: u64 = 5;
/// `MAX_LOTS * MAX_QUOTE_DEBIT_PER_LOT`, the exact worst case one order escrows.
const QUOTE_RESERVE: u64 = MAX_LOTS * MAX_QUOTE_DEBIT_PER_LOT;

/// The seven actions whose artifact triple is authored today.
const AUTHORED: [Action; 7] = [
    Action::Consider,
    Action::Freeze,
    Action::InitializeSettlement,
    Action::Collect,
    Action::Materialize,
    Action::Distribute,
    Action::Close,
];

fn id(low: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value[0] = low;
    value
}

// ---------------------------------------------------------------------------
// The compartment authority, read out of the emitted artifact
// ---------------------------------------------------------------------------

/// Emit one action's real EffectProgram V3 body.
fn effect_artifact(action: Action) -> Vec<u8> {
    let (fixed, item) = general_effect_instruction_count_v3(action);
    let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; fixed + item];
    let mut templates = vec![0_u8; general_effect_template_bytes_v3(action)];
    let len = general_effect_program_bytes_v3(action).expect("program width");
    let mut scratch = vec![0_u8; len];
    let mut output = vec![0_u8; len];
    encode_general_effect_program_v3_atomic(
        action,
        &mut instructions,
        &mut templates,
        &mut scratch,
        &mut output,
    )
    .expect("effect program");
    output
}

/// Read the compartments off the sole Custody `Transfer` template one action
/// emits, without restating which route index carries it.
fn emitted_transfer_compartments(action: Action) -> Option<(CompartmentV1, CompartmentV1)> {
    let bytes = effect_artifact(action);
    let program = ProgramV3::decode(&bytes).expect("effect program decodes");
    let mut found = None;
    for index in 0..program.route_count() {
        let Ok((template, _)) = program.route_template(index) else {
            continue;
        };
        let Ok(request) = CustodyRequestV1::decode(template) else {
            continue;
        };
        if request.operation != OperationV1::Transfer {
            continue;
        }
        assert!(
            found.is_none(),
            "action {action:?} emits more than one Custody Transfer template"
        );
        found = Some((request.source_compartment, request.destination_compartment));
    }
    found
}

/// **The test the tree did not have, and the reason the escrow ruling was only
/// half landed.**
///
/// Decision 0010 §2 moved `Collect` to `Settlement(order_id) ->
/// Settlement(candidate_id)`, and the ruling reached the packet builder and the
/// batch record but not the artifact. The artifact kept the pre-ruling
/// `External -> Settlement` literal, and because only `Materialize` has its
/// compartment bytes patched at runtime, that literal is what a chain-executed
/// `Collect` carried. Custody's own `Transfer` validation makes the two
/// readings mutually exclusive, so a frame either side accepted the other
/// refused.
///
/// This compares the EMITTED bytes against the one table both sides now read.
#[test]
fn every_emitted_custody_template_carries_the_compartments_the_one_table_names() {
    for action in AUTHORED {
        let emitted = emitted_transfer_compartments(action);
        let declared = general_action_template_compartments_v1(action);
        assert_eq!(
            emitted, declared,
            "action {action:?} emits compartments the escrow table does not name",
        );
    }
}

/// The escrow ruling, stated against the artifact rather than against a comment.
#[test]
fn the_emitted_collect_draws_on_a_settlement_vault_and_never_on_an_external_owner() {
    assert_eq!(
        emitted_transfer_compartments(Action::Collect),
        Some((CompartmentV1::Settlement, CompartmentV1::Settlement)),
    );
    let movement = general_child_custody_movement_v1(GeneralChildEffectV1::CollectCollateral)
        .expect("Collect moves collateral");
    assert_eq!(movement.source_context, VaultContextV1::Order);
    assert_eq!(movement.destination_context, VaultContextV1::Candidate);
}

/// Custody ties the compartment tag and the vault context together; a table row
/// that violates that pairing would be refused by a child program at runtime
/// rather than here.
#[test]
fn every_named_movement_satisfies_custodys_own_transfer_shape() {
    for effect in [
        GeneralChildEffectV1::CollectClaims,
        GeneralChildEffectV1::CollectCollateral,
        GeneralChildEffectV1::MintCompleteSet,
        GeneralChildEffectV1::MergeCompleteSet,
        GeneralChildEffectV1::DistributeClaims,
        GeneralChildEffectV1::DistributeCollateral,
        GeneralChildEffectV1::PaySurplus,
        GeneralChildEffectV1::EscrowClaims,
        GeneralChildEffectV1::EscrowCollateral,
        GeneralChildEffectV1::ReleaseClaims,
        GeneralChildEffectV1::ReleaseCollateral,
    ] {
        let Some(movement) = general_child_custody_movement_v1(effect) else {
            assert!(
                !effect.moves_collateral(),
                "collateral effect {effect:?} names no movement"
            );
            continue;
        };
        assert!(
            movement.is_custody_admissible(),
            "movement for {effect:?} pairs a compartment with the wrong context kind",
        );
        assert!(
            effect.moves_collateral(),
            "{effect:?} named a vault movement"
        );
    }
}

/// An order's escrow and a candidate's inventory share one compartment tag and
/// differ only by context -- which is decision 0010 §2's argument for declining
/// a new tag, now checkable.
#[test]
fn the_escrow_and_the_inventory_are_one_pool_separated_by_a_seed() {
    let collect = general_child_custody_movement_v1(GeneralChildEffectV1::CollectCollateral)
        .expect("collect");
    assert_eq!(collect.source_compartment, collect.destination_compartment);
    assert_ne!(collect.source_context, collect.destination_context);
}

// ---------------------------------------------------------------------------
// The work escrow, physically
// ---------------------------------------------------------------------------

fn candidate_opening() -> GeneralCandidateOpeningV1 {
    GeneralCandidateOpeningV1 {
        outcome_count: WIDTH,
        page_count: 2,
        page_revision: PAGE_REVISION,
        submitted_slot: SUBMISSION_SLOT,
        candidate_id: id(0x51),
        batch_id: id(0x52),
        solver_id: id(40),
        row_count: ROW_COUNT,
        reward_rate_lamports: REWARD_RATE,
    }
}

fn active_root() -> GeneralRootV2 {
    GeneralRootV2::active(id(1), id(2), 7).expect("active root")
}

fn batch_opening() -> GeneralBatchOpeningV1 {
    GeneralBatchOpeningV1 {
        outcome_count: WIDTH,
        sequence: 0,
        generation: 7,
        market: id(1),
        product_id: id(3),
        config_id: id(2),
        price_scale: PRICE_SCALE,
        collection_close_slot: COLLECTION_CLOSE,
        settlement_close_slot: SETTLEMENT_CLOSE,
        max_orders: 4,
    }
}

fn open_batch(root: &mut GeneralRootV2) -> GeneralBatchV1 {
    let revision = root.revision();
    GeneralBatchV1::open(root, batch_opening(), revision, ADMISSION_SLOT).expect("open batch")
}

/// One maker's order, placed and escrowed against a live batch.
fn place(batch: &mut GeneralBatchV1, owner: u8, nonce: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; general_order_len_v1(WIDTH).expect("order width")];
    GeneralOrderV1::encode_into(
        GeneralOrderHeaderV1 {
            outcome_count: WIDTH,
            nonce,
            owner_id: id(owner),
            market: id(1),
            batch_id: batch.batch_id(),
            generation: 7,
            max_lots: MAX_LOTS,
            max_quote_debit_per_lot: MAX_QUOTE_DEBIT_PER_LOT,
            valid_until_slot: SETTLEMENT_CLOSE,
        },
        &[1, 0, 0],
        &[0, 1, 0],
        GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::Placed,
            admitted_slot: ADMISSION_SLOT,
            released_slot: 0,
        },
        &mut bytes,
    )
    .expect("order record");
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let claims: Vec<u64> = (0..WIDTH)
        .map(|index| order.claim_reserve(index).expect("reserve"))
        .collect();
    batch
        .admit(
            order,
            MakerFundingV1 {
                owner_id: id(owner),
                available_quote: 1_000,
                available_claims: &claims,
            },
            ADMISSION_SLOT,
        )
        .expect("admit and escrow");
    bytes
}

fn candidate_bytes(batch_id: [u8; 32]) -> Vec<u8> {
    let mut bytes = vec![0_u8; candidate_len(WIDTH).expect("candidate width")];
    let header = CandidateHeaderV2 {
        outcome_count: WIDTH,
        page_count: 2,
        candidate_coordinate: 1,
        price_scale: PRICE_SCALE,
        candidate_id: id(0xff),
        product_id: id(3),
        batch_id,
    };
    let prices = [50_u64, 30, 20];
    CandidateV2::encode_into(header, &prices, &mut bytes).expect("draft candidate");
    let identity =
        crate::candidate_v1::general_candidate_identity_v1(&bytes).expect("candidate identity");
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id: identity,
            ..header
        },
        &prices,
        &mut bytes,
    )
    .expect("addressed candidate");
    bytes
}

/// A real closed batch and a real submission funded to its exact capacity.
fn submitted() -> (GeneralBatchV1, Vec<u8>, GeneralCandidateV1) {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let order = place(&mut batch, 4, 1);
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close batch");
    let candidate = candidate_bytes(batch.batch_id());
    let opening = GeneralCandidateOpeningV1 {
        batch_id: batch.batch_id(),
        ..candidate_opening()
    };
    let submission = GeneralCandidateV1::submit(
        batch,
        CandidateV2::decode(&candidate).expect("candidate"),
        PAGE_REVISION,
        ROW_COUNT,
        REWARD_RATE,
        id(40),
        opening.work_capacity().expect("work capacity"),
        SUBMISSION_SLOT,
    )
    .expect("submit");
    (batch, order, submission)
}

/// Advance one submission's escrow accounting as if `cranks` rows were paid.
///
/// The record layer is the authority for the layout, so this round-trips
/// through the canonical bytes rather than reaching into private state.
fn spend(submission: GeneralCandidateV1, cranks: u32) -> GeneralCandidateV1 {
    let mut bytes = submission.to_bytes();
    let spent = u64::from(cranks) * submission.opening().reward_rate_lamports;
    let remaining = submission.state().verification_remaining - spent;
    bytes[192..200].copy_from_slice(&remaining.to_le_bytes());
    GeneralCandidateV1::decode(&bytes).expect("spent submission")
}

fn observation(escrow: u64, beneficiary: u64) -> WorkEscrowObservationV1 {
    WorkEscrowObservationV1 {
        escrow_lamports: escrow,
        rent_floor: RENT_FLOOR,
        beneficiary_lamports: beneficiary,
    }
}

fn funded_lamports(submission: GeneralCandidateV1) -> u64 {
    work_escrow_required_lamports_v1(submission, RENT_FLOOR).expect("required")
}

/// **The gap decision 0010 §6 item 3 did not name.** A submission whose account
/// holds nothing at all satisfies `validate_capitalization`, because that
/// function compares the record against itself. The physical conjunct is the
/// only thing that can tell a funded escrow from a claimed one.
#[test]
fn an_escrow_that_holds_nothing_re_proves_its_own_accounting_and_still_refuses() {
    let (_, _, submission) = submitted();
    submission
        .validate_capitalization(0)
        .expect("the record is self-consistent");
    assert_eq!(
        authenticate_work_escrow_v1(submission, 0, observation(0, 0)),
        Err(GeneralEscrowErrorV1::Uncapitalized),
    );
    authenticate_work_escrow_v1(submission, 0, observation(funded_lamports(submission), 0))
        .expect("a funded escrow authenticates");
}

#[test]
fn submission_funding_is_exact_in_both_directions() {
    let opening = candidate_opening();
    let required = RENT_FLOOR + opening.work_capacity().expect("capacity");
    let plan = WorkEscrowFundingPlanV1::new(opening, RENT_FLOOR, required, 0).expect("funding");
    assert_eq!(plan.escrow_after(), required);
    assert_eq!(plan.solver_after(), 0);
    plan.validate_post(0, required).expect("exact post");
    // One lamport short of the requirement is unfunded, not a partial fund.
    assert_eq!(
        WorkEscrowFundingPlanV1::new(opening, RENT_FLOOR, required - 1, 0).err(),
        Some(GeneralEscrowErrorV1::Unfunded),
    );
    // Over-funding is not a donation a prepaid compartment may keep: the plan
    // still moves exactly the requirement, and a post that kept the surplus in
    // the escrow refuses.
    let over = WorkEscrowFundingPlanV1::new(opening, RENT_FLOOR, required + 7, 0).expect("funding");
    assert_eq!(over.escrow_after(), required);
    assert_eq!(over.solver_after(), 7);
    assert_eq!(
        over.validate_post(0, required + 7),
        Err(GeneralEscrowErrorV1::PostconditionMismatch),
    );
}

#[test]
fn a_submission_account_that_is_not_vacant_refuses_to_be_funded_again() {
    let opening = candidate_opening();
    assert_eq!(
        WorkEscrowFundingPlanV1::new(opening, RENT_FLOOR, u64::MAX / 2, 1).err(),
        Some(GeneralEscrowErrorV1::Unfunded),
    );
}

/// The funded-permissionless-walk property, physically: performing the verb
/// moves the reward out of an account the protocol already held.
#[test]
fn every_crank_is_paid_out_of_the_candidates_own_escrow() {
    let (_, _, submission) = submitted();
    let before = observation(funded_lamports(submission), 100);
    let successor = spend(submission, 1);
    let reward = WorkRewardV1 {
        lamports: REWARD_RATE,
        compartment: WorkCompartmentV1::Verification,
    };
    let plan = WorkEscrowDrawPlanV1::new(before, successor, 1, reward).expect("draw");
    assert_eq!(plan.escrow_after(), before.escrow_lamports - REWARD_RATE);
    assert_eq!(plan.beneficiary_after(), 100 + REWARD_RATE);
    plan.validate_post(plan.escrow_after(), plan.beneficiary_after())
        .expect("exact post");
    assert_eq!(
        plan.validate_post(plan.escrow_after(), plan.beneficiary_after() + 1),
        Err(GeneralEscrowErrorV1::PostconditionMismatch),
    );
}

/// HOSTILE. A crank whose record says one thing and whose account says another.
#[test]
fn a_draw_whose_successor_record_disagrees_with_the_balance_refuses() {
    let (_, _, submission) = submitted();
    let before = observation(funded_lamports(submission), 0);
    // The successor spent two cranks; the movement pays for one.
    let successor = spend(submission, 2);
    let reward = WorkRewardV1 {
        lamports: REWARD_RATE,
        compartment: WorkCompartmentV1::Verification,
    };
    assert_eq!(
        WorkEscrowDrawPlanV1::new(before, successor, 2, reward).err(),
        Some(GeneralEscrowErrorV1::Uncapitalized),
    );
}

/// HOSTILE. A crank drawn past its compartment.
#[test]
fn a_crank_cannot_be_drawn_past_the_work_the_escrow_was_sized_for() {
    let (_, _, submission) = submitted();
    // Every verification crank already spent: the compartment holds only the
    // cleanup reward, and a verification draw against it has nothing to take.
    let spent = spend(submission, ROW_COUNT + 1);
    let before = observation(funded_lamports(spent), 0);
    let reward = WorkRewardV1 {
        lamports: REWARD_RATE,
        compartment: WorkCompartmentV1::Verification,
    };
    assert_eq!(
        WorkEscrowDrawPlanV1::new(before, spent, ROW_COUNT, reward).err(),
        Some(GeneralEscrowErrorV1::Uncapitalized),
    );
}

/// HOSTILE. A crank paid out of the rent floor.
///
/// The floor is not a compartment. Paying out of it leaves the record
/// collectable, and the refusal would otherwise surface later, on an unrelated
/// instruction, as a rent failure nobody could attribute to a crank.
#[test]
fn a_crank_cannot_be_paid_out_of_the_rent_floor() {
    let (_, _, submission) = submitted();
    let successor = spend(submission, 1);
    // An escrow holding the rent floor and one reward: the accounting says one
    // crank remains, and the balance says the only lamports left are the floor.
    let before = WorkEscrowObservationV1 {
        escrow_lamports: RENT_FLOOR,
        rent_floor: RENT_FLOOR,
        beneficiary_lamports: 0,
    };
    let reward = WorkRewardV1 {
        lamports: REWARD_RATE,
        compartment: WorkCompartmentV1::Verification,
    };
    assert_eq!(
        WorkEscrowDrawPlanV1::new(before, successor, 1, reward).err(),
        Some(GeneralEscrowErrorV1::Overdrawn),
    );
}

/// Closure conserves, and the rent goes back to the solver who paid it.
#[test]
fn close_out_pays_the_cleanup_crank_and_returns_the_rest_and_the_rent_to_the_solver() {
    let (_, _, submission) = submitted();
    let mut spent = spend(submission, 1);
    let before = observation(funded_lamports(spent), 11);
    let (cleanup, refund) = spent.close_out().expect("close out");
    let plan = WorkEscrowClosePlanV1::new(before, cleanup, refund, 900).expect("close plan");
    assert_eq!(plan.cleanup_reward(), REWARD_RATE);
    assert_eq!(plan.solver_credit(), refund + RENT_FLOOR);
    assert_eq!(
        plan.cleanup_reward() + plan.solver_credit(),
        plan.escrow_before(),
    );
    plan.validate_post(0, plan.cranker_after(), plan.solver_after())
        .expect("exact post");
    // A close that leaves anything behind is a fourth party to a three-way move.
    assert_eq!(
        plan.validate_post(1, plan.cranker_after(), plan.solver_after()),
        Err(GeneralEscrowErrorV1::PostconditionMismatch),
    );
}

/// HOSTILE. The cleanup crank must come out of the cleanup compartment.
#[test]
fn a_close_paid_out_of_the_verification_compartment_refuses() {
    let (_, _, submission) = submitted();
    let before = observation(funded_lamports(submission), 0);
    assert_eq!(
        WorkEscrowClosePlanV1::new(
            before,
            WorkRewardV1 {
                lamports: REWARD_RATE,
                compartment: WorkCompartmentV1::Verification,
            },
            submission.state().verification_remaining,
            0,
        )
        .err(),
        Some(GeneralEscrowErrorV1::Substitution),
    );
}

/// HOSTILE. A second close finds an empty account and cannot conserve.
#[test]
fn a_second_close_out_has_nothing_to_conserve() {
    let (_, _, submission) = submitted();
    let mut spent = spend(submission, 1);
    let (cleanup, refund) = spent.close_out().expect("close out");
    assert_eq!(spent.state().status, GeneralCandidateStatusV1::Submitted);
    // The record refuses the second draw on its own.
    assert!(spent.close_out().is_err());
    // And so does the balance: the account is at zero after the first move.
    assert_eq!(
        WorkEscrowClosePlanV1::new(observation(0, 0), cleanup, refund, 0).err(),
        Some(GeneralEscrowErrorV1::Uncapitalized),
    );
}

// ---------------------------------------------------------------------------
// The order escrow, physically
// ---------------------------------------------------------------------------

fn order_observation(context: [u8; 32], vault: u64, maker: u64) -> OrderEscrowObservationV1 {
    OrderEscrowObservationV1 {
        escrow_context: context,
        vault_quote_atoms: vault,
        maker_quote_atoms: maker,
    }
}

#[test]
fn admission_moves_the_makers_worst_case_into_a_vault_keyed_by_the_order() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = place(&mut batch, 4, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    // `place` already admitted; re-run the transition on a fresh batch to hold
    // the escrow value this movement is built from.
    let escrow = OrderEscrowV1 {
        order_id: order.order_id(),
        owner_id: order.header().owner_id,
        outcome_count: WIDTH,
        quote_atoms: QUOTE_RESERVE,
        direction: EscrowDirectionV1::Deposit,
    };
    let plan = OrderEscrowPlanV1::new(
        batch,
        order,
        escrow,
        order_observation(order.order_id(), 0, 1_000),
    )
    .expect("deposit plan");
    assert_eq!(plan.vault_after(), QUOTE_RESERVE);
    assert_eq!(plan.maker_after(), 1_000 - QUOTE_RESERVE);
    plan.validate_post(plan.vault_after(), plan.maker_after())
        .expect("exact post");
    for outcome in 0..WIDTH {
        authenticate_order_escrow_claims_v1(order, EscrowDirectionV1::Deposit, outcome, 0)
            .expect("a fresh escrow Position holds nothing");
    }
}

/// HOSTILE. Cross-order escrow: one maker's collateral in another's movement.
///
/// This is the property decision 0010 §2 called "a property of the address".
/// The address is only a property if something checks that the vault presented
/// is the one the order names.
#[test]
fn an_escrow_keyed_by_another_order_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let first = place(&mut batch, 4, 1);
    let second = place(&mut batch, 5, 2);
    let order = GeneralOrderV1::decode(&first).expect("order");
    let other = GeneralOrderV1::decode(&second).expect("other order");
    assert_ne!(order.order_id(), other.order_id());
    let escrow = OrderEscrowV1 {
        order_id: order.order_id(),
        owner_id: order.header().owner_id,
        outcome_count: WIDTH,
        quote_atoms: QUOTE_RESERVE,
        direction: EscrowDirectionV1::Deposit,
    };
    assert_eq!(
        OrderEscrowPlanV1::new(
            batch,
            order,
            escrow,
            order_observation(other.order_id(), 0, 1_000),
        )
        .err(),
        Some(GeneralEscrowErrorV1::Substitution),
    );
}

/// HOSTILE. Cross-batch escrow: an order bound to another batch entirely.
#[test]
fn an_order_from_another_batch_cannot_reach_this_batchs_escrow() {
    let mut root = active_root();
    let mut first = open_batch(&mut root);
    let bytes = place(&mut first, 4, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    // A second batch at the next sequence: a different identity, same shape.
    let revision = root.revision();
    first.close(&mut root, revision).expect("close first");
    let revision = root.revision();
    let second = GeneralBatchV1::open(
        &mut root,
        GeneralBatchOpeningV1 {
            sequence: 1,
            ..batch_opening()
        },
        revision,
        ADMISSION_SLOT,
    )
    .expect("second batch");
    assert_ne!(first.batch_id(), second.batch_id());
    let escrow = OrderEscrowV1 {
        order_id: order.order_id(),
        owner_id: order.header().owner_id,
        outcome_count: WIDTH,
        quote_atoms: QUOTE_RESERVE,
        direction: EscrowDirectionV1::Refund,
    };
    assert_eq!(
        OrderEscrowPlanV1::new(
            second,
            order,
            escrow,
            order_observation(order.order_id(), QUOTE_RESERVE, 0),
        )
        .err(),
        Some(GeneralEscrowErrorV1::Substitution),
    );
}

/// HOSTILE. A double refund: the second finds the vault already empty.
#[test]
fn a_refunded_escrow_cannot_be_refunded_again() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = place(&mut batch, 4, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let escrow = batch
        .cancel(order, order.header().owner_id, ADMISSION_SLOT)
        .expect("cancel");
    let plan = OrderEscrowPlanV1::new(
        batch,
        order,
        escrow,
        order_observation(order.order_id(), QUOTE_RESERVE, 0),
    )
    .expect("refund plan");
    assert_eq!(plan.vault_after(), 0);
    assert_eq!(plan.maker_after(), QUOTE_RESERVE);
    // The same movement against the post-refund vault refuses: the whole
    // reserve is no longer there to return.
    assert_eq!(
        OrderEscrowPlanV1::new(
            batch,
            order,
            escrow,
            order_observation(order.order_id(), 0, QUOTE_RESERVE),
        )
        .err(),
        Some(GeneralEscrowErrorV1::Uncapitalized),
    );
}

/// A post-window release quotes no amount: what remains IS the refund, and the
/// bound the address was said to give for free is now checked.
#[test]
fn a_residual_release_returns_the_balance_and_can_never_exceed_the_reserve() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = place(&mut batch, 4, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let escrow = batch
        .release(order, SETTLEMENT_CLOSE)
        .expect("post-window release");
    assert_eq!(escrow.quote_atoms, 0);
    let partly_collected = QUOTE_RESERVE - 7;
    let plan = OrderEscrowPlanV1::new(
        batch,
        order,
        escrow,
        order_observation(order.order_id(), partly_collected, 3),
    )
    .expect("residual plan");
    assert_eq!(plan.quote_atoms(), partly_collected);
    assert_eq!(plan.vault_after(), 0);
    assert_eq!(plan.maker_after(), 3 + partly_collected);
    // A vault holding more than the order ever reserved is not this order's
    // escrow, whatever its address says.
    assert_eq!(
        OrderEscrowPlanV1::new(
            batch,
            order,
            escrow,
            order_observation(order.order_id(), QUOTE_RESERVE + 1, 0),
        )
        .err(),
        Some(GeneralEscrowErrorV1::Uncapitalized),
    );
    for outcome in 0..WIDTH {
        let reserve = order.claim_reserve(outcome).expect("reserve");
        authenticate_order_escrow_claims_v1(order, EscrowDirectionV1::Residual, outcome, reserve)
            .expect("an untouched escrow releases in full");
        assert_eq!(
            authenticate_order_escrow_claims_v1(
                order,
                EscrowDirectionV1::Residual,
                outcome,
                reserve + 1,
            ),
            Err(GeneralEscrowErrorV1::Uncapitalized),
        );
    }
}

/// HOSTILE. Settle without escrow.
///
/// The pure transition refuses a row whose order is not `Placed`; what it
/// cannot see is whether the vault the row names holds the debit. Without this
/// a candidate verified against an escrow could settle against an empty one.
#[test]
fn a_collect_cannot_draw_on_an_escrow_that_does_not_hold_the_debit() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = place(&mut batch, 4, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close batch");
    authenticate_collect_from_escrow_v1(
        batch,
        order,
        order_observation(order.order_id(), QUOTE_RESERVE, 0),
        QUOTE_RESERVE,
    )
    .expect("a funded escrow covers its own worst case");
    assert_eq!(
        authenticate_collect_from_escrow_v1(
            batch,
            order,
            order_observation(order.order_id(), QUOTE_RESERVE - 1, 0),
            QUOTE_RESERVE,
        ),
        Err(GeneralEscrowErrorV1::Overdrawn),
    );
    // And a Collect that names another order's vault reaches nothing.
    assert_eq!(
        authenticate_collect_from_escrow_v1(
            batch,
            order,
            order_observation(id(0x77), QUOTE_RESERVE, 0),
            1,
        ),
        Err(GeneralEscrowErrorV1::Substitution),
    );
}
