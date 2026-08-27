//! Real-SBF batched-fold campaign: N Fold instructions in one transaction.
//!
//! The per-instruction campaign in `tests/resolution_work.rs` drives one Fold
//! per transaction.  This file measures the per-transaction CU of composing N
//! singleton Fold instructions into a single transaction and holds the batched
//! path to two semantic gates: the final account state must be byte-identical
//! to the same folds driven one per transaction, and one invalid Fold in the
//! middle of a batch must revert the entire transaction back to its prestate.

mod resolution_work_common;

use {
    clutch_sbf::instructions::resolution_work,
    clutch_solana_layout::resolution_work::ResolutionWorkAccountV1,
    resolution_work_common::{
        send, send_as_payer_with_limit, snapshot, succeed, succeed_with_limit, Scenario,
    },
    solana_instruction::Instruction,
    solana_program_test::tokio,
    solana_signer::Signer,
};

const MAX_RECORDS: u64 = 32;
const DENSE_FOLD_WIDTH: u8 = 4;
const DENSE_FOLD_CALLS: u8 = 8;
const FIRST_TRANSACTION_CALLS: u8 = 6;
const SECOND_TRANSACTION_CALLS: u8 = 2;
const PACKET_BUDGET_BYTES: usize = 1_232;

/// Measured batch sizes.  Twelve is the largest probe the *compute* bound
/// admits: at the measured ~82-96k CU per Fold instruction, twelve folds stay
/// under the 1,120,000-CU raw admission bound implied by the 1,400,000-CU
/// transaction ceiling with 5/4 headroom, while thirteen worst-case folds
/// would not.
///
/// Compute is not the binding constraint on the wire, though, and six is here
/// because of the other one.  The keeper's `fold-wire-probe` measured the
/// serialized message at every width and had a real validator's
/// `sendTransaction` agree: six Fold instructions frame at 1,216 bytes and
/// seven do not, at 1,347 against the 1,232-byte packet budget.  A
/// twelve-fold message is 2,002 bytes and cannot be sent at all, so a batch
/// row at width twelve prices a transaction no keeper can submit.  Six is
/// measured so the largest *sendable* batch is a measured row rather than an
/// interpolation between two widths that bracket it.
const BATCH_SIZES: [u8; 5] = [2, 4, 6, 8, 12];

fn singleton_folds(
    scenario: &Scenario,
    work: &ResolutionWorkAccountV1,
    count: u8,
) -> Vec<Instruction> {
    (0..u64::from(count))
        .map(|offset| scenario.fold(work, work.next_bucket + offset, 1))
        .collect()
}

fn dense_folds(scenario: &Scenario, work: &ResolutionWorkAccountV1, count: u8) -> Vec<Instruction> {
    (0..u64::from(count))
        .map(|offset| {
            scenario.fold(
                work,
                work.next_bucket + offset * u64::from(DENSE_FOLD_WIDTH),
                DENSE_FOLD_WIDTH,
            )
        })
        .collect()
}

#[tokio::test]
async fn batched_folds_match_singleton_transactions_byte_exactly() {
    for batch in BATCH_SIZES {
        let span = u64::from(batch);
        let mut batched = Scenario::start(2, span, false).await;
        let batched_actor = batched.actor.insecure_clone();
        let batched_worker = batched.worker.insecure_clone();
        let begin = batched.begin(30 + batch, 100);
        let begin_cu = succeed_with_limit(
            &mut batched,
            core::slice::from_ref(&begin),
            &[&batched_actor],
            resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
        )
        .await;
        let work = batched.work_state().await;
        let worker_before = batched
            .account(batched_worker.pubkey())
            .await
            .unwrap()
            .lamports;
        let folds = singleton_folds(&batched, &work, batch);
        let batch_cu = succeed(&mut batched, &folds, &[&batched_worker]).await;
        let folded = batched.work_state().await;
        assert_eq!(folded.next_bucket, work.end_bucket_exclusive);
        assert_eq!(folded.fold_count, u64::from(batch));
        assert_eq!(
            batched
                .account(batched_worker.pubkey())
                .await
                .unwrap()
                .lamports,
            worker_before + u64::from(batch) * resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1
        );

        let mut singleton = Scenario::start(2, span, false).await;
        let singleton_actor = singleton.actor.insecure_clone();
        let singleton_worker = singleton.worker.insecure_clone();
        let begin = singleton.begin(30 + batch, 100);
        succeed_with_limit(
            &mut singleton,
            core::slice::from_ref(&begin),
            &[&singleton_actor],
            resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
        )
        .await;
        let mut singleton_cu = Vec::new();
        for _ in 0..batch {
            let state = singleton.work_state().await;
            let fold = singleton.fold(&state, state.next_bucket, 1);
            singleton_cu.push(
                succeed_with_limit(
                    &mut singleton,
                    &[fold],
                    &[&singleton_worker],
                    resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
                )
                .await,
            );
        }
        assert_eq!(singleton.work, batched.work);
        assert_eq!(singleton.reserve, batched.reserve);
        let watch = [batched.work, batched.reserve, batched_worker.pubkey()];
        assert_eq!(
            snapshot(&mut batched, &watch).await,
            snapshot(&mut singleton, &watch).await
        );
        let singletons = singleton_cu
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "ResolutionWork SBF CU: batch={batch} Begin={begin_cu} FoldBatch({batch})={batch_cu} SingletonFolds={singletons}"
        );
    }
}

#[tokio::test]
async fn one_invalid_fold_mid_batch_reverts_every_prior_fold() {
    const BATCH: u8 = 8;
    const INVALID_INDEX: usize = 4;
    let mut scenario = Scenario::start(2, u64::from(BATCH), false).await;
    let actor = scenario.actor.insecure_clone();
    let worker = scenario.worker.insecure_clone();
    let begin = scenario.begin(50, 100);
    succeed_with_limit(
        &mut scenario,
        &[begin],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = scenario.work_state().await;
    let watch = [scenario.work, scenario.reserve, worker.pubkey()];
    let prestate = snapshot(&mut scenario, &watch).await;
    let mut torn = singleton_folds(&scenario, &work, BATCH);
    // The fifth instruction expects a cursor one bucket past the position the
    // four preceding folds actually reach, so it must refuse mid-transaction.
    torn[INVALID_INDEX] = scenario.fold(&work, work.next_bucket + INVALID_INDEX as u64 + 1, 1);
    assert!(send(&mut scenario, &torn, &[&worker]).await.0.is_err());
    assert_eq!(snapshot(&mut scenario, &watch).await, prestate);
    assert_eq!(scenario.work_state().await.fold_count, 0);
    // The refused batch wedged nothing: the correct batch still completes.
    let folds = singleton_folds(&scenario, &work, BATCH);
    let retry_cu = succeed(&mut scenario, &folds, &[&worker]).await;
    let folded = scenario.work_state().await;
    assert_eq!(folded.next_bucket, work.end_bucket_exclusive);
    assert_eq!(folded.fold_count, u64::from(BATCH));
    println!(
        "ResolutionWork SBF CU: batch={BATCH} MidBatchInvalidFold=REVERTED FoldBatchRetry({BATCH})={retry_cu}"
    );
}

#[tokio::test]
async fn record_dense_fold4_six_plus_two_matches_eight_transaction_reference() {
    let mut composed = Scenario::start(2, MAX_RECORDS, false).await;
    let actor = composed.actor.insecure_clone();
    let worker = composed.worker.insecure_clone();
    let keeper = composed.keeper.insecure_clone();
    let actor_initial = composed.account(actor.pubkey()).await.unwrap().lamports;
    let begin = composed.begin(70, 100);
    let begin_cu = succeed_with_limit(
        &mut composed,
        &[begin],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let initial_work = composed.work_state().await;
    let initial_reserve_lamports = composed.account(composed.reserve).await.unwrap().lamports;

    let first = dense_folds(&composed, &initial_work, FIRST_TRANSACTION_CALLS);
    let (result, first_cu, first_packet_bytes) =
        send_as_payer_with_limit(&mut composed, &first, &worker, &[], 1_400_000).await;
    result.unwrap();
    assert!(first_packet_bytes <= PACKET_BUDGET_BYTES);
    let after_first = composed.work_state().await;
    assert_eq!(after_first.next_bucket, initial_work.next_bucket + 24);
    assert_eq!(after_first.fold_count, u64::from(FIRST_TRANSACTION_CALLS));
    assert_eq!(
        after_first.funding.rewards_paid,
        u64::from(FIRST_TRANSACTION_CALLS) * resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1
    );

    let second = dense_folds(&composed, &after_first, SECOND_TRANSACTION_CALLS);
    let (result, second_cu, second_packet_bytes) =
        send_as_payer_with_limit(&mut composed, &second, &worker, &[], 1_400_000).await;
    result.unwrap();
    assert!(second_packet_bytes <= PACKET_BUDGET_BYTES);
    let composed_complete = composed.work_state().await;
    assert_eq!(
        composed_complete.next_bucket,
        initial_work.end_bucket_exclusive
    );
    assert_eq!(composed_complete.fold_count, u64::from(DENSE_FOLD_CALLS));
    let fold_rewards =
        u64::from(DENSE_FOLD_CALLS) * resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1;
    assert_eq!(composed_complete.funding.rewards_paid, fold_rewards);
    assert_eq!(composed_complete.funding.charges_paid, 0);
    assert_eq!(
        composed.account(composed.reserve).await.unwrap().lamports,
        initial_reserve_lamports - fold_rewards
    );

    // The separate-transaction reference uses the same Fold(4) ABI and the
    // same keeper-shaped fee payer; only transaction grouping differs.
    let mut reference = Scenario::start(2, MAX_RECORDS, false).await;
    let reference_actor = reference.actor.insecure_clone();
    let reference_worker = reference.worker.insecure_clone();
    let reference_keeper = reference.keeper.insecure_clone();
    let begin = reference.begin(70, 100);
    succeed_with_limit(
        &mut reference,
        &[begin],
        &[&reference_actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let mut reference_fold_cu = Vec::with_capacity(usize::from(DENSE_FOLD_CALLS));
    for _ in 0..DENSE_FOLD_CALLS {
        let state = reference.work_state().await;
        let fold = reference.fold(&state, state.next_bucket, DENSE_FOLD_WIDTH);
        let (result, units, _) = send_as_payer_with_limit(
            &mut reference,
            &[fold],
            &reference_worker,
            &[],
            resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
        )
        .await;
        result.unwrap();
        reference_fold_cu.push(units);
    }

    let pre_finalize_watch = [
        composed.work,
        composed.reserve,
        composed.plane.resolution.address,
    ];
    assert_eq!(
        snapshot(&mut composed, &pre_finalize_watch).await,
        snapshot(&mut reference, &pre_finalize_watch).await,
        "transaction grouping must not change Work, Reserve, or Resolution bytes"
    );

    let actor_before_finalize = composed.account(actor.pubkey()).await.unwrap().lamports;
    let keeper_before_finalize = composed.account(keeper.pubkey()).await.unwrap().lamports;
    let finalize = composed.finalize(&composed_complete, actor.pubkey());
    let finalize_cu = succeed_with_limit(
        &mut composed,
        &[finalize],
        &[&keeper],
        resolution_work::RESOLUTION_WORK_FINALIZE_CU_LIMIT_V1,
    )
    .await;
    let expected_refund =
        composed.deposit - fold_rewards - resolution_work::RESOLUTION_WORK_FINALIZE_REWARD_V1;
    assert_eq!(
        composed.account(actor.pubkey()).await.unwrap().lamports,
        actor_before_finalize + expected_refund
    );
    assert_eq!(
        composed.account(actor.pubkey()).await.unwrap().lamports,
        actor_initial - fold_rewards - resolution_work::RESOLUTION_WORK_FINALIZE_REWARD_V1
    );
    assert_eq!(
        composed.account(keeper.pubkey()).await.unwrap().lamports,
        keeper_before_finalize + resolution_work::RESOLUTION_WORK_FINALIZE_REWARD_V1
    );
    assert!(composed.account(composed.work).await.is_none());
    assert!(composed.account(composed.reserve).await.is_none());

    let reference_complete = reference.work_state().await;
    let reference_actor_before = reference
        .account(reference_actor.pubkey())
        .await
        .unwrap()
        .lamports;
    let finalize = reference.finalize(&reference_complete, reference_actor.pubkey());
    succeed_with_limit(
        &mut reference,
        &[finalize],
        &[&reference_keeper],
        resolution_work::RESOLUTION_WORK_FINALIZE_CU_LIMIT_V1,
    )
    .await;
    assert_eq!(
        reference
            .account(reference_actor.pubkey())
            .await
            .unwrap()
            .lamports,
        reference_actor_before + expected_refund
    );
    let post_finalize_watch = [
        composed.work,
        composed.reserve,
        composed.plane.resolution.address,
    ];
    assert_eq!(
        snapshot(&mut composed, &post_finalize_watch).await,
        snapshot(&mut reference, &post_finalize_watch).await,
        "final Work/Reserve closure and Resolution bytes must be identical"
    );

    println!(
        "ResolutionWork SBF CU: Fold4Plan=[6,2] Begin={begin_cu} Fold4Tx1(6)={first_cu} packet_bytes={first_packet_bytes} Fold4Tx2(2)={second_cu} packet_bytes={second_packet_bytes} Finalize={finalize_cu} RuntimeFoldRewards={fold_rewards} PayerRefund={expected_refund} ReferenceSingletonFold4CU={}",
        reference_fold_cu
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
}

#[tokio::test]
async fn invalid_middle_fold4_reverts_the_record_dense_transaction() {
    const INVALID_INDEX: usize = 3;
    let mut scenario = Scenario::start(2, MAX_RECORDS, false).await;
    let actor = scenario.actor.insecure_clone();
    let worker = scenario.worker.insecure_clone();
    let begin = scenario.begin(71, 100);
    succeed_with_limit(
        &mut scenario,
        &[begin],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = scenario.work_state().await;
    let watch = [scenario.work, scenario.reserve, worker.pubkey()];
    let prestate = snapshot(&mut scenario, &watch).await;
    let mut torn = dense_folds(&scenario, &work, FIRST_TRANSACTION_CALLS);
    torn[INVALID_INDEX] = scenario.fold(
        &work,
        work.next_bucket + INVALID_INDEX as u64 * u64::from(DENSE_FOLD_WIDTH) + 1,
        DENSE_FOLD_WIDTH,
    );
    let (result, units) = send(&mut scenario, &torn, &[&worker]).await;
    assert!(result.is_err());
    assert_eq!(snapshot(&mut scenario, &watch).await, prestate);
    let unchanged = scenario.work_state().await;
    assert_eq!(unchanged.fold_count, 0);
    assert_eq!(unchanged.funding.rewards_paid, 0);

    let retry = dense_folds(&scenario, &work, FIRST_TRANSACTION_CALLS);
    let retry_cu = succeed(&mut scenario, &retry, &[&worker]).await;
    let retried = scenario.work_state().await;
    assert_eq!(retried.next_bucket, work.next_bucket + 24);
    assert_eq!(retried.fold_count, u64::from(FIRST_TRANSACTION_CALLS));
    assert_eq!(
        retried.funding.rewards_paid,
        u64::from(FIRST_TRANSACTION_CALLS) * resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1
    );
    println!(
        "ResolutionWork SBF CU: Fold4Plan=[6,2] MidFold4Invalid=REVERTED failure_cu={units} Fold4Tx1Retry(6)={retry_cu}"
    );
}
