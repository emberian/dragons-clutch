#![cfg(any())]
//! Historical Direct V3 bank campaign. The wire remains decode-only, while
//! the handler is intentionally absent from every current program artifact.
//!
//! Real-SBF campaign for the complete routed Direct V3 lifecycle.
//!
//! Everything the family owns is driven through the routed instructions on a
//! bank that injects only the pre-existing plane (Market, Terms, PriceGrid,
//! the 96-byte DirectBatchPolicy V3 artifact, two live Positions) plus
//! one-lamport prefunds on every predictable PDA. The campaign covers the
//! full settled lifecycle with model-anchored Position-transfer numbers,
//! five-candidate replacement/tie/noncompetitive dispositions predicted from
//! the same verifier scores the model ranks with, admitted-tick replay
//! refusal, late-failure byte-and-lamport rollback, donation observation and
//! close-time disposition, exact rent/reward conservation per keypair, the
//! zero/one/two pre-freeze aborts, all three lapse phases, and the
//! predecessor's exact failure mode as a regression: a legacy-shaped
//! PlaceOrder refuses against V4 state and a V4-shaped placement refuses
//! against legacy epochs.

use {
    clutch_batch_policy_identity::{
        direct_lifecycle_v3::admission_transcript_v3,
        direct_window_v1::{
            canonical_account_candidate_id, verify_direct_two_order_candidate,
            DirectTwoOrderInputV1, DIRECT_CANDIDATE_STATUS_SELECTED,
            DIRECT_CANDIDATE_STATUS_VERIFIED, DIRECT_POLICY_V1,
        },
        FullRelationDomainV1, FullScoreV1, Identity32V1,
    },
    clutch_sbf::{
        error::ClutchError,
        instructions::artifact::CLOCK_SYSVAR_ID,
        instructions::direct_selection_v3::{
            DIRECT_NEUTRAL_SINK_V3, DIRECT_VERIFIER_RELEASE_ID_V3,
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        direct_selection_v3::{
            DirectBatchPolicyV3, DirectEpochV4Account, DirectKeeperRewardsV3,
            DirectReservationV2Account, DirectV3Intent, DirectWindowV3Account,
            DirectWorkBudgetV1Account, DIRECT_CANDIDATE_STATUS_REVERIFIED,
            DIRECT_CANDIDATE_V3_BYTES, DIRECT_EPOCH_V4_BYTES, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY,
            DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN, DIRECT_LIFECYCLE_PHASE_SELECTED,
            DIRECT_LIFECYCLE_PHASE_TERMINAL, DIRECT_RESERVATION_V2_BYTES,
            DIRECT_TERMINAL_REASON_EMPTY_LAPSE, DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE,
            DIRECT_TERMINAL_REASON_PREFREEZE_ABORT, DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE,
            DIRECT_TERMINAL_REASON_SETTLED, DIRECT_WORK_BUDGET_BYTES,
        },
        reservation::{RESERVATION_STATE_ACTIVE, RESERVATION_STATE_ENTITLED},
        Hash32, Intent, MarketAccount, OrderRecord, OrderSlot, PositionAccount, PriceGridAccount,
        EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED, EPOCH_PHASE_OPEN,
        EPOCH_PHASE_SETTLED, MAX_GRID_TICKS, MAX_INTENT_BYTES, MAX_OUTCOMES,
    },
    clutch_solana_reference::DirectV3Request,
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
/// Model-anchored quantity: with `quantity == price_scale`, the settlement
/// consideration in atoms equals the winning price, exactly as the model's
/// settle tests anchor it.
const QUANTITY: u64 = 10_000;
const BUY_LIMIT: u64 = 10_000;
const SELL_LIMIT: u64 = 0;
const BUYER_CASH: u64 = 30_000;
const EPOCH_INDEX: u64 = 7;
const OPEN_SLOT: u64 = 100;
const CLOSE_SLOT: u64 = 110;
const SELECTION_DEADLINE: u64 = 120;
const SETTLEMENT_DEADLINE: u64 = 140;
const CU_LIMIT: u32 = 1_400_000;
const REWARDS: DirectKeeperRewardsV3 = DirectKeeperRewardsV3 {
    begin_verification: 5_000,
    verify_candidate: 7_000,
    finalize_selection: 9_000,
    settle: 11_000,
    lapse: 13_000,
};
/// begin + 3*verify + finalize + max(settle, lapse) = 48_000.
const REWARD_DEPOSIT: u64 = 48_000;
const WALLET: u64 = 1_000_000_000;

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

fn sink_address() -> Address {
    Address::new_from_array(DIRECT_NEUTRAL_SINK_V3.to_bytes())
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

fn direct_v3(sequence: u64, intent: DirectV3Intent) -> Vec<u8> {
    let request = DirectV3Request { sequence, intent };
    let mut bytes = vec![0; 13 + MAX_INTENT_BYTES];
    let written = request.encode(&mut bytes).unwrap();
    bytes.truncate(written);
    bytes
}

/// One epoch's complete predictable address plane.
struct EpochPlane {
    epoch_id: Hash32,
    epoch_index: u64,
    epoch: Address,
    policy: Address,
    policy_digest: Hash32,
    page: Address,
    window: Address,
    work: Address,
    buy_reservation: Address,
    sell_reservation: Address,
}

struct Fixture {
    market: Hash32,
    market_account: Address,
    terms: Address,
    grid: Address,
    buyer_position: Address,
    seller_position: Address,
    buy_owner: Keypair,
    sell_owner: Keypair,
    sponsor: Keypair,
    keeper: Keypair,
    submitter: Keypair,
    low_funder: Keypair,
    epochs: Vec<EpochPlane>,
}

impl Fixture {
    fn plane(&self, index: usize) -> &EpochPlane {
        &self.epochs[index]
    }

    fn candidate_address(&self, plane: usize, candidate_id: Identity32V1) -> Address {
        pda(
            seeds::SEED_DIRECT_CANDIDATE_V3,
            &[&self.plane(plane).epoch_id.bytes(), &candidate_id.0],
        )
        .0
    }

    fn init(&self, payer: Address, plane: usize) -> Instruction {
        let plane = self.plane(plane);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::InitEpoch {
                    market: self.market,
                    epoch_index: plane.epoch_index,
                    policy: plane.policy_digest,
                    submission_opens_slot: OPEN_SLOT,
                    submission_closes_slot: CLOSE_SLOT,
                    selection_deadline_slot: SELECTION_DEADLINE,
                    settlement_deadline_slot: SETTLEMENT_DEADLINE,
                    neutral_lamport_sink: Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()),
                },
            ),
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(plane.epoch, false),
                AccountMeta::new_readonly(self.market_account, false),
                AccountMeta::new_readonly(self.terms, false),
                AccountMeta::new_readonly(self.grid, false),
                AccountMeta::new_readonly(plane.policy, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(clock_address(), false),
            ],
        )
    }

    fn init_page(&self, payer: Address, plane: usize) -> Instruction {
        let plane = self.plane(plane);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitOrderPage {
                    market: self.market,
                    epoch: plane.epoch_id,
                    page_index: 0,
                    page_count: 1,
                },
            ),
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(plane.page, false),
                AccountMeta::new_readonly(self.market_account, false),
                AccountMeta::new(plane.epoch, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
        )
    }

    fn order(&self, plane: usize, rank: u64, side: u8, limit: u64) -> OrderRecord {
        let owner = if side == 0 {
            Hash32::from_bytes(self.buy_owner.pubkey().to_bytes())
        } else {
            Hash32::from_bytes(self.sell_owner.pubkey().to_bytes())
        };
        OrderRecord {
            owner,
            order_id: canonical_order_id(rank),
            outcome: 0,
            side,
            quantity: QUANTITY,
            limit,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: self.plane(plane).epoch_index,
        }
    }

    /// The routed nine-account V4 placement.
    fn place(
        &self,
        plane_index: usize,
        sequence: u64,
        order: OrderRecord,
        reservation: Address,
    ) -> Instruction {
        let plane = self.plane(plane_index);
        let (position, actor) = if order.side == 0 {
            (self.buyer_position, self.buy_owner.pubkey())
        } else {
            (self.seller_position, self.sell_owner.pubkey())
        };
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::PlaceOrder {
                    market: self.market,
                    epoch: plane.epoch_id,
                    max_fee_atoms: 0,
                    slot: OrderSlot::Single(order),
                },
            ),
            vec![
                AccountMeta::new(actor, true),
                AccountMeta::new(plane.epoch, false),
                AccountMeta::new_readonly(self.grid, false),
                AccountMeta::new(plane.page, false),
                AccountMeta::new(position, false),
                AccountMeta::new(reservation, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(plane.policy, false),
            ],
        )
    }

    /// The legacy eight-account placement shape (the predecessor's hazard).
    fn place_legacy_shape(&self, plane_index: usize, order: OrderRecord) -> Instruction {
        let mut instruction = self.place(plane_index, 0, order, self.plane(plane_index).epoch);
        instruction.accounts.truncate(8);
        // Restore the legacy account list: [actor, epoch, grid, page,
        // position, reservation, system, rent].
        instruction.accounts[5] =
            AccountMeta::new(pda(seeds::SEED_RESERVATION, &[&[0x77; 32]]).0, false);
        instruction
    }

    fn freeze(&self, sponsor: Address, plane_index: usize) -> Instruction {
        let plane = self.plane(plane_index);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::FreezeEpoch {
                    market: self.market,
                    epoch: plane.epoch_id,
                    reward_deposit: REWARD_DEPOSIT,
                    rewards: REWARDS,
                },
            ),
            vec![
                AccountMeta::new(sponsor, true),
                AccountMeta::new(plane.epoch, false),
                AccountMeta::new(plane.page, false),
                AccountMeta::new_readonly(self.grid, false),
                AccountMeta::new_readonly(plane.policy, false),
                AccountMeta::new(plane.buy_reservation, false),
                AccountMeta::new(plane.sell_reservation, false),
                AccountMeta::new(plane.work, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(clock_address(), false),
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn submit(
        &self,
        payer: Address,
        plane_index: usize,
        outcome_price: u64,
        candidate: Address,
        retained: &[Address],
        displaced_payer: Option<Address>,
    ) -> Instruction {
        let plane = self.plane(plane_index);
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(plane.epoch, false),
            AccountMeta::new_readonly(plane.policy, false),
            AccountMeta::new_readonly(self.grid, false),
            AccountMeta::new_readonly(plane.page, false),
            AccountMeta::new_readonly(plane.buy_reservation, false),
            AccountMeta::new_readonly(plane.sell_reservation, false),
            AccountMeta::new(plane.window, false),
            AccountMeta::new(candidate, false),
        ];
        metas.extend(
            retained
                .iter()
                .map(|address| AccountMeta::new(*address, false)),
        );
        if retained.len() == 3 {
            metas.push(AccountMeta::new(sink_address(), false));
            metas.push(AccountMeta::new(
                displaced_payer.expect("full top needs a displaced payer"),
                false,
            ));
        }
        metas.extend([
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(clock_address(), false),
        ]);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::SubmitCandidate {
                    market: self.market,
                    epoch: plane.epoch_id,
                    outcome_price,
                },
            ),
            metas,
        )
    }

    fn begin(&self, keeper: Address, plane_index: usize) -> Instruction {
        let plane = self.plane(plane_index);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::BeginVerification {
                    market: self.market,
                    epoch: plane.epoch_id,
                },
            ),
            vec![
                AccountMeta::new(keeper, true),
                AccountMeta::new(plane.epoch, false),
                AccountMeta::new(plane.window, false),
                AccountMeta::new(plane.work, false),
                AccountMeta::new_readonly(clock_address(), false),
            ],
        )
    }

    fn verify(
        &self,
        keeper: Address,
        plane_index: usize,
        retained_index: u8,
        candidate: Address,
    ) -> Instruction {
        let plane = self.plane(plane_index);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::VerifyCandidate {
                    market: self.market,
                    epoch: plane.epoch_id,
                    retained_index,
                },
            ),
            vec![
                AccountMeta::new(keeper, true),
                AccountMeta::new_readonly(plane.epoch, false),
                AccountMeta::new_readonly(self.grid, false),
                AccountMeta::new_readonly(plane.page, false),
                AccountMeta::new(plane.window, false),
                AccountMeta::new(candidate, false),
                AccountMeta::new(plane.work, false),
                AccountMeta::new_readonly(clock_address(), false),
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finalize(
        &self,
        payer: Address,
        plane_index: usize,
        retained: &[Address],
        receipt: Address,
        pot: Address,
        loser_payers: &[Address],
    ) -> Instruction {
        let plane = self.plane(plane_index);
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(plane.epoch, false),
            AccountMeta::new_readonly(plane.page, false),
            AccountMeta::new(plane.window, false),
            AccountMeta::new(plane.buy_reservation, false),
            AccountMeta::new(plane.sell_reservation, false),
        ];
        metas.extend(
            retained
                .iter()
                .map(|address| AccountMeta::new(*address, false)),
        );
        metas.extend([
            AccountMeta::new(receipt, false),
            AccountMeta::new(pot, false),
            AccountMeta::new(plane.work, false),
            AccountMeta::new(sink_address(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(clock_address(), false),
        ]);
        metas.extend(
            loser_payers
                .iter()
                .map(|address| AccountMeta::new(*address, false)),
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::FinalizeSelection {
                    market: self.market,
                    epoch: plane.epoch_id,
                },
            ),
            metas,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn settle(
        &self,
        keeper: Address,
        plane_index: usize,
        candidate: Address,
        receipt: Address,
        pot: Address,
        candidate_payer: Address,
        receipt_pot_payer: Address,
    ) -> Instruction {
        let plane = self.plane(plane_index);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::Settle {
                    market: self.market,
                    epoch: plane.epoch_id,
                },
            ),
            vec![
                AccountMeta::new(keeper, true),
                AccountMeta::new(plane.epoch, false),
                AccountMeta::new_readonly(plane.page, false),
                AccountMeta::new(plane.window, false),
                AccountMeta::new(candidate, false),
                AccountMeta::new(self.buyer_position, false),
                AccountMeta::new(self.seller_position, false),
                AccountMeta::new(plane.buy_reservation, false),
                AccountMeta::new(plane.sell_reservation, false),
                AccountMeta::new(receipt, false),
                AccountMeta::new(pot, false),
                AccountMeta::new(plane.work, false),
                AccountMeta::new(sink_address(), false),
                AccountMeta::new_readonly(clock_address(), false),
                AccountMeta::new(candidate_payer, false),
                AccountMeta::new(self.submitter.pubkey(), false),
                AccountMeta::new(receipt_pot_payer, false),
                AccountMeta::new(receipt_pot_payer, false),
                AccountMeta::new(self.sponsor.pubkey(), false),
                AccountMeta::new(self.buy_owner.pubkey(), false),
                AccountMeta::new(self.sell_owner.pubkey(), false),
            ],
        )
    }

    fn abort(
        &self,
        plane_index: usize,
        with_page: bool,
        reservations: &[Address],
        positions: &[Address],
        payers: &[Address],
    ) -> Instruction {
        let plane = self.plane(plane_index);
        let mut metas = vec![
            AccountMeta::new(plane.epoch, false),
            AccountMeta::new_readonly(clock_address(), false),
            AccountMeta::new(sink_address(), false),
        ];
        if with_page {
            metas.push(AccountMeta::new_readonly(plane.page, false));
        }
        metas.extend(
            reservations
                .iter()
                .map(|address| AccountMeta::new(*address, false)),
        );
        metas.extend(
            positions
                .iter()
                .map(|address| AccountMeta::new(*address, false)),
        );
        metas.extend(
            payers
                .iter()
                .map(|address| AccountMeta::new(*address, false)),
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::AbortUnfrozen {
                    market: self.market,
                    epoch: plane.epoch_id,
                },
            ),
            metas,
        )
    }

    fn lapse_empty(&self, keeper: Address, plane_index: usize) -> Instruction {
        let plane = self.plane(plane_index);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::LapseEmpty {
                    market: self.market,
                    epoch: plane.epoch_id,
                },
            ),
            vec![
                AccountMeta::new(keeper, true),
                AccountMeta::new(plane.epoch, false),
                AccountMeta::new_readonly(plane.page, false),
                AccountMeta::new(plane.buy_reservation, false),
                AccountMeta::new(plane.sell_reservation, false),
                AccountMeta::new(self.buyer_position, false),
                AccountMeta::new(self.seller_position, false),
                AccountMeta::new(plane.work, false),
                AccountMeta::new(sink_address(), false),
                AccountMeta::new_readonly(clock_address(), false),
                AccountMeta::new(self.sponsor.pubkey(), false),
                AccountMeta::new(self.buy_owner.pubkey(), false),
                AccountMeta::new(self.sell_owner.pubkey(), false),
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn lapse_unselected(
        &self,
        keeper: Address,
        plane_index: usize,
        candidates: &[Address],
        candidate_payers: &[Address],
    ) -> Instruction {
        let plane = self.plane(plane_index);
        let mut metas = vec![
            AccountMeta::new(keeper, true),
            AccountMeta::new(plane.epoch, false),
            AccountMeta::new_readonly(plane.page, false),
            AccountMeta::new(plane.window, false),
        ];
        metas.extend(
            candidates
                .iter()
                .map(|address| AccountMeta::new(*address, false)),
        );
        metas.extend([
            AccountMeta::new(plane.buy_reservation, false),
            AccountMeta::new(plane.sell_reservation, false),
            AccountMeta::new(self.buyer_position, false),
            AccountMeta::new(self.seller_position, false),
            AccountMeta::new(plane.work, false),
            AccountMeta::new(sink_address(), false),
            AccountMeta::new_readonly(clock_address(), false),
            AccountMeta::new(self.submitter.pubkey(), false),
        ]);
        metas.extend(
            candidate_payers
                .iter()
                .map(|address| AccountMeta::new(*address, false)),
        );
        metas.extend([
            AccountMeta::new(self.sponsor.pubkey(), false),
            AccountMeta::new(self.buy_owner.pubkey(), false),
            AccountMeta::new(self.sell_owner.pubkey(), false),
        ]);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::LapseUnselected {
                    market: self.market,
                    epoch: plane.epoch_id,
                },
            ),
            metas,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn lapse_selected(
        &self,
        keeper: Address,
        plane_index: usize,
        candidate: Address,
        receipt: Address,
        pot: Address,
        candidate_payer: Address,
        receipt_pot_payer: Address,
    ) -> Instruction {
        let plane = self.plane(plane_index);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &direct_v3(
                0,
                DirectV3Intent::LapseSelected {
                    market: self.market,
                    epoch: plane.epoch_id,
                },
            ),
            vec![
                AccountMeta::new(keeper, true),
                AccountMeta::new(plane.epoch, false),
                AccountMeta::new_readonly(plane.page, false),
                AccountMeta::new(plane.window, false),
                AccountMeta::new(candidate, false),
                AccountMeta::new(receipt, false),
                AccountMeta::new(pot, false),
                AccountMeta::new(plane.buy_reservation, false),
                AccountMeta::new(plane.sell_reservation, false),
                AccountMeta::new(self.buyer_position, false),
                AccountMeta::new(self.seller_position, false),
                AccountMeta::new(plane.work, false),
                AccountMeta::new(sink_address(), false),
                AccountMeta::new_readonly(clock_address(), false),
                AccountMeta::new(self.submitter.pubkey(), false),
                AccountMeta::new(candidate_payer, false),
                AccountMeta::new(receipt_pot_payer, false),
                AccountMeta::new(receipt_pot_payer, false),
                AccountMeta::new(self.sponsor.pubkey(), false),
                AccountMeta::new(self.buy_owner.pubkey(), false),
                AccountMeta::new(self.sell_owner.pubkey(), false),
            ],
        )
    }
}

async fn start(epoch_indexes: &[u64]) -> (ProgramTestContext, Fixture) {
    let realm = h(0x61);
    let profile = h(0x62);
    let feed = h(0x63);
    let market = h(0x31);
    let buy_owner = Keypair::new();
    let sell_owner = Keypair::new();
    let sponsor = Keypair::new();
    let keeper = Keypair::new();
    let submitter = Keypair::new();
    let low_funder = Keypair::new();

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

    let buy_owner_id = Hash32::from_bytes(buy_owner.pubkey().to_bytes());
    let sell_owner_id = Hash32::from_bytes(sell_owner.pubkey().to_bytes());
    let (buyer_position_address, buyer_position_bump) = pda(
        seeds::SEED_POSITION,
        &[&market.bytes(), &buy_owner_id.bytes()],
    );
    let buyer_position = PositionAccount {
        market,
        owner: buy_owner_id,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: BUYER_CASH,
        reserved_cash_atoms: 0,
        stored_bump: buyer_position_bump,
        close_state: 0,
    };
    let (seller_position_address, seller_position_bump) = pda(
        seeds::SEED_POSITION,
        &[&market.bytes(), &sell_owner_id.bytes()],
    );
    let mut seller_internal = [0u64; MAX_OUTCOMES];
    seller_internal[0] = QUANTITY.saturating_mul(epoch_indexes.len() as u64);
    let seller_position = PositionAccount {
        market,
        owner: sell_owner_id,
        generation: 0,
        internal: seller_internal,
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        stored_bump: seller_position_bump,
        close_state: 0,
    };

    let artifact = DirectBatchPolicyV3::direct(DIRECT_VERIFIER_RELEASE_ID_V3).unwrap();
    let mut artifact_bytes = vec![0; 96];
    artifact.encode(&mut artifact_bytes).unwrap();

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
        buyer_position_address,
        encode(account_len::POSITION, |out| buyer_position.encode(out)),
    );
    add_state(
        &mut test,
        seller_position_address,
        encode(account_len::POSITION, |out| seller_position.encode(out)),
    );

    let mut epochs = Vec::new();
    for epoch_index in epoch_indexes {
        let epoch_id = canonical_epoch_id(market, *epoch_index);
        let policy_digest = artifact.digest_for_epoch(epoch_id).unwrap();
        let (policy_address, _) = pda(
            seeds::SEED_DIRECT_BATCH_POLICY_V3,
            &[&epoch_id.bytes(), &policy_digest.bytes()],
        );
        add_state(&mut test, policy_address, artifact_bytes.clone());
        let (epoch_address, _) = pda(
            seeds::SEED_EPOCH,
            &[&market.bytes(), &epoch_index.to_le_bytes()],
        );
        let (page_address, _) = pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &0u16.to_le_bytes()]);
        let (window_address, _) = pda(seeds::SEED_DIRECT_WINDOW_V3, &[&epoch_id.bytes()]);
        let (work_address, _) = pda(seeds::SEED_DIRECT_WORK_V3, &[&epoch_id.bytes()]);
        let buy_reservation_id = clutch_solana_layout::reservation::canonical_reservation_id(
            market,
            epoch_id,
            buy_owner_id,
            0,
            canonical_order_id(1),
        );
        let sell_reservation_id = clutch_solana_layout::reservation::canonical_reservation_id(
            market,
            epoch_id,
            sell_owner_id,
            0,
            canonical_order_id(2),
        );
        let (buy_reservation, _) = pda(seeds::SEED_RESERVATION, &[&buy_reservation_id.bytes()]);
        let (sell_reservation, _) = pda(seeds::SEED_RESERVATION, &[&sell_reservation_id.bytes()]);
        // One-lamport prefund on every predictable PDA the lifecycle creates,
        // including every on-grid candidate target.
        for address in [
            epoch_address,
            page_address,
            window_address,
            work_address,
            buy_reservation,
            sell_reservation,
        ] {
            test.add_account(address, system_slot(1));
        }
        let mut price = 1_000u64;
        while price <= 9_000 {
            let mut prices = [0; MAX_OUTCOMES];
            prices[0] = price;
            prices[1] = PRICE_SCALE - price;
            let candidate_id = canonical_account_candidate_id(
                Identity32V1(epoch_id.bytes()),
                Identity32V1(market.bytes()),
                &prices,
            );
            let (candidate_address, _) = pda(
                seeds::SEED_DIRECT_CANDIDATE_V3,
                &[&epoch_id.bytes(), &candidate_id.0],
            );
            test.add_account(candidate_address, system_slot(1));
            price += 1_000;
        }
        epochs.push(EpochPlane {
            epoch_id,
            epoch_index: *epoch_index,
            epoch: epoch_address,
            policy: policy_address,
            policy_digest,
            page: page_address,
            window: window_address,
            work: work_address,
            buy_reservation,
            sell_reservation,
        });
    }

    for wallet in [
        buy_owner.pubkey(),
        sell_owner.pubkey(),
        sponsor.pubkey(),
        keeper.pubkey(),
        submitter.pubkey(),
    ] {
        test.add_account(wallet, system_slot(WALLET));
    }
    test.add_account(
        low_funder.pubkey(),
        system_slot(rent_exempt(DIRECT_CANDIDATE_V3_BYTES) / 2),
    );

    let fixture = Fixture {
        market,
        market_account: market_address,
        terms: terms_address,
        grid: grid_address,
        buyer_position: buyer_position_address,
        seller_position: seller_position_address,
        buy_owner,
        sell_owner,
        sponsor,
        keeper,
        submitter,
        low_funder,
        epochs,
    };
    (test.start_with_context().await, fixture)
}

/// One transaction; a nonce keeps otherwise-identical retries distinct.
async fn send_nonced(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    extra_signer: Option<&Keypair>,
    nonce: u32,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT - nonce),
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
    if outcome.result.is_err() {
        if let Some(metadata) = &outcome.metadata {
            for line in &metadata.log_messages {
                eprintln!("LOG {line}");
            }
        }
    }
    let units = outcome
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    (outcome.result, units)
}

async fn send(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    extra_signer: Option<&Keypair>,
) -> (Result<(), TransactionError>, u64) {
    send_nonced(context, instruction, extra_signer, 0).await
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

async fn lamports(context: &mut ProgramTestContext, address: Address) -> u64 {
    account(context, address)
        .await
        .map(|account| account.lamports)
        .unwrap_or_default()
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

fn assert_cu(label: &str, units: u64) {
    eprintln!("DirectV3 CU {label}: {units}");
    assert!(
        units < u64::from(CU_LIMIT),
        "{label} used {units} CU at the 1.4M transaction cap"
    );
}

/// Physical-close evidence.  Each `close_funded_account` route is observed as
/// three raw numbers in the bank log: what every account it closes held
/// immediately before the route ran (`CLOSE`), what each recorded recipient
/// and the frozen neutral sink gained (`REFUND`), and the exact equality
/// between the two (`CONSERVE`).  Nothing here is estimated: the totals come
/// from bank reads either side of one transaction, and the equality is
/// asserted, so a route that leaked or invented a lamport fails the test
/// rather than printing a pretty number.
fn note_close(route: &str, label: &str, pre_close: u64) {
    eprintln!("DirectV3 CLOSE {route} {label}: pre_close {pre_close}");
}

fn note_refund(route: &str, label: &str, delta: i128) {
    eprintln!("DirectV3 REFUND {route} {label}: delta {delta}");
}

fn assert_conserved(route: &str, closed_total: u64, recipient_total: i128) {
    eprintln!("DirectV3 CONSERVE {route}: closed {closed_total} recipients {recipient_total}");
    assert_eq!(
        i128::from(closed_total),
        recipient_total,
        "{route}: closed principal did not land exactly on the recorded recipients"
    );
}

/// A route that both closes and creates accounts is stated as a closed system
/// instead: over every account the route can touch, the lamport deltas sum to
/// zero.  Nothing is created from nowhere and nothing leaves the observed set.
fn assert_zero_sum(route: &str, net: i128) {
    eprintln!("DirectV3 CONSERVE {route}: net {net}");
    assert_eq!(
        net, 0,
        "{route}: lamports entered or left the observed account set"
    );
}

/// Rent that no handler can reclaim, recorded rather than refunded.
fn note_strand(route: &str, label: &str, held: u64) {
    eprintln!("DirectV3 STRAND {route} {label}: {held}");
}

/// Byte-and-lamport prestate equality after a late-failing transaction.
fn note_rollback(route: &str, watched: usize) {
    eprintln!("DirectV3 ROLLBACK {route}: {watched} accounts byte-and-lamport identical");
}

/// Lamports held by a live program-owned account.  `None` for a PDA that is
/// only a system-owned prefund slot: nothing there is closable principal, so
/// it must never be reported as a close.
async fn live_principal(context: &mut ProgramTestContext, address: Address) -> Option<u64> {
    account(context, address)
        .await
        .filter(|state| state.owner == PROGRAM_ID)
        .map(|state| state.lamports)
}

async fn wallets(context: &mut ProgramTestContext, addresses: &[Address]) -> Vec<u64> {
    let mut out = Vec::with_capacity(addresses.len());
    for address in addresses {
        out.push(lamports(context, *address).await);
    }
    out
}

/// Total the deltas across one route's recorded recipients, printing each.
fn refund_total(route: &str, labels: &[&str], before: &[u64], after: &[u64]) -> i128 {
    let mut total = 0i128;
    for (index, label) in labels.iter().enumerate() {
        let delta = i128::from(after[index]) - i128::from(before[index]);
        note_refund(route, label, delta);
        total += delta;
    }
    total
}

/// Rebuild the exact expected Candidate through the same verifier the model
/// and the program rank with.
fn expected_candidate(
    epoch: &DirectEpochV4Account,
    price: u64,
    submitted_slot: u64,
    stored_bump: u8,
) -> clutch_batch_policy_identity::direct_window_v1::DirectCandidateV2 {
    let common = epoch.direct.common;
    let domain = FullRelationDomainV1 {
        relation_version: common.relation_version,
        market_id: Identity32V1(common.market.bytes()),
        book_id: Identity32V1(common.book.bytes()),
        epoch_id: Identity32V1(common.epoch.bytes()),
        policy_id: Identity32V1(common.policy.bytes()),
        order_set_id: Identity32V1(common.order_set.bytes()),
        epoch_index: common.epoch_index,
        outcome_count: common.outcome_count,
        owner_count: common.owner_count,
        price_scale: common.price_scale,
        remainder_seed: common.remainder_seed,
        policy: DIRECT_POLICY_V1,
    };
    let mut prices = [0; MAX_OUTCOMES];
    prices[0] = price;
    prices[1] = PRICE_SCALE - price;
    verify_direct_two_order_candidate(
        &domain,
        DirectTwoOrderInputV1 {
            prices,
            buy_limit: BUY_LIMIT,
            sell_limit: SELL_LIMIT,
            quantity: QUANTITY,
            submitted_slot,
            buy_index: 0,
            sell_index: 1,
            outcome: 0,
            stored_bump,
        },
    )
    .unwrap()
}

/// The full settled lifecycle on one bank, with exact evidence at each stage.
#[tokio::test]
async fn direct_v3_full_lifecycle_settles_exactly() {
    let (mut context, fixture) = start(&[EPOCH_INDEX]).await;
    let payer = context.payer.pubkey();
    let plane = 0usize;
    let epoch_address = fixture.plane(plane).epoch;

    // Wallet baselines for exact rent/reward conservation at terminal.
    let submitter_start = lamports(&mut context, fixture.submitter.pubkey()).await;
    let sponsor_start = lamports(&mut context, fixture.sponsor.pubkey()).await;
    let keeper_start = lamports(&mut context, fixture.keeper.pubkey()).await;
    let buy_owner_start = lamports(&mut context, fixture.buy_owner.pubkey()).await;
    let sell_owner_start = lamports(&mut context, fixture.sell_owner.pubkey()).await;

    // Init: a wrong policy identity in the intent refuses before creation.
    let mut bad_init = fixture.init(payer, plane);
    let good_bytes = bad_init.data.clone();
    bad_init.data = direct_v3(
        0,
        DirectV3Intent::InitEpoch {
            market: fixture.market,
            epoch_index: EPOCH_INDEX,
            policy: h(0x82),
            submission_opens_slot: OPEN_SLOT,
            submission_closes_slot: CLOSE_SLOT,
            selection_deadline_slot: SELECTION_DEADLINE,
            settlement_deadline_slot: SETTLEMENT_DEADLINE,
            neutral_lamport_sink: Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()),
        },
    );
    let refused = send(&mut context, bad_init, None).await;
    assert_eq!(custom(refused.0), ClutchError::MismatchedState as u32);
    assert_eq!(
        account(&mut context, epoch_address).await.unwrap().owner,
        SYSTEM_PROGRAM
    );
    let mut init = fixture.init(payer, plane);
    init.data = good_bytes;
    let (result, init_cu) = send(&mut context, init, None).await;
    result.unwrap();
    assert_cu("InitDirectEpochV4", init_cu);
    let epoch_account = account(&mut context, epoch_address).await.unwrap();
    assert_eq!(epoch_account.owner, PROGRAM_ID);
    // The one-lamport prefund never became payer principal.
    assert_eq!(
        epoch_account.lamports,
        rent_exempt(DIRECT_EPOCH_V4_BYTES) + 1
    );
    let epoch_state = DirectEpochV4Account::decode(&epoch_account.data).unwrap();
    assert_eq!(
        epoch_state.lifecycle_phase,
        DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN
    );
    assert_eq!(epoch_state.epoch_funding.prior_donation_lamports, 1);
    assert_eq!(
        epoch_state.epoch_funding.payer_principal_lamports,
        rent_exempt(DIRECT_EPOCH_V4_BYTES)
    );

    // Predecessor regression, direction one: the legacy eight-account
    // PlaceOrder shape refuses against V4 state before any mutation.
    let legacy_shape = fixture.place_legacy_shape(plane, fixture.order(plane, 1, 0, BUY_LIMIT));
    let refused = send(&mut context, legacy_shape, Some(&fixture.buy_owner)).await;
    assert_eq!(custom(refused.0), ClutchError::AccountCount as u32);

    // Page zero of one, funded through the Epoch's page ledger.
    let (result, page_cu) = send(&mut context, fixture.init_page(payer, plane), None).await;
    result.unwrap();
    assert_cu("InitOrderPageV4", page_cu);
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, epoch_address).await).unwrap();
    assert_eq!(epoch_state.direct.common.page_count, 1);
    assert_eq!(
        epoch_state.page_funding.payer_principal_lamports,
        rent_exempt(account_len::ORDER_PAGE)
    );
    assert_eq!(epoch_state.page_funding.prior_donation_lamports, 1);

    // Zero-envelope refusal: a zero-limit zero-fee buy reserves nothing.
    let refused = send(
        &mut context,
        fixture.place(
            plane,
            0,
            fixture.order(plane, 1, 0, 0),
            fixture.plane(plane).buy_reservation,
        ),
        Some(&fixture.buy_owner),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::MismatchedState as u32);

    // Two funded placements create the exact Reservation V2 pair.
    let (result, place_buy_cu) = send(
        &mut context,
        fixture.place(
            plane,
            0,
            fixture.order(plane, 1, 0, BUY_LIMIT),
            fixture.plane(plane).buy_reservation,
        ),
        Some(&fixture.buy_owner),
    )
    .await;
    result.unwrap();
    assert_cu("PlaceOrderV4 buy", place_buy_cu);
    let (result, place_sell_cu) = send(
        &mut context,
        fixture.place(
            plane,
            1,
            fixture.order(plane, 2, 1, SELL_LIMIT),
            fixture.plane(plane).sell_reservation,
        ),
        Some(&fixture.sell_owner),
    )
    .await;
    result.unwrap();
    assert_cu("PlaceOrderV4 sell", place_sell_cu);
    // Placement replay refuses on the page's own order count.
    let refused = send(
        &mut context,
        fixture.place(
            plane,
            1,
            fixture.order(plane, 2, 1, SELL_LIMIT),
            fixture.plane(plane).sell_reservation,
        ),
        Some(&fixture.sell_owner),
    )
    .await;
    assert!(refused.0.is_err());
    let buyer =
        PositionAccount::decode(&bytes(&mut context, fixture.buyer_position).await).unwrap();
    assert_eq!(buyer.cash_atoms, BUYER_CASH);
    assert_eq!(
        buyer.reserved_cash_atoms,
        QUANTITY * BUY_LIMIT / PRICE_SCALE
    );
    let seller =
        PositionAccount::decode(&bytes(&mut context, fixture.seller_position).await).unwrap();
    assert_eq!(seller.internal[0], 0);
    let reservation = DirectReservationV2Account::decode(
        &bytes(&mut context, fixture.plane(plane).buy_reservation).await,
        Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()),
    )
    .unwrap();
    assert_eq!(reservation.reservation.state, RESERVATION_STATE_ACTIVE);
    assert_eq!(reservation.funding.prior_donation_lamports, 1);
    assert_eq!(
        reservation.funding.payer_principal_lamports,
        rent_exempt(DIRECT_RESERVATION_V2_BYTES)
    );

    // Freeze under an insufficiently funded sponsor rolls back after the
    // staged page/epoch writes: byte-and-lamport late-failure rollback.
    let watched = [
        epoch_address,
        fixture.plane(plane).page,
        fixture.plane(plane).buy_reservation,
        fixture.plane(plane).sell_reservation,
        fixture.plane(plane).work,
    ];
    let before = snapshot(&mut context, &watched).await;
    let refused = send(
        &mut context,
        fixture.freeze(fixture.low_funder.pubkey(), plane),
        Some(&fixture.low_funder),
    )
    .await;
    assert!(refused.0.is_err());
    assert_eq!(snapshot(&mut context, &watched).await, before);
    note_rollback("FreezeDirectEpochV4 underfunded", watched.len());

    let (result, freeze_cu) = send(
        &mut context,
        fixture.freeze(fixture.sponsor.pubkey(), plane),
        Some(&fixture.sponsor),
    )
    .await;
    result.unwrap();
    assert_cu("FreezeDirectEpochV4", freeze_cu);
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, epoch_address).await).unwrap();
    assert_eq!(epoch_state.direct.common.phase, EPOCH_PHASE_FROZEN);
    assert_eq!(
        epoch_state.lifecycle_phase,
        DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY
    );
    let work_account = account(&mut context, fixture.plane(plane).work)
        .await
        .unwrap();
    assert_eq!(work_account.owner, PROGRAM_ID);
    assert_eq!(
        work_account.lamports,
        rent_exempt(DIRECT_WORK_BUDGET_BYTES) + REWARD_DEPOSIT + 1
    );
    let sink_hash = Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes());
    let budget = DirectWorkBudgetV1Account::decode(&work_account.data, sink_hash).unwrap();
    assert_eq!(budget.reward_balance, REWARD_DEPOSIT);
    assert_eq!(budget.funding.prior_donation_lamports, 1);
    // Freeze replay refuses fail-closed: the WorkBudget target exists.
    let refused = send(
        &mut context,
        fixture.freeze(fixture.sponsor.pubkey(), plane),
        Some(&fixture.sponsor),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::AlreadyInitialized as u32);

    // Staged work with no Window yet fail-closes on account shape.
    context.warp_to_slot(OPEN_SLOT).unwrap();
    let refused = send(
        &mut context,
        fixture.begin(fixture.keeper.pubkey(), plane),
        Some(&fixture.keeper),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::WrongProgramOwner as u32);

    // Five-candidate campaign: retained admissions, a late-failure
    // rollback, a bounded replacement, an explicit no-state rejection, and
    // admitted-tick replay, with every disposition cross-checked against the
    // exact verifier scores the model ranks with.
    let mut expected_top: Vec<(Identity32V1, FullScoreV1, u64)> = Vec::new();
    let mut expected_transcript = Identity32V1::ZERO;
    let mut expected_admissions = 0u8;
    let mut expected_bitmap = 0u64;
    let mut max_submit_cu = 0u64;

    let candidate_for = |price: u64| {
        let mut prices = [0; MAX_OUTCOMES];
        prices[0] = price;
        prices[1] = PRICE_SCALE - price;
        canonical_account_candidate_id(
            Identity32V1(fixture.plane(plane).epoch_id.bytes()),
            Identity32V1(fixture.market.bytes()),
            &prices,
        )
    };
    let score_for = |price: u64| {
        let (_, bump) = pda(
            seeds::SEED_DIRECT_CANDIDATE_V3,
            &[
                &fixture.plane(plane).epoch_id.bytes(),
                &candidate_for(price).0,
            ],
        );
        expected_candidate(&epoch_state, price, OPEN_SLOT, bump)
    };

    // Steps 1, 2, 4: retained admissions at 2000, 5000, 4000.
    for (step, price) in [(0u32, 2_000u64), (4, 5_000), (5, 4_000)] {
        let retained: Vec<Address> = expected_top
            .iter()
            .map(|(id, _, _)| fixture.candidate_address(plane, *id))
            .collect();
        // Step 5 is preceded by the late-failure rollback: an underfunded
        // payer fails the candidate-create CPI after the staged epoch and
        // window writes, and every byte and lamport rolls back.
        if price == 4_000 {
            let watched = [
                epoch_address,
                fixture.plane(plane).window,
                retained[0],
                retained[1],
            ];
            let before = snapshot(&mut context, &watched).await;
            let refused = send(
                &mut context,
                fixture.submit(
                    fixture.low_funder.pubkey(),
                    plane,
                    price,
                    fixture.candidate_address(plane, candidate_for(price)),
                    &retained,
                    None,
                ),
                Some(&fixture.low_funder),
            )
            .await;
            assert!(refused.0.is_err());
            assert_eq!(snapshot(&mut context, &watched).await, before);
            note_rollback("SubmitDirectCandidateV3 underfunded", watched.len());
        }
        let candidate = score_for(price);
        let candidate_id = candidate_for(price);
        assert_eq!(candidate.candidate_id, candidate_id);
        let (result, units) = send_nonced(
            &mut context,
            fixture.submit(
                fixture.submitter.pubkey(),
                plane,
                price,
                fixture.candidate_address(plane, candidate_id),
                &retained,
                None,
            ),
            Some(&fixture.submitter),
            step,
        )
        .await;
        result.unwrap_or_else(|error| panic!("submit at {price} refused: {error:?}"));
        assert_cu(&format!("SubmitDirectCandidateV3 price={price}"), units);
        max_submit_cu = max_submit_cu.max(units);
        let tick = (price / 1_000) as u8;
        expected_top.push((candidate_id, candidate.score(), price));
        expected_top.sort_by(|a, b| b.1.total_order(&a.1));
        expected_admissions += 1;
        expected_bitmap |= 1 << tick;
        expected_transcript = admission_transcript_v3(
            expected_transcript,
            expected_admissions,
            tick,
            candidate.entry(),
        );
        let created = account(&mut context, fixture.candidate_address(plane, candidate_id))
            .await
            .unwrap();
        assert_eq!(created.owner, PROGRAM_ID);
        assert_eq!(created.lamports, rent_exempt(DIRECT_CANDIDATE_V3_BYTES) + 1);
        assert_eq!(created.data[425], DIRECT_CANDIDATE_STATUS_VERIFIED);
    }

    // Step 5: 6000 must displace the current worst; the former worst account
    // closes and its exact principal returns to its payer.
    let retained: Vec<Address> = expected_top
        .iter()
        .map(|(id, _, _)| fixture.candidate_address(plane, *id))
        .collect();
    let replacement = score_for(6_000);
    assert!(
        replacement.score().is_better_than(&expected_top[2].1),
        "6000 must be competitive against the worst of the top three"
    );
    let displaced = expected_top[2];
    let submitter_before = lamports(&mut context, fixture.submitter.pubkey()).await;
    // Close evidence: the displaced candidate's whole balance leaves in the
    // admitting transaction; the new candidate's principal leaves with it, so
    // the funding payer is exactly neutral and the sink keeps the donation.
    let displaced_address = fixture.candidate_address(plane, displaced.0);
    let admitted_address = fixture.candidate_address(plane, candidate_for(6_000));
    note_close(
        "SubmitDirectCandidateV3 displacing",
        "direct.candidate.v3.displaced",
        live_principal(&mut context, displaced_address)
            .await
            .expect("the displaced candidate is live before the route"),
    );
    let displacing_labels = [
        "direct.candidate.v3.displaced",
        "direct.candidate.v3.admitted",
        "submitter",
        "neutral_sink",
    ];
    let displacing_watch = [
        displaced_address,
        admitted_address,
        fixture.submitter.pubkey(),
        sink_address(),
    ];
    let displacing_before = wallets(&mut context, &displacing_watch).await;
    let (result, units) = send(
        &mut context,
        fixture.submit(
            fixture.submitter.pubkey(),
            plane,
            6_000,
            fixture.candidate_address(plane, candidate_for(6_000)),
            &retained,
            Some(fixture.submitter.pubkey()),
        ),
        Some(&fixture.submitter),
    )
    .await;
    result.unwrap();
    assert_cu("SubmitDirectCandidateV3 price=6000 replacement", units);
    max_submit_cu = max_submit_cu.max(units);
    assert!(account(&mut context, displaced_address).await.is_none());
    // Replacement is lamport-neutral for the payer that funded both: the
    // displaced principal came back while the new principal went out.
    assert_eq!(
        lamports(&mut context, fixture.submitter.pubkey()).await,
        submitter_before
    );
    // Close exactness across the same transaction: everything the displaced
    // account held reappears on its recorded payer, on the admitted
    // candidate the same payer funded, or in the neutral sink.
    let displacing_after = wallets(&mut context, &displacing_watch).await;
    assert_zero_sum(
        "SubmitDirectCandidateV3 displacing",
        refund_total(
            "SubmitDirectCandidateV3 displacing",
            &displacing_labels,
            &displacing_before,
            &displacing_after,
        ),
    );
    expected_top.pop();
    expected_top.push((candidate_for(6_000), replacement.score(), 6_000));
    expected_top.sort_by(|a, b| b.1.total_order(&a.1));
    expected_admissions += 1;
    expected_bitmap |= 1 << 6;
    expected_transcript = admission_transcript_v3(
        expected_transcript,
        expected_admissions,
        6,
        replacement.entry(),
    );

    // Step 6: 3000 is valid but cannot beat the worst retained score. The
    // explicit no-state outcome succeeds and changes nothing.
    let retained: Vec<Address> = expected_top
        .iter()
        .map(|(id, _, _)| fixture.candidate_address(plane, *id))
        .collect();
    assert!(
        !score_for(3_000).score().is_better_than(&expected_top[2].1),
        "3000 must be noncompetitive against the retained top three"
    );
    let watched = [
        epoch_address,
        fixture.plane(plane).window,
        retained[0],
        retained[1],
        retained[2],
    ];
    let before = snapshot(&mut context, &watched).await;
    let (result, units) = send(
        &mut context,
        fixture.submit(
            fixture.submitter.pubkey(),
            plane,
            3_000,
            fixture.candidate_address(plane, candidate_for(3_000)),
            &retained,
            Some(fixture.submitter.pubkey()),
        ),
        Some(&fixture.submitter),
    )
    .await;
    result.unwrap();
    assert_cu("SubmitDirectCandidateV3 price=3000 noncompetitive", units);
    max_submit_cu = max_submit_cu.max(units);
    // The prefunded target stays an untouched System-owned one-lamport slot:
    // it is not captured by the submitter, keeper, or protocol.
    let rejected_target = account(
        &mut context,
        fixture.candidate_address(plane, candidate_for(3_000)),
    )
    .await
    .unwrap();
    assert_eq!(rejected_target.owner, SYSTEM_PROGRAM);
    assert_eq!(rejected_target.lamports, 1);
    assert!(rejected_target.data.is_empty());
    assert_eq!(snapshot(&mut context, &watched).await, before);

    // The window commits exactly the model bookkeeping: competitive bitmap,
    // competitive admission count, the exported transcript fold, and the
    // score-ordered retained prefix.
    let window = DirectWindowV3Account::decode(
        &bytes(&mut context, fixture.plane(plane).window).await,
        sink_hash,
    )
    .unwrap();
    assert_eq!(window.seen_competitive_ticks, expected_bitmap);
    assert_eq!(window.window.admitted_count, u64::from(expected_admissions));
    assert_eq!(window.window.admission_transcript, expected_transcript);
    assert_eq!(usize::from(window.window.top_count), 3);
    for (index, (id, _, _)) in expected_top.iter().enumerate() {
        assert_eq!(window.window.top[index].candidate_id, *id, "top {index}");
    }

    // Admitted-tick replay refuses. A currently retained tick already fails
    // on the aliased Candidate account; the displaced tick, whose account no
    // longer exists, refuses on the persistent competitive bitmap.
    let refused = send_nonced(
        &mut context,
        fixture.submit(
            fixture.submitter.pubkey(),
            plane,
            expected_top[0].2,
            fixture.candidate_address(plane, candidate_for(expected_top[0].2)),
            &retained,
            Some(fixture.submitter.pubkey()),
        ),
        Some(&fixture.submitter),
        1,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::AccountAlias as u32);
    let refused = send_nonced(
        &mut context,
        fixture.submit(
            fixture.submitter.pubkey(),
            plane,
            displaced.2,
            fixture.candidate_address(plane, candidate_for(displaced.2)),
            &retained,
            Some(fixture.submitter.pubkey()),
        ),
        Some(&fixture.submitter),
        2,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        ClutchError::Replay as u32,
        "displaced tick replay"
    );

    // Donation observation: an unsolicited transfer to the Window makes the
    // no-state rejection refuse, because nothing could persist the higher
    // monotone bound; Begin later persists it.
    let donation_ix =
        solana_system_interface::instruction::transfer(&payer, &fixture.plane(plane).window, 7);
    let (result, _) = send(&mut context, donation_ix, None).await;
    result.unwrap();
    assert!(!score_for(7_000).score().is_better_than(&expected_top[2].1));
    let refused = send(
        &mut context,
        fixture.submit(
            fixture.submitter.pubkey(),
            plane,
            7_000,
            fixture.candidate_address(plane, candidate_for(7_000)),
            &retained,
            Some(fixture.submitter.pubkey()),
        ),
        Some(&fixture.submitter),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::MismatchedState as u32);

    // Early staged work refuses on time while submissions remain open.
    let refused = send_nonced(
        &mut context,
        fixture.begin(fixture.keeper.pubkey(), plane),
        Some(&fixture.keeper),
        1,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::NotActive as u32);

    assert_cu("SubmitDirectCandidateV3 max", max_submit_cu);

    // Staged verification across the selection window.
    context.warp_to_slot(CLOSE_SLOT).unwrap();
    // Submissions after close refuse.
    let refused = send_nonced(
        &mut context,
        fixture.submit(
            fixture.submitter.pubkey(),
            plane,
            8_000,
            fixture.candidate_address(plane, candidate_for(8_000)),
            &retained,
            Some(fixture.submitter.pubkey()),
        ),
        Some(&fixture.submitter),
        3,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::NotActive as u32);

    let (result, begin_cu) = send(
        &mut context,
        fixture.begin(fixture.keeper.pubkey(), plane),
        Some(&fixture.keeper),
    )
    .await;
    result.unwrap();
    assert_cu("BeginDirectVerificationV3", begin_cu);
    let window = DirectWindowV3Account::decode(
        &bytes(&mut context, fixture.plane(plane).window).await,
        sink_hash,
    )
    .unwrap();
    // The unsolicited 7 lamports are now a persisted monotone lower bound on
    // top of the one-lamport prefund donation.
    assert_eq!(window.funding.prior_donation_lamports, 1 + 7);
    let refused = send_nonced(
        &mut context,
        fixture.begin(fixture.keeper.pubkey(), plane),
        Some(&fixture.keeper),
        1,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::NotActive as u32);

    // Finalize before the mask completes refuses.
    let (receipt_address, _) = pda(
        seeds::SEED_DIRECT_RECEIPT_V3,
        &[
            &fixture.plane(plane).epoch_id.bytes(),
            &expected_top[0].0 .0,
            &0u16.to_le_bytes(),
        ],
    );
    let (pot_address, _) = pda(
        seeds::SEED_DIRECT_POT_V3,
        &[
            &fixture.plane(plane).epoch_id.bytes(),
            &expected_top[0].0 .0,
        ],
    );
    context.set_account(&receipt_address, &system_slot(1).into());
    context.set_account(&pot_address, &system_slot(1).into());
    let loser_payers = vec![fixture.submitter.pubkey(); 2];
    let refused = send(
        &mut context,
        fixture.finalize(
            fixture.keeper.pubkey(),
            plane,
            &retained,
            receipt_address,
            pot_address,
            &loser_payers,
        ),
        Some(&fixture.keeper),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::MismatchedState as u32);

    let mut max_verify_cu = 0u64;
    for index in 0..3u8 {
        let (result, units) = send(
            &mut context,
            fixture.verify(
                fixture.keeper.pubkey(),
                plane,
                index,
                retained[usize::from(index)],
            ),
            Some(&fixture.keeper),
        )
        .await;
        result.unwrap_or_else(|error| panic!("verify {index} refused: {error:?}"));
        assert_cu(&format!("VerifyDirectCandidateV3 index={index}"), units);
        max_verify_cu = max_verify_cu.max(units);
        let candidate_account = bytes(&mut context, retained[usize::from(index)]).await;
        assert_eq!(candidate_account[425], DIRECT_CANDIDATE_STATUS_REVERIFIED);
    }
    // Verification replay refuses on the mask.
    let refused = send_nonced(
        &mut context,
        fixture.verify(fixture.keeper.pubkey(), plane, 0, retained[0]),
        Some(&fixture.keeper),
        1,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::Replay as u32);
    // A candidate account substituted at the wrong retained index refuses.
    let refused = send_nonced(
        &mut context,
        fixture.verify(fixture.keeper.pubkey(), plane, 0, retained[1]),
        Some(&fixture.keeper),
        2,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::Replay as u32);

    // Recipient substitution at Finalize refuses: a loser's principal can
    // return only to its recorded payer.  This is close-route rollback
    // evidence, not merely a refusal code: the two candidate accounts the
    // route would have closed are byte-and-lamport identical afterwards.
    let wrong_payers = vec![fixture.keeper.pubkey(); 2];
    let close_watch = [retained[1], retained[2], receipt_address, pot_address];
    let close_before = snapshot(&mut context, &close_watch).await;
    let refused = send_nonced(
        &mut context,
        fixture.finalize(
            fixture.keeper.pubkey(),
            plane,
            &retained,
            receipt_address,
            pot_address,
            &wrong_payers,
        ),
        Some(&fixture.keeper),
        1,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::MismatchedState as u32);
    assert_eq!(snapshot(&mut context, &close_watch).await, close_before);
    note_rollback(
        "FinalizeDirectSelectionV3 wrong close recipient",
        close_watch.len(),
    );

    // Close evidence, Finalize: the two unselected candidates close to their
    // recorded payer while the receipt and pot are created, so the route is
    // stated as a closed system over every account it can move.
    for (index, loser) in retained[1..].iter().enumerate() {
        note_close(
            "FinalizeDirectSelectionV3",
            &format!("direct.candidate.v3.loser{index}"),
            live_principal(&mut context, *loser)
                .await
                .expect("a retained loser is live before Finalize"),
        );
    }
    let finalize_labels = [
        "direct.candidate.v3.loser0",
        "direct.candidate.v3.loser1",
        "direct.receipt",
        "direct.final_pot",
        "direct.work_budget.v1",
        "direct.window.v3",
        "direct.epoch.v4",
        "submitter",
        "sponsor",
        "keeper",
        "neutral_sink",
    ];
    let finalize_watch = [
        retained[1],
        retained[2],
        receipt_address,
        pot_address,
        fixture.plane(plane).work,
        fixture.plane(plane).window,
        epoch_address,
        fixture.submitter.pubkey(),
        fixture.sponsor.pubkey(),
        fixture.keeper.pubkey(),
        sink_address(),
    ];
    let finalize_before = wallets(&mut context, &finalize_watch).await;
    let (result, finalize_cu) = send_nonced(
        &mut context,
        fixture.finalize(
            fixture.keeper.pubkey(),
            plane,
            &retained,
            receipt_address,
            pot_address,
            &loser_payers,
        ),
        Some(&fixture.keeper),
        2,
    )
    .await;
    result.unwrap();
    assert_cu("FinalizeDirectSelectionV3", finalize_cu);
    let finalize_after = wallets(&mut context, &finalize_watch).await;
    assert_zero_sum(
        "FinalizeDirectSelectionV3",
        refund_total(
            "FinalizeDirectSelectionV3",
            &finalize_labels,
            &finalize_before,
            &finalize_after,
        ),
    );
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, epoch_address).await).unwrap();
    assert_eq!(epoch_state.lifecycle_phase, DIRECT_LIFECYCLE_PHASE_SELECTED);
    assert_eq!(epoch_state.direct.common.phase, EPOCH_PHASE_CLEARED);
    assert_eq!(epoch_state.terminal.candidate.bytes(), expected_top[0].0 .0);
    assert_eq!(epoch_state.terminal.selected_slot, CLOSE_SLOT);
    for loser in &retained[1..] {
        assert!(account(&mut context, *loser).await.is_none());
    }
    let winner_bytes_now = bytes(&mut context, retained[0]).await;
    assert_eq!(winner_bytes_now[425], DIRECT_CANDIDATE_STATUS_SELECTED);
    for reservation in [
        fixture.plane(plane).buy_reservation,
        fixture.plane(plane).sell_reservation,
    ] {
        let value =
            DirectReservationV2Account::decode(&bytes(&mut context, reservation).await, sink_hash)
                .unwrap();
        assert_eq!(value.reservation.state, RESERVATION_STATE_ENTITLED);
    }

    // Exact settlement with the model-anchored Position-transfer numbers.
    let winner_price = expected_top[0].2;
    let consideration = QUANTITY * winner_price / PRICE_SCALE;
    assert_eq!(consideration, winner_price, "quantity == scale anchoring");
    // Close evidence, Settle: seven transients close in one transaction.
    // Every lamport they hold must reappear on a recorded payer, on the
    // keeper as the frozen settle reward, or in the frozen neutral sink.
    let settle_close_labels = [
        "direct.window.v3",
        "direct.work_budget.v1",
        "direct.reservation.v2.buy",
        "direct.reservation.v2.sell",
        "direct.candidate.v3.winner",
        "direct.receipt",
        "direct.final_pot",
    ];
    let settle_closes = [
        fixture.plane(plane).window,
        fixture.plane(plane).work,
        fixture.plane(plane).buy_reservation,
        fixture.plane(plane).sell_reservation,
        retained[0],
        receipt_address,
        pot_address,
    ];
    let mut settle_closed_total = 0u64;
    for (index, address) in settle_closes.iter().enumerate() {
        let held = live_principal(&mut context, *address)
            .await
            .unwrap_or_else(|| panic!("{} is live before Settle", settle_close_labels[index]));
        note_close("SettleDirectV3", settle_close_labels[index], held);
        settle_closed_total += held;
    }
    let settle_labels = [
        "submitter",
        "sponsor",
        "keeper",
        "buy_owner",
        "sell_owner",
        "neutral_sink",
    ];
    let settle_watch = [
        fixture.submitter.pubkey(),
        fixture.sponsor.pubkey(),
        fixture.keeper.pubkey(),
        fixture.buy_owner.pubkey(),
        fixture.sell_owner.pubkey(),
        sink_address(),
    ];
    let settle_before = wallets(&mut context, &settle_watch).await;
    let (result, settle_cu) = send(
        &mut context,
        fixture.settle(
            fixture.keeper.pubkey(),
            plane,
            retained[0],
            receipt_address,
            pot_address,
            fixture.submitter.pubkey(),
            fixture.keeper.pubkey(),
        ),
        Some(&fixture.keeper),
    )
    .await;
    result.unwrap();
    assert_cu("SettleDirectV3", settle_cu);
    let settle_after = wallets(&mut context, &settle_watch).await;
    assert_conserved(
        "SettleDirectV3",
        settle_closed_total,
        refund_total(
            "SettleDirectV3",
            &settle_labels,
            &settle_before,
            &settle_after,
        ),
    );

    let buyer =
        PositionAccount::decode(&bytes(&mut context, fixture.buyer_position).await).unwrap();
    assert_eq!(buyer.cash_atoms, BUYER_CASH - consideration);
    assert_eq!(buyer.reserved_cash_atoms, 0);
    assert_eq!(buyer.internal[0], QUANTITY);
    let seller =
        PositionAccount::decode(&bytes(&mut context, fixture.seller_position).await).unwrap();
    assert_eq!(seller.cash_atoms, consideration);
    assert_eq!(seller.internal[0], 0);

    // Durable Epoch receipt decodes after every transient authority is gone.
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, epoch_address).await).unwrap();
    assert_eq!(epoch_state.lifecycle_phase, DIRECT_LIFECYCLE_PHASE_TERMINAL);
    assert_eq!(epoch_state.direct.common.phase, EPOCH_PHASE_SETTLED);
    assert_eq!(epoch_state.terminal.reason, DIRECT_TERMINAL_REASON_SETTLED);
    assert_eq!(epoch_state.terminal.quantity, QUANTITY);
    assert_eq!(epoch_state.terminal.price, winner_price);
    assert_eq!(
        epoch_state.terminal.consideration_price_units,
        u128::from(QUANTITY) * u128::from(winner_price)
    );
    assert_eq!(epoch_state.terminal.terminal_reservation_count, 2);
    for closed in [
        fixture.plane(plane).window,
        fixture.plane(plane).work,
        fixture.plane(plane).buy_reservation,
        fixture.plane(plane).sell_reservation,
        retained[0],
        receipt_address,
        pot_address,
    ] {
        assert!(
            account(&mut context, closed).await.is_none(),
            "transient {closed} must close"
        );
    }
    // The two families with no close handler at all: recorded, not refunded.
    note_strand(
        "SettleDirectV3",
        "direct.epoch.v4",
        lamports(&mut context, epoch_address).await,
    );
    note_strand(
        "SettleDirectV3",
        "artifact.direct_batch_policy_v3.final",
        lamports(&mut context, fixture.plane(plane).policy).await,
    );
    note_strand(
        "SettleDirectV3",
        "order.page",
        lamports(&mut context, fixture.plane(plane).page).await,
    );
    // Settle replay refuses: the transient authority no longer decodes.
    let refused = send_nonced(
        &mut context,
        fixture.settle(
            fixture.keeper.pubkey(),
            plane,
            retained[0],
            receipt_address,
            pot_address,
            fixture.submitter.pubkey(),
            fixture.keeper.pubkey(),
        ),
        Some(&fixture.keeper),
        1,
    )
    .await;
    assert!(refused.0.is_err());

    // Exact rent and reward conservation per keypair (none pays fees here).
    let rewards_paid = REWARDS.begin_verification
        + 3 * REWARDS.verify_candidate
        + REWARDS.finalize_selection
        + REWARDS.settle;
    assert_eq!(
        lamports(&mut context, fixture.keeper.pubkey()).await,
        keeper_start + rewards_paid,
        "keeper earned exactly the frozen rewards"
    );
    assert_eq!(
        lamports(&mut context, fixture.sponsor.pubkey()).await,
        sponsor_start - rewards_paid,
        "sponsor lost exactly the paid rewards; rent and refund returned"
    );
    assert_eq!(
        lamports(&mut context, fixture.submitter.pubkey()).await,
        submitter_start,
        "submitter recovered every candidate and window principal exactly"
    );
    assert_eq!(
        lamports(&mut context, fixture.buy_owner.pubkey()).await,
        buy_owner_start,
        "buy owner recovered the reservation principal exactly"
    );
    assert_eq!(
        lamports(&mut context, fixture.sell_owner.pubkey()).await,
        sell_owner_start,
        "sell owner recovered the reservation principal exactly"
    );
}

/// Pre-freeze abort releases and closes the exact zero/one/two prefix, and a
/// V4-shaped placement refuses against every legacy epoch length.
#[tokio::test]
async fn direct_v3_prefreeze_abort_releases_every_prefix() {
    let (mut context, fixture) = start(&[7, 8, 9]).await;
    let payer = context.payer.pubkey();
    let sink_hash = Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes());

    for plane in 0..3usize {
        let (result, _) = send(&mut context, fixture.init(payer, plane), None).await;
        result.unwrap();
    }
    for plane in [1usize, 2] {
        let (result, _) = send(&mut context, fixture.init_page(payer, plane), None).await;
        result.unwrap();
    }
    // One buy on plane 1; a full pair on plane 2.
    let (result, _) = send(
        &mut context,
        fixture.place(
            1,
            0,
            fixture.order(1, 1, 0, BUY_LIMIT),
            fixture.plane(1).buy_reservation,
        ),
        Some(&fixture.buy_owner),
    )
    .await;
    result.unwrap();
    let (result, _) = send(
        &mut context,
        fixture.place(
            2,
            0,
            fixture.order(2, 1, 0, BUY_LIMIT),
            fixture.plane(2).buy_reservation,
        ),
        Some(&fixture.buy_owner),
    )
    .await;
    result.unwrap();
    let (result, _) = send(
        &mut context,
        fixture.place(
            2,
            1,
            fixture.order(2, 2, 1, SELL_LIMIT),
            fixture.plane(2).sell_reservation,
        ),
        Some(&fixture.sell_owner),
    )
    .await;
    result.unwrap();

    // Predecessor regression, direction two: the V4 placement shape refuses
    // against a legacy 344-byte Direct Epoch V3 account.
    let legacy_epoch = Address::new_from_array([0x9c; 32]);
    let mut legacy_bytes = vec![0u8; clutch_solana_layout::direct_selection::DIRECT_EPOCH_BYTES];
    legacy_bytes[0] = 7;
    context.set_account(
        &legacy_epoch,
        &Account {
            lamports: rent_exempt(legacy_bytes.len()),
            data: legacy_bytes,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }
        .into(),
    );
    let mut v4_place_on_legacy = fixture.place(
        1,
        1,
        fixture.order(1, 2, 1, SELL_LIMIT),
        fixture.plane(1).sell_reservation,
    );
    v4_place_on_legacy.accounts[1] = AccountMeta::new(legacy_epoch, false);
    let refused = send(&mut context, v4_place_on_legacy, Some(&fixture.sell_owner)).await;
    assert_eq!(custom(refused.0), ClutchError::AccountCount as u32);

    // The legacy per-order CancelOrder wire also cannot reach V4 state: its
    // epoch role admits only the legacy lengths, so a 672-byte V4 Epoch
    // refuses on data length. V4 has no per-order cancellation at all; the
    // bounded pre-freeze terminal route is the whole-epoch abort below.
    let cancel_on_v4 = Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            1,
            Intent::CancelOrder {
                market: fixture.market,
                epoch: fixture.plane(1).epoch_id,
                owner: Hash32::from_bytes(fixture.buy_owner.pubkey().to_bytes()),
                order_id: canonical_order_id(1),
                generation: 1,
            },
        ),
        vec![
            AccountMeta::new(fixture.buy_owner.pubkey(), true),
            AccountMeta::new_readonly(fixture.plane(1).epoch, false),
            AccountMeta::new(fixture.plane(1).page, false),
            AccountMeta::new(fixture.buyer_position, false),
            AccountMeta::new(fixture.plane(1).buy_reservation, false),
        ],
    );
    let refused = send(&mut context, cancel_on_v4, Some(&fixture.buy_owner)).await;
    assert_eq!(custom(refused.0), ClutchError::WrongDataLength as u32);

    // Abort before submission open refuses.
    let refused = send(&mut context, fixture.abort(0, false, &[], &[], &[]), None).await;
    assert_eq!(custom(refused.0), ClutchError::NotActive as u32);

    context.warp_to_slot(OPEN_SLOT).unwrap();
    // Late Freeze refuses once submission has opened.
    let refused = send(
        &mut context,
        fixture.freeze(fixture.sponsor.pubkey(), 2),
        Some(&fixture.sponsor),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::NotActive as u32);

    // Zero-reservation abort: three accounts, durable PREFREEZE_ABORT.
    let abort_labels = [
        "direct.epoch.v4",
        "order.page",
        "direct.reservation.v2.buy",
        "direct.reservation.v2.sell",
        "buy_owner",
        "sell_owner",
        "neutral_sink",
    ];
    let abort_watch = |plane: usize| {
        [
            fixture.plane(plane).epoch,
            fixture.plane(plane).page,
            fixture.plane(plane).buy_reservation,
            fixture.plane(plane).sell_reservation,
            fixture.buy_owner.pubkey(),
            fixture.sell_owner.pubkey(),
            sink_address(),
        ]
    };
    let abort0_watch = abort_watch(0);
    let abort0_before = wallets(&mut context, &abort0_watch).await;
    note_close("AbortUnfrozenDirectV4 empty", "reservation_prefix", 0);
    let (result, abort0_cu) =
        send(&mut context, fixture.abort(0, false, &[], &[], &[]), None).await;
    result.unwrap();
    assert_cu("AbortUnfrozenDirectV4 empty", abort0_cu);
    let abort0_after = wallets(&mut context, &abort0_watch).await;
    assert_zero_sum(
        "AbortUnfrozenDirectV4 empty",
        refund_total(
            "AbortUnfrozenDirectV4 empty",
            &abort_labels,
            &abort0_before,
            &abort0_after,
        ),
    );
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, fixture.plane(0).epoch).await).unwrap();
    assert_eq!(epoch_state.lifecycle_phase, DIRECT_LIFECYCLE_PHASE_TERMINAL);
    assert_eq!(
        epoch_state.terminal.reason,
        DIRECT_TERMINAL_REASON_PREFREEZE_ABORT
    );
    assert_eq!(epoch_state.terminal.terminal_reservation_count, 0);
    assert_eq!(epoch_state.direct.common.phase, EPOCH_PHASE_OPEN);
    // Abort replay refuses on terminal lifecycle.
    let refused = send_nonced(
        &mut context,
        fixture.abort(0, false, &[], &[], &[]),
        None,
        1,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::NotActive as u32);

    // One-reservation abort restores the buyer Position and refunds exactly.
    let buy_owner_before = lamports(&mut context, fixture.buy_owner.pubkey()).await;
    let abort1_watch = abort_watch(1);
    let abort1_before = wallets(&mut context, &abort1_watch).await;
    note_close(
        "AbortUnfrozenDirectV4 one",
        "direct.reservation.v2.buy",
        live_principal(&mut context, fixture.plane(1).buy_reservation)
            .await
            .expect("the one-order prefix is live before the abort"),
    );
    let (result, abort1_cu) = send(
        &mut context,
        fixture.abort(
            1,
            true,
            &[fixture.plane(1).buy_reservation],
            &[fixture.buyer_position],
            &[fixture.buy_owner.pubkey()],
        ),
        None,
    )
    .await;
    result.unwrap();
    assert_cu("AbortUnfrozenDirectV4 one", abort1_cu);
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, fixture.plane(1).epoch).await).unwrap();
    assert_eq!(epoch_state.terminal.terminal_reservation_count, 1);
    assert!(epoch_state.terminal.candidate != Hash32::ZERO);
    assert!(account(&mut context, fixture.plane(1).buy_reservation)
        .await
        .is_none());
    assert_eq!(
        lamports(&mut context, fixture.buy_owner.pubkey()).await,
        buy_owner_before + rent_exempt(DIRECT_RESERVATION_V2_BYTES)
    );
    let abort1_after = wallets(&mut context, &abort1_watch).await;
    assert_zero_sum(
        "AbortUnfrozenDirectV4 one",
        refund_total(
            "AbortUnfrozenDirectV4 one",
            &abort_labels,
            &abort1_before,
            &abort1_after,
        ),
    );

    // Two-reservation abort: recipient substitution refuses first, and the
    // prefix it would have closed rolls back byte-and-lamport identical.
    let abort_close_watch = [
        fixture.plane(2).buy_reservation,
        fixture.plane(2).sell_reservation,
        fixture.buyer_position,
        fixture.seller_position,
    ];
    let abort_close_before = snapshot(&mut context, &abort_close_watch).await;
    let refused = send(
        &mut context,
        fixture.abort(
            2,
            true,
            &[
                fixture.plane(2).buy_reservation,
                fixture.plane(2).sell_reservation,
            ],
            &[fixture.buyer_position, fixture.seller_position],
            &[fixture.sell_owner.pubkey(), fixture.buy_owner.pubkey()],
        ),
        None,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::MismatchedState as u32);
    assert_eq!(
        snapshot(&mut context, &abort_close_watch).await,
        abort_close_before
    );
    note_rollback(
        "AbortUnfrozenDirectV4 wrong close recipient",
        abort_close_watch.len(),
    );
    let abort2_watch = abort_watch(2);
    let abort2_before = wallets(&mut context, &abort2_watch).await;
    let mut abort2_closed = 0u64;
    for (label, address) in [
        (
            "direct.reservation.v2.buy",
            fixture.plane(2).buy_reservation,
        ),
        (
            "direct.reservation.v2.sell",
            fixture.plane(2).sell_reservation,
        ),
    ] {
        let held = live_principal(&mut context, address)
            .await
            .unwrap_or_else(|| panic!("{label} is live before the two-order abort"));
        note_close("AbortUnfrozenDirectV4 two", label, held);
        abort2_closed += held;
    }
    let (result, abort2_cu) = send(
        &mut context,
        fixture.abort(
            2,
            true,
            &[
                fixture.plane(2).buy_reservation,
                fixture.plane(2).sell_reservation,
            ],
            &[fixture.buyer_position, fixture.seller_position],
            &[fixture.buy_owner.pubkey(), fixture.sell_owner.pubkey()],
        ),
        None,
    )
    .await;
    result.unwrap();
    assert_cu("AbortUnfrozenDirectV4 two", abort2_cu);
    let abort2_after = wallets(&mut context, &abort2_watch).await;
    assert_zero_sum(
        "AbortUnfrozenDirectV4 two",
        refund_total(
            "AbortUnfrozenDirectV4 two",
            &abort_labels,
            &abort2_before,
            &abort2_after,
        ),
    );
    // Both owners recover exactly the frozen reservation principal; the
    // one-lamport prefund donation is the only lamport that moves elsewhere.
    assert_eq!(
        lamports(&mut context, fixture.buy_owner.pubkey()).await
            + lamports(&mut context, fixture.sell_owner.pubkey()).await
            - abort2_before[4]
            - abort2_before[5],
        2 * rent_exempt(DIRECT_RESERVATION_V2_BYTES)
    );
    assert_eq!(
        abort2_closed,
        2 * (rent_exempt(DIRECT_RESERVATION_V2_BYTES) + 1)
    );
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, fixture.plane(2).epoch).await).unwrap();
    assert_eq!(epoch_state.terminal.terminal_reservation_count, 2);
    // Every reservation envelope returned to its Position exactly.
    let buyer =
        PositionAccount::decode(&bytes(&mut context, fixture.buyer_position).await).unwrap();
    assert_eq!(buyer.cash_atoms, BUYER_CASH);
    assert_eq!(buyer.reserved_cash_atoms, 0);
    let seller =
        PositionAccount::decode(&bytes(&mut context, fixture.seller_position).await).unwrap();
    assert_eq!(seller.internal[0], QUANTITY * 3);
    assert_eq!(seller.cash_atoms, 0);
    let _ = sink_hash;
}

/// All three lapse phases write their distinct durable receipts and return
/// every transient principal exactly once.
#[tokio::test]
async fn direct_v3_lapse_covers_every_frozen_phase() {
    let (mut context, fixture) = start(&[7, 8, 9]).await;
    let payer = context.payer.pubkey();
    let sink_hash = Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes());
    let keeper_start = lamports(&mut context, fixture.keeper.pubkey()).await;

    // Freeze all three epochs with full pairs before submission opens.
    for plane in 0..3usize {
        let (result, _) = send(&mut context, fixture.init(payer, plane), None).await;
        result.unwrap();
        let (result, _) = send(&mut context, fixture.init_page(payer, plane), None).await;
        result.unwrap();
        let (result, _) = send(
            &mut context,
            fixture.place(
                plane,
                0,
                fixture.order(plane, 1, 0, BUY_LIMIT),
                fixture.plane(plane).buy_reservation,
            ),
            Some(&fixture.buy_owner),
        )
        .await;
        result.unwrap();
        let (result, _) = send(
            &mut context,
            fixture.place(
                plane,
                1,
                fixture.order(plane, 2, 1, SELL_LIMIT),
                fixture.plane(plane).sell_reservation,
            ),
            Some(&fixture.sell_owner),
        )
        .await;
        result.unwrap();
        let (result, _) = send(
            &mut context,
            fixture.freeze(fixture.sponsor.pubkey(), plane),
            Some(&fixture.sponsor),
        )
        .await;
        result.unwrap();
    }
    let buyer_frozen =
        PositionAccount::decode(&bytes(&mut context, fixture.buyer_position).await).unwrap();
    let seller_frozen =
        PositionAccount::decode(&bytes(&mut context, fixture.seller_position).await).unwrap();

    // One submission on planes 1 and 2.
    context.warp_to_slot(OPEN_SLOT).unwrap();
    let mut candidate_addresses = Vec::new();
    let mut winner_ids = Vec::new();
    for plane in [1usize, 2] {
        let mut prices = [0; MAX_OUTCOMES];
        prices[0] = 5_000;
        prices[1] = 5_000;
        let candidate_id = canonical_account_candidate_id(
            Identity32V1(fixture.plane(plane).epoch_id.bytes()),
            Identity32V1(fixture.market.bytes()),
            &prices,
        );
        let candidate_address = fixture.candidate_address(plane, candidate_id);
        let (result, _) = send(
            &mut context,
            fixture.submit(
                fixture.submitter.pubkey(),
                plane,
                5_000,
                candidate_address,
                &[],
                None,
            ),
            Some(&fixture.submitter),
        )
        .await;
        result.unwrap();
        candidate_addresses.push(candidate_address);
        winner_ids.push(candidate_id);
    }

    // Lapse before the deadline refuses.
    let refused = send(
        &mut context,
        fixture.lapse_empty(fixture.keeper.pubkey(), 0),
        Some(&fixture.keeper),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::NotActive as u32);

    // Select plane 2 through the staged path.
    context.warp_to_slot(CLOSE_SLOT).unwrap();
    let (result, _) = send(
        &mut context,
        fixture.begin(fixture.keeper.pubkey(), 2),
        Some(&fixture.keeper),
    )
    .await;
    result.unwrap();
    let (result, _) = send(
        &mut context,
        fixture.verify(fixture.keeper.pubkey(), 2, 0, candidate_addresses[1]),
        Some(&fixture.keeper),
    )
    .await;
    result.unwrap();
    let (receipt_address, _) = pda(
        seeds::SEED_DIRECT_RECEIPT_V3,
        &[
            &fixture.plane(2).epoch_id.bytes(),
            &winner_ids[1].0,
            &0u16.to_le_bytes(),
        ],
    );
    let (pot_address, _) = pda(
        seeds::SEED_DIRECT_POT_V3,
        &[&fixture.plane(2).epoch_id.bytes(), &winner_ids[1].0],
    );
    context.set_account(&receipt_address, &system_slot(1).into());
    context.set_account(&pot_address, &system_slot(1).into());
    let (result, _) = send(
        &mut context,
        fixture.finalize(
            fixture.keeper.pubkey(),
            2,
            &[candidate_addresses[1]],
            receipt_address,
            pot_address,
            &[],
        ),
        Some(&fixture.keeper),
    )
    .await;
    result.unwrap();

    // Empty and pre-selection lapses at the selection deadline.  Each lapse
    // is measured as a closed system: the transients it closes, the exact
    // deltas on every recorded payer, the keeper's frozen lapse reward, and
    // the neutral sink, summing to zero.
    let lapse_wallet_labels = [
        "submitter",
        "sponsor",
        "keeper",
        "buy_owner",
        "sell_owner",
        "neutral_sink",
    ];
    let lapse_wallets = [
        fixture.submitter.pubkey(),
        fixture.sponsor.pubkey(),
        fixture.keeper.pubkey(),
        fixture.buy_owner.pubkey(),
        fixture.sell_owner.pubkey(),
        sink_address(),
    ];
    let lapse_plane_labels = [
        "direct.epoch.v4",
        "order.page",
        "direct.window.v3",
        "direct.work_budget.v1",
        "direct.reservation.v2.buy",
        "direct.reservation.v2.sell",
    ];

    context.warp_to_slot(SELECTION_DEADLINE).unwrap();
    let mut empty_labels: Vec<&str> = lapse_plane_labels.to_vec();
    empty_labels.extend_from_slice(&lapse_wallet_labels);
    let mut empty_watch = vec![
        fixture.plane(0).epoch,
        fixture.plane(0).page,
        fixture.plane(0).window,
        fixture.plane(0).work,
        fixture.plane(0).buy_reservation,
        fixture.plane(0).sell_reservation,
    ];
    empty_watch.extend_from_slice(&lapse_wallets);
    let empty_before = wallets(&mut context, &empty_watch).await;
    for (index, label) in lapse_plane_labels.iter().enumerate().skip(2) {
        if let Some(held) = live_principal(&mut context, empty_watch[index]).await {
            note_close("LapseEmptyDirectV3", label, held);
        }
    }
    let (result, lapse_empty_cu) = send(
        &mut context,
        fixture.lapse_empty(fixture.keeper.pubkey(), 0),
        Some(&fixture.keeper),
    )
    .await;
    result.unwrap();
    assert_cu("LapseEmptyDirectV3", lapse_empty_cu);
    let empty_after = wallets(&mut context, &empty_watch).await;
    assert_zero_sum(
        "LapseEmptyDirectV3",
        refund_total(
            "LapseEmptyDirectV3",
            &empty_labels,
            &empty_before,
            &empty_after,
        ),
    );
    note_strand(
        "LapseEmptyDirectV3",
        "direct.epoch.v4",
        lamports(&mut context, fixture.plane(0).epoch).await,
    );
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, fixture.plane(0).epoch).await).unwrap();
    assert_eq!(
        epoch_state.terminal.reason,
        DIRECT_TERMINAL_REASON_EMPTY_LAPSE
    );
    assert_eq!(epoch_state.direct.common.phase, EPOCH_PHASE_LAPSED);

    let mut unselected_labels: Vec<&str> = lapse_plane_labels.to_vec();
    unselected_labels.push("direct.candidate.v3");
    unselected_labels.extend_from_slice(&lapse_wallet_labels);
    let mut unselected_watch = vec![
        fixture.plane(1).epoch,
        fixture.plane(1).page,
        fixture.plane(1).window,
        fixture.plane(1).work,
        fixture.plane(1).buy_reservation,
        fixture.plane(1).sell_reservation,
        candidate_addresses[0],
    ];
    unselected_watch.extend_from_slice(&lapse_wallets);
    let unselected_before = wallets(&mut context, &unselected_watch).await;
    for index in [2usize, 3, 4, 5, 6] {
        if let Some(held) = live_principal(&mut context, unselected_watch[index]).await {
            note_close("LapseUnselectedDirectV3", unselected_labels[index], held);
        }
    }
    let (result, lapse_unselected_cu) = send(
        &mut context,
        fixture.lapse_unselected(
            fixture.keeper.pubkey(),
            1,
            &[candidate_addresses[0]],
            &[fixture.submitter.pubkey()],
        ),
        Some(&fixture.keeper),
    )
    .await;
    result.unwrap();
    assert_cu("LapseUnselectedDirectV3", lapse_unselected_cu);
    let unselected_after = wallets(&mut context, &unselected_watch).await;
    assert_zero_sum(
        "LapseUnselectedDirectV3",
        refund_total(
            "LapseUnselectedDirectV3",
            &unselected_labels,
            &unselected_before,
            &unselected_after,
        ),
    );
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, fixture.plane(1).epoch).await).unwrap();
    assert_eq!(
        epoch_state.terminal.reason,
        DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE
    );

    // The selected plane refuses a post-selection lapse before its deadline.
    let refused = send(
        &mut context,
        fixture.lapse_selected(
            fixture.keeper.pubkey(),
            2,
            candidate_addresses[1],
            receipt_address,
            pot_address,
            fixture.submitter.pubkey(),
            fixture.keeper.pubkey(),
        ),
        Some(&fixture.keeper),
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::NotActive as u32);
    context.warp_to_slot(SETTLEMENT_DEADLINE).unwrap();
    let mut selected_labels: Vec<&str> = lapse_plane_labels.to_vec();
    selected_labels.extend_from_slice(&[
        "direct.candidate.v3",
        "direct.receipt",
        "direct.final_pot",
    ]);
    selected_labels.extend_from_slice(&lapse_wallet_labels);
    let mut selected_watch = vec![
        fixture.plane(2).epoch,
        fixture.plane(2).page,
        fixture.plane(2).window,
        fixture.plane(2).work,
        fixture.plane(2).buy_reservation,
        fixture.plane(2).sell_reservation,
        candidate_addresses[1],
        receipt_address,
        pot_address,
    ];
    selected_watch.extend_from_slice(&lapse_wallets);
    let selected_before = wallets(&mut context, &selected_watch).await;
    for index in [2usize, 3, 4, 5, 6, 7, 8] {
        if let Some(held) = live_principal(&mut context, selected_watch[index]).await {
            note_close("LapseSelectedDirectV3", selected_labels[index], held);
        }
    }
    let (result, lapse_selected_cu) = send_nonced(
        &mut context,
        fixture.lapse_selected(
            fixture.keeper.pubkey(),
            2,
            candidate_addresses[1],
            receipt_address,
            pot_address,
            fixture.submitter.pubkey(),
            fixture.keeper.pubkey(),
        ),
        Some(&fixture.keeper),
        1,
    )
    .await;
    result.unwrap();
    assert_cu("LapseSelectedDirectV3", lapse_selected_cu);
    let selected_after = wallets(&mut context, &selected_watch).await;
    assert_zero_sum(
        "LapseSelectedDirectV3",
        refund_total(
            "LapseSelectedDirectV3",
            &selected_labels,
            &selected_before,
            &selected_after,
        ),
    );
    let epoch_state =
        DirectEpochV4Account::decode(&bytes(&mut context, fixture.plane(2).epoch).await).unwrap();
    assert_eq!(
        epoch_state.terminal.reason,
        DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE
    );
    assert_eq!(epoch_state.terminal.candidate.bytes(), winner_ids[1].0);
    assert_eq!(epoch_state.terminal.selected_slot, CLOSE_SLOT);

    // Every trade was lapsed, never settled: both Positions return exactly to
    // their frozen prestates and all transient authority closed.
    let buyer =
        PositionAccount::decode(&bytes(&mut context, fixture.buyer_position).await).unwrap();
    let seller =
        PositionAccount::decode(&bytes(&mut context, fixture.seller_position).await).unwrap();
    assert_eq!(buyer.cash_atoms, buyer_frozen.cash_atoms);
    assert_eq!(buyer.reserved_cash_atoms, 0);
    assert_eq!(buyer.internal, [0; MAX_OUTCOMES]);
    assert_eq!(seller.cash_atoms, 0);
    assert_eq!(seller.internal[0], seller_frozen.internal[0] + QUANTITY * 3);
    for plane in 0..3usize {
        for closed in [
            fixture.plane(plane).work,
            fixture.plane(plane).buy_reservation,
            fixture.plane(plane).sell_reservation,
        ] {
            assert!(account(&mut context, closed).await.is_none());
        }
    }
    for closed in [
        fixture.plane(1).window,
        fixture.plane(2).window,
        candidate_addresses[0],
        candidate_addresses[1],
        receipt_address,
        pot_address,
    ] {
        assert!(account(&mut context, closed).await.is_none());
    }
    // Keeper earned exactly the staged and lapse rewards it performed.
    let keeper_rewards = REWARDS.begin_verification
        + REWARDS.verify_candidate
        + REWARDS.finalize_selection
        + 3 * REWARDS.lapse;
    assert_eq!(
        lamports(&mut context, fixture.keeper.pubkey()).await,
        keeper_start + keeper_rewards
    );
    // Every lapsed epoch strands exactly the two families with no close
    // handler: the durable Epoch V4 receipt and the epoch-addressed final
    // policy artifact.  Neither is refundable in this runtime.
    for plane in 0..3usize {
        note_strand(
            "lapse",
            "direct.epoch.v4",
            lamports(&mut context, fixture.plane(plane).epoch).await,
        );
        note_strand(
            "lapse",
            "artifact.direct_batch_policy_v3.final",
            lamports(&mut context, fixture.plane(plane).policy).await,
        );
    }
    let _ = sink_hash;
}
