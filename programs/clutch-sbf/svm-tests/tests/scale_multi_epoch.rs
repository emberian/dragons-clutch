//! Scale campaign 5 — **many epochs live at once**.
//!
//! Every general-plane suite in the tree drives exactly one epoch of exactly
//! one market on its bank.  Nothing there could have caught state bleeding
//! between two live epochs, because there has never been a second one.
//!
//! This campaign runs **three** epochs concurrently on one bank:
//!
//! * `A7` and `A8` — two epochs of the *same* market, so they share every
//!   owner's Position and both encumber it at once;
//! * `B7` — an epoch of a second market, with its own Positions.
//!
//! They are driven interleaved at every stage a keeper would interleave them:
//! placement round-robins across all three, the freeze walks all three, the
//! clearing walk cranks each one batch at a time in rotation, selection closes
//! all three, and entitlement and consumption alternate slice by slice.  Each
//! epoch's final allocation is then asserted to be exactly what it would have
//! been alone, and market B's positions are asserted to carry only B's effect.
//!
//! The hostile half — every cross-epoch presentation the account lists make
//! *syntactically* possible:
//!
//! * a freeze of `A7` presenting `A8`'s page;
//! * a clearing advance of `A7` presenting `B7`'s page;
//! * a clearing advance of `A8` presenting `A7`'s candidate feed;
//! * an entitlement in `A8` presenting `A7`'s reservation;
//! * a consumption in `B7` presenting a receipt minted under `A7`;
//! * a finalize of `A8` presenting `A7`'s retained candidate record.
//!
//! Each is an executed refusal on the same bank, and each is followed by the
//! honest form landing, so the refusal is the binding and not an accident of
//! ordering.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

mod scale_common;

use {
    clutch_solana_layout::{
        clearing::CANDIDATE_WINDOW_SLOTS, reservation::RESERVATION_STATE_CONSUMED, EpochAccount,
        FinalPotAccount, Hash32, CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED,
        EPOCH_PHASE_FROZEN, MAX_GRID_TICKS, MAX_OUTCOMES, POT_PHASE_CLOSED,
    },
    scale_common::{
        bytes_of, create_checkpoint, custom, frozen_state, plan_submission, read_position,
        read_record, read_reservation, send, send_walk, single, slice_ends, submit_seal, Frozen,
        Lab, MarketSpec, Meter, Plane, PlannedOrder, Submission,
    },
    solana_program_test::tokio,
    solana_signer::Signer,
};

const FREEZE_DEADLINE: u64 = 500;
const OUTCOMES: u8 = 4;
const TICK_COUNT: u8 = MAX_GRID_TICKS as u8;
const SPACING: u64 = 100;
const PRICE_SCALE: u64 = 10_000;
const START_CASH: u64 = 1_000_000;
const START_EGGS: u64 = 1_000;
const OWNERS: usize = 4;
const QUANTITY: u64 = 4;
const LABEL: &str = "multi_epoch";

/// One live epoch under the crank.
struct Live {
    name: &'static str,
    plane: Plane,
    orders: Vec<PlannedOrder>,
    frozen: Option<Frozen>,
    submission: Option<Submission>,
}

impl Live {
    fn frozen(&self) -> &Frozen {
        self.frozen.as_ref().expect("frozen")
    }
    fn submission(&self) -> &Submission {
        self.submission.as_ref().expect("submitted")
    }
}

/// Two buys and two sells on outcome 0, four Eggs each — one whole collateral
/// atom per order, so every epoch's arithmetic is exact on its own.
fn book_plan(owner_ids: &[Hash32], epoch_index: u64, buy: u64, sell: u64) -> Vec<PlannedOrder> {
    vec![
        (0, single(owner_ids[0], 1, 0, 0, QUANTITY, buy, epoch_index)),
        (1, single(owner_ids[1], 2, 0, 0, QUANTITY, buy, epoch_index)),
        (
            2,
            single(owner_ids[2], 3, 0, 1, QUANTITY, sell, epoch_index),
        ),
        (
            3,
            single(owner_ids[3], 4, 0, 1, QUANTITY, sell, epoch_index),
        ),
    ]
}

/// Three epochs, two markets, one bank: interleaved to completion, with no
/// state crossing between them.
#[tokio::test]
async fn three_concurrent_epochs_clear_without_cross_epoch_bleed() {
    let mut meter = Meter::new(LABEL);
    let mut lab = Lab::new(OWNERS);
    let spec = |market_byte| MarketSpec {
        market_byte,
        outcomes: OUTCOMES,
        tick_count: TICK_COUNT,
        tick_spacing: SPACING,
        price_scale: PRICE_SCALE,
        start_cash: START_CASH,
        start_eggs: START_EGGS,
    };
    let market_a = lab.market(spec(0x60));
    let market_b = lab.market(spec(0x61));
    assert_ne!(
        market_a.positions[0], market_b.positions[0],
        "each market owns its own Positions"
    );
    let a7 = lab.epoch(&market_a, 7, 1, FREEZE_DEADLINE);
    let a8 = lab.epoch(&market_a, 8, 1, FREEZE_DEADLINE);
    let b7 = lab.epoch(&market_b, 7, 1, FREEZE_DEADLINE);
    assert_ne!(a7.epoch_id, a8.epoch_id);
    assert_ne!(a7.epoch_id, b7.epoch_id);
    let (mut context, keeper, owners) = lab.start().await;
    let keeper_key = keeper.pubkey();

    let owner_ids: Vec<Hash32> = owners.iter().map(|owner| owner.id).collect();
    let buy_limit = market_a.tick(50);
    let sell_limit = market_a.tick(10);
    let mut live = vec![
        Live {
            name: "a7",
            orders: book_plan(&owner_ids, a7.epoch_index, buy_limit, sell_limit),
            plane: a7,
            frozen: None,
            submission: None,
        },
        Live {
            name: "a8",
            orders: book_plan(&owner_ids, a8.epoch_index, buy_limit, sell_limit),
            plane: a8,
            frozen: None,
            submission: None,
        },
        Live {
            name: "b7",
            orders: book_plan(&owner_ids, b7.epoch_index, buy_limit, sell_limit),
            plane: b7,
            frozen: None,
            submission: None,
        },
    ];

    /* ------------------------------------------------------------------ */
    /* Creation and placement, round-robin.                                */
    /* ------------------------------------------------------------------ */

    let mut nonce = 0u32;
    for entry in &live {
        let (result, units) = send(
            &mut context,
            &[entry.plane.init_epoch(keeper_key)],
            &[&keeper],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(&format!("{}.init_epoch_ledgered", entry.name), units);
        let (result, units) = send(
            &mut context,
            &[entry.plane.init_page(keeper_key, 0)],
            &[&keeper],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(&format!("{}.init_order_page_ledgered", entry.name), units);
    }

    // The interleaving that matters: slot k of every epoch, then slot k + 1.
    for slot in 0..4u64 {
        for entry in &live {
            let (owner_index, order) = entry.orders[slot as usize];
            let owner = &owners[owner_index];
            let (result, units) = send(
                &mut context,
                &[entry.plane.place(
                    owner.key.pubkey(),
                    entry.plane.positions[owner_index],
                    0,
                    slot,
                    order,
                )],
                &[&owner.key],
                nonce,
            )
            .await;
            result.unwrap();
            nonce += 1;
            if slot == 0 {
                meter.record(&format!("{}.place_order_single", entry.name), units);
            }
        }
    }

    // Two epochs of one market both encumber the same Position at once: the
    // sellers' Eggs are out of their Positions twice over.
    let seller_a = read_position(&mut context, live[0].plane.positions[2]).await;
    assert_eq!(
        seller_a.internal[0],
        START_EGGS - 2 * QUANTITY,
        "A7 and A8 each hold four of this seller's Eggs"
    );
    let seller_b = read_position(&mut context, live[2].plane.positions[2]).await;
    assert_eq!(
        seller_b.internal[0],
        START_EGGS - QUANTITY,
        "market B's Position carries only B's encumbrance"
    );

    /* ------------------------------------------------------------------ */
    /* The freeze, with the cross-epoch page refusal first.                */
    /* ------------------------------------------------------------------ */

    context.warp_to_slot(FREEZE_DEADLINE).unwrap();

    // HOSTILE: A7's freeze presenting A8's page.
    let mut crossed = live[0].plane.freeze();
    crossed.accounts[3] = solana_instruction::AccountMeta::new(live[1].plane.pages[0], false);
    let (result, _) = send(&mut context, &[crossed], &[], nonce).await;
    nonce += 1;
    let code = custom(result);
    eprintln!("scale.{LABEL} CROSS_EPOCH freeze_a7_with_a8_page refusal=0x{code:x}");
    assert_eq!(
        code,
        clutch_sbf::error::ClutchError::MismatchedState as u32,
        "a page of a different epoch is not this epoch's page"
    );
    assert_eq!(
        EpochAccount::decode(&bytes_of(&mut context, live[0].plane.epoch_account).await)
            .unwrap()
            .order_set,
        Hash32::ZERO,
        "the refused freeze sealed nothing"
    );

    for entry in &live {
        let (result, units) = send(&mut context, &[entry.plane.freeze()], &[], nonce).await;
        result.unwrap();
        nonce += 1;
        meter.record(&format!("{}.freeze_epoch_1page_4orders", entry.name), units);
    }
    for entry in live.iter_mut() {
        let frozen = frozen_state(&mut context, &entry.plane).await;
        assert_eq!(frozen.epoch.phase, EPOCH_PHASE_FROZEN);
        assert_eq!(frozen.book.len, 4);
        entry.frozen = Some(frozen);
    }
    // Three distinct frozen sets: same book shape, different epoch identities.
    assert_ne!(
        live[0].frozen().epoch.order_set,
        live[1].frozen().epoch.order_set
    );

    /* ------------------------------------------------------------------ */
    /* Submission and the interleaved walk.                                */
    /* ------------------------------------------------------------------ */

    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..usize::from(OUTCOMES)].fill(PRICE_SCALE / u64::from(OUTCOMES));
    for (at, entry) in live.iter_mut().enumerate() {
        let frozen = entry.frozen.as_ref().unwrap();
        let submission =
            plan_submission(&entry.plane, &frozen.epoch, &frozen.book, prices, 0, None);
        assert_eq!(submission.fills, vec![QUANTITY; 4]);
        entry.submission = Some(submission);
        let _ = at;
    }
    // Distinct epochs give distinct candidate identities for the same prices.
    assert_ne!(live[0].submission().id, live[1].submission().id);
    assert_ne!(live[0].submission().id, live[2].submission().id);

    for (at, entry) in live.iter().enumerate() {
        let (result, _) = submit_seal(
            &mut context,
            &entry.plane,
            &keeper,
            entry.submission(),
            Some(entry.submission().witness.len),
            &[],
            None,
            &mut meter,
            entry.name,
            1_000 + 400 * at as u32,
        )
        .await;
        result.unwrap();
    }

    for (at, entry) in live.iter().enumerate() {
        create_checkpoint(
            &mut context,
            &entry.plane,
            &keeper,
            entry.submission().id,
            &mut meter,
            entry.name,
            3_000 + 10 * at as u32,
        )
        .await;
    }

    // HOSTILE: A7's advance presenting B7's page.
    let mut crossed = live[0].plane.advance(
        live[0].submission().id,
        0,
        16,
        &live[0].frozen().reservations,
    );
    crossed.accounts[3] =
        solana_instruction::AccountMeta::new_readonly(live[2].plane.pages[0], false);
    let (result, _) = send_walk(&mut context, crossed, &[], nonce).await;
    nonce += 1;
    let code = custom(result);
    eprintln!("scale.{LABEL} CROSS_EPOCH advance_a7_with_b7_page refusal=0x{code:x}");
    assert_eq!(
        code,
        clutch_sbf::error::ClutchError::MismatchedState as u32,
        "another market's page is not this epoch's page"
    );

    // HOSTILE: A8's advance presenting A7's candidate feed.
    let mut crossed = live[1].plane.advance(
        live[1].submission().id,
        0,
        16,
        &live[1].frozen().reservations,
    );
    crossed.accounts[1] = solana_instruction::AccountMeta::new_readonly(
        live[0].plane.candidate_feed(live[0].submission().id),
        false,
    );
    let (result, _) = send_walk(&mut context, crossed, &[], nonce).await;
    nonce += 1;
    let feed_code = custom(result);
    eprintln!("scale.{LABEL} CROSS_EPOCH advance_a8_with_a7_feed refusal=0x{feed_code:x}");
    assert_eq!(
        feed_code,
        clutch_sbf::error::ClutchError::MismatchedState as u32,
        "another epoch's sealed feed is not this candidate's feed"
    );

    // The honest interleaved crank: one batch per epoch, in rotation.
    for entry in &live {
        let (result, units) = send_walk(
            &mut context,
            entry
                .plane
                .advance(entry.submission().id, 0, 16, &entry.frozen().reservations),
            &[],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(
            &format!("{}.advance_pass1_page0_4orders", entry.name),
            units,
        );
    }
    for entry in &live {
        let (result, units) = send_walk(
            &mut context,
            entry
                .plane
                .advance_slices(entry.submission().id, entry.submission().witness.len),
            &[],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(&format!("{}.advance_slices", entry.name), units);
    }
    for entry in &live {
        let (result, units) = send_walk(
            &mut context,
            entry.plane.advance(entry.submission().id, 0, 16, &[]),
            &[],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(&format!("{}.advance_pass2_page0", entry.name), units);
    }
    for entry in &live {
        let (result, units) = send_walk(
            &mut context,
            entry.plane.complete(entry.submission().id),
            &[],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(&format!("{}.complete_clear_work", entry.name), units);
    }

    /* ------------------------------------------------------------------ */
    /* Selection, with the cross-epoch registry refusal first.             */
    /* ------------------------------------------------------------------ */

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();

    /* HOSTILE: A8's finalize presenting A7's *existing* retained record and
     * feed in its registry slot.  Both accounts are real, program-owned, and
     * sealed — they simply bind a different epoch. */
    let mut crossed = live[1].plane.finalize(&[live[1].submission().id]);
    crossed.accounts[3] = solana_instruction::AccountMeta::new(
        live[0].plane.candidate_record(live[0].submission().id),
        false,
    );
    crossed.accounts[4] = solana_instruction::AccountMeta::new_readonly(
        live[0].plane.candidate_feed(live[0].submission().id),
        false,
    );
    let (result, _) = send(&mut context, &[crossed], &[], nonce).await;
    nonce += 1;
    let code = custom(result);
    eprintln!("scale.{LABEL} CROSS_EPOCH finalize_a8_with_a7_record refusal=0x{code:x}");
    assert_eq!(
        code,
        clutch_sbf::error::ClutchError::MismatchedState as u32,
        "another epoch's candidate record is not this registry entry"
    );
    assert_eq!(
        EpochAccount::decode(&bytes_of(&mut context, live[1].plane.epoch_account).await)
            .unwrap()
            .phase,
        EPOCH_PHASE_FROZEN,
        "the refused finalize left the phase alone"
    );

    for entry in &live {
        let (result, units) = send(
            &mut context,
            &[entry.plane.finalize(&[entry.submission().id])],
            &[],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(
            &format!("{}.finalize_selection_1retained", entry.name),
            units,
        );
        assert_eq!(
            read_record(&mut context, &entry.plane, entry.submission().id)
                .await
                .status,
            CANDIDATE_STATUS_SELECTED
        );
        assert_eq!(
            EpochAccount::decode(&bytes_of(&mut context, entry.plane.epoch_account).await)
                .unwrap()
                .phase,
            EPOCH_PHASE_CLEARED
        );
    }

    for entry in &live {
        let (result, units) = send_walk(
            &mut context,
            entry
                .plane
                .freeze_entitlement(keeper_key, entry.submission().id),
            &[&keeper],
            nonce,
        )
        .await;
        result.unwrap();
        nonce += 1;
        meter.record(
            &format!("{}.freeze_entitlement_ledgered", entry.name),
            units,
        );
        let pot =
            FinalPotAccount::decode(&bytes_of(&mut context, entry.plane.pot()).await).unwrap();
        assert_eq!(pot.phase, POT_PHASE_CLOSED);
    }
    // Three distinct pots, one per epoch.
    assert_ne!(live[0].plane.pot(), live[1].plane.pot());
    assert_ne!(live[0].plane.pot(), live[2].plane.pot());

    /* ------------------------------------------------------------------ */
    /* Entitlement and consumption, alternating — with the cross-epoch      */
    /* reservation and receipt refusals in the middle of the ladder.        */
    /* ------------------------------------------------------------------ */

    // HOSTILE: A8's entitlement presenting A7's reservation for the same
    // owner and order.  The two epochs give the same order a different
    // canonical reservation, and the seam re-derives it from the frozen
    // record.
    let (buy0, sell0) = slice_ends(&live[1].submission().witness, 0);
    let alien_reservation = live[0].frozen().reservations[buy0];
    assert_ne!(alien_reservation, live[1].frozen().reservations[buy0]);
    let (result, _) = send(
        &mut context,
        &[live[1].plane.entitle_single(
            keeper_key,
            live[1].submission().id,
            0,
            alien_reservation,
            live[1].frozen().reservations[sell0],
        )],
        &[&keeper],
        nonce,
    )
    .await;
    nonce += 1;
    let code = custom(result);
    eprintln!("scale.{LABEL} CROSS_EPOCH entitle_a8_with_a7_reservation refusal=0x{code:x}");
    assert_eq!(
        code,
        clutch_sbf::error::ClutchError::MismatchedState as u32,
        "another epoch's reservation for the same owner and order is not this one"
    );
    assert!(
        scale_common::account(
            &mut context,
            live[1].plane.receipt(live[1].submission().id, 0)
        )
        .await
        .is_none(),
        "the refused entitlement minted no receipt"
    );

    let slices = live[0].submission().witness.len;
    for slice_index in 0..slices {
        for entry in &live {
            let (buy, sell) = slice_ends(&entry.submission().witness, slice_index);
            let (result, units) = send(
                &mut context,
                &[entry.plane.entitle_single(
                    keeper_key,
                    entry.submission().id,
                    slice_index,
                    entry.frozen().reservations[buy],
                    entry.frozen().reservations[sell],
                )],
                &[&keeper],
                nonce,
            )
            .await;
            result.unwrap();
            nonce += 1;
            meter.record(
                &format!(
                    "{}.entitle_slice_single_ledgered_slice{slice_index}",
                    entry.name
                ),
                units,
            );
        }
        for entry in &live {
            let (buy, sell) = slice_ends(&entry.submission().witness, slice_index);
            let (result, units) = send(
                &mut context,
                &[entry.plane.settle_single(
                    entry.submission().id,
                    entry.plane.positions[entry.orders[buy].0],
                    entry.plane.positions[entry.orders[sell].0],
                    entry.frozen().reservations[buy],
                    entry.frozen().reservations[sell],
                    slice_index,
                    entry.frozen().live_pages[buy],
                )],
                &[],
                nonce,
            )
            .await;
            result.unwrap();
            nonce += 1;
            meter.record(
                &format!("{}.settle_page_direct_slice{slice_index}", entry.name),
                units,
            );
        }

        // HOSTILE, mid-ladder: B7's consumption presenting A7's receipt for
        // this slice.  The receipt PDA is per epoch, so B7's wire coordinate
        // names an account that binds a different epoch entirely.
        if slice_index == 0 {
            let (buy, sell) = slice_ends(&live[2].submission().witness, slice_index);
            let mut crossed = live[2].plane.settle_single(
                live[2].submission().id,
                live[2].plane.positions[live[2].orders[buy].0],
                live[2].plane.positions[live[2].orders[sell].0],
                live[2].frozen().reservations[buy],
                live[2].frozen().reservations[sell],
                slice_index,
                live[2].frozen().live_pages[buy],
            );
            crossed.accounts[6] = solana_instruction::AccountMeta::new(
                live[0].plane.receipt(live[0].submission().id, slice_index),
                false,
            );
            let (result, _) = send(&mut context, &[crossed], &[], nonce).await;
            nonce += 1;
            let code = custom(result);
            eprintln!("scale.{LABEL} CROSS_EPOCH settle_b7_with_a7_receipt refusal=0x{code:x}");
            assert_eq!(
                code,
                clutch_sbf::error::codec_code(clutch_solana_layout::CodecError::MismatchedBinding),
                "a receipt minted under another epoch binds that epoch"
            );
        }
    }

    /* ------------------------------------------------------------------ */
    /* Each epoch's allocation, and no bleed.                              */
    /* ------------------------------------------------------------------ */

    for entry in &live {
        for (at, address) in entry.frozen().reservations.iter().enumerate() {
            let reservation = read_reservation(&mut context, *address).await;
            assert_eq!(
                reservation.state, RESERVATION_STATE_CONSUMED,
                "{} reservation {at}",
                entry.name
            );
            assert!(reservation.remaining_is_zero());
        }
        let pot =
            FinalPotAccount::decode(&bytes_of(&mut context, entry.plane.pot()).await).unwrap();
        assert_eq!(pot.rounding_pot_price_units, 0);
        assert_eq!(pot.pot_cash_price_units, 0);
        assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
    }

    /* Market A carried two epochs, so each of its Positions shows exactly
     * twice one epoch's effect; market B's shows exactly once.  A single atom
     * or Egg crossing between the two epochs — or between the two markets —
     * lands here. */
    let a_positions = &live[0].plane.positions;
    let b_positions = &live[2].plane.positions;
    for buyer in [0usize, 1] {
        let a = read_position(&mut context, a_positions[buyer]).await;
        assert_eq!(a.cash_atoms, START_CASH - 2, "market A buyer {buyer}");
        assert_eq!(a.reserved_cash_atoms, 0);
        assert_eq!(a.internal[0], START_EGGS + 2 * QUANTITY);
        let b = read_position(&mut context, b_positions[buyer]).await;
        assert_eq!(b.cash_atoms, START_CASH - 1, "market B buyer {buyer}");
        assert_eq!(b.reserved_cash_atoms, 0);
        assert_eq!(b.internal[0], START_EGGS + QUANTITY);
    }
    for seller in [2usize, 3] {
        let a = read_position(&mut context, a_positions[seller]).await;
        assert_eq!(a.cash_atoms, START_CASH + 2, "market A seller {seller}");
        assert_eq!(a.internal[0], START_EGGS - 2 * QUANTITY);
        let b = read_position(&mut context, b_positions[seller]).await;
        assert_eq!(b.cash_atoms, START_CASH + 1, "market B seller {seller}");
        assert_eq!(b.internal[0], START_EGGS - QUANTITY);
    }
    // Untraded outcomes never moved, in either market.
    for positions in [a_positions, b_positions] {
        for position in positions.iter() {
            let account = read_position(&mut context, *position).await;
            for outcome in 1..usize::from(OUTCOMES) {
                assert_eq!(account.internal[outcome], START_EGGS);
            }
        }
    }
    // Whole-bank cash conservation across both markets: every epoch's debits
    // equal its credits, so the total is exactly the genesis total.
    let mut total = 0u64;
    for positions in [a_positions, b_positions] {
        for position in positions.iter() {
            total += read_position(&mut context, *position).await.cash_atoms;
        }
    }
    assert_eq!(total, 2 * OWNERS as u64 * START_CASH);

    meter.finish();
}
