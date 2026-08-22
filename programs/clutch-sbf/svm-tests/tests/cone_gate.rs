//! Real-SBF evidence that the moment-cone gate (V1b) is bound on chain.
//!
//! `DUAL_IS_THE_MEASURE.md` §7.4 shows that above degree one the simplex stops
//! being the no-arbitrage body: an interior claim of a degree-two basis peaks
//! strictly below one, so `p = S·e_j` passes the V1 simplex gate while no
//! probability measure has it as a moment vector, and the split-and-sell
//! position against it is executable in the admitted order language.  §7.6
//! derives the exact cone and the finite certified family that refuses such a
//! candidate; `clutch_batch::relation_v1::validate_price_moment_cone` is that
//! family, and it runs only when the caller binds a basis.
//!
//! The seam this file gates is the *binding*: `InitEpoch` copies
//! `TermsAccount::basis_degree` into `EpochAccount::basis_degree`, and
//! `AdvanceClearWork` turns that byte into `BasisDescriptorV1::ClampedUniform`
//! for `begin_with_basis`.  Without it a degree-two market clears ungated, and
//! every existing degree-≤1 suite is byte-identical either way because the
//! stage is the constant true there (Corollary 7.6.7).
//!
//! The market is the smallest one where the gate has teeth: three outcomes at
//! degree two, which is the single-span Bernstein grid, so the implemented
//! family is *exactly* moment-cone membership there (Corollary 7.6.6).  The
//! ceiling of the interior claim is `1/2`, its butterfly weight is `1`, and
//! the Hankel quadric is `p_1² ≤ 4·p_0·p_2`.  At `S = 10_000`:
//!
//! * `[2_500, 5_000, 2_500]` is the atom price vector of the midpoint — it
//!   sits exactly on all three boundaries and is admitted;
//! * `[2_000, 6_000, 2_000]` overprices the interior claim (`6_000 > S/2`) and
//!   is refused, with the arbitrage `1·𝟙 − 2·e_1`: one complete set short two
//!   units of claim one, whose payoff is nonnegative at every resolved value
//!   and whose price at this candidate is `10_000 − 12_000 < 0`.
//!
//! Claim plane: SBF-EXECUTED (bank), explicitly PROFILE-ADMITTED: no.

use {
    clutch_batch::relation_v1::{
        canonical_candidate, canonical_pairing, verify_ignoring_claimed_aggregates_with_basis,
        BasisDegreeV1, BasisDescriptorV1, BookV1, ErrorV1, LegRefV1, PairingWitnessV1,
        RelationDomainV1,
    },
    clutch_batch::relation_v1_stream::{ClearWorkV1, FeedStatusV1},
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
    },
    clutch_sbf::{
        error::codec_code,
        instructions::artifact::CLOCK_SYSVAR_ID,
        instructions::orders_batch::{
            self,
            clear_walk::{
                ADVANCE_CLEAR_SLICES_ACCOUNT_COUNT, ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT,
                COMPLETE_CLEAR_WORK_ACCOUNT_COUNT,
            },
            general_epoch::{FREEZE_EPOCH_FIXED_ACCOUNT_COUNT, INIT_EPOCH_ACCOUNT_COUNT},
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        clearing::{
            clear_work_body, init_candidate_feed, write_fill, write_slice_at, CandidateFeedHeader,
            LegRef, PairingSlice, CANDIDATE_FEED_FLAG_SLICES_DECLARED,
        },
        projection::{project_slot, OwnerInterner},
        reservation::canonical_reservation_id,
        stream, CandidateRecord, CodecError, EpochAccount, Hash32, MarketAccount, OrderPageAccount,
        OrderRecord, OrderSlot, PayoutVectorBytes, PositionAccount, PriceGridAccount,
        CANDIDATE_STATUS_REFUSED, CANDIDATE_STATUS_SUBMITTED, CANDIDATE_STATUS_VERIFIED,
        MAX_GRID_TICKS, MAX_KNOTS, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, MAX_PAYOUTS, MAX_SLICES,
        PAYOUT_MAP_UNUSED,
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
/// Three claims: the single-span grid at degree two, where the implemented
/// family is exactly moment-cone membership.
const OUTCOMES: u8 = 3;
/// The degree the fixture market's immutable terms freeze.
const DEGREE: u8 = 2;
/// `log2` of the uniform knot gap; mandatory at degree ≥ 2.
const SPACING: u8 = 3;

/// The atom price vector of the midpoint: tight on the ceiling, on the
/// butterfly, and on the Hankel quadric, and inside the cone.
const IN_CONE: [u64; 3] = [2_500, 5_000, 2_500];
/// Over the interior claim's ceiling of `S/2`: outside the cone, with the
/// certificate `1·𝟙 − 2·e_1`.
const OUT_OF_CONE: [u64; 3] = [2_000, 6_000, 2_000];

/// Byte offset of `EpochAccount::basis_degree` inside one encoded epoch.
///
/// The trailing four bytes are `basis_degree, phase, stored_bump, flags`; the
/// tests that use this assert the honest byte reads [`DEGREE`] before they
/// move it, so a layout change surfaces as this file's own failure rather
/// than as a silent no-op tamper.
const EPOCH_BASIS_DEGREE_OFFSET: usize = account_len::EPOCH - 4;

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
    F: FnOnce(&mut [u8]) -> Result<usize, CodecError>,
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

/// The basis the program derives from the epoch byte, on the host side.
fn epoch_basis(epoch: &EpochAccount) -> BasisDescriptorV1 {
    BasisDescriptorV1::ClampedUniform(BasisDegreeV1::from_u8(epoch.basis_degree).unwrap())
}

fn price_vector(prices: [u64; 3]) -> [u64; MAX_OUTCOMES] {
    let mut all = [0u64; MAX_OUTCOMES];
    all[..3].copy_from_slice(&prices);
    all
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

/// The book: two crossing single-Egg pairs, on claims 0 and 1, over three
/// distinct owners.  `(owner index, outcome, side, quantity, limit)`, in rank
/// order; side 0 buys, side 1 sells.
const BOOK: [(usize, u8, u8, u64, u64); 4] = [
    (0, 0, 0, 10_000, 4_000),
    (1, 0, 1, 10_000, 2_000),
    (2, 1, 0, 10_000, 7_000),
    (1, 1, 1, 10_000, 4_000),
];

fn order_slot(
    owner: Hash32,
    rank: u64,
    outcome: u8,
    side: u8,
    quantity: u64,
    limit: u64,
) -> OrderSlot {
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

/// A three-outcome, **degree-two** terms artifact.
///
/// The degree-two shape the codec demands (`TermsAccount::validate`): the §2.1
/// count rule `K = n + 1 − d` gives two knots, uniform spacing is mandatory
/// above degree one so the gap is `2^SPACING` and the declaration must name
/// it, and a derived-basis market has no payout map, so every entry is
/// `PAYOUT_MAP_UNUSED`.
fn degree_two_terms(
    realm: Hash32,
    profile: Hash32,
    feed: Hash32,
) -> clutch_solana_layout::TermsAccount {
    let mut terms = fixture_terms(realm, profile, feed);
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut weights = [0u64; MAX_OUTCOMES];
    weights[0] = 8;
    payouts[0] = PayoutVectorBytes {
        denominator: 8,
        weights,
    };
    terms.outcome_count = OUTCOMES;
    terms.payout_count = 1;
    terms.payouts = payouts;
    terms.failure_payout_index = 0;
    terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    terms.basis_degree = DEGREE;
    terms.knot_count = OUTCOMES + 1 - DEGREE;
    terms.uniform_log2_spacing = SPACING;
    let mut knots = [0u128; MAX_KNOTS];
    let mut knot = 0usize;
    while knot < terms.knot_count as usize {
        knots[knot] = (knot as u128) << SPACING;
        knot += 1;
    }
    terms.knots = knots;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    terms
}

async fn start() -> (ProgramTestContext, Fixture) {
    let realm = h(0x71);
    let profile = h(0x72);
    let feed = h(0x73);
    let market = h(0x3b);

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

    let mut terms = degree_two_terms(realm, profile, feed);
    terms.price_grid = grid.grid;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    let (terms_address, terms_bump) =
        pda(seeds::SEED_TERMS, &[&realm.bytes(), &terms.terms.bytes()]);
    terms.stored_bump = terms_bump;
    assert_eq!(terms.basis_degree, DEGREE);

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

    // Cash for the two buyers, Eggs on both traded claims for the one seller.
    let mut owners = Vec::new();
    for (key, cash, eggs) in [
        (Keypair::new(), 60_000u64, [0u64, 0, 0]),
        (Keypair::new(), 0, [30_000u64, 30_000, 0]),
        (Keypair::new(), 100_000, [0u64, 0, 0]),
    ] {
        let id = Hash32::from_bytes(key.pubkey().to_bytes());
        let (position_address, position_bump) =
            pda(seeds::SEED_POSITION, &[&market.bytes(), &id.bytes()]);
        let mut internal = [0u64; MAX_OUTCOMES];
        internal[..3].copy_from_slice(&eggs);
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

/// One walk transaction: heap frame, compute ceiling, one instruction.
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

async fn read_epoch(context: &mut ProgramTestContext, fixture: &Fixture) -> EpochAccount {
    EpochAccount::decode(&account(context, fixture.epoch_account).await.unwrap().data).unwrap()
}

/// Place the four-order book and freeze it at the deadline.
async fn build_frozen_book(context: &mut ProgramTestContext, fixture: &Fixture) {
    let payer = context.payer.pubkey();
    let (result, _) = send(context, &[fixture.init_epoch(payer)], None, 0).await;
    result.unwrap();
    let (result, _) = send(context, &[fixture.init_page(payer)], None, 1).await;
    result.unwrap();

    for (rank, (owner_index, outcome, side, quantity, limit)) in BOOK.into_iter().enumerate() {
        let owner = &fixture.owners[owner_index];
        let slot = order_slot(owner.id, rank as u64 + 1, outcome, side, quantity, limit);
        let (result, _) = send(
            context,
            &[fixture.place(owner, rank as u64, slot)],
            Some(&owner.key),
            10 + rank as u32,
        )
        .await;
        result.unwrap();
    }

    context.warp_to_slot(FREEZE_DEADLINE).unwrap();
    let (result, _) = send(context, &[fixture.freeze()], None, 20).await;
    result.unwrap();
}

/// The projected book, exactly as the walk projects it.
fn project_book(page: &[u8]) -> BookV1 {
    let mut book = BookV1::empty();
    let mut owners = OwnerInterner::new();
    let mut live = 0u16;
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
    book.len = live as u8;
    book
}

/// Install the solver-written feed and its SUBMITTED candidate record.
async fn install_candidate(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    epoch: &EpochAccount,
    prices: [u64; MAX_OUTCOMES],
    fills: &[u64],
    witness: &PairingWitnessV1,
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
        distinct_owners: epoch.owner_count,
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
        distinct_owners: epoch.owner_count,
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

/// The checkpoint at its canonical PDA, by the real five-instruction staged
/// creation.
async fn create_checkpoint(context: &mut ProgramTestContext, fixture: &Fixture, candidate: Hash32) {
    let payer = context.payer.pubkey();
    let creation = [
        fixture.init_clear_work(payer, candidate),
        fixture.grow_clear_work(candidate, 1),
        fixture.grow_clear_work(candidate, 2),
        fixture.grow_clear_work(candidate, 3),
        fixture.grow_clear_work(candidate, 4),
    ];
    let (result, _) = send(context, &creation, None, 100).await;
    result.unwrap();
}

/// Every live reservation of the frozen page, in walk order — the list pass 1
/// owes when it consumes the whole page in one batch.
async fn page_reservations(context: &mut ProgramTestContext, fixture: &Fixture) -> Vec<Address> {
    let page =
        OrderPageAccount::decode(&account(context, fixture.page).await.unwrap().data).unwrap();
    let mut reservations = Vec::new();
    for index in 0..page.order_count as usize {
        let record = &page.orders[index];
        if record.is_live() {
            reservations.push(fixture.reservation(record.owner(), record.order_id()));
        }
    }
    reservations
}

/// Drive an existing checkpoint to its verdict, following the checkpoint's own
/// reported status; returns the decoded body and the completing CU.
///
/// The idle checkpoint reports `Complete` (there is nothing to feed yet), so
/// `is_idle` is what distinguishes "never begun" from "verdict reached" — the
/// distinction `ClearWorkV1::is_idle` exists for.
async fn drive_to_verdict(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    candidate: Hash32,
    label: &str,
) -> (Box<ClearWorkV1>, u64) {
    let work_address = fixture.clear_work(candidate).0;
    let reservations = page_reservations(context, fixture).await;
    let mut step = 0u32;
    loop {
        let work = account(context, work_address).await.unwrap().data;
        let mut body = Box::new(ClearWorkV1::new());
        body.decode_into(clear_work_body(&work).unwrap()).unwrap();
        let instruction = if body.is_idle() {
            fixture.advance(candidate, MAX_ORDERS_PER_PAGE as u16, &reservations)
        } else {
            match body.status() {
                FeedStatusV1::Complete => break,
                FeedStatusV1::NeedSlices => fixture.advance_slices(candidate, MAX_SLICES as u16),
                FeedStatusV1::NeedOrders { pass } => {
                    let owed: &[Address] = if pass == 1 { &reservations } else { &[] };
                    fixture.advance(candidate, MAX_ORDERS_PER_PAGE as u16, owed)
                }
            }
        };
        let (result, units) = send_walk(context, instruction, 110 + step).await;
        result.unwrap();
        eprintln!("{label} step {step} CU: {units}");
        step += 1;
        assert!(step < 16, "the feed must terminate");
    }

    let (result, units) = send_walk(context, fixture.complete(candidate), 130).await;
    result.unwrap();
    eprintln!("{label} CompleteClearWork CU: {units}");
    let work = account(context, work_address).await.unwrap().data;
    let mut body = Box::new(ClearWorkV1::new());
    body.decode_into(clear_work_body(&work).unwrap()).unwrap();
    (body, units)
}

/// Build the canonical candidate and witness for one price vector over the
/// frozen book and install both, with the checkpoint created and idle.
async fn stage_at(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    prices: [u64; 3],
) -> CandidateFeedHeader {
    let epoch = read_epoch(context, fixture).await;
    let page = account(context, fixture.page).await.unwrap().data;
    let book = project_book(&page);
    let domain = zero_sentinel_domain(&epoch);
    let prices = price_vector(prices);
    let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
    let witness = canonical_pairing(&domain, &book, &candidate).unwrap();
    let feed = install_candidate(
        context,
        fixture,
        &epoch,
        prices,
        &candidate.fills[..book.len as usize],
        &witness,
    )
    .await;
    create_checkpoint(context, fixture, feed.candidate).await;
    feed
}

/// [`stage_at`], walked all the way to its persisted verdict.
async fn walk_at(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    prices: [u64; 3],
    label: &str,
) -> (Box<ClearWorkV1>, CandidateRecord) {
    let feed = stage_at(context, fixture, prices).await;
    let (body, _) = drive_to_verdict(context, fixture, feed.candidate, label).await;
    let record = CandidateRecord::decode(
        &account(context, fixture.candidate_record(feed.candidate).0)
            .await
            .unwrap()
            .data,
    )
    .unwrap();
    (body, record)
}

/// The wiring, in one assertion: the degree the immutable terms froze is the
/// byte the epoch carries.
#[tokio::test]
async fn init_epoch_copies_the_terms_basis_degree_onto_the_epoch() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let (result, _) = send(&mut context, &[fixture.init_epoch(payer)], None, 0).await;
    result.unwrap();
    let epoch = read_epoch(&mut context, &fixture).await;
    assert_eq!(epoch.basis_degree, DEGREE);
    assert_eq!(epoch.outcome_count, OUTCOMES);
    // And the epoch is exactly what `binds_terms` re-derives from the terms
    // it names: the copy is checkable, not a second truth.
    let terms = clutch_solana_layout::TermsAccount::decode(
        &account(&mut context, fixture.terms_account)
            .await
            .unwrap()
            .data,
    )
    .unwrap();
    let grid = PriceGridAccount::decode(
        &account(&mut context, fixture.grid_account)
            .await
            .unwrap()
            .data,
    )
    .unwrap();
    assert_eq!(epoch.binds_terms(&terms, &grid), Ok(()));
    let mut swapped = epoch;
    swapped.basis_degree = 1;
    assert_eq!(
        swapped.binds_terms(&terms, &grid),
        Err(CodecError::MismatchedBinding)
    );
}

/// The headline: one book, two price vectors, one bit of difference.
#[tokio::test]
async fn the_degree_two_walk_refuses_outside_the_cone_and_clears_inside_it() {
    // Out of the cone: the walk must refuse at V1b, and the record must be
    // REFUSED — the relation refusal is a verdict, not a transaction fault,
    // so the discriminating read is the checkpoint's own error.
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let epoch = read_epoch(&mut context, &fixture).await;
    assert_eq!(epoch.basis_degree, DEGREE);
    assert_eq!(epoch.order_count, BOOK.len() as u16);

    // The host twin refuses on the same coordinates, at the same stage.
    {
        let page = account(&mut context, fixture.page).await.unwrap().data;
        let book = project_book(&page);
        let domain = zero_sentinel_domain(&epoch);
        let prices = price_vector(OUT_OF_CONE);
        let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
        let witness = canonical_pairing(&domain, &book, &candidate).unwrap();
        assert_eq!(
            verify_ignoring_claimed_aggregates_with_basis(
                &domain,
                &book,
                &candidate,
                Some(&witness),
                epoch_basis(&epoch),
            ),
            Err(ErrorV1::PriceOutsideMomentCone { outcome: 1 })
        );
        // And the same coordinates are accepted with no basis bound: the
        // degree byte is the whole difference.
        assert!(verify_ignoring_claimed_aggregates_with_basis(
            &domain,
            &book,
            &candidate,
            Some(&witness),
            BasisDescriptorV1::UNGATED,
        )
        .is_ok());
    }

    /* One advance and the close: a refusal latched at or below the
     * V0-complete major stage ends the feed at the first pass boundary
     * (`end_pass`'s `v0_complete` short circuit), so the refused candidate
     * never pays for the slice pass or pass two. */
    let (body, record) = walk_at(&mut context, &fixture, OUT_OF_CONE, "out-of-cone").await;
    assert_eq!(
        body.verdict(),
        Some(Err(ErrorV1::PriceOutsideMomentCone { outcome: 1 }))
    );
    assert_eq!(record.status, CANDIDATE_STATUS_REFUSED);
    assert_eq!(record.score_digest, Hash32::ZERO);

    // In the cone: the same book, the same degree, and the walk clears.
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let (body, record) = walk_at(&mut context, &fixture, IN_CONE, "in-cone").await;
    let summary = body.verdict().unwrap().copied().unwrap();
    assert_eq!(record.status, CANDIDATE_STATUS_VERIFIED);
    assert_ne!(record.score_digest, Hash32::ZERO);
    assert!(
        summary.score.weighted_direct_volume > 0,
        "the in-cone candidate actually trades"
    );
}

/// The hostile: the epoch's degree byte is the gate, and moving it moves the
/// verdict on an otherwise identical walk.
///
/// An *in-range* swap is not catchable on this path and the test says so
/// plainly: `AdvanceClearWork` presents the epoch, the feed, the checkpoint,
/// and one page — never the terms — so no validator it can reach holds the
/// authority the byte was copied from.  That is the whole reason the copy
/// happens once, at `InitEpoch`, under `EpochAccount::binds_terms`'s reach,
/// and the reason the byte is inside a program-owned account that only this
/// program writes.  What a bank-god `set_account` demonstrates here is the
/// converse of the headline: the same book and the same out-of-cone prices
/// clear once the epoch claims degree one.
#[tokio::test]
async fn a_downgraded_epoch_degree_byte_ungates_the_same_out_of_cone_price() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;

    let honest = account(&mut context, fixture.epoch_account).await.unwrap();
    assert_eq!(honest.data[EPOCH_BASIS_DEGREE_OFFSET], DEGREE);
    let mut downgraded = EpochAccount::decode(&honest.data).unwrap();
    downgraded.basis_degree = 1;
    let mut forged = honest.clone();
    forged.data = encode(account_len::EPOCH, |out| downgraded.encode(out));
    context.set_account(&fixture.epoch_account, &forged.into());

    let (body, record) = walk_at(&mut context, &fixture, OUT_OF_CONE, "downgraded").await;
    assert_ne!(
        body.verdict(),
        Some(Err(ErrorV1::PriceOutsideMomentCone { outcome: 1 })),
        "degree one makes V1b the constant true"
    );
    assert_eq!(record.status, CANDIDATE_STATUS_VERIFIED);
}

/// A degree byte above the implemented ceiling never reaches the walk: the
/// epoch codec's own bound refuses it first.
///
/// `EpochAccount::encode` runs `validate`, so this tamper cannot be written
/// through the codec at all — it has to be a raw byte edit, which is exactly
/// why the walk's `BasisDegreeV1::from_u8` refusal is stated rather than
/// assumed.
#[tokio::test]
async fn an_out_of_range_epoch_degree_byte_refuses_at_the_epoch_codec() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;

    // An otherwise perfectly good in-cone candidate, staged and ready to walk.
    let feed = stage_at(&mut context, &fixture, IN_CONE).await;
    let reservations = page_reservations(&mut context, &fixture).await;

    let honest = account(&mut context, fixture.epoch_account).await.unwrap();
    assert_eq!(honest.data[EPOCH_BASIS_DEGREE_OFFSET], DEGREE);
    let mut raw = EpochAccount::decode(&honest.data).unwrap();
    raw.basis_degree = clutch_solana_layout::MAX_BASIS_DEGREE + 1;
    assert_eq!(
        raw.validate(),
        Err(CodecError::InvalidEnum),
        "the codec refuses to write the tamper"
    );

    let mut forged = honest.clone();
    forged.data[EPOCH_BASIS_DEGREE_OFFSET] = clutch_solana_layout::MAX_BASIS_DEGREE + 1;
    assert_eq!(
        EpochAccount::decode(&forged.data),
        Err(CodecError::InvalidEnum)
    );
    context.set_account(&fixture.epoch_account, &forged.into());

    // The advance decodes the epoch before it reaches the walk, so the whole
    // clearing plane is closed while the byte is out of range — the walk's own
    // `BasisDegreeV1::from_u8` refusal is the layer behind this one.
    let (result, _) = send_walk(
        &mut context,
        fixture.advance(feed.candidate, MAX_ORDERS_PER_PAGE as u16, &reservations),
        200,
    )
    .await;
    assert_eq!(custom(result), codec_code(CodecError::InvalidEnum));

    // Restored, the identical advance is admitted: nothing but the degree byte
    // moved.
    context.set_account(&fixture.epoch_account, &honest.into());
    let (result, _) = send_walk(
        &mut context,
        fixture.advance(feed.candidate, MAX_ORDERS_PER_PAGE as u16, &reservations),
        201,
    )
    .await;
    result.unwrap();
}
