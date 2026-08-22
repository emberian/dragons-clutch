//! Real-SBF evidence for the entitlement freeze and generalized consumption
//! (T2-8) — the Tier 2 headline: **the portfolio order actually clears**.
//!
//! `Intent::FreezeEntitlement` (tag 58), `Intent::EntitleSlice` (59), and the
//! widened `Intent::SettlePage`, driven end to end on one bank against a real
//! frozen general book: place portfolio + single orders → freeze → submit →
//! walk to `VERIFIED` through the real tags 51-53 → select → entitle →
//! consume → conservation assertions across the whole value plane.
//!
//! The gates:
//! * the headline walk — a five-live-order book (one retirement, so live
//!   ranks and stored ranks differ) with a single-Egg crossing, a portfolio
//!   full pair, and one ineligible order: every collateral atom and every
//!   position unit accounted, positions byte-compared against the verified
//!   summary's implied allocation, the pot provably empty;
//! * every refusal the plan names — entitle before CLEARED, entitle of a
//!   non-selected candidate, double-entitle, consume of an unentitled
//!   receipt, double-consume, a partial pair presentation, and a tampered
//!   receipt — each with full rollback;
//! * the retired rows, at this file's boundary — a verified virtual-split
//!   candidate now freezes and opens its pot (the `VirtualPot` row retired;
//!   the mint itself is driven in `vpot_split.rs`) and a partial-fill
//!   candidate clears slice by slice;
//! * the lapse record — a lapsed epoch's reservations stand ACTIVE and the
//!   cancellation path (`CancelOrder`) keeps requiring an OPEN epoch: the
//!   post-lapse exit is the owner-signed terminal release (tag 60), driven
//!   with the whole close family in `terminal_closure.rs`.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.  The reference adapter
//! refuses both new intents with `UnsupportedIntent`; the oracle is the
//! layout codec plus the frozen receipt/pot codecs and the host relation.

use {
    clutch_batch::relation_v1::{
        canonical_candidate, LegRefV1, PairingSliceV1, PairingWitnessV1, RelationDomainV1,
    },
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
    },
    clutch_sbf::{
        error::{codec_code, ClutchError},
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
            canonical_reservation_id, ReservationAccount, RESERVATION_ACCOUNT_BYTES,
            RESERVATION_STATE_ACTIVE, RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED,
        },
        stream, CandidateFeedChunk, CandidateRecord, EpochAccount, FinalPotAccount, Hash32,
        MarketAccount, OrderRecord, OrderSlot, PortfolioRecord, PositionAccount, PriceGridAccount,
        SettlementReceiptAccount, CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED,
        EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED, FEED_FILLS_PER_CHUNK, FEED_SLICES_PER_CHUNK,
        MAX_GRID_TICKS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED, POT_PHASE_CLOSED,
        POT_PHASE_OPEN,
        RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED, RECEIPT_FLAG_SLICE_EXHAUSTED,
    },
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_terms, layout_request, request_heap_frame_data,
        COMPUTE_BUDGET, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM,
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
const EPOCH_INDEX: u64 = 7;
const FREEZE_DEADLINE: u64 = 500;
const CU_LIMIT: u32 = 1_400_000;
const HEAP_FRAME: u32 = 262_144;
const WALLET: u64 = 5_000_000_000;
const OUTCOMES: u8 = 4;
/// Every position starts with this much cash and this many Eggs per outcome.
const START_CASH: u64 = 1_000_000;
const START_EGGS: u64 = 1_000;

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

/// One program-owned account image, for the hostile substitutions that write
/// bytes no instruction could have produced.
fn program_account(data: Vec<u8>) -> Account {
    Account {
        lamports: rent_exempt(data.len()),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
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
            metas.push(AccountMeta::new_readonly(
                self.candidate_feed(*candidate),
                false,
            ));
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

    /// The fixed prefix every `EntitleSlice` shares: payer, plane, and the
    /// one-page frozen set.
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
        metas.push(AccountMeta::new(
            self.receipt(candidate, slice_index),
            false,
        ));
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

    /// The potted direct-slice shape: the seven-account list plus the epoch's
    /// final pot, which a completing end draws its rounding residue from.
    #[allow(clippy::too_many_arguments)] // one argument per account role
    fn settle_single_potted(
        &self,
        candidate: Hash32,
        sequence: u64,
        buyer_position: Address,
        seller_position: Address,
        buy_reservation: Address,
        sell_reservation: Address,
        slice_index: u16,
    ) -> Instruction {
        let mut instruction = self.settle_single(
            candidate,
            sequence,
            buyer_position,
            seller_position,
            buy_reservation,
            sell_reservation,
            slice_index,
        );
        instruction
            .accounts
            .push(AccountMeta::new(self.pot(), false));
        assert_eq!(
            instruction.accounts.len(),
            orders_batch::SETTLE_PAGE_POTTED_ACCOUNT_COUNT
        );
        instruction
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

    /// Owner-signed post-terminal release on a CLEARED epoch: the zero-fill
    /// proof shape (tag 60), one page prefix wide.
    fn release_cleared(
        &self,
        owner: &Owner,
        reservation: Address,
        selected: Hash32,
        sequence: u64,
    ) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(owner.key.pubkey(), true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(reservation, false),
            AccountMeta::new(owner.position, false),
            AccountMeta::new_readonly(self.candidate_record(selected), false),
            AccountMeta::new_readonly(self.candidate_feed(selected), false),
            AccountMeta::new_readonly(self.page, false),
        ];
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                clutch_solana_layout::Intent::ReleaseTerminalReservation {
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

    fn portfolio(
        &self,
        owner: &Owner,
        rank: u64,
        side: u8,
        coefficients: [u64; 2],
        lots: u64,
        limit_collateral_per_lot: u64,
    ) -> OrderSlot {
        let mut padded = [0u64; MAX_OUTCOMES];
        padded[..2].copy_from_slice(&coefficients);
        OrderSlot::Portfolio(PortfolioRecord {
            owner: owner.id,
            order_id: canonical_order_id(rank),
            side,
            active_len: 2,
            flags: 0,
            coefficients: padded,
            lots,
            limit_collateral_per_lot,
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
    let market = h(0x3c);

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

    // Egg balances live only on the market's active outcomes: the
    // portfolio seam's canonical-padding rule refuses nonzero balances
    // beyond `outcome_count`.
    let mut start_eggs = [0u64; MAX_OUTCOMES];
    start_eggs[..OUTCOMES as usize].fill(START_EGGS);
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

async fn snapshot(context: &mut ProgramTestContext, addresses: &[Address]) -> Vec<Vec<u8>> {
    let mut all = Vec::with_capacity(addresses.len());
    for address in addresses {
        all.push(bytes_of(context, *address).await);
    }
    all
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

/// Place one book plan (with optional retirements), then freeze at the
/// deadline.
async fn build_frozen_book(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    orders: &[(usize, OrderSlot)],
    cancels: &[(usize, Hash32)],
) {
    let payer = context.payer.pubkey();
    let (result, _) = send(context, &[fixture.init_epoch(payer)], None, 0).await;
    result.unwrap();
    let (result, _) = send(context, &[fixture.init_page(payer)], None, 1).await;
    result.unwrap();
    for (sequence, (owner_index, slot)) in orders.iter().enumerate() {
        let owner = &fixture.owners[*owner_index];
        let (result, _) = send(
            context,
            &[fixture.place(owner, sequence as u64, *slot)],
            Some(&owner.key),
            10 + sequence as u32,
        )
        .await;
        result.unwrap();
    }
    for (at, (owner_index, order_id)) in cancels.iter().enumerate() {
        let owner = &fixture.owners[*owner_index];
        let (result, _) = send(
            context,
            &[fixture.cancel(owner, *order_id, 2)],
            Some(&owner.key),
            30 + at as u32,
        )
        .await;
        result.unwrap();
    }
    context.warp_to_slot(FREEZE_DEADLINE).unwrap();
    let (result, _) = send(context, &[fixture.freeze()], None, 40).await;
    result.unwrap();
}

/// The frozen epoch, the projected host book, and the walk-order reservation
/// list of the live orders.
async fn frozen_state(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> (
    EpochAccount,
    clutch_batch::relation_v1::BookV1,
    Vec<Address>,
) {
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

/// Compute one candidate's canonical coordinates host-side, with an explicit
/// witness.
fn plan_submission(
    fixture: &Fixture,
    epoch: &EpochAccount,
    book: &clutch_batch::relation_v1::BookV1,
    prices: [u64; MAX_OUTCOMES],
    imbalance: i64,
    witness: PairingWitnessV1,
) -> Submission {
    let domain = zero_sentinel_domain(epoch);
    let candidate = canonical_candidate(&domain, book, &prices, imbalance, 0).unwrap();
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
/// seal against the current registry.
async fn submit_seal(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    submission: &Submission,
    declared: Option<u16>,
    retained: &[Hash32],
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

    let (result, _) = send(
        context,
        &[fixture.seal(submission, retained)],
        None,
        nonce + 40,
    )
    .await;
    result.unwrap();
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
    let (result, _) = send_walk(context, fixture.complete(submission.id), nonce + 4).await;
    result.unwrap();
}

/// One explicit witness slice.
fn slice(buy: u8, sell_ref: LegRefV1, outcome: u8, quantity: u64) -> PairingSliceV1 {
    PairingSliceV1 {
        buy_ref: LegRefV1::Order(buy),
        sell_ref,
        outcome,
        quantity,
    }
}

/// Pack explicit slices into a canonical-padded witness.
fn witness_of(slices: &[PairingSliceV1]) -> PairingWitnessV1 {
    let mut witness = PairingWitnessV1::empty();
    witness.slices[..slices.len()].copy_from_slice(slices);
    witness.len = slices.len() as u16;
    witness
}

async fn read_reservation(
    context: &mut ProgramTestContext,
    address: Address,
) -> ReservationAccount {
    ReservationAccount::decode(&bytes_of(context, address).await).unwrap()
}

/// Every owner's free-plus-reserved cash, summed across the book.
async fn owner_cash(context: &mut ProgramTestContext, fixture: &Fixture) -> u64 {
    let mut total = 0u64;
    for owner in &fixture.owners {
        total += read_position(context, owner.position).await.cash_atoms;
    }
    total
}

/// Every Egg on one outcome the book can account for: what the Positions
/// hold plus what the reservations still own.
///
/// Admission moves a sell's Eggs *out* of its Position and into its
/// reservation, so only this sum is conserved at every transaction boundary —
/// which is exactly the claim a partial clearing has to make good on.
async fn book_eggs(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    reservations: &[Address],
    outcome: usize,
) -> u64 {
    let mut total = 0u64;
    for owner in &fixture.owners {
        total += read_position(context, owner.position).await.internal[outcome];
    }
    for address in reservations {
        total += read_reservation(context, *address).await.remaining_internal[outcome];
    }
    total
}

async fn read_position(context: &mut ProgramTestContext, address: Address) -> PositionAccount {
    PositionAccount::decode(&bytes_of(context, address).await).unwrap()
}

/// The Tier 2 headline gate: the portfolio order actually clears, with the
/// whole value plane accounted, plus the plan's refusal battery on the same
/// bank.
#[tokio::test]
async fn portfolio_order_actually_clears_with_conservation() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let a = 0usize; // single buyer (and the unfilled ineligible buy)
    let b = 1usize; // single seller (and the retired order)
    let c = 2usize; // portfolio buyer
    let d = 3usize; // portfolio seller

    // Stored ranks: 1 = B's retired sell, 2 = A buy o0 q8@3000,
    // 3 = B sell o0 q8@2000, 4 = C portfolio buy [2,2]x3 @ 2/lot,
    // 5 = D portfolio sell [6,6]x1 @ 1/lot, 6 = A ineligible buy o0 q5@1000.
    // Live ranks after the retirement: 0..=4 map onto stored 2..=6.
    let orders = [
        (b, fixture.single(&fixture.owners[b], 1, 0, 1, 3, 2_500)),
        (a, fixture.single(&fixture.owners[a], 2, 0, 0, 8, 3_000)),
        (b, fixture.single(&fixture.owners[b], 3, 0, 1, 8, 2_000)),
        (c, fixture.portfolio(&fixture.owners[c], 4, 0, [2, 2], 3, 2)),
        (d, fixture.portfolio(&fixture.owners[d], 5, 1, [6, 6], 1, 1)),
        (a, fixture.single(&fixture.owners[a], 6, 0, 0, 5, 1_000)),
    ];
    build_frozen_book(
        &mut context,
        &fixture,
        &orders,
        &[(b, canonical_order_id(1))],
    )
    .await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;
    assert_eq!(book.len, 5);

    // The candidate: even prices, no imbalance, and a hand-aligned witness
    // whose slices are exactly the pairings T2-8 consumes — one single-Egg
    // crossing and the portfolio pair's two per-outcome legs.
    let witness = witness_of(&[
        slice(0, LegRefV1::Order(1), 0, 8),
        slice(2, LegRefV1::Order(3), 0, 6),
        slice(2, LegRefV1::Order(3), 1, 6),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let alpha = plan_submission(&fixture, &epoch, &book, prices, 0, witness);
    assert_eq!(alpha.fills, vec![8, 8, 3, 1, 0]);
    // A second, never-verified candidate: its entitlement must refuse.
    let mut beta_prices = [0u64; MAX_OUTCOMES];
    beta_prices[..4].copy_from_slice(&[3_000, 3_000, 2_000, 2_000]);
    let beta = plan_submission(
        &fixture,
        &epoch,
        &book,
        beta_prices,
        0,
        PairingWitnessV1::empty(),
    );

    submit_seal(&mut context, &fixture, &alpha, Some(3), &[], 100).await;
    submit_seal(&mut context, &fixture, &beta, None, &[alpha.id], 160).await;
    walk_to_verdict(&mut context, &fixture, &alpha, &reservations, 300).await;

    // Entitle before CLEARED refuses: the epoch is still FROZEN.
    let early = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, alpha.id),
        330,
    )
    .await;
    assert_eq!(custom(early.0), ClutchError::NotActive as u32);

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let retained = [alpha.id, beta.id];
    let (result, _) = send(&mut context, &[fixture.finalize(&retained)], None, 340).await;
    result.unwrap();
    let record =
        CandidateRecord::decode(&bytes_of(&mut context, fixture.candidate_record(alpha.id)).await)
            .unwrap();
    assert_eq!(record.status, CANDIDATE_STATUS_SELECTED);
    let epoch_now =
        EpochAccount::decode(&bytes_of(&mut context, fixture.epoch_account).await).unwrap();
    assert_eq!(epoch_now.phase, EPOCH_PHASE_CLEARED);

    // Entitle of a non-selected candidate refuses.
    let unselected = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, beta.id),
        341,
    )
    .await;
    assert_eq!(custom(unselected.0), ClutchError::MismatchedState as u32);

    // The entitlement freeze: pot from the verified summary, provably empty.
    let (result, units) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, alpha.id),
        342,
    )
    .await;
    result.unwrap();
    eprintln!("FreezeEntitlement CU: {units}");
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(pot.candidate, alpha.id);
    assert_eq!(pot.phase, POT_PHASE_CLOSED);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.rounding_pot_price_units, 0);
    // The pot PDA's existence is the freeze's replay guard.
    let replay = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, alpha.id),
        343,
    )
    .await;
    assert_eq!(custom(replay.0), ClutchError::AlreadyInitialized as u32);

    // The walk-order reservation list: [A#2, B#3, C#4, D#5, A#6].
    let (res_a, res_b, res_c, res_d, res_idle) = (
        reservations[0],
        reservations[1],
        reservations[2],
        reservations[3],
        reservations[4],
    );
    let position = |index: usize| fixture.owners[index].position;

    // Consume before entitle refuses: the receipt account does not exist.
    let unentitled = send(
        &mut context,
        &[fixture.settle_single(alpha.id, 1, position(a), position(b), res_a, res_b, 0)],
        None,
        344,
    )
    .await;
    assert_eq!(custom(unentitled.0), ClutchError::WrongProgramOwner as u32);

    // Entitle the single-Egg crossing (slice 0).
    let (result, units) = send(
        &mut context,
        &[fixture.entitle_single(payer, alpha.id, 0, res_a, res_b)],
        None,
        345,
    )
    .await;
    result.unwrap();
    eprintln!("EntitleSlice (single) CU: {units}");
    assert_eq!(
        read_reservation(&mut context, res_a).await.state,
        RESERVATION_STATE_ENTITLED
    );
    assert_eq!(
        read_reservation(&mut context, res_b).await.state,
        RESERVATION_STATE_ENTITLED
    );
    // Double-entitle refuses on the receipt's own non-existence: the ends'
    // stamps re-derive and agree — they must, or a later slice of the same
    // order could never be entitled — so the one-shot guard is the PDA.
    let again = send(
        &mut context,
        &[fixture.entitle_single(payer, alpha.id, 0, res_a, res_b)],
        None,
        346,
    )
    .await;
    assert_eq!(custom(again.0), ClutchError::AlreadyInitialized as u32);

    // Entitle the portfolio pair atomically from its first slice.
    let (result, units) = send(
        &mut context,
        &[fixture.entitle_pair(payer, alpha.id, 1, res_c, res_d, &[1, 2])],
        None,
        347,
    )
    .await;
    result.unwrap();
    eprintln!("EntitleSlice (portfolio pair, 2 receipts) CU: {units}");
    // A non-entry pair slice is not an entitlement point.
    let wrong_entry = send(
        &mut context,
        &[fixture.entitle_pair(payer, alpha.id, 2, res_c, res_d, &[1, 2])],
        None,
        348,
    )
    .await;
    assert_eq!(custom(wrong_entry.0), ClutchError::MismatchedState as u32);

    // Tamper on a receipt refuses: a codec-consistent quantity forgery still
    // breaks against the per-order ledger — twelve Egg atoms cannot fit in an
    // eight-atom entitled total — and the refusal rolls back.
    let receipt0_address = fixture.receipt(alpha.id, 0);
    let honest_receipt = account(&mut context, receipt0_address).await.unwrap();
    let mut forged = SettlementReceiptAccount::decode(&honest_receipt.data).unwrap();
    forged.quantity = 12;
    forged.consideration_price_units = 12 * 2_500;
    let mut forged_account = honest_receipt.clone();
    forged_account.data = encode(account_len::SETTLEMENT_RECEIPT, |out| forged.encode(out));
    context.set_account(&receipt0_address, &AccountSharedData::from(forged_account));
    let watched = [position(a), position(b), res_a, res_b];
    let before = snapshot(&mut context, &watched).await;
    let tampered = send(
        &mut context,
        &[fixture.settle_single(alpha.id, 1, position(a), position(b), res_a, res_b, 0)],
        None,
        349,
    )
    .await;
    assert_eq!(
        custom(tampered.0),
        ClutchError::AggregateClosureMismatch as u32
    );
    assert_eq!(snapshot(&mut context, &watched).await, before);
    context.set_account(&receipt0_address, &AccountSharedData::from(honest_receipt));

    // Consume the single-Egg crossing.
    let (result, units) = send(
        &mut context,
        &[fixture.settle_single(alpha.id, 1, position(a), position(b), res_a, res_b, 0)],
        None,
        350,
    )
    .await;
    result.unwrap();
    eprintln!("SettlePage (entitled direct slice) CU: {units}");
    // Double-consume refuses on the exhausted receipt and CONSUMED
    // reservations alike.
    let twice = send(
        &mut context,
        &[fixture.settle_single(alpha.id, 1, position(a), position(b), res_a, res_b, 0)],
        None,
        351,
    )
    .await;
    assert_eq!(custom(twice.0), ClutchError::MismatchedState as u32);

    // A partial pair presentation undersums and refuses.
    let partial = send(
        &mut context,
        &[fixture.settle_pair(alpha.id, 2, position(c), position(d), res_c, res_d, &[1])],
        None,
        352,
    )
    .await;
    assert_eq!(custom(partial.0), ClutchError::MismatchedState as u32);

    // THE HEADLINE TRANSACTION: the portfolio order actually clears.
    let (result, units) = send(
        &mut context,
        &[fixture.settle_pair(alpha.id, 2, position(c), position(d), res_c, res_d, &[1, 2])],
        None,
        353,
    )
    .await;
    result.unwrap();
    eprintln!("SettlePage (entitled portfolio full pair) CU: {units}");
    let pair_twice = send(
        &mut context,
        &[fixture.settle_pair(alpha.id, 2, position(c), position(d), res_c, res_d, &[1, 2])],
        None,
        354,
    )
    .await;
    assert_eq!(custom(pair_twice.0), ClutchError::MismatchedState as u32);

    /* -------------------------------------------------------------------- */
    /* Conservation across the whole value plane.                            */
    /* -------------------------------------------------------------------- */

    // The verified summary's implied allocation, byte-compared: every
    // position is the exact post-state the frozen prices and full fills
    // imply.  A pays 2 atoms for 8 Eggs of outcome 0 and keeps the standing
    // ineligible-order encumbrance; B is its mirror plus the returned
    // retirement; C pays 3 atoms for the [6,6] vector; D is its mirror.
    let expected = [
        // (owner index, cash, reserved, egg deltas per outcome)
        (a, START_CASH - 2, 1u64, [8i64, 0, 0, 0]),
        (b, START_CASH + 2, 0, [-8, 0, 0, 0]),
        (c, START_CASH - 3, 0, [6, 6, 0, 0]),
        (d, START_CASH + 3, 0, [-6, -6, 0, 0]),
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

    // Cash conservation: buyer debits equal seller credits, so total
    // position cash is exactly the genesis total.
    let mut total_cash = 0u64;
    let mut egg_totals = [0u64; MAX_OUTCOMES];
    for owner in &fixture.owners {
        let position = read_position(&mut context, owner.position).await;
        total_cash += position.cash_atoms;
        for (outcome, total) in egg_totals.iter_mut().enumerate() {
            *total += position.internal[outcome];
        }
    }
    assert_eq!(total_cash, 4 * START_CASH);
    // Egg conservation: every position unit accounted — consumed sell
    // reservations moved their whole envelope to the buyers, and no Egg
    // remains encumbered anywhere.
    for total in egg_totals.iter().take(OUTCOMES as usize) {
        assert_eq!(*total, 4 * START_EGGS);
    }

    // The consumed reservations are their own archive: CONSUMED, remaining
    // zero, the exact initial envelope intact.
    for (address, initial_cash, initial_egg) in [
        (res_a, 3u64, [0u64, 0, 0, 0]),
        (res_b, 0, [8, 0, 0, 0]),
        (res_c, 6, [0, 0, 0, 0]),
        (res_d, 0, [6, 6, 0, 0]),
    ] {
        let reservation = read_reservation(&mut context, address).await;
        assert_eq!(reservation.state, RESERVATION_STATE_CONSUMED);
        assert!(reservation.remaining_is_zero());
        assert_eq!(reservation.initial_cash_atoms, initial_cash);
        assert_eq!(&reservation.initial_internal[..4], &initial_egg);
    }
    // Released reservations + refunds sum exactly: the buy envelopes (3 + 6)
    // released; consideration moved (2 + 3); the rest (1 + 3) is the buyers'
    // implicit price-improvement refund into free cash.  With the pot's
    // scalars zero, the identity closes: 9 == 5 + 4 + 0.
    assert_eq!(3 + 6, (2 + 3) + (1 + 3) + pot.pot_cash_price_units as u64);

    // The receipts are exhausted, exactly once each.
    for (slice_index, quantity) in [(0u16, 8u64), (1, 6), (2, 6)] {
        let receipt = SettlementReceiptAccount::decode(
            &bytes_of(&mut context, fixture.receipt(alpha.id, slice_index)).await,
        )
        .unwrap();
        assert_eq!(receipt.settled_quantity, quantity);
        assert_eq!(
            receipt.consumed_flags,
            RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED
        );
    }

    // The unfilled ineligible order's reservation stands ACTIVE, and the
    // cancellation path refuses post-clear (release requires OPEN): the
    // owner-signed terminal release is its own intent (tag 60), driven with
    // full closure evidence in `terminal_closure.rs`.
    let idle = read_reservation(&mut context, res_idle).await;
    assert_eq!(idle.state, RESERVATION_STATE_ACTIVE);
    assert_eq!(idle.remaining_cash_atoms, 1);
    let release = send(
        &mut context,
        &[fixture.cancel(&fixture.owners[a], canonical_order_id(6), 2)],
        Some(&fixture.owners[a].key),
        360,
    )
    .await;
    assert_eq!(custom(release.0), ClutchError::NotActive as u32);
}

/// A verified virtual-split candidate now *freezes*: the `VirtualPot` row is
/// retired, and the pot opens carrying the churn expectation.
///
/// The mint itself is a `SettlePage` concern and is driven end to end in
/// `vpot_split.rs`; what this pins is the boundary this file used to hold —
/// the freeze no longer refuses, it records, and it opens the pot even on a
/// book that realizes no rounding residue at all.
#[tokio::test]
async fn virtual_split_candidate_freezes_and_opens_the_pot() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    // Four buys, one per outcome, all strict at even prices: the canonical
    // candidate at imbalance +4 fills them from the global virtual split.
    let orders = [
        (0, fixture.single(&fixture.owners[0], 1, 0, 0, 4, 3_000)),
        (1, fixture.single(&fixture.owners[1], 2, 1, 0, 4, 3_000)),
        (2, fixture.single(&fixture.owners[2], 3, 2, 0, 4, 3_000)),
        (3, fixture.single(&fixture.owners[3], 4, 3, 0, 4, 3_000)),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let witness = witness_of(&[
        slice(0, LegRefV1::Split, 0, 4),
        slice(1, LegRefV1::Split, 1, 4),
        slice(2, LegRefV1::Split, 2, 4),
        slice(3, LegRefV1::Split, 3, 4),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let split = plan_submission(&fixture, &epoch, &book, prices, 4, witness);
    assert_eq!(split.virtual_split, 4);
    assert_eq!(split.virtual_merge, 0);

    submit_seal(&mut context, &fixture, &split, Some(4), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &split, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(&mut context, &[fixture.finalize(&[split.id])], None, 340).await;
    result.unwrap();
    let epoch_now =
        EpochAccount::decode(&bytes_of(&mut context, fixture.epoch_account).await).unwrap();
    assert_eq!(epoch_now.phase, EPOCH_PHASE_CLEARED);

    // The freeze records the verified virtual legs and opens the pot, whose
    // scalars all start at economic zero: nothing has been collected yet, and
    // `SettlePage` is what fills and drains them.
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, split.id),
        342,
    )
    .await;
    result.unwrap();
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(pot.candidate, split.id);
    assert_eq!(pot.phase, POT_PHASE_OPEN);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.rounding_pot_price_units, 0);
}

/// A verified partial-fill candidate clears slice by slice, and every
/// remainder comes back exactly once.
///
/// The first gap exhibit of the PartialFillLedger wave, flipped: a
/// marginal-pro-rata book — one strict buy of eight against two marginal
/// sells of six, canonical fills `[8, 4, 4]` — driven all the way from
/// admission to two consumed receipts.  The buy end fragments across two
/// counterparties and completes on its second slice; each sell end fills
/// partially and returns its two unfilled Eggs on completion.
#[tokio::test]
async fn partial_fill_candidate_clears_slice_by_slice_and_returns_every_remainder() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    // One strict buy against two marginal sells: canonical fills are
    // [8, 4, 4] — both sells fill partially, pro rata.
    let orders = [
        (0, fixture.single(&fixture.owners[0], 1, 0, 0, 8, 3_000)),
        (1, fixture.single(&fixture.owners[1], 2, 0, 1, 6, 2_500)),
        (2, fixture.single(&fixture.owners[2], 3, 0, 1, 6, 2_500)),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let witness = witness_of(&[
        slice(0, LegRefV1::Order(1), 0, 4),
        slice(0, LegRefV1::Order(2), 0, 4),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let partial = plan_submission(&fixture, &epoch, &book, prices, 0, witness);
    assert_eq!(partial.fills, vec![8, 4, 4]);

    submit_seal(&mut context, &fixture, &partial, Some(2), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &partial, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(&mut context, &[fixture.finalize(&[partial.id])], None, 340).await;
    result.unwrap();

    // Every per-owner conversion is a whole atom, so the pot freezes empty.
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, partial.id),
        342,
    )
    .await;
    result.unwrap();

    let position = |index: usize| fixture.owners[index].position;
    let cash_before = owner_cash(&mut context, &fixture).await;
    let eggs_before = book_eggs(&mut context, &fixture, &reservations, 0).await;

    /* Slice 0: the buyer's first touch stamps its *whole order's* eight Egg
     * atoms, not this slice's four, and the seller's stamps its four. */
    let (result, units) = send(
        &mut context,
        &[fixture.entitle_single(payer, partial.id, 0, reservations[0], reservations[1])],
        None,
        343,
    )
    .await;
    result.unwrap();
    eprintln!("EntitleSlice (fragmented buy) CU: {units}");
    let buyer = read_reservation(&mut context, reservations[0]).await;
    assert_eq!(buyer.state, RESERVATION_STATE_ENTITLED);
    assert_eq!(buyer.entitled_units, 8);
    assert_eq!(buyer.consumed_units, 0);
    let seller_one = read_reservation(&mut context, reservations[1]).await;
    assert_eq!(seller_one.entitled_units, 4);
    assert_eq!(seller_one.initial_internal[0], 6);

    /* Out of order on purpose: slice 0 is consumed before slice 1 is even
     * entitled, so the later entitlement touch meets a drawn-down envelope. */
    let (result, units) = send(
        &mut context,
        &[fixture.settle_single(
            partial.id,
            1,
            position(0),
            position(1),
            reservations[0],
            reservations[1],
            0,
        )],
        None,
        344,
    )
    .await;
    result.unwrap();
    eprintln!("SettlePage (partial slice) CU: {units}");
    let buyer = read_reservation(&mut context, reservations[0]).await;
    assert_eq!(buyer.state, RESERVATION_STATE_ENTITLED);
    assert_eq!(buyer.consumed_units, 4);
    // initial = consumed + remaining + released, per cash: 3 = 1 + 2 + 0.
    assert_eq!(buyer.initial_cash_atoms, 3);
    assert_eq!(buyer.remaining_cash_atoms, 2);
    // The first seller reached its stamped total, so it completed: CONSUMED,
    // empty envelope, and its two unfilled Eggs are back in its Position.
    let seller_one = read_reservation(&mut context, reservations[1]).await;
    assert_eq!(seller_one.state, RESERVATION_STATE_CONSUMED);
    assert_eq!(seller_one.consumed_units, seller_one.entitled_units);
    assert!(seller_one.remaining_is_zero());
    assert_eq!(
        read_position(&mut context, position(1)).await.internal[0],
        START_EGGS - 6 + 2
    );

    /* A filled order's remainder returns only through completion: the
     * owner-signed terminal release keeps refusing a nonzero fill. */
    let refused = send(
        &mut context,
        &[fixture.release_cleared(&fixture.owners[2], reservations[2], partial.id, 2)],
        Some(&fixture.owners[2].key),
        345,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::MismatchedState as u32);

    /* A forged stamp cannot survive recomputation.  The buyer's ledger is
     * rewritten in place to claim twelve entitled Egg atoms instead of eight
     * — internally consistent bytes, four consumed of twelve — and the next
     * slice of that same order re-derives the total from the digest-verified
     * feed and refuses the mismatch before minting anything. */
    let buyer_address = reservations[0];
    let honest_buyer = account(&mut context, buyer_address).await.unwrap();
    let mut forged = ReservationAccount::decode(&honest_buyer.data).unwrap();
    forged.entitled_units = 12;
    forged.validate().unwrap();
    let mut forged_account = honest_buyer.clone();
    forged_account.data = encode(RESERVATION_ACCOUNT_BYTES, |out| forged.encode(out));
    context.set_account(&buyer_address, &AccountSharedData::from(forged_account));
    let refused = send(
        &mut context,
        &[fixture.entitle_single(payer, partial.id, 1, reservations[0], reservations[2])],
        None,
        346,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        codec_code(clutch_solana_layout::CodecError::MismatchedBinding)
    );
    assert!(account(&mut context, fixture.receipt(partial.id, 1))
        .await
        .is_none());
    context.set_account(&buyer_address, &AccountSharedData::from(honest_buyer));

    // Slice 1: the buyer is already ENTITLED and drawn down; its stamp is
    // re-derived and required equal, and the second seller stamps its own.
    let (result, _) = send(
        &mut context,
        &[fixture.entitle_single(payer, partial.id, 1, reservations[0], reservations[2])],
        None,
        347,
    )
    .await;
    result.unwrap();
    assert_eq!(
        read_reservation(&mut context, reservations[0])
            .await
            .entitled_units,
        8
    );

    /* A receipt may only be consumed against its own pair.  Slice 1's receipt
     * names the second seller; presenting the first seller's reservation — a
     * completed one from the same fragmented buy — refuses. */
    let crossed = send(
        &mut context,
        &[fixture.settle_single(
            partial.id,
            2,
            position(0),
            position(1),
            reservations[0],
            reservations[1],
            1,
        )],
        None,
        348,
    )
    .await;
    assert_eq!(custom(crossed.0), ClutchError::MismatchedState as u32);

    // Consuming it completes the buy end: the remaining cash — price
    // improvement and unfilled refund in one number — is released once.
    let (result, _) = send(
        &mut context,
        &[fixture.settle_single(
            partial.id,
            2,
            position(0),
            position(2),
            reservations[0],
            reservations[2],
            1,
        )],
        None,
        349,
    )
    .await;
    result.unwrap();

    for (at, address) in reservations.iter().enumerate() {
        let reservation = read_reservation(&mut context, *address).await;
        assert_eq!(
            reservation.state, RESERVATION_STATE_CONSUMED,
            "reservation {at}"
        );
        assert_eq!(reservation.consumed_units, reservation.entitled_units);
        assert!(reservation.remaining_is_zero());
        assert_eq!(reservation.fee_debited_atoms, 0);
        assert_eq!(reservation.fee_carry_numerator, 0);
    }

    // The exact per-owner numbers: the buyer paid two atoms for eight Eggs
    // and got its whole reserve back above that; each seller took one atom
    // and its two unfilled Eggs.
    let buyer_position = read_position(&mut context, position(0)).await;
    assert_eq!(buyer_position.cash_atoms, START_CASH - 2);
    assert_eq!(buyer_position.reserved_cash_atoms, 0);
    assert_eq!(buyer_position.internal[0], START_EGGS + 8);
    for seller in [1usize, 2] {
        let account = read_position(&mut context, position(seller)).await;
        assert_eq!(account.cash_atoms, START_CASH + 1);
        assert_eq!(account.reserved_cash_atoms, 0);
        assert_eq!(account.internal[0], START_EGGS - 6 + 2);
    }
    // Whole-plane closure: cash and Eggs are conserved across the book.
    assert_eq!(owner_cash(&mut context, &fixture).await, cash_before);
    assert_eq!(
        book_eggs(&mut context, &fixture, &reservations, 0).await,
        eggs_before
    );

    // Replay on an exhausted receipt refuses and moves nothing.
    let watched = [position(0), position(2), reservations[0], reservations[2]];
    let before = snapshot(&mut context, &watched).await;
    let replay = send(
        &mut context,
        &[fixture.settle_single(
            partial.id,
            2,
            position(0),
            position(2),
            reservations[0],
            reservations[2],
            1,
        )],
        None,
        350,
    )
    .await;
    assert_eq!(custom(replay.0), ClutchError::MismatchedState as u32);
    assert_eq!(snapshot(&mut context, &watched).await, before);

    // Re-entitling a slice whose ends are already CONSUMED refuses before any
    // write: a terminal reservation cannot take a new slice.
    let replay = send(
        &mut context,
        &[fixture.entitle_single(payer, partial.id, 1, reservations[0], reservations[2])],
        None,
        351,
    )
    .await;
    assert_eq!(custom(replay.0), ClutchError::MismatchedState as u32);
}

/// A book whose slices pair a portfolio sell against single-Egg buys clears
/// through the per-slice seam, one leg at a time.
///
/// The second gap exhibit of the wave, flipped.  The portfolio end is
/// entitled once, stamped with its whole basket's eight Egg atoms — its
/// filled lots times its coefficient sum — and completes only when both of
/// its legs have been consumed by different counterparties.
#[tokio::test]
async fn mixed_portfolio_and_single_book_clears_leg_by_leg() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    // One portfolio sell of one lot of [4, 4] — four Eggs on outcome 0 and
    // four on outcome 1 — against one single buy per outcome.  Every slice
    // converts exactly: 4 * 2_500 is a whole collateral atom.
    let orders = [
        (3, fixture.portfolio(&fixture.owners[3], 1, 1, [4, 4], 1, 1)),
        (0, fixture.single(&fixture.owners[0], 2, 0, 0, 4, 3_000)),
        (1, fixture.single(&fixture.owners[1], 3, 1, 0, 4, 3_000)),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let witness = witness_of(&[
        slice(1, LegRefV1::Order(0), 0, 4),
        slice(2, LegRefV1::Order(0), 1, 4),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let mixed = plan_submission(&fixture, &epoch, &book, prices, 0, witness);
    assert_eq!(mixed.fills, vec![1, 4, 4]);

    submit_seal(&mut context, &fixture, &mixed, Some(2), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &mixed, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(&mut context, &[fixture.finalize(&[mixed.id])], None, 340).await;
    result.unwrap();

    // Every per-owner conversion is whole, so the pot freezes empty.
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, mixed.id),
        342,
    )
    .await;
    result.unwrap();

    let position = |index: usize| fixture.owners[index].position;
    let cash_before = owner_cash(&mut context, &fixture).await;
    let eggs_before = [
        book_eggs(&mut context, &fixture, &reservations, 0).await,
        book_eggs(&mut context, &fixture, &reservations, 1).await,
    ];

    // Both legs entitle and consume, in slice order.  The buy ends are
    // single-Egg and complete immediately; the portfolio sell completes only
    // on the second leg.
    for slice_index in [0u16, 1] {
        let buyer = 1 + slice_index as usize;
        let (result, units) = send(
            &mut context,
            &[fixture.entitle_single(
                payer,
                mixed.id,
                slice_index,
                reservations[buyer],
                reservations[0],
            )],
            None,
            343 + slice_index as u32,
        )
        .await;
        result.unwrap();
        eprintln!("EntitleSlice (mixed leg {slice_index}) CU: {units}");
        // The basket's stamp is its filled lots times its coefficient sum,
        // and it is written once: the second touch re-derives and agrees.
        let seller = read_reservation(&mut context, reservations[0]).await;
        assert_eq!(seller.entitled_units, 8);
        assert_eq!(
            seller.order_kind,
            clutch_solana_layout::ORDER_KIND_PORTFOLIO
        );

        let (result, units) = send(
            &mut context,
            &[fixture.settle_single(
                mixed.id,
                u64::from(slice_index) + 1,
                position(buyer - 1),
                position(3),
                reservations[buyer],
                reservations[0],
                slice_index,
            )],
            None,
            345 + slice_index as u32,
        )
        .await;
        result.unwrap();
        eprintln!("SettlePage (mixed leg {slice_index}) CU: {units}");
        let seller = read_reservation(&mut context, reservations[0]).await;
        assert_eq!(seller.consumed_units, 4 * (u64::from(slice_index) + 1));
        assert_eq!(
            seller.state,
            if slice_index == 1 {
                RESERVATION_STATE_CONSUMED
            } else {
                RESERVATION_STATE_ENTITLED
            }
        );
        assert_eq!(
            read_reservation(&mut context, reservations[buyer])
                .await
                .state,
            RESERVATION_STATE_CONSUMED
        );
    }

    // The basket sold whole, so nothing came back to it; each buyer holds its
    // leg and paid one atom of the two it reserved.
    let seller_position = read_position(&mut context, position(3)).await;
    assert_eq!(seller_position.cash_atoms, START_CASH + 2);
    assert_eq!(seller_position.internal[0], START_EGGS - 4);
    assert_eq!(seller_position.internal[1], START_EGGS - 4);
    for (buyer, outcome) in [(0usize, 0usize), (1, 1)] {
        let account = read_position(&mut context, position(buyer)).await;
        assert_eq!(account.cash_atoms, START_CASH - 1);
        assert_eq!(account.reserved_cash_atoms, 0);
        assert_eq!(account.internal[outcome], START_EGGS + 4);
    }
    assert_eq!(owner_cash(&mut context, &fixture).await, cash_before);
    for outcome in [0usize, 1] {
        assert_eq!(
            book_eggs(&mut context, &fixture, &reservations, outcome).await,
            eggs_before[outcome]
        );
    }
}

/// A witness whose slices do not convert **settles**, because the conversion
/// was never per slice.
///
/// Two buyers of eight against two sellers of eight, paired six-and-two
/// crosswise at a quarter-scale price.  No single slice is a whole number of
/// atoms at 2,500 per Egg, but every *order* moves twenty thousand price units
/// — two whole atoms — so the model's rounding pot is empty, and the seam's
/// conversion of each end's cumulative value telescopes to exactly that.  The
/// partial-fill wave recorded this shape as its standing residual; the
/// rounding-pot realization retires it.
///
/// Watch the six-and-two: buyer zero's first slice debits both its atoms at
/// once (six Eggs is fifteen thousand price units, which rounds up to two) and
/// its second debits none, while seller two's two slices credit one each.  The
/// slice numbers are lopsided; the order numbers are exact.
#[tokio::test]
async fn slices_that_do_not_convert_settle_through_the_cumulative_conversion() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let orders = [
        (0, fixture.single(&fixture.owners[0], 1, 0, 0, 8, 3_000)),
        (1, fixture.single(&fixture.owners[1], 2, 0, 0, 8, 3_000)),
        (2, fixture.single(&fixture.owners[2], 3, 0, 1, 8, 2_000)),
        (3, fixture.single(&fixture.owners[3], 4, 0, 1, 8, 2_000)),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    // Crosswise six-and-two: every owner totals eight, no slice is a whole
    // number of atoms at 2_500 per Egg.
    let witness = witness_of(&[
        slice(0, LegRefV1::Order(2), 0, 6),
        slice(0, LegRefV1::Order(3), 0, 2),
        slice(1, LegRefV1::Order(2), 0, 2),
        slice(1, LegRefV1::Order(3), 0, 6),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let strand = plan_submission(&fixture, &epoch, &book, prices, 0, witness);
    assert_eq!(strand.fills, vec![8, 8, 8, 8]);

    submit_seal(&mut context, &fixture, &strand, Some(4), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &strand, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(&mut context, &[fixture.finalize(&[strand.id])], None, 340).await;
    result.unwrap();

    // The per-owner conversions are exact, so the pot freezes empty and stays
    // CLOSED: nothing here is expected to go unallocated.
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, strand.id),
        342,
    )
    .await;
    result.unwrap();
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(pot.rounding_pot_price_units, 0);
    assert_eq!(pot.phase, POT_PHASE_CLOSED);

    let position = |index: usize| fixture.owners[index].position;
    let cash_before = owner_cash(&mut context, &fixture).await;
    let eggs_before = book_eggs(&mut context, &fixture, &reservations, 0).await;

    // Every slice entitles and consumes, in slice order.  The per-slice atom
    // legs are lopsided; the per-order totals are not.
    let legs = [(0u16, 0usize, 2usize), (1, 0, 3), (2, 1, 2), (3, 1, 3)];
    for (slice_index, buyer, seller) in legs {
        let (result, units) = send(
            &mut context,
            &[fixture.entitle_single(
                payer,
                strand.id,
                slice_index,
                reservations[buyer],
                reservations[seller],
            )],
            None,
            343 + slice_index as u32,
        )
        .await;
        result.unwrap();
        eprintln!("EntitleSlice (strand {slice_index}) CU: {units}");
        assert_eq!(
            read_reservation(&mut context, reservations[buyer])
                .await
                .entitled_units,
            8
        );

        let (result, units) = send(
            &mut context,
            &[fixture.settle_single(
                strand.id,
                u64::from(slice_index) + 1,
                position(buyer),
                position(seller),
                reservations[buyer],
                reservations[seller],
                slice_index,
            )],
            None,
            360 + slice_index as u32,
        )
        .await;
        result.unwrap();
        eprintln!("SettlePage (strand {slice_index}) CU: {units}");
    }

    // Buyers reserved three atoms each against a limit of 3,000 and paid two;
    // sellers received two each.  Nothing was left over anywhere, so the whole
    // plane conserves to the atom.
    for buyer in [0usize, 1] {
        let account = read_position(&mut context, position(buyer)).await;
        assert_eq!(account.cash_atoms, START_CASH - 2);
        assert_eq!(account.reserved_cash_atoms, 0);
        assert_eq!(account.internal[0], START_EGGS + 8);
    }
    for seller in [2usize, 3] {
        let account = read_position(&mut context, position(seller)).await;
        assert_eq!(account.cash_atoms, START_CASH + 2);
        assert_eq!(account.internal[0], START_EGGS - 8);
    }
    for address in &reservations {
        let reservation = read_reservation(&mut context, *address).await;
        assert_eq!(reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(reservation.consumed_units, 8);
    }
    assert_eq!(owner_cash(&mut context, &fixture).await, cash_before);
    assert_eq!(
        book_eggs(&mut context, &fixture, &reservations, 0).await,
        eggs_before
    );
    // The pot never moved: an exact epoch expects no residue and realizes none.
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(pot.rounding_pot_price_units, 0);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
}

/// The rounding pot, funded and drained on a real bank.
///
/// One buyer of five at a limit of half the scale against one seller of five
/// at a fifth, cleared at a quarter: each end's whole-order value is 12,500
/// price units against a scale of 10,000.  The relation's terminal-owner
/// conversion rounds the payer **up** to two atoms and the payee **down** to
/// one, so its verified `rounding_pot` is 7,500 + 2,500 = 10,000 price units —
/// exactly one collateral atom, which is `debited - credited`.
///
/// The pot holds no value at any point.  It is created OPEN carrying that
/// expectation, the completing slice draws the whole of it down, and the atom
/// itself is simply never credited: the book's owners end one atom lighter and
/// it stays unallocated in the market's collateral pool.  The pot reaching
/// zero is the closure `CloseGeneralPot` demands.
#[tokio::test]
async fn an_inexact_book_funds_the_pot_and_drains_it_to_empty() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let orders = [
        (0, fixture.single(&fixture.owners[0], 1, 0, 0, 5, 5_000)),
        (1, fixture.single(&fixture.owners[1], 2, 0, 1, 5, 2_000)),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let witness = witness_of(&[slice(0, LegRefV1::Order(1), 0, 5)]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let inexact = plan_submission(&fixture, &epoch, &book, prices, 0, witness);
    assert_eq!(inexact.fills, vec![5, 5]);

    submit_seal(&mut context, &fixture, &inexact, Some(1), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &inexact, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(&mut context, &[fixture.finalize(&[inexact.id])], None, 340).await;
    result.unwrap();

    // The freeze records the verified expectation instead of refusing it, and
    // the pot opens rather than closing empty.
    let (result, units) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, inexact.id),
        342,
    )
    .await;
    result.unwrap();
    eprintln!("FreezeEntitlement (inexact) CU: {units}");
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(pot.rounding_pot_price_units, 10_000);
    assert_eq!(pot.phase, clutch_solana_layout::POT_PHASE_OPEN);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);

    let position = |index: usize| fixture.owners[index].position;
    let cash_before = owner_cash(&mut context, &fixture).await;
    let eggs_before = book_eggs(&mut context, &fixture, &reservations, 0).await;

    let (result, units) = send(
        &mut context,
        &[fixture.entitle_single(payer, inexact.id, 0, reservations[0], reservations[1])],
        None,
        343,
    )
    .await;
    result.unwrap();
    eprintln!("EntitleSlice (inexact) CU: {units}");
    let receipt = SettlementReceiptAccount::decode(
        &bytes_of(&mut context, fixture.receipt(inexact.id, 0)).await,
    )
    .unwrap();
    // The receipt records the exact price-unit value; nothing rounds here.
    assert_eq!(receipt.consideration_price_units, 12_500);

    // The seven-account shape cannot realize residue it has no pot to draw
    // from, and refuses before a byte moves.
    let watched = [
        position(0),
        position(1),
        reservations[0],
        reservations[1],
        fixture.receipt(inexact.id, 0),
        fixture.pot(),
    ];
    let before = snapshot(&mut context, &watched).await;
    let refused = send(
        &mut context,
        &[fixture.settle_single(
            inexact.id,
            1,
            position(0),
            position(1),
            reservations[0],
            reservations[1],
            0,
        )],
        None,
        344,
    )
    .await;
    assert_eq!(custom(refused.0), ClutchError::AccountCount as u32);
    assert_eq!(snapshot(&mut context, &watched).await, before);

    // A pot whose expectation is short of what the slice realizes refuses
    // rather than going negative.
    let honest = bytes_of(&mut context, fixture.pot()).await;
    let mut short_value = pot;
    short_value.rounding_pot_price_units = 5_000;
    let mut short = vec![0u8; account_len::FINAL_POT];
    short_value.encode(&mut short).unwrap();
    context.set_account(&fixture.pot(), &program_account(short).into());
    let refused = send(
        &mut context,
        &[fixture.settle_single_potted(
            inexact.id,
            1,
            position(0),
            position(1),
            reservations[0],
            reservations[1],
            0,
        )],
        None,
        345,
    )
    .await;
    assert_eq!(
        custom(refused.0),
        ClutchError::AggregateClosureMismatch as u32
    );
    context.set_account(&fixture.pot(), &program_account(honest).into());

    let (result, units) = send(
        &mut context,
        &[fixture.settle_single_potted(
            inexact.id,
            1,
            position(0),
            position(1),
            reservations[0],
            reservations[1],
            0,
        )],
        None,
        346,
    )
    .await;
    result.unwrap();
    eprintln!("SettlePage (potted) CU: {units}");

    // The payer paid two atoms, the payee received one, and the pot is empty.
    let buyer = read_position(&mut context, position(0)).await;
    let seller = read_position(&mut context, position(1)).await;
    assert_eq!(buyer.cash_atoms, START_CASH - 2);
    assert_eq!(buyer.reserved_cash_atoms, 0);
    assert_eq!(buyer.internal[0], START_EGGS + 5);
    assert_eq!(seller.cash_atoms, START_CASH + 1);
    assert_eq!(seller.internal[0], START_EGGS - 5);
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(pot.rounding_pot_price_units, 0);
    assert_eq!(pot.pot_cash_price_units, 0);
    assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);

    // Eggs conserve exactly; cash conserves less exactly one atom, which is
    // the verified pot divided by the price scale.  Nothing holds that atom.
    assert_eq!(
        book_eggs(&mut context, &fixture, &reservations, 0).await,
        eggs_before
    );
    assert_eq!(
        owner_cash(&mut context, &fixture).await,
        cash_before - (10_000 / PRICE_SCALE)
    );
    for address in &reservations {
        let reservation = read_reservation(&mut context, *address).await;
        assert_eq!(reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(reservation.consumed_units, 5);
    }
    // Replay on the exhausted receipt refuses with the pot untouched.
    let refused = send(
        &mut context,
        &[fixture.settle_single_potted(
            inexact.id,
            1,
            position(0),
            position(1),
            reservations[0],
            reservations[1],
            0,
        )],
        None,
        347,
    )
    .await;
    assert!(refused.0.is_err());
}

/// The honest refusal this wave leaves behind: the relation converts per
/// **owner**, a reservation carries only its own order.
///
/// One owner holds two filled buy orders of five against a seller of ten, at a
/// quarter scale.  The relation sums that owner's two orders to 25,000 price
/// units and rounds **once** — three atoms, with a 5,000-unit residue — while
/// the runtime can only round each order on its own, which would take four
/// atoms and realize 17,500 units of residue against a verified pot of 10,000.
///
/// The seam does not try.  `distinct_owners == filled_order_count` is
/// recomputed from the digest-verified feed, it fails here two against three,
/// and `EntitleSlice` refuses before any receipt exists.
#[tokio::test]
async fn two_filled_orders_for_one_owner_refuse_the_inexact_conversion() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let orders = [
        (0, fixture.single(&fixture.owners[0], 1, 0, 0, 5, 5_000)),
        (0, fixture.single(&fixture.owners[0], 2, 0, 0, 5, 5_000)),
        (1, fixture.single(&fixture.owners[1], 3, 0, 1, 10, 2_000)),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;

    let witness = witness_of(&[
        slice(0, LegRefV1::Order(2), 0, 5),
        slice(1, LegRefV1::Order(2), 0, 5),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let split_owner = plan_submission(&fixture, &epoch, &book, prices, 0, witness);
    assert_eq!(split_owner.fills, vec![5, 5, 10]);

    submit_seal(&mut context, &fixture, &split_owner, Some(2), &[], 100).await;
    walk_to_verdict(&mut context, &fixture, &split_owner, &reservations, 300).await;
    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.finalize(&[split_owner.id])],
        None,
        340,
    )
    .await;
    result.unwrap();

    // The freeze opens: the *owner* arithmetic is consistent, and the pot
    // records one atom of expected residue.
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(payer, split_owner.id),
        342,
    )
    .await;
    result.unwrap();
    let pot = FinalPotAccount::decode(&bytes_of(&mut context, fixture.pot()).await).unwrap();
    assert_eq!(pot.rounding_pot_price_units, 10_000);
    // Two participating owners, three filled orders: the coincidence the
    // per-order realization needs does not hold, and every slice refuses.
    let record = CandidateRecord::decode(
        &bytes_of(&mut context, fixture.candidate_record(split_owner.id)).await,
    )
    .unwrap();
    assert_eq!(record.distinct_owners, 2);
    for (slice_index, buyer) in [(0u16, 0usize), (1, 1)] {
        let refused = send(
            &mut context,
            &[fixture.entitle_single(
                payer,
                split_owner.id,
                slice_index,
                reservations[buyer],
                reservations[2],
            )],
            None,
            343 + slice_index as u32,
        )
        .await;
        assert_eq!(custom(refused.0), ClutchError::NotYetImplemented as u32);
        assert!(
            account(&mut context, fixture.receipt(split_owner.id, slice_index))
                .await
                .is_none()
        );
    }
    for address in &reservations {
        let reservation = read_reservation(&mut context, *address).await;
        assert_eq!(reservation.state, RESERVATION_STATE_ACTIVE);
        assert_eq!(reservation.entitled_units, 0);
    }
}

/// A lapsed epoch's reservations stand ACTIVE and the only release path
/// refuses honestly: no post-lapse release or expiry path exists yet, which
/// is the standing blocker the settlement ledger records.
#[tokio::test]
async fn lapsed_epoch_reservations_stand_under_the_expiry_blocker() {
    let (mut context, fixture) = start().await;
    let payer = context.payer.pubkey();
    let orders = [
        (0, fixture.single(&fixture.owners[0], 1, 0, 0, 8, 3_000)),
        (1, fixture.single(&fixture.owners[1], 2, 0, 1, 8, 2_000)),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (_, _, reservations) = frozen_state(&mut context, &fixture).await;

    // Nothing submitted: the deadline lapses the epoch honestly.
    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(&mut context, &[fixture.finalize(&[])], None, 100).await;
    result.unwrap();
    let epoch_now =
        EpochAccount::decode(&bytes_of(&mut context, fixture.epoch_account).await).unwrap();
    assert_eq!(epoch_now.phase, EPOCH_PHASE_LAPSED);

    // The reservations stand ACTIVE with their whole envelopes...
    for address in &reservations {
        let reservation = read_reservation(&mut context, *address).await;
        assert_eq!(reservation.state, RESERVATION_STATE_ACTIVE);
        assert_eq!(
            reservation.remaining_cash_atoms,
            reservation.initial_cash_atoms
        );
        assert_eq!(reservation.remaining_internal, reservation.initial_internal);
    }
    // ...and the cancellation path requires an OPEN epoch and refuses
    // honestly.  The post-lapse exit is the owner-signed terminal release
    // (tag 60), a different intent with its own evidence in
    // `terminal_closure.rs`; cancellation itself never bends.
    let release = send(
        &mut context,
        &[fixture.cancel(&fixture.owners[0], canonical_order_id(1), 2)],
        Some(&fixture.owners[0].key),
        101,
    )
    .await;
    assert_eq!(custom(release.0), ClutchError::NotActive as u32);
    let _ = payer;
}
