//! Real-SBF promotion campaign for resumable occupation resolution.

mod resolution_work_common;

use {
    clutch_sbf::instructions::resolution_work,
    clutch_solana_layout::occupation_resolution::OccupationResolutionAccount,
    resolution_work_common::{
        send, snapshot, succeed, succeed_with_limit, Scenario, INCINERATOR, POST_RESERVE_DONATION,
        POST_WORK_DONATION, RESERVE_DONATION, SPAN, SUBSTITUTE_ARCHIVE, SUBSTITUTE_SINK,
        WORK_DONATION,
    },
    solana_instruction::AccountMeta,
    solana_program_test::tokio,
    solana_signer::Signer,
    solana_system_interface::instruction as system_instruction,
};

#[tokio::test]
async fn begin_fold_finalize_is_prefund_safe_replay_safe_and_byte_exact() {
    let mut staged = Scenario::start(2, SPAN, false).await;
    let actor = staged.actor.insecure_clone();
    let worker = staged.worker.insecure_clone();
    let keeper = staged.keeper.insecure_clone();
    let work_address = staged.work;
    let reserve_address = staged.reserve;
    let initial_actor = staged.account(actor.pubkey()).await.unwrap().lamports;
    succeed(
        &mut staged,
        &[
            system_instruction::transfer(&actor.pubkey(), &work_address, WORK_DONATION),
            system_instruction::transfer(&actor.pubkey(), &reserve_address, RESERVE_DONATION),
        ],
        &[&actor],
    )
    .await;
    let prefund_watch = [work_address, reserve_address, actor.pubkey()];
    let before_underfunded = snapshot(&mut staged, &prefund_watch).await;
    let underfunded = staged.begin_with_deposit(6, 100, staged.deposit - 1);
    assert!(send(&mut staged, &[underfunded], &[&actor])
        .await
        .0
        .is_err());
    assert_eq!(
        snapshot(&mut staged, &prefund_watch).await,
        before_underfunded
    );
    let begin = staged.begin(7, 100);
    let begin_cu = succeed_with_limit(
        &mut staged,
        core::slice::from_ref(&begin),
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = staged.work_state().await;
    assert_eq!(
        work.funding.donation_lamports,
        WORK_DONATION + RESERVE_DONATION
    );
    assert_eq!(
        staged.account(staged.work).await.unwrap().lamports,
        staged.work_rent + WORK_DONATION
    );
    assert_eq!(
        staged.account(staged.reserve).await.unwrap().lamports,
        staged.deposit - staged.work_rent + RESERVE_DONATION
    );
    assert_eq!(
        staged.account(actor.pubkey()).await.unwrap().lamports,
        initial_actor - staged.deposit - WORK_DONATION - RESERVE_DONATION
    );
    let replay_watch = [staged.work, staged.reserve, actor.pubkey()];
    let before_replay = snapshot(&mut staged, &replay_watch).await;
    assert!(send(&mut staged, &[begin], &[&actor]).await.0.is_err());
    assert_eq!(snapshot(&mut staged, &replay_watch).await, before_replay);

    let work_pair = [work_address, reserve_address];
    let before_wrong = snapshot(&mut staged, &work_pair).await;
    let wrong_fold = staged.fold(&work, work.next_bucket + 1, 1);
    assert!(send(&mut staged, &[wrong_fold], &[&worker],)
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut staged, &work_pair).await, before_wrong);
    succeed(
        &mut staged,
        &[
            system_instruction::transfer(&actor.pubkey(), &work_address, POST_WORK_DONATION),
            system_instruction::transfer(&actor.pubkey(), &reserve_address, POST_RESERVE_DONATION),
        ],
        &[&actor],
    )
    .await;
    let worker_before = staged.account(worker.pubkey()).await.unwrap().lamports;
    let fold1 = staged.fold(&work, work.next_bucket, 1);
    let fold1_cu = succeed_with_limit(
        &mut staged,
        &[fold1],
        &[&worker],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    assert_eq!(
        staged.account(worker.pubkey()).await.unwrap().lamports,
        worker_before + resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1
    );
    let work = staged.work_state().await;
    assert_eq!(
        work.funding.donation_lamports,
        WORK_DONATION + RESERVE_DONATION + POST_WORK_DONATION + POST_RESERVE_DONATION
    );
    let early_watch = [
        staged.plane.market.address,
        staged.plane.kernel.address,
        staged.plane.supply.address,
        staged.plane.resolution.address,
        staged.work,
        staged.reserve,
    ];
    let before_early = snapshot(&mut staged, &early_watch).await;
    let early_finalize = staged.finalize(&work, actor.pubkey());
    assert!(send(&mut staged, &[early_finalize], &[&keeper],)
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut staged, &early_watch).await, before_early);
    let fold2 = staged.fold(&work, work.next_bucket, 2);
    let fold2_cu = succeed_with_limit(
        &mut staged,
        &[fold2],
        &[&worker],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = staged.work_state().await;
    staged.bank.warp_to_slot(101).unwrap();
    let late_watch = [
        staged.plane.market.address,
        staged.plane.kernel.address,
        staged.plane.supply.address,
        staged.plane.resolution.address,
        staged.work,
        staged.reserve,
        INCINERATOR,
    ];
    let before_expired_complete_abort = snapshot(&mut staged, &late_watch).await;
    let expired_complete_abort = staged.abort(&work, keeper.pubkey());
    assert!(send(&mut staged, &[expired_complete_abort], &[&keeper])
        .await
        .0
        .is_err());
    assert_eq!(
        snapshot(&mut staged, &late_watch).await,
        before_expired_complete_abort
    );
    let before_wrong_sink = snapshot(&mut staged, &late_watch).await;
    let mut wrong_sink = staged.finalize(&work, actor.pubkey());
    let sink_index = wrong_sink.accounts.len() - 2;
    wrong_sink.accounts[sink_index] = AccountMeta::new(SUBSTITUTE_SINK, false);
    assert!(send(&mut staged, &[wrong_sink], &[&keeper])
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut staged, &late_watch).await, before_wrong_sink);
    let keeper_before = staged.account(keeper.pubkey()).await.unwrap().lamports;
    let finalize = staged.finalize(&work, actor.pubkey());
    let finalize_cu = succeed_with_limit(
        &mut staged,
        &[finalize],
        &[&keeper],
        resolution_work::RESOLUTION_WORK_FINALIZE_CU_LIMIT_V1,
    )
    .await;
    assert_eq!(
        staged.account(keeper.pubkey()).await.unwrap().lamports,
        keeper_before + resolution_work::RESOLUTION_WORK_FINALIZE_REWARD_V1
    );
    assert!(staged.account(staged.work).await.is_none());
    assert!(staged.account(staged.reserve).await.is_none());
    assert_eq!(
        staged.account(INCINERATOR).await.unwrap().lamports,
        WORK_DONATION + RESERVE_DONATION + POST_WORK_DONATION + POST_RESERVE_DONATION
    );
    assert_eq!(
        staged.account(actor.pubkey()).await.unwrap().lamports,
        initial_actor
            - WORK_DONATION
            - RESERVE_DONATION
            - POST_WORK_DONATION
            - POST_RESERVE_DONATION
            - (2 * resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1)
            - resolution_work::RESOLUTION_WORK_FINALIZE_REWARD_V1
    );
    let staged_resolution = staged
        .account(staged.plane.resolution.address)
        .await
        .unwrap()
        .data;
    let resolved = OccupationResolutionAccount::decode(&staged_resolution).unwrap();
    assert_eq!(resolved.resolved_slot, 0);

    let mut monolithic = Scenario::start(2, SPAN, false).await;
    let monolithic_actor = monolithic.actor.insecure_clone();
    let monolithic_ix = monolithic.monolithic();
    let monolithic_cu = succeed(&mut monolithic, &[monolithic_ix], &[&monolithic_actor]).await;
    let monolithic_resolution = monolithic
        .account(monolithic.plane.resolution.address)
        .await
        .unwrap()
        .data;
    assert_eq!(staged_resolution, monolithic_resolution);
    for (staged_address, monolithic_address) in [
        (staged.plane.market.address, monolithic.plane.market.address),
        (staged.plane.kernel.address, monolithic.plane.kernel.address),
        (staged.plane.supply.address, monolithic.plane.supply.address),
    ] {
        assert_eq!(
            staged.account(staged_address).await.unwrap().data,
            monolithic.account(monolithic_address).await.unwrap().data
        );
    }
    println!(
        "ResolutionWork SBF CU: Begin={begin_cu} Fold1={fold1_cu} Fold2={fold2_cu} Finalize={finalize_cu} monolithic={monolithic_cu}"
    );
}

#[tokio::test]
async fn expiry_keeper_abort_releases_lock_without_capturing_donations() {
    let mut scenario = Scenario::start(2, SPAN, false).await;
    let actor = scenario.actor.insecure_clone();
    let keeper = scenario.keeper.insecure_clone();
    let work_address = scenario.work;
    let reserve_address = scenario.reserve;
    let initial_actor = scenario.account(actor.pubkey()).await.unwrap().lamports;
    succeed(
        &mut scenario,
        &[
            system_instruction::transfer(&actor.pubkey(), &work_address, WORK_DONATION),
            system_instruction::transfer(&actor.pubkey(), &reserve_address, RESERVE_DONATION),
        ],
        &[&actor],
    )
    .await;
    let begin = scenario.begin(9, 20);
    let begin_cu = succeed_with_limit(
        &mut scenario,
        &[begin],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = scenario.work_state().await;
    let early_abort = scenario.abort(&work, keeper.pubkey());
    assert!(send(&mut scenario, &[early_abort], &[&keeper],)
        .await
        .0
        .is_err());
    scenario.bank.warp_to_slot(21).unwrap();
    let keeper_before = scenario.account(keeper.pubkey()).await.unwrap().lamports;
    let expired_abort = scenario.abort(&work, keeper.pubkey());
    let abort_cu = succeed_with_limit(
        &mut scenario,
        &[expired_abort],
        &[&keeper],
        resolution_work::RESOLUTION_WORK_ABORT_CU_LIMIT_V1,
    )
    .await;
    assert_eq!(
        scenario.account(keeper.pubkey()).await.unwrap().lamports,
        keeper_before + resolution_work::RESOLUTION_WORK_ABORT_REWARD_V1
    );
    assert!(scenario.account(scenario.work).await.is_none());
    assert!(scenario.account(scenario.reserve).await.is_none());
    assert_eq!(
        scenario.account(INCINERATOR).await.unwrap().lamports,
        WORK_DONATION + RESERVE_DONATION
    );
    assert_eq!(
        scenario.account(actor.pubkey()).await.unwrap().lamports,
        initial_actor
            - WORK_DONATION
            - RESERVE_DONATION
            - resolution_work::RESOLUTION_WORK_ABORT_REWARD_V1
    );
    let reopen = scenario.begin(10, 100);
    let reopen_cu = succeed_with_limit(
        &mut scenario,
        &[reopen],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    assert_ne!(
        scenario.work_state().await.work_commitment,
        work.work_commitment
    );
    println!(
        "ResolutionWork SBF CU: BeginAbort={begin_cu} AbortExpired={abort_cu} BeginReopen={reopen_cu}"
    );
}

#[tokio::test]
async fn source_spec_bump_and_same_domain_archive_substitution_refuse_atomically() {
    let mut malformed = Scenario::start(2, SPAN, true).await;
    let actor = malformed.actor.insecure_clone();
    let malformed_begin = malformed.begin(11, 100);
    assert!(send(&mut malformed, &[malformed_begin], &[&actor])
        .await
        .0
        .is_err());
    assert!(malformed.account(malformed.work).await.is_none());
    assert!(malformed.account(malformed.reserve).await.is_none());

    let mut scenario = Scenario::start(2, SPAN, false).await;
    let actor = scenario.actor.insecure_clone();
    let worker = scenario.worker.insecure_clone();
    let begin = scenario.begin(12, 100);
    succeed_with_limit(
        &mut scenario,
        &[begin],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = scenario.work_state().await;
    let work_pair = [scenario.work, scenario.reserve];
    let before = snapshot(&mut scenario, &work_pair).await;
    let mut wrong = scenario.fold(&work, work.next_bucket, 1);
    wrong.accounts[4] = AccountMeta::new_readonly(SUBSTITUTE_ARCHIVE, false);
    assert!(send(&mut scenario, &[wrong], &[&worker]).await.0.is_err());
    assert_eq!(snapshot(&mut scenario, &work_pair).await, before);
}

#[tokio::test]
async fn every_bounded_fold_size_has_a_real_sbf_cost_row() {
    for record_count in 1_u8..=4 {
        let mut scenario = Scenario::start(2, u64::from(record_count), false).await;
        let actor = scenario.actor.insecure_clone();
        let worker = scenario.worker.insecure_clone();
        let begin = scenario.begin(20 + record_count, 100);
        let begin_cu = succeed_with_limit(
            &mut scenario,
            &[begin],
            &[&actor],
            resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
        )
        .await;
        let work = scenario.work_state().await;
        let fold = scenario.fold(&work, work.next_bucket, record_count);
        let fold_cu = succeed_with_limit(
            &mut scenario,
            &[fold],
            &[&worker],
            resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
        )
        .await;
        assert_eq!(
            scenario.work_state().await.next_bucket,
            work.end_bucket_exclusive
        );
        println!(
            "ResolutionWork SBF CU: span={record_count} Begin={begin_cu} Fold({record_count})={fold_cu}"
        );
    }
}
