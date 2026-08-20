//! Real-SBF evidence for candidate window closure and selection (T2-7).
//!
//! `Intent::SubmitCandidate` (tag 54), `Intent::WriteCandidateFeed` (55),
//! `Intent::SealCandidate` (56), and `Intent::FinalizeSelection` (57), driven
//! end to end on the bank against a real frozen general book: submissions
//! ride the staged wire (create → chunked content writes → seal), sealed
//! candidates walk to `VERIFIED` through the real tags 51-53, and selection
//! compares only verified candidates by `FullScoreV1::total_order` over their
//! persisted verified components — with the tie digest re-derived from the
//! presented feed bytes, never trusted as stored.
//!
//! The five gates:
//! * multi-candidate selection — two `VERIFIED` candidates with distinct
//!   scores and one `SUBMITTED`-but-unverified with absurd claims: the
//!   total-order winner is selected, the unverified one is excluded, and
//!   selection refuses to run before the deadline;
//! * the beyond-128-bit tie — two verified candidates with equal score
//!   components whose full-width digests decide, exactly as the research
//!   crate's `full_digest_binds_explicit_witness_and_score_order_uses_all_256_bits`
//!   fixture pins (lower digest wins, decided past the first 16 bytes);
//! * displacement — a fourth candidate displacing the worst retained one
//!   closes the displaced feed in the admitting transaction and supersedes
//!   its record with a zeroed `score_digest`, per the documented rule;
//! * tamper — a forged `score_digest` refuses, a tampered stored fill
//!   refuses (the re-derivation catches both), double-finalize refuses;
//! * the honest lapse — zero verified candidates at the deadline lapse the
//!   epoch to `EPOCH_PHASE_LAPSED` with nothing selected.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.  The reference adapter
//! refuses all four intents with `UnsupportedIntent`; the oracle is the
//! layout codec plus the host relation.

use {
    clutch_batch::relation_v1::{
        canonical_candidate, canonical_pairing, LegRefV1, PairingWitnessV1, RelationDomainV1,
    },
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1, FullScoreV1, Identity32V1,
    },
    clutch_sbf::{
        error::ClutchError,
        instructions::artifact::CLOCK_SYSVAR_ID,
        instructions::orders_batch::{
            clear_walk::{ADVANCE_CLEAR_SLICES_ACCOUNT_COUNT, ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT, COMPLETE_CLEAR_WORK_ACCOUNT_COUNT},
            selection::{
                FINALIZE_SELECTION_FIXED_ACCOUNT_COUNT, SEAL_CANDIDATE_FIXED_ACCOUNT_COUNT,
                SUBMIT_CANDIDATE_ACCOUNT_COUNT, WRITE_CANDIDATE_FEED_ACCOUNT_COUNT,
            },
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        clearing::{
            CandidateFeedHeader, EpochWindowAccount, LegRef, PairingSlice,
            CANDIDATE_FEED_STAGE_TAG, CANDIDATE_WINDOW_SLOTS,
        },
        projection::{project_slot, OwnerInterner},
        reservation::canonical_reservation_id,
        stream, CandidateFeedChunk, CandidateRecord, EpochAccount, Hash32, MarketAccount,
        OrderRecord, OrderSlot, PositionAccount, PriceGridAccount, CANDIDATE_STATUS_SELECTED,
        CANDIDATE_STATUS_SUBMITTED, CANDIDATE_STATUS_SUPERSEDED, CANDIDATE_STATUS_VERIFIED,
        EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED, FEED_FILLS_PER_CHUNK,
        FEED_SLICES_PER_CHUNK, MAX_GRID_TICKS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
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
    owners: Vec<Owner>,
}

/// One candidate's host-computed coordinates and its account addresses.
struct Submission {
    id: Hash32,
    prices: [u64; MAX_OUTCOMES],
    honored_aon_mask: u64,
    fills: Vec<u64>,
    witness: PairingWitnessV1,
    record: Address,
    feed: Address,
}

impl Fixture {
    fn reservation(&self, owner: Hash32, order_id: Hash32) -> Address {
        let id = canonical_reservation_id(self.market, self.epoch_id, owner, 0, order_id);
        pda(seeds::SEED_RESERVATION, &[&id.bytes()]).0
    }

    fn candidate_feed(&self, candidate: Hash32) -> Address {
        pda(
            seeds::SEED_CANDIDATE_FEED,
            &[&self.epoch_id.bytes(), &candidate.bytes()],
        )
        .0
    }

    fn candidate_record(&self, candidate: Hash32) -> Address {
        pda(
            seeds::SEED_CANDIDATE,
            &[&self.epoch_id.bytes(), &candidate.bytes()],
        )
        .0
    }

    fn clear_work(&self, candidate: Hash32) -> Address {
        pda(
            seeds::SEED_CLEAR_WORK,
            &[&self.epoch_id.bytes(), &candidate.bytes()],
        )
        .0
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
            vec![
                AccountMeta::new(owner.key.pubkey(), true),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(self.grid_account, false),
                AccountMeta::new(self.page, false),
                AccountMeta::new(owner.position, false),
                AccountMeta::new(reservation, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
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
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::FreezeEpoch {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            vec![
                AccountMeta::new(self.epoch_account, false),
                AccountMeta::new(self.window_account, false),
                AccountMeta::new_readonly(clock_address(), false),
                AccountMeta::new(self.page, false),
            ],
        )
    }

    #[allow(clippy::too_many_arguments)] // one argument per wire coordinate
    fn submit(
        &self,
        payer: Address,
        submission: &Submission,
        declared_slices: Option<u16>,
        weighted_direct_volume: i128,
        limit_surplus_price_units: u128,
        distinct_owners: u16,
    ) -> Instruction {
        let metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.window_account, false),
            AccountMeta::new(submission.record, false),
            AccountMeta::new(submission.feed, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        assert_eq!(metas.len(), SUBMIT_CANDIDATE_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::SubmitCandidate {
                    market: self.market,
                    epoch: self.epoch_id,
                    prices: submission.prices,
                    virtual_split: 0,
                    virtual_merge: 0,
                    honored_aon_mask: submission.honored_aon_mask,
                    declared_slices,
                    weighted_direct_volume,
                    limit_surplus_price_units,
                    distinct_owners,
                },
            ),
            metas,
        )
    }

    fn write_chunk(
        &self,
        submission: &Submission,
        sequence: u64,
        chunk: CandidateFeedChunk,
    ) -> Instruction {
        let metas = vec![AccountMeta::new(submission.feed, false)];
        assert_eq!(metas.len(), WRITE_CANDIDATE_FEED_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                clutch_solana_layout::Intent::WriteCandidateFeed {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate: submission.id,
                    chunk,
                },
            ),
            metas,
        )
    }

    fn seal(
        &self,
        submission: &Submission,
        retained: &[Hash32],
        displaced_feed: Option<Address>,
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new(submission.feed, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        assert_eq!(metas.len(), SEAL_CANDIDATE_FIXED_ACCOUNT_COUNT);
        for candidate in retained {
            metas.push(AccountMeta::new(self.candidate_record(*candidate), false));
        }
        if let Some(feed) = displaced_feed {
            metas.push(AccountMeta::new(feed, false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::SealCandidate {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate: submission.id,
                },
            ),
            metas,
        )
    }

    fn finalize(&self, retained: &[Hash32]) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(self.epoch_account, false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        assert_eq!(metas.len(), FINALIZE_SELECTION_FIXED_ACCOUNT_COUNT);
        for candidate in retained {
            metas.push(AccountMeta::new(self.candidate_record(*candidate), false));
            metas.push(AccountMeta::new_readonly(self.candidate_feed(*candidate), false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::FinalizeSelection {
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
                AccountMeta::new(self.clear_work(candidate), false),
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
            vec![AccountMeta::new(self.clear_work(candidate), false)],
        )
    }

    fn advance(
        &self,
        candidate: Hash32,
        max_orders: u16,
        reservations: &[Address],
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new(self.clear_work(candidate), false),
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

    fn advance_slices(&self, candidate: Hash32, max_slices: u16) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new(self.clear_work(candidate), false),
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
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new(self.clear_work(candidate), false),
            AccountMeta::new(self.candidate_record(candidate), false),
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
    let market = h(0x3b);

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
    for _ in 0..4 {
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

/// Place the five-slot book (four live after one retirement), freeze at the
/// deadline.  Crossings on outcomes 0 and 1; outcomes 2 and 3 carry no
/// orders, which is what the tie test's price freedom rides on.
async fn build_frozen_book(context: &mut ProgramTestContext, fixture: &Fixture) {
    let payer = context.payer.pubkey();
    let (result, _) = send(context, &[fixture.init_epoch(payer)], None, 0).await;
    result.unwrap();
    let (result, _) = send(context, &[fixture.init_page(payer)], None, 1).await;
    result.unwrap();

    let a = &fixture.owners[0];
    let b = &fixture.owners[1];
    let c = &fixture.owners[2];
    let d = &fixture.owners[3];
    let orders = [
        (a, fixture.single(a, 1, 0, 0, 10, 5_000)),
        (b, fixture.single(b, 2, 0, 1, 10, 2_500)),
        (c, fixture.single(c, 3, 1, 0, 8, 5_000)),
        (d, fixture.single(d, 4, 1, 1, 8, 2_500)),
        (a, fixture.single(a, 5, 1, 0, 2, 2_500)), // rank 5, retired below
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
        &[fixture.cancel(&fixture.owners[0], canonical_order_id(5), 2)],
        Some(&fixture.owners[0].key),
        20,
    )
    .await;
    result.unwrap();

    context.warp_to_slot(FREEZE_DEADLINE).unwrap();
    let (result, _) = send(context, &[fixture.freeze()], None, 21).await;
    result.unwrap();
}

/// The frozen epoch, the projected host book, and the walk-order reservation
/// list of the four live orders.
async fn frozen_state(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> (EpochAccount, clutch_batch::relation_v1::BookV1, Vec<Address>) {
    let epoch =
        EpochAccount::decode(&account(context, fixture.epoch_account).await.unwrap().data)
            .unwrap();
    assert_eq!(epoch.phase, EPOCH_PHASE_FROZEN);
    let page = account(context, fixture.page).await.unwrap().data;
    let mut book = clutch_batch::relation_v1::BookV1::empty();
    let mut owners = OwnerInterner::new();
    let mut reservations = Vec::new();
    let header = stream::OrderPageHeader::decode(&page).unwrap();
    let mut cursor = stream::OrderSlotCursor::new(&page).unwrap();
    let mut live = 0u16;
    let mut index = 0usize;
    while index < header.order_count as usize {
        let slot = cursor.next_slot().unwrap().unwrap();
        index += 1;
        if let Some(order) = project_slot(&slot, live as u64 + 1, &mut owners).unwrap() {
            book.orders[live as usize] = order;
            reservations.push(fixture.reservation(slot.owner(), slot.order_id()));
            live += 1;
        }
    }
    book.len = live as u8;
    assert_eq!(book.len, 4);
    (epoch, book, reservations)
}

/// Compute one candidate's canonical coordinates host-side.
fn plan_submission(
    fixture: &Fixture,
    epoch: &EpochAccount,
    book: &clutch_batch::relation_v1::BookV1,
    prices: [u64; MAX_OUTCOMES],
) -> Submission {
    let domain = zero_sentinel_domain(epoch);
    let candidate = canonical_candidate(&domain, book, &prices, 0, 0).unwrap();
    let witness = canonical_pairing(&domain, book, &candidate).unwrap();
    // The identity the program will recompute, restated through the layout
    // codec's own preimage so the test's addresses are the program's.
    let mut shell = CandidateFeedHeader {
        candidate: Hash32::ZERO,
        epoch: fixture.epoch_id,
        market: fixture.market,
        order_set: epoch.order_set,
        prices,
        virtual_split: 0,
        virtual_merge: 0,
        honored_aon_mask: candidate.honored_aon_mask,
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        claimed_digest: 0,
        churn: 0,
        declared_slices: 0,
        distinct_owners: 0,
        order_len: book.len,
        outcome_count: OUTCOMES,
        stored_bump: 0,
        flags: 0,
    };
    shell.candidate = shell.recomputed_candidate_digest().unwrap();
    let id = shell.candidate;
    Submission {
        id,
        prices,
        honored_aon_mask: candidate.honored_aon_mask,
        fills: candidate.fills[..book.len as usize].to_vec(),
        witness,
        record: fixture.candidate_record(id),
        feed: fixture.candidate_feed(id),
    }
}

/// Drive one submission through the staged wire: create, chunked content,
/// seal against the current registry.  Returns the CU of the seal.
#[allow(clippy::too_many_arguments)] // one argument per campaign coordinate
async fn submit_seal(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    submission: &Submission,
    claims: (i128, u128, u16),
    retained: &[Hash32],
    displaced_feed: Option<Address>,
    nonce: u32,
) -> (Result<(), TransactionError>, u64) {
    let payer = context.payer.pubkey();
    let (result, units) = send(
        context,
        &[fixture.submit(
            payer,
            submission,
            Some(submission.witness.len),
            claims.0,
            claims.1,
            claims.2,
        )],
        None,
        nonce,
    )
    .await;
    result.unwrap();
    eprintln!("SubmitCandidate CU: {units}");

    // Fills, then slices, at the sequential cursor.
    let mut written = 0u64;
    for chunk_fills in submission.fills.chunks(FEED_FILLS_PER_CHUNK) {
        let mut fills = [0u64; FEED_FILLS_PER_CHUNK];
        fills[..chunk_fills.len()].copy_from_slice(chunk_fills);
        let (result, units) = send(
            context,
            &[fixture.write_chunk(
                submission,
                written,
                CandidateFeedChunk::Fills {
                    count: chunk_fills.len() as u8,
                    fills,
                },
            )],
            None,
            nonce + 1 + written as u32,
        )
        .await;
        result.unwrap();
        eprintln!("WriteCandidateFeed (fills x{}) CU: {units}", chunk_fills.len());
        written += chunk_fills.len() as u64;
    }
    let leg = |leg: LegRefV1| match leg {
        LegRefV1::Order(index) => LegRef::Order(index),
        LegRefV1::Split => LegRef::Split,
        LegRefV1::Merge => LegRef::Merge,
    };
    let all_slices: Vec<PairingSlice> = (0..submission.witness.len as usize)
        .map(|k| {
            let slice = submission.witness.slices[k];
            PairingSlice {
                buy_ref: leg(slice.buy_ref),
                sell_ref: leg(slice.sell_ref),
                outcome: slice.outcome,
                quantity: slice.quantity,
            }
        })
        .collect();
    for chunk_slices in all_slices.chunks(FEED_SLICES_PER_CHUNK) {
        let mut slices = [PairingSlice::PADDING; FEED_SLICES_PER_CHUNK];
        slices[..chunk_slices.len()].copy_from_slice(chunk_slices);
        let (result, units) = send(
            context,
            &[fixture.write_chunk(
                submission,
                written,
                CandidateFeedChunk::Slices {
                    count: chunk_slices.len() as u8,
                    slices,
                },
            )],
            None,
            nonce + 20 + written as u32,
        )
        .await;
        result.unwrap();
        eprintln!(
            "WriteCandidateFeed (slices x{}) CU: {units}",
            chunk_slices.len()
        );
        written += chunk_slices.len() as u64;
    }

    let (result, units) = send(
        context,
        &[fixture.seal(submission, retained, displaced_feed)],
        None,
        nonce + 40,
    )
    .await;
    if result.is_ok() {
        eprintln!("SealCandidate ({} retained) CU: {units}", retained.len());
    }
    (result, units)
}

/// Walk one sealed candidate to its verdict through the real tags 51-53.
async fn walk_to_verdict(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    submission: &Submission,
    reservations: &[Address],
    nonce: u32,
) {
    let payer = context.payer.pubkey();
    let creation = [
        fixture.init_clear_work(payer, submission.id),
        fixture.grow_clear_work(submission.id, 1),
        fixture.grow_clear_work(submission.id, 2),
        fixture.grow_clear_work(submission.id, 3),
        fixture.grow_clear_work(submission.id, 4),
    ];
    let (result, _) = send(context, &creation, None, nonce).await;
    result.unwrap();
    // Pass 1 (with the reservation sweep), slices, pass 2, close.
    let (result, _) = send_walk(
        context,
        fixture.advance(submission.id, 16, reservations),
        nonce + 1,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(
        context,
        fixture.advance_slices(submission.id, submission.witness.len),
        nonce + 2,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(context, fixture.advance(submission.id, 16, &[]), nonce + 3).await;
    result.unwrap();
    let (result, units) = send_walk(context, fixture.complete(submission.id), nonce + 4).await;
    result.unwrap();
    eprintln!("CompleteClearWork CU: {units}");
}

/// One record's stored score as the selection-plane `FullScoreV1`.
fn stored_score(record: &CandidateRecord) -> FullScoreV1 {
    FullScoreV1 {
        weighted_direct_volume: record.weighted_direct_volume,
        limit_surplus_price_units: record.limit_surplus_price_units,
        distinct_owners: record.distinct_owners,
        churn: record.churn,
        digest: Identity32V1(record.score_digest.bytes()),
    }
}

async fn read_record(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    candidate: Hash32,
) -> CandidateRecord {
    CandidateRecord::decode(
        &account(context, fixture.candidate_record(candidate))
            .await
            .unwrap()
            .data,
    )
    .unwrap()
}

fn even_prices() -> [u64; MAX_OUTCOMES] {
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < OUTCOMES as usize {
        prices[i] = PRICE_SCALE / OUTCOMES as u64;
        i += 1;
    }
    prices
}

fn prices_of(head: [u64; 4]) -> [u64; MAX_OUTCOMES] {
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&head);
    prices
}

/// Gate (i): the total-order winner among VERIFIED candidates is selected,
/// the unverified candidate's absurd claims are excluded, selection refuses
/// before the deadline, and the second finalize refuses.
#[tokio::test]
async fn selection_picks_the_total_order_winner_and_excludes_the_unverified() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    // Two economically distinct verified candidates and one absurd claim.
    let alpha = plan_submission(&fixture, &epoch, &book, even_prices());
    let beta = plan_submission(&fixture, &epoch, &book, prices_of([5_000, 2_500, 1_500, 1_000]));
    let gamma = plan_submission(&fixture, &epoch, &book, prices_of([2_500, 5_000, 1_500, 1_000]));

    let (result, _) =
        submit_seal(&mut context, &fixture, &alpha, (0, 0, 0), &[], None, 100).await;
    result.unwrap();
    let (result, _) = submit_seal(
        &mut context,
        &fixture,
        &beta,
        (0, 0, 0),
        &[alpha.id],
        None,
        160,
    )
    .await;
    result.unwrap();
    // The unverified candidate claims a score nothing real could reach: if
    // selection read claims, gamma would win.
    let (result, _) = submit_seal(
        &mut context,
        &fixture,
        &gamma,
        (i128::MAX / 2, u128::MAX / 2, 4),
        &[alpha.id, beta.id],
        None,
        220,
    )
    .await;
    result.unwrap();

    walk_to_verdict(&mut context, &fixture, &alpha, &reservations, 300).await;
    walk_to_verdict(&mut context, &fixture, &beta, &reservations, 320).await;

    let alpha_record = read_record(&mut context, &fixture, alpha.id).await;
    let beta_record = read_record(&mut context, &fixture, beta.id).await;
    assert_eq!(alpha_record.status, CANDIDATE_STATUS_VERIFIED);
    assert_eq!(beta_record.status, CANDIDATE_STATUS_VERIFIED);
    let alpha_score = stored_score(&alpha_record);
    let beta_score = stored_score(&beta_record);
    // Distinct scores, on the components themselves — the gate's shape.
    assert_ne!(
        (
            alpha_score.weighted_direct_volume,
            alpha_score.limit_surplus_price_units
        ),
        (
            beta_score.weighted_direct_volume,
            beta_score.limit_surplus_price_units
        )
    );
    let expected_winner = if alpha_score.is_better_than(&beta_score) {
        alpha.id
    } else {
        beta.id
    };
    let expected_loser = if expected_winner == alpha.id {
        beta.id
    } else {
        alpha.id
    };

    let retained = [alpha.id, beta.id, gamma.id];
    // Before the deadline the whole world refuses selection.
    let (result, _) = send(&mut context, &[fixture.finalize(&retained)], None, 400).await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, units) = send(&mut context, &[fixture.finalize(&retained)], None, 401).await;
    result.unwrap();
    eprintln!("FinalizeSelection (3 retained, 2 verified) CU: {units}");

    // The winner is the total-order best VERIFIED candidate; the unverified
    // claim competed for nothing.
    let winner_record = read_record(&mut context, &fixture, expected_winner).await;
    assert_eq!(winner_record.status, CANDIDATE_STATUS_SELECTED);
    assert_ne!(winner_record.score_digest, Hash32::ZERO);
    let loser_record = read_record(&mut context, &fixture, expected_loser).await;
    assert_eq!(loser_record.status, CANDIDATE_STATUS_VERIFIED);
    let gamma_record = read_record(&mut context, &fixture, gamma.id).await;
    assert_eq!(gamma_record.status, CANDIDATE_STATUS_SUBMITTED);

    let cleared =
        EpochAccount::decode(&account(&mut context, fixture.epoch_account).await.unwrap().data)
            .unwrap();
    assert_eq!(cleared.phase, EPOCH_PHASE_CLEARED);
    let window = EpochWindowAccount::decode(
        &account(&mut context, fixture.window_account).await.unwrap().data,
    )
    .unwrap();
    assert_eq!(window.selected_candidate, expected_winner);
    assert_eq!(
        window.selected_slot,
        FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS
    );

    // The second finalize refuses: the phase left FROZEN exactly once.
    let (result, _) = send(&mut context, &[fixture.finalize(&retained)], None, 402).await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);
    // And the closed window takes no further submission.
    let late = plan_submission(&fixture, &epoch, &book, prices_of([2_500, 3_500, 2_000, 2_000]));
    let payer = context.payer.pubkey();
    let (result, _) = send(
        &mut context,
        &[fixture.submit(payer, &late, None, 0, 0, 0)],
        None,
        403,
    )
    .await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);
}

/// Gate (i), the tie half: two verified candidates with equal score
/// components resolve by the full-width tie digest — smaller digest wins,
/// decided beyond the first 128 bits — exactly as the research crate's
/// fixture pins for `FullScoreV1::total_order`.
#[tokio::test]
async fn a_beyond_128_bit_score_tie_resolves_by_the_full_width_digest() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    // Identical prices on the traded outcomes 0 and 1; the difference lives
    // entirely on the orderless outcomes 2 and 3, so fills — and every score
    // component — are identical while the candidate identities (and with
    // them the full-width tie digests) differ.
    let one = plan_submission(&fixture, &epoch, &book, prices_of([2_500, 2_500, 3_000, 2_000]));
    let two = plan_submission(&fixture, &epoch, &book, prices_of([2_500, 2_500, 2_000, 3_000]));
    assert_eq!(one.fills, two.fills);

    let (result, _) = submit_seal(&mut context, &fixture, &one, (0, 0, 0), &[], None, 100).await;
    result.unwrap();
    let (result, _) =
        submit_seal(&mut context, &fixture, &two, (0, 0, 0), &[one.id], None, 160).await;
    result.unwrap();
    walk_to_verdict(&mut context, &fixture, &one, &reservations, 300).await;
    walk_to_verdict(&mut context, &fixture, &two, &reservations, 320).await;

    let one_record = read_record(&mut context, &fixture, one.id).await;
    let two_record = read_record(&mut context, &fixture, two.id).await;
    assert_eq!(one_record.status, CANDIDATE_STATUS_VERIFIED);
    assert_eq!(two_record.status, CANDIDATE_STATUS_VERIFIED);
    let one_score = stored_score(&one_record);
    let two_score = stored_score(&two_record);
    // The tie is real: all four components equal, only the digests differ.
    assert_eq!(one_score.weighted_direct_volume, two_score.weighted_direct_volume);
    assert_eq!(
        one_score.limit_surplus_price_units,
        two_score.limit_surplus_price_units
    );
    assert_eq!(one_score.distinct_owners, two_score.distinct_owners);
    assert_eq!(one_score.churn, two_score.churn);
    assert_ne!(one_score.digest, two_score.digest);
    // The research fixture's rule: the lexicographically smaller full-width
    // digest wins.
    let expected_winner = if one_score.digest.0 < two_score.digest.0 {
        one.id
    } else {
        two.id
    };
    assert_eq!(
        expected_winner,
        if one_score.is_better_than(&two_score) {
            one.id
        } else {
            two.id
        },
        "total_order and the raw digest comparison must agree on a component tie"
    );
    // And the fixture's depth statement, restated on these very scores: a
    // digest difference *beyond* the first 128 bits still decides the order.
    let mut deep_left = one_score;
    let mut deep_right = one_score;
    deep_left.digest.0[20] = 1;
    deep_right.digest.0[20] = 2;
    assert!(deep_left.is_better_than(&deep_right));

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let retained = [one.id, two.id];
    let (result, units) = send(&mut context, &[fixture.finalize(&retained)], None, 400).await;
    result.unwrap();
    eprintln!("FinalizeSelection (2 verified, digest tie) CU: {units}");

    let winner_record = read_record(&mut context, &fixture, expected_winner).await;
    assert_eq!(winner_record.status, CANDIDATE_STATUS_SELECTED);
    let window = EpochWindowAccount::decode(
        &account(&mut context, fixture.window_account).await.unwrap().data,
    )
    .unwrap();
    assert_eq!(window.selected_candidate, expected_winner);
}

/// Gate (ii): a fourth candidate displacing per the bounded-registry rule
/// closes the displaced feed in the admitting transaction and supersedes the
/// displaced record with a zeroed `score_digest`; a non-competitive fifth
/// refuses without touching the registry.
#[tokio::test]
async fn a_fourth_candidate_displaces_the_worst_and_supersedes_it() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let alpha = plan_submission(&fixture, &epoch, &book, even_prices());
    let beta = plan_submission(&fixture, &epoch, &book, prices_of([5_000, 2_500, 1_500, 1_000]));
    let gamma = plan_submission(&fixture, &epoch, &book, prices_of([2_500, 5_000, 1_500, 1_000]));
    let delta = plan_submission(&fixture, &epoch, &book, prices_of([3_000, 3_000, 2_000, 2_000]));
    let echo = plan_submission(&fixture, &epoch, &book, prices_of([3_500, 2_500, 2_000, 2_000]));

    // Fill the registry: alpha (verified below — the worst by components),
    // beta and gamma retained on large claims.
    let (result, _) =
        submit_seal(&mut context, &fixture, &alpha, (0, 0, 0), &[], None, 100).await;
    result.unwrap();
    let (result, _) = submit_seal(
        &mut context,
        &fixture,
        &beta,
        (1i128 << 100, 0, 4),
        &[alpha.id],
        None,
        160,
    )
    .await;
    result.unwrap();
    let (result, _) = submit_seal(
        &mut context,
        &fixture,
        &gamma,
        (1i128 << 101, 0, 4),
        &[alpha.id, beta.id],
        None,
        220,
    )
    .await;
    result.unwrap();
    walk_to_verdict(&mut context, &fixture, &alpha, &reservations, 300).await;
    let alpha_verified = read_record(&mut context, &fixture, alpha.id).await;
    assert_eq!(alpha_verified.status, CANDIDATE_STATUS_VERIFIED);
    assert_ne!(alpha_verified.score_digest, Hash32::ZERO);
    // The verified score is honest and small; the claims are absurd, so the
    // verified candidate is the worst retained by components.
    assert!(alpha_verified.weighted_direct_volume < 1i128 << 100);

    let alpha_record_lamports_before = account(&mut context, alpha.record).await.unwrap().lamports;
    let alpha_feed_lamports = account(&mut context, fixture.candidate_feed(alpha.id))
        .await
        .unwrap()
        .lamports;

    // The fourth candidate displaces the worst: its claim beats alpha's
    // verified components.
    let (result, units) = submit_seal(
        &mut context,
        &fixture,
        &delta,
        (1i128 << 102, 0, 4),
        &[alpha.id, beta.id, gamma.id],
        Some(fixture.candidate_feed(alpha.id)),
        340,
    )
    .await;
    result.unwrap();
    eprintln!("SealCandidate (displacing) CU: {units}");

    // The displaced record: SUPERSEDED, verified digest zeroed — the
    // documented rule — and carrying its closed feed's lamports.
    let superseded = read_record(&mut context, &fixture, alpha.id).await;
    assert_eq!(superseded.status, CANDIDATE_STATUS_SUPERSEDED);
    assert_eq!(superseded.score_digest, Hash32::ZERO);
    assert!(account(&mut context, fixture.candidate_feed(alpha.id))
        .await
        .is_none());
    let alpha_record_lamports_after = account(&mut context, alpha.record).await.unwrap().lamports;
    assert_eq!(
        alpha_record_lamports_after,
        alpha_record_lamports_before + alpha_feed_lamports
    );
    let window = EpochWindowAccount::decode(
        &account(&mut context, fixture.window_account).await.unwrap().data,
    )
    .unwrap();
    assert_eq!(window.retained_count, 3);
    assert!(window.retained.contains(&delta.id));
    assert!(!window.retained.contains(&alpha.id));

    // A non-competitive fifth refuses with its own code; the registry and
    // the would-be displaced feed stand untouched.
    let worst_feed = fixture.candidate_feed(beta.id);
    let (result, _) = submit_seal(
        &mut context,
        &fixture,
        &echo,
        (-1, 0, 0),
        &[window.retained[0], window.retained[1], window.retained[2]],
        Some(worst_feed),
        420,
    )
    .await;
    assert_eq!(custom(result), ClutchError::CandidateNotCompetitive as u32);
    assert!(account(&mut context, worst_feed).await.is_some());
    let after = EpochWindowAccount::decode(
        &account(&mut context, fixture.window_account).await.unwrap().data,
    )
    .unwrap();
    assert_eq!(after.retained, window.retained);
    // The refused pair still exists, staged: the seal may retry later.
    let echo_feed = account(&mut context, fixture.candidate_feed(echo.id))
        .await
        .unwrap();
    assert_eq!(echo_feed.data[0], CANDIDATE_FEED_STAGE_TAG);
}

/// Gate (iii), the tamper half: a forged stored digest refuses, a tampered
/// stored fill region refuses (re-derivation catches both), and the honest
/// state finalizes once both forgeries are reverted.
#[tokio::test]
async fn tampered_digests_and_stored_regions_refuse_selection() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let solo = plan_submission(&fixture, &epoch, &book, even_prices());
    let (result, _) = submit_seal(&mut context, &fixture, &solo, (0, 0, 0), &[], None, 100).await;
    result.unwrap();
    walk_to_verdict(&mut context, &fixture, &solo, &reservations, 300).await;
    assert_eq!(
        read_record(&mut context, &fixture, solo.id).await.status,
        CANDIDATE_STATUS_VERIFIED
    );
    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let retained = [solo.id];

    // (a) A forged score_digest on the record: still a decodable record —
    // the digest is a free field — but the re-derivation refuses it.
    let honest_record = account(&mut context, solo.record).await.unwrap();
    let mut forged_record = honest_record.clone();
    let mut decoded = CandidateRecord::decode(&honest_record.data).unwrap();
    let mut forged_digest = decoded.score_digest.bytes();
    forged_digest[7] ^= 0x40;
    decoded.score_digest = Hash32::from_bytes(forged_digest);
    forged_record.data = encode(account_len::CANDIDATE, |out| decoded.encode(out));
    context.set_account(&solo.record, &forged_record.clone().into());
    let (result, _) = send(&mut context, &[fixture.finalize(&retained)], None, 400).await;
    assert_eq!(custom(result), ClutchError::ScoreDigestMismatch as u32);
    context.set_account(&solo.record, &honest_record.clone().into());

    // (b) A tampered stored fill: the feed still verifies as a feed — fills
    // are free bytes — but the re-derived digest no longer matches the
    // verified one.
    let honest_feed = account(&mut context, fixture.candidate_feed(solo.id))
        .await
        .unwrap();
    let mut tampered_feed = honest_feed.clone();
    // One atom onto the first stored fill (offset 346 is the fill region).
    let first_fill = u64::from_le_bytes(tampered_feed.data[346..354].try_into().unwrap());
    tampered_feed.data[346..354].copy_from_slice(&(first_fill + 1).to_le_bytes());
    context.set_account(&fixture.candidate_feed(solo.id), &tampered_feed.into());
    let (result, _) = send(&mut context, &[fixture.finalize(&retained)], None, 401).await;
    assert_eq!(custom(result), ClutchError::ScoreDigestMismatch as u32);
    context.set_account(&fixture.candidate_feed(solo.id), &honest_feed.into());

    // With the honest state restored the same selection succeeds.
    let (result, _) = send(&mut context, &[fixture.finalize(&retained)], None, 402).await;
    result.unwrap();
    assert_eq!(
        read_record(&mut context, &fixture, solo.id).await.status,
        CANDIDATE_STATUS_SELECTED
    );
}

/// Gate (iii), the lapse half: zero verified candidates at the deadline
/// lapse the epoch honestly — `EPOCH_PHASE_LAPSED`, nothing selected,
/// records standing — and the second finalize refuses.
#[tokio::test]
async fn an_empty_verified_set_lapses_honestly() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let (epoch, book, _) = frozen_state(&mut context, &fixture).await;

    // One sealed-but-never-walked candidate: SUBMITTED is not VERIFIED.
    let idle = plan_submission(&fixture, &epoch, &book, even_prices());
    let (result, _) = submit_seal(&mut context, &fixture, &idle, (7, 7, 4), &[], None, 100).await;
    result.unwrap();

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let retained = [idle.id];
    let (result, units) = send(&mut context, &[fixture.finalize(&retained)], None, 400).await;
    result.unwrap();
    eprintln!("FinalizeSelection (lapse, 0 verified) CU: {units}");

    let lapsed =
        EpochAccount::decode(&account(&mut context, fixture.epoch_account).await.unwrap().data)
            .unwrap();
    assert_eq!(lapsed.phase, EPOCH_PHASE_LAPSED);
    let window = EpochWindowAccount::decode(
        &account(&mut context, fixture.window_account).await.unwrap().data,
    )
    .unwrap();
    assert_eq!(window.selected_candidate, Hash32::ZERO);
    assert_eq!(window.selected_slot, 0);
    // The unverified record stands exactly as it was: nothing was invented.
    assert_eq!(
        read_record(&mut context, &fixture, idle.id).await.status,
        CANDIDATE_STATUS_SUBMITTED
    );
    // The lapse is terminal for this join: no second finalize, no late walk
    // (the walk requires FROZEN), no late submission.
    let (result, _) = send(&mut context, &[fixture.finalize(&retained)], None, 401).await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);
    let payer = context.payer.pubkey();
    let late = plan_submission(&fixture, &epoch, &book, prices_of([2_500, 3_500, 2_000, 2_000]));
    let (result, _) = send(
        &mut context,
        &[fixture.submit(payer, &late, None, 0, 0, 0)],
        None,
        402,
    )
    .await;
    assert_eq!(custom(result), ClutchError::NotActive as u32);
}
