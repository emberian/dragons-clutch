//! Real-SBF evidence for deterministic narrow candidate submission.
//!
//! `SubmitDirectPage` creates only a `SUBMITTED` Candidate and its exact
//! CandidateFeed from one authenticated, frozen, fully reserved two-order
//! book. It does not select the candidate, move the Epoch to `CLEARED`, or
//! create a SettlementReceipt. These tests exercise the real program and
//! System program, including predictable-PDA prefunding and a failure after
//! the first creation CPI whose entire transaction must roll back.

use {
    clutch_sbf::{
        error::ClutchError,
        instructions::{artifact::CLOCK_SYSVAR_ID, orders_batch},
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id,
        clearing::{
            fill_at, slice_at, verify_candidate_feed, LegRef, PairingSlice,
            CANDIDATE_FEED_FLAG_SLICES_DECLARED,
        },
        reservation::{
            canonical_reservation_id, ReservationAccount, ReservationPlan,
            RESERVATION_ACCOUNT_BYTES,
        },
        stream, CandidateRecord, EpochAccount, Hash32, Intent, OrderRecord, OrderSlot,
        PriceGridAccount, CANDIDATE_STATUS_SUBMITTED, EPOCH_PHASE_FROZEN, MAX_GRID_TICKS,
        MAX_OUTCOMES, RELATION_VERSION,
    },
    clutch_svm_fixture::{
        compute_unit_limit_data, layout_request, COMPUTE_BUDGET, PROGRAM_ID, RENT_SYSVAR,
        SYSTEM_PROGRAM,
    },
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
const CU_LIMIT: u32 = 1_400_000;

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

fn system_slot(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    }
}

#[derive(Clone, Copy)]
enum BookShape {
    Direct,
    CrossOutcome,
}

struct Fixture {
    epoch: Address,
    grid: Address,
    page: Address,
    reservation_zero: Address,
    reservation_one: Address,
    candidate: Address,
    feed: Address,
    market: Hash32,
    epoch_id: Hash32,
}

impl Fixture {
    fn instruction(&self, payer: Address, swap_reservations: bool) -> Instruction {
        let (zero, one) = if swap_reservations {
            (self.reservation_one, self.reservation_zero)
        } else {
            (self.reservation_zero, self.reservation_one)
        };
        let metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.epoch, false),
            AccountMeta::new_readonly(self.grid, false),
            AccountMeta::new_readonly(self.page, false),
            AccountMeta::new_readonly(zero, false),
            AccountMeta::new_readonly(one, false),
            AccountMeta::new(self.candidate, false),
            AccountMeta::new(self.feed, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(Address::new_from_array(CLOCK_SYSVAR_ID.to_bytes()), false),
        ];
        assert_eq!(metas.len(), orders_batch::SUBMIT_DIRECT_PAGE_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::SubmitDirectPage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index: 0,
                },
            ),
            metas,
        )
    }

    fn immutable(&self) -> [Address; 5] {
        [
            self.epoch,
            self.grid,
            self.page,
            self.reservation_zero,
            self.reservation_one,
        ]
    }
}

fn candidate_for(epoch: &EpochAccount, outcome: u8, limit: u64) -> CandidateRecord {
    let mut prices = [0; MAX_OUTCOMES];
    prices[usize::from(outcome)] = limit;
    prices[1usize - usize::from(outcome)] = epoch.price_scale - limit;
    let mut candidate = CandidateRecord {
        candidate: Hash32::ZERO,
        epoch: epoch.epoch,
        market: epoch.market,
        prices,
        virtual_split: 0,
        virtual_merge: 0,
        honored_aon_mask: 0,
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        churn: 0,
        submitted_slot: 0,
        distinct_owners: 0,
        order_len: 2,
        outcome_count: 2,
        status: CANDIDATE_STATUS_SUBMITTED,
        stored_bump: 0,
        flags: 0,
    };
    candidate.candidate = candidate.recomputed_candidate_digest().unwrap();
    candidate
}

async fn start(
    shape: BookShape,
    candidate_prefund: u64,
    feed_prefund: u64,
    low_funder: bool,
) -> (BanksClient, Keypair, Option<Keypair>, Fixture) {
    let market = h(0x31);
    let epoch_id = canonical_epoch_id(market, EPOCH_INDEX);
    let terms = h(0x51);
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
    let sell = OrderSlot::Single(OrderRecord {
        owner: sell_owner,
        order_id: canonical_order_id(2),
        outcome: if matches!(shape, BookShape::CrossOutcome) {
            1
        } else {
            0
        },
        side: 1,
        quantity: 4,
        limit: 5_000,
        minimum_fill: 0,
        flags: 0,
        generation: 1,
        expiry_epoch: EPOCH_INDEX,
    });

    let mut ticks = [0; MAX_GRID_TICKS];
    ticks[0] = 1_000;
    ticks[1] = 5_000;
    let mut grid = PriceGridAccount {
        grid: Hash32::ZERO,
        realm: h(0x61),
        price_scale: PRICE_SCALE,
        tick_count: 2,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid.grid = grid.recomputed_grid_id().unwrap();
    let (grid_address, grid_bump) =
        pda(seeds::SEED_GRID, &[&grid.realm.bytes(), &grid.grid.bytes()]);
    grid.stored_bump = grid_bump;

    let page_index_bytes = 0u16.to_le_bytes();
    let (page_address, page_bump) = pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &page_index_bytes]);
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
        price_grid: grid.grid,
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
        phase: EPOCH_PHASE_FROZEN,
        stored_bump: epoch_bump,
        flags: 0,
    };

    let buy_plan = ReservationPlan::for_order(&buy, 2, PRICE_SCALE, 0).unwrap();
    let buy_reservation_id =
        canonical_reservation_id(market, epoch_id, buy_owner, 0, buy.order_id());
    let (buy_reservation_address, buy_reservation_bump) =
        pda(seeds::SEED_RESERVATION, &[&buy_reservation_id.bytes()]);
    let buy_reservation = ReservationAccount::active(
        market,
        epoch_id,
        buy_owner,
        buy.order_id(),
        grid.grid,
        terms,
        policy,
        0,
        buy.generation(),
        0,
        buy_reservation_bump,
        buy_plan,
    )
    .unwrap();
    let sell_plan = ReservationPlan::for_order(&sell, 2, PRICE_SCALE, 0).unwrap();
    let sell_reservation_id =
        canonical_reservation_id(market, epoch_id, sell_owner, 0, sell.order_id());
    let (sell_reservation_address, sell_reservation_bump) =
        pda(seeds::SEED_RESERVATION, &[&sell_reservation_id.bytes()]);
    let sell_reservation = ReservationAccount::active(
        market,
        epoch_id,
        sell_owner,
        sell.order_id(),
        grid.grid,
        terms,
        policy,
        0,
        sell.generation(),
        0,
        sell_reservation_bump,
        sell_plan,
    )
    .unwrap();

    let candidate = candidate_for(&epoch, 0, 5_000);
    let (candidate_address, _) = pda(
        seeds::SEED_CANDIDATE,
        &[&epoch_id.bytes(), &candidate.candidate.bytes()],
    );
    let (feed_address, _) = pda(
        seeds::SEED_CANDIDATE_FEED,
        &[&epoch_id.bytes(), &candidate.candidate.bytes()],
    );

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
        grid_address,
        encode(account_len::PRICE_GRID, |out| grid.encode(out)),
    );
    add_state(&mut test, page_address, page);
    add_state(
        &mut test,
        buy_reservation_address,
        encode(RESERVATION_ACCOUNT_BYTES, |out| buy_reservation.encode(out)),
    );
    add_state(
        &mut test,
        sell_reservation_address,
        encode(RESERVATION_ACCOUNT_BYTES, |out| {
            sell_reservation.encode(out)
        }),
    );
    if candidate_prefund != 0 {
        test.add_account(candidate_address, system_slot(candidate_prefund));
    }
    if feed_prefund != 0 {
        test.add_account(feed_address, system_slot(feed_prefund));
    }
    let funder = if low_funder {
        let keypair = Keypair::new();
        // Enough for Candidate, deliberately far below CandidateFeed rent.
        test.add_account(
            keypair.pubkey(),
            system_slot(rent_exempt(account_len::CANDIDATE) + 1_000),
        );
        Some(keypair)
    } else {
        None
    };
    let fixture = Fixture {
        epoch: epoch_address,
        grid: grid_address,
        page: page_address,
        reservation_zero: buy_reservation_address,
        reservation_one: sell_reservation_address,
        candidate: candidate_address,
        feed: feed_address,
        market,
        epoch_id,
    };
    let (banks, payer, _) = test.start().await;
    (banks, payer, funder, fixture)
}

async fn send(
    banks: &mut BanksClient,
    fee_payer: &Keypair,
    funder: Option<&Keypair>,
    instruction: Instruction,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT),
        Vec::new(),
    );
    let mut signers = vec![fee_payer];
    if let Some(funder) = funder {
        signers.push(funder);
    }
    let transaction = Transaction::new_signed_with_payer(
        &[budget, instruction],
        Some(&fee_payer.pubkey()),
        &signers,
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

async fn account(banks: &mut BanksClient, address: Address) -> Option<Account> {
    banks.get_account(address).await.unwrap()
}

async fn bytes(banks: &mut BanksClient, address: Address) -> Vec<u8> {
    account(banks, address).await.expect("account exists").data
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
async fn prefunded_submission_is_exact_once_and_leaves_authority_frozen() {
    let (mut banks, payer, _, fixture) = start(BookShape::Direct, 1, 1, false).await;
    let immutable_before = snapshot(&mut banks, fixture.immutable()).await;
    let (result, units) = send(
        &mut banks,
        &payer,
        None,
        fixture.instruction(payer.pubkey(), false),
    )
    .await;
    result.unwrap();
    assert!(units < u64::from(CU_LIMIT));
    eprintln!("SubmitDirectPage prefunded CU: {units}");

    let candidate_account = account(&mut banks, fixture.candidate).await.unwrap();
    let candidate = CandidateRecord::decode(&candidate_account.data).unwrap();
    assert_eq!(candidate.status, CANDIDATE_STATUS_SUBMITTED);
    assert_eq!(candidate.weighted_direct_volume, 0);
    assert_eq!(candidate.distinct_owners, 0);
    assert!(candidate.submitted_slot > 0);
    assert_eq!(candidate_account.owner, PROGRAM_ID);
    assert_eq!(
        candidate_account.lamports,
        rent_exempt(account_len::CANDIDATE)
    );

    let feed_account = account(&mut banks, fixture.feed).await.unwrap();
    let feed = verify_candidate_feed(&feed_account.data).unwrap();
    assert_eq!(feed.candidate, candidate.candidate);
    assert_eq!(feed.claimed_digest, 0);
    assert_eq!(feed.flags, CANDIDATE_FEED_FLAG_SLICES_DECLARED);
    assert_eq!(fill_at(&feed_account.data, &feed, 0).unwrap(), 4);
    assert_eq!(fill_at(&feed_account.data, &feed, 1).unwrap(), 4);
    assert_eq!(
        slice_at(&feed_account.data, &feed, 0).unwrap(),
        PairingSlice {
            buy_ref: LegRef::Order(0),
            sell_ref: LegRef::Order(1),
            outcome: 0,
            quantity: 4,
        }
    );
    assert_eq!(feed_account.owner, PROGRAM_ID);
    assert_eq!(
        feed_account.lamports,
        rent_exempt(account_len::CANDIDATE_FEED)
    );
    assert_eq!(
        snapshot(&mut banks, fixture.immutable()).await,
        immutable_before
    );
    let epoch = EpochAccount::decode(&bytes(&mut banks, fixture.epoch).await).unwrap();
    assert_eq!(epoch.phase, EPOCH_PHASE_FROZEN);

    let candidate_after = candidate_account;
    let feed_after = feed_account;
    let replay = send(
        &mut banks,
        &payer,
        None,
        fixture.instruction(payer.pubkey(), false),
    )
    .await;
    assert_eq!(custom(replay.0), ClutchError::AlreadyInitialized as u32);
    assert_eq!(
        account(&mut banks, fixture.candidate).await.unwrap(),
        candidate_after
    );
    assert_eq!(account(&mut banks, fixture.feed).await.unwrap(), feed_after);
}

#[tokio::test]
async fn substitution_cross_outcome_and_late_cpi_failure_roll_back() {
    let (mut banks, payer, _, fixture) = start(BookShape::Direct, 0, 0, false).await;
    let immutable_before = snapshot(&mut banks, fixture.immutable()).await;
    let mut wrong_targets = fixture.instruction(payer.pubkey(), false);
    wrong_targets.accounts.swap(6, 7);
    let wrong_targets = send(&mut banks, &payer, None, wrong_targets).await;
    assert_eq!(custom(wrong_targets.0), ClutchError::WrongPda as u32);
    assert!(account(&mut banks, fixture.candidate).await.is_none());
    assert!(account(&mut banks, fixture.feed).await.is_none());
    assert_eq!(
        snapshot(&mut banks, fixture.immutable()).await,
        immutable_before
    );

    let swapped = send(
        &mut banks,
        &payer,
        None,
        fixture.instruction(payer.pubkey(), true),
    )
    .await;
    assert_eq!(custom(swapped.0), ClutchError::MismatchedState as u32);
    assert!(account(&mut banks, fixture.candidate).await.is_none());
    assert!(account(&mut banks, fixture.feed).await.is_none());
    assert_eq!(
        snapshot(&mut banks, fixture.immutable()).await,
        immutable_before
    );

    let (mut banks, payer, _, cross) = start(BookShape::CrossOutcome, 0, 0, false).await;
    let before = snapshot(&mut banks, cross.immutable()).await;
    let refused = send(
        &mut banks,
        &payer,
        None,
        cross.instruction(payer.pubkey(), false),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::MismatchedState as u32);
    assert!(account(&mut banks, cross.candidate).await.is_none());
    assert!(account(&mut banks, cross.feed).await.is_none());
    assert_eq!(snapshot(&mut banks, cross.immutable()).await, before);

    let (mut banks, payer, funder, fixture) = start(BookShape::Direct, 0, 0, true).await;
    let funder = funder.unwrap();
    let funder_before = account(&mut banks, funder.pubkey()).await.unwrap();
    let immutable_before = snapshot(&mut banks, fixture.immutable()).await;
    let failed = send(
        &mut banks,
        &payer,
        Some(&funder),
        fixture.instruction(funder.pubkey(), false),
    )
    .await;
    // The real runtime surfaces the nested System program's insufficient-funds
    // custom code (`1`) rather than the caller's mapped adapter code. The
    // important protocol property here is the bank-level atomic rollback of
    // the Candidate creation that completed before that nested refusal.
    assert_eq!(custom(failed.0), 1);
    assert!(account(&mut banks, fixture.candidate).await.is_none());
    assert!(account(&mut banks, fixture.feed).await.is_none());
    assert_eq!(
        account(&mut banks, funder.pubkey()).await.unwrap(),
        funder_before
    );
    assert_eq!(
        snapshot(&mut banks, fixture.immutable()).await,
        immutable_before
    );
}
