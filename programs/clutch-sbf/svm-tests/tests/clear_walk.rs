//! Real-SBF evidence for the on-chain streaming walk (T2-6b).
//!
//! `Intent::AdvanceClearWork` (tag 51) drives pass 1 of the checkpoint feed
//! across arbitrary transaction boundaries — digest-verified page walk,
//! tombstone skipping, owner interning persisted in the checkpoint's
//! layout-owned region, per-order ACTIVE-reservation verification, candidate
//! fills by live rank, `begin` with the zero-sentinel domain on the first
//! advance — then binds `(order_set, consumed_fold)` at pass-1 completion and
//! walks pass 2 to the feed's verdict.
//!
//! The oracle is a **host twin**: the same projected book and fills driven
//! through the same `ClearWorkV1` on the host, compared byte-for-byte against
//! the on-chain body at pass-1 end and at completion — plus the tamper
//! battery: substituted page, substituted epoch (`require_continuation`),
//! wholesale checkpoint-body substitution (the anchor comparison over the
//! codec's 29 documented residual tamper regions), substituted feed fills
//! (the fold seal), reservation missing/RELEASED/wrong-plan, the 65th
//! interned owner, and the pass-1 owner-count gate.
//!
//! Claim plane: SBF-EXECUTED (bank), explicitly PROFILE-ADMITTED: no.

use {
    clutch_batch::relation_v1::{ErrorV1, RelationDomainV1, ScoreV1},
    clutch_batch::relation_v1_stream::{ClearWorkV1, FeedStatusV1, StreamCandidateV1},
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
    },
    clutch_sbf::{
        error::ClutchError,
        instructions::artifact::CLOCK_SYSVAR_ID,
        instructions::orders_batch::{
            self,
            clear_walk::ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT,
            general_epoch::{FREEZE_EPOCH_FIXED_ACCOUNT_COUNT, INIT_EPOCH_ACCOUNT_COUNT},
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        clearing::{
            self, clear_work_body, init_candidate_feed, read_owner_interner, verify_clear_work,
            write_fill, CandidateFeedHeader, CLEAR_WORK_STATUS_BOUND, CLEAR_WORK_STATUS_OPEN,
        },
        projection::{project_slot, OwnerInterner},
        reservation::{canonical_reservation_id, ReservationAccount},
        stream, EpochAccount, Hash32, MarketAccount, OrderPageAccount, OrderRecord, OrderSlot,
        PortfolioRecord, PositionAccount, PriceGridAccount, MAX_GRID_TICKS, MAX_OUTCOMES,
        MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
    },
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_terms, layout_request, request_heap_frame_data,
        COMPUTE_BUDGET, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM,
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
const HEAP_FRAME: u32 = 262_144;
const WALLET: u64 = 2_000_000_000;
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

fn program_account(data: Vec<u8>) -> Account {
    Account {
        lamports: rent_exempt(data.len()),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

/// The T2-5 zero-sentinel domain, verbatim, from the frozen epoch — the host
/// half of the differential builds the domain exactly as the program does.
fn zero_sentinel_domain(epoch: &EpochAccount) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: epoch.relation_version,
        market_id: 0,
        book_id: 0,
        epoch: epoch.epoch_index,
        policy_id: 0,
        order_set_id: 0,
        outcome_count: epoch.outcome_count,
        owner_count: epoch.owner_count,
        price_scale: epoch.price_scale,
        remainder_seed: epoch.remainder_seed,
        policy: GENERAL_CLEARING_POLICY_V1,
    }
}

fn stream_candidate_of(feed: &CandidateFeedHeader) -> StreamCandidateV1 {
    StreamCandidateV1 {
        order_len: feed.order_len,
        prices: feed.prices,
        virtual_split: feed.virtual_split,
        virtual_merge: feed.virtual_merge,
        honored_aon_mask: feed.honored_aon_mask,
        claimed_score: ScoreV1::ZERO,
        canonical_candidate_digest: 0,
        declared_slices: feed.declared_slices(),
    }
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
    market_account: Address,
    terms_account: Address,
    grid_account: Address,
    policy_account: Address,
    epoch_account: Address,
    window_account: Address,
    page: Address,
    alice: Owner,
    bob: Owner,
    carol: Owner,
}

impl Fixture {
    fn reservation(&self, owner: Hash32, order_id: Hash32) -> Address {
        let id = canonical_reservation_id(self.market, self.epoch_id, owner, 0, order_id);
        pda(seeds::SEED_RESERVATION, &[&id.bytes()]).0
    }

    fn candidate_feed(&self, candidate: Hash32) -> (Address, u8) {
        pda(
            seeds::SEED_CANDIDATE_FEED,
            &[&self.epoch_id.bytes(), &candidate.bytes()],
        )
    }

    fn clear_work(&self, candidate: Hash32) -> (Address, u8) {
        pda(
            seeds::SEED_CLEAR_WORK,
            &[&self.epoch_id.bytes(), &candidate.bytes()],
        )
    }

    fn init_epoch(&self, payer: Address) -> Instruction {
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
                    policy: self.policy_digest,
                    freeze_deadline_slot: FREEZE_DEADLINE,
                },
            ),
            metas,
        )
    }

    fn init_page(&self, payer: Address) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::InitOrderPage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index: 0,
                    page_count: 1,
                },
            ),
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(self.page, false),
                AccountMeta::new_readonly(self.market_account, false),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
        )
    }

    fn place(&self, owner: &Owner, sequence: u64, slot: OrderSlot) -> Instruction {
        let reservation = self.reservation(slot.owner(), slot.order_id());
        let metas = vec![
            AccountMeta::new(owner.key.pubkey(), true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.grid_account, false),
            AccountMeta::new(self.page, false),
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

    fn cancel(&self, owner: &Owner, order_id: Hash32, generation: u64) -> Instruction {
        let reservation = self.reservation(owner.id, order_id);
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
            vec![
                AccountMeta::new_readonly(owner.key.pubkey(), true),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new(self.page, false),
                AccountMeta::new(owner.position, false),
                AccountMeta::new(reservation, false),
            ],
        )
    }

    fn freeze(&self) -> Instruction {
        let metas = vec![
            AccountMeta::new(self.epoch_account, false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new_readonly(clock_address(), false),
            AccountMeta::new(self.page, false),
        ];
        assert_eq!(metas.len(), FREEZE_EPOCH_FIXED_ACCOUNT_COUNT + 1);
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

    fn init_clear_work(&self, payer: Address, candidate: Hash32) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::InitClearWork {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(self.clear_work(candidate).0, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
        )
    }

    fn grow_clear_work(&self, candidate: Hash32, sequence: u64) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                clutch_solana_layout::Intent::GrowClearWork {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            vec![AccountMeta::new(self.clear_work(candidate).0, false)],
        )
    }

    fn advance(&self, candidate: Hash32, max_orders: u16, reservations: &[Address]) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate).0, false),
            AccountMeta::new(self.clear_work(candidate).0, false),
            AccountMeta::new_readonly(self.page, false),
        ];
        assert_eq!(metas.len(), ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT);
        for reservation in reservations {
            metas.push(AccountMeta::new_readonly(*reservation, false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::AdvanceClearWork {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                    max_orders,
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

/// A four-outcome terms artifact (see `general_epoch.rs` for the shape).
fn general_terms(
    realm: Hash32,
    profile: Hash32,
    feed: Hash32,
) -> clutch_solana_layout::TermsAccount {
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

async fn start() -> (ProgramTestContext, Fixture) {
    let realm = h(0x61);
    let profile = h(0x62);
    let feed = h(0x63);
    let market = h(0x37);

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
    let policy_digest =
        Hash32::from_bytes(batch_policy_digest(&GENERAL_CLEARING_POLICY_V1).unwrap().0);
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
    let (page_address, _) = pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &0u16.to_le_bytes()]);

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

    let mut owners = Vec::new();
    for (key, cash, egg0, egg1) in [
        (Keypair::new(), 60_000u64, 0u64, 0u64),
        (Keypair::new(), 0, 30_000, 100),
        (Keypair::new(), 100_000, 0, 0),
    ] {
        let id = Hash32::from_bytes(key.pubkey().to_bytes());
        let (position_address, position_bump) =
            pda(seeds::SEED_POSITION, &[&market.bytes(), &id.bytes()]);
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[0] = egg0;
        internal[1] = egg1;
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
    let carol = owners.pop().unwrap();
    let bob = owners.pop().unwrap();
    let alice = owners.pop().unwrap();

    let fixture = Fixture {
        market,
        epoch_id,
        policy_digest,
        market_account: market_address,
        terms_account: terms_address,
        grid_account: grid_address,
        policy_account: policy_address,
        epoch_account: epoch_address,
        window_account: window_address,
        page: page_address,
        alice,
        bob,
        carol,
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

/// One walk transaction: heap frame, compute ceiling, one advance.
async fn send_walk(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    nonce: u32,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let heap = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &request_heap_frame_data(HEAP_FRAME),
        Vec::new(),
    );
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT - nonce),
        Vec::new(),
    );
    let transaction = Transaction::new_signed_with_payer(
        &[heap, budget, instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
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

/// Place the five-slot mixed book, cancel one order, freeze at the deadline.
///
/// Slots: alice buy o0 (rank 1), bob sell o0 (rank 2), carol portfolio buy
/// over three outcomes (rank 3), alice buy o1 (rank 4, then retired), bob
/// sell o1 (rank 5).  Four live orders, one tombstone, three distinct owners.
async fn build_frozen_book(context: &mut ProgramTestContext, fixture: &Fixture) {
    let payer = context.payer.pubkey();
    let (result, _) = send(context, &[fixture.init_epoch(payer)], None, 0).await;
    result.unwrap();
    let (result, _) = send(context, &[fixture.init_page(payer)], None, 1).await;
    result.unwrap();

    let orders = [
        (
            &fixture.alice,
            fixture.single(&fixture.alice, 1, 0, 0, 10_000, 5_000),
        ),
        (
            &fixture.bob,
            fixture.single(&fixture.bob, 2, 0, 1, 10_000, 5_000),
        ),
        (&fixture.carol, fixture.portfolio(&fixture.carol, 3)),
        (
            &fixture.alice,
            fixture.single(&fixture.alice, 4, 1, 0, 10, 2_000),
        ),
        (&fixture.bob, fixture.single(&fixture.bob, 5, 1, 1, 10, 0)),
    ];
    for (sequence, (owner, slot)) in orders.into_iter().enumerate() {
        let (result, _) = send(
            context,
            &[fixture.place(owner, sequence as u64, slot)],
            Some(&owner.key),
            10 + sequence as u32,
        )
        .await;
        result.unwrap();
    }
    let (result, _) = send(
        context,
        &[fixture.cancel(&fixture.alice, canonical_order_id(4), 2)],
        Some(&fixture.alice.key),
        20,
    )
    .await;
    result.unwrap();

    context.warp_to_slot(FREEZE_DEADLINE).unwrap();
    let (result, _) = send(context, &[fixture.freeze()], None, 21).await;
    result.unwrap();
}

/// Fabricate the solver-written feed for the frozen set and install it at its
/// canonical PDA; T2-7's `SubmitCandidate` is the eventual writer.
///
/// No pairing witness is declared, which under the frozen `ExplicitSlices`
/// policy makes the eventual verdict a refusal — deliberately: T2-6b's gate
/// is the walk, the binding, and the anchor stack, and a witness-free feed is
/// the one shape whose *both* order passes are reachable before
/// `AdvanceClearSlices` (T2-6c) exists.
async fn install_feed(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    fills: [u64; 4],
) -> CandidateFeedHeader {
    let epoch =
        EpochAccount::decode(&account(context, fixture.epoch_account).await.unwrap().data).unwrap();
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < OUTCOMES as usize {
        prices[i] = PRICE_SCALE / OUTCOMES as u64;
        i += 1;
    }
    let mut header = CandidateFeedHeader {
        candidate: Hash32::ZERO,
        epoch: fixture.epoch_id,
        market: fixture.market,
        order_set: epoch.order_set,
        prices,
        virtual_split: 0,
        virtual_merge: 0,
        honored_aon_mask: 0,
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        claimed_digest: 0,
        churn: 0,
        declared_slices: 0,
        distinct_owners: 3,
        order_len: 4,
        outcome_count: OUTCOMES,
        stored_bump: 0,
        flags: 0,
    };
    header.candidate = header.recomputed_candidate_digest().unwrap();
    let (feed_address, feed_bump) = fixture.candidate_feed(header.candidate);
    header.stored_bump = feed_bump;
    let mut bytes = vec![0u8; account_len::CANDIDATE_FEED];
    init_candidate_feed(&mut bytes, &header).unwrap();
    for (index, fill) in fills.into_iter().enumerate() {
        write_fill(&mut bytes, index as u8, fill).unwrap();
    }
    context.set_account(&feed_address, &program_account(bytes).into());
    header
}

/// Create the checkpoint at its canonical PDA: five real instructions.
async fn create_checkpoint(context: &mut ProgramTestContext, fixture: &Fixture, candidate: Hash32) {
    let payer = context.payer.pubkey();
    let instructions = [
        fixture.init_clear_work(payer, candidate),
        fixture.grow_clear_work(candidate, 1),
        fixture.grow_clear_work(candidate, 2),
        fixture.grow_clear_work(candidate, 3),
        fixture.grow_clear_work(candidate, 4),
    ];
    let (result, _) = send(context, &instructions, None, 30).await;
    result.unwrap();
}

/// The host twin: the exact projection and feed sequence the program runs,
/// against the exact bank bytes, on the host `ClearWorkV1`.
fn drive_host_pass(
    body: &mut ClearWorkV1,
    epoch: &EpochAccount,
    page_bytes: &[u8],
    feed_bytes: &[u8],
    feed: &CandidateFeedHeader,
    owners: &mut OwnerInterner,
    begin: bool,
) -> FeedStatusV1 {
    if begin {
        body.begin(
            &zero_sentinel_domain(epoch),
            &stream_candidate_of(feed),
            false,
        )
        .unwrap();
    }
    let header = stream::OrderPageHeader::decode(page_bytes).unwrap();
    let mut cursor = stream::OrderSlotCursor::new(page_bytes).unwrap();
    let mut live = 0u16;
    let mut index = 0usize;
    while index < header.order_count as usize {
        let slot = cursor.next_slot().unwrap().unwrap();
        index += 1;
        let Some(order) = project_slot(&slot, live as u64 + 1, owners).unwrap() else {
            continue;
        };
        let fill = clearing::fill_at(feed_bytes, feed, live as u8).unwrap();
        live += 1;
        body.push_order(&order, fill).unwrap();
    }
    body.end_pass().unwrap()
}

#[tokio::test]
async fn the_walk_binds_pass_one_and_matches_the_host_twin_byte_for_byte() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let feed = install_feed(&mut context, &fixture, [7, 7, 0, 0]).await;
    let candidate = feed.candidate;
    create_checkpoint(&mut context, &fixture, candidate).await;

    let epoch = EpochAccount::decode(
        &account(&mut context, fixture.epoch_account)
            .await
            .unwrap()
            .data,
    )
    .unwrap();
    assert_eq!(epoch.owner_count, 3);
    assert_eq!(epoch.order_count, 5);
    let page_bytes = account(&mut context, fixture.page).await.unwrap().data;
    let feed_bytes = account(&mut context, fixture.candidate_feed(candidate).0)
        .await
        .unwrap()
        .data;

    // Reservation list per batch, in walk order over live records.
    let res_1 = fixture.reservation(fixture.alice.id, canonical_order_id(1));
    let res_2 = fixture.reservation(fixture.bob.id, canonical_order_id(2));
    let res_3 = fixture.reservation(fixture.carol.id, canonical_order_id(3));
    let res_5 = fixture.reservation(fixture.bob.id, canonical_order_id(5));

    // Pass 1 across three transactions at arbitrary boundaries: 1, 2, then
    // the rest (a tombstone straddles the last batch).
    let (result, units) = send_walk(&mut context, fixture.advance(candidate, 1, &[res_1]), 0).await;
    result.unwrap();
    eprintln!("AdvanceClearWork begin+1 order CU: {units}");
    let work_address = fixture.clear_work(candidate).0;
    let after_one = account(&mut context, work_address).await.unwrap().data;
    let header_one = verify_clear_work(&after_one).unwrap();
    assert_eq!(header_one.status, CLEAR_WORK_STATUS_OPEN);
    assert_eq!(header_one.page_cursor, 0);
    assert_eq!(header_one.slot_cursor, 1);
    assert_eq!(header_one.live_rank, 1);
    assert_eq!(read_owner_interner(&after_one).unwrap().count(), 1);

    let (result, units) = send_walk(
        &mut context,
        fixture.advance(candidate, 2, &[res_2, res_3]),
        1,
    )
    .await;
    result.unwrap();
    eprintln!("AdvanceClearWork 2 orders CU: {units}");
    let after_three = account(&mut context, work_address).await.unwrap().data;
    let header_three = verify_clear_work(&after_three).unwrap();
    assert_eq!(header_three.slot_cursor, 3);
    assert_eq!(header_three.live_rank, 3);
    assert_eq!(read_owner_interner(&after_three).unwrap().count(), 3);

    let (result, units) =
        send_walk(&mut context, fixture.advance(candidate, 16, &[res_5]), 2).await;
    result.unwrap();
    eprintln!(
        "AdvanceClearWork final pass-1 batch (tombstone + 1 order + end_pass + bind) CU: {units}"
    );

    // Pass-1 end: bound to the frozen set, cursor rewound for pass 2 (no
    // witness declared, so the feed proceeds straight to the second order
    // pass), and the interning region carries first-appearance order.
    let bound = account(&mut context, work_address).await.unwrap().data;
    let header_bound = verify_clear_work(&bound).unwrap();
    assert_eq!(header_bound.status, CLEAR_WORK_STATUS_BOUND);
    assert_eq!(header_bound.order_set, epoch.order_set);
    assert_eq!(header_bound.page_cursor, 0);
    assert_eq!(header_bound.slot_cursor, 0);
    assert_eq!(header_bound.live_rank, 0);
    let interned = read_owner_interner(&bound).unwrap();
    assert_eq!(interned.count(), 3);
    assert_eq!(
        interned.owners(),
        &[fixture.alice.id, fixture.bob.id, fixture.carol.id]
    );

    // The host twin, over the same bank bytes: pass 1 sealed, byte for byte.
    let mut twin = Box::new(ClearWorkV1::new());
    let mut twin_owners = OwnerInterner::new();
    let status = drive_host_pass(
        &mut twin,
        &epoch,
        &page_bytes,
        &feed_bytes,
        &feed,
        &mut twin_owners,
        true,
    );
    assert_eq!(status, FeedStatusV1::NeedOrders { pass: 2 });
    assert_eq!(header_bound.consumed_fold, twin.consumed_fold());
    let mut twin_bytes = vec![0u8; ClearWorkV1::ENCODED_BYTES];
    twin.encode_into(&mut twin_bytes).unwrap();
    assert_eq!(clear_work_body(&bound).unwrap(), &twin_bytes[..]);

    // Pass 2 in one batch, to the feed's verdict.
    let (result, units) = send_walk(&mut context, fixture.advance(candidate, 16, &[]), 3).await;
    result.unwrap();
    eprintln!("AdvanceClearWork pass-2 (4 orders + end_pass) CU: {units}");
    let complete = account(&mut context, work_address).await.unwrap().data;
    let header_complete = verify_clear_work(&complete).unwrap();
    assert_eq!(header_complete.status, CLEAR_WORK_STATUS_BOUND);
    assert_eq!(header_complete.page_cursor, epoch.page_count);

    let status = drive_host_pass(
        &mut twin,
        &epoch,
        &page_bytes,
        &feed_bytes,
        &feed,
        &mut twin_owners,
        false,
    );
    assert_eq!(status, FeedStatusV1::Complete);
    twin.encode_into(&mut twin_bytes).unwrap();
    assert_eq!(clear_work_body(&complete).unwrap(), &twin_bytes[..]);
    // The on-chain verdict equals the host twin's, refusal for refusal.  The
    // least-position latch wins: the arbitrary fixture fills put volume on an
    // ineligible order (M07's witness-fill walk), which the ladder reports
    // ahead of the missing explicit witness (M12) this feed also earns.
    let mut decoded = Box::new(ClearWorkV1::new());
    decoded
        .decode_into(clear_work_body(&complete).unwrap())
        .unwrap();
    assert_eq!(decoded.verdict(), Some(Err(ErrorV1::IneligibleFill)));
    assert_eq!(decoded.verdict(), twin.verdict());

    // A further advance on the complete feed refuses: the cursor rests one
    // past the set, so there is no page for the walk to name.
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 1, &[]), 4).await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);

    // The heap-frame surcharge, measured: the identical no-op transfer with
    // and without the request.
    let transfer = solana_system_interface::instruction::transfer(
        &context.payer.pubkey(),
        &fixture.alice.key.pubkey(),
        1,
    );
    let (result, with_heap) = send_walk(&mut context, transfer.clone(), 5).await;
    result.unwrap();
    let (result, without_heap) = send(&mut context, &[transfer], None, 6).await;
    result.unwrap();
    eprintln!(
        "request_heap_frame(262144) surcharge CU: {} (with {} / without {})",
        with_heap - without_heap,
        with_heap,
        without_heap
    );
}

#[tokio::test]
async fn the_reservation_sweep_refuses_missing_released_and_wrong_plan() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let feed = install_feed(&mut context, &fixture, [7, 7, 0, 0]).await;
    let candidate = feed.candidate;
    create_checkpoint(&mut context, &fixture, candidate).await;

    let res_1_address = fixture.reservation(fixture.alice.id, canonical_order_id(1));
    let honest = account(&mut context, res_1_address).await.unwrap();

    // Missing: the batch pushes one live order and presents no reservation.
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 1, &[]), 0).await;
    assert_eq!(custom(result), ClutchError::AccountCount as u32);

    // RELEASED: no program path releases an ACTIVE reservation of a FROZEN
    // epoch (cancellation requires OPEN), so the state is forged off-chain
    // exactly to prove the walk refuses it.
    let released = ReservationAccount::decode(&honest.data)
        .unwrap()
        .released(2)
        .unwrap();
    let mut forged = honest.clone();
    forged.data = encode(
        clutch_solana_layout::reservation::RESERVATION_ACCOUNT_BYTES,
        |out| released.encode(out),
    );
    context.set_account(&res_1_address, &forged.into());
    let (result, _) = send_walk(
        &mut context,
        fixture.advance(candidate, 1, &[res_1_address]),
        1,
    )
    .await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);

    // Wrong plan: an envelope that does not re-derive from the projected
    // record at zero fee (one atom of phantom funding on both sides).
    let mut wrong_plan = ReservationAccount::decode(&honest.data).unwrap();
    wrong_plan.initial_cash_atoms += 1;
    wrong_plan.remaining_cash_atoms += 1;
    let mut forged = honest.clone();
    forged.data = encode(
        clutch_solana_layout::reservation::RESERVATION_ACCOUNT_BYTES,
        |out| wrong_plan.encode(out),
    );
    context.set_account(&res_1_address, &forged.into());
    let (result, _) = send_walk(
        &mut context,
        fixture.advance(candidate, 1, &[res_1_address]),
        2,
    )
    .await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);

    // The honest reservation restored, the same batch lands.
    context.set_account(&res_1_address, &honest.into());
    let (result, _) = send_walk(
        &mut context,
        fixture.advance(candidate, 1, &[res_1_address]),
        3,
    )
    .await;
    result.unwrap();
}

#[tokio::test]
async fn every_tamper_anchor_refuses_with_its_own_code() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let feed = install_feed(&mut context, &fixture, [7, 7, 0, 0]).await;
    let candidate = feed.candidate;
    create_checkpoint(&mut context, &fixture, candidate).await;
    let work_address = fixture.clear_work(candidate).0;
    let feed_address = fixture.candidate_feed(candidate).0;

    let res_1 = fixture.reservation(fixture.alice.id, canonical_order_id(1));
    let res_2 = fixture.reservation(fixture.bob.id, canonical_order_id(2));
    let res_3 = fixture.reservation(fixture.carol.id, canonical_order_id(3));
    let res_5 = fixture.reservation(fixture.bob.id, canonical_order_id(5));

    // Substituted page content: a record byte moved under the stored digest.
    // The walk's `verify_page` recomputes the fold over every slot, so the
    // digest-verified page walk refuses before a single order is projected.
    let honest_page = account(&mut context, fixture.page).await.unwrap();
    let mut swapped = OrderPageAccount::decode(&honest_page.data).unwrap();
    let stored_digest = swapped.page_digest;
    match &mut swapped.orders[0] {
        OrderSlot::Single(record) => record.quantity += 1,
        other => panic!("expected alice's single order, found {other:?}"),
    }
    assert_ne!(swapped.recomputed_page_digest().unwrap(), stored_digest);
    // Encode through the codec (which insists on a coherent digest), then
    // restore the stale stored digest byte-for-byte: the exact shape of a
    // record edit under an unmoved commitment.
    swapped.page_digest = swapped.recomputed_page_digest().unwrap();
    let mut forged = honest_page.clone();
    forged.data = encode(account_len::ORDER_PAGE, |out| swapped.encode(out));
    let digest_at = forged
        .data
        .windows(32)
        .position(|window| window == swapped.page_digest.bytes())
        .unwrap();
    forged.data[digest_at..digest_at + 32].copy_from_slice(&stored_digest.bytes());
    context.set_account(&fixture.page, &forged.into());
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 1, &[res_1]), 0).await;
    assert_eq!(custom(result), 0x1000 + 14, "codec MismatchedBinding");
    context.set_account(&fixture.page, &honest_page.clone().into());

    // One honest batch, then the 65th-owner refusal: the interning region is
    // forged full of 64 alien owners, so the next live record's owner has no
    // tag left to mint.
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 1, &[res_1]), 1).await;
    result.unwrap();
    let honest_work = account(&mut context, work_address).await.unwrap();
    let mut full_table = OwnerInterner::new();
    for byte in 1..=64u8 {
        let mut owner = [0xE0u8; 32];
        owner[31] = byte;
        full_table.intern(Hash32::from_bytes(owner)).unwrap();
    }
    let mut forged = honest_work.clone();
    clearing::write_owner_interner(&mut forged.data, &full_table).unwrap();
    context.set_account(&work_address, &forged.into());
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 1, &[res_2]), 2).await;
    assert_eq!(custom(result), 0x1000 + 4, "codec InvalidCount: 65th owner");
    context.set_account(&work_address, &honest_work.clone().into());

    // Owner-count gate at pass-1 end: an epoch claiming one more distinct
    // owner than the walk interned refuses the bind.
    let honest_epoch = account(&mut context, fixture.epoch_account).await.unwrap();
    let mut wrong_count = EpochAccount::decode(&honest_epoch.data).unwrap();
    wrong_count.owner_count += 1;
    let mut forged = honest_epoch.clone();
    forged.data = encode(account_len::EPOCH, |out| wrong_count.encode(out));
    context.set_account(&fixture.epoch_account, &forged.into());
    let (result, _) = send_walk(
        &mut context,
        fixture.advance(candidate, 16, &[res_2, res_3, res_5]),
        3,
    )
    .await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);
    context.set_account(&fixture.epoch_account, &honest_epoch.clone().into());

    // Complete pass 1 honestly; the checkpoint binds.
    let (result, _) = send_walk(
        &mut context,
        fixture.advance(candidate, 16, &[res_2, res_3, res_5]),
        4,
    )
    .await;
    result.unwrap();
    let bound_work = account(&mut context, work_address).await.unwrap();
    assert_eq!(
        verify_clear_work(&bound_work.data).unwrap().status,
        CLEAR_WORK_STATUS_BOUND
    );

    // require_continuation: a bound resume against an epoch showing a
    // different frozen set refuses before anything is read from the body.
    let mut wrong_set = EpochAccount::decode(&honest_epoch.data).unwrap();
    wrong_set.order_set = h(0x5A);
    // Keep the page-set arithmetic self-consistent so only the set identity
    // moves; the epoch codec has no cross-field digest to catch this, which
    // is exactly why the checkpoint anchors it.
    let mut forged = honest_epoch.clone();
    forged.data = encode(account_len::EPOCH, |out| wrong_set.encode(out));
    context.set_account(&fixture.epoch_account, &forged.into());
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 16, &[]), 5).await;
    assert_eq!(custom(result), 0x1000 + 14, "require_continuation");
    context.set_account(&fixture.epoch_account, &honest_epoch.clone().into());

    // Wholesale body substitution: another *internally consistent* bound
    // checkpoint — a host twin driven over the same book with different
    // fills — decodes cleanly, and only the anchor comparison
    // `body.consumed_fold() == header.consumed_fold` catches it.
    let epoch = EpochAccount::decode(&honest_epoch.data).unwrap();
    let page_bytes = account(&mut context, fixture.page).await.unwrap().data;
    let feed_bytes = account(&mut context, feed_address).await.unwrap().data;
    let mut alien = Box::new(ClearWorkV1::new());
    let mut alien_owners = OwnerInterner::new();
    let mut alien_feed_bytes = feed_bytes.clone();
    write_fill(&mut alien_feed_bytes, 0, 9_999).unwrap();
    let status = drive_host_pass(
        &mut alien,
        &epoch,
        &page_bytes,
        &alien_feed_bytes,
        &feed,
        &mut alien_owners,
        true,
    );
    assert_eq!(status, FeedStatusV1::NeedOrders { pass: 2 });
    let mut substituted = account(&mut context, work_address).await.unwrap();
    alien
        .encode_into(clearing::clear_work_body_mut(&mut substituted.data).unwrap())
        .unwrap();
    context.set_account(&work_address, &substituted.into());
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 16, &[]), 6).await;
    assert_eq!(
        custom(result),
        ClutchError::ResumeFoldMismatch as u32,
        "the anchor comparison"
    );
    context.set_account(&work_address, &bound_work.clone().into());

    // Substituted feed fills between the passes: pass 2 folds the new fill,
    // and the codec's own seal refuses the pass at its end — and because the
    // refusing transaction rolls back, the poison is never persisted.
    let honest_feed = account(&mut context, feed_address).await.unwrap();
    let mut tampered_feed = honest_feed.clone();
    write_fill(&mut tampered_feed.data, 0, 3).unwrap();
    context.set_account(&feed_address, &tampered_feed.into());
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 16, &[]), 7).await;
    assert_eq!(
        custom(result),
        ClutchError::ResumeFoldMismatch as u32,
        "the fold seal"
    );
    context.set_account(&feed_address, &honest_feed.into());
    let untouched = account(&mut context, work_address).await.unwrap();
    assert_eq!(untouched.data, bound_work.data, "no poison persisted");

    // With every anchor restored, pass 2 completes.
    let (result, _) = send_walk(&mut context, fixture.advance(candidate, 16, &[]), 8).await;
    result.unwrap();
}
