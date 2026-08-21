//! Real-SBF evidence for the entitled direct-slice settlement consumption.
//!
//! The candidate and receipt are installed as already-selected,
//! already-entitled genesis fixtures so this file isolates *consumption*; the
//! live freeze path that creates them (tags 58-59) is driven end to end in
//! `entitled_clearing.rs`.  The transition under test is the real program
//! ELF: it authenticates every canonical PDA, consumes two exact `ENTITLED`
//! reservations, transfers cash and one Egg atom family together, releases
//! the buyer's unused envelope, and exhausts the receipt once — the T2-8
//! seven-account `SettlePage` shape, no page and no feed in the list.

use {
    clutch_sbf::{error::ClutchError, instructions::orders_batch, seeds},
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id,
        reservation::{
            canonical_reservation_id, ReservationAccount, ReservationPlan,
            RESERVATION_ACCOUNT_BYTES,
        },
        stream, CandidateRecord, EpochAccount, Hash32, Intent, OrderRecord, OrderSlot,
        PositionAccount, SettlementReceiptAccount, CANDIDATE_STATUS_SELECTED,
        CANDIDATE_STATUS_SUBMITTED, EPOCH_PHASE_CLEARED, MAX_OUTCOMES, RECEIPT_FLAG_BUY_CONSUMED,
        RECEIPT_FLAG_SELL_CONSUMED, RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_DIRECT,
        RELATION_VERSION,
    },
    clutch_svm_fixture::{compute_unit_limit_data, layout_request, COMPUTE_BUDGET, PROGRAM_ID},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, BanksClient, ProgramTest},
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const PRICE_SCALE: u64 = 10_000;
const EPOCH_INDEX: u64 = 7;

fn h(byte: u8) -> Hash32 {
    Hash32::from_bytes([byte; 32])
}

fn pda(prefix: &[u8], suffixes: &[&[u8]]) -> (Address, u8) {
    let mut all = Vec::with_capacity(1 + suffixes.len());
    all.push(prefix);
    all.extend_from_slice(suffixes);
    Address::find_program_address(&all, &PROGRAM_ID)
}

fn rent_exempt(len: usize) -> u64 {
    solana_rent::Rent::default().minimum_balance(len).max(1)
}

fn encode<F>(len: usize, writer: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, clutch_solana_layout::CodecError>,
{
    let mut bytes = vec![0; len];
    assert_eq!(writer(&mut bytes).unwrap(), len);
    bytes
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    StaleCandidate,
    CrossOutcome,
}

struct Fixture {
    epoch: Address,
    candidate: Address,
    buyer_position: Address,
    seller_position: Address,
    buyer_reservation: Address,
    seller_reservation: Address,
    receipt: Address,
    market: Hash32,
    epoch_id: Hash32,
}

impl Fixture {
    fn instruction(&self, swap_positions: bool) -> Instruction {
        let (buyer, seller) = if swap_positions {
            (self.seller_position, self.buyer_position)
        } else {
            (self.buyer_position, self.seller_position)
        };
        let metas = vec![
            AccountMeta::new_readonly(self.epoch, false),
            AccountMeta::new_readonly(self.candidate, false),
            AccountMeta::new(buyer, false),
            AccountMeta::new(seller, false),
            AccountMeta::new(self.buyer_reservation, false),
            AccountMeta::new(self.seller_reservation, false),
            AccountMeta::new(self.receipt, false),
        ];
        assert_eq!(metas.len(), orders_batch::SETTLE_PAGE_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                1,
                Intent::SettlePage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index: 0,
                },
            ),
            metas,
        )
    }

    fn writable(&self) -> [Address; 5] {
        [
            self.buyer_position,
            self.seller_position,
            self.buyer_reservation,
            self.seller_reservation,
            self.receipt,
        ]
    }
}

fn add_state(test: &mut ProgramTest, address: Address, data: Vec<u8>) {
    test.add_account(
        address,
        Account {
            lamports: rent_exempt(data.len()),
            data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn start(mutation: Mutation) -> (BanksClient, Keypair, Fixture) {
    let market = h(0x31);
    let epoch_id = canonical_epoch_id(market, EPOCH_INDEX);
    let terms = h(0x51);
    let grid = h(0x52);
    let policy = h(0x53);
    let buy_owner = h(0x41);
    let sell_owner = h(0x42);
    let buy = OrderSlot::Single(OrderRecord {
        owner: buy_owner,
        order_id: canonical_order_id(1),
        outcome: 0,
        side: 0,
        quantity: 4,
        limit: 5_000,
        minimum_fill: 0,
        flags: 0,
        generation: 1,
        expiry_epoch: EPOCH_INDEX,
    });
    let sell_outcome = if matches!(mutation, Mutation::CrossOutcome) {
        1
    } else {
        0
    };
    let sell = OrderSlot::Single(OrderRecord {
        owner: sell_owner,
        order_id: canonical_order_id(2),
        outcome: sell_outcome,
        side: 1,
        quantity: 4,
        limit: 5_000,
        minimum_fill: 0,
        flags: 0,
        generation: 1,
        expiry_epoch: EPOCH_INDEX,
    });

    // The page is built host-side only to derive a real frozen `order_set`;
    // the entitled consumption shape presents no page account.
    let page_index_bytes = 0u16.to_le_bytes();
    let (_, page_bump) = pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &page_index_bytes]);
    let mut page = vec![0; account_len::ORDER_PAGE];
    stream::init_page(&mut page, market, epoch_id, 0, 1, page_bump).unwrap();
    stream::append_slot(&mut page, buy).unwrap();
    stream::append_slot(&mut page, sell).unwrap();
    let (order_set, order_count) = stream::frozen_set_commitment(&[&page]).unwrap();
    stream::seal_page(&mut page, order_set, order_count).unwrap();

    let epoch_index_bytes = EPOCH_INDEX.to_le_bytes();
    let (epoch_address, epoch_bump) =
        pda(seeds::SEED_EPOCH, &[&market.bytes(), &epoch_index_bytes]);
    let epoch = EpochAccount {
        epoch: epoch_id,
        market,
        book: h(0x55),
        terms,
        price_grid: grid,
        policy,
        order_set,
        first_order_id: buy.order_id(),
        last_order_id: sell.order_id(),
        epoch_index: EPOCH_INDEX,
        relation_version: RELATION_VERSION,
        price_scale: PRICE_SCALE,
        remainder_seed: 9,
        owner_count: 2,
        page_count: 1,
        order_count: 2,
        outcome_count: 2,
        phase: EPOCH_PHASE_CLEARED,
        stored_bump: epoch_bump,
        flags: 0,
    };

    let mut prices = [0; MAX_OUTCOMES];
    prices[0] = 5_000;
    prices[1] = 5_000;
    let mut candidate = CandidateRecord {
        candidate: Hash32::ZERO,
        epoch: epoch_id,
        market,
        prices,
        virtual_split: 0,
        virtual_merge: 0,
        honored_aon_mask: 0,
        weighted_direct_volume: 8,
        limit_surplus_price_units: 0,
        // v3: a SELECTED record carries a verified tie digest; the stale
        // (still-SUBMITTED) mutation carries none.
        score_digest: if matches!(mutation, Mutation::StaleCandidate) {
            Hash32::ZERO
        } else {
            Hash32::from_bytes([0x5d; 32])
        },
        churn: 0,
        submitted_slot: 80,
        distinct_owners: 2,
        order_len: 2,
        outcome_count: 2,
        status: if matches!(mutation, Mutation::StaleCandidate) {
            CANDIDATE_STATUS_SUBMITTED
        } else {
            CANDIDATE_STATUS_SELECTED
        },
        stored_bump: 0,
        flags: 0,
    };
    candidate.candidate = candidate.recomputed_candidate_digest().unwrap();
    let (candidate_address, candidate_bump) = pda(
        seeds::SEED_CANDIDATE,
        &[&epoch_id.bytes(), &candidate.candidate.bytes()],
    );
    candidate.stored_bump = candidate_bump;

    let (buyer_position_address, buyer_position_bump) =
        pda(seeds::SEED_POSITION, &[&market.bytes(), &buy_owner.bytes()]);
    let buyer_position = PositionAccount {
        market,
        owner: buy_owner,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 10,
        reserved_cash_atoms: 2,
        stored_bump: buyer_position_bump,
        close_state: 0,
    };
    let (seller_position_address, seller_position_bump) = pda(
        seeds::SEED_POSITION,
        &[&market.bytes(), &sell_owner.bytes()],
    );
    let seller_position = PositionAccount {
        market,
        owner: sell_owner,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        stored_bump: seller_position_bump,
        close_state: 0,
    };

    let buy_plan = ReservationPlan::for_order(&buy, 2, PRICE_SCALE, 0).unwrap();
    let buy_reservation_id =
        canonical_reservation_id(market, epoch_id, buy_owner, 0, buy.order_id());
    let (buyer_reservation_address, buyer_reservation_bump) =
        pda(seeds::SEED_RESERVATION, &[&buy_reservation_id.bytes()]);
    let mut buyer_reservation = ReservationAccount::active(
        market,
        epoch_id,
        buy_owner,
        buy.order_id(),
        grid,
        terms,
        policy,
        0,
        buy.generation(),
        0,
        buyer_reservation_bump,
        buy_plan,
    )
    .unwrap();
    let sell_plan = ReservationPlan::for_order(&sell, 2, PRICE_SCALE, 0).unwrap();
    let sell_reservation_id =
        canonical_reservation_id(market, epoch_id, sell_owner, 0, sell.order_id());
    let (seller_reservation_address, seller_reservation_bump) =
        pda(seeds::SEED_RESERVATION, &[&sell_reservation_id.bytes()]);
    let mut seller_reservation = ReservationAccount::active(
        market,
        epoch_id,
        sell_owner,
        sell.order_id(),
        grid,
        terms,
        policy,
        0,
        sell.generation(),
        0,
        seller_reservation_bump,
        sell_plan,
    )
    .unwrap();
    // The entitlement freeze's poststate: the untouched envelope, ENTITLED,
    // each end stamped with its whole order's entitled total.  Both orders
    // are wholly filled by this one slice, so the total is its quantity and
    // this one consumption completes both ends.
    let buyer_reservation = buyer_reservation.entitled(4).unwrap();
    let seller_reservation = seller_reservation.entitled(4).unwrap();

    let slice_bytes = 0u16.to_le_bytes();
    let (receipt_address, receipt_bump) = pda(
        seeds::SEED_RECEIPT,
        &[
            &epoch_id.bytes(),
            &candidate.candidate.bytes(),
            &slice_bytes,
        ],
    );
    let receipt = SettlementReceiptAccount {
        epoch: epoch_id,
        market,
        candidate: candidate.candidate,
        buy_order_id: buy.order_id(),
        sell_order_id: sell.order_id(),
        consideration_price_units: 20_000,
        quantity: 4,
        settled_quantity: 0,
        price: 5_000,
        sequence: 1,
        slice_index: 0,
        outcome: 0,
        leg_kind: RECEIPT_LEG_DIRECT,
        consumed_flags: 0,
        stored_bump: receipt_bump,
        flags: 0,
    };

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    add_state(
        &mut test,
        epoch_address,
        encode(account_len::EPOCH, |out| epoch.encode(out)),
    );
    add_state(
        &mut test,
        candidate_address,
        encode(account_len::CANDIDATE, |out| candidate.encode(out)),
    );
    add_state(
        &mut test,
        buyer_position_address,
        encode(account_len::POSITION, |out| buyer_position.encode(out)),
    );
    add_state(
        &mut test,
        seller_position_address,
        encode(account_len::POSITION, |out| seller_position.encode(out)),
    );
    add_state(
        &mut test,
        buyer_reservation_address,
        encode(RESERVATION_ACCOUNT_BYTES, |out| {
            buyer_reservation.encode(out)
        }),
    );
    add_state(
        &mut test,
        seller_reservation_address,
        encode(RESERVATION_ACCOUNT_BYTES, |out| {
            seller_reservation.encode(out)
        }),
    );
    add_state(
        &mut test,
        receipt_address,
        encode(account_len::SETTLEMENT_RECEIPT, |out| receipt.encode(out)),
    );
    let fixture = Fixture {
        epoch: epoch_address,
        candidate: candidate_address,
        buyer_position: buyer_position_address,
        seller_position: seller_position_address,
        buyer_reservation: buyer_reservation_address,
        seller_reservation: seller_reservation_address,
        receipt: receipt_address,
        market,
        epoch_id,
    };
    let (banks, payer, _) = test.start().await;
    (banks, payer, fixture)
}

async fn send(
    banks: &mut BanksClient,
    payer: &Keypair,
    instruction: Instruction,
    nonce: u32,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    // The nonce keeps a replayed instruction's transaction bytes distinct,
    // so the bank refuses it in the program rather than as a duplicate.
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(1_400_000 - nonce),
        Vec::new(),
    );
    let transaction = Transaction::new_signed_with_payer(
        &[budget, instruction],
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );
    let outcome = banks
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap();
    let units = outcome
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    (outcome.result, units)
}

async fn bytes(banks: &mut BanksClient, address: Address) -> Vec<u8> {
    banks
        .get_account(address)
        .await
        .unwrap()
        .expect("fixture account exists")
        .data
}

async fn snapshot(banks: &mut BanksClient, addresses: [Address; 5]) -> [Vec<u8>; 5] {
    [
        bytes(banks, addresses[0]).await,
        bytes(banks, addresses[1]).await,
        bytes(banks, addresses[2]).await,
        bytes(banks, addresses[3]).await,
        bytes(banks, addresses[4]).await,
    ]
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn direct_full_slice_settles_once_and_substitution_rolls_back() {
    let (mut banks, payer, fixture) = start(Mutation::None).await;
    let watched = fixture.writable();
    let before = snapshot(&mut banks, watched).await;
    let swapped = send(&mut banks, &payer, fixture.instruction(true), 0).await;
    assert_eq!(custom(swapped.0), ClutchError::MismatchedState as u32);
    assert_eq!(snapshot(&mut banks, watched).await, before);

    let (result, units) = send(&mut banks, &payer, fixture.instruction(false), 1).await;
    result.unwrap();
    assert!(units < 1_400_000);
    eprintln!("SettlePage direct full-slice CU: {units}");
    let buyer = PositionAccount::decode(&bytes(&mut banks, fixture.buyer_position).await).unwrap();
    let seller =
        PositionAccount::decode(&bytes(&mut banks, fixture.seller_position).await).unwrap();
    let buy_res =
        ReservationAccount::decode(&bytes(&mut banks, fixture.buyer_reservation).await).unwrap();
    let sell_res =
        ReservationAccount::decode(&bytes(&mut banks, fixture.seller_reservation).await).unwrap();
    let receipt =
        SettlementReceiptAccount::decode(&bytes(&mut banks, fixture.receipt).await).unwrap();
    assert_eq!(buyer.cash_atoms, 8);
    assert_eq!(buyer.reserved_cash_atoms, 0);
    assert_eq!(buyer.internal[0], 4);
    assert_eq!(seller.cash_atoms, 2);
    assert!(buy_res.remaining_is_zero() && sell_res.remaining_is_zero());
    assert_eq!(receipt.settled_quantity, 4);
    assert_eq!(
        receipt.consumed_flags,
        RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED
    );

    let after = snapshot(&mut banks, watched).await;
    let replay = send(&mut banks, &payer, fixture.instruction(false), 2).await;
    assert_eq!(custom(replay.0), ClutchError::MismatchedState as u32);
    assert_eq!(snapshot(&mut banks, watched).await, after);
}

#[tokio::test]
async fn stale_candidate_and_cross_outcome_refuse_with_full_rollback() {
    for mutation in [Mutation::StaleCandidate, Mutation::CrossOutcome] {
        let (mut banks, payer, fixture) = start(mutation).await;
        let watched = fixture.writable();
        let before = snapshot(&mut banks, watched).await;
        let result = send(&mut banks, &payer, fixture.instruction(false), 3).await;
        let code = custom(result.0);
        assert!(
            code == ClutchError::NotActive as u32 || code == ClutchError::MismatchedState as u32
        );
        assert_eq!(snapshot(&mut banks, watched).await, before);
    }
}
