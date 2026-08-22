//! Scale campaign 6 — **the rounding pot at scale**.
//!
//! `entitled_clearing.rs` funds and drains the pot on the smallest inexact
//! book there is: one buyer, one seller, one slice, one atom of residue.  The
//! claim that matters at scale is different — that a pot funded by **many**
//! owners drains to exactly zero across **many** completions, never goes
//! negative, and leaves the book short by exactly the verified expectation and
//! not one atom more.
//!
//! The book: twelve independent inexact pairs on outcome 0, twenty-four owners,
//! one order each, across two pages.  Each buyer wants five Eggs at half the
//! scale, each seller offers five at a fifth, and the candidate prices the
//! outcome at a quarter — so each end's whole-order value is 12,500 price
//! units against a scale of 10,000.  The relation's terminal-owner conversion
//! rounds each payer **up** to two atoms and each payee **down** to one, so the
//! verified `rounding_pot` is twelve whole atoms: 120,000 price units.
//!
//! Every participating owner holds exactly one filled order, which is the
//! checked coincidence `EntitleSlice` requires before it will mint a receipt
//! for an inexact end (`distinct_owners == filled_order_count`, recomputed
//! from the digest-verified feed).  Twenty-four owners, twenty-four filled
//! orders, twelve completions, and the pot walks 120,000 → 0 in twelve exact
//! steps of 10,000.
//!
//! The pot holds no value at any point: it is created OPEN carrying the
//! expectation, each completing slice draws its share down, and the atoms are
//! simply never credited — the book ends twelve atoms lighter and they stay
//! unallocated in the market's collateral pool.  Eggs conserve exactly.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

mod scale_common;

use {
    clutch_sbf::error::ClutchError,
    clutch_solana_layout::{
        clearing::CANDIDATE_WINDOW_SLOTS, reservation::RESERVATION_STATE_CONSUMED, FinalPotAccount,
        SettlementReceiptAccount, MAX_GRID_TICKS, MAX_OUTCOMES, POT_PHASE_OPEN,
    },
    scale_common::{
        account, book_eggs, build_frozen_book, bytes_of, custom, frozen_state, owner_cash,
        plan_submission, program_account, read_position, read_reservation, send, send_walk, single,
        slice_ends, snapshot, submit_seal, walk_to_verdict, Lab, MarketSpec, Meter, PlannedOrder,
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
/// Twelve inexact pairs: twenty-four owners, one filled order each.
const PAIRS: usize = 12;
const OWNERS: usize = 2 * PAIRS;
const QUANTITY: u64 = 5;
/// Each pair leaves exactly one collateral atom unallocated.
const RESIDUE_PER_PAIR: u128 = PRICE_SCALE as u128;
const LABEL: &str = "pot";

/// A twelve-atom rounding pot, funded by twenty-four owners and drained to
/// zero across twelve completions.
#[tokio::test]
async fn a_large_rounding_pot_drains_to_zero_across_many_completions() {
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::new(OWNERS);
    let market = lab.market(MarketSpec {
        market_byte: 0x5c,
        outcomes: OUTCOMES,
        tick_count: TICK_COUNT,
        tick_spacing: SPACING,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    });
    let plane = lab.epoch(&market, EPOCH_INDEX, 2, FREEZE_DEADLINE);
    let (mut context, keeper, owners) = lab.start().await;
    let keeper_key = keeper.pubkey();

    let price = PRICE_SCALE / u64::from(OUTCOMES);
    let buy_limit = market.tick(50);
    let sell_limit = market.tick(20);

    // Buyers first, then sellers: every owner holds exactly one order, which is
    // the coincidence the inexact route requires.
    let mut orders: Vec<PlannedOrder> = Vec::with_capacity(OWNERS);
    for (buyer, owner) in owners.iter().enumerate().take(PAIRS) {
        orders.push((
            buyer,
            single(
                owner.id,
                orders.len() as u64 + 1,
                0,
                0,
                QUANTITY,
                buy_limit,
                EPOCH_INDEX,
            ),
        ));
    }
    for (seller, owner) in owners.iter().enumerate().skip(PAIRS) {
        orders.push((
            seller,
            single(
                owner.id,
                orders.len() as u64 + 1,
                0,
                1,
                QUANTITY,
                sell_limit,
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
        "",
        0,
    )
    .await;
    let frozen = frozen_state(&mut context, &plane).await;
    assert_eq!(frozen.epoch.order_count as usize, OWNERS);
    assert_eq!(frozen.epoch.owner_count as usize, OWNERS);

    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..usize::from(OUTCOMES)].fill(price);
    let submission = plan_submission(&plane, &frozen.epoch, &frozen.book, prices, 0, None);
    assert_eq!(
        submission.fills,
        vec![QUANTITY; OWNERS],
        "supply equals demand, so every order fills whole and inexactly"
    );
    let slices = submission.witness.len;
    assert_eq!(slices as usize, PAIRS, "one slice per inexact pair");

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

    // The freeze records the verified expectation rather than refusing it: the
    // pot opens carrying twelve atoms of residue.
    let (result, units) = send_walk(
        &mut context,
        plane.freeze_entitlement(keeper_key, submission.id),
        &[&keeper],
        1_101,
    )
    .await;
    result.unwrap();
    meter.record("freeze_entitlement_inexact_ledgered", units);
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, plane.pot()).await).unwrap();
    let expected_pot = RESIDUE_PER_PAIR * PAIRS as u128;
    assert_eq!(pot.rounding_pot_price_units, expected_pot);
    assert_eq!(pot.phase, POT_PHASE_OPEN);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
    eprintln!(
        "scale.{LABEL} POT opened_price_units={} atoms={}",
        pot.rounding_pot_price_units,
        pot.rounding_pot_price_units / u128::from(PRICE_SCALE)
    );

    let cash_before = owner_cash(&mut context, &plane.positions).await;
    let eggs_before = book_eggs(&mut context, &plane.positions, &frozen.reservations, 0).await;

    /* The drain, one completion at a time.  Every slice completes both of its
     * ends, so every one of them realizes exactly one atom of the pot. */
    let mut nonce = 1_200u32;
    let mut remaining = expected_pot;
    for slice_index in 0..slices {
        let (buy, sell) = slice_ends(&submission.witness, slice_index);
        let buy_position = plane.positions[orders[buy].0];
        let sell_position = plane.positions[orders[sell].0];

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
        meter.record(
            &format!("entitle_slice_single_inexact_ledgered_slice{slice_index}"),
            units,
        );
        // The receipt records the exact price-unit value; nothing rounds here.
        let receipt = SettlementReceiptAccount::decode(
            &bytes_of(&mut context, plane.receipt(submission.id, slice_index)).await,
        )
        .unwrap();
        assert_eq!(receipt.consideration_price_units, 12_500);

        if slice_index == 0 {
            // The seven-account shape cannot realize residue it has no pot to
            // draw from, and refuses before a byte moves.
            let watched = [
                buy_position,
                sell_position,
                frozen.reservations[buy],
                frozen.reservations[sell],
                plane.pot(),
            ];
            let before = snapshot(&mut context, &watched).await;
            let (result, _) = send(
                &mut context,
                &[plane.settle_single(
                    submission.id,
                    buy_position,
                    sell_position,
                    frozen.reservations[buy],
                    frozen.reservations[sell],
                    slice_index,
                    frozen.live_pages[buy],
                )],
                &[],
                nonce,
            )
            .await;
            nonce += 1;
            assert_eq!(custom(result), ClutchError::AccountCount as u32);
            assert_eq!(snapshot(&mut context, &watched).await, before);

            // A pot short of what the slice realizes refuses rather than going
            // negative.
            let honest = bytes_of(&mut context, plane.pot()).await;
            let mut short_value =
                FinalPotAccount::decode(&bytes_of(&mut context, plane.pot()).await).unwrap();
            short_value.rounding_pot_price_units = RESIDUE_PER_PAIR / 2;
            let mut short = vec![0u8; clutch_solana_layout::account_len::FINAL_POT];
            short_value.encode(&mut short).unwrap();
            context.set_account(&plane.pot(), &program_account(short).into());
            let (result, _) = send(
                &mut context,
                &[plane.settle_single_potted(
                    submission.id,
                    buy_position,
                    sell_position,
                    frozen.reservations[buy],
                    frozen.reservations[sell],
                    slice_index,
                    frozen.live_pages[buy],
                )],
                &[],
                nonce,
            )
            .await;
            nonce += 1;
            assert_eq!(
                custom(result),
                ClutchError::AggregateClosureMismatch as u32,
                "a short pot refuses rather than going negative"
            );
            context.set_account(&plane.pot(), &program_account(honest).into());
        }

        let (result, units) = send(
            &mut context,
            &[plane.settle_single_potted(
                submission.id,
                buy_position,
                sell_position,
                frozen.reservations[buy],
                frozen.reservations[sell],
                slice_index,
                frozen.live_pages[buy],
            )],
            &[],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(&format!("settle_page_potted_slice{slice_index}"), units);

        // The pot stepped down by exactly one pair's residue, and never below.
        remaining -= RESIDUE_PER_PAIR;
        let pot = FinalPotAccount::decode(&bytes_of(&mut context, plane.pot()).await).unwrap();
        assert_eq!(
            pot.rounding_pot_price_units, remaining,
            "the pot drew down exactly one atom on slice {slice_index}"
        );
        assert_eq!(pot.pot_cash_price_units, 0);
        assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);

        // Both ends completed; the payer paid two atoms and the payee took one.
        for (label, address) in [
            ("buy", frozen.reservations[buy]),
            ("sell", frozen.reservations[sell]),
        ] {
            let reservation = read_reservation(&mut context, address).await;
            assert_eq!(
                reservation.state, RESERVATION_STATE_CONSUMED,
                "{label} end of slice {slice_index}"
            );
            assert_eq!(reservation.consumed_units, QUANTITY);
            assert!(reservation.remaining_is_zero());
        }
        assert_eq!(
            read_position(&mut context, buy_position).await.cash_atoms,
            START_CASH - 2
        );
        assert_eq!(
            read_position(&mut context, sell_position).await.cash_atoms,
            START_CASH + 1
        );

        // Eggs conserve at every boundary; cash is short exactly the realized
        // residue and no more.
        assert_eq!(
            book_eggs(&mut context, &plane.positions, &frozen.reservations, 0).await,
            eggs_before,
            "Eggs conserved after slice {slice_index}"
        );
        let realized = (expected_pot - remaining) / u128::from(PRICE_SCALE);
        assert_eq!(
            owner_cash(&mut context, &plane.positions).await,
            cash_before - realized as u64,
            "cash short exactly the realized residue after slice {slice_index}"
        );
    }

    /* ------------------------------------------------------------------ */
    /* The pot, empty.                                                     */
    /* ------------------------------------------------------------------ */

    assert_eq!(remaining, 0);
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, plane.pot()).await).unwrap();
    assert_eq!(pot.rounding_pot_price_units, 0, "drained to zero");
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
    eprintln!(
        "scale.{LABEL} POT drained_to={} completions={PAIRS}",
        pot.rounding_pot_price_units
    );

    // Eggs conserve exactly; cash conserves less exactly the verified pot,
    // which nothing holds.
    assert_eq!(
        book_eggs(&mut context, &plane.positions, &frozen.reservations, 0).await,
        eggs_before
    );
    assert_eq!(
        owner_cash(&mut context, &plane.positions).await,
        cash_before - (expected_pot / u128::from(PRICE_SCALE)) as u64
    );
    for buyer in 0..PAIRS {
        let position = read_position(&mut context, plane.positions[buyer]).await;
        assert_eq!(position.cash_atoms, START_CASH - 2);
        assert_eq!(position.reserved_cash_atoms, 0);
        assert_eq!(position.internal[0], START_EGGS + QUANTITY);
    }
    for seller in PAIRS..OWNERS {
        let position = read_position(&mut context, plane.positions[seller]).await;
        assert_eq!(position.cash_atoms, START_CASH + 1);
        assert_eq!(position.reserved_cash_atoms, 0);
        assert_eq!(position.internal[0], START_EGGS - QUANTITY);
    }

    // A replay on the last exhausted receipt refuses with the drained pot
    // untouched.
    let (buy, sell) = slice_ends(&submission.witness, slices - 1);
    let pot_before = bytes_of(&mut context, plane.pot()).await;
    let (result, _) = send(
        &mut context,
        &[plane.settle_single_potted(
            submission.id,
            plane.positions[orders[buy].0],
            plane.positions[orders[sell].0],
            frozen.reservations[buy],
            frozen.reservations[sell],
            slices - 1,
            frozen.live_pages[buy],
        )],
        &[],
        nonce,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(bytes_of(&mut context, plane.pot()).await, pot_before);
    assert!(account(&mut context, plane.pot()).await.is_some());

    meter.finish();
}
