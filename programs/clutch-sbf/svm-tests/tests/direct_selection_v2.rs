//! Real-SBF evidence for the bounded V2 authority chain and its measured cost.
//!
//! Candidate, Window, receipt, pot, and Epoch accounts start as predictable
//! one-lamport System accounts. The real program initializes/freezes the Epoch
//! and admits a streaming candidate set without genesis-injected selection
//! authority.
//!
//! Full top-three selection used to be a measured STOP here: complete
//! cached-domain re-execution exceeded the 1.4m-CU transaction cap and rolled
//! back every authority mutation. That was a cost of the software SHA-256 the
//! program used to carry, not of the selection itself, and it no longer holds
//! now that identities are hashed with the runtime syscall. The measurement
//! below records the new cost. It is not a promotion decision: V3 still stages
//! verification and bounds persistent candidate rent, and whether this path may
//! be called live is a question about rent and staging that this file does not
//! answer.

use {
    clutch_batch_policy_identity::{
        batch_policy_digest,
        direct_window_v1::{
            canonical_account_candidate_id, verify_direct_two_order_candidate,
            DirectTwoOrderInputV1, DIRECT_CANDIDATE_ACCOUNT_BYTES,
            DIRECT_CANDIDATE_STATUS_SUPERSEDED, DIRECT_POLICY_V1, DIRECT_WINDOW_ACCOUNT_BYTES,
        },
        encode_batch_policy, FullRelationDomainV1, Identity32V1, BATCH_POLICY_BYTES,
    },
    clutch_sbf::{error::ClutchError, instructions::artifact::CLOCK_SYSVAR_ID, seeds},
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        direct_selection::{
            decode_direct_candidate, decode_direct_window, encode_direct_candidate,
            encode_direct_window, DirectEpochV3Account, DIRECT_EPOCH_BYTES,
        },
        reservation::{
            canonical_reservation_id, ReservationAccount, ReservationPlan,
            RESERVATION_ACCOUNT_BYTES,
        },
        stream, Hash32, Intent, MarketAccount, OrderRecord, OrderSlot, PositionAccount,
        PriceGridAccount, EPOCH_PHASE_FROZEN, EPOCH_PHASE_OPEN, MAX_GRID_TICKS, MAX_OUTCOMES,
    },
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_terms, layout_request, COMPUTE_BUDGET, PROGRAM_ID,
        RENT_SYSVAR, SYSTEM_PROGRAM,
    },
    solana_account::{Account, AccountSharedData},
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const PRICE_SCALE: u64 = 10_000;
const QUANTITY: u64 = 20;
const EPOCH_INDEX: u64 = 7;
const OPEN_SLOT: u64 = 10;
const CLOSE_SLOT: u64 = 20;
const CU_LIMIT: u32 = 1_400_000;
const CANDIDATE_PRICES: [u64; 5] = [2_500, 3_000, 7_000, 5_000, 7_500];

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

fn candidate_identity(epoch: Hash32, market: Hash32, price: u64) -> Identity32V1 {
    let mut prices = [0; MAX_OUTCOMES];
    prices[0] = price;
    prices[1] = PRICE_SCALE - price;
    canonical_account_candidate_id(
        Identity32V1(epoch.bytes()),
        Identity32V1(market.bytes()),
        &prices,
    )
}

struct Fixture {
    market: Hash32,
    epoch_id: Hash32,
    market_account: Address,
    terms: Address,
    grid: Address,
    policy: Address,
    wrong_policy: Address,
    epoch: Address,
    page: Address,
    buy_reservation: Address,
    sell_reservation: Address,
    window: Address,
    candidates: [Address; CANDIDATE_PRICES.len()],
    candidate_ids: [Identity32V1; CANDIDATE_PRICES.len()],
    receipt: Address,
    pot: Address,
}

impl Fixture {
    fn init(&self, payer: Address, policy: Address) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitDirectEpochV3 {
                    market: self.market,
                    epoch_index: EPOCH_INDEX,
                    policy: Hash32::from_bytes(batch_policy_digest(&DIRECT_POLICY_V1).unwrap().0),
                    submission_opens_slot: OPEN_SLOT,
                    submission_closes_slot: CLOSE_SLOT,
                },
            ),
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(self.epoch, false),
                AccountMeta::new_readonly(self.market_account, false),
                AccountMeta::new_readonly(self.terms, false),
                AccountMeta::new_readonly(self.grid, false),
                AccountMeta::new_readonly(policy, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(
                    Address::new_from_array(CLOCK_SYSVAR_ID.to_bytes()),
                    false,
                ),
            ],
        )
    }

    fn freeze(&self) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::FreezeDirectEpochV3 {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            vec![
                AccountMeta::new(self.epoch, false),
                AccountMeta::new(self.page, false),
                AccountMeta::new_readonly(self.grid, false),
                AccountMeta::new_readonly(self.policy, false),
                AccountMeta::new_readonly(self.buy_reservation, false),
                AccountMeta::new_readonly(self.sell_reservation, false),
                AccountMeta::new_readonly(
                    Address::new_from_array(CLOCK_SYSVAR_ID.to_bytes()),
                    false,
                ),
            ],
        )
    }

    async fn retained(&self, context: &mut ProgramTestContext) -> Vec<Address> {
        let Some(account) = account(context, self.window).await else {
            return Vec::new();
        };
        if account.owner != PROGRAM_ID {
            return Vec::new();
        }
        let window = decode_direct_window(&account.data).unwrap();
        window.top[..usize::from(window.top_count)]
            .iter()
            .map(|entry| {
                pda(
                    seeds::SEED_DIRECT_CANDIDATE,
                    &[&self.epoch_id.bytes(), &entry.candidate_id.0],
                )
                .0
            })
            .collect()
    }

    async fn submit(
        &self,
        context: &mut ProgramTestContext,
        payer: Address,
        candidate_index: usize,
    ) -> Instruction {
        let retained = self.retained(context).await;
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.epoch, false),
            AccountMeta::new_readonly(self.policy, false),
            AccountMeta::new_readonly(self.grid, false),
            AccountMeta::new_readonly(self.page, false),
            AccountMeta::new_readonly(self.buy_reservation, false),
            AccountMeta::new_readonly(self.sell_reservation, false),
            AccountMeta::new(self.window, false),
            AccountMeta::new(self.candidates[candidate_index], false),
        ];
        metas.extend(
            retained
                .into_iter()
                .map(|address| AccountMeta::new(address, false)),
        );
        metas.extend([
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(Address::new_from_array(CLOCK_SYSVAR_ID.to_bytes()), false),
        ]);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::SubmitDirectCandidateV2 {
                    market: self.market,
                    epoch: self.epoch_id,
                    outcome_price: CANDIDATE_PRICES[candidate_index],
                },
            ),
            metas,
        )
    }

    async fn select(
        &self,
        context: &mut ProgramTestContext,
        payer: Address,
        policy: Address,
        swap_first_two: bool,
    ) -> Instruction {
        let mut retained = self.retained(context).await;
        if swap_first_two && retained.len() >= 2 {
            retained.swap(0, 1);
        }
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(self.epoch, false),
            AccountMeta::new_readonly(policy, false),
            AccountMeta::new_readonly(self.grid, false),
            AccountMeta::new_readonly(self.page, false),
            AccountMeta::new(self.buy_reservation, false),
            AccountMeta::new(self.sell_reservation, false),
            AccountMeta::new(self.window, false),
        ];
        metas.extend(
            retained
                .into_iter()
                .map(|address| AccountMeta::new(address, false)),
        );
        metas.extend([
            AccountMeta::new(self.receipt, false),
            AccountMeta::new(self.pot, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(Address::new_from_array(CLOCK_SYSVAR_ID.to_bytes()), false),
        ]);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::SelectDirectWindowV1 {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            metas,
        )
    }

    fn selection_state(&self) -> Vec<Address> {
        let mut addresses = vec![
            self.epoch,
            self.window,
            self.buy_reservation,
            self.sell_reservation,
            self.receipt,
            self.pot,
        ];
        addresses.extend(self.candidates);
        addresses
    }
}

async fn start(cross_outcome: bool) -> (ProgramTestContext, Keypair, Fixture) {
    let realm = h(0x61);
    let profile = h(0x62);
    let feed = h(0x63);
    let market = h(0x31);
    let epoch_id = canonical_epoch_id(market, EPOCH_INDEX);
    let buy_owner = h(0x41);
    let sell_owner = h(0x42);

    let mut ticks = [0; MAX_GRID_TICKS];
    ticks[..5].copy_from_slice(&[2_500, 3_000, 5_000, 7_000, 7_500]);
    let mut grid = PriceGridAccount {
        grid: Hash32::ZERO,
        realm,
        price_scale: PRICE_SCALE,
        tick_count: 5,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid.grid = grid.recomputed_grid_id().unwrap();
    let (grid_address, grid_bump) = pda(seeds::SEED_GRID, &[&realm.bytes(), &grid.grid.bytes()]);
    grid.stored_bump = grid_bump;

    let mut terms = fixture_terms(realm, profile, feed);
    terms.price_grid = grid.grid;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    let (terms_address, terms_bump) =
        pda(seeds::SEED_TERMS, &[&realm.bytes(), &terms.terms.bytes()]);
    terms.stored_bump = terms_bump;

    let (market_address, market_bump) = pda(seeds::SEED_MARKET, &[&realm.bytes(), &market.bytes()]);
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    outcomes[0] = canonical_outcome_id(market, 0);
    outcomes[1] = canonical_outcome_id(market, 1);
    let market_state = MarketAccount {
        market,
        realm,
        profile,
        terms: terms.terms,
        outcome_count: 2,
        lifecycle: 0,
        stored_bump: market_bump,
        hoard_bump: 0,
        outcomes,
        feed,
        collateral_cap: terms.collateral_cap,
        created_slot: 0,
        reserved: Hash32::ZERO,
    };

    let mut policy_bytes = vec![0; BATCH_POLICY_BYTES];
    encode_batch_policy(&DIRECT_POLICY_V1, &mut policy_bytes).unwrap();
    let policy_digest = Hash32::from_bytes(batch_policy_digest(&DIRECT_POLICY_V1).unwrap().0);
    let (policy_address, _) = pda(
        seeds::SEED_BATCH_POLICY,
        &[&epoch_id.bytes(), &policy_digest.bytes()],
    );
    let wrong_policy_address = Address::new_from_array([0x9d; 32]);

    let buy = OrderSlot::Single(OrderRecord {
        owner: buy_owner,
        order_id: canonical_order_id(1),
        outcome: 0,
        side: 0,
        quantity: QUANTITY,
        limit: 7_500,
        minimum_fill: 0,
        flags: 0,
        generation: 1,
        expiry_epoch: EPOCH_INDEX,
    });
    let sell = OrderSlot::Single(OrderRecord {
        owner: sell_owner,
        order_id: canonical_order_id(2),
        outcome: u8::from(cross_outcome),
        side: 1,
        quantity: QUANTITY,
        limit: 2_500,
        minimum_fill: 0,
        flags: 0,
        generation: 1,
        expiry_epoch: EPOCH_INDEX,
    });
    let page_index = 0u16.to_le_bytes();
    let (page_address, page_bump) = pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &page_index]);
    let mut page = vec![0; account_len::ORDER_PAGE];
    stream::init_page(&mut page, market, epoch_id, 0, 1, page_bump).unwrap();
    stream::append_slot(&mut page, buy).unwrap();
    stream::append_slot(&mut page, sell).unwrap();

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
        terms.terms,
        policy_digest,
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
        terms.terms,
        policy_digest,
        0,
        sell.generation(),
        0,
        sell_reservation_bump,
        sell_plan,
    )
    .unwrap();

    let (buyer_position_address, buyer_position_bump) =
        pda(seeds::SEED_POSITION, &[&market.bytes(), &buy_owner.bytes()]);
    let buyer_position = PositionAccount {
        market,
        owner: buy_owner,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 100,
        reserved_cash_atoms: buy_plan.cash_atoms,
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

    let epoch_index = EPOCH_INDEX.to_le_bytes();
    let (epoch_address, _) = pda(seeds::SEED_EPOCH, &[&market.bytes(), &epoch_index]);
    let (window_address, _) = pda(seeds::SEED_DIRECT_WINDOW, &[&epoch_id.bytes()]);
    let candidate_ids = CANDIDATE_PRICES.map(|price| candidate_identity(epoch_id, market, price));
    let candidates = candidate_ids.map(|candidate_id| {
        pda(
            seeds::SEED_DIRECT_CANDIDATE,
            &[&epoch_id.bytes(), &candidate_id.0],
        )
        .0
    });
    let selected_id = candidate_ids[3];
    let (receipt_address, _) = pda(
        seeds::SEED_DIRECT_RECEIPT,
        &[&epoch_id.bytes(), &selected_id.0, &0u16.to_le_bytes()],
    );
    let (pot_address, _) = pda(seeds::SEED_DIRECT_POT, &[&epoch_id.bytes(), &selected_id.0]);

    let low_funder = Keypair::new();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    add_state(
        &mut test,
        market_address,
        encode(account_len::MARKET, |out| market_state.encode(out)),
    );
    add_state(
        &mut test,
        terms_address,
        encode(account_len::TERMS, |out| terms.encode(out)),
    );
    add_state(
        &mut test,
        grid_address,
        encode(account_len::PRICE_GRID, |out| grid.encode(out)),
    );
    add_state(&mut test, policy_address, policy_bytes.clone());
    add_state(&mut test, wrong_policy_address, policy_bytes);
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
    for address in [epoch_address, window_address, receipt_address, pot_address]
        .into_iter()
        .chain(candidates)
    {
        test.add_account(address, system_slot(1));
    }
    test.add_account(
        low_funder.pubkey(),
        system_slot(rent_exempt(account_len::SETTLEMENT_RECEIPT) + 100),
    );

    let fixture = Fixture {
        market,
        epoch_id,
        market_account: market_address,
        terms: terms_address,
        grid: grid_address,
        policy: policy_address,
        wrong_policy: wrong_policy_address,
        epoch: epoch_address,
        page: page_address,
        buy_reservation: buy_reservation_address,
        sell_reservation: sell_reservation_address,
        window: window_address,
        candidates,
        candidate_ids,
        receipt: receipt_address,
        pot: pot_address,
    };
    (test.start_with_context().await, low_funder, fixture)
}

async fn send(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    extra_signer: Option<&Keypair>,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT),
        Vec::new(),
    );
    let mut signers = vec![&context.payer];
    if let Some(signer) = extra_signer {
        signers.push(signer);
    }
    let transaction = Transaction::new_signed_with_payer(
        &[budget, instruction],
        Some(&context.payer.pubkey()),
        &signers,
        blockhash,
    );
    let outcome = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap();
    let units = outcome
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    (outcome.result, units)
}

async fn account(context: &mut ProgramTestContext, address: Address) -> Option<Account> {
    context.banks_client.get_account(address).await.unwrap()
}

async fn bytes(context: &mut ProgramTestContext, address: Address) -> Vec<u8> {
    account(context, address)
        .await
        .expect("account exists")
        .data
}

async fn snapshot(context: &mut ProgramTestContext, addresses: &[Address]) -> Vec<Option<Account>> {
    let mut out = Vec::with_capacity(addresses.len());
    for address in addresses {
        out.push(account(context, *address).await);
    }
    out
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn direct_selection_v2_is_a_measured_select_stop_with_rollback() {
    let (mut context, low_funder, fixture) = start(false).await;
    let payer = context.payer.pubkey();

    let epoch_before = snapshot(&mut context, &[fixture.epoch]).await;
    let bad_init = send(
        &mut context,
        fixture.init(payer, fixture.wrong_policy),
        None,
    )
    .await;
    assert_eq!(custom(bad_init.0), ClutchError::WrongPda as u32);
    assert_eq!(snapshot(&mut context, &[fixture.epoch]).await, epoch_before);

    let (result, init_cu) = send(&mut context, fixture.init(payer, fixture.policy), None).await;
    result.unwrap();
    let epoch_account = account(&mut context, fixture.epoch).await.unwrap();
    assert_eq!(epoch_account.owner, PROGRAM_ID);
    assert_eq!(epoch_account.lamports, rent_exempt(DIRECT_EPOCH_BYTES));
    assert_eq!(
        DirectEpochV3Account::decode(&epoch_account.data)
            .unwrap()
            .common
            .phase,
        EPOCH_PHASE_OPEN
    );

    let (result, freeze_cu) = send(&mut context, fixture.freeze(), None).await;
    result.unwrap();
    assert_eq!(
        DirectEpochV3Account::decode(&bytes(&mut context, fixture.epoch).await)
            .unwrap()
            .common
            .phase,
        EPOCH_PHASE_FROZEN
    );

    let frozen = DirectEpochV3Account::decode(&bytes(&mut context, fixture.epoch).await).unwrap();
    let domain = FullRelationDomainV1 {
        relation_version: frozen.common.relation_version,
        market_id: Identity32V1(frozen.common.market.bytes()),
        book_id: Identity32V1(frozen.common.book.bytes()),
        epoch_id: Identity32V1(frozen.common.epoch.bytes()),
        policy_id: Identity32V1(frozen.common.policy.bytes()),
        order_set_id: Identity32V1(frozen.common.order_set.bytes()),
        epoch_index: frozen.common.epoch_index,
        outcome_count: frozen.common.outcome_count,
        owner_count: frozen.common.owner_count,
        price_scale: frozen.common.price_scale,
        remainder_seed: frozen.common.remainder_seed,
        policy: DIRECT_POLICY_V1,
    };
    let (candidate_address, candidate_bump) = pda(
        seeds::SEED_DIRECT_CANDIDATE,
        &[&fixture.epoch_id.bytes(), &fixture.candidate_ids[0].0],
    );
    assert_eq!(candidate_address, fixture.candidates[0]);
    let mut prices = [0; MAX_OUTCOMES];
    prices[0] = CANDIDATE_PRICES[0];
    prices[1] = PRICE_SCALE - CANDIDATE_PRICES[0];
    assert_eq!(
        verify_direct_two_order_candidate(
            &domain,
            DirectTwoOrderInputV1 {
                prices,
                buy_limit: 7_500,
                sell_limit: 2_500,
                quantity: QUANTITY,
                submitted_slot: OPEN_SLOT,
                buy_index: 0,
                sell_index: 1,
                outcome: 0,
                stored_bump: candidate_bump,
            },
        )
        .unwrap()
        .candidate_id,
        fixture.candidate_ids[0]
    );

    context.warp_to_slot(OPEN_SLOT).unwrap();
    let mut max_submit_cu = 0u64;
    for index in 0..CANDIDATE_PRICES.len() {
        let instruction = fixture.submit(&mut context, payer, index).await;
        let (result, units) = send(&mut context, instruction, None).await;
        result.unwrap_or_else(|error| {
            panic!(
                "candidate {index} at price {} refused: {error:?}",
                CANDIDATE_PRICES[index]
            )
        });
        eprintln!(
            "DirectSelectionV2 submit index={index} price={} cu={units}",
            CANDIDATE_PRICES[index]
        );
        max_submit_cu = max_submit_cu.max(units);
        let candidate_account = account(&mut context, fixture.candidates[index])
            .await
            .unwrap();
        assert_eq!(candidate_account.owner, PROGRAM_ID);
        assert_eq!(
            candidate_account.lamports,
            rent_exempt(DIRECT_CANDIDATE_ACCOUNT_BYTES)
        );
    }
    let window_account = account(&mut context, fixture.window).await.unwrap();
    assert_eq!(window_account.owner, PROGRAM_ID);
    assert_eq!(
        window_account.lamports,
        rent_exempt(DIRECT_WINDOW_ACCOUNT_BYTES)
    );
    let window = decode_direct_window(&window_account.data).unwrap();
    assert_eq!(window.admitted_count, CANDIDATE_PRICES.len() as u64);
    assert_eq!(window.top_count, 3);
    assert_eq!(window.top[0].candidate_id, fixture.candidate_ids[3]);
    let rejected =
        decode_direct_candidate(&bytes(&mut context, fixture.candidates[4]).await).unwrap();
    assert_eq!(rejected.status, DIRECT_CANDIDATE_STATUS_SUPERSEDED);

    let replay_watch = fixture.selection_state();
    let replay_before = snapshot(&mut context, &replay_watch).await;
    let replay_ix = fixture.submit(&mut context, payer, 3).await;
    assert!(send(&mut context, replay_ix, None).await.0.is_err());
    assert_eq!(snapshot(&mut context, &replay_watch).await, replay_before);

    let early = fixture
        .select(&mut context, payer, fixture.policy, false)
        .await;
    let early_before = snapshot(&mut context, &replay_watch).await;
    assert_eq!(
        custom(send(&mut context, early, None).await.0),
        ClutchError::NotActive as u32
    );
    assert_eq!(snapshot(&mut context, &replay_watch).await, early_before);

    context.warp_to_slot(CLOSE_SLOT).unwrap();
    let swapped = fixture
        .select(&mut context, payer, fixture.policy, true)
        .await;
    let swapped_before = snapshot(&mut context, &replay_watch).await;
    assert_eq!(
        custom(send(&mut context, swapped, None).await.0),
        ClutchError::MismatchedState as u32
    );
    assert_eq!(snapshot(&mut context, &replay_watch).await, swapped_before);

    let substituted = fixture
        .select(&mut context, payer, fixture.wrong_policy, false)
        .await;
    let substituted_before = snapshot(&mut context, &replay_watch).await;
    assert_eq!(
        custom(send(&mut context, substituted, None).await.0),
        ClutchError::WrongPda as u32
    );
    assert_eq!(
        snapshot(&mut context, &replay_watch).await,
        substituted_before
    );

    let selected_candidate = pda(
        seeds::SEED_DIRECT_CANDIDATE,
        &[&fixture.epoch_id.bytes(), &fixture.candidate_ids[3].0],
    )
    .0;
    let original_candidate = account(&mut context, selected_candidate).await.unwrap();
    let mut corrupted = decode_direct_candidate(&original_candidate.data).unwrap();
    corrupted.weighted_direct_volume += 1;
    let mut corrupted_account = original_candidate.clone();
    corrupted_account.data = encode(DIRECT_CANDIDATE_ACCOUNT_BYTES, |out| {
        encode_direct_candidate(&corrupted, out)
    });
    context.set_account(
        &selected_candidate,
        &AccountSharedData::from(corrupted_account),
    );
    let corrupted_before = snapshot(&mut context, &replay_watch).await;
    let corrupted_select = fixture
        .select(&mut context, payer, fixture.policy, false)
        .await;
    assert_eq!(
        custom(send(&mut context, corrupted_select, None).await.0),
        ClutchError::MismatchedState as u32
    );
    assert_eq!(
        snapshot(&mut context, &replay_watch).await,
        corrupted_before
    );
    context.set_account(
        &selected_candidate,
        &AccountSharedData::from(original_candidate),
    );

    let original_window = account(&mut context, fixture.window).await.unwrap();
    let mut bad_schedule = decode_direct_window(&original_window.data).unwrap();
    bad_schedule.opens_slot += 1;
    let mut bad_window_account = original_window.clone();
    bad_window_account.data = encode(DIRECT_WINDOW_ACCOUNT_BYTES, |out| {
        encode_direct_window(&bad_schedule, out)
    });
    context.set_account(
        &fixture.window,
        &AccountSharedData::from(bad_window_account),
    );
    let bad_schedule_before = snapshot(&mut context, &replay_watch).await;
    let bad_schedule_select = fixture
        .select(&mut context, payer, fixture.policy, false)
        .await;
    assert_eq!(
        custom(send(&mut context, bad_schedule_select, None).await.0),
        ClutchError::MismatchedState as u32
    );
    assert_eq!(
        snapshot(&mut context, &replay_watch).await,
        bad_schedule_before
    );
    context.set_account(&fixture.window, &AccountSharedData::from(original_window));

    let mut rollback_watch = replay_watch.clone();
    rollback_watch.push(low_funder.pubkey());
    let rollback_before = snapshot(&mut context, &rollback_watch).await;
    let low_fund_select = fixture
        .select(&mut context, low_funder.pubkey(), fixture.policy, false)
        .await;
    assert!(send(&mut context, low_fund_select, Some(&low_funder))
        .await
        .0
        .is_err());
    assert_eq!(
        snapshot(&mut context, &rollback_watch).await,
        rollback_before
    );

    // Until the program hashed with the SHA-256 syscall, this select exhausted
    // the whole 1.4m-CU cap and rolled back: the software compression function
    // it carried made complete cached-domain re-execution unaffordable, and
    // that measurement is the stated reason V3 stages verification instead.
    // With the syscall the same instruction now completes with room to spare.
    // This is a *measurement*, not a promotion: nothing here says V2 selection
    // should be called live, and the staged V3 path remains the shipped one.
    // What the assertion holds down is that the cost moved and that the
    // instruction now has an effect instead of stopping mid-flight.
    let select = fixture
        .select(&mut context, payer, fixture.policy, false)
        .await;
    let before_select = snapshot(&mut context, &replay_watch).await;
    let (result, select_cu) = send(&mut context, select, None).await;
    result.expect("V2 select completes once the program hashes with the syscall");
    assert!(
        select_cu < u64::from(CU_LIMIT),
        "select consumed {select_cu} of a {CU_LIMIT} cap"
    );
    assert_ne!(
        snapshot(&mut context, &replay_watch).await,
        before_select,
        "a completed select must have committed the selection it computed"
    );

    for units in [init_cu, freeze_cu, max_submit_cu, select_cu] {
        assert!(units < u64::from(CU_LIMIT));
    }
    eprintln!(
        "DirectSelectionV2 CU init={init_cu} freeze={freeze_cu} submit_max={max_submit_cu} select={select_cu}"
    );
}

#[tokio::test]
async fn cross_outcome_freeze_refuses_without_mutation() {
    let (mut context, _, fixture) = start(true).await;
    let payer = context.payer.pubkey();
    send(&mut context, fixture.init(payer, fixture.policy), None)
        .await
        .0
        .unwrap();
    let watched = [
        fixture.epoch,
        fixture.page,
        fixture.buy_reservation,
        fixture.sell_reservation,
    ];
    let before = snapshot(&mut context, &watched).await;
    assert_eq!(
        custom(send(&mut context, fixture.freeze(), None).await.0),
        ClutchError::MismatchedState as u32
    );
    assert_eq!(snapshot(&mut context, &watched).await, before);
}
