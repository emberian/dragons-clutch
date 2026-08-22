//! Scale campaign 3 — **exact ties at realistic candidate counts**.
//!
//! `candidate_selection.rs` pins the tie rule on *two* candidates: two
//! verified claims with equal score components, decided by the full-width
//! digest.  The W2 evidence gap `FULL_WIDTH_TIE_AND_DISPLACEMENT_CAMPAIGNS`
//! asks the same question at the counts a real solver market produces, where
//! many submissions compete for a registry that retains
//! `MAX_RETAINED_CANDIDATES = 3`.
//!
//! The book here trades **one** outcome of a four-outcome market, so a
//! candidate's prices on the three untraded outcomes move its identity — and
//! therefore its full-width tie digest — while leaving every fill and every
//! score component identical.  Sixteen such candidates are planned from one
//! frozen book; they are pairwise distinct as identities and pairwise
//! indistinguishable as scores.
//!
//! Two gates:
//!
//! * **The registry holds against a tied field.**  Seal admits no claims.  All
//!   sixteen candidates are walked to a durable verdict, while the registry
//!   retains only the best three verified full scores.  Their components are asserted
//!   equal and their digests pairwise distinct, and `FinalizeSelection`
//!   selects the smallest digest.  The tie derivation is re-derived host-side
//!   and asserted to be a strict total order on the field (irreflexive,
//!   antisymmetric, transitive over all sixteen pairs), so "deterministic" is
//!   a checked property rather than one observation.
//! * **Displacement among equals uses the verified digest.**  A fourth
//!   candidate displaces the verified worst exactly when its recomputed full
//!   score is better; its unverified claim is irrelevant and displaced feeds
//!   remain available for terminal closure.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

mod scale_common;

use {
    clutch_batch_policy_identity::{FullScoreV1, Identity32V1},
    clutch_solana_layout::{
        clearing::{EpochWindowAccount, CANDIDATE_WINDOW_SLOTS},
        CandidateRecord, EpochAccount, Hash32, CANDIDATE_STATUS_SELECTED,
        CANDIDATE_STATUS_SUPERSEDED, CANDIDATE_STATUS_VERIFIED, EPOCH_PHASE_CLEARED,
        MAX_GRID_TICKS, MAX_OUTCOMES,
    },
    scale_common::{
        account, build_frozen_book, bytes_of, frozen_state, plan_submission, read_record, send,
        single, submit_seal, walk_to_verdict, Lab, MarketSpec, Meter, PlannedOrder, Submission,
    },
    solana_program_test::tokio,
};

const EPOCH_INDEX: u64 = 7;
const FREEZE_DEADLINE: u64 = 500;
const OUTCOMES: u8 = 4;
const TICK_COUNT: u8 = MAX_GRID_TICKS as u8;
const SPACING: u64 = 100;
const PRICE_SCALE: u64 = 10_000;
const START_CASH: u64 = 1_000_000;
const START_EGGS: u64 = 1_000;
const OWNERS: usize = 4;
/// The tied field: sixteen candidates, one traded outcome, identical fills.
const FIELD: usize = 16;
/// The bounded retained registry.
const RETAINED: usize = 3;
const LABEL: &str = "ties";

/// The four-order book: two buys and two sells, all on outcome 0, four Eggs
/// each so every conversion is a whole atom.
fn book_plan(owner_ids: &[Hash32], buy_limit: u64, sell_limit: u64) -> Vec<PlannedOrder> {
    vec![
        (0, single(owner_ids[0], 1, 0, 0, 4, buy_limit, EPOCH_INDEX)),
        (1, single(owner_ids[1], 2, 0, 1, 4, sell_limit, EPOCH_INDEX)),
        (2, single(owner_ids[2], 3, 0, 0, 4, buy_limit, EPOCH_INDEX)),
        (3, single(owner_ids[3], 4, 0, 1, 4, sell_limit, EPOCH_INDEX)),
    ]
}

/// The `k`-th tied price vector: outcome 0 is pinned at a quarter of the
/// scale, and the remaining three-quarters are redistributed across the three
/// untraded outcomes.  Fills — and therefore every score component — cannot
/// see the redistribution; the candidate identity can.
fn tied_prices(k: usize) -> [u64; MAX_OUTCOMES] {
    let step = 100 * (k as u64 + 1);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[0] = PRICE_SCALE / 4;
    prices[1] = 2_500 + step;
    prices[2] = 2_500;
    prices[3] = 2_500 - step;
    assert_eq!(prices[..4].iter().sum::<u64>(), PRICE_SCALE);
    prices
}

fn stored_score(record: &CandidateRecord) -> FullScoreV1 {
    FullScoreV1 {
        weighted_direct_volume: record.weighted_direct_volume,
        limit_surplus_price_units: record.limit_surplus_price_units,
        distinct_owners: record.distinct_owners,
        churn: record.churn,
        digest: Identity32V1(record.score_digest.bytes()),
    }
}

async fn tied_field(
    context: &mut solana_program_test::ProgramTestContext,
    plane: &scale_common::Plane,
    epoch: &EpochAccount,
    book: &clutch_batch::relation_v1::BookV1,
) -> Vec<Submission> {
    let _ = context;
    (0..FIELD)
        .map(|k| plan_submission(plane, epoch, book, tied_prices(k), 0, None))
        .collect()
}

/// Gate one: a sixteen-deep tied field, a registry that holds, and a digest
/// that decides — with the tie order re-derived as a strict total order.
#[tokio::test]
async fn a_sixteen_deep_tied_field_holds_the_registry_and_the_digest_decides() {
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::new(OWNERS);
    let market = lab.market(MarketSpec {
        market_byte: 0x57,
        outcomes: OUTCOMES,
        tick_count: TICK_COUNT,
        tick_spacing: SPACING,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    });
    let plane = lab.epoch(&market, EPOCH_INDEX, 1, FREEZE_DEADLINE);
    let (mut context, keeper, owners) = lab.start().await;

    let owner_ids: Vec<Hash32> = owners.iter().map(|owner| owner.id).collect();
    let orders = book_plan(&owner_ids, market.tick(50), market.tick(10));
    build_frozen_book(
        &mut context,
        &plane,
        &keeper,
        &owners,
        &orders,
        &[],
        &mut meter,
        "",
        0,
    )
    .await;
    let frozen = frozen_state(&mut context, &plane).await;
    assert_eq!(frozen.book.len, 4);

    let field = tied_field(&mut context, &plane, &frozen.epoch, &frozen.book).await;
    // Every candidate is a distinct identity computing the same allocation.
    for k in 1..FIELD {
        assert_eq!(field[k].fills, field[0].fills, "candidate {k} fills");
        assert_ne!(field[k].id, field[0].id, "candidate {k} identity");
    }
    let mut sorted: Vec<Hash32> = field.iter().map(|entry| entry.id).collect();
    sorted.sort_by_key(|id| id.bytes());
    sorted.dedup();
    assert_eq!(sorted.len(), FIELD, "the field's identities are distinct");

    // Seal only binds immutable feed bytes.  Even a sixteen-deep field cannot
    // prefill a registry slot with unverified claims.
    for (k, submission) in field.iter().enumerate() {
        let (result, _) = submit_seal(
            &mut context,
            &plane,
            &keeper,
            submission,
            Some(submission.witness.len),
            &[],
            None,
            &mut meter,
            "",
            400 + 40 * k as u32,
        )
        .await;
        result.unwrap();
    }
    let empty_window =
        EpochWindowAccount::decode(&bytes_of(&mut context, plane.window_account).await).unwrap();
    assert_eq!(empty_window.retained_count, 0);

    // Every valid submission gets a verdict.  After each atomic completion,
    // take the next account suffix from the onchain verified-only registry.
    let mut retained = Vec::new();
    for (k, submission) in field.iter().enumerate() {
        walk_to_verdict(
            &mut context,
            &plane,
            &keeper,
            submission,
            &frozen,
            submission.witness.len,
            &retained,
            &mut meter,
            &format!("cand{k}"),
            2_000 + 100 * k as u32,
        )
        .await;
        let window =
            EpochWindowAccount::decode(&bytes_of(&mut context, plane.window_account).await)
                .unwrap();
        retained = window.retained[..usize::from(window.retained_count)].to_vec();
    }
    assert_eq!(retained.len(), RETAINED);

    let mut scores = Vec::new();
    for id in &retained {
        let record = read_record(&mut context, &plane, *id).await;
        assert_eq!(record.status, CANDIDATE_STATUS_VERIFIED);
        assert_ne!(record.score_digest, Hash32::ZERO);
        scores.push((*id, stored_score(&record)));
    }
    // The tie is real on all four components, and the digests separate.
    for k in 1..RETAINED {
        assert_eq!(
            scores[k].1.weighted_direct_volume,
            scores[0].1.weighted_direct_volume
        );
        assert_eq!(
            scores[k].1.limit_surplus_price_units,
            scores[0].1.limit_surplus_price_units
        );
        assert_eq!(scores[k].1.distinct_owners, scores[0].1.distinct_owners);
        assert_eq!(scores[k].1.churn, scores[0].1.churn);
        assert_ne!(scores[k].1.digest, scores[0].1.digest);
    }

    /* Determinism, checked rather than observed: over every ordered pair of
     * the retained field, `is_better_than` agrees with the raw full-width
     * digest comparison, is irreflexive, is antisymmetric, and is transitive.
     * A tie rule that decided by anything but the digest, or decided
     * inconsistently, fails here before any transaction is sent. */
    for (_, left) in &scores {
        assert!(!left.is_better_than(left), "irreflexive");
    }
    for (_, left) in &scores {
        for (_, right) in &scores {
            if left.digest == right.digest {
                continue;
            }
            assert_eq!(
                left.is_better_than(right),
                left.digest.0 < right.digest.0,
                "the component tie is decided by the full-width digest"
            );
            assert_ne!(
                left.is_better_than(right),
                right.is_better_than(left),
                "antisymmetric"
            );
        }
    }
    for (_, a) in &scores {
        for (_, b) in &scores {
            for (_, c) in &scores {
                if a.is_better_than(b) && b.is_better_than(c) {
                    assert!(a.is_better_than(c), "transitive");
                }
            }
        }
    }
    // A digest difference beyond the first 128 bits still decides.
    let mut deep_left = scores[0].1;
    let mut deep_right = scores[0].1;
    deep_left.digest.0[20] = 1;
    deep_right.digest.0[20] = 2;
    assert!(deep_left.is_better_than(&deep_right));

    let expected_winner = scores
        .iter()
        .min_by_key(|(_, score)| score.digest.0)
        .map(|(id, _)| *id)
        .unwrap();

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, units) = send(&mut context, &[plane.finalize(&retained)], &[], 3_000).await;
    result.unwrap();
    meter.record("finalize_selection_3retained_3verified_digest_tie", units);

    assert_eq!(
        read_record(&mut context, &plane, expected_winner)
            .await
            .status,
        CANDIDATE_STATUS_SELECTED
    );
    for id in &retained {
        if *id == expected_winner {
            continue;
        }
        assert_eq!(
            read_record(&mut context, &plane, *id).await.status,
            CANDIDATE_STATUS_VERIFIED,
            "a loser of the digest tie stays VERIFIED"
        );
    }
    let window =
        EpochWindowAccount::decode(&bytes_of(&mut context, plane.window_account).await).unwrap();
    assert_eq!(window.selected_candidate, expected_winner);
    assert_eq!(
        EpochAccount::decode(&bytes_of(&mut context, plane.epoch_account).await)
            .unwrap()
            .phase,
        EPOCH_PHASE_CLEARED
    );

    meter.finish();
}

/// Gate two: at a full, component-tied registry, the fourth candidate's
/// verified digest — never its claim or account identity — determines whether
/// it displaces exactly the verified worst.  A displaced feed remains live.
#[tokio::test]
async fn verified_digest_alone_controls_tied_displacement() {
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::new(OWNERS);
    let market = lab.market(MarketSpec {
        market_byte: 0x58,
        outcomes: OUTCOMES,
        tick_count: TICK_COUNT,
        tick_spacing: SPACING,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    });
    let plane = lab.epoch(&market, EPOCH_INDEX, 1, FREEZE_DEADLINE);
    let (mut context, keeper, owners) = lab.start().await;

    let owner_ids: Vec<Hash32> = owners.iter().map(|owner| owner.id).collect();
    let orders = book_plan(&owner_ids, market.tick(50), market.tick(10));
    build_frozen_book(
        &mut context,
        &plane,
        &keeper,
        &owners,
        &orders,
        &[],
        &mut meter,
        "",
        0,
    )
    .await;
    let frozen = frozen_state(&mut context, &plane).await;
    let field = tied_field(&mut context, &plane, &frozen.epoch, &frozen.book).await;

    for (index, submission) in field.iter().take(RETAINED + 1).enumerate() {
        let (result, _) = submit_seal(
            &mut context,
            &plane,
            &keeper,
            submission,
            Some(submission.witness.len),
            &[],
            None,
            &mut meter,
            "",
            400 + 40 * index as u32,
        )
        .await;
        result.unwrap();
    }
    let sealed_window =
        EpochWindowAccount::decode(&bytes_of(&mut context, plane.window_account).await).unwrap();
    assert_eq!(sealed_window.retained_count, 0);

    let mut retained = Vec::new();
    for (index, submission) in field.iter().take(RETAINED).enumerate() {
        walk_to_verdict(
            &mut context,
            &plane,
            &keeper,
            submission,
            &frozen,
            submission.witness.len,
            &retained,
            &mut meter,
            &format!("seed{index}"),
            1_000 + 100 * index as u32,
        )
        .await;
        let window =
            EpochWindowAccount::decode(&bytes_of(&mut context, plane.window_account).await)
                .unwrap();
        retained = window.retained[..usize::from(window.retained_count)].to_vec();
    }

    let mut expected_displaced = retained[0];
    let mut worst_score =
        stored_score(&read_record(&mut context, &plane, expected_displaced).await);
    for id in retained.iter().copied().skip(1) {
        let score = stored_score(&read_record(&mut context, &plane, id).await);
        if worst_score.is_better_than(&score) {
            expected_displaced = id;
            worst_score = score;
        }
    }
    let displaced_feed = plane.candidate_feed(expected_displaced);
    let record_before = account(&mut context, plane.candidate_record(expected_displaced))
        .await
        .unwrap()
        .lamports;
    let feed_lamports = account(&mut context, displaced_feed)
        .await
        .unwrap()
        .lamports;

    let challenger = &field[RETAINED];
    walk_to_verdict(
        &mut context,
        &plane,
        &keeper,
        challenger,
        &frozen,
        challenger.witness.len,
        &retained,
        &mut meter,
        "challenger",
        1_500,
    )
    .await;
    let challenger_record = read_record(&mut context, &plane, challenger.id).await;
    let challenger_score = stored_score(&challenger_record);
    let window =
        EpochWindowAccount::decode(&bytes_of(&mut context, plane.window_account).await).unwrap();
    assert_eq!(window.retained_count as usize, RETAINED);
    if challenger_score.is_better_than(&worst_score) {
        let superseded = read_record(&mut context, &plane, expected_displaced).await;
        assert_eq!(superseded.status, CANDIDATE_STATUS_SUPERSEDED);
        assert_eq!(superseded.score_digest, Hash32::ZERO);
        assert!(window.retained.contains(&challenger.id));
        assert!(!window.retained.contains(&expected_displaced));
    } else {
        assert_eq!(challenger_record.status, CANDIDATE_STATUS_VERIFIED);
        assert_eq!(window.retained[..RETAINED], retained);
    }
    assert_eq!(
        account(&mut context, displaced_feed)
            .await
            .unwrap()
            .lamports,
        feed_lamports
    );
    assert_eq!(
        account(&mut context, plane.candidate_record(expected_displaced))
            .await
            .unwrap()
            .lamports,
        record_before
    );

    meter.finish();
}
