//! Real-SBF evidence for the general epoch lifecycle (T2-6a).
//!
//! `Intent::InitEpoch` (tag 49) creates the general `EpochAccount` and its
//! `EpochWindowAccount` deadline companion on the bank; the general
//! `PlaceOrder` arm admits single **and** portfolio orders against it;
//! `CancelOrder` retires one; and `Intent::FreezeEpoch` (tag 50) seals the
//! complete page set at/after the window deadline, stamping the recomputed
//! set commitment and the exact interned distinct-owner count.
//!
//! The oracle is the layout codec byte-for-byte (the reference adapter
//! refuses both intents as `UnsupportedIntent`, per the genesis precedent):
//! every expected account image below is produced by
//! `clutch_solana_layout`'s own encoders and compared against bank bytes.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

use {
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes, direct_window_v1::DIRECT_POLICY_V1,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
    },
    clutch_sbf::{
        error::ClutchError,
        instructions::artifact::CLOCK_SYSVAR_ID,
        instructions::orders_batch::{
            self,
            general_epoch::{FREEZE_EPOCH_FIXED_ACCOUNT_COUNT, INIT_EPOCH_ACCOUNT_COUNT},
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        clearing::{
            open_general_epoch, EpochWindowAccount, CANDIDATE_WINDOW_SLOTS,
            EPOCH_WINDOW_ACCOUNT_BYTES, MAX_RETAINED_CANDIDATES,
        },
        reservation::canonical_reservation_id,
        stream, EpochAccount, Hash32, MarketAccount, OrderPageAccount, OrderRecord, OrderSlot,
        PortfolioRecord, PositionAccount, PriceGridAccount, TermsAccount, EPOCH_PHASE_FROZEN,
        EPOCH_PHASE_OPEN, MAX_GRID_TICKS, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, MAX_PAYOUTS,
        PAYOUT_MAP_UNUSED,
    },
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_terms, layout_request, COMPUTE_BUDGET, PROGRAM_ID,
        RENT_SYSVAR, SYSTEM_PROGRAM,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const PRICE_SCALE: u64 = 10_000;
const EPOCH_INDEX: u64 = 7;
const FREEZE_DEADLINE: u64 = 500;
const CU_LIMIT: u32 = 1_400_000;
const WALLET: u64 = 1_000_000_000;
const OUTCOMES: u8 = 4;

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

fn clock_address() -> Address {
    Address::new_from_array(CLOCK_SYSVAR_ID.to_bytes())
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

/// A four-outcome terms artifact: the fixture terms widened to the general
/// plane's width, so the epoch this campaign freezes is exactly the shape the
/// direct profile's `== 2` gates refuse.
fn general_terms(realm: Hash32, profile: Hash32, feed: Hash32) -> TermsAccount {
    let mut terms = fixture_terms(realm, profile, feed);
    let mut payouts = [clutch_solana_layout::PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < OUTCOMES as usize {
        let mut weights = [0; MAX_OUTCOMES];
        weights[outcome] = 1;
        payouts[outcome] = clutch_solana_layout::PayoutVectorBytes {
            denominator: 1,
            weights,
        };
        payout_map[outcome] = outcome as u8;
        outcome += 1;
    }
    terms.outcome_count = OUTCOMES;
    terms.payout_count = OUTCOMES;
    terms.payouts = payouts;
    terms.payout_map = payout_map;
    // Degree-0 count rule: `outcome_count - 1` strictly increasing boundaries.
    let mut knots = [0u128; clutch_solana_layout::MAX_KNOTS];
    let mut knot = 0usize;
    while knot < OUTCOMES as usize - 1 {
        knots[knot] = knot as u128 + 1;
        knot += 1;
    }
    terms.knot_count = OUTCOMES - 1;
    terms.knots = knots;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    terms
}

struct Owner {
    key: Keypair,
    id: Hash32,
    position: Address,
}

struct Fixture {
    market: Hash32,
    epoch_id: Hash32,
    policy_digest: Hash32,
    terms_digest: Hash32,
    grid_digest: Hash32,
    market_account: Address,
    terms_account: Address,
    grid_account: Address,
    policy_account: Address,
    epoch_account: Address,
    window_account: Address,
    pages: Vec<Address>,
    alice: Owner,
    bob: Owner,
    carol: Owner,
}

impl Fixture {
    fn reservation(&self, owner: Hash32, order_id: Hash32) -> Address {
        let id = canonical_reservation_id(self.market, self.epoch_id, owner, 0, order_id);
        pda(seeds::SEED_RESERVATION, &[&id.bytes()]).0
    }

    fn init_epoch(&self, payer: Address, policy: Hash32, deadline: u64) -> Instruction {
        let metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.market_account, false),
            AccountMeta::new_readonly(self.terms_account, false),
            AccountMeta::new_readonly(self.grid_account, false),
            AccountMeta::new_readonly(self.policy_account, false),
            AccountMeta::new(self.epoch_account, false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        assert_eq!(metas.len(), INIT_EPOCH_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::InitEpoch {
                    market: self.market,
                    epoch_index: EPOCH_INDEX,
                    policy,
                    freeze_deadline_slot: deadline,
                },
            ),
            metas,
        )
    }

    fn init_page(&self, payer: Address, page_index: u16, page_count: u16) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::InitOrderPage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index,
                    page_count,
                },
            ),
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(self.pages[page_index as usize], false),
                AccountMeta::new_readonly(self.market_account, false),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
        )
    }

    fn place(&self, owner: &Owner, page_index: u16, sequence: u64, slot: OrderSlot) -> Instruction {
        let reservation = self.reservation(slot.owner(), slot.order_id());
        let metas = vec![
            AccountMeta::new(owner.key.pubkey(), true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.grid_account, false),
            AccountMeta::new(self.pages[page_index as usize], false),
            AccountMeta::new(owner.position, false),
            AccountMeta::new(reservation, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), orders_batch::PLACE_ORDER_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                clutch_solana_layout::Intent::PlaceOrder {
                    market: self.market,
                    epoch: self.epoch_id,
                    max_fee_atoms: 0,
                    slot,
                },
            ),
            metas,
        )
    }

    fn cancel(
        &self,
        owner: &Owner,
        page_index: u16,
        order_id: Hash32,
        generation: u64,
    ) -> Instruction {
        let reservation = self.reservation(owner.id, order_id);
        let metas = vec![
            AccountMeta::new_readonly(owner.key.pubkey(), true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(self.pages[page_index as usize], false),
            AccountMeta::new(owner.position, false),
            AccountMeta::new(reservation, false),
        ];
        assert_eq!(metas.len(), orders_batch::CANCEL_ORDER_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                generation,
                clutch_solana_layout::Intent::CancelOrder {
                    market: self.market,
                    epoch: self.epoch_id,
                    owner: owner.id,
                    order_id,
                    generation,
                },
            ),
            metas,
        )
    }

    fn freeze(&self, pages: &[Address]) -> Instruction {
        // The window is writable from the T2-7 revision on: the freeze
        // stamps the candidate-window schedule and the live cardinality.
        let mut metas = vec![
            AccountMeta::new(self.epoch_account, false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        assert_eq!(metas.len(), FREEZE_EPOCH_FIXED_ACCOUNT_COUNT);
        for page in pages {
            metas.push(AccountMeta::new(*page, false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::FreezeEpoch {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            metas,
        )
    }

    fn single(
        &self,
        owner: &Owner,
        rank: u64,
        outcome: u8,
        side: u8,
        quantity: u64,
        limit: u64,
    ) -> OrderSlot {
        OrderSlot::Single(OrderRecord {
            owner: owner.id,
            order_id: canonical_order_id(rank),
            outcome,
            side,
            quantity,
            limit,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: EPOCH_INDEX,
        })
    }

    fn portfolio(&self, owner: &Owner, rank: u64) -> OrderSlot {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[0] = 1;
        coefficients[1] = 1;
        coefficients[2] = 1;
        OrderSlot::Portfolio(PortfolioRecord {
            owner: owner.id,
            order_id: canonical_order_id(rank),
            side: 0,
            active_len: 3,
            flags: 0,
            coefficients,
            lots: 5,
            limit_collateral_per_lot: 9_000,
            minimum_fill_lots: 0,
            generation: 1,
            expiry_epoch: EPOCH_INDEX,
        })
    }
}

async fn start(page_count: usize) -> (ProgramTestContext, Fixture) {
    let realm = h(0x61);
    let profile = h(0x62);
    let feed = h(0x63);
    let market = h(0x35);

    let mut ticks = [0; MAX_GRID_TICKS];
    let mut tick = 0usize;
    while tick <= 10 {
        ticks[tick] = tick as u64 * 1_000;
        tick += 1;
    }
    let mut grid = PriceGridAccount {
        grid: Hash32::ZERO,
        realm,
        price_scale: PRICE_SCALE,
        tick_count: 11,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid.grid = grid.recomputed_grid_id().unwrap();
    let (grid_address, grid_bump) = pda(seeds::SEED_GRID, &[&realm.bytes(), &grid.grid.bytes()]);
    grid.stored_bump = grid_bump;

    let mut terms = general_terms(realm, profile, feed);
    terms.price_grid = grid.grid;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    let (terms_address, terms_bump) =
        pda(seeds::SEED_TERMS, &[&realm.bytes(), &terms.terms.bytes()]);
    terms.stored_bump = terms_bump;

    let (market_address, market_bump) = pda(seeds::SEED_MARKET, &[&realm.bytes(), &market.bytes()]);
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < OUTCOMES as usize {
        outcomes[outcome] = canonical_outcome_id(market, outcome as u8);
        outcome += 1;
    }
    let market_state = MarketAccount {
        market,
        realm,
        profile,
        terms: terms.terms,
        outcome_count: OUTCOMES,
        lifecycle: 0,
        stored_bump: market_bump,
        hoard_bump: 0,
        outcomes,
        feed,
        collateral_cap: terms.collateral_cap,
        created_slot: 0,
        reserved: Hash32::ZERO,
    };

    let epoch_id = canonical_epoch_id(market, EPOCH_INDEX);
    let policy_digest = Hash32::from_bytes(batch_policy_digest(&GENERAL_CLEARING_POLICY_V1).unwrap().0);
    let (policy_address, _) = pda(
        seeds::SEED_BATCH_POLICY,
        &[&epoch_id.bytes(), &policy_digest.bytes()],
    );
    let (epoch_address, _) = pda(
        seeds::SEED_EPOCH,
        &[&market.bytes(), &EPOCH_INDEX.to_le_bytes()],
    );
    let (window_address, _) = pda(
        seeds::SEED_EPOCH_WINDOW,
        &[&market.bytes(), &EPOCH_INDEX.to_le_bytes()],
    );
    let mut pages = Vec::new();
    for index in 0..page_count {
        pages.push(
            pda(
                seeds::SEED_PAGE,
                &[&epoch_id.bytes(), &(index as u16).to_le_bytes()],
            )
            .0,
        );
    }

    let alice = Keypair::new();
    let bob = Keypair::new();
    let carol = Keypair::new();
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
    add_state(
        &mut test,
        policy_address,
        canonical_batch_policy_bytes(&GENERAL_CLEARING_POLICY_V1)
            .unwrap()
            .to_vec(),
    );
    // The wrong-policy fixture: the direct profile's artifact, sealed at its
    // own canonical general-plane address for this epoch.
    let direct_digest = Hash32::from_bytes(batch_policy_digest(&DIRECT_POLICY_V1).unwrap().0);
    let (direct_policy_address, _) = pda(
        seeds::SEED_BATCH_POLICY,
        &[&epoch_id.bytes(), &direct_digest.bytes()],
    );
    add_state(
        &mut test,
        direct_policy_address,
        canonical_batch_policy_bytes(&DIRECT_POLICY_V1).unwrap().to_vec(),
    );

    let mut owners = Vec::new();
    for (key, cash, eggs) in [
        (alice, 60_000u64, 0u64),
        (bob, 0, 30_000),
        (carol, 100_000, 0),
    ] {
        let id = Hash32::from_bytes(key.pubkey().to_bytes());
        let (position_address, position_bump) =
            pda(seeds::SEED_POSITION, &[&market.bytes(), &id.bytes()]);
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[0] = eggs;
        let position = PositionAccount {
            market,
            owner: id,
            generation: 0,
            internal,
            cash_atoms: cash,
            reserved_cash_atoms: 0,
            stored_bump: position_bump,
            close_state: 0,
        };
        add_state(
            &mut test,
            position_address,
            encode(account_len::POSITION, |out| position.encode(out)),
        );
        test.add_account(key.pubkey(), system_slot(WALLET));
        owners.push(Owner {
            key,
            id,
            position: position_address,
        });
    }
    let carol_owner = owners.pop().unwrap();
    let bob_owner = owners.pop().unwrap();
    let alice_owner = owners.pop().unwrap();

    let fixture = Fixture {
        market,
        epoch_id,
        policy_digest,
        terms_digest: terms.terms,
        grid_digest: grid.grid,
        market_account: market_address,
        terms_account: terms_address,
        grid_account: grid_address,
        policy_account: policy_address,
        epoch_account: epoch_address,
        window_account: window_address,
        pages,
        alice: alice_owner,
        bob: bob_owner,
        carol: carol_owner,
    };
    (test.start_with_context().await, fixture)
}

async fn send(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    extra_signer: Option<&Keypair>,
    nonce: u32,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT - nonce),
        Vec::new(),
    );
    let mut all = vec![budget];
    all.extend_from_slice(instructions);
    let mut signers = vec![&context.payer];
    if let Some(signer) = extra_signer {
        signers.push(signer);
    }
    let transaction = Transaction::new_signed_with_payer(
        &all,
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

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn the_general_lifecycle_initializes_places_cancels_and_freezes() {
    let (mut context, fixture) = start(1).await;
    let payer = context.payer.pubkey();

    // Init: both accounts land byte-for-byte as the layout encoders write them.
    let (result, units) = send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.policy_digest, FREEZE_DEADLINE)],
        None,
        0,
    )
    .await;
    result.unwrap();
    eprintln!("InitEpoch CU: {units}");

    let epoch_account = account(&mut context, fixture.epoch_account).await.unwrap();
    let (_, epoch_bump) = pda(
        seeds::SEED_EPOCH,
        &[&fixture.market.bytes(), &EPOCH_INDEX.to_le_bytes()],
    );
    let expected_epoch = open_general_epoch(
        fixture.market,
        fixture.terms_digest,
        fixture.grid_digest,
        fixture.policy_digest,
        EPOCH_INDEX,
        PRICE_SCALE,
        OUTCOMES,
        // `general_terms` is the degree-zero boundary-table shape, and the
        // epoch must carry exactly the degree its terms froze.
        0,
        epoch_bump,
    )
    .unwrap();
    assert_eq!(
        epoch_account.data,
        encode(account_len::EPOCH, |out| expected_epoch.encode(out))
    );
    assert_eq!(expected_epoch.policy, fixture.policy_digest);

    let window_account = account(&mut context, fixture.window_account).await.unwrap();
    let (_, window_bump) = pda(
        seeds::SEED_EPOCH_WINDOW,
        &[&fixture.market.bytes(), &EPOCH_INDEX.to_le_bytes()],
    );
    let expected_window = EpochWindowAccount {
        epoch: fixture.epoch_id,
        market: fixture.market,
        epoch_index: EPOCH_INDEX,
        freeze_deadline_slot: FREEZE_DEADLINE,
        selection_deadline_slot: 0,
        selected_slot: 0,
        selected_candidate: Hash32::ZERO,
        retained: [Hash32::ZERO; MAX_RETAINED_CANDIDATES],
        live_order_count: 0,
        retained_count: 0,
        stored_bump: window_bump,
        flags: 0,
    };
    assert_eq!(
        window_account.data,
        encode(EPOCH_WINDOW_ACCOUNT_BYTES, |out| expected_window.encode(out))
    );

    // Duplicate init refuses on the existing target.
    let (result, _) = send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.policy_digest, FREEZE_DEADLINE)],
        None,
        1,
    )
    .await;
    assert_eq!(custom(result), ClutchError::AlreadyInitialized as u32);

    // A mixed book through the general PlaceOrder arm: a single cross on
    // outcome 0, one three-outcome portfolio buy, and a fourth order that is
    // then cancelled.
    let (result, _) = send(&mut context, &[fixture.init_page(payer, 0, 1)], None, 2).await;
    result.unwrap();

    let buy = fixture.single(&fixture.alice, 1, 0, 0, 10_000, 5_000);
    let sell = fixture.single(&fixture.bob, 2, 0, 1, 10_000, 5_000);
    let portfolio = fixture.portfolio(&fixture.carol, 3);
    let doomed = fixture.single(&fixture.alice, 4, 1, 0, 10, 2_000);

    let (result, units) = send(
        &mut context,
        &[fixture.place(&fixture.alice, 0, 0, buy)],
        Some(&fixture.alice.key),
        3,
    )
    .await;
    result.unwrap();
    eprintln!("PlaceOrder (single, general epoch) CU: {units}");
    let (result, _) = send(
        &mut context,
        &[fixture.place(&fixture.bob, 0, 1, sell)],
        Some(&fixture.bob.key),
        4,
    )
    .await;
    result.unwrap();
    let (result, units) = send(
        &mut context,
        &[fixture.place(&fixture.carol, 0, 2, portfolio)],
        Some(&fixture.carol.key),
        5,
    )
    .await;
    result.unwrap();
    eprintln!("PlaceOrder (portfolio, general epoch) CU: {units}");
    let (result, _) = send(
        &mut context,
        &[fixture.place(&fixture.alice, 0, 3, doomed)],
        Some(&fixture.alice.key),
        6,
    )
    .await;
    result.unwrap();

    // Cancel the fourth order; its retirement is a tombstone in the frozen set.
    let (result, _) = send(
        &mut context,
        &[fixture.cancel(&fixture.alice, 0, canonical_order_id(4), 2)],
        Some(&fixture.alice.key),
        7,
    )
    .await;
    result.unwrap();

    // Early freeze refuses: the deadline is the whole authority.
    let (result, _) = send(
        &mut context,
        &[fixture.freeze(&fixture.pages)],
        None,
        8,
    )
    .await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);

    // The host-side oracle: the same commitment and seal the program must
    // produce, computed from the pre-freeze bank bytes by the layout codec.
    let open_page = account(&mut context, fixture.pages[0]).await.unwrap().data;
    let (expected_order_set, expected_count) =
        stream::frozen_set_commitment(&[&open_page]).unwrap();
    let mut expected_page = open_page.clone();
    stream::seal_page(&mut expected_page, expected_order_set, expected_count).unwrap();

    context.warp_to_slot(FREEZE_DEADLINE).unwrap();
    let (result, units) = send(
        &mut context,
        &[fixture.freeze(&fixture.pages)],
        None,
        9,
    )
    .await;
    result.unwrap();
    eprintln!("FreezeEpoch (1 page, 4 orders) CU: {units}");

    let frozen_page = account(&mut context, fixture.pages[0]).await.unwrap().data;
    assert_eq!(frozen_page, expected_page);

    let frozen = EpochAccount::decode(&account(&mut context, fixture.epoch_account).await.unwrap().data)
        .unwrap();
    assert_eq!(frozen.phase, EPOCH_PHASE_FROZEN);
    assert_eq!(frozen.order_set, expected_order_set);
    assert_eq!(frozen.order_count, expected_count);
    assert_eq!(frozen.order_count, 4);
    assert_eq!(frozen.page_count, 1);
    // Three distinct owners: alice (whose second order retired), bob, carol.
    assert_eq!(frozen.owner_count, 3);
    let page_header = stream::OrderPageHeader::decode(&frozen_page).unwrap();
    assert_eq!(frozen.first_order_id, page_header.first_order_id);
    assert_eq!(frozen.last_order_id, page_header.last_order_id);
    assert_eq!(page_header.tombstone_count, 1);
    // The frozen epoch binds its sealed set: the post-state check, re-run
    // host-side over the exact bank bytes.
    stream::epoch_binds_page_set(&frozen, &[&frozen_page]).unwrap();
    // The freeze opened the candidate window: the schedule is the pinned
    // span from the freeze slot, and the stamped live cardinality is the
    // slot count minus the one retirement.
    let stamped = EpochWindowAccount::decode(
        &account(&mut context, fixture.window_account).await.unwrap().data,
    )
    .unwrap();
    assert_eq!(
        stamped.selection_deadline_slot,
        FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS
    );
    assert_eq!(stamped.live_order_count, 3);
    assert_eq!(stamped.retained_count, 0);
    assert_eq!(stamped.selected_candidate, Hash32::ZERO);

    // Double freeze refuses; the frozen book takes no placement and releases
    // no ACTIVE reservation (cancellation requires OPEN).
    let (result, _) = send(
        &mut context,
        &[fixture.freeze(&fixture.pages)],
        None,
        10,
    )
    .await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);
    let late = fixture.single(&fixture.alice, 5, 0, 0, 10, 2_000);
    let (result, _) = send(
        &mut context,
        &[fixture.place(&fixture.alice, 0, 4, late)],
        Some(&fixture.alice.key),
        11,
    )
    .await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);
    let (result, _) = send(
        &mut context,
        &[fixture.cancel(&fixture.alice, 0, canonical_order_id(1), 3)],
        Some(&fixture.alice.key),
        12,
    )
    .await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);
}

#[tokio::test]
async fn init_refuses_wrong_policy_stale_deadline_and_wrong_window() {
    let (mut context, fixture) = start(1).await;
    let payer = context.payer.pubkey();

    // The direct profile's artifact at its canonical address is not the
    // general profile: the digest re-derives but the policy comparison
    // refuses, so a general epoch can never open under the wrong frozen
    // policy.
    let direct_digest = Hash32::from_bytes(batch_policy_digest(&DIRECT_POLICY_V1).unwrap().0);
    let (direct_policy_address, _) = pda(
        seeds::SEED_BATCH_POLICY,
        &[&fixture.epoch_id.bytes(), &direct_digest.bytes()],
    );
    let mut wrong_policy = fixture.init_epoch(payer, direct_digest, FREEZE_DEADLINE);
    wrong_policy.accounts[4] = AccountMeta::new_readonly(direct_policy_address, false);
    let (result, _) = send(&mut context, &[wrong_policy], None, 0).await;
    assert_eq!(custom(result), ClutchError::AuthorizationUnavailable as u32);

    // A digest the presented artifact does not re-derive refuses the same
    // way (the artifact is the general one, the claimed digest is not its).
    let mut wrong_digest = fixture.init_epoch(payer, direct_digest, FREEZE_DEADLINE);
    wrong_digest.accounts[4] = AccountMeta::new_readonly(fixture.policy_account, false);
    let (result, _) = send(&mut context, &[wrong_digest], None, 1).await;
    assert_eq!(custom(result), ClutchError::AuthorizationUnavailable as u32);

    // A deadline at or behind the clock refuses at creation.
    context.warp_to_slot(FREEZE_DEADLINE + 1).unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.policy_digest, FREEZE_DEADLINE)],
        None,
        2,
    )
    .await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);

    // Nothing was created by any refusal above.
    assert!(account(&mut context, fixture.epoch_account).await.is_none());
    assert!(account(&mut context, fixture.window_account).await.is_none());
}

#[tokio::test]
async fn freeze_refuses_a_missing_page_and_an_expired_record() {
    let (mut context, fixture) = start(2).await;
    let payer = context.payer.pubkey();

    // The epoch creation and the page creations ride separate transactions:
    // a transaction's writability is message-wide, so an epoch made writable
    // for its own creation would arrive writable at `InitOrderPage`, whose
    // role table correctly refuses a writable epoch.
    let (result, _) = send(
        &mut context,
        &[fixture.init_epoch(payer, fixture.policy_digest, FREEZE_DEADLINE)],
        None,
        0,
    )
    .await;
    result.unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.init_page(payer, 0, 2), fixture.init_page(payer, 1, 2)],
        None,
        30,
    )
    .await;
    result.unwrap();

    // A two-page set: page zero must be dense, so alice fills all sixteen
    // slots; bob's one order opens page one.
    for rank in 1..=(MAX_ORDERS_PER_PAGE as u64) {
        let order = fixture.single(&fixture.alice, rank, 0, 0, 10, 2_000);
        let (result, _) = send(
            &mut context,
            &[fixture.place(&fixture.alice, 0, rank - 1, order)],
            Some(&fixture.alice.key),
            rank as u32,
        )
        .await;
        result.unwrap();
    }
    let bob_order = fixture.single(&fixture.bob, MAX_ORDERS_PER_PAGE as u64 + 1, 0, 1, 10, 2_000);
    let (result, _) = send(
        &mut context,
        &[fixture.place(&fixture.bob, 1, 0, bob_order)],
        Some(&fixture.bob.key),
        20,
    )
    .await;
    result.unwrap();

    context.warp_to_slot(FREEZE_DEADLINE).unwrap();

    // A freeze naming only part of the set refuses: every page's stored
    // `page_count` is 2, and one page is not two.
    let (result, _) = send(
        &mut context,
        &[fixture.freeze(&fixture.pages[..1])],
        None,
        21,
    )
    .await;
    assert_eq!(custom(result), 0x1000 + 14, "codec MismatchedBinding");

    // An expired live record refuses the whole freeze: page one is rewritten
    // host-side with a record whose expiry is behind this epoch — a shape no
    // placement can produce — and the post-state binding walk catches it.
    let honest_page_one = account(&mut context, fixture.pages[1]).await.unwrap();
    let mut expired = OrderPageAccount::decode(&honest_page_one.data).unwrap();
    match &mut expired.orders[0] {
        OrderSlot::Single(record) => record.expiry_epoch = EPOCH_INDEX - 1,
        other => panic!("expected bob's single order, found {other:?}"),
    }
    expired.page_digest = expired.recomputed_page_digest().unwrap();
    let mut forged = honest_page_one.clone();
    forged.data = encode(account_len::ORDER_PAGE, |out| expired.encode(out));
    context.set_account(&fixture.pages[1], &forged.into());
    let (result, _) = send(&mut context, &[fixture.freeze(&fixture.pages)], None, 22).await;
    assert_eq!(custom(result), 0x1000 + 14, "codec MismatchedBinding");
    // The refusal rolled the whole freeze back: nothing was sealed.
    let page_zero = account(&mut context, fixture.pages[0]).await.unwrap().data;
    assert_eq!(stream::OrderPageHeader::decode(&page_zero).unwrap().frozen, 0);
    let epoch = EpochAccount::decode(&account(&mut context, fixture.epoch_account).await.unwrap().data)
        .unwrap();
    assert_eq!(epoch.phase, EPOCH_PHASE_OPEN);

    // With the honest page restored, the same freeze succeeds and stamps the
    // exact two-page commitment.
    context.set_account(&fixture.pages[1], &honest_page_one.into());
    let page_one = account(&mut context, fixture.pages[1]).await.unwrap().data;
    let (expected_order_set, expected_count) =
        stream::frozen_set_commitment(&[&page_zero, &page_one]).unwrap();
    let (result, units) = send(&mut context, &[fixture.freeze(&fixture.pages)], None, 23).await;
    result.unwrap();
    eprintln!("FreezeEpoch (2 pages, 17 orders) CU: {units}");
    let frozen = EpochAccount::decode(&account(&mut context, fixture.epoch_account).await.unwrap().data)
        .unwrap();
    assert_eq!(frozen.phase, EPOCH_PHASE_FROZEN);
    assert_eq!(frozen.order_set, expected_order_set);
    assert_eq!(frozen.order_count, expected_count);
    assert_eq!(frozen.order_count, MAX_ORDERS_PER_PAGE as u16 + 1);
    assert_eq!(frozen.page_count, 2);
    assert_eq!(frozen.owner_count, 2);
}
