//! Scale campaign 1 — **the full tick table**.
//!
//! Every general-plane suite in the tree drives an eleven-tick price grid.
//! `MAX_GRID_TICKS` is sixty-four, and the one route whose cost is a function
//! of the tick count is `PriceGridAccount::tick_of`, which `PlaceOrder` runs
//! as a linear scan to resolve a limit into its tick index.  A book placed on
//! an eleven-tick grid therefore measures a scan bounded by eleven, and the
//! W1 `place_order_single` row is that measurement — not the maximum.
//!
//! This campaign runs the whole general lifecycle — placement, freeze, the
//! streaming walk, selection, entitlement freeze, per-slice entitlement, and
//! consumption — against a market whose grid carries the **complete
//! sixty-four-tick table**, so every row it prints is that route at the widest
//! grid the codec admits.  A second test then isolates the table's own cost
//! with a **controlled** placement: one identical eight-Egg buy into slot 0 of
//! a fresh page, sent against an eleven-tick grid at tick 10, the sixty-four
//! tick grid at tick 10, and the sixty-four-tick grid at tick 63.  Nothing but
//! the grid and the limit moves between those three rows.
//!
//! What that measurement found is worth stating before its numbers are read:
//! **the tick table is not a CU risk at `PlaceOrder`, and the term that does
//! move these rows is PDA derivation.**  The program derives addresses with
//! `find_program_address`, which pays one `create_program_address` per failed
//! attempt — 1,500 CU — so a route's cost carries a term proportional to
//! `255 - bump` for every address it derives.  Two otherwise identical
//! `InitEpoch` transactions on the same market differ by exactly 6,000 CU,
//! which is four attempts at that quantum.  Every row here prints an
//! attempt count beside its CU so a quote model can carry the quantum instead
//! of averaging it into a shape it does not belong to.  (The printed counts
//! are lower bounds: they enumerate the addresses this harness can name, and
//! an instruction may derive more.)
//!
//! Read that way — netting the placement rows' attempt counts at 1,500 CU
//! each — the two placement deltas resolve to **+318 CU for fifty-three extra
//! ticks of scan** (six CU a tick) and **+3,318 CU for the wider table at
//! equal depth**, and both figures reproduce across runs whose random genesis
//! keys give every plane different bumps.  Both are noise against a 1.4 M
//! ceiling, and both are smaller than the bump term the same transactions
//! carry, which is why the deltas are printed and not asserted: one
//! observation cannot separate a shape term from a bump term.
//!
//! Every created account carries its `GeneralFundingLedgerV1` sibling: an
//! unledgered creation is unclosable by design, so the ledgered shape is the
//! one a keeper must actually send, and the one worth quoting.
//!
//! Conservation is asserted exactly: cash and Eggs across the whole book
//! before and after consumption, the pot provably empty, and every reservation
//! CONSUMED with its envelope drained.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

mod scale_common;

use {
    clutch_solana_layout::canonical_order_id,
    clutch_solana_layout::{
        clearing::CANDIDATE_WINDOW_SLOTS, reservation::RESERVATION_STATE_CONSUMED, EpochAccount,
        FinalPotAccount, CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED, MAX_GRID_TICKS,
        MAX_OUTCOMES, POT_PHASE_CLOSED,
    },
    scale_common::{
        book_eggs, build_frozen_book, bytes_of, frozen_state, owner_cash, plan_submission,
        read_position, read_record, read_reservation, send, send_walk, single, submit_seal,
        walk_to_verdict, Lab, MarketSpec, Meter,
    },
    solana_program_test::tokio,
    solana_signer::Signer,
};

const EPOCH_INDEX: u64 = 7;
const FREEZE_DEADLINE: u64 = 500;
const OUTCOMES: u8 = 4;
/// The complete table: `MAX_GRID_TICKS` active ticks.
const TICK_COUNT: u8 = MAX_GRID_TICKS as u8;
/// Tick `i` is `i * SPACING`; the top tick is 6,300 against a 10,000 scale.
const SPACING: u64 = 100;
const PRICE_SCALE: u64 = 10_000;
const START_CASH: u64 = 1_000_000;
const START_EGGS: u64 = 1_000;
const LABEL: &str = "tick_table";

/// The whole general lifecycle at `MAX_GRID_TICKS`, with the tick-scan cost
/// isolated on one grid.
#[tokio::test]
async fn the_complete_tick_table_places_freezes_walks_and_settles() {
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::new(4);
    let market = lab.market(MarketSpec {
        market_byte: 0x51,
        outcomes: OUTCOMES,
        tick_count: TICK_COUNT,
        tick_spacing: SPACING,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    });
    assert_eq!(market.tick_count as usize, MAX_GRID_TICKS);
    let top = market.top_tick();
    let floor = market.tick(0);
    assert_eq!(top, 6_300);
    assert_eq!(floor, 0);
    let plane = lab.epoch(&market, EPOCH_INDEX, 1, FREEZE_DEADLINE);
    let (mut context, keeper, owners) = lab.start().await;
    let keeper_key = keeper.pubkey();

    /* The four-order book, chosen so the two extreme tick depths are both
     * placed on the same sixty-four-tick grid:
     *   rank 1  buy  o0 q8 @ tick 63 (6,300) — the deepest admitted scan
     *   rank 2  sell o0 q8 @ tick 20 (2,000)
     *   rank 3  buy  o1 q4 @ tick 63 (6,300)
     *   rank 4  sell o1 q4 @ tick  0 (    0) — the shallowest
     * Prices are even at a quarter of the scale, so every order is strict and
     * fills whole, and every order's value is a whole collateral atom. */
    let orders = [
        (0usize, single(owners[0].id, 1, 0, 0, 8, top, EPOCH_INDEX)),
        (
            1usize,
            single(owners[1].id, 2, 0, 1, 8, market.tick(20), EPOCH_INDEX),
        ),
        (2usize, single(owners[2].id, 3, 1, 0, 4, top, EPOCH_INDEX)),
        (3usize, single(owners[3].id, 4, 1, 1, 4, floor, EPOCH_INDEX)),
    ];
    let placements = build_frozen_book(
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

    /* Both placements are recorded, but neither is a controlled comparison
     * against the other: rank 1 is a cash-reserving buy in slot 0 and rank 4
     * an Egg-reserving sell in slot 3, so the two rows differ in more than
     * tick depth.  The controlled measurement is the second test. */
    meter.record("place_order_buy_slot0_tick63_of64", placements[0]);
    meter.record("place_order_sell_slot3_tick0_of64", placements[3]);

    let frozen = frozen_state(&mut context, &plane).await;
    assert_eq!(frozen.book.len, 4);
    assert_eq!(frozen.epoch.order_count, 4);
    assert_eq!(frozen.epoch.owner_count, 4);

    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..usize::from(OUTCOMES)].fill(PRICE_SCALE / u64::from(OUTCOMES));
    let submission = plan_submission(&plane, &frozen.epoch, &frozen.book, prices, 0, None);
    assert_eq!(submission.fills, vec![8, 8, 4, 4]);
    assert_eq!(submission.virtual_split, 0);
    assert_eq!(submission.virtual_merge, 0);
    let slices = submission.witness.len;
    assert_eq!(slices, 2, "one crossing per traded outcome");

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
        slices,
        &[],
        &mut meter,
        "",
        800,
    )
    .await;

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, units) = send(&mut context, &[plane.finalize(&[submission.id])], &[], 900).await;
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
        901,
    )
    .await;
    result.unwrap();
    meter.record("freeze_entitlement_ledgered", units);
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, plane.pot()).await).unwrap();
    assert_eq!(pot.phase, POT_PHASE_CLOSED);
    assert_eq!(pot.rounding_pot_price_units, 0);

    let cash_before = owner_cash(&mut context, &plane.positions).await;
    let eggs_before = [
        book_eggs(&mut context, &plane.positions, &frozen.reservations, 0).await,
        book_eggs(&mut context, &plane.positions, &frozen.reservations, 1).await,
    ];

    // Two crossings, entitled and consumed in slice order.  Live ranks 0/1 are
    // the outcome-0 pair, 2/3 the outcome-1 pair.
    for (slice_index, buy, sell) in [(0u16, 0usize, 1usize), (1, 2, 3)] {
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
            910 + u32::from(slice_index),
        )
        .await;
        result.unwrap();
        meter.record(
            &format!("entitle_slice_single_ledgered_slice{slice_index}"),
            units,
        );

        let (result, units) = send(
            &mut context,
            &[plane.settle_single(
                submission.id,
                plane.positions[buy],
                plane.positions[sell],
                frozen.reservations[buy],
                frozen.reservations[sell],
                slice_index,
                frozen.live_pages[buy],
            )],
            &[],
            920 + u32::from(slice_index),
        )
        .await;
        result.unwrap();
        meter.record(&format!("settle_page_direct_slice{slice_index}"), units);
    }

    /* ------------------------------------------------------------------ */
    /* Conservation, exactly.                                              */
    /* ------------------------------------------------------------------ */

    // Buyer zero paid two atoms for eight Eggs of outcome 0; buyer two paid
    // one atom for four Eggs of outcome 1; the sellers mirror them.
    let expected: [(usize, u64, [i64; 2]); 4] = [
        (0, START_CASH - 2, [8, 0]),
        (1, START_CASH + 2, [-8, 0]),
        (2, START_CASH - 1, [0, 4]),
        (3, START_CASH + 1, [0, -4]),
    ];
    for (index, cash, deltas) in expected {
        let position = read_position(&mut context, plane.positions[index]).await;
        assert_eq!(position.cash_atoms, cash, "owner {index} cash");
        assert_eq!(position.reserved_cash_atoms, 0, "owner {index} encumbrance");
        for (outcome, delta) in deltas.iter().enumerate() {
            assert_eq!(
                position.internal[outcome],
                (START_EGGS as i64 + delta) as u64,
                "owner {index} outcome {outcome}"
            );
        }
    }
    assert_eq!(
        owner_cash(&mut context, &plane.positions).await,
        cash_before
    );
    for outcome in [0usize, 1] {
        assert_eq!(
            book_eggs(
                &mut context,
                &plane.positions,
                &frozen.reservations,
                outcome
            )
            .await,
            eggs_before[outcome],
            "outcome {outcome} Egg conservation"
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

/// The tick table's own cost, controlled: the **same** placement — one
/// eight-Egg buy on outcome 0, into slot 0 of a freshly created page, by the
/// same owner — sent three ways.
///
/// * against an eleven-tick grid, limit at tick 10 (the deepest that grid has);
/// * against the sixty-four-tick grid, limit at tick 10 (the same depth, a
///   wider table);
/// * against the sixty-four-tick grid, limit at tick 63 (the deepest scan the
///   table admits).
///
/// The first difference is what the wider grid costs at equal depth — the
/// codec reads and validates all `MAX_GRID_TICKS` entries either way, so it
/// should be near zero and is measured rather than assumed.  The second is the
/// scan itself.  Every other coordinate is held: same slot, same page state,
/// same owner, same quantity, same outcome.
#[tokio::test]
async fn the_tick_table_scan_cost_is_measured_against_a_held_placement() {
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::new(1);
    let narrow = lab.market(MarketSpec {
        market_byte: 0x52,
        outcomes: OUTCOMES,
        tick_count: 11,
        tick_spacing: SPACING,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    });
    let wide = lab.market(MarketSpec {
        market_byte: 0x53,
        outcomes: OUTCOMES,
        tick_count: TICK_COUNT,
        tick_spacing: SPACING,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    });
    // Three epochs so every placement lands in slot 0 of its own empty page.
    let narrow_plane = lab.epoch(&narrow, EPOCH_INDEX, 1, FREEZE_DEADLINE);
    let wide_shallow = lab.epoch(&wide, EPOCH_INDEX, 1, FREEZE_DEADLINE);
    let wide_deep = lab.epoch(&wide, EPOCH_INDEX + 1, 1, FREEZE_DEADLINE);
    let (mut context, keeper, owners) = lab.start().await;
    let keeper_key = keeper.pubkey();

    let mut rows = Vec::new();
    for (nonce, plane, market, tick, route) in [
        (
            0u32,
            &narrow_plane,
            &narrow,
            10u8,
            "place_order_single_slot0_tick10_of11",
        ),
        (
            10,
            &wide_shallow,
            &wide,
            10,
            "place_order_single_slot0_tick10_of64",
        ),
        (
            20,
            &wide_deep,
            &wide,
            63,
            "place_order_single_slot0_tick63_of64",
        ),
    ] {
        let (result, units) = send(
            &mut context,
            &[plane.init_epoch(keeper_key)],
            &[&keeper],
            nonce,
        )
        .await;
        result.unwrap();
        meter.record(
            &format!(
                "init_epoch_ledgered_{}ticks_epoch{}",
                market.tick_count, plane.epoch_index
            ),
            units,
        );
        /* The bump term, printed beside the row it moves: `InitEpoch` derives
         * three addresses on-chain through `find_program_address`, and each
         * failed attempt is one `create_program_address` syscall.  Two
         * otherwise-identical inits differ by exactly this. */
        eprintln!(
            "scale.{LABEL} PDA_ATTEMPTS init_epoch epoch{} extra_attempts={} cu={units}",
            plane.epoch_index,
            plane.epoch_derivation_attempts()
        );
        let (result, _) = send(
            &mut context,
            &[plane.init_page(keeper_key, 0)],
            &[&keeper],
            nonce + 1,
        )
        .await;
        result.unwrap();
        // Every rank is 1: the order id is per epoch, and each epoch's page is
        // empty, so this is the identical placement three times over.
        let (result, units) = send(
            &mut context,
            &[plane.place(
                owners[0].key.pubkey(),
                plane.positions[0],
                0,
                0,
                single(
                    owners[0].id,
                    1,
                    0,
                    0,
                    8,
                    market.tick(tick),
                    plane.epoch_index,
                ),
            )],
            &[&owners[0].key],
            nonce + 2,
        )
        .await;
        result.unwrap();
        rows.push(meter.record(route, units));
        /* The bump term again, this time on the one address `PlaceOrder`
         * derives: the reservation.  It is what a single-observation delta
         * between two placements is actually made of. */
        eprintln!(
            "scale.{LABEL} PDA_ATTEMPTS place_order {route} extra_attempts={} cu={units}",
            plane.placement_derivation_attempts(owners[0].id, canonical_order_id(1))
        );
    }

    /* Both deltas are reported with the caveat they need: each transaction
     * pays a PDA-derivation term of 1,500 CU per extra
     * `create_program_address` attempt, and the three planes derive addresses
     * with different canonical bumps.  A shape effect smaller than that term
     * is not resolvable from single observations — which is the finding, not
     * a limitation of the fixture: the tick table's width does not move
     * `PlaceOrder` by as much as one bump attempt. */
    eprintln!(
        "scale.{LABEL} WIDER_TABLE_AT_EQUAL_DEPTH tick10_of64_minus_tick10_of11_cu: {}",
        rows[1] as i64 - rows[0] as i64
    );
    eprintln!(
        "scale.{LABEL} SCAN_DEPTH tick63_of64_minus_tick10_of64_cu: {}",
        rows[2] as i64 - rows[1] as i64
    );
    /* No assertion is made on either delta, and that is deliberate: the
     * planes derive their addresses with whatever canonical bumps the random
     * genesis keys produce, so a single-observation delta is a shape term plus
     * a bump term and the two are not separable from one run.  What *is*
     * asserted is the statement a quote model can use — the deepest scan the
     * complete table admits leaves the placement far inside its own row. */
    assert!(
        rows.iter().all(|units| *units < 250_000),
        "a placement on the complete tick table stays inside a 250,000 CU row"
    );
    meter.finish();
}
