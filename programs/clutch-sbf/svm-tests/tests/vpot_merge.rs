//! Real-SBF evidence for the **virtual-merge burn join** — the settlement
//! ledger's last row, and the mirror of the split.
//!
//! A merge is the one-ended *buy*: `relation_v1.rs:1562` and `:3806` make
//! `LegRefV1::Merge` legal only in a slice's `buy_ref`, so its real ends are
//! **sellers** the pot owes cash.  `relation_v1.rs:3830-3832` makes it absorb
//! `mu` Egg atoms on *every* outcome — `mu` complete sets — and
//! `relation_v1.rs:2749-2757` prices that at `mu * price_scale`, exactly one
//! collateral atom per set, because prices lie on the scaled simplex.
//!
//! Which fixes the order, and it is the split's read backwards.  The pot's
//! slice-side cash identity is `(sum_buys V - sum_sells V) + pending gaps`,
//! and a merge slice adds to `sum_sells V` alone: credit a payee first and the
//! pot is negative by exactly the value of the sets it has not burned, which
//! would release `mu` collateral atoms against claims the market still has
//! outstanding.  So a split *collects, mints, delivers* and a merge
//! **delivers, burns, pays** — and that separates a sell end's Egg movement
//! from its cash, which is what `ReservationAccount` v3's `paid_units` records.
//!
//! What this campaign drives, end to end on one bank against a real frozen
//! general book:
//!
//! * **deliver, then burn, then pay.**  Four sells, one per outcome, absorbed
//!   entirely by the global virtual merge at `mu = 4`.  Each delivers its Eggs
//!   into the pot with its payment ledger left behind; the delivery that
//!   completes `mu` sets on every outcome burns them through
//!   `split::pooled_set_transition`, and the released collateral pays all four
//!   sellers.  Conservation is asserted to the atom at every step — owner
//!   cash, the Hoard, the kernel aggregate, the supply ledger, the book's own
//!   Egg total and the pot's two scalars — and the pot ends exactly empty.
//! * **the rounding case.**  A merge book whose every order is worth half a
//!   collateral atom: nobody is credited a single atom, the whole
//!   `mu * price_scale` is realized as rounding residue, and the pot still
//!   closes to zero on both terms.
//! * **the mixed epoch.**  An ordinary crossing and four merge legs through
//!   one pot, with a sell end whose ledger is shared between them.
//! * **the honest refusals.**  A payment before the burn, a payment before its
//!   own delivery, a forged payment ledger, a delivery past `mu`, a forged pot
//!   inventory the supply ledger does not cover, a replayed delivery and a
//!   replayed payment, and a direct account shape presented for a merge
//!   receipt.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.  The oracle is the layout
//! codec plus the host relation, never a second model of either.

mod vpot_common;

use {
    clutch_batch::relation_v1::{LegRefV1, PairingSliceV1},
    clutch_sbf::error::ClutchError,
    clutch_solana_layout::{
        account_len,
        reservation::{
            RESERVATION_ACCOUNT_BYTES, RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED,
        },
        EpochAccount, FinalPotAccount, HoardAccount, SettlementReceiptAccount, SupplyLedgerAccount,
        EPOCH_PHASE_CLEARED, MAX_OUTCOMES, POT_PHASE_OPEN, RECEIPT_FLAG_BUY_CONSUMED,
        RECEIPT_FLAG_SELL_CONSUMED, RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_MERGE,
    },
    clutch_solana_reference::KernelAccount,
    solana_account::AccountSharedData,
    solana_address::Address,
    solana_program_test::{tokio, ProgramTestContext},
    solana_signer::Signer,
    vpot_common::*,
};

/// The candidate the headline walk drives: four single sells, one per outcome,
/// four atoms each, marginal at the even price vector.
///
/// Nothing buys, so conservation forces the whole flow through the global
/// virtual merge: `sell_flow[i] + sigma == buy_flow[i] + mu` on every outcome
/// with `buy_flow == 0` and `sigma == 0` gives `mu == 4`.  Each sell is worth
/// `4 * 2500 == 10 000` price units, exactly one collateral atom, so the book
/// realizes no rounding residue at all and the pot's only expectation is the
/// churn.
const SELL_QUANTITY: u64 = 4;
const CLEARED_PRICE: u64 = 2_500;
const MU: u64 = 4;
/// `quantity * price / price_scale`, what each sell actually receives.
const CREDITED_ATOMS: u64 = 1;

struct Cleared {
    candidate: clutch_solana_layout::Hash32,
    reservations: Vec<Address>,
}

/// One explicit witness slice whose *buy* end is the global virtual merge.
fn merge_slice(sell: u8, outcome: u8, quantity: u64) -> PairingSliceV1 {
    PairingSliceV1 {
        buy_ref: LegRefV1::Merge,
        sell_ref: LegRefV1::Order(sell),
        outcome,
        quantity,
    }
}

/// Place, freeze, submit, walk to VERIFIED, and select the churn candidate.
async fn cleared_merge_book(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    quantity: u64,
    mu: u64,
) -> Cleared {
    let orders: Vec<(usize, clutch_solana_layout::OrderSlot)> = (0..4usize)
        .map(|index| {
            (
                index,
                fixture.single(
                    &fixture.owners[index],
                    index as u64 + 1,
                    index as u8,
                    1,
                    quantity,
                    CLEARED_PRICE,
                ),
            )
        })
        .collect();
    build_frozen_book(context, fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(context, fixture).await;

    let witness = witness_of(&[
        merge_slice(0, 0, quantity),
        merge_slice(1, 1, quantity),
        merge_slice(2, 2, quantity),
        merge_slice(3, 3, quantity),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].fill(CLEARED_PRICE);
    let submission = plan_submission(fixture, &epoch, &book, prices, -(mu as i64), witness);
    assert_eq!(submission.virtual_merge, mu);
    assert_eq!(submission.virtual_split, 0);
    assert_eq!(submission.fills, vec![quantity; 4]);

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

/// Every Egg the market accounts for on one outcome: the book's own total —
/// Positions plus reservations — plus whatever the pot is holding in trust.
///
/// This is the sum the supply ledger must equal at every transaction boundary,
/// and a merge delivery is precisely the move that shifts value from the first
/// term to the second without changing the whole.
async fn accounted_eggs(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    reservations: &[Address],
    outcome: usize,
) -> u64 {
    let book = book_eggs(context, fixture, reservations, outcome).await;
    let pot = FinalPotAccount::decode(&bytes_of(context, fixture.pot()).await).unwrap();
    book + pot.pot_internal[outcome]
}

/// The headline gate: a virtual-merge candidate clears deliver → burn → pay,
/// with every ledger accounted to the atom and the pot provably empty.
#[tokio::test]
async fn a_virtual_merge_candidate_clears_and_the_pot_ends_exactly_empty() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let cleared = cleared_merge_book(&mut context, &fixture, SELL_QUANTITY, MU).await;

    // The freeze has not run yet, so there is no pot to read: the opening
    // ledgers come from the accounts that already exist.
    let hoard_before = HoardAccount::decode(&bytes_of(&mut context, fixture.hoard_account).await)
        .unwrap()
        .collateral_atoms;
    let cash_before = owner_cash(&mut context, &fixture).await;

    /* The freeze records `mu` exactly as it records `sigma`, and opens the
     * pot: a churned epoch must discharge its churn even though this book
     * realizes no rounding residue at all. */
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
        "a merged epoch opens its pot even with no rounding residue"
    );
    assert_eq!(
        frozen.hoard_collateral, hoard_before,
        "the freeze burns nothing"
    );
    assert_market_closes(&frozen);

    // Entitle all four merge slices.  Each creates a `RECEIPT_LEG_MERGE`
    // receipt whose *buy* order id is canonically zero — the mirror of a
    // split receipt's zero sell id.
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
        assert_eq!(receipt.leg_kind, RECEIPT_LEG_MERGE);
        assert_eq!(receipt.buy_order_id, clutch_solana_layout::Hash32::ZERO);
        assert_eq!(receipt.quantity, SELL_QUANTITY);
        assert_eq!(receipt.consumed_flags, 0);
        let reservation =
            read_reservation(&mut context, cleared.reservations[index as usize]).await;
        assert_eq!(reservation.entitled_units, SELL_QUANTITY);
        assert_eq!(reservation.consumed_units, 0);
        assert_eq!(reservation.paid_units, 0);
    }
    // Entitlement is a freeze, not a transfer: no ledger moved.
    let entitled = ledgers(&mut context, &fixture).await;
    assert_eq!(entitled.hoard_collateral, hoard_before);
    assert_eq!(entitled.owner_cash, cash_before);
    assert_market_closes(&entitled);

    /* DELIVER, four times.  Each sell end's Eggs leave its reservation for the
     * pot's inventory, its quantity ledger advances and its payment ledger
     * does not.  The fourth completes `mu` sets on every outcome and burns
     * them: the Hoard, the kernel aggregate and the internal ledger all fall
     * by exactly `mu`, and the released collateral becomes the pot's cash. */
    for index in 0..4u16 {
        let before = ledgers(&mut context, &fixture).await;
        let outcome = index as usize;
        let accounted_before =
            accounted_eggs(&mut context, &fixture, &cleared.reservations, outcome).await;
        let position = fixture.owners[outcome].position;
        let position_before = read_position(&mut context, position).await;
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                cleared.candidate,
                u64::from(index) + 1,
                position,
                cleared.reservations[outcome],
                index,
            ),
            500 + index as u32,
        )
        .await;
        result.unwrap();
        let after = ledgers(&mut context, &fixture).await;
        let position_after = read_position(&mut context, position).await;

        let burned = if index == 3 { MU } else { 0 };
        assert_eq!(
            after.hoard_collateral,
            before.hoard_collateral - burned,
            "delivery {index} burns {burned} complete sets"
        );
        for check in 0..OUTCOMES as usize {
            assert_eq!(
                after.kernel_supply[check],
                before.kernel_supply[check] - burned,
                "kernel aggregate on outcome {check}"
            );
            assert_eq!(
                after.internal_supply[check],
                before.internal_supply[check] - burned,
                "supply ledger on outcome {check}"
            );
        }
        assert_eq!(
            after.pot_cash,
            before.pot_cash + u128::from(burned) * u128::from(PRICE_SCALE),
            "the burn releases exactly one collateral atom per set"
        );
        assert_eq!(
            after.hoard_token_amount, before.hoard_token_amount,
            "a pooled set change moves no Token-2022 atom"
        );
        assert_eq!(
            after.pot_internal[outcome],
            if burned == 0 { SELL_QUANTITY } else { 0 },
            "the pot holds this leg until the burn takes the whole set"
        );
        assert_eq!(
            position_after.cash_atoms, position_before.cash_atoms,
            "delivering pays nothing"
        );
        assert_market_closes(&after);

        /* The Egg conservation the delivery is: what the book held plus what
         * the pot holds equals the ledger, before and after — falling only
         * when the burn destroys claims. */
        let accounted_after =
            accounted_eggs(&mut context, &fixture, &cleared.reservations, outcome).await;
        assert_eq!(
            accounted_after,
            accounted_before - burned,
            "outcome {outcome}: custody moved, only the burn destroys"
        );
        assert_eq!(accounted_after, after.internal_supply[outcome]);

        // Delivered, not paid: the two ledgers have separated, and the
        // reservation stays ENTITLED because of it.
        let reservation = read_reservation(&mut context, cleared.reservations[outcome]).await;
        assert_eq!(reservation.state, RESERVATION_STATE_ENTITLED);
        assert_eq!(reservation.consumed_units, SELL_QUANTITY);
        assert_eq!(reservation.paid_units, 0);
        assert_eq!(reservation.unpaid_units(), SELL_QUANTITY);
        assert!(reservation.remaining_is_zero());

        let receipt = SettlementReceiptAccount::decode(
            &bytes_of(&mut context, fixture.receipt(cleared.candidate, index)).await,
        )
        .unwrap();
        assert_eq!(receipt.consumed_flags, RECEIPT_FLAG_SELL_CONSUMED);
        assert_eq!(receipt.settled_quantity, 0);
    }

    // Every merge leg has delivered and the sets are burned, so the pot holds
    // exactly `mu * S` of collateral that backs nothing any more.
    let burned = ledgers(&mut context, &fixture).await;
    assert_eq!(burned.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(burned.pot_cash, u128::from(MU) * u128::from(PRICE_SCALE));
    assert_eq!(burned.hoard_collateral, hoard_before - MU);
    assert_eq!(burned.owner_cash, cash_before, "nobody has been paid yet");

    /* PAY, four times.  Each sell end is credited exactly one collateral atom
     * — its whole order is worth `4 * 2500 == 10 000` price units — out of the
     * cash the burn released, and its payment ledger catches up with its
     * delivery, which is what closes the reservation. */
    for index in 0..4u16 {
        let before = ledgers(&mut context, &fixture).await;
        let outcome = index as usize;
        let position = fixture.owners[outcome].position;
        let position_before = read_position(&mut context, position).await;
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                cleared.candidate,
                u64::from(index) + 1,
                position,
                cleared.reservations[outcome],
                index,
            ),
            600 + index as u32,
        )
        .await;
        result.unwrap();
        let after = ledgers(&mut context, &fixture).await;
        let position_after = read_position(&mut context, position).await;

        assert_eq!(
            position_after.cash_atoms,
            position_before.cash_atoms + CREDITED_ATOMS,
            "sell {index} is credited exactly one atom"
        );
        assert_eq!(
            position_after.internal, position_before.internal,
            "paying moves no Egg"
        );
        assert_eq!(
            after.pot_cash,
            before.pot_cash - u128::from(CREDITED_ATOMS) * u128::from(PRICE_SCALE),
            "the whole credit comes out of the pot: there is no buyer"
        );
        assert_eq!(
            after.hoard_collateral, before.hoard_collateral,
            "the backing already fell at the burn; the credit only re-attributes it"
        );
        assert_eq!(after.kernel_supply, before.kernel_supply);
        assert_eq!(after.internal_supply, before.internal_supply);
        assert_eq!(after.pot_internal, [0; MAX_OUTCOMES]);
        assert_market_closes(&after);

        let reservation = read_reservation(&mut context, cleared.reservations[outcome]).await;
        assert_eq!(reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(reservation.paid_units, reservation.consumed_units);
        assert_eq!(reservation.paid_units, reservation.entitled_units);

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

    /* The whole-plane statement.  Four complete sets stopped existing and the
     * four collateral atoms that backed them reached the sellers; the pot is
     * empty on all three of its scalars, which is exactly `CloseGeneralPot`'s
     * economic-zero precondition. */
    let closed = ledgers(&mut context, &fixture).await;
    assert_eq!(closed.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(closed.pot_cash, 0);
    assert_eq!(closed.pot_rounding, 0);
    assert_eq!(closed.owner_cash, cash_before + MU);
    assert_eq!(closed.hoard_collateral, hoard_before - MU);
    for outcome in 0..OUTCOMES as usize {
        assert_eq!(
            closed.kernel_supply[outcome],
            frozen.kernel_supply[outcome] - MU
        );
        assert_eq!(
            closed.internal_supply[outcome],
            frozen.internal_supply[outcome] - MU
        );
    }
    assert_market_closes(&closed);
    // Each seller gave up its four Eggs and was paid one atom for them.
    for (index, owner) in fixture.owners.iter().enumerate() {
        let position = read_position(&mut context, owner.position).await;
        assert_eq!(position.internal[index], START_EGGS - SELL_QUANTITY);
        assert_eq!(position.cash_atoms, START_CASH + CREDITED_ATOMS);
        assert_eq!(position.reserved_cash_atoms, 0);
    }
}

/// The rounding case, in the merge direction: a book whose every order is
/// worth half a collateral atom.
///
/// Under `TerminalOwnerFloor` a payee rounds *down*, so no seller is credited a
/// single atom and the entire `mu * price_scale` the burn released is realized
/// as rounding residue — value that stays unallocated in the market's pool
/// because it is simply never credited.  The pot still closes to zero on both
/// terms, which is the statement that the runtime's per-order conversions
/// summed to the relation's verified `rounding_pot`.
#[tokio::test]
async fn a_merge_epoch_realizes_its_whole_rounding_pot_through_the_same_burn() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    // Two atoms at 2500 is 5000 price units: exactly half a collateral atom.
    let dust = 2u64;
    let cleared = cleared_merge_book(&mut context, &fixture, dust, dust).await;

    let hoard_before = HoardAccount::decode(&bytes_of(&mut context, fixture.hoard_account).await)
        .unwrap()
        .collateral_atoms;
    let cash_before = owner_cash(&mut context, &fixture).await;
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, cleared.candidate),
        342,
    )
    .await;
    result.unwrap();

    /* The relation's verified expectation, recorded rather than assumed: four
     * payees each short 5000 price units of their exact value. */
    let frozen = ledgers(&mut context, &fixture).await;
    assert_eq!(frozen.pot_rounding, 4 * 5_000);
    assert_eq!(
        frozen.pot_rounding,
        u128::from(dust) * u128::from(PRICE_SCALE),
        "the whole merge proceeds are residue on this book"
    );

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
    for index in 0..4u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                cleared.candidate,
                u64::from(index) + 1,
                fixture.owners[index as usize].position,
                cleared.reservations[index as usize],
                index,
            ),
            500 + index as u32,
        )
        .await;
        result.unwrap();
    }
    let burned = ledgers(&mut context, &fixture).await;
    assert_eq!(burned.pot_cash, u128::from(dust) * u128::from(PRICE_SCALE));
    assert_eq!(burned.hoard_collateral, hoard_before - dust);

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
        assert_eq!(
            read_position(&mut context, position).await.cash_atoms,
            position_before.cash_atoms,
            "half an atom of value credits zero atoms"
        );
        assert_eq!(
            after.pot_rounding,
            before.pot_rounding - 5_000,
            "the completing payment draws its own share of the expectation down"
        );
        assert_eq!(
            after.pot_cash,
            before.pot_cash - 5_000,
            "the residue leaves the pot without reaching anybody"
        );
        assert_eq!(
            after.hoard_collateral, before.hoard_collateral,
            "unallocated collateral stays in the pool"
        );
    }

    let closed = ledgers(&mut context, &fixture).await;
    assert_eq!(closed.pot_cash, 0);
    assert_eq!(closed.pot_rounding, 0);
    assert_eq!(closed.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(
        closed.owner_cash, cash_before,
        "nobody was paid, and nothing was created"
    );
    assert_eq!(closed.hoard_collateral, hoard_before - dust);
    assert_market_closes(&closed);
}

/// Which owner each mixed-book merge slice's real end belongs to: `B`, `C`,
/// `D`, then `A` again for the outcome-3 sell it also placed.
const MERGE_OWNER: [usize; 4] = [1, 2, 3, 0];

/// The mixed book both mixed-epoch gates drive, cleared and entitled.
///
/// `A` buys four on outcome 0, `B` sells eight there, `C` and `D` sell four on
/// outcomes 1 and 2, and `A` also sells four on outcome 3.  Conservation gives
/// `mu = 4` on every outcome: `B` fills half from `A`'s buy and half into the
/// merge, so its per-order ledger is shared between an ordinary slice and a
/// virtual one.  Every filled order's value is a whole number of collateral
/// atoms, so the book realizes no residue and the pot's only expectation is
/// churn.
async fn entitled_mixed_book(context: &mut ProgramTestContext, fixture: &Fixture) -> Cleared {
    let payer = context.payer.pubkey();
    let orders = vec![
        (
            0usize,
            fixture.single(&fixture.owners[0], 1, 0, 0, 4, 3_000),
        ),
        (
            1usize,
            fixture.single(&fixture.owners[1], 2, 0, 1, 8, CLEARED_PRICE),
        ),
        (
            2usize,
            fixture.single(&fixture.owners[2], 3, 1, 1, 4, CLEARED_PRICE),
        ),
        (
            3usize,
            fixture.single(&fixture.owners[3], 4, 2, 1, 4, CLEARED_PRICE),
        ),
        (
            0usize,
            fixture.single(&fixture.owners[0], 5, 3, 1, 4, CLEARED_PRICE),
        ),
    ];
    build_frozen_book(context, fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(context, fixture).await;

    let witness = witness_of(&[
        slice(0, LegRefV1::Order(1), 0, 4),
        merge_slice(1, 0, 4),
        merge_slice(2, 1, 4),
        merge_slice(3, 2, 4),
        merge_slice(4, 3, 4),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].fill(CLEARED_PRICE);
    let submission = plan_submission(fixture, &epoch, &book, prices, -(MU as i64), witness);
    assert_eq!(submission.virtual_merge, MU);
    assert_eq!(submission.virtual_split, 0);
    assert_eq!(submission.fills, vec![4, 8, 4, 4, 4]);

    submit_seal(context, fixture, &submission, Some(5), &[], 100).await;
    walk_to_verdict(context, fixture, &submission, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + clutch_solana_layout::clearing::CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(context, &[fixture.finalize(&[submission.id])], None, 340).await;
    result.unwrap();
    let (result, _) = send_walk(
        context,
        fixture.freeze_entitlement(payer, submission.id),
        342,
    )
    .await;
    result.unwrap();

    // Entitle: one direct slice, four merge ones.
    let (result, _) = send_walk(
        context,
        fixture.entitle_single(payer, submission.id, 0, reservations[0], reservations[1]),
        400,
    )
    .await;
    result.unwrap();
    for index in 1..5u16 {
        let (result, _) = send_walk(
            context,
            fixture.entitle_virtual(payer, submission.id, index, reservations[index as usize]),
            400 + index as u32,
        )
        .await;
        result.unwrap();
    }
    Cleared {
        candidate: submission.id,
        reservations,
    }
}

/// Mixed: one ordinary crossing and four merge legs in the same epoch, with a
/// sell end whose ledger is shared between the two.
#[tokio::test]
async fn a_mixed_epoch_settles_ordinary_and_merge_slices_through_one_pot() {
    let (mut context, fixture) = start().await;
    let Cleared {
        candidate: submission_id,
        reservations,
    } = entitled_mixed_book(&mut context, &fixture).await;

    let hoard_before = HoardAccount::decode(&bytes_of(&mut context, fixture.hoard_account).await)
        .unwrap()
        .collateral_atoms;
    let cash_before = owner_cash(&mut context, &fixture).await;

    /* The ordinary slice, on the **potted** shape: a churned epoch makes the
     * pot mandatory on every slice, in the merge direction exactly as in the
     * split's, because the cash ledger only closes if every slice feeds it. */
    let bare = fixture.settle_single(
        submission_id,
        1,
        fixture.owners[0].position,
        fixture.owners[1].position,
        reservations[0],
        reservations[1],
        0,
    );
    let refused = send_walk(&mut context, bare, 490).await;
    assert_eq!(
        custom(refused.0),
        ClutchError::AccountCount as u32,
        "a merged epoch makes the pot mandatory on an ordinary slice too"
    );

    let before = ledgers(&mut context, &fixture).await;
    let (result, _) = send_walk(
        &mut context,
        fixture.settle_single_potted(
            submission_id,
            1,
            fixture.owners[0].position,
            fixture.owners[1].position,
            reservations[0],
            reservations[1],
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
    /* `B`'s ledgers advanced together on the direct slice, and half its order
     * is still outstanding for the merge. */
    let shared = read_reservation(&mut context, reservations[1]).await;
    assert_eq!(shared.entitled_units, 8);
    assert_eq!(shared.consumed_units, 4);
    assert_eq!(shared.paid_units, 4);
    assert_eq!(shared.state, RESERVATION_STATE_ENTITLED);

    // Deliver the four merge legs; the last completes the sets and burns.
    for index in 1..5u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                submission_id,
                u64::from(index) + 1,
                fixture.owners[MERGE_OWNER[index as usize - 1]].position,
                reservations[index as usize],
                index,
            ),
            500 + index as u32,
        )
        .await;
        result.unwrap();
    }
    let burned = ledgers(&mut context, &fixture).await;
    assert_eq!(burned.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(burned.pot_cash, u128::from(MU) * u128::from(PRICE_SCALE));
    assert_eq!(burned.hoard_collateral, hoard_before - MU);
    // `B`'s Egg ledger is now full and its payment ledger is half a step
    // behind — the window the schema exists to describe.
    let shared = read_reservation(&mut context, reservations[1]).await;
    assert_eq!(shared.consumed_units, 8);
    assert_eq!(shared.paid_units, 4);
    assert_eq!(shared.state, RESERVATION_STATE_ENTITLED);

    // Pay them.
    for index in 1..5u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                submission_id,
                u64::from(index) + 1,
                fixture.owners[MERGE_OWNER[index as usize - 1]].position,
                reservations[index as usize],
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
    assert_eq!(closed.hoard_collateral, hoard_before - MU);
    /* One atom left `A` for `B` on the direct slice and four came out of the
     * burn, so the owners hold four more than they started with. */
    assert_eq!(closed.owner_cash, cash_before + MU);
    assert_market_closes(&closed);
    let shared = read_reservation(&mut context, reservations[1]).await;
    assert_eq!(shared.state, RESERVATION_STATE_CONSUMED);
    assert_eq!(shared.paid_units, 8);
    // `A` bought four on outcome 0 and sold four on outcome 3.
    let a = read_position(&mut context, fixture.owners[0].position).await;
    assert_eq!(a.internal[0], START_EGGS + 4);
    assert_eq!(a.internal[3], START_EGGS - 4);
    // `B` sold eight on outcome 0: four to `A`, four into the merge.
    let b = read_position(&mut context, fixture.owners[1].position).await;
    assert_eq!(b.internal[0], START_EGGS - 8);
}

/// The hostile battery, on one bank: every way a merge leg could be made to
/// pay out value the market has not stopped backing.
#[tokio::test]
async fn the_merge_seam_refuses_every_unfunded_route() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let cleared = cleared_merge_book(&mut context, &fixture, SELL_QUANTITY, MU).await;
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

    /* 1. Deliver three of the four legs.  The sets are not complete, so
     *    nothing burns and the pot still holds no cash at all. */
    for index in 0..3u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                cleared.candidate,
                u64::from(index) + 1,
                fixture.owners[index as usize].position,
                cleared.reservations[index as usize],
                index,
            ),
            500 + index as u32,
        )
        .await;
        result.unwrap();
    }
    let partial = ledgers(&mut context, &fixture).await;
    assert_eq!(partial.pot_cash, 0, "an incomplete set releases nothing");
    assert_eq!(partial.pot_internal[0], SELL_QUANTITY);
    assert_eq!(partial.pot_internal[3], 0);
    assert_eq!(
        partial.hoard_collateral,
        HoardAccount::decode(&bytes_of(&mut context, fixture.hoard_account).await)
            .unwrap()
            .collateral_atoms
    );

    /* 2. A payment before the burn.  Leg 0 has delivered, so its receipt's
     *    flags do route to the paying phase — and the pot, holding nothing,
     *    refuses rather than releasing collateral the market still needs. */
    let refused = send_walk(
        &mut context,
        fixture.settle_virtual(
            cleared.candidate,
            1,
            fixture.owners[0].position,
            cleared.reservations[0],
            0,
        ),
        510,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        ClutchError::AggregateClosureMismatch as u32,
        "a credit before the burn that funds it refuses"
    );
    let unchanged = ledgers(&mut context, &fixture).await;
    assert_eq!(unchanged.pot_cash, 0);
    assert_eq!(unchanged.owner_cash, partial.owner_cash);
    assert_market_closes(&unchanged);

    /* 3. A replayed delivery.  Leg 0's receipt already carries
     *    `SELL_CONSUMED`, so the only phase left is the payment above; the
     *    Eggs cannot move twice, and the reservation proves it. */
    let reservation = read_reservation(&mut context, cleared.reservations[0]).await;
    assert_eq!(reservation.consumed_units, reservation.entitled_units);
    assert_eq!(reservation.paid_units, 0);
    assert_eq!(unchanged.pot_internal[0], SELL_QUANTITY);

    /* 4. A forged payment ledger.  Hand-write leg 0's reservation claiming it
     *    has already been paid for more than it delivered — the shape the
     *    codec's `paid_units <= consumed_units` refuses outright. */
    let mut forged = reservation;
    forged.paid_units = forged.consumed_units + 1;
    let mut bytes = vec![0u8; RESERVATION_ACCOUNT_BYTES];
    assert!(
        forged.encode(&mut bytes).is_err(),
        "an overpaid ledger cannot even be encoded"
    );
    /* A ledger forged the *other* way is encodable — it is the honest merge
     *  window — so the refusal has to come from the seam.  Claim leg 1 was
     *  paid without delivering, and the direct seam's lockstep rule refuses
     *  it before any value moves. */
    let mut behind = read_reservation(&mut context, cleared.reservations[3]).await;
    assert_eq!(behind.consumed_units, 0);
    behind.consumed_units = 1;
    let mut behind_bytes = vec![0u8; RESERVATION_ACCOUNT_BYTES];
    behind
        .encode(&mut behind_bytes)
        .expect("a delivered-unpaid ledger is the honest merge window");

    /* 5. A forged pot inventory the supply ledger does not cover.  The burn
     *    runs the same C2 internal bound the mint does. */
    let mut forged_pot =
        FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    let ledger =
        SupplyLedgerAccount::decode(&bytes_of(&mut context, fixture.supply_account).await).unwrap();
    for outcome in 0..OUTCOMES as usize {
        forged_pot.pot_internal[outcome] = ledger.internal_supply[outcome] + 1;
    }
    context.set_account(
        &fixture.pot(),
        &AccountSharedData::from(program_account(encode(account_len::FINAL_POT, |out| {
            forged_pot.encode(out)
        }))),
    );
    let refused = send_walk(
        &mut context,
        fixture.settle_virtual(
            cleared.candidate,
            4,
            fixture.owners[3].position,
            cleared.reservations[3],
            3,
        ),
        520,
    )
    .await;
    assert!(
        refused.0.is_err(),
        "a pot whose inventory the ledger does not cover cannot burn"
    );
}

/// A delivery that would push one outcome past `mu` refuses.
///
/// The relation makes the merge absorb exactly `mu` on every outcome, so an
/// inventory that overshoots is a witness the verdict never priced — and the
/// bound is checked against the record's own churn, not against the pot.
#[tokio::test]
async fn a_delivery_past_the_candidates_mu_never_reaches_the_pot() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let cleared = cleared_merge_book(&mut context, &fixture, SELL_QUANTITY, MU).await;
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, cleared.candidate),
        342,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(
        &mut context,
        fixture.entitle_virtual(payer, cleared.candidate, 0, cleared.reservations[0]),
        400,
    )
    .await;
    result.unwrap();

    // Hand the pot an inventory that already holds this outcome's whole `mu`.
    let mut forged = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    forged.pot_internal[0] = MU;
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
            1,
            fixture.owners[0].position,
            cleared.reservations[0],
            0,
        ),
        500,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        ClutchError::AggregateClosureMismatch as u32,
        "the pot never absorbs more than mu on any outcome"
    );
}

/// A direct account shape never settles a merge receipt, and a replayed
/// payment has no phase left.
#[tokio::test]
async fn the_direct_shape_and_a_replayed_payment_both_refuse() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let cleared = cleared_merge_book(&mut context, &fixture, SELL_QUANTITY, MU).await;
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

    // The direct seven-account shape presented for a merge receipt: its
    // `leg_kind` is not `RECEIPT_LEG_DIRECT`, so the direct seam refuses it
    // rather than settling half a slice.
    let direct = fixture.settle_single(
        cleared.candidate,
        1,
        fixture.owners[0].position,
        fixture.owners[0].position,
        cleared.reservations[0],
        cleared.reservations[0],
        0,
    );
    let refused = send_walk(&mut context, direct, 480).await;
    assert!(
        refused.0.is_err(),
        "the direct shape never settles a merge receipt"
    );

    // Run the whole epoch, then replay the last payment.
    for index in 0..4u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                cleared.candidate,
                u64::from(index) + 1,
                fixture.owners[index as usize].position,
                cleared.reservations[index as usize],
                index,
            ),
            500 + index as u32,
        )
        .await;
        result.unwrap();
    }
    for index in 0..4u16 {
        let (result, _) = send_walk(
            &mut context,
            fixture.settle_virtual(
                cleared.candidate,
                u64::from(index) + 1,
                fixture.owners[index as usize].position,
                cleared.reservations[index as usize],
                index,
            ),
            600 + index as u32,
        )
        .await;
        result.unwrap();
    }
    let closed = ledgers(&mut context, &fixture).await;
    assert_eq!(closed.pot_cash, 0);

    let refused = send_walk(
        &mut context,
        fixture.settle_virtual(
            cleared.candidate,
            1,
            fixture.owners[0].position,
            cleared.reservations[0],
            0,
        ),
        700,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        ClutchError::MismatchedState as u32,
        "an exhausted receipt has no phase left"
    );
    let after = ledgers(&mut context, &fixture).await;
    assert_eq!(after.owner_cash, closed.owner_cash);
    assert_eq!(after.hoard_collateral, closed.hoard_collateral);
    assert_market_closes(&after);
}

/// The one ordering rule the split ledger imposes, stated on the bank: a sell
/// end shared between an ordinary slice and a merge leg must settle the
/// ordinary one **first**.
///
/// A direct slice moves Eggs and settles collateral in the same transition, so
/// it requires both of an end's ledgers level — and a merge delivery is exactly
/// what leaves them unlevel.  Nothing becomes unreachable by it: a direct slice
/// never depends on a virtual one, so the order direct-then-deliver-then-burn-
/// then-pay is always available.  One keeper order is forbidden, not a state.
#[tokio::test]
async fn a_shared_sell_end_settles_its_ordinary_slice_before_it_delivers() {
    let (mut context, fixture) = start().await;
    let cleared = entitled_mixed_book(&mut context, &fixture).await;

    // Deliver `B`'s merge leg first: its Eggs go to the pot and its payment
    // ledger stays behind, which is legal and persisted.
    let (result, _) = send_walk(
        &mut context,
        fixture.settle_virtual(
            cleared.candidate,
            2,
            fixture.owners[MERGE_OWNER[0]].position,
            cleared.reservations[1],
            1,
        ),
        500,
    )
    .await;
    result.unwrap();
    let shared = read_reservation(&mut context, cleared.reservations[1]).await;
    assert_eq!(shared.consumed_units, 4);
    assert_eq!(shared.paid_units, 0);
    assert_eq!(shared.unpaid_units(), 4);

    // Now the ordinary slice, which would have to settle four units of cash
    // against four units of Eggs that are already gone.
    let refused = send_walk(
        &mut context,
        fixture.settle_single_potted(
            cleared.candidate,
            1,
            fixture.owners[0].position,
            fixture.owners[1].position,
            cleared.reservations[0],
            cleared.reservations[1],
            0,
        ),
        501,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        ClutchError::MismatchedState as u32,
        "the direct seam settles both ledgers at once and needs them level"
    );
    // Nothing moved, and the state the keeper should have reached from is
    // still reachable — it just has to run the ordinary slice first.
    let after = ledgers(&mut context, &fixture).await;
    assert_eq!(after.pot_cash, 0);
    assert_eq!(after.pot_internal[0], 4);
    assert_market_closes(&after);
}
