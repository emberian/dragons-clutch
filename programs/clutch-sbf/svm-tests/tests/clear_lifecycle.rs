//! The Tier 2 headline gate (T2-6c): a 3-page, 40-order general book with two
//! portfolio orders and three tombstones, walked on the bank across many
//! transactions with resumption at arbitrary boundaries — order pass 1 with
//! the per-order reservation sweep, the declared pairing-witness slice pass,
//! order pass 2, and the close — with the final on-chain verdict equal to the
//! host relation's verdict on the projected book, in both directions:
//!
//! * an accepted candidate persists `VERIFIED` plus the recomputed
//!   `FullScoreV1` — components equal to `verify_submitted_candidate`'s and
//!   the tie digest equal to `full_relation_candidate_digest` — never the
//!   claimed u128;
//! * a forged fill persists `REFUSED`, with the on-chain checkpoint's
//!   relation error equal to the host verifier's on the same coordinates.
//!
//! Claim plane: SBF-EXECUTED (bank), explicitly PROFILE-ADMITTED: no.

use {
    clutch_batch::relation_v1::{
        canonical_candidate, canonical_pairing, verify_ignoring_claimed_aggregates, BookV1,
        LegRefV1, RelationDomainV1,
    },
    clutch_batch::relation_v1_stream::ClearWorkV1,
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes, complete_submitted_candidate,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1, verify_submitted_candidate,
        FullRelationDomainV1, FullSubmittedCandidateV1, Identity32V1,
    },
    clutch_sbf::{
        error::ClutchError,
        instructions::artifact::CLOCK_SYSVAR_ID,
        instructions::orders_batch::{
            self,
            clear_walk::{
                ADVANCE_CLEAR_SLICES_ACCOUNT_COUNT, ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT,
                COMPLETE_CLEAR_WORK_ACCOUNT_COUNT,
            },
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        clearing::{
            clear_work_body, init_candidate_feed, verify_clear_work, write_fill,
            write_slice_at, CandidateFeedHeader, LegRef, PairingSlice,
            CANDIDATE_FEED_FLAG_SLICES_DECLARED, CLEAR_WORK_STATUS_BOUND,
            CLEAR_WORK_STATUS_COMPLETE,
        },
        projection::{project_slot, OwnerInterner},
        reservation::canonical_reservation_id,
        stream, CandidateRecord, EpochAccount, Hash32, MarketAccount, OrderPageAccount,
        OrderRecord, OrderSlot, PortfolioRecord, PositionAccount, PriceGridAccount,
        CANDIDATE_STATUS_REFUSED, CANDIDATE_STATUS_SUBMITTED, CANDIDATE_STATUS_VERIFIED,
        MAX_GRID_TICKS, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
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
const WALLET: u64 = 5_000_000_000;
const OUTCOMES: u8 = 4;
const PAGES: u16 = 3;
const SLOTS: u64 = 40;

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

/// The T2-5 zero-sentinel domain, verbatim, from the frozen epoch.
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

/// The full-width domain the close recomputes the tie digest under.
fn full_domain(epoch: &EpochAccount) -> FullRelationDomainV1 {
    FullRelationDomainV1 {
        relation_version: epoch.relation_version,
        market_id: Identity32V1(epoch.market.bytes()),
        book_id: Identity32V1(epoch.book.bytes()),
        epoch_id: Identity32V1(epoch.epoch.bytes()),
        policy_id: Identity32V1(epoch.policy.bytes()),
        order_set_id: Identity32V1(epoch.order_set.bytes()),
        epoch_index: epoch.epoch_index,
        outcome_count: epoch.outcome_count,
        owner_count: epoch.owner_count,
        price_scale: epoch.price_scale,
        remainder_seed: epoch.remainder_seed,
        policy: GENERAL_CLEARING_POLICY_V1,
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
    pages: Vec<Address>,
    /// The 40 placements in rank order: `(owner index, slot)`.
    book_plan: Vec<(usize, OrderSlot)>,
    /// Ranks retired after placement, one per page.
    cancels: Vec<u64>,
    owners: Vec<Owner>,
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

    fn candidate_record(&self, candidate: Hash32) -> (Address, u8) {
        pda(
            seeds::SEED_CANDIDATE,
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
            vec![
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
            ],
        )
    }

    fn init_page(&self, payer: Address, page_index: u16) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::InitOrderPage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index,
                    page_count: PAGES,
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

    fn cancel(&self, owner: &Owner, page_index: u16, order_id: Hash32, generation: u64) -> Instruction {
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
                AccountMeta::new(self.pages[page_index as usize], false),
                AccountMeta::new(owner.position, false),
                AccountMeta::new(reservation, false),
            ],
        )
    }

    fn freeze(&self) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(self.epoch_account, false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        for page in &self.pages {
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

    fn advance(
        &self,
        candidate: Hash32,
        page_index: u16,
        max_orders: u16,
        reservations: &[Address],
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate).0, false),
            AccountMeta::new(self.clear_work(candidate).0, false),
            AccountMeta::new_readonly(self.pages[page_index as usize], false),
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

    fn advance_slices(&self, candidate: Hash32, max_slices: u16) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate).0, false),
            AccountMeta::new(self.clear_work(candidate).0, false),
        ];
        assert_eq!(metas.len(), ADVANCE_CLEAR_SLICES_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::AdvanceClearSlices {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                    max_slices,
                },
            ),
            metas,
        )
    }

    fn complete(&self, candidate: Hash32) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate).0, false),
            AccountMeta::new(self.clear_work(candidate).0, false),
            AccountMeta::new(self.candidate_record(candidate).0, false),
        ];
        assert_eq!(metas.len(), COMPLETE_CLEAR_WORK_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::CompleteClearWork {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            metas,
        )
    }
}

/// A four-outcome terms artifact (see `general_epoch.rs` for the shape).
fn general_terms(realm: Hash32, profile: Hash32, feed: Hash32) -> clutch_solana_layout::TermsAccount {
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

fn single(owner: Hash32, rank: u64, outcome: u8, side: u8, quantity: u64, limit: u64) -> OrderSlot {
    OrderSlot::Single(OrderRecord {
        owner,
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

fn portfolio(
    owner: Hash32,
    rank: u64,
    coefficients_head: &[u64],
    lots: u64,
    limit_per_lot: u64,
) -> OrderSlot {
    let mut coefficients = [0u64; MAX_OUTCOMES];
    coefficients[..coefficients_head.len()].copy_from_slice(coefficients_head);
    OrderSlot::Portfolio(PortfolioRecord {
        owner,
        order_id: canonical_order_id(rank),
        side: 0,
        active_len: coefficients_head.len() as u8,
        flags: 0,
        coefficients,
        lots,
        limit_collateral_per_lot: limit_per_lot,
        minimum_fill_lots: 0,
        generation: 1,
        expiry_epoch: EPOCH_INDEX,
    })
}

/// The 40-slot mixed book: six owners, two portfolio buys, buys strict at
/// 5,000 against 2,500 prices, sells marginal at exactly 2,500 (so the
/// pro-rata pools and the canonical dust assignment are live), per-outcome
/// supply strictly above demand.  Owner indexes: 0 alice (buys), 1 bob
/// (sells), 2 carol (sells), 3 dave (buys), 4 erin (portfolio), 5 frank
/// (portfolio).
fn book_plan(owner_ids: &[Hash32]) -> Vec<(usize, OrderSlot)> {
    let a = owner_ids[0];
    let b = owner_ids[1];
    let c = owner_ids[2];
    let d = owner_ids[3];
    let e = owner_ids[4];
    let f = owner_ids[5];
    let mut plan: Vec<(usize, OrderSlot)> = Vec::new();
    let mut rank = 0u64;
    let mut push = |owner_index: usize, build: &dyn Fn(u64) -> OrderSlot| {
        rank += 1;
        plan.push((owner_index, build(rank)));
    };
    // Page 0 (ranks 1..=16).  Rank 7 is retired later.
    push(0, &|r| single(a, r, 0, 0, 10, 5_000));
    push(1, &|r| single(b, r, 0, 1, 15, 2_500));
    push(3, &|r| single(d, r, 0, 0, 5, 5_000));
    push(2, &|r| single(c, r, 0, 1, 9, 2_500));
    push(4, &|r| portfolio(e, r, &[1, 1], 4, 6_000));
    push(0, &|r| single(a, r, 1, 0, 8, 5_000));
    push(0, &|r| single(a, r, 1, 0, 3, 5_000)); // rank 7, retired
    push(1, &|r| single(b, r, 1, 1, 12, 2_500));
    push(3, &|r| single(d, r, 1, 0, 5, 5_000));
    push(2, &|r| single(c, r, 1, 1, 10, 2_500));
    push(5, &|r| portfolio(f, r, &[0, 1, 1, 1], 2, 8_000));
    push(0, &|r| single(a, r, 2, 0, 6, 5_000));
    push(1, &|r| single(b, r, 2, 1, 9, 2_500));
    push(3, &|r| single(d, r, 2, 0, 5, 5_000));
    push(2, &|r| single(c, r, 2, 1, 7, 2_500));
    push(0, &|r| single(a, r, 3, 0, 4, 5_000));
    // Page 1 (ranks 17..=32).  Rank 20 is retired later.
    push(1, &|r| single(b, r, 3, 1, 8, 2_500));
    push(3, &|r| single(d, r, 3, 0, 5, 5_000));
    push(2, &|r| single(c, r, 3, 1, 6, 2_500));
    push(1, &|r| single(b, r, 0, 1, 6, 2_500)); // rank 20, retired
    push(0, &|r| single(a, r, 0, 0, 3, 5_000));
    push(0, &|r| single(a, r, 1, 0, 2, 5_000));
    push(3, &|r| single(d, r, 2, 0, 2, 5_000));
    push(3, &|r| single(d, r, 3, 0, 1, 5_000));
    push(1, &|r| single(b, r, 0, 1, 6, 2_500));
    push(1, &|r| single(b, r, 1, 1, 5, 2_500));
    push(1, &|r| single(b, r, 2, 1, 4, 2_500));
    push(1, &|r| single(b, r, 3, 1, 3, 2_500));
    push(2, &|r| single(c, r, 0, 1, 5, 2_500));
    push(2, &|r| single(c, r, 1, 1, 4, 2_500));
    push(2, &|r| single(c, r, 2, 1, 3, 2_500));
    push(2, &|r| single(c, r, 3, 1, 2, 2_500));
    // Page 2 (ranks 33..=40).  Rank 35 is retired later.
    push(0, &|r| single(a, r, 2, 0, 1, 5_000));
    push(0, &|r| single(a, r, 3, 0, 1, 5_000));
    push(2, &|r| single(c, r, 1, 1, 2, 2_500)); // rank 35, retired
    push(3, &|r| single(d, r, 0, 0, 2, 5_000));
    push(3, &|r| single(d, r, 1, 0, 2, 5_000));
    push(1, &|r| single(b, r, 0, 1, 2, 2_500));
    push(1, &|r| single(b, r, 2, 1, 2, 2_500));
    push(2, &|r| single(c, r, 1, 1, 2, 2_500));
    assert_eq!(plan.len(), SLOTS as usize);
    plan
}

async fn start() -> (ProgramTestContext, Fixture) {
    let realm = h(0x61);
    let profile = h(0x62);
    let feed = h(0x63);
    let market = h(0x39);

    let mut ticks = [0; MAX_GRID_TICKS];
    let mut tick = 0usize;
    while tick <= 10 {
        ticks[tick] = tick as u64 * 500;
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
    let mut pages = Vec::new();
    for index in 0..PAGES {
        pages.push(pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &index.to_le_bytes()]).0);
    }

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
    for _ in 0..6 {
        let key = Keypair::new();
        let id = Hash32::from_bytes(key.pubkey().to_bytes());
        let (position_address, position_bump) =
            pda(seeds::SEED_POSITION, &[&market.bytes(), &id.bytes()]);
        let position = PositionAccount {
            market,
            owner: id,
            generation: 0,
            internal: [1_000; MAX_OUTCOMES],
            cash_atoms: 1_000_000,
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

    let owner_ids: Vec<Hash32> = owners.iter().map(|owner| owner.id).collect();
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
        pages,
        book_plan: book_plan(&owner_ids),
        cancels: vec![7, 20, 35],
        owners,
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

/// Place all 40 orders, retire three (one per page), and freeze at the
/// deadline.
async fn build_frozen_book(context: &mut ProgramTestContext, fixture: &Fixture) {
    let payer = context.payer.pubkey();
    let (result, _) = send(context, &[fixture.init_epoch(payer)], None, 0).await;
    result.unwrap();
    let pages: Vec<Instruction> = (0..PAGES).map(|index| fixture.init_page(payer, index)).collect();
    let (result, _) = send(context, &pages, None, 1).await;
    result.unwrap();

    for (index, (owner_index, slot)) in fixture.book_plan.iter().enumerate() {
        let rank = index as u64 + 1;
        let page_index = ((rank - 1) / MAX_ORDERS_PER_PAGE as u64) as u16;
        let sequence = (rank - 1) % MAX_ORDERS_PER_PAGE as u64;
        let owner = &fixture.owners[*owner_index];
        let (result, _) = send(
            context,
            &[fixture.place(owner, page_index, sequence, *slot)],
            Some(&owner.key),
            100 + index as u32,
        )
        .await;
        result.unwrap();
    }
    for (index, rank) in fixture.cancels.iter().enumerate() {
        let page_index = ((*rank - 1) / MAX_ORDERS_PER_PAGE as u64) as u16;
        let owner_index = fixture.book_plan[*rank as usize - 1].0;
        let owner = &fixture.owners[owner_index];
        let (result, _) = send(
            context,
            &[fixture.cancel(owner, page_index, canonical_order_id(*rank), 2)],
            Some(&owner.key),
            200 + index as u32,
        )
        .await;
        result.unwrap();
    }
    context.warp_to_slot(FREEZE_DEADLINE).unwrap();
    let (result, units) = send(context, &[fixture.freeze()], None, 210).await;
    result.unwrap();
    eprintln!("FreezeEpoch (3 pages, 40 orders) CU: {units}");
}

/// Project the frozen bank pages into the relation's book, exactly as the
/// walk does.
fn project_book(pages: &[Vec<u8>]) -> BookV1 {
    let mut book = BookV1::empty();
    let mut owners = OwnerInterner::new();
    let mut live = 0u16;
    for page in pages {
        let header = stream::OrderPageHeader::decode(page).unwrap();
        let mut cursor = stream::OrderSlotCursor::new(page).unwrap();
        let mut index = 0usize;
        while index < header.order_count as usize {
            let slot = cursor.next_slot().unwrap().unwrap();
            index += 1;
            if let Some(order) = project_slot(&slot, live as u64 + 1, &mut owners).unwrap() {
                book.orders[live as usize] = order;
                live += 1;
            }
        }
    }
    book.len = live as u8;
    book
}

/// Install the solver-written feed and its SUBMITTED candidate record for the
/// given fills and declared witness; T2-7's `SubmitCandidate` is the eventual
/// writer of both.
async fn install_candidate(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    epoch: &EpochAccount,
    prices: [u64; MAX_OUTCOMES],
    fills: &[u64],
    witness: &clutch_batch::relation_v1::PairingWitnessV1,
) -> CandidateFeedHeader {
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
        declared_slices: witness.len,
        distinct_owners: 6,
        order_len: fills.len() as u8,
        outcome_count: OUTCOMES,
        stored_bump: 0,
        flags: CANDIDATE_FEED_FLAG_SLICES_DECLARED,
    };
    header.candidate = header.recomputed_candidate_digest().unwrap();
    let (feed_address, feed_bump) = fixture.candidate_feed(header.candidate);
    header.stored_bump = feed_bump;
    let mut bytes = vec![0u8; account_len::CANDIDATE_FEED];
    init_candidate_feed(&mut bytes, &header).unwrap();
    for (index, fill) in fills.iter().enumerate() {
        write_fill(&mut bytes, index as u8, *fill).unwrap();
    }
    for k in 0..witness.len as usize {
        let slice = witness.slices[k];
        let leg = |leg: LegRefV1| match leg {
            LegRefV1::Order(index) => LegRef::Order(index),
            LegRefV1::Split => LegRef::Split,
            LegRefV1::Merge => LegRef::Merge,
        };
        write_slice_at(
            &mut bytes,
            k as u16,
            &PairingSlice {
                buy_ref: leg(slice.buy_ref),
                sell_ref: leg(slice.sell_ref),
                outcome: slice.outcome,
                quantity: slice.quantity,
            },
        )
        .unwrap();
    }
    context.set_account(&feed_address, &program_account(bytes).into());

    let mut record = CandidateRecord {
        candidate: header.candidate,
        epoch: fixture.epoch_id,
        market: fixture.market,
        prices,
        virtual_split: 0,
        virtual_merge: 0,
        honored_aon_mask: 0,
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        score_digest: Hash32::ZERO,
        churn: 0,
        submitted_slot: FREEZE_DEADLINE,
        distinct_owners: 6,
        order_len: fills.len() as u8,
        outcome_count: OUTCOMES,
        status: CANDIDATE_STATUS_SUBMITTED,
        stored_bump: 0,
        flags: 0,
    };
    let (record_address, record_bump) = fixture.candidate_record(header.candidate);
    record.stored_bump = record_bump;
    context.set_account(
        &record_address,
        &program_account(encode(account_len::CANDIDATE, |out| record.encode(out))).into(),
    );
    header
}

/// Drive one whole order pass on the bank with a cycling batch-size schedule,
/// reading the checkpoint cursor back each iteration and computing the exact
/// pass-1 reservation list the next batch owes.
#[allow(clippy::too_many_arguments)] // one argument per campaign coordinate
async fn drive_order_pass(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    candidate: Hash32,
    pages: &[Vec<u8>],
    batches: &[u16],
    pass_one: bool,
    nonce_base: u32,
    label: &str,
) {
    let work_address = fixture.clear_work(candidate).0;
    let mut step = 0usize;
    loop {
        let work = account(context, work_address).await.unwrap().data;
        let header = verify_clear_work(&work).unwrap();
        if pass_one {
            if header.status == CLEAR_WORK_STATUS_BOUND {
                break;
            }
        } else if header.page_cursor as usize == pages.len() {
            break;
        }
        let max_orders = batches[step % batches.len()];
        let page_index = header.page_cursor;
        let page = OrderPageAccount::decode(&pages[page_index as usize]).unwrap();
        let mut reservations = Vec::new();
        if pass_one {
            let mut live_in_batch = 0u16;
            let mut slot = header.slot_cursor as usize;
            while slot < page.order_count as usize && live_in_batch < max_orders {
                let record = &page.orders[slot];
                if record.is_live() {
                    reservations.push(fixture.reservation(record.owner(), record.order_id()));
                    live_in_batch += 1;
                }
                slot += 1;
            }
        }
        let (result, units) = send_walk(
            context,
            fixture.advance(candidate, page_index, max_orders, &reservations),
            nonce_base + step as u32,
        )
        .await;
        result.unwrap();
        eprintln!(
            "{label} advance page {page_index} slot {} max {max_orders} ({} reservations) CU: {units}",
            header.slot_cursor,
            reservations.len()
        );
        step += 1;
        assert!(step < 64, "the pass must terminate");
    }
}

/// Drive the whole lifecycle for one installed candidate; returns the CU of
/// the completing transaction.
async fn drive_lifecycle(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    candidate: Hash32,
    pages: &[Vec<u8>],
    declared_slices: u16,
) -> u64 {
    // The checkpoint, by the real five-instruction staged creation.
    let payer = context.payer.pubkey();
    let creation = [
        fixture.init_clear_work(payer, candidate),
        fixture.grow_clear_work(candidate, 1),
        fixture.grow_clear_work(candidate, 2),
        fixture.grow_clear_work(candidate, 3),
        fixture.grow_clear_work(candidate, 4),
    ];
    let (result, _) = send(context, &creation, None, 300).await;
    result.unwrap();

    // Pass 1 at deliberately ragged boundaries.
    drive_order_pass(context, fixture, candidate, pages, &[1, 3, 16, 2], true, 310, "pass-1").await;

    // The slice pass, split across two transactions.
    let first = declared_slices / 2 + 1;
    let (result, units) = send_walk(context, fixture.advance_slices(candidate, first), 330).await;
    result.unwrap();
    eprintln!("AdvanceClearSlices ({first} of {declared_slices} slices) CU: {units}");
    let rest = declared_slices; // an over-wide bound: the pass closes itself.
    let (result, units) = send_walk(context, fixture.advance_slices(candidate, rest), 331).await;
    result.unwrap();
    eprintln!("AdvanceClearSlices (rest of {declared_slices} + end_pass) CU: {units}");

    // Pass 2 at different boundaries.
    drive_order_pass(context, fixture, candidate, pages, &[16, 5], false, 340, "pass-2").await;

    // The close.
    let (result, units) = send_walk(context, fixture.complete(candidate), 350).await;
    result.unwrap();
    eprintln!("CompleteClearWork CU: {units}");
    units
}

#[tokio::test]
async fn the_forty_order_book_walks_to_the_host_relations_verdict() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;

    let epoch = EpochAccount::decode(&account(&mut context, fixture.epoch_account).await.unwrap().data)
        .unwrap();
    assert_eq!(epoch.order_count, SLOTS as u16);
    assert_eq!(epoch.page_count, PAGES);
    assert_eq!(epoch.owner_count, 6);
    let mut pages = Vec::new();
    for page in &fixture.pages {
        pages.push(account(&mut context, *page).await.unwrap().data);
    }
    let book = project_book(&pages);
    assert_eq!(book.len, 37);

    // The host relation's own candidate over the projected book: canonical
    // fills and the canonical explicit witness at the fixture price vector.
    let domain = zero_sentinel_domain(&epoch);
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < OUTCOMES as usize {
        prices[i] = PRICE_SCALE / OUTCOMES as u64;
        i += 1;
    }
    let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let witness = canonical_pairing(&domain, &book, &candidate).unwrap();
    assert!(witness.len > 0);
    // Both portfolio orders actually fill: the shape Tier 2 exists to clear.
    assert!(candidate.fills[4] > 0, "erin's portfolio fills");
    // Frank's rank-11 portfolio sits at live index 9: the rank-7 retirement
    // shifts every later rank down one.
    assert!(candidate.fills[9] > 0, "frank's portfolio fills");

    let feed = install_candidate(
        &mut context,
        &fixture,
        &epoch,
        prices,
        &candidate.fills[..book.len as usize],
        &witness,
    )
    .await;
    let candidate_id = feed.candidate;

    drive_lifecycle(&mut context, &fixture, candidate_id, &pages, witness.len).await;

    // The checkpoint completed; its body holds the accepting summary.
    let work = account(&mut context, fixture.clear_work(candidate_id).0)
        .await
        .unwrap()
        .data;
    assert_eq!(
        verify_clear_work(&work).unwrap().status,
        CLEAR_WORK_STATUS_COMPLETE
    );
    let mut body = Box::new(ClearWorkV1::new());
    body.decode_into(clear_work_body(&work).unwrap()).unwrap();
    let streamed = body.verdict().unwrap().copied().unwrap();

    // HOST DIFFERENTIAL 1: the streamed economics equal the host verifier's
    // on the same coordinates (the two obsolete u64-domain digest fields are
    // explicit zero sentinels on the full-width path).
    let full = full_domain(&epoch);
    let raw = FullSubmittedCandidateV1::from_relation_candidate(&full, &candidate).unwrap();
    let (completed, feed_binding) =
        complete_submitted_candidate(&full, &book, &raw, Some(&witness)).unwrap();
    let verified =
        verify_submitted_candidate(&full, &book, &completed, &feed_binding, Some(&witness))
            .unwrap();
    let mut economics = streamed;
    economics.score.digest = 0;
    economics.candidate_digest = 0;
    assert_eq!(economics, verified.economics);

    // HOST DIFFERENTIAL 2: the persisted record is VERIFIED and carries
    // exactly the host's recomputed FullScoreV1 — components and the
    // full-width tie digest, never the claimed u128.
    let record = CandidateRecord::decode(
        &account(&mut context, fixture.candidate_record(candidate_id).0)
            .await
            .unwrap()
            .data,
    )
    .unwrap();
    assert_eq!(record.status, CANDIDATE_STATUS_VERIFIED);
    assert_eq!(record.weighted_direct_volume, verified.score.weighted_direct_volume);
    assert_eq!(
        record.limit_surplus_price_units,
        verified.score.limit_surplus_price_units
    );
    assert_eq!(record.distinct_owners, verified.score.distinct_owners);
    assert_eq!(record.churn, verified.score.churn);
    assert_eq!(record.score_digest.bytes(), verified.score.digest.0);
    assert_ne!(record.score_digest, Hash32::ZERO);
    // The persisted claims were replaced by the verifier's numbers, and the
    // portfolio volume is in them: weighted direct volume counts every
    // filled leg.
    assert!(record.weighted_direct_volume > 0);

    // A second close refuses: the checkpoint is complete and the record has
    // left SUBMITTED.
    let (result, _) = send_walk(&mut context, fixture.complete(candidate_id), 400).await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);
    // So does a further advance of either pass.
    let (result, _) = send_walk(
        &mut context,
        fixture.advance(candidate_id, 0, 1, &[]),
        401,
    )
    .await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);
    let (result, _) = send_walk(&mut context, fixture.advance_slices(candidate_id, 1), 402).await;
    assert_eq!(custom(result), ClutchError::MismatchedState as u32);
}

#[tokio::test]
async fn a_forged_fill_is_refused_by_bank_and_host_with_one_error() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;

    let epoch = EpochAccount::decode(&account(&mut context, fixture.epoch_account).await.unwrap().data)
        .unwrap();
    let mut pages = Vec::new();
    for page in &fixture.pages {
        pages.push(account(&mut context, *page).await.unwrap().data);
    }
    let book = project_book(&pages);
    let domain = zero_sentinel_domain(&epoch);
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < OUTCOMES as usize {
        prices[i] = PRICE_SCALE / OUTCOMES as u64;
        i += 1;
    }
    let honest = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let witness = canonical_pairing(&domain, &book, &honest).unwrap();
    // One forged fill atom: the candidate identity is unchanged (fills are
    // not free coordinates), so this is exactly a solver lying about a fill.
    let mut forged = honest;
    forged.fills[0] += 1;

    let feed = install_candidate(
        &mut context,
        &fixture,
        &epoch,
        prices,
        &forged.fills[..book.len as usize],
        &witness,
    )
    .await;
    let candidate_id = feed.candidate;
    drive_lifecycle(&mut context, &fixture, candidate_id, &pages, witness.len).await;

    // The bank's verdict is the host's refusal, error for error.
    let host = verify_ignoring_claimed_aggregates(&domain, &book, &forged, Some(&witness));
    let relation_error = host.unwrap_err();
    let work = account(&mut context, fixture.clear_work(candidate_id).0)
        .await
        .unwrap()
        .data;
    assert_eq!(
        verify_clear_work(&work).unwrap().status,
        CLEAR_WORK_STATUS_COMPLETE
    );
    let mut body = Box::new(ClearWorkV1::new());
    body.decode_into(clear_work_body(&work).unwrap()).unwrap();
    assert_eq!(body.verdict(), Some(Err(relation_error)));

    // REFUSED persisted; no verified identity minted.
    let record = CandidateRecord::decode(
        &account(&mut context, fixture.candidate_record(candidate_id).0)
            .await
            .unwrap()
            .data,
    )
    .unwrap();
    assert_eq!(record.status, CANDIDATE_STATUS_REFUSED);
    assert_eq!(record.score_digest, Hash32::ZERO);
}
