//! Scale campaign 2 — **the maximum book**.
//!
//! `MAX_ORDER_PAGES` is four and `MAX_ORDERS_PER_PAGE` is sixteen, so a general
//! epoch's frozen set holds at most `MAX_EPOCH_ORDERS = 64` records.  The
//! heaviest shape any suite in the tree had driven was three pages and forty
//! orders (`clear_lifecycle.rs`), which is what the W1 table's
//! `freeze_epoch_3pages_40orders` row measures.  This campaign drives the
//! **complete** set: four dense pages, sixty-four live records, and every
//! page-set-wide route — the freeze, the four-page pass-1 walk, the
//! thirty-two-slice pairing pass, the four-page pass-2 walk, and every
//! `EntitleSlice` (which must present the whole bound page set) — measured at
//! that maximum.
//!
//! The book is eight owners deep — four pure buyers, four pure sellers, so no
//! canonical pairing can ever name one owner on both ends — with two orders
//! per owner per outcome across four outcomes.  Every order is four Eggs at a
//! quarter of the price scale, which is exactly one collateral atom, so every
//! conversion is whole, the pot freezes CLOSED, and conservation is exact to
//! the atom.
//!
//! Every created account carries its `GeneralFundingLedgerV1` sibling: the
//! shape a keeper must actually send.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

mod scale_common;

use {
    clutch_solana_layout::{
        clearing::CANDIDATE_WINDOW_SLOTS, reservation::RESERVATION_STATE_CONSUMED, stream,
        EpochAccount, FinalPotAccount, CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED,
        MAX_EPOCH_ORDERS, MAX_GRID_TICKS, MAX_ORDERS_PER_PAGE, MAX_ORDER_PAGES, MAX_OUTCOMES,
        POT_PHASE_CLOSED,
    },
    scale_common::{
        book_eggs, build_frozen_book, bytes_of, frozen_state, owner_cash, plan_submission,
        read_position, read_record, read_reservation, send, send_walk, single, slice_ends,
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
/// Four pure buyers and four pure sellers.
const BUYERS: usize = 4;
const SELLERS: usize = 4;
const OWNERS: usize = BUYERS + SELLERS;
/// Two orders per owner per outcome; four Eggs each.
const PER_OWNER_PER_OUTCOME: u64 = 2;
const QUANTITY: u64 = 4;
/// Slices per batch of the pairing pass.
const SLICE_BATCH: u16 = 8;
const LABEL: &str = "max_book";

/// The sixty-four-record book: place it dense across four pages, freeze it,
/// walk it, select it, entitle every slice against the whole page set, and
/// consume all of it with exact conservation.
#[tokio::test]
async fn the_maximum_sixty_four_order_book_clears_across_four_dense_pages() {
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::new(OWNERS);
    let market = lab.market(MarketSpec {
        market_byte: 0x55,
        outcomes: OUTCOMES,
        tick_count: TICK_COUNT,
        tick_spacing: SPACING,
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

    /* The plan: for every outcome, two rounds of four buys and four sells.
     * Buyers are strict at tick 50 (5,000), sellers strict at tick 10 (1,000),
     * and the candidate prices every outcome at a quarter of the scale, so
     * supply equals demand on every outcome and every record fills whole. */
    let buy_limit = market.tick(50);
    let sell_limit = market.tick(10);
    let mut orders: Vec<PlannedOrder> = Vec::with_capacity(MAX_EPOCH_ORDERS);
    let mut rank = 0u64;
    for outcome in 0..OUTCOMES {
        for _ in 0..PER_OWNER_PER_OUTCOME {
            for (buyer, owner) in owners.iter().enumerate().take(BUYERS) {
                rank += 1;
                orders.push((
                    buyer,
                    single(owner.id, rank, outcome, 0, QUANTITY, buy_limit, EPOCH_INDEX),
                ));
            }
            for (seller, owner) in owners.iter().enumerate().skip(BUYERS) {
                rank += 1;
                orders.push((
                    seller,
                    single(
                        owner.id,
                        rank,
                        outcome,
                        1,
                        QUANTITY,
                        sell_limit,
                        EPOCH_INDEX,
                    ),
                ));
            }
        }
    }
    assert_eq!(orders.len(), MAX_EPOCH_ORDERS, "the complete frozen set");

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

    // Every page is dense and the epoch binds the complete four-page set.
    let frozen = frozen_state(&mut context, &plane).await;
    assert_eq!(frozen.epoch.page_count, MAX_ORDER_PAGES as u16);
    assert_eq!(frozen.epoch.order_count, MAX_EPOCH_ORDERS as u16);
    assert_eq!(frozen.epoch.owner_count, OWNERS as u16);
    assert_eq!(frozen.book.len as usize, MAX_EPOCH_ORDERS);
    for (index, bytes) in frozen.page_bytes.iter().enumerate() {
        let header = stream::OrderPageHeader::decode(bytes).unwrap();
        assert_eq!(
            header.order_count as usize, MAX_ORDERS_PER_PAGE,
            "page {index} is dense"
        );
        assert_eq!(header.tombstone_count, 0);
    }
    let refs: Vec<&[u8]> = frozen.page_bytes.iter().map(|page| &page[..]).collect();
    stream::epoch_binds_page_set(&frozen.epoch, &refs).unwrap();

    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..usize::from(OUTCOMES)].fill(PRICE_SCALE / u64::from(OUTCOMES));
    let submission = plan_submission(&plane, &frozen.epoch, &frozen.book, prices, 0, None);
    assert_eq!(
        submission.fills,
        vec![QUANTITY; MAX_EPOCH_ORDERS],
        "supply equals demand on every outcome, so every record fills whole"
    );
    assert_eq!(submission.virtual_split, 0);
    assert_eq!(submission.virtual_merge, 0);
    let slices = submission.witness.len;
    eprintln!("scale.{LABEL} WITNESS declared_slices={slices} orders={MAX_EPOCH_ORDERS}");

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
        &[],
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
    assert_eq!(
        read_record(&mut context, &plane, submission.id)
            .await
            .status,
        CANDIDATE_STATUS_SELECTED
    );
    assert_eq!(
        EpochAccount::decode(&bytes_of(&mut context, plane.epoch_account).await)
            .unwrap()
            .phase,
        EPOCH_PHASE_CLEARED
    );

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
        "every conversion is whole, so the pot freezes empty"
    );

    let cash_before = owner_cash(&mut context, &plane.positions).await;
    let eggs_before: Vec<u64> = {
        let mut all = Vec::new();
        for outcome in 0..usize::from(OUTCOMES) {
            all.push(
                book_eggs(
                    &mut context,
                    &plane.positions,
                    &frozen.reservations,
                    outcome,
                )
                .await,
            );
        }
        all
    };

    /* Every slice, entitled and consumed against the complete four-page set.
     * The entitlement is the page-set-wide route: it re-derives both live
     * orders by walking every bound page, which is the shape whose maximum
     * this campaign exists to measure. */
    for slice_index in 0..slices {
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
            1_200 + u32::from(slice_index),
        )
        .await;
        result.unwrap();
        meter.record(
            &format!("entitle_slice_single_4pages_ledgered_slice{slice_index}"),
            units,
        );

        let (result, units) = send(
            &mut context,
            &[plane.settle_single(
                submission.id,
                plane.positions[buy_owner(&orders, buy)],
                plane.positions[buy_owner(&orders, sell)],
                frozen.reservations[buy],
                frozen.reservations[sell],
                slice_index,
                frozen.live_pages[buy],
            )],
            &[],
            1_300 + u32::from(slice_index),
        )
        .await;
        result.unwrap();
        meter.record(
            &format!("settle_page_direct_4pages_slice{slice_index}"),
            units,
        );
    }

    /* ------------------------------------------------------------------ */
    /* Conservation across the complete book.                              */
    /* ------------------------------------------------------------------ */

    // Each buyer holds two orders per outcome, each four Eggs at one atom;
    // each seller mirrors it.
    let per_outcome_eggs = PER_OWNER_PER_OUTCOME * QUANTITY;
    let per_owner_atoms = PER_OWNER_PER_OUTCOME * u64::from(OUTCOMES);
    for buyer in 0..BUYERS {
        let position = read_position(&mut context, plane.positions[buyer]).await;
        assert_eq!(position.cash_atoms, START_CASH - per_owner_atoms);
        assert_eq!(position.reserved_cash_atoms, 0);
        for outcome in 0..usize::from(OUTCOMES) {
            assert_eq!(position.internal[outcome], START_EGGS + per_outcome_eggs);
        }
    }
    for seller in BUYERS..OWNERS {
        let position = read_position(&mut context, plane.positions[seller]).await;
        assert_eq!(position.cash_atoms, START_CASH + per_owner_atoms);
        assert_eq!(position.reserved_cash_atoms, 0);
        for outcome in 0..usize::from(OUTCOMES) {
            assert_eq!(position.internal[outcome], START_EGGS - per_outcome_eggs);
        }
    }
    assert_eq!(
        owner_cash(&mut context, &plane.positions).await,
        cash_before
    );
    for (outcome, before) in eggs_before.iter().enumerate() {
        assert_eq!(
            book_eggs(
                &mut context,
                &plane.positions,
                &frozen.reservations,
                outcome
            )
            .await,
            *before,
            "outcome {outcome} Egg conservation across the maximum book"
        );
    }
    for (at, address) in frozen.reservations.iter().enumerate() {
        let reservation = read_reservation(&mut context, *address).await;
        assert_eq!(
            reservation.state, RESERVATION_STATE_CONSUMED,
            "reservation {at}"
        );
        assert!(reservation.remaining_is_zero(), "reservation {at} drained");
    }
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, plane.pot()).await).unwrap();
    assert_eq!(pot.rounding_pot_price_units, 0);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);

    meter.finish();
}

/// The Lab owner index that placed the live order at `live` — the plan is
/// tombstone-free, so live rank and plan position coincide.
fn buy_owner(orders: &[PlannedOrder], live: usize) -> usize {
    orders[live].0
}
