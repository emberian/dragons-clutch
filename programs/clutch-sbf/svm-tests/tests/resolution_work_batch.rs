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
    resolution_work_common::{send, snapshot, succeed, succeed_with_limit, Scenario},
    solana_instruction::Instruction,
    solana_program_test::tokio,
    solana_signer::Signer,
};

/// Measured batch sizes.  Twelve is the largest probe: at the measured
/// ~82-96k CU per Fold instruction, twelve folds stay under the 1,120,000-CU
/// raw admission bound implied by the 1,400,000-CU transaction ceiling with
/// 5/4 headroom, while thirteen worst-case folds would not.
const BATCH_SIZES: [u8; 4] = [2, 4, 8, 12];

fn singleton_folds(
    scenario: &Scenario,
    work: &ResolutionWorkAccountV1,
    count: u8,
) -> Vec<Instruction> {
    (0..u64::from(count))
        .map(|offset| scenario.fold(work, work.next_bucket + offset, 1))
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
            worker_before
                + u64::from(batch) * resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1
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
    torn[INVALID_INDEX] = scenario.fold(
        &work,
        work.next_bucket + INVALID_INDEX as u64 + 1,
        1,
    );
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
