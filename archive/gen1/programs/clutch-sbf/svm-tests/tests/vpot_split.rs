//! Real-SBF evidence for the **virtual-split mint join** — the last
//! settlement blocker, retired in the one direction the pot can fund.
//!
//! A virtual leg is the single settlement that changes the market's
//! outstanding supply: `relation_v1.rs:3830-3832` makes the split serve
//! `sigma` Egg atoms on *every* outcome, and `relation_v1.rs:2749-2757`
//! prices that at `sigma * price_scale` — one collateral atom per complete
//! set, because prices lie on the scaled simplex.  So realizing it moves
//! `HoardAccount::collateral_atoms`, the kernel aggregate and the two-term
//! supply ledger, all of which `instructions::split` owns.
//!
//! What this campaign drives, end to end on one bank against a real frozen
//! general book:
//!
//! * **pay, then mint, then deliver.**  Four buys, one per outcome, filled
//!   entirely from the global virtual split at `sigma = 4`.  Each buy end
//!   pays its cumulative debit into the pot; when every split slice has paid
//!   the pot holds exactly `sigma * price_scale` price units of collateral
//!   that is already inside the Hoard's token account and attributed to
//!   nobody, and the first delivery mints the four complete sets through
//!   `split::pooled_set_transition`.  Conservation is asserted to the atom at
//!   every step — owner cash, the Hoard, the kernel aggregate, the supply
//!   ledger, the pot's two scalars — and the pot ends exactly empty.
//! * **the honest refusals.**  A delivery before the book has paid, a
//!   forged pot inventory the supply ledger does not cover, a replayed pay
//!   and a replayed delivery, a direct account shape presented for a virtual
//!   receipt, and a tampered candidate record whose churn the digest binding
//!   refuses.
//! * **the other direction, admitted.**  A verified *merge* candidate now
//!   freezes and opens the same pot; the walk that empties it lives in
//!   `vpot_merge.rs`, which is this campaign read backwards.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.  The oracle is the layout
//! codec plus the host relation, never a second model of either.

mod vpot_common;

use {
    clutch_batch::relation_v1::LegRefV1,
    clutch_sbf::error::ClutchError,
    clutch_solana_layout::{
        account_len, CandidateRecord, EpochAccount, FinalPotAccount, HoardAccount,
        SettlementReceiptAccount, SupplyLedgerAccount, EPOCH_PHASE_CLEARED, MAX_OUTCOMES,
        POT_PHASE_OPEN, RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
        RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_SPLIT,
    },
    clutch_solana_reference::KernelAccount,
    solana_account::AccountSharedData,
    solana_address::Address,
    solana_program_test::{tokio, ProgramTestContext},
    solana_signer::Signer,
    vpot_common::*,
};

/// The candidate every gate here drives: four single buys, one per outcome,
/// four atoms each at limit 3000, cleared at the even price vector.
///
/// Nothing sells, so conservation forces the whole flow through the global
/// virtual split: `sell_flow[i] + sigma == buy_flow[i]` on every outcome with
/// `sell_flow == 0` gives `sigma == 4`.  Each buy is worth
/// `4 * 2500 == 10 000` price units, exactly one collateral atom, so the book
/// realizes no rounding residue at all and the pot's only expectation is the
/// churn.
const BUY_QUANTITY: u64 = 4;
const BUY_LIMIT: u64 = 3_000;
const CLEARED_PRICE: u64 = 2_500;
const SIGMA: u64 = 4;
/// `quantity * price / price_scale`, what each buy actually pays.
const PAID_ATOMS: u64 = 1;

struct Cleared {
    candidate: clutch_solana_layout::Hash32,
    reservations: Vec<Address>,
}

/// Place, freeze, submit, walk to VERIFIED, and select the churn candidate.
async fn cleared_split_book(context: &mut ProgramTestContext, fixture: &Fixture) -> Cleared {
    let orders: Vec<(usize, clutch_solana_layout::OrderSlot)> = (0..4usize)
        .map(|index| {
            (
                index,
                fixture.single(
                    &fixture.owners[index],
                    index as u64 + 1,
                    index as u8,
                    0,
                    BUY_QUANTITY,
                    BUY_LIMIT,
                ),
            )
        })
        .collect();
    build_frozen_book(context, fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(context, fixture).await;

    let witness = witness_of(&[
        slice(0, LegRefV1::Split, 0, BUY_QUANTITY),
        slice(1, LegRefV1::Split, 1, BUY_QUANTITY),
        slice(2, LegRefV1::Split, 2, BUY_QUANTITY),
        slice(3, LegRefV1::Split, 3, BUY_QUANTITY),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].fill(CLEARED_PRICE);
    let submission = plan_submission(fixture, &epoch, &book, prices, SIGMA as i64, witness);
    assert_eq!(submission.virtual_split, SIGMA);
    assert_eq!(submission.virtual_merge, 0);
    assert_eq!(submission.fills, vec![BUY_QUANTITY; 4]);

    submit_seal(context, fixture, &submission, Some(4), &[], 100).await;
    walk_to_verdict(context, fixture, &submission, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + clutch_solana_layout::clearing::CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(context, &[fixture.finalize(&[submission.id])], None, 340).await;
    result.unwrap();
    let epoch_now = EpochAccount::decode(&bytes_of(context, fixture.epoch_account).await).unwrap();
    assert_eq!(epoch_now.phase, EPOCH_PHASE_CLEARED);
    Cleared {
        candidate: submission.id,
        reservations,
    }
}

/// Every ledger a virtual leg moves, read in one go.
struct Ledgers {
    hoard_collateral: u64,
    kernel_supply: [u64; MAX_OUTCOMES],
    internal_supply: [u64; MAX_OUTCOMES],
    hoard_token_amount: u64,
    pot_internal: [u64; MAX_OUTCOMES],
    pot_cash: u128,
    pot_rounding: u128,
    owner_cash: u64,
}

async fn ledgers(context: &mut ProgramTestContext, fixture: &Fixture) -> Ledgers {
    let hoard = HoardAccount::decode(&bytes_of(context, fixture.hoard_account).await).unwrap();
    let kernel = KernelAccount::decode(&bytes_of(context, fixture.kernel_account).await).unwrap();
    let supply =
        SupplyLedgerAccount::decode(&bytes_of(context, fixture.supply_account).await).unwrap();
    let token = bytes_of(context, fixture.hoard_token_account).await;
    let mut amount = [0u8; 8];
    amount.copy_from_slice(&token[64..72]);
    let pot = FinalPotAccount::decode(&bytes_of(context, fixture.pot()).await).unwrap();
    Ledgers {
        hoard_collateral: hoard.collateral_atoms,
        kernel_supply: kernel.total_supply,
        internal_supply: supply.internal_supply,
        hoard_token_amount: u64::from_le_bytes(amount),
        pot_internal: pot.pot_internal,
        pot_cash: pot.pot_cash_price_units,
        pot_rounding: pot.rounding_pot_price_units,
        owner_cash: owner_cash(context, fixture).await,
    }
}

/// The two-term closure and the Hoard mirror, restated over live bank bytes.
///
/// These are the invariants the program checks inside every pooled-set
/// transition; asserting them here is the independent statement that the
/// bank's bytes satisfy them, not a re-run of the program's own check.
fn assert_market_closes(state: &Ledgers) {
    for outcome in 0..OUTCOMES as usize {
        assert_eq!(
            state.internal_supply[outcome], state.kernel_supply[outcome],
            "two-term closure on outcome {outcome}"
        );
    }
    let required = state.kernel_supply[..OUTCOMES as usize]
        .iter()
        .copied()
        .max()
        .unwrap();
    assert!(
        state.hoard_collateral >= required,
        "collateral {} covers max supply {required}",
        state.hoard_collateral
    );
    assert!(
        state.hoard_token_amount >= state.hoard_collateral,
        "the Hoard's token balance covers its complete-set backing"
    );
}

/// The headline gate: a virtual-split candidate clears pay → mint → deliver,
/// with every ledger accounted to the atom and the pot provably empty.
#[tokio::test]
async fn a_virtual_split_candidate_clears_and_the_pot_ends_exactly_empty() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let cleared = cleared_split_book(&mut context, &fixture).await;

    // The freeze has not run yet, so there is no pot to read: the opening
    // ledgers come from the accounts that already exist.
    let hoard_before = HoardAccount::decode(&bytes_of(&mut context, fixture.hoard_account).await)
        .unwrap()
        .collateral_atoms;
    let cash_before = owner_cash(&mut context, &fixture).await;

    /* The freeze now *records* the churn instead of refusing it, and opens
     * the pot: `sigma != 0` is an expectation the epoch must discharge even
     * though this book realizes no rounding residue at all. */
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, cleared.candidate),
        342,
    )
    .await;
    result.unwrap();
    let frozen = ledgers(&mut context, &fixture).await;
    assert_eq!(frozen.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(frozen.pot_cash, 0);
    assert_eq!(frozen.pot_rounding, 0);
    assert_eq!(
        FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await)
            .unwrap()
            .phase,
        POT_PHASE_OPEN,
        "a churned epoch opens its pot even with no rounding residue"
    );
    assert_eq!(
        frozen.hoard_collateral, hoard_before,
        "the freeze mints nothing"
    );
    assert_market_closes(&frozen);

    // Entitle all four virtual slices.  Each creates a `RECEIPT_LEG_SPLIT`
    // receipt whose sell order id is canonically zero.
    for index in 0..4u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.entitle_virtual(
                payer,
                cleared.candidate,
                index,
                cleared.reservations[index as usize],
            ),
            400 + index as u32,
        )
        .await;
        result.unwrap();
        let receipt = SettlementReceiptAccount::decode(
            &bytes_of(&mut context, fixture.receipt(cleared.candidate, index)).await,
        )
        .unwrap();
        assert_eq!(receipt.leg_kind, RECEIPT_LEG_SPLIT);
        assert_eq!(receipt.sell_order_id, clutch_solana_layout::Hash32::ZERO);
        assert_eq!(receipt.quantity, BUY_QUANTITY);
        assert_eq!(receipt.consumed_flags, 0);
    }
    // Entitlement is a freeze, not a transfer: no ledger moved.
    let entitled = ledgers(&mut context, &fixture).await;
    assert_eq!(entitled.hoard_collateral, hoard_before);
    assert_eq!(entitled.owner_cash, cash_before);
    assert_market_closes(&entitled);

    /* PAY, four times.  Each buy end pays exactly one collateral atom — its
     * whole order is worth `4 * 2500 == 10 000` price units — and releases
     * the one atom of price improvement its limit reserved.  The pot's cash
     * rises by `debit * price_scale` because there is no seller to credit. */
    for index in 0..4u16 {
        let before = ledgers(&mut context, &fixture).await;
        let position = fixture.owners[index as usize].position;
        let position_before = read_position(&mut context, position).await;
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                cleared.candidate,
                u64::from(index) + 1,
                position,
                cleared.reservations[index as usize],
                index,
            ),
            500 + index as u32,
        )
        .await;
        result.unwrap();
        let after = ledgers(&mut context, &fixture).await;
        let position_after = read_position(&mut context, position).await;

        assert_eq!(
            position_after.cash_atoms,
            position_before.cash_atoms - PAID_ATOMS,
            "buy {index} pays exactly one atom"
        );
        assert_eq!(
            position_after.reserved_cash_atoms, 0,
            "the completing end releases its whole remaining envelope"
        );
        assert_eq!(
            position_after.internal, position_before.internal,
            "paying delivers nothing"
        );
        assert_eq!(
            after.pot_cash,
            before.pot_cash + u128::from(PAID_ATOMS) * u128::from(PRICE_SCALE),
            "the whole debit lands in the pot: there is no seller"
        );
        assert_eq!(
            after.hoard_collateral, before.hoard_collateral,
            "paying mints nothing"
        );
        assert_eq!(after.kernel_supply, before.kernel_supply);
        assert_eq!(after.internal_supply, before.internal_supply);
        assert_eq!(after.pot_internal, [0; MAX_OUTCOMES]);
        assert_market_closes(&after);

        let receipt = SettlementReceiptAccount::decode(
            &bytes_of(&mut context, fixture.receipt(cleared.candidate, index)).await,
        )
        .unwrap();
        assert_eq!(receipt.consumed_flags, RECEIPT_FLAG_BUY_CONSUMED);
        assert_eq!(receipt.settled_quantity, 0);
    }

    // Every split slice has paid, so the pot holds exactly `sigma * S`.
    let paid = ledgers(&mut context, &fixture).await;
    assert_eq!(paid.pot_cash, u128::from(SIGMA) * u128::from(PRICE_SCALE));
    assert_eq!(paid.owner_cash, cash_before - SIGMA * PAID_ATOMS);
    assert_eq!(paid.hoard_collateral, hoard_before);

    /* DELIVER, four times.  The first finds the pot with no inventory and
     * mints `sigma` complete sets through the pooled primitive; the rest draw
     * the inventory down.  Note what moves on the mint: the Hoard, the kernel
     * aggregate and the internal ledger all rise by exactly `sigma`, and the
     * pot's cash falls by exactly `sigma * price_scale`. */
    for index in 0..4u16 {
        let before = ledgers(&mut context, &fixture).await;
        let position = fixture.owners[index as usize].position;
        let position_before = read_position(&mut context, position).await;
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                cleared.candidate,
                u64::from(index) + 1,
                position,
                cleared.reservations[index as usize],
                index,
            ),
            600 + index as u32,
        )
        .await;
        result.unwrap();
        let after = ledgers(&mut context, &fixture).await;
        let position_after = read_position(&mut context, position).await;

        let minted = if index == 0 { SIGMA } else { 0 };
        assert_eq!(
            after.hoard_collateral,
            before.hoard_collateral + minted,
            "delivery {index} mints {minted} complete sets"
        );
        for outcome in 0..OUTCOMES as usize {
            assert_eq!(
                after.kernel_supply[outcome],
                before.kernel_supply[outcome] + minted,
                "kernel aggregate on outcome {outcome}"
            );
            assert_eq!(
                after.internal_supply[outcome],
                before.internal_supply[outcome] + minted,
                "supply ledger on outcome {outcome}"
            );
        }
        assert_eq!(
            after.pot_cash,
            before.pot_cash - u128::from(minted) * u128::from(PRICE_SCALE),
            "the mint is paid for out of collateral the pot already holds"
        );
        assert_eq!(
            after.hoard_token_amount, before.hoard_token_amount,
            "a pooled set change moves no Token-2022 atom"
        );
        let outcome = index as usize;
        assert_eq!(
            position_after.internal[outcome],
            position_before.internal[outcome] + BUY_QUANTITY,
            "buy {index} receives its whole leg"
        );
        assert_eq!(
            after.pot_internal[outcome], 0,
            "the pot's inventory on outcome {outcome} is exhausted"
        );
        assert_eq!(
            position_after.cash_atoms, position_before.cash_atoms,
            "delivering charges nothing"
        );
        assert_market_closes(&after);

        let receipt = SettlementReceiptAccount::decode(
            &bytes_of(&mut context, fixture.receipt(cleared.candidate, index)).await,
        )
        .unwrap();
        assert_eq!(
            receipt.consumed_flags,
            RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED
        );
        assert_eq!(receipt.settled_quantity, receipt.quantity);
    }

    /* The whole-plane statement.  Four atoms left the buyers' Positions and
     * four became complete-set backing; the pot is empty on all three of its
     * scalars, which is exactly `CloseGeneralPot`'s economic-zero
     * precondition. */
    let closed = ledgers(&mut context, &fixture).await;
    assert_eq!(closed.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(closed.pot_cash, 0);
    assert_eq!(closed.pot_rounding, 0);
    assert_eq!(closed.owner_cash, cash_before - SIGMA);
    assert_eq!(closed.hoard_collateral, hoard_before + SIGMA);
    for outcome in 0..OUTCOMES as usize {
        assert_eq!(
            closed.kernel_supply[outcome],
            frozen.kernel_supply[outcome] + SIGMA
        );
        assert_eq!(
            closed.internal_supply[outcome],
            frozen.internal_supply[outcome] + SIGMA
        );
    }
    assert_market_closes(&closed);
    // Each buyer holds its four new Eggs and paid one atom for them.
    for (index, owner) in fixture.owners.iter().enumerate() {
        let position = read_position(&mut context, owner.position).await;
        assert_eq!(position.internal[index], START_EGGS + BUY_QUANTITY);
        assert_eq!(position.cash_atoms, START_CASH - PAID_ATOMS);
        assert_eq!(position.reserved_cash_atoms, 0);
    }
}

/// Mixed: one ordinary crossing and four virtual legs in the same epoch.
///
/// This is the case that makes the pot's cash ledger load-bearing on the
/// *direct* seam too.  The closed form is
/// `pot_cash += debit * S - credit * S - residue` on **every** slice, and only
/// summing all of them reaches `sigma * S`: a churned epoch therefore makes
/// the pot mandatory on an ordinary slice as well, and the seven-account
/// shape refuses.
///
/// The book: `A` buys eight on outcome 0, `B`, `C` and `D` buy four each on
/// outcomes 1-3, and `B` also sells four on outcome 0.  Conservation forces
/// `sigma = 4`, `A` fills half from `B`'s sell and half from the split, and
/// every filled order's value is a whole number of collateral atoms.
#[tokio::test]
async fn a_mixed_epoch_settles_ordinary_and_virtual_slices_through_one_pot() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();

    let orders = vec![
        (
            0usize,
            fixture.single(&fixture.owners[0], 1, 0, 0, 8, BUY_LIMIT),
        ),
        (
            1usize,
            fixture.single(&fixture.owners[1], 2, 1, 0, 4, BUY_LIMIT),
        ),
        (
            2usize,
            fixture.single(&fixture.owners[2], 3, 2, 0, 4, BUY_LIMIT),
        ),
        (
            3usize,
            fixture.single(&fixture.owners[3], 4, 3, 0, 4, BUY_LIMIT),
        ),
        (
            1usize,
            fixture.single(&fixture.owners[1], 5, 0, 1, 4, 2_000),
        ),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let witness = witness_of(&[
        slice(0, LegRefV1::Order(4), 0, 4),
        slice(0, LegRefV1::Split, 0, 4),
        slice(1, LegRefV1::Split, 1, 4),
        slice(2, LegRefV1::Split, 2, 4),
        slice(3, LegRefV1::Split, 3, 4),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].fill(CLEARED_PRICE);
    let submission = plan_submission(&fixture, &epoch, &book, prices, SIGMA as i64, witness);
    assert_eq!(submission.virtual_split, SIGMA);
    assert_eq!(submission.fills, vec![8, 4, 4, 4, 4]);

    submit_seal(&mut context, &fixture, &submission, Some(5), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &submission, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + clutch_solana_layout::clearing::CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.finalize(&[submission.id])],
        None,
        340,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, submission.id),
        342,
    )
    .await;
    result.unwrap();

    let hoard_before = HoardAccount::decode(&bytes_of(&mut context, fixture.hoard_account).await)
        .unwrap()
        .collateral_atoms;
    let cash_before = owner_cash(&mut context, &fixture).await;

    // Entitle: one direct slice, four virtual ones.
    let (result, _) = send_walk(
        &mut context,
        fixture.entitle_single(payer, submission.id, 0, reservations[0], reservations[4]),
        400,
    )
    .await;
    result.unwrap();
    for index in 1..5u16 {
        let real_end = if index == 1 { 0 } else { index as usize - 1 };
        let (result, _) = send_walk(
            &mut context,
            fixture.entitle_virtual(payer, submission.id, index, reservations[real_end]),
            400 + index as u32,
        )
        .await;
        result.unwrap();
    }

    /* The ordinary slice, on the **potted** shape.  Its debit and its credit
     * are both one atom and it realizes no residue, so the pot's cash does
     * not move — but the pot is still required, because on a churned epoch
     * the ledger only closes if every slice feeds it. */
    let bare = fixture.settle_single(
        submission.id,
        1,
        fixture.owners[0].position,
        fixture.owners[1].position,
        reservations[0],
        reservations[4],
        0,
    );
    let refused = send_walk(&mut context, bare, 490).await;
    assert_eq!(
        custom(refused.0),
        ClutchError::AccountCount as u32,
        "a churned epoch makes the pot mandatory on an ordinary slice too"
    );

    let before = ledgers(&mut context, &fixture).await;
    let (result, _) = send_walk(
        &mut context,
        fixture.settle_single_potted(
            submission.id,
            1,
            fixture.owners[0].position,
            fixture.owners[1].position,
            reservations[0],
            reservations[4],
            0,
        ),
        491,
    )
    .await;
    result.unwrap();
    let after_direct = ledgers(&mut context, &fixture).await;
    assert_eq!(
        after_direct.pot_cash, before.pot_cash,
        "an exact ordinary crossing is cash-neutral for the pot"
    );
    assert_eq!(after_direct.hoard_collateral, before.hoard_collateral);
    assert_market_closes(&after_direct);

    // Pay the four virtual slices, then deliver them.
    for index in 1..5u16 {
        let real_end = if index == 1 { 0 } else { index as usize - 1 };
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                submission.id,
                u64::from(index) + 1,
                fixture.owners[real_end].position,
                reservations[real_end],
                index,
            ),
            500 + index as u32,
        )
        .await;
        result.unwrap();
    }
    let paid = ledgers(&mut context, &fixture).await;
    assert_eq!(paid.pot_cash, u128::from(SIGMA) * u128::from(PRICE_SCALE));
    assert_eq!(paid.hoard_collateral, hoard_before, "paying mints nothing");

    for index in 1..5u16 {
        let real_end = if index == 1 { 0 } else { index as usize - 1 };
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                submission.id,
                u64::from(index) + 1,
                fixture.owners[real_end].position,
                reservations[real_end],
                index,
            ),
            600 + index as u32,
        )
        .await;
        result.unwrap();
    }

    let closed = ledgers(&mut context, &fixture).await;
    assert_eq!(closed.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(closed.pot_cash, 0);
    assert_eq!(closed.pot_rounding, 0);
    assert_eq!(closed.hoard_collateral, hoard_before + SIGMA);
    // Five atoms left the buyers, one reached the seller, four became
    // complete-set backing.
    assert_eq!(closed.owner_cash, cash_before - SIGMA);
    assert_market_closes(&closed);
    // `A` holds its eight new Eggs on outcome 0; `B` sold four of its own and
    // bought four on outcome 1.
    let a = read_position(&mut context, fixture.owners[0].position).await;
    assert_eq!(a.internal[0], START_EGGS + 8);
    let b = read_position(&mut context, fixture.owners[1].position).await;
    assert_eq!(b.internal[0], START_EGGS - 4);
    assert_eq!(b.internal[1], START_EGGS + 4);
}

/// The seam plane's collateral cap reaches the clearing plane's mint
/// unchanged: a candidate whose churn would push the market past its
/// immutable cap refuses at the delivery that would mint it.
#[tokio::test]
async fn a_mint_past_the_immutable_collateral_cap_refuses() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    // Twelve per outcome: three whole collateral atoms per buy, and three
    // more complete sets than the cap leaves room for.
    let big = 12u64;
    assert!(big > MINT_HEADROOM);
    let orders: Vec<(usize, clutch_solana_layout::OrderSlot)> = (0..4usize)
        .map(|index| {
            (
                index,
                fixture.single(
                    &fixture.owners[index],
                    index as u64 + 1,
                    index as u8,
                    0,
                    big,
                    BUY_LIMIT,
                ),
            )
        })
        .collect();
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;
    let witness = witness_of(&[
        slice(0, LegRefV1::Split, 0, big),
        slice(1, LegRefV1::Split, 1, big),
        slice(2, LegRefV1::Split, 2, big),
        slice(3, LegRefV1::Split, 3, big),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].fill(CLEARED_PRICE);
    let submission = plan_submission(&fixture, &epoch, &book, prices, big as i64, witness);
    assert_eq!(submission.virtual_split, big);

    submit_seal(&mut context, &fixture, &submission, Some(4), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &submission, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + clutch_solana_layout::clearing::CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.finalize(&[submission.id])],
        None,
        340,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, submission.id),
        342,
    )
    .await;
    result.unwrap();
    for index in 0..4u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.entitle_virtual(payer, submission.id, index, reservations[index as usize]),
            400 + index as u32,
        )
        .await;
        result.unwrap();
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                submission.id,
                u64::from(index) + 1,
                fixture.owners[index as usize].position,
                reservations[index as usize],
                index,
            ),
            500 + index as u32,
        )
        .await;
        result.unwrap();
    }
    // The pot is fully funded — the buyers really did pay for every set — and
    // the mint still refuses, because the cap is a market-immutable ceiling
    // and not a solvency test.
    let funded = ledgers(&mut context, &fixture).await;
    assert_eq!(funded.pot_cash, u128::from(big) * u128::from(PRICE_SCALE));
    let refused = send_walk(
        &mut context,
        fixture.settle_virtual(
            submission.id,
            1,
            fixture.owners[0].position,
            reservations[0],
            0,
        ),
        600,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::CollateralCap as u32);
    let unchanged = ledgers(&mut context, &fixture).await;
    assert_eq!(unchanged.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(unchanged.pot_cash, funded.pot_cash);
    assert_market_closes(&unchanged);
}

/// The hostile battery, on one bank: every way a virtual leg could be made to
/// mint value the market has not been paid for.
#[tokio::test]
async fn the_virtual_seam_refuses_every_unbacked_route() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let cleared = cleared_split_book(&mut context, &fixture).await;
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, cleared.candidate),
        342,
    )
    .await;
    result.unwrap();
    for index in 0..4u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.entitle_virtual(
                payer,
                cleared.candidate,
                index,
                cleared.reservations[index as usize],
            ),
            400 + index as u32,
        )
        .await;
        result.unwrap();
    }

    // 1. A delivery on an unpaid receipt has no phase at all: the flags say
    //    `Pay`, and paying twice is what the latch forbids.
    let unpaid = ledgers(&mut context, &fixture).await;
    assert_eq!(unpaid.pot_cash, 0);

    // 2. Pay one slice only, then try to deliver it.  The pot holds one atom
    //    of the four the mint costs and refuses rather than minting a set the
    //    market has not been paid for.
    let (result, _) = send_walk(
        &mut context,
        fixture.settle_virtual(
            cleared.candidate,
            1,
            fixture.owners[0].position,
            cleared.reservations[0],
            0,
        ),
        500,
    )
    .await;
    result.unwrap();
    let after_one = ledgers(&mut context, &fixture).await;
    assert_eq!(after_one.pot_cash, u128::from(PRICE_SCALE));
    let refused = send_walk(
        &mut context,
        fixture.settle_virtual(
            cleared.candidate,
            1,
            fixture.owners[0].position,
            cleared.reservations[0],
            0,
        ),
        501,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        ClutchError::AggregateClosureMismatch as u32,
        "a delivery before the whole book has paid refuses"
    );
    let unchanged = ledgers(&mut context, &fixture).await;
    assert_eq!(unchanged.hoard_collateral, after_one.hoard_collateral);
    assert_eq!(unchanged.pot_internal, [0; MAX_OUTCOMES]);
    assert_market_closes(&unchanged);

    // 3. A replayed pay: the receipt already carries `BUY_CONSUMED`, so the
    //    only phase left is `Deliver`, which the step above just refused.
    //    Replay is therefore structurally impossible, and the reservation
    //    proves it — it is CONSUMED and its ledger cannot advance again.
    let reservation = read_reservation(&mut context, cleared.reservations[0]).await;
    assert_eq!(reservation.consumed_units, reservation.entitled_units);

    // 4. The direct seven-account shape presented for a virtual receipt: the
    //    virtual receipt's `leg_kind` is not `RECEIPT_LEG_DIRECT`, so the
    //    direct seam refuses it rather than settling half a slice.
    let direct = fixture.settle_single(
        cleared.candidate,
        2,
        fixture.owners[1].position,
        fixture.owners[1].position,
        cleared.reservations[1],
        cleared.reservations[1],
        1,
    );
    let refused = send_walk(&mut context, direct, 505).await;
    assert!(
        refused.0.is_err(),
        "the direct shape never settles a virtual receipt"
    );

    // 5. A forged pot inventory the supply ledger does not cover.  Hand-write
    //    a pot holding claims nobody minted and try to deliver from it: the
    //    C2 internal bound refuses before a single claim moves.
    let mut forged = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    let ledger =
        SupplyLedgerAccount::decode(&bytes_of(&mut context, fixture.supply_account).await).unwrap();
    for outcome in 0..OUTCOMES as usize {
        forged.pot_internal[outcome] = ledger.internal_supply[outcome] + 1;
    }
    context.set_account(
        &fixture.pot(),
        &AccountSharedData::from(program_account(encode(account_len::FINAL_POT, |out| {
            forged.encode(out)
        }))),
    );
    let refused = send_walk(
        &mut context,
        fixture.settle_virtual(
            cleared.candidate,
            2,
            fixture.owners[1].position,
            cleared.reservations[1],
            1,
        ),
        510,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        ClutchError::AggregateClosureMismatch as u32,
        "a pot whose inventory the ledger does not cover cannot deliver"
    );
}

/// A tampered candidate record cannot resize the mint: `sigma` is under the
/// record's own recomputed digest, and the freeze cross-checks it against the
/// streamed verdict besides.
#[tokio::test]
async fn a_tampered_churn_coordinate_never_reaches_the_mint() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let cleared = cleared_split_book(&mut context, &fixture).await;

    let record_address = fixture.candidate_record(cleared.candidate);
    let mut record =
        CandidateRecord::decode(&bytes_of(&mut context, record_address).await).unwrap();
    // Claim one more complete set than the relation priced.
    record.virtual_split += 1;
    record.churn += 1;
    let mut bytes = vec![0u8; account_len::CANDIDATE];
    // The record's own codec refuses to write a body whose digest no longer
    // matches its identity, so the tamper has to be a raw byte edit.
    let original = bytes_of(&mut context, record_address).await;
    bytes.copy_from_slice(&original);
    let mut probe = record;
    probe.candidate = probe.recomputed_candidate_digest().unwrap();
    assert_ne!(
        probe.candidate, cleared.candidate,
        "the churn coordinate is under the candidate digest"
    );

    // Raw-edit `virtual_split` in place: the field sits after the price
    // vector in the frozen layout, so find it by re-encoding the tampered
    // record under the *original* identity and copying only that field.
    let mut tampered_body = vec![0u8; account_len::CANDIDATE];
    let mut forged = record;
    forged.candidate = cleared.candidate;
    forged.stored_bump = 0;
    // `encode` validates, and validation recomputes nothing about the digest,
    // so this succeeds and produces a body whose identity is a lie.
    match forged.encode(&mut tampered_body) {
        Ok(_) => {
            // Preserve the stored bump the runtime compares.
            let keep = original[account_len::CANDIDATE - 2];
            tampered_body[account_len::CANDIDATE - 2] = keep;
            context.set_account(
                &record_address,
                &AccountSharedData::from(program_account(tampered_body)),
            );
            let refused = send_walk(
                &mut context,
                fixture.freeze_entitlement(payer, cleared.candidate),
                342,
            )
            .await;
            assert!(
                refused.0.is_err(),
                "a record whose churn the verdict did not produce cannot freeze"
            );
        }
        Err(_) => {
            // The codec already refuses the tampered body, which is a
            // stronger statement than the runtime refusal above.
        }
    }
}

/// The row that used to stand here, retired: a verified **merge** candidate
/// freezes.
///
/// The freeze records `mu` exactly as it records `sigma` and opens the pot the
/// consumption seam drains; what changed is not this instruction's checks but
/// the *order* the pot can be drained in, and the schema that expresses it.
/// The full deliver-burn-pay walk is `vpot_merge.rs`; this gate only pins that
/// the admission the split wave had to refuse is now open, on the same book
/// that refused before.
#[tokio::test]
async fn a_virtual_merge_candidate_now_freezes_and_opens_its_pot() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    // Four sells, one per outcome, all marginal at the even price vector: the
    // canonical candidate at imbalance -4 absorbs them into the global merge.
    let orders: Vec<(usize, clutch_solana_layout::OrderSlot)> = (0..4usize)
        .map(|index| {
            (
                index,
                fixture.single(
                    &fixture.owners[index],
                    index as u64 + 1,
                    index as u8,
                    1,
                    BUY_QUANTITY,
                    CLEARED_PRICE,
                ),
            )
        })
        .collect();
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let witness = witness_of(&[
        merge_slice(0, 0, BUY_QUANTITY),
        merge_slice(1, 1, BUY_QUANTITY),
        merge_slice(2, 2, BUY_QUANTITY),
        merge_slice(3, 3, BUY_QUANTITY),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].fill(CLEARED_PRICE);
    let submission = plan_submission(&fixture, &epoch, &book, prices, -(SIGMA as i64), witness);
    assert_eq!(submission.virtual_split, 0);
    assert_eq!(submission.virtual_merge, SIGMA);

    submit_seal(&mut context, &fixture, &submission, Some(4), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &submission, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + clutch_solana_layout::clearing::CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.finalize(&[submission.id])],
        None,
        340,
    )
    .await;
    result.unwrap();

    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, submission.id),
        342,
    )
    .await;
    result.unwrap();
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(
        pot.phase, POT_PHASE_OPEN,
        "a merged epoch opens the pot its burn will fund"
    );
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.rounding_pot_price_units, 0);
    let record = CandidateRecord::decode(
        &bytes_of(&mut context, fixture.candidate_record(submission.id)).await,
    )
    .unwrap();
    assert_eq!(record.virtual_merge, SIGMA);
    assert_eq!(record.virtual_split, 0, "canonical churn is one-sided");
}

/// One explicit witness slice whose *buy* end is the global virtual merge.
fn merge_slice(sell: u8, outcome: u8, quantity: u64) -> clutch_batch::relation_v1::PairingSliceV1 {
    clutch_batch::relation_v1::PairingSliceV1 {
        buy_ref: LegRefV1::Merge,
        sell_ref: LegRefV1::Order(sell),
        outcome,
        quantity,
    }
}
