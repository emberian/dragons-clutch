//! Real-SBF evidence for the disagreement exhibit — two models, one price
//! (docs/design/DISAGREEMENT_EXHIBIT_DESIGN_2026-08-20.md, artifact L2).
//!
//! Two named estimators' pinned integer belief vectors were lowered by the
//! published deterministic book-former into a 13-order book: ten crossing
//! single-Egg quotes at each model's own value, one 50-lot "disagreement
//! package" portfolio pair whose coefficients are the per-mille excess of
//! one belief over the other, and one uncrossed low-ball that ends
//! ineligible.  The book places through the general clearing plane (tags
//! 47-59), freezes, walks the pre-registered candidate
//! `p = [0, 120, 2840, 5780, 1240, 20, 0, 0]` to VERIFIED through the real
//! streaming relation, selects, entitles, and settles through Settle x6 —
//! five entitled single crossings plus the portfolio full pair — with the
//! whole value plane conserved and the pot provably empty.  Every number
//! asserted here is re-derived by docs/site-plan/disagreement_check.py.
//!
//! **T0 first** (`degree1_terms_admission_through_the_general_plane`): the
//! design requires confirming that the general plane admits a *degree-1 v3
//! terms* market — basis_degree 1, knot_count 8, u128 cent knots
//! 10,000..24,000, general spacing (`UNIFORM_SPACING_NONE`, admitted at
//! d=1), STAT-TERMINAL-01, EDGE-CLAMP-01, payout map entirely unused, a
//! uniform 8/64 failure refund preset — expected yes, because the relation
//! sees the basis only through partition of unity.  Degree-0 substitution
//! is forbidden (CURRENT_TRUTH.md §7's no-impersonation rule); if any seam
//! refuses degree-1 terms, that test fails and records the refusal.
//!
//! Genesis positions (cash 10,000 atoms and 16,000 Eggs per outcome per
//! model) are INJECTED laboratory bank state — no Endow/Split executes; the
//! deposit boundary stays refused (`0x79`) on the sealed default artifact.
//!
//! Claim plane: SBF-EXECUTED (bank), UNPROMOTED, fees zero, policy
//! `GENERAL_CLEARING_POLICY_V1` (frozen 2026-08-20).

use {
    clutch_batch::relation_v1::{
        canonical_candidate, LegRefV1, PairingSliceV1, PairingWitnessV1, RelationDomainV1,
    },
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
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
            entitlement::{
                ENTITLE_SLICE_FIXED_ACCOUNT_COUNT, FREEZE_ENTITLEMENT_ACCOUNT_COUNT,
                SETTLE_PAIR_FIXED_ACCOUNT_COUNT,
            },
            selection::{SEAL_CANDIDATE_FIXED_ACCOUNT_COUNT, SUBMIT_CANDIDATE_ACCOUNT_COUNT},
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        clearing::{CandidateFeedHeader, LegRef, PairingSlice, CANDIDATE_WINDOW_SLOTS},
        projection::{project_slot, OwnerInterner},
        reservation::{
            canonical_reservation_id, ReservationAccount, RESERVATION_STATE_ACTIVE,
            RESERVATION_STATE_CONSUMED,
        },
        stream, CandidateFeedChunk, CandidateRecord, EpochAccount, FinalPotAccount, Hash32,
        MarketAccount, OrderRecord, OrderSlot, PayoutVectorBytes, PortfolioRecord,
        PositionAccount, PriceGridAccount, SettlementReceiptAccount, CANDIDATE_STATUS_SELECTED,
        EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, FEED_FILLS_PER_CHUNK, FEED_SLICES_PER_CHUNK,
        MAX_GRID_TICKS, MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
        POT_PHASE_CLOSED, RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
        RECEIPT_FLAG_SLICE_EXHAUSTED, UNIFORM_SPACING_NONE,
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
/// Eight hats on the Friday-clutch grid; the exhibit's active width.
const OUTCOMES: u8 = 8;
/// Injected genesis: cash atoms per model position (stated: no Endow runs).
const START_CASH: u64 = 10_000;
/// Injected genesis: Eggs per outcome per model position (no Split runs).
const START_EGGS: u64 = 16_000;
/// Eggs per single-Egg quote (the book-former's z).
const Z: u64 = 500;
/// Lots on the disagreement package (the divisibility rule's L).
const LOTS: u64 = 50;

/// The pre-registered candidate on the active prefix.
const P: [u64; 8] = [0, 120, 2_840, 5_780, 1_240, 20, 0, 0];
/// The package coefficients c+ = (v_G - v_E)+ on the active prefix.
const C_PLUS: [u64; 8] = [0, 0, 299, 0, 0, 32, 0, 0];
/// The knot grid in u128 cents ($100..$240 step $20).
const KNOT_CENTS: [u128; 8] = [
    10_000, 12_000, 14_000, 16_000, 18_000, 20_000, 22_000, 24_000,
];
/// The 10-tick price grid: exactly the set of single-Egg limits.
const TICKS: [u64; 10] = [0, 32, 98, 127, 1_213, 1_266, 2_662, 2_961, 5_696, 5_945];

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
    virtual_split: u64,
    virtual_merge: u64,
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

    fn pot(&self) -> Address {
        pda(seeds::SEED_POT, &[&self.epoch_id.bytes()]).0
    }

    fn receipt(&self, candidate: Hash32, slice_index: u16) -> Address {
        pda(
            seeds::SEED_RECEIPT,
            &[
                &self.epoch_id.bytes(),
                &candidate.bytes(),
                &slice_index.to_le_bytes(),
            ],
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

    fn submit(
        &self,
        payer: Address,
        submission: &Submission,
        declared_slices: Option<u16>,
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
                    virtual_split: submission.virtual_split,
                    virtual_merge: submission.virtual_merge,
                    honored_aon_mask: 0,
                    declared_slices,
                    weighted_direct_volume: 0,
                    limit_surplus_price_units: 0,
                    distinct_owners: 0,
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
            vec![AccountMeta::new(submission.feed, false)],
        )
    }

    fn seal(&self, submission: &Submission, retained: &[Hash32]) -> Instruction {
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

    fn advance(&self, candidate: Hash32, max_orders: u16, reservations: &[Address]) -> Instruction {
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

    fn freeze_entitlement(&self, payer: Address, candidate: Hash32) -> Instruction {
        let metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new_readonly(self.clear_work(candidate), false),
            AccountMeta::new(self.pot(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), FREEZE_ENTITLEMENT_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::FreezeEntitlement {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            metas,
        )
    }

    /// The fixed prefix every `EntitleSlice` shares.
    fn entitle_prefix(&self, payer: Address, candidate: Hash32) -> Vec<AccountMeta> {
        let metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new_readonly(self.pot(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), ENTITLE_SLICE_FIXED_ACCOUNT_COUNT);
        metas
    }

    fn entitle_single(
        &self,
        payer: Address,
        candidate: Hash32,
        slice_index: u16,
        buy_reservation: Address,
        sell_reservation: Address,
    ) -> Instruction {
        let mut metas = self.entitle_prefix(payer, candidate);
        metas.push(AccountMeta::new_readonly(self.page, false));
        metas.push(AccountMeta::new(buy_reservation, false));
        metas.push(AccountMeta::new(sell_reservation, false));
        metas.push(AccountMeta::new(self.receipt(candidate, slice_index), false));
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::EntitleSlice {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                    slice_index,
                },
            ),
            metas,
        )
    }

    fn entitle_pair(
        &self,
        payer: Address,
        candidate: Hash32,
        slice_index: u16,
        buy_reservation: Address,
        sell_reservation: Address,
        receipt_slices: &[u16],
    ) -> Instruction {
        let mut metas = self.entitle_prefix(payer, candidate);
        metas.push(AccountMeta::new_readonly(self.page, false));
        metas.push(AccountMeta::new_readonly(self.terms_account, false));
        metas.push(AccountMeta::new(buy_reservation, false));
        metas.push(AccountMeta::new(sell_reservation, false));
        for slice in receipt_slices {
            metas.push(AccountMeta::new(self.receipt(candidate, *slice), false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::EntitleSlice {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                    slice_index,
                },
            ),
            metas,
        )
    }

    #[allow(clippy::too_many_arguments)] // one argument per account role
    fn settle_single(
        &self,
        candidate: Hash32,
        sequence: u64,
        buyer_position: Address,
        seller_position: Address,
        buy_reservation: Address,
        sell_reservation: Address,
        slice_index: u16,
    ) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new(buyer_position, false),
            AccountMeta::new(seller_position, false),
            AccountMeta::new(buy_reservation, false),
            AccountMeta::new(sell_reservation, false),
            AccountMeta::new(self.receipt(candidate, slice_index), false),
        ];
        assert_eq!(metas.len(), orders_batch::SETTLE_PAGE_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                clutch_solana_layout::Intent::SettlePage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index: 0,
                },
            ),
            metas,
        )
    }

    #[allow(clippy::too_many_arguments)] // one argument per account role
    fn settle_pair(
        &self,
        candidate: Hash32,
        sequence: u64,
        buyer_position: Address,
        seller_position: Address,
        buy_reservation: Address,
        sell_reservation: Address,
        receipt_slices: &[u16],
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new_readonly(self.terms_account, false),
            AccountMeta::new(buyer_position, false),
            AccountMeta::new(seller_position, false),
            AccountMeta::new(buy_reservation, false),
            AccountMeta::new(sell_reservation, false),
        ];
        assert_eq!(metas.len(), SETTLE_PAIR_FIXED_ACCOUNT_COUNT);
        metas.push(AccountMeta::new_readonly(self.page, false));
        for slice in receipt_slices {
            metas.push(AccountMeta::new(self.receipt(candidate, *slice), false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                clutch_solana_layout::Intent::SettlePage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index: 0,
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

    /// The disagreement package: active_len 6 with zero coefficients INSIDE
    /// the active prefix (nonzeros at outcomes 2 and 5) — the admission the
    /// design asks to be checked explicitly.
    fn package(&self, owner: &Owner, rank: u64, side: u8, limit_per_lot: u64) -> OrderSlot {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[..8].copy_from_slice(&C_PLUS);
        OrderSlot::Portfolio(PortfolioRecord {
            owner: owner.id,
            order_id: canonical_order_id(rank),
            side,
            active_len: 6,
            flags: 0,
            coefficients,
            lots: LOTS,
            limit_collateral_per_lot: limit_per_lot,
            minimum_fill_lots: 0,
            generation: 1,
            expiry_epoch: EPOCH_INDEX,
        })
    }
}

/// The degree-1 v3 terms artifact the exhibit clears under — a REAL
/// derived-basis terms account, per the design's no-impersonation rule:
/// basis_degree 1, knot_count 8 (= outcome_count at degree 1), u128 cent
/// knots 10,000..24,000, general spacing declared `UNIFORM_SPACING_NONE`
/// (the $20 gap is not a power of two; admitted at degree 1),
/// STAT-TERMINAL-01, EDGE-CLAMP-01, payout_map entirely unused, and one
/// uniform failure-refund preset 8/64 per outcome.
fn degree1_terms(realm: Hash32, profile: Hash32, feed: Hash32) -> clutch_solana_layout::TermsAccount {
    let mut terms = fixture_terms(realm, profile, feed);
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut weights = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < OUTCOMES as usize {
        weights[outcome] = 8;
        outcome += 1;
    }
    payouts[0] = PayoutVectorBytes {
        denominator: 64,
        weights,
    };
    let mut knots = [0u128; MAX_KNOTS];
    knots[..8].copy_from_slice(&KNOT_CENTS);
    terms.outcome_count = OUTCOMES;
    terms.payout_count = 1;
    terms.payouts = payouts;
    terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    terms.basis_degree = 1;
    terms.knot_count = 8;
    terms.knots = knots;
    terms.uniform_log2_spacing = UNIFORM_SPACING_NONE;
    terms.failure_payout_index = 0;
    terms.statistic_id = 1; // STAT-TERMINAL-01
    terms.edge_policy_id = 1; // EDGE-CLAMP-01
    terms.collateral_cap = 1_000_000;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    terms
}

async fn start() -> (ProgramTestContext, Fixture) {
    let realm = h(0x61);
    let profile = h(0x62);
    let feed = h(0x63);
    let market = h(0x3d);

    let mut ticks = [0; MAX_GRID_TICKS];
    ticks[..TICKS.len()].copy_from_slice(&TICKS);
    let mut grid = PriceGridAccount {
        grid: Hash32::ZERO,
        realm,
        price_scale: PRICE_SCALE,
        tick_count: TICKS.len() as u8,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid.grid = grid.recomputed_grid_id().unwrap();
    let (grid_address, grid_bump) = pda(seeds::SEED_GRID, &[&realm.bytes(), &grid.grid.bytes()]);
    grid.stored_bump = grid_bump;

    let mut terms = degree1_terms(realm, profile, feed);
    terms.price_grid = grid.grid;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    terms.validate().expect("the degree-1 terms artifact validates");
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

    // Injected genesis (stated, per the design's plane-creep rule): Egg
    // balances only on the market's 8 active outcomes — the portfolio
    // seam's canonical-padding rule refuses nonzero balances beyond
    // `outcome_count` — and no Endow or Split ever executes.
    let mut start_eggs = [0u64; MAX_OUTCOMES];
    start_eggs[..OUTCOMES as usize].fill(START_EGGS);
    let mut owners = Vec::new();
    for _ in 0..2 {
        let key = Keypair::new();
        let id = Hash32::from_bytes(key.pubkey().to_bytes());
        let (position_address, position_bump) =
            pda(seeds::SEED_POSITION, &[&market.bytes(), &id.bytes()]);
        let position = PositionAccount {
            market,
            owner: id,
            generation: 0,
            internal: start_eggs,
            cash_atoms: START_CASH,
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

async fn bytes_of(context: &mut ProgramTestContext, address: Address) -> Vec<u8> {
    account(context, address).await.unwrap().data
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

/// The 13-order disagreement book, in placement (= stored = live) rank
/// order: five crossed single quotes (each side at its own model's value),
/// the 50-lot package pair, and E's uncrossed low-ball.  G is owner 0
/// ("the Gaussian"), E is owner 1 ("the empiricist").
fn book_plan(fixture: &Fixture) -> Vec<(usize, OrderSlot)> {
    let g = &fixture.owners[0];
    let e = &fixture.owners[1];
    vec![
        (0, fixture.single(g, 1, 1, 1, Z, 98)),      // G sells @120-knot at 98
        (1, fixture.single(e, 2, 1, 0, Z, 127)),     // E buys @120-knot at 127
        (0, fixture.single(g, 3, 2, 0, Z, 2_961)),   // G buys @140-knot at 2961
        (1, fixture.single(e, 4, 2, 1, Z, 2_662)),   // E sells @140-knot at 2662
        (1, fixture.single(e, 5, 3, 0, Z, 5_945)),   // E buys @160-knot at 5945
        (0, fixture.single(g, 6, 3, 1, Z, 5_696)),   // G sells @160-knot at 5696
        (1, fixture.single(e, 7, 4, 0, Z, 1_266)),   // E buys @180-knot at 1266
        (0, fixture.single(g, 8, 4, 1, Z, 1_213)),   // G sells @180-knot at 1213
        (0, fixture.single(g, 9, 5, 0, Z, 32)),      // G buys @200-knot at 32
        (1, fixture.single(e, 10, 5, 1, Z, 0)),      // E sells @200-knot at 0
        (0, fixture.package(g, 11, 0, 88)),          // G package buy, 88 atoms/lot
        (1, fixture.package(e, 12, 1, 80)),          // E package sell, 80 atoms/lot
        (1, fixture.single(e, 13, 1, 0, Z, 98)),     // E low-ball buy: ineligible
    ]
}

/// Place the book, then freeze at the deadline.
async fn build_frozen_book(context: &mut ProgramTestContext, fixture: &Fixture) {
    let payer = context.payer.pubkey();
    let (result, _) = send(context, &[fixture.init_epoch(payer)], None, 0).await;
    result.expect("InitEpoch admits the degree-1 terms market");
    let (result, _) = send(context, &[fixture.init_page(payer)], None, 1).await;
    result.unwrap();
    for (sequence, (owner_index, slot)) in book_plan(fixture).iter().enumerate() {
        let owner = &fixture.owners[*owner_index];
        let (result, _) = send(
            context,
            &[fixture.place(owner, sequence as u64, *slot)],
            Some(&owner.key),
            10 + sequence as u32,
        )
        .await;
        result.unwrap_or_else(|error| {
            panic!("PlaceOrder rank {} refused: {error:?}", sequence + 1)
        });
    }
    context.warp_to_slot(FREEZE_DEADLINE).unwrap();
    let (result, _) = send(context, &[fixture.freeze()], None, 40).await;
    result.expect("FreezeEpoch seals the degree-1 book");
}

/// The frozen epoch, the projected host book, and the walk-order
/// reservation list.
async fn frozen_state(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> (EpochAccount, clutch_batch::relation_v1::BookV1, Vec<Address>) {
    let epoch = EpochAccount::decode(&bytes_of(context, fixture.epoch_account).await).unwrap();
    assert_eq!(epoch.phase, EPOCH_PHASE_FROZEN);
    let page = bytes_of(context, fixture.page).await;
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
    (epoch, book, reservations)
}

/// The pre-registered candidate's host-computed coordinates.
fn plan_submission(
    fixture: &Fixture,
    epoch: &EpochAccount,
    book: &clutch_batch::relation_v1::BookV1,
    witness: PairingWitnessV1,
) -> Submission {
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..8].copy_from_slice(&P);
    let domain = zero_sentinel_domain(epoch);
    let candidate = canonical_candidate(&domain, book, &prices, 0, 0)
        .expect("the pre-registered candidate is relation-valid");
    let mut shell = CandidateFeedHeader {
        candidate: Hash32::ZERO,
        epoch: fixture.epoch_id,
        market: fixture.market,
        order_set: epoch.order_set,
        prices,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        honored_aon_mask: 0,
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
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        fills: candidate.fills[..book.len as usize].to_vec(),
        witness,
        record: fixture.candidate_record(id),
        feed: fixture.candidate_feed(id),
    }
}

/// Drive one submission through the staged wire: create, chunked content,
/// seal.
async fn submit_seal(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    submission: &Submission,
    declared: Option<u16>,
    nonce: u32,
) {
    let payer = context.payer.pubkey();
    let (result, _) = send(
        context,
        &[fixture.submit(payer, submission, declared)],
        None,
        nonce,
    )
    .await;
    result.unwrap();

    let mut written = 0u64;
    for chunk_fills in submission.fills.chunks(FEED_FILLS_PER_CHUNK) {
        let mut fills = [0u64; FEED_FILLS_PER_CHUNK];
        fills[..chunk_fills.len()].copy_from_slice(chunk_fills);
        let (result, _) = send(
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
        let (result, _) = send(
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
        written += chunk_slices.len() as u64;
    }

    let (result, _) = send(context, &[fixture.seal(submission, &[])], None, nonce + 40).await;
    result.unwrap();
}

/// Walk the sealed candidate to its verdict through the real tags 51-53,
/// reporting each stage's compute units.
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
    let (result, units) = send(context, &creation, None, nonce).await;
    result.unwrap();
    eprintln!("InitClearWork + 4 grows CU: {units}");
    let (result, units) = send_walk(
        context,
        fixture.advance(submission.id, 16, reservations),
        nonce + 1,
    )
    .await;
    result.unwrap();
    eprintln!("AdvanceClearWork (pass 1, 13 orders) CU: {units}");
    let (result, units) = send_walk(
        context,
        fixture.advance_slices(submission.id, submission.witness.len),
        nonce + 2,
    )
    .await;
    result.unwrap();
    eprintln!("AdvanceClearSlices (7 slices) CU: {units}");
    let (result, units) =
        send_walk(context, fixture.advance(submission.id, 16, &[]), nonce + 3).await;
    result.unwrap();
    eprintln!("AdvanceClearWork (pass 2) CU: {units}");
    let (result, units) = send_walk(context, fixture.complete(submission.id), nonce + 4).await;
    result.unwrap();
    eprintln!("CompleteClearWork CU: {units}");
}

/// One explicit witness slice.
fn slice(buy: u8, sell: u8, outcome: u8, quantity: u64) -> PairingSliceV1 {
    PairingSliceV1 {
        buy_ref: LegRefV1::Order(buy),
        sell_ref: LegRefV1::Order(sell),
        outcome,
        quantity,
    }
}

/// The exhibit's 7-slice pairing: five single crossings and the package
/// pair's two per-outcome legs (14,950 Eggs @140-knot, 1,600 @200-knot).
fn exhibit_witness() -> PairingWitnessV1 {
    let slices = [
        slice(1, 0, 1, Z),                 // E buys / G sells the 120-knot
        slice(2, 3, 2, Z),                 // G buys / E sells the 140-knot
        slice(4, 5, 3, Z),                 // E buys / G sells the 160-knot
        slice(6, 7, 4, Z),                 // E buys / G sells the 180-knot
        slice(8, 9, 5, Z),                 // G buys / E sells the 200-knot
        slice(10, 11, 2, LOTS * C_PLUS[2]), // package leg: 14,950 @140-knot
        slice(10, 11, 5, LOTS * C_PLUS[5]), // package leg: 1,600 @200-knot
    ];
    let mut witness = PairingWitnessV1::empty();
    witness.slices[..slices.len()].copy_from_slice(&slices);
    witness.len = slices.len() as u16;
    witness
}

async fn read_reservation(context: &mut ProgramTestContext, address: Address) -> ReservationAccount {
    ReservationAccount::decode(&bytes_of(context, address).await).unwrap()
}

async fn read_position(context: &mut ProgramTestContext, address: Address) -> PositionAccount {
    PositionAccount::decode(&bytes_of(context, address).await).unwrap()
}

/// **T0** — the design's gate before anything else: the general plane
/// admits a degree-1 v3 terms market end to end through its admission
/// seams — terms decode/validate + the InitEpoch binding matrix, PlaceOrder
/// for degree-1 single-Egg quotes (tick-verified) and for the package with
/// zero coefficients inside its active prefix, and the freeze's page-set
/// binding.  Expected yes: the relation sees the basis only through
/// partition of unity.  A refusal here fails loudly with the exact code and
/// the walk must NOT be re-attempted on degree-0 terms.
#[tokio::test]
async fn degree1_terms_admission_through_the_general_plane() {
    let (mut context, fixture) = start().await;
    build_frozen_book(&mut context, &fixture).await;
    let (epoch, book, _) = frozen_state(&mut context, &fixture).await;
    assert_eq!(epoch.phase, EPOCH_PHASE_FROZEN);
    assert_eq!(epoch.outcome_count, OUTCOMES);
    assert_eq!(book.len, 13);
    eprintln!("T0: degree-1 terms admitted; 13-order book frozen on the general plane");
}

/// The headline: the disagreement book clears end to end — the T2-8 route
/// through Settle x6 — with the whole value plane conserved, the cleared
/// prices strictly between the two beliefs, and the pot provably empty.
#[tokio::test]
async fn disagreement_book_clears_between_the_beliefs_with_conservation() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    build_frozen_book(&mut context, &fixture).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;
    assert_eq!(book.len, 13);
    assert_eq!(epoch.owner_count, 2);

    // The pre-registered candidate: every crossing order fills fully, the
    // package fills its whole 50 lots on both sides, the low-ball fills
    // zero, and no virtual leg exists (imbalance 0).
    let alpha = plan_submission(&fixture, &epoch, &book, exhibit_witness());
    assert_eq!(
        alpha.fills,
        vec![Z, Z, Z, Z, Z, Z, Z, Z, Z, Z, LOTS, LOTS, 0],
        "canonical fills: ten full single crossings, the full package pair, \
         the low-ball at zero"
    );
    assert_eq!(alpha.virtual_split, 0);
    assert_eq!(alpha.virtual_merge, 0);

    submit_seal(&mut context, &fixture, &alpha, Some(7), 100).await;
    walk_to_verdict(&mut context, &fixture, &alpha, &reservations, 300).await;

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(&mut context, &[fixture.finalize(&[alpha.id])], None, 340).await;
    result.unwrap();
    let record =
        CandidateRecord::decode(&bytes_of(&mut context, fixture.candidate_record(alpha.id)).await)
            .unwrap();
    assert_eq!(record.status, CANDIDATE_STATUS_SELECTED);
    let epoch_now =
        EpochAccount::decode(&bytes_of(&mut context, fixture.epoch_account).await).unwrap();
    assert_eq!(epoch_now.phase, EPOCH_PHASE_CLEARED);

    // The entitlement freeze: pot from the verified summary, provably empty.
    let (result, units) = send_walk(&mut context, fixture.freeze_entitlement(payer, alpha.id), 342)
        .await;
    result.unwrap();
    eprintln!("FreezeEntitlement CU: {units}");
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(pot.candidate, alpha.id);
    assert_eq!(pot.phase, POT_PHASE_CLOSED);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.rounding_pot_price_units, 0);

    // Walk-order reservations: ranks 1..=13 (no retirements, so stored and
    // live ranks agree).  Slices 0..=4 pair them as
    // (buy, sell) = (1,0), (2,3), (4,5), (6,7), (8,9); the package pair is
    // (10,11); 12 is the low-ball.
    let g = 0usize;
    let e = 1usize;
    let position = |index: usize| fixture.owners[index].position;
    let single_slices: [(u16, usize, usize); 5] = [
        (0, 1, 0), // slice, buy reservation index, sell reservation index
        (1, 2, 3),
        (2, 4, 5),
        (3, 6, 7),
        (4, 8, 9),
    ];
    for (slice_index, buy, sell) in single_slices {
        let (result, units) = send(
            &mut context,
            &[fixture.entitle_single(
                payer,
                alpha.id,
                slice_index,
                reservations[buy],
                reservations[sell],
            )],
            None,
            345 + slice_index as u32,
        )
        .await;
        result.unwrap();
        eprintln!("EntitleSlice (single {slice_index}) CU: {units}");
    }
    let (result, units) = send(
        &mut context,
        &[fixture.entitle_pair(
            payer,
            alpha.id,
            5,
            reservations[10],
            reservations[11],
            &[5, 6],
        )],
        None,
        351,
    )
    .await;
    result.unwrap();
    eprintln!("EntitleSlice (portfolio pair, 2 receipts) CU: {units}");

    // Settle x6: five entitled single crossings, then the portfolio full
    // pair.  Buyer/seller positions follow each slice's sides.
    let single_settles: [(u16, usize, usize, usize, usize); 5] = [
        (0, e, g, 1, 0), // slice, buyer, seller, buy res, sell res
        (1, g, e, 2, 3),
        (2, e, g, 4, 5),
        (3, e, g, 6, 7),
        (4, g, e, 8, 9),
    ];
    for (slice_index, buyer, seller, buy, sell) in single_settles {
        let (result, units) = send(
            &mut context,
            &[fixture.settle_single(
                alpha.id,
                1 + slice_index as u64,
                position(buyer),
                position(seller),
                reservations[buy],
                reservations[sell],
                slice_index,
            )],
            None,
            360 + slice_index as u32,
        )
        .await;
        result.unwrap();
        eprintln!("SettlePage (entitled single, slice {slice_index}) CU: {units}");
    }
    let (result, units) = send(
        &mut context,
        &[fixture.settle_pair(
            alpha.id,
            6,
            position(g),
            position(e),
            reservations[10],
            reservations[11],
            &[5, 6],
        )],
        None,
        366,
    )
    .await;
    result.unwrap();
    eprintln!("SettlePage (entitled portfolio full pair) CU: {units}");

    /* -------------------------------------------------------------------- */
    /* The conservation battery: the whole value plane, to the atom.         */
    /* -------------------------------------------------------------------- */

    // Positions byte-compared to the verified summary's implied allocation.
    // G: pays 4,392 (142 + 1 + 4,249), receives 357 (6 + 289 + 62): net
    // -4,035.  E is the exact mirror, with the low-ball's 5 atoms standing.
    // Every number here re-derives in docs/site-plan/disagreement_check.py.
    let expected: [(usize, u64, u64, [i64; 8]); 2] = [
        (g, START_CASH - 4_035, 0, [0, -500, 15_450, -500, -500, 2_100, 0, 0]),
        (e, START_CASH + 4_035, 5, [0, 500, -15_450, 500, 500, -2_100, 0, 0]),
    ];
    for (index, cash, reserved, deltas) in expected {
        let owner = &fixture.owners[index];
        let observed = read_position(&mut context, owner.position).await;
        let mut start_eggs = [0u64; MAX_OUTCOMES];
        start_eggs[..OUTCOMES as usize].fill(START_EGGS);
        let mut implied = PositionAccount {
            market: fixture.market,
            owner: owner.id,
            generation: 0,
            internal: start_eggs,
            cash_atoms: cash,
            reserved_cash_atoms: reserved,
            stored_bump: observed.stored_bump,
            close_state: 0,
        };
        for (outcome, delta) in deltas.iter().enumerate() {
            implied.internal[outcome] = (START_EGGS as i64 + delta) as u64;
        }
        assert_eq!(
            bytes_of(&mut context, owner.position).await,
            encode(account_len::POSITION, |out| implied.encode(out)),
            "owner {index} position bytes equal the implied allocation"
        );
    }

    // Cash conservation: totals exactly the injected genesis.
    let mut total_cash = 0u64;
    let mut egg_totals = [0u64; MAX_OUTCOMES];
    for owner in &fixture.owners {
        let position = read_position(&mut context, owner.position).await;
        total_cash += position.cash_atoms;
        for (outcome, total) in egg_totals.iter_mut().enumerate() {
            *total += position.internal[outcome];
        }
    }
    assert_eq!(total_cash, 2 * START_CASH);
    for total in egg_totals.iter().take(OUTCOMES as usize) {
        assert_eq!(*total, 2 * START_EGGS);
    }
    for total in egg_totals.iter().skip(OUTCOMES as usize) {
        assert_eq!(*total, 0);
    }

    // The consumed reservations are their own archive: CONSUMED, remaining
    // zero, the exact initial envelope intact.  (Cash envelopes are
    // ceil(q*limit/S) -- the sup-norm story: 7/149/298/64/2 on the single
    // buys and 50 x 88 = 4,400 on the package buy.)
    let mut expected_internal = [[0u64; 8]; 12];
    expected_internal[0][1] = Z; // G's 120-knot sell envelope
    expected_internal[3][2] = Z; // E's 140-knot sell envelope
    expected_internal[5][3] = Z; // G's 160-knot sell envelope
    expected_internal[7][4] = Z; // G's 180-knot sell envelope
    expected_internal[9][5] = Z; // E's 200-knot sell envelope
    expected_internal[11][2] = LOTS * C_PLUS[2]; // E's package legs
    expected_internal[11][5] = LOTS * C_PLUS[5];
    let expected_cash: [u64; 12] = [0, 7, 149, 0, 298, 0, 64, 0, 2, 0, 4_400, 0];
    for index in 0..12 {
        let reservation = read_reservation(&mut context, reservations[index]).await;
        assert_eq!(reservation.state, RESERVATION_STATE_CONSUMED, "reservation {index}");
        assert!(reservation.remaining_is_zero(), "reservation {index} drained");
        assert_eq!(
            reservation.initial_cash_atoms, expected_cash[index],
            "reservation {index} cash envelope"
        );
        assert_eq!(
            &reservation.initial_internal[..8],
            &expected_internal[index],
            "reservation {index} Egg envelope"
        );
    }

    // Release identity across the whole book: released buy envelopes
    // (4,551 + 369 = 4,920) = considerations (4,392 + 357 = 4,749) +
    // price-improvement refunds (159 + 12 = 171) + pot (0).
    assert_eq!(4_551 + 369, (4_392 + 357) + (159 + 12) + pot.pot_cash_price_units as u64);

    // The receipts are exhausted, exactly once each: five single crossings
    // of 500 Eggs and the package's two legs.
    for (slice_index, quantity) in
        [(0u16, Z), (1, Z), (2, Z), (3, Z), (4, Z), (5, LOTS * C_PLUS[2]), (6, LOTS * C_PLUS[5])]
    {
        let receipt = SettlementReceiptAccount::decode(
            &bytes_of(&mut context, fixture.receipt(alpha.id, slice_index)).await,
        )
        .unwrap();
        assert_eq!(receipt.settled_quantity, quantity, "receipt {slice_index}");
        assert_eq!(
            receipt.consumed_flags,
            RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED,
            "receipt {slice_index} exhausted"
        );
    }

    // The uncrossed low-ball's reservation stands ACTIVE at its 5-atom
    // envelope, and cancellation still requires an OPEN epoch: the
    // post-clear exit is the owner-signed terminal release (tag 60), whose
    // evidence lives in terminal_closure.rs.
    let lowball = read_reservation(&mut context, reservations[12]).await;
    assert_eq!(lowball.state, RESERVATION_STATE_ACTIVE);
    assert_eq!(lowball.remaining_cash_atoms, 5);
    let release = send(
        &mut context,
        &[fixture.cancel(&fixture.owners[e], canonical_order_id(13), 2)],
        Some(&fixture.owners[e].key),
        370,
    )
    .await;
    assert_eq!(custom(release.0), ClutchError::NotActive as u32);
}
