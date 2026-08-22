//! Scale campaign 4 — **partial fills at scale**.
//!
//! `entitled_clearing.rs` pins the per-slice seam on the smallest shape that
//! exhibits it: one strict buy of eight against two marginal sells of six,
//! two receipts.  This campaign drives the same seam at the size that makes
//! it a load statement — **thirty orders, thirty owners, two pages, and two
//! dozen receipts** — with the consumption order deliberately scrambled and
//! the entitlement of the second half deferred until after the first half has
//! already drawn its ends down.
//!
//! The book: twelve strict buys of twelve Eggs against eighteen marginal sells
//! of ten, all on outcome 0 at a quarter of the price scale.  Demand is 144
//! against a supply of 180, so every sell fills pro rata at eight and every
//! buy fills whole — every one of them fragmenting across two or three
//! counterparties, and every sell splitting between two or three buyers.
//! Each order's whole-order value is a whole collateral atom (twelve Eggs is
//! three, eight is two), so the pot freezes CLOSED and the arithmetic is exact
//! rather than potted; the pot at scale is campaign 6.
//!
//! What is asserted:
//!
//! * conservation of cash and of outcome-0 Eggs **after every single
//!   consumption**, not only at the end — the sum a partial clearing has to
//!   make good on is Positions plus live reservation envelopes;
//! * the completion ladder — a reservation stays ENTITLED while
//!   `consumed_units < entitled_units` and flips CONSUMED exactly when they
//!   meet, at which point its remainder returns to its Position once and only
//!   once;
//! * every end's stamp is its **whole order's** total from the first touch,
//!   re-derived and required equal by every later touch;
//! * the hostile shapes on the same bank: a receipt presented against the
//!   wrong counterparty refuses, a replay on an exhausted receipt refuses and
//!   moves nothing, and a terminal release of a filled reservation refuses.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

mod scale_common;

use {
    clutch_sbf::error::ClutchError,
    clutch_solana_layout::{
        clearing::CANDIDATE_WINDOW_SLOTS,
        reservation::{RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED},
        FinalPotAccount, MAX_GRID_TICKS, MAX_OUTCOMES, POT_PHASE_CLOSED,
    },
    scale_common::{
        book_eggs, build_frozen_book, bytes_of, custom, frozen_state, owner_cash, plan_submission,
        read_position, read_reservation, send, send_walk, single, slice_ends, snapshot,
        submit_seal, walk_to_verdict, Lab, MarketSpec, Meter, PlannedOrder,
    },
    solana_program_test::tokio,
    solana_signer::Signer,
};

const EPOCH_INDEX: u64 = 7;
const FREEZE_DEADLINE: u64 = 500;
const OUTCOMES: u8 = 4;
const TICK_COUNT: u8 = MAX_GRID_TICKS as u8;
const SPACING: u64 = 100;
const PRICE_SCALE: u64 = 10_000;
const START_CASH: u64 = 1_000_000;
const START_EGGS: u64 = 1_000;
/// Twelve strict buys of twelve against eighteen marginal sells of ten.
const BUYERS: usize = 12;
const SELLERS: usize = 18;
const OWNERS: usize = BUYERS + SELLERS;
const BUY_QUANTITY: u64 = 12;
const SELL_QUANTITY: u64 = 10;
/// Pro rata: 144 of demand against 180 of supply is four fifths.
const SELL_FILL: u64 = 8;
const SLICE_BATCH: u16 = 8;
const LABEL: &str = "partial_fills";

/// Two dozen receipts, out of order, with conservation checked at every step.
#[tokio::test]
async fn many_orders_fill_partially_across_many_slices_with_exact_conservation() {
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::new(OWNERS);
    let market = lab.market(MarketSpec {
        market_byte: 0x5a,
        outcomes: OUTCOMES,
        tick_count: TICK_COUNT,
        tick_spacing: SPACING,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    });
    // Thirty records need two pages; the second is deliberately partial.
    let plane = lab.epoch(&market, EPOCH_INDEX, 2, FREEZE_DEADLINE);
    let (mut context, keeper, owners) = lab.start().await;
    let keeper_key = keeper.pubkey();

    let price = PRICE_SCALE / u64::from(OUTCOMES);
    let buy_limit = market.tick(50);
    let sell_limit = market.tick(25);
    assert_eq!(
        sell_limit, price,
        "the sells are marginal at the clearing price"
    );

    let mut orders: Vec<PlannedOrder> = Vec::with_capacity(OWNERS);
    for (buyer, owner) in owners.iter().enumerate().take(BUYERS) {
        orders.push((
            buyer,
            single(
                owner.id,
                orders.len() as u64 + 1,
                0,
                0,
                BUY_QUANTITY,
                buy_limit,
                EPOCH_INDEX,
            ),
        ));
    }
    for (seller, owner) in owners.iter().enumerate().skip(BUYERS) {
        orders.push((
            seller,
            single(
                owner.id,
                orders.len() as u64 + 1,
                0,
                1,
                SELL_QUANTITY,
                sell_limit,
                EPOCH_INDEX,
            ),
        ));
    }
    assert_eq!(orders.len(), OWNERS);

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
    assert_eq!(frozen.epoch.order_count as usize, OWNERS);
    assert_eq!(frozen.epoch.owner_count as usize, OWNERS);
    assert_eq!(frozen.epoch.page_count, 2);

    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..usize::from(OUTCOMES)].fill(price);
    let submission = plan_submission(&plane, &frozen.epoch, &frozen.book, prices, 0, None);
    let mut expected: Vec<u64> = vec![BUY_QUANTITY; BUYERS];
    expected.extend(vec![SELL_FILL; SELLERS]);
    assert_eq!(
        submission.fills, expected,
        "every buy fills whole and every marginal sell fills pro rata"
    );
    let slices = submission.witness.len;
    eprintln!("scale.{LABEL} WITNESS declared_slices={slices} orders={OWNERS}");
    assert!(
        slices >= 24,
        "the staircase must fragment into at least two dozen slices, got {slices}"
    );

    let (result, _) = submit_seal(
        &mut context,
        &plane,
        &keeper,
        &submission,
        Some(slices),
        &[],
        None,
        &mut meter,
        "",
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
        &mut meter,
        "",
        900,
    )
    .await;

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, units) = send(
        &mut context,
        &[plane.finalize(&[submission.id])],
        &[],
        1_100,
    )
    .await;
    result.unwrap();
    meter.record("finalize_selection_1retained", units);

    let (result, units) = send_walk(
        &mut context,
        plane.freeze_entitlement(keeper_key, submission.id),
        &[&keeper],
        1_101,
    )
    .await;
    result.unwrap();
    meter.record("freeze_entitlement_ledgered", units);
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, plane.pot()).await).unwrap();
    assert_eq!(
        pot.phase, POT_PHASE_CLOSED,
        "every whole-order value is a whole atom"
    );

    let cash_before = owner_cash(&mut context, &plane.positions).await;
    let eggs_before = book_eggs(&mut context, &plane.positions, &frozen.reservations, 0).await;

    // The scrambled schedule: entitle the back half first, consume the front
    // half on a stride, then entitle the front half — whose ends are already
    // drawn down — and consume the back half in reverse.
    let half = slices / 2;
    let front: Vec<u16> = (0..half).collect();
    let back: Vec<u16> = (half..slices).collect();
    let stride: Vec<u16> = {
        let mut all = Vec::new();
        let modulus = u32::from(half);
        for k in 0..modulus {
            all.push(((k * 5) % modulus) as u16);
        }
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), usize::from(half), "the stride is a permutation");
        (0..modulus).map(|k| ((k * 5) % modulus) as u16).collect()
    };

    let mut nonce = 1_200u32;
    let mut entitle_worst = 0u64;
    let mut settle_worst = 0u64;

    let back_first: Vec<u16> = back.iter().rev().copied().collect();

    for slice_index in back_first.iter().copied() {
        let (buy, sell) = slice_ends(&submission.witness, slice_index);
        let (result, units) = send(
            &mut context,
            &[plane.entitle_single(
                keeper_key,
                submission.id,
                slice_index,
                frozen.reservations[buy],
                frozen.reservations[sell],
            )],
            &[&keeper],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        entitle_worst = entitle_worst.max(units);
        meter.record(
            &format!("entitle_slice_single_2pages_ledgered_slice{slice_index}"),
            units,
        );
        // The stamp is the whole order's total, not this slice's quantity.
        assert_eq!(
            read_reservation(&mut context, frozen.reservations[buy])
                .await
                .entitled_units,
            BUY_QUANTITY,
            "buy end {buy} is stamped with its whole order"
        );
        assert_eq!(
            read_reservation(&mut context, frozen.reservations[sell])
                .await
                .entitled_units,
            SELL_FILL,
            "sell end {sell} is stamped with its filled total"
        );
    }

    // Consume the entitled back half in a strided order, checking the whole
    // value plane after each one.
    let back_stride: Vec<u16> = {
        let n = back.len() as u32;
        (0..n).map(|k| back[((k * 5) % n) as usize]).collect()
    };
    for slice_index in back_stride.iter().copied() {
        consume_and_check(
            &mut context,
            &plane,
            &frozen,
            &submission,
            &orders,
            slice_index,
            &mut meter,
            &mut settle_worst,
            &mut nonce,
            cash_before,
            eggs_before,
        )
        .await;
    }

    // Now entitle the front half: several of its ends are already drawn down
    // by the consumptions above, and each re-derives the same stamp.
    for slice_index in front.iter().copied() {
        let (buy, sell) = slice_ends(&submission.witness, slice_index);
        let (result, units) = send(
            &mut context,
            &[plane.entitle_single(
                keeper_key,
                submission.id,
                slice_index,
                frozen.reservations[buy],
                frozen.reservations[sell],
            )],
            &[&keeper],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        entitle_worst = entitle_worst.max(units);
        meter.record(
            &format!("entitle_slice_single_2pages_ledgered_slice{slice_index}"),
            units,
        );
    }

    /* Hostile, mid-ladder: slice `front[0]`'s receipt names its own pair, and
     * presenting a different sell end refuses before a byte moves. */
    let (buy0, sell0) = slice_ends(&submission.witness, front[0]);
    let other_sell = (BUYERS..OWNERS)
        .find(|candidate| *candidate != sell0)
        .unwrap();
    let watched = [
        plane.positions[orders[buy0].0],
        plane.positions[orders[sell0].0],
        frozen.reservations[buy0],
        frozen.reservations[sell0],
    ];
    let before = snapshot(&mut context, &watched).await;
    let (result, _) = send(
        &mut context,
        &[plane.settle_single(
            submission.id,
            plane.positions[orders[buy0].0],
            plane.positions[orders[other_sell].0],
            frozen.reservations[buy0],
            frozen.reservations[other_sell],
            front[0],
            frozen.live_pages[buy0],
        )],
        &[],
        nonce,
    )
    .await;
    nonce += 1;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);
    assert_eq!(snapshot(&mut context, &watched).await, before);

    /* A filled order's remainder returns only through completion: the
     * owner-signed terminal release keeps refusing a nonzero fill. */
    let (result, _) = send(
        &mut context,
        &[plane.release_cleared(
            owners[orders[buy0].0].key.pubkey(),
            frozen.reservations[buy0],
            plane.positions[orders[buy0].0],
            submission.id,
            2,
            frozen.live_pages[buy0],
        )],
        &[&owners[orders[buy0].0].key],
        nonce,
    )
    .await;
    nonce += 1;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);

    for slice_index in stride.iter().copied() {
        consume_and_check(
            &mut context,
            &plane,
            &frozen,
            &submission,
            &orders,
            slice_index,
            &mut meter,
            &mut settle_worst,
            &mut nonce,
            cash_before,
            eggs_before,
        )
        .await;
    }

    // A replay on an exhausted receipt refuses and moves nothing.
    let before = snapshot(&mut context, &watched).await;
    let (result, _) = send(
        &mut context,
        &[plane.settle_single(
            submission.id,
            plane.positions[orders[buy0].0],
            plane.positions[orders[sell0].0],
            frozen.reservations[buy0],
            frozen.reservations[sell0],
            front[0],
            frozen.live_pages[buy0],
        )],
        &[],
        nonce,
    )
    .await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);
    assert_eq!(snapshot(&mut context, &watched).await, before);

    /* ------------------------------------------------------------------ */
    /* The ledger, closed.                                                 */
    /* ------------------------------------------------------------------ */

    for (at, address) in frozen.reservations.iter().enumerate() {
        let reservation = read_reservation(&mut context, *address).await;
        assert_eq!(
            reservation.state, RESERVATION_STATE_CONSUMED,
            "reservation {at} completed"
        );
        assert_eq!(reservation.consumed_units, reservation.entitled_units);
        assert!(reservation.remaining_is_zero());
        assert_eq!(reservation.fee_debited_atoms, 0);
        assert_eq!(reservation.fee_carry_numerator, 0);
    }
    // Each buyer paid three atoms for twelve Eggs and got the rest of its
    // six-atom reserve back; each seller took two atoms and its two unfilled
    // Eggs.
    for buyer in 0..BUYERS {
        let position = read_position(&mut context, plane.positions[buyer]).await;
        assert_eq!(position.cash_atoms, START_CASH - 3, "buyer {buyer} cash");
        assert_eq!(position.reserved_cash_atoms, 0);
        assert_eq!(position.internal[0], START_EGGS + BUY_QUANTITY);
    }
    for seller in BUYERS..OWNERS {
        let position = read_position(&mut context, plane.positions[seller]).await;
        assert_eq!(position.cash_atoms, START_CASH + 2, "seller {seller} cash");
        assert_eq!(position.reserved_cash_atoms, 0);
        assert_eq!(
            position.internal[0],
            START_EGGS - SELL_QUANTITY + (SELL_QUANTITY - SELL_FILL)
        );
    }
    assert_eq!(
        owner_cash(&mut context, &plane.positions).await,
        cash_before
    );
    assert_eq!(
        book_eggs(&mut context, &plane.positions, &frozen.reservations, 0).await,
        eggs_before
    );
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, plane.pot()).await).unwrap();
    assert_eq!(pot.rounding_pot_price_units, 0);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);

    eprintln!(
        "scale.{LABEL} WORST_FAMILY entitle_slice_single_cu={entitle_worst} settle_page_direct_cu={settle_worst}"
    );
    meter.finish();
}

/// Consume one entitled slice, then re-assert the whole value plane.
#[allow(clippy::too_many_arguments)] // one argument per campaign coordinate
async fn consume_and_check(
    context: &mut solana_program_test::ProgramTestContext,
    plane: &scale_common::Plane,
    frozen: &scale_common::Frozen,
    submission: &scale_common::Submission,
    orders: &[PlannedOrder],
    slice_index: u16,
    meter: &mut Meter,
    settle_worst: &mut u64,
    nonce: &mut u32,
    cash_before: u64,
    eggs_before: u64,
) {
    let (buy, sell) = slice_ends(&submission.witness, slice_index);
    let buy_before = read_reservation(context, frozen.reservations[buy]).await;
    let sell_before = read_reservation(context, frozen.reservations[sell]).await;
    let quantity = submission.witness.slices[usize::from(slice_index)].quantity;

    let (result, units) = send(
        context,
        &[plane.settle_single(
            submission.id,
            plane.positions[orders[buy].0],
            plane.positions[orders[sell].0],
            frozen.reservations[buy],
            frozen.reservations[sell],
            slice_index,
            frozen.live_pages[buy],
        )],
        &[],
        *nonce,
    )
    .await;
    result.unwrap();
    *nonce += 1;
    *settle_worst = (*settle_worst).max(units);
    meter.record(
        &format!("settle_page_direct_2pages_slice{slice_index}"),
        units,
    );

    // The completion ladder: each end took exactly this slice's quantity, and
    // flipped CONSUMED exactly when its stamp was met.
    for (label, address, before) in [
        ("buy", frozen.reservations[buy], buy_before),
        ("sell", frozen.reservations[sell], sell_before),
    ] {
        let after = read_reservation(context, address).await;
        assert_eq!(
            after.consumed_units,
            before.consumed_units + quantity,
            "{label} end of slice {slice_index} consumed its slice"
        );
        assert_eq!(
            after.entitled_units, before.entitled_units,
            "stamp is stable"
        );
        if after.consumed_units == after.entitled_units {
            assert_eq!(
                after.state, RESERVATION_STATE_CONSUMED,
                "{label} end of slice {slice_index} completed"
            );
            assert!(after.remaining_is_zero());
        } else {
            assert_eq!(
                after.state, RESERVATION_STATE_ENTITLED,
                "{label} end of slice {slice_index} is still open"
            );
        }
    }

    // Conservation, after every single consumption.
    assert_eq!(
        owner_cash(context, &plane.positions).await,
        cash_before,
        "cash conserved after slice {slice_index}"
    );
    assert_eq!(
        book_eggs(context, &plane.positions, &frozen.reservations, 0).await,
        eggs_before,
        "Eggs conserved after slice {slice_index}"
    );
}
