//! Fixed-book Entitle witness-width campaign.
//!
//! Existing Entitle scale rows grow the frozen book and the pairing witness at
//! the same time.  This campaign holds the live book at four dense pages and
//! sixty-four orders while fragmenting the same 416-Egg crossing into 1, 32,
//! 128, and 416 explicit slices.  The other sixty-two orders are valid but
//! out of the money.  The measured difference is therefore witness-walk work,
//! not page- or order-walk work, while the fixed book is itself maximal.
//!
//! The 129-slice case is the receipt-coordinate regression: it entitles slice
//! index 128, the first coordinate admitted by the candidate feed that the
//! former `2 * MAX_EPOCH_ORDERS` receipt bound refused.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

mod scale_common;

use {
    clutch_batch::relation_v1::{verify_pairing_witness, LegRefV1, PairingSliceV1},
    clutch_solana_layout::{
        clearing::CANDIDATE_WINDOW_SLOTS, SettlementReceiptAccount, MAX_EPOCH_ORDERS,
        MAX_GRID_TICKS, MAX_ORDER_PAGES, MAX_OUTCOMES, MAX_SLICES,
    },
    scale_common::{
        build_frozen_book, bytes_of, frozen_state, plan_submission, send, send_walk, single,
        submit_seal, walk_to_verdict, witness_of, Lab, MarketSpec, Meter, PlannedOrder,
    },
    solana_program_test::tokio,
    solana_signer::Signer,
};

const EPOCH_INDEX: u64 = 7;
const FREEZE_DEADLINE: u64 = 500;
const OUTCOMES: u8 = 4;
const PRICE_SCALE: u64 = 10_000;
const START_CASH: u64 = 1_000_000;
const START_EGGS: u64 = 1_000;
const QUANTITY: u64 = MAX_SLICES as u64;
const OWNERS: usize = 2;
const SLICE_BATCH: u16 = 8;
const LABEL: &str = "entitle_witness";

/// Fragment the same complete crossing into exactly `width` positive slices.
/// The first slice carries the remainder and every later slice carries one
/// Egg, so the book, fills, owners, prices, and total flow stay fixed.
fn fragmented_witness(width: u16) -> clutch_batch::relation_v1::PairingWitnessV1 {
    assert!(width != 0 && usize::from(width) <= MAX_SLICES);
    let first = QUANTITY - u64::from(width) + 1;
    let mut slices = Vec::with_capacity(usize::from(width));
    slices.push(PairingSliceV1 {
        buy_ref: LegRefV1::Order(0),
        sell_ref: LegRefV1::Order(1),
        outcome: 0,
        quantity: first,
    });
    for _ in 1..width {
        slices.push(PairingSliceV1 {
            buy_ref: LegRefV1::Order(0),
            sell_ref: LegRefV1::Order(1),
            outcome: 0,
            quantity: 1,
        });
    }
    assert_eq!(
        slices.iter().map(|slice| slice.quantity).sum::<u64>(),
        QUANTITY
    );
    witness_of(&slices)
}

async fn entitle_at_width(
    width: u16,
    target_slice: u16,
) -> (Result<(), solana_transaction_error::TransactionError>, u64) {
    assert!(target_slice < width);
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::deterministic(OWNERS);
    let market = lab.market(MarketSpec {
        market_byte: 0x5d,
        outcomes: OUTCOMES,
        tick_count: MAX_GRID_TICKS as u8,
        tick_spacing: 100,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    });
    let plane = lab.epoch(
        &market,
        EPOCH_INDEX,
        MAX_ORDER_PAGES as u16,
        FREEZE_DEADLINE,
    );
    let (mut context, keeper, owners) = lab.start().await;
    let keeper_key = keeper.pubkey();

    let mut orders: Vec<PlannedOrder> = vec![
        (
            0,
            single(
                owners[0].id,
                1,
                0,
                0,
                QUANTITY,
                market.tick(50),
                EPOCH_INDEX,
            ),
        ),
        (
            1,
            single(
                owners[1].id,
                2,
                0,
                1,
                QUANTITY,
                market.tick(10),
                EPOCH_INDEX,
            ),
        ),
    ];
    while orders.len() < MAX_EPOCH_ORDERS {
        let side = orders.len() % 2;
        let owner = side;
        let limit = if side == 0 {
            // A buy below the quarter-scale clearing price cannot fill.
            market.tick(10)
        } else {
            // A sell above the quarter-scale clearing price cannot fill.
            market.tick(50)
        };
        orders.push((
            owner,
            single(
                owners[owner].id,
                orders.len() as u64 + 1,
                0,
                side as u8,
                1,
                limit,
                EPOCH_INDEX,
            ),
        ));
    }
    build_frozen_book(
        &mut context,
        &plane,
        &keeper,
        &owners,
        &orders,
        &[],
        &mut meter,
        &format!("w{width}"),
        0,
    )
    .await;
    let frozen = frozen_state(&mut context, &plane).await;
    assert_eq!(frozen.epoch.page_count, MAX_ORDER_PAGES as u16);
    assert_eq!(frozen.epoch.order_count as usize, MAX_EPOCH_ORDERS);

    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..usize::from(OUTCOMES)].fill(PRICE_SCALE / u64::from(OUTCOMES));
    let witness = fragmented_witness(width);
    let submission = plan_submission(
        &plane,
        &frozen.epoch,
        &frozen.book,
        prices,
        0,
        Some(witness),
    );
    assert_eq!(submission.fills[..2], [QUANTITY, QUANTITY]);
    assert!(
        submission.fills[2..].iter().all(|fill| *fill == 0),
        "the fixed book has exactly two filled orders"
    );
    assert_eq!(submission.witness.len, width);
    verify_pairing_witness(
        &scale_common::zero_sentinel_domain(&frozen.epoch),
        &frozen.book,
        &clutch_batch::relation_v1::canonical_candidate(
            &scale_common::zero_sentinel_domain(&frozen.epoch),
            &frozen.book,
            &prices,
            0,
            0,
        )
        .unwrap(),
        &submission.witness,
    )
    .unwrap();

    let (result, _) = submit_seal(
        &mut context,
        &plane,
        &keeper,
        &submission,
        Some(width),
        &[],
        None,
        &mut meter,
        &format!("w{width}"),
        400,
    )
    .await;
    result.unwrap();
    walk_to_verdict(
        &mut context,
        &plane,
        &keeper,
        &submission,
        &frozen,
        SLICE_BATCH,
        &[],
        &mut meter,
        &format!("w{width}"),
        1_000,
    )
    .await;

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(
        &mut context,
        &[plane.finalize(&[submission.id])],
        &[],
        2_000,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(
        &mut context,
        plane.freeze_entitlement(keeper_key, submission.id),
        &[&keeper],
        2_001,
    )
    .await;
    result.unwrap();

    let (buy, sell) = scale_common::slice_ends(&submission.witness, target_slice);
    let (result, units) = send(
        &mut context,
        &[plane.entitle_single(
            keeper_key,
            submission.id,
            target_slice,
            frozen.reservations[buy],
            frozen.reservations[sell],
        )],
        &[&keeper],
        2_100 + u32::from(width),
    )
    .await;
    eprintln!(
        "scale.{LABEL}/entitle_slice_single_4pages_64orders_witness{width}_slice{target_slice} CU: {units} result={result:?}"
    );

    if result.is_ok() {
        let receipt = SettlementReceiptAccount::decode(
            &bytes_of(&mut context, plane.receipt(submission.id, target_slice)).await,
        )
        .unwrap();
        assert_eq!(receipt.slice_index, target_slice);
        assert_eq!(
            receipt.quantity,
            submission.witness.slices[usize::from(target_slice)].quantity
        );
    }
    (result, units)
}

#[tokio::test]
async fn fixed_book_separates_entitle_witness_width_and_reaches_receipt_index_128() {
    for width in [1u16, 32, 128] {
        let (result, units) = entitle_at_width(width, 0).await;
        result.unwrap_or_else(|error| panic!("width {width} refused after {units} CU: {error:?}"));
    }

    let (result, units) = entitle_at_width(129, 128).await;
    result.unwrap_or_else(|error| {
        panic!("receipt slice index 128 refused after {units} CU: {error:?}")
    });

    let (result, units) = entitle_at_width(MAX_SLICES as u16, 0).await;
    result.unwrap_or_else(|error| {
        panic!("maximum admitted witness refused after {units} CU: {error:?}")
    });
}
