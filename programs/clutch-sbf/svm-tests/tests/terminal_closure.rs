//! Real-SBF evidence for TerminalClosure (tags 60-67) — the general clearing
//! plane's lifecycle end: the full T2-8 lifecycle driven to CLEARED, then
//! **everything closeable closed**, in dependency order, down to the epoch
//! root, with every lamport accounted; and the lapsed twin (freeze, no
//! verified candidate, lapse) released and closed the same way.
//!
//! The gates:
//! * the hostile terminal walk — for every close path: double-close refuses,
//!   close-before-economic-zero refuses, wrong-payer refuses, wrong sink
//!   refuses, wrong-order-in-the-DAG refuses, release of a filled or
//!   already-consumed reservation refuses, a non-owner release refuses —
//!   each an executed refusal on the same bank;
//! * exact conservation — every close credits **exactly the recorded
//!   principal to the exact recorded payer** (the general funding ledger's
//!   payer; the reservation's stored owner), every surplus (two injected
//!   donations here) lands at the frozen incinerator and is measured, and
//!   the machinery's whole pre-close rent inventory equals reclaimed plus
//!   burned to the lamport;
//! * the residual — after the root closes, the epoch's on-chain footprint is
//!   exactly the declared-permanent set: the sealed 64-byte batch-policy
//!   artifact (plus owner Positions and market infrastructure, which are not
//!   epoch machinery); the reclaimed total is printed as the headline;
//! * the recorded tolerance — a candidate submitted **without** its optional
//!   funding ledger is unclosable by design (its close refuses, no payer is
//!   guessed) and the epoch root closes past it, leaving its pair standing
//!   as the recorded residual.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.  The reference adapter
//! refuses every tag of the family with `UnsupportedIntent`; the oracle is
//! the layout codec plus lamport conservation on this real bank.

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
        instructions::orders_batch::terminal_closure::{
            CLOSE_CANDIDATE_FIXED_ACCOUNT_COUNT, CLOSE_CLEAR_WORK_FIXED_ACCOUNT_COUNT,
            CLOSE_EPOCH_FIXED_ACCOUNT_COUNT, CLOSE_PAGE_FIXED_ACCOUNT_COUNT,
            CLOSE_POT_FIXED_ACCOUNT_COUNT, CLOSE_RECEIPT_ACCOUNT_COUNT,
            CLOSE_RESERVATION_ACCOUNT_COUNT, GENERAL_NEUTRAL_SINK_V1,
            RELEASE_CLEARED_FIXED_ACCOUNT_COUNT, RELEASE_LAPSED_ACCOUNT_COUNT,
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        clearing::{CandidateFeedHeader, GeneralFundingLedgerV1, CANDIDATE_WINDOW_SLOTS},
        projection::{project_slot, OwnerInterner},
        reservation::{
            ReservationAccount, RESERVATION_ACCOUNT_BYTES, RESERVATION_STATE_ACTIVE,
            RESERVATION_STATE_RELEASED,
        },
        stream, CandidateFeedChunk, EpochAccount, Hash32, MarketAccount, OrderRecord, OrderSlot,
        PortfolioRecord, PositionAccount, PriceGridAccount, EPOCH_PHASE_CLEARED,
        EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED, FEED_FILLS_PER_CHUNK, FEED_SLICES_PER_CHUNK,
        MAX_GRID_TICKS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
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
const START_CASH: u64 = 1_000_000;
const START_EGGS: u64 = 1_000;
/// Post-creation donations injected to prove the surplus route: one onto a
/// consumed reservation archive, one onto an exhausted receipt.
const RESERVATION_DONATION: u64 = 7_777;
const RECEIPT_DONATION: u64 = 5_000;

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

fn sink_address() -> Address {
    Address::new_from_array(GENERAL_NEUTRAL_SINK_V1.to_bytes())
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
    /// The dedicated machinery funder: signs and funds every ledgered
    /// creation, so every ledgered close must pay exactly this wallet.
    keeper: Keypair,
    owners: Vec<Owner>,
}

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
        let id = clutch_solana_layout::reservation::canonical_reservation_id(
            self.market,
            self.epoch_id,
            owner,
            0,
            order_id,
        );
        pda(seeds::SEED_RESERVATION, &[&id.bytes()]).0
    }

    fn ledger(&self, target: Address) -> Address {
        pda(seeds::SEED_GENERAL_FUNDING, &[&target.to_bytes()]).0
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

    /// `InitEpoch` with the trailing funding ledger (the keeper funds).
    fn init_epoch(&self) -> Instruction {
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
                AccountMeta::new(self.keeper.pubkey(), true),
                AccountMeta::new_readonly(self.market_account, false),
                AccountMeta::new_readonly(self.terms_account, false),
                AccountMeta::new_readonly(self.grid_account, false),
                AccountMeta::new_readonly(self.policy_account, false),
                AccountMeta::new(self.epoch_account, false),
                AccountMeta::new(self.window_account, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(clock_address(), false),
                AccountMeta::new(self.ledger(self.epoch_account), false),
            ],
        )
    }

    /// `InitOrderPage` with the trailing funding ledger (the keeper funds).
    fn init_page(&self) -> Instruction {
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
                AccountMeta::new(self.keeper.pubkey(), true),
                AccountMeta::new(self.page, false),
                AccountMeta::new_readonly(self.market_account, false),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new(self.ledger(self.page), false),
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

    /// `SubmitCandidate`, with or without the trailing funding ledger.
    fn submit(
        &self,
        submission: &Submission,
        declared_slices: Option<u16>,
        ledgered: bool,
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(self.keeper.pubkey(), true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.window_account, false),
            AccountMeta::new(submission.record, false),
            AccountMeta::new(submission.feed, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        if ledgered {
            metas.push(AccountMeta::new(self.ledger(submission.record), false));
        }
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

    fn seal(&self, submission: &Submission, _retained: &[Hash32]) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.window_account, false),
            AccountMeta::new(submission.feed, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
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

    /// `InitClearWork` with the trailing funding ledger (the keeper funds).
    fn init_clear_work(&self, candidate: Hash32) -> Instruction {
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
                AccountMeta::new(self.keeper.pubkey(), true),
                AccountMeta::new(self.clear_work(candidate), false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new(self.ledger(self.clear_work(candidate)), false),
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
            vec![
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(self.candidate_feed(candidate), false),
                AccountMeta::new(self.clear_work(candidate), false),
            ],
        )
    }

    fn complete(&self, candidate: Hash32, retained: &[Hash32]) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new(self.clear_work(candidate), false),
            AccountMeta::new(self.candidate_record(candidate), false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        for retained_candidate in retained {
            metas.push(AccountMeta::new(
                self.candidate_record(*retained_candidate),
                false,
            ));
        }
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

    /// `FreezeEntitlement` with the trailing funding ledger.
    fn freeze_entitlement(&self, candidate: Hash32) -> Instruction {
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
            vec![
                AccountMeta::new(self.keeper.pubkey(), true),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(self.candidate_record(candidate), false),
                AccountMeta::new_readonly(self.clear_work(candidate), false),
                AccountMeta::new(self.pot(), false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new(self.ledger(self.pot()), false),
            ],
        )
    }

    /// `EntitleSlice`, single shape, with the receipt's funding ledger.
    fn entitle_single(
        &self,
        candidate: Hash32,
        slice_index: u16,
        buy_reservation: Address,
        sell_reservation: Address,
    ) -> Instruction {
        let receipt = self.receipt(candidate, slice_index);
        let metas = vec![
            AccountMeta::new(self.keeper.pubkey(), true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new_readonly(self.pot(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(self.page, false),
            AccountMeta::new(buy_reservation, false),
            AccountMeta::new(sell_reservation, false),
            AccountMeta::new(receipt, false),
            AccountMeta::new(self.ledger(receipt), false),
        ];
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

    /// `EntitleSlice`, portfolio-pair shape, with per-receipt ledgers.
    fn entitle_pair(
        &self,
        candidate: Hash32,
        slice_index: u16,
        buy_reservation: Address,
        sell_reservation: Address,
        receipt_slices: &[u16],
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(self.keeper.pubkey(), true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new_readonly(self.pot(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(self.page, false),
            AccountMeta::new_readonly(self.terms_account, false),
            AccountMeta::new(buy_reservation, false),
            AccountMeta::new(sell_reservation, false),
        ];
        for slice in receipt_slices {
            metas.push(AccountMeta::new(self.receipt(candidate, *slice), false));
        }
        for slice in receipt_slices {
            metas.push(AccountMeta::new(
                self.ledger(self.receipt(candidate, *slice)),
                false,
            ));
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

    #[allow(clippy::too_many_arguments)]
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
            vec![
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(self.candidate_record(candidate), false),
                AccountMeta::new(buyer_position, false),
                AccountMeta::new(seller_position, false),
                AccountMeta::new(buy_reservation, false),
                AccountMeta::new(sell_reservation, false),
                AccountMeta::new(self.receipt(candidate, slice_index), false),
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
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
            AccountMeta::new_readonly(self.page, false),
        ];
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

    /* ------------------------- TerminalClosure ------------------------- */

    /// Owner-signed release on a LAPSED epoch: the four-account shape.
    fn release_lapsed(&self, owner: &Owner, reservation: Address, sequence: u64) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(owner.key.pubkey(), true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(reservation, false),
            AccountMeta::new(owner.position, false),
        ];
        assert_eq!(metas.len(), RELEASE_LAPSED_ACCOUNT_COUNT);
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

    /// Owner-signed release on a CLEARED epoch: the zero-fill proof shape.
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
        assert_eq!(metas.len(), RELEASE_CLEARED_FIXED_ACCOUNT_COUNT + 1);
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

    fn close_receipt(
        &self,
        candidate: Hash32,
        slice_index: u16,
        recipient: Address,
        sink: Address,
    ) -> Instruction {
        let receipt = self.receipt(candidate, slice_index);
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(receipt, false),
            AccountMeta::new(self.ledger(receipt), false),
            AccountMeta::new(recipient, false),
            AccountMeta::new(sink, false),
        ];
        assert_eq!(metas.len(), CLOSE_RECEIPT_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::CloseGeneralReceipt {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                    slice_index,
                },
            ),
            metas,
        )
    }

    fn close_reservation(&self, reservation: Address, recipient: Address) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(reservation, false),
            AccountMeta::new_readonly(self.page, false),
            AccountMeta::new(recipient, false),
            AccountMeta::new(sink_address(), false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), CLOSE_RESERVATION_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::CloseGeneralReservation {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            metas,
        )
    }

    fn close_page(&self, live_reservations: &[Address]) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(self.page, false),
            AccountMeta::new(self.ledger(self.page), false),
            AccountMeta::new(self.keeper.pubkey(), false),
            AccountMeta::new(sink_address(), false),
        ];
        assert_eq!(metas.len(), CLOSE_PAGE_FIXED_ACCOUNT_COUNT);
        for reservation in live_reservations {
            metas.push(AccountMeta::new_readonly(*reservation, false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::CloseGeneralPage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index: 0,
                },
            ),
            metas,
        )
    }

    fn close_pot(&self) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(self.pot(), false),
            AccountMeta::new(self.ledger(self.pot()), false),
            AccountMeta::new(self.keeper.pubkey(), false),
            AccountMeta::new(sink_address(), false),
            AccountMeta::new_readonly(self.page, false),
        ];
        assert_eq!(metas.len(), CLOSE_POT_FIXED_ACCOUNT_COUNT + 1);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::CloseGeneralPot {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            metas,
        )
    }

    fn close_candidate(&self, candidate: Hash32, selected: bool) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(self.candidate_record(candidate), false),
            AccountMeta::new(self.candidate_feed(candidate), false),
            AccountMeta::new(self.ledger(self.candidate_record(candidate)), false),
            AccountMeta::new(self.keeper.pubkey(), false),
            AccountMeta::new(sink_address(), false),
        ];
        assert_eq!(metas.len(), CLOSE_CANDIDATE_FIXED_ACCOUNT_COUNT);
        if selected {
            metas.push(AccountMeta::new_readonly(self.pot(), false));
            metas.push(AccountMeta::new_readonly(self.page, false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::CloseGeneralCandidate {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            metas,
        )
    }

    /// `CloseGeneralClearWork`; `pot_proof` presents the live pot, and a
    /// closed-record checkpoint needs no tail at all.
    fn close_clear_work(&self, candidate: Hash32, pot_proof: bool) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new(self.clear_work(candidate), false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new(self.ledger(self.clear_work(candidate)), false),
            AccountMeta::new(self.keeper.pubkey(), false),
            AccountMeta::new(sink_address(), false),
        ];
        assert_eq!(metas.len(), CLOSE_CLEAR_WORK_FIXED_ACCOUNT_COUNT);
        if pot_proof {
            metas.push(AccountMeta::new_readonly(self.pot(), false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::CloseGeneralClearWork {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            metas,
        )
    }

    fn close_epoch(&self, retained: &[Hash32]) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(self.epoch_account, false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new(self.ledger(self.epoch_account), false),
            AccountMeta::new(self.keeper.pubkey(), false),
            AccountMeta::new(sink_address(), false),
            AccountMeta::new_readonly(self.pot(), false),
        ];
        assert_eq!(metas.len(), CLOSE_EPOCH_FIXED_ACCOUNT_COUNT);
        metas.push(AccountMeta::new_readonly(self.page, false));
        for candidate in retained {
            let record = self.candidate_record(*candidate);
            metas.push(AccountMeta::new_readonly(record, false));
            metas.push(AccountMeta::new_readonly(self.ledger(record), false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                clutch_solana_layout::Intent::CloseGeneralEpoch {
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

/// A four-outcome terms artifact (the `entitled_clearing` shape).
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

    let keeper = Keypair::new();
    test.add_account(keeper.pubkey(), system_slot(WALLET));

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
        keeper,
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
    extra_signer: Option<&Keypair>,
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
    let mut signers = vec![&context.payer];
    if let Some(signer) = extra_signer {
        signers.push(signer);
    }
    let transaction = Transaction::new_signed_with_payer(
        &[heap, budget, instruction],
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

async fn bytes_of(context: &mut ProgramTestContext, address: Address) -> Vec<u8> {
    account(context, address).await.unwrap().data
}

async fn lamports_of(context: &mut ProgramTestContext, address: Address) -> u64 {
    account(context, address)
        .await
        .map(|value| value.lamports)
        .unwrap_or(0)
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

/// Transfer lamports into an arbitrary account (a hostile donation), built
/// byte-for-byte so no interface-crate type dance is needed.
fn donate(from: Address, to: Address, lamports: u64) -> Instruction {
    let mut data = [0u8; 12];
    data[0..4].copy_from_slice(&2u32.to_le_bytes());
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    Instruction::new_with_bytes(
        SYSTEM_PROGRAM,
        &data,
        vec![AccountMeta::new(from, true), AccountMeta::new(to, false)],
    )
}

async fn build_frozen_book(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    orders: &[(usize, OrderSlot)],
    cancels: &[(usize, Hash32)],
) {
    let (result, _) = send(context, &[fixture.init_epoch()], Some(&fixture.keeper), 0).await;
    result.unwrap();
    let (result, _) = send(context, &[fixture.init_page()], Some(&fixture.keeper), 1).await;
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

async fn submit_seal(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    submission: &Submission,
    declared: Option<u16>,
    retained: &[Hash32],
    ledgered: bool,
    nonce: u32,
) {
    let (result, _) = send(
        context,
        &[fixture.submit(submission, declared, ledgered)],
        Some(&fixture.keeper),
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
        LegRefV1::Order(index) => clutch_solana_layout::clearing::LegRef::Order(index),
        LegRefV1::Split => clutch_solana_layout::clearing::LegRef::Split,
        LegRefV1::Merge => clutch_solana_layout::clearing::LegRef::Merge,
    };
    let all_slices: Vec<clutch_solana_layout::clearing::PairingSlice> = (0..submission.witness.len
        as usize)
        .map(|k| {
            let slice = submission.witness.slices[k];
            clutch_solana_layout::clearing::PairingSlice {
                buy_ref: leg(slice.buy_ref),
                sell_ref: leg(slice.sell_ref),
                outcome: slice.outcome,
                quantity: slice.quantity,
            }
        })
        .collect();
    for chunk_slices in all_slices.chunks(FEED_SLICES_PER_CHUNK) {
        let mut slices =
            [clutch_solana_layout::clearing::PairingSlice::PADDING; FEED_SLICES_PER_CHUNK];
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

async fn walk_to_verdict(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    submission: &Submission,
    reservations: &[Address],
    nonce: u32,
) {
    let creation = [
        fixture.init_clear_work(submission.id),
        fixture.grow_clear_work(submission.id, 1),
        fixture.grow_clear_work(submission.id, 2),
        fixture.grow_clear_work(submission.id, 3),
        fixture.grow_clear_work(submission.id, 4),
    ];
    let (result, _) = send(context, &creation, Some(&fixture.keeper), nonce).await;
    result.unwrap();
    let (result, _) = send_walk(
        context,
        fixture.advance(submission.id, 16, reservations),
        None,
        nonce + 1,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(
        context,
        fixture.advance_slices(submission.id, submission.witness.len),
        None,
        nonce + 2,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(
        context,
        fixture.advance(submission.id, 16, &[]),
        None,
        nonce + 3,
    )
    .await;
    result.unwrap();
    let (result, _) = send_walk(
        context,
        fixture.complete(submission.id, &[]),
        None,
        nonce + 4,
    )
    .await;
    result.unwrap();
}

fn slice(buy: u8, sell_ref: LegRefV1, outcome: u8, quantity: u64) -> PairingSliceV1 {
    PairingSliceV1 {
        buy_ref: LegRefV1::Order(buy),
        sell_ref,
        outcome,
        quantity,
    }
}

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

async fn read_position(context: &mut ProgramTestContext, address: Address) -> PositionAccount {
    PositionAccount::decode(&bytes_of(context, address).await).unwrap()
}

/// The recorded principal of one ledgered close: what the ledger says the
/// payer is owed, read from the live account before the close runs.
async fn recorded_principal(context: &mut ProgramTestContext, ledger: Address) -> u64 {
    GeneralFundingLedgerV1::decode(&bytes_of(context, ledger).await)
        .unwrap()
        .payer_principal_lamports
}

/// Run one close, asserting the recipient is credited exactly `expected`.
async fn close_exact(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    recipient: Address,
    expected: u64,
    nonce: u32,
) -> u64 {
    let before = lamports_of(context, recipient).await;
    let (result, _) = send(context, &[instruction], None, nonce).await;
    result.unwrap();
    let after = lamports_of(context, recipient).await;
    assert_eq!(
        after - before,
        expected,
        "the close credits exactly the recorded principal"
    );
    expected
}

/// THE HEADLINE: the full general lifecycle to CLEARED, the hostile terminal
/// walk, then everything closeable closed — and the residual is exactly the
/// declared-permanent set, with the reclaimed lamports printed.
#[tokio::test]
async fn cleared_epoch_closes_to_the_declared_permanent_set() {
    let (mut context, fixture) = start().await;
    let a = 0usize;
    let b = 1usize;
    let c = 2usize;
    let d = 3usize;

    // The T2-8 headline book: one retirement, a single-Egg crossing, a
    // portfolio full pair, and one ineligible (zero-fill) order.
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

    let witness = witness_of(&[
        slice(0, LegRefV1::Order(1), 0, 8),
        slice(2, LegRefV1::Order(3), 0, 6),
        slice(2, LegRefV1::Order(3), 1, 6),
    ]);
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let alpha = plan_submission(&fixture, &epoch, &book, prices, 0, witness);
    assert_eq!(alpha.fills, vec![8, 8, 3, 1, 0]);
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

    submit_seal(&mut context, &fixture, &alpha, Some(3), &[], true, 100).await;
    submit_seal(&mut context, &fixture, &beta, None, &[alpha.id], true, 160).await;
    walk_to_verdict(&mut context, &fixture, &alpha, &reservations, 300).await;

    // No close is admitted before terminality: the epoch is still FROZEN.
    let early = send(
        &mut context,
        &[fixture.close_candidate(beta.id, false)],
        None,
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
    let epoch_now =
        EpochAccount::decode(&bytes_of(&mut context, fixture.epoch_account).await).unwrap();
    assert_eq!(epoch_now.phase, EPOCH_PHASE_CLEARED);

    let (res_a, res_b, res_c, res_d, res_idle) = (
        reservations[0],
        reservations[1],
        reservations[2],
        reservations[3],
        reservations[4],
    );
    let res_cancelled = fixture.reservation(fixture.owners[b].id, canonical_order_id(1));
    let position = |index: usize| fixture.owners[index].position;

    // A filled order's ACTIVE reservation is NOT releasable on a CLEARED
    // epoch: the verified allocation owns its envelope.
    let filled = send(
        &mut context,
        &[fixture.release_cleared(&fixture.owners[a], res_a, alpha.id, 2)],
        Some(&fixture.owners[a].key),
        341,
    )
    .await;
    assert_eq!(custom(filled.0), ClutchError::MismatchedState as u32);

    // Freeze, entitle, and consume the whole verified allocation.
    let (result, _) = send_walk(
        &mut context,
        fixture.freeze_entitlement(alpha.id),
        Some(&fixture.keeper),
        342,
    )
    .await;
    result.unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.entitle_single(alpha.id, 0, res_a, res_b)],
        Some(&fixture.keeper),
        343,
    )
    .await;
    result.unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.entitle_pair(alpha.id, 1, res_c, res_d, &[1, 2])],
        Some(&fixture.keeper),
        344,
    )
    .await;
    result.unwrap();
    let (result, _) = send(
        &mut context,
        &[fixture.settle_single(alpha.id, 1, position(a), position(b), res_a, res_b, 0)],
        None,
        345,
    )
    .await;
    result.unwrap();

    // Close-before-economic-zero refuses: receipt 1 is entitled, unconsumed.
    let unconsumed = send(
        &mut context,
        &[fixture.close_receipt(alpha.id, 1, fixture.keeper.pubkey(), sink_address())],
        None,
        346,
    )
    .await;
    assert_eq!(custom(unconsumed.0), ClutchError::MismatchedState as u32);

    let (result, _) = send(
        &mut context,
        &[fixture.settle_pair(alpha.id, 2, position(c), position(d), res_c, res_d, &[1, 2])],
        None,
        347,
    )
    .await;
    result.unwrap();

    /* ------------------------------------------------------------------ */
    /* The machinery inventory, complete, before any close.                */
    /* ------------------------------------------------------------------ */
    let machinery = vec![
        fixture.epoch_account,
        fixture.window_account,
        fixture.page,
        fixture.pot(),
        fixture.clear_work(alpha.id),
        alpha.record,
        alpha.feed,
        beta.record,
        beta.feed,
        fixture.receipt(alpha.id, 0),
        fixture.receipt(alpha.id, 1),
        fixture.receipt(alpha.id, 2),
        res_a,
        res_b,
        res_c,
        res_d,
        res_idle,
        res_cancelled,
        fixture.ledger(fixture.epoch_account),
        fixture.ledger(fixture.page),
        fixture.ledger(fixture.clear_work(alpha.id)),
        fixture.ledger(alpha.record),
        fixture.ledger(beta.record),
        fixture.ledger(fixture.pot()),
        fixture.ledger(fixture.receipt(alpha.id, 0)),
        fixture.ledger(fixture.receipt(alpha.id, 1)),
        fixture.ledger(fixture.receipt(alpha.id, 2)),
    ];

    // Two hostile donations, injected after creation: their only exit is the
    // incinerator.
    let donor = context.payer.pubkey();
    let (result, _) = send(
        &mut context,
        &[
            donate(donor, res_b, RESERVATION_DONATION),
            donate(donor, fixture.receipt(alpha.id, 0), RECEIPT_DONATION),
        ],
        None,
        348,
    )
    .await;
    result.unwrap();

    let mut inventory = 0u64;
    for address in &machinery {
        inventory += lamports_of(&mut context, *address).await;
    }
    let sink_before = lamports_of(&mut context, sink_address()).await;
    let mut reclaimed = 0u64;

    /* ------------------------------------------------------------------ */
    /* The hostile walk, then the closes, in dependency order.             */
    /* ------------------------------------------------------------------ */

    // Wrong payer refuses: the recipient must be the exact recorded payer.
    let wrong_payer = send(
        &mut context,
        &[fixture.close_receipt(alpha.id, 0, fixture.owners[a].key.pubkey(), sink_address())],
        None,
        349,
    )
    .await;
    assert_eq!(custom(wrong_payer.0), ClutchError::MismatchedState as u32);
    // Wrong sink refuses: only the frozen incinerator receives surplus.
    let wrong_sink = send(
        &mut context,
        &[fixture.close_receipt(
            alpha.id,
            0,
            fixture.keeper.pubkey(),
            fixture.owners[a].key.pubkey(),
        )],
        None,
        350,
    )
    .await;
    assert_eq!(custom(wrong_sink.0), ClutchError::MismatchedState as u32);

    // Wrong DAG order refuses, top to bottom: page before the idle release,
    // pot before the page, the selected pair before the pot, the root before
    // everything.
    let live_order = [res_a, res_b, res_c, res_d, res_idle];
    let page_early = send(&mut context, &[fixture.close_page(&live_order)], None, 351).await;
    assert_eq!(custom(page_early.0), ClutchError::MismatchedState as u32);
    let pot_early = send(&mut context, &[fixture.close_pot()], None, 352).await;
    assert_eq!(custom(pot_early.0), ClutchError::MismatchedState as u32);
    let selected_early = send(
        &mut context,
        &[fixture.close_candidate(alpha.id, true)],
        None,
        353,
    )
    .await;
    assert_eq!(
        custom(selected_early.0),
        ClutchError::MismatchedState as u32
    );
    let work_early = send(
        &mut context,
        &[fixture.close_clear_work(alpha.id, false)],
        None,
        354,
    )
    .await;
    assert_eq!(custom(work_early.0), ClutchError::AccountCount as u32);
    let root_early = send(&mut context, &[fixture.close_epoch(&retained)], None, 355).await;
    assert_eq!(custom(root_early.0), ClutchError::MismatchedState as u32);
    // Reservation archives close only after their page.
    let archive_early = send(
        &mut context,
        &[fixture.close_reservation(res_a, fixture.owners[a].key.pubkey())],
        None,
        356,
    )
    .await;
    assert_eq!(custom(archive_early.0), ClutchError::MismatchedState as u32);
    // An ACTIVE reservation refuses its rent close outright: the envelope
    // still owns value — economic close precedes rent close.
    let still_active = send(
        &mut context,
        &[fixture.close_reservation(res_idle, fixture.owners[a].key.pubkey())],
        None,
        390,
    )
    .await;
    assert_eq!(custom(still_active.0), ClutchError::MismatchedState as u32);
    // Wrong payer refuses on the page path too: the ledger recorded the
    // keeper, not an order owner.
    let page_wrong_payer = send(
        &mut context,
        &[{
            let mut wrong = fixture.close_page(&[res_a, res_b, res_c, res_d, res_idle]);
            wrong.accounts[3] = AccountMeta::new(fixture.owners[a].key.pubkey(), false);
            wrong
        }],
        None,
        391,
    )
    .await;
    assert_eq!(
        custom(page_wrong_payer.0),
        ClutchError::MismatchedState as u32
    );

    // A non-owner cannot release someone else's reservation.
    let not_owner = send(
        &mut context,
        &[fixture.release_cleared(&fixture.owners[b], res_idle, alpha.id, 2)],
        Some(&fixture.owners[b].key),
        357,
    )
    .await;
    assert_eq!(custom(not_owner.0), ClutchError::UnauthorizedActor as u32);
    // A consumed reservation cannot release.
    let consumed = send(
        &mut context,
        &[fixture.release_cleared(&fixture.owners[a], res_a, alpha.id, 2)],
        Some(&fixture.owners[a].key),
        358,
    )
    .await;
    assert_eq!(custom(consumed.0), ClutchError::MismatchedState as u32);

    // The one legitimate release: the ineligible zero-fill order's envelope
    // returns to its owner's Position, owner-signed, fill proven zero.
    let before = read_position(&mut context, position(a)).await;
    assert_eq!(before.reserved_cash_atoms, 1);
    let (result, _) = send(
        &mut context,
        &[fixture.release_cleared(&fixture.owners[a], res_idle, alpha.id, 2)],
        Some(&fixture.owners[a].key),
        359,
    )
    .await;
    result.unwrap();
    let after = read_position(&mut context, position(a)).await;
    assert_eq!(after.reserved_cash_atoms, 0);
    assert_eq!(after.cash_atoms, before.cash_atoms);
    assert_eq!(
        read_reservation(&mut context, res_idle).await.state,
        RESERVATION_STATE_RELEASED
    );
    // Release replays refuse on state: the reservation is no longer ACTIVE.
    let replay = send(
        &mut context,
        &[fixture.release_cleared(&fixture.owners[a], res_idle, alpha.id, 3)],
        Some(&fixture.owners[a].key),
        360,
    )
    .await;
    assert_eq!(custom(replay.0), ClutchError::MismatchedState as u32);

    // Receipts close (economic zero proven), each paying the keeper exactly
    // its recorded principal; the injected receipt donation burns.
    for (slice_index, nonce) in [(0u16, 361u32), (1, 362), (2, 363)] {
        let receipt = fixture.receipt(alpha.id, slice_index);
        let owed = recorded_principal(&mut context, fixture.ledger(receipt)).await;
        reclaimed += close_exact(
            &mut context,
            fixture.close_receipt(
                alpha.id,
                slice_index,
                fixture.keeper.pubkey(),
                sink_address(),
            ),
            fixture.keeper.pubkey(),
            owed,
            nonce,
        )
        .await;
    }
    // Double-close refuses: the receipt account no longer exists.
    let twice = send(
        &mut context,
        &[fixture.close_receipt(alpha.id, 0, fixture.keeper.pubkey(), sink_address())],
        None,
        364,
    )
    .await;
    assert_eq!(custom(twice.0), ClutchError::WrongProgramOwner as u32);

    // The page closes: every live record's reservation proven settled or
    // released, in slot order.
    let owed = recorded_principal(&mut context, fixture.ledger(fixture.page)).await;
    reclaimed += close_exact(
        &mut context,
        fixture.close_page(&live_order),
        fixture.keeper.pubkey(),
        owed,
        365,
    )
    .await;

    // The reservation archives close, each paying its stored owner exactly
    // the rent-exempt principal; the injected donation on B's burns.
    for (address, owner_index, nonce) in [
        (res_a, a, 366u32),
        (res_b, b, 367),
        (res_c, c, 368),
        (res_d, d, 369),
        (res_idle, a, 370),
        (res_cancelled, b, 371),
    ] {
        reclaimed += close_exact(
            &mut context,
            fixture.close_reservation(address, fixture.owners[owner_index].key.pubkey()),
            fixture.owners[owner_index].key.pubkey(),
            rent_exempt(RESERVATION_ACCOUNT_BYTES),
            nonce,
        )
        .await;
    }

    // The pot closes behind the pages.
    let owed = recorded_principal(&mut context, fixture.ledger(fixture.pot())).await;
    reclaimed += close_exact(
        &mut context,
        fixture.close_pot(),
        fixture.keeper.pubkey(),
        owed,
        372,
    )
    .await;

    // Both candidate pairs close: the loser freely, the SELECTED pair only
    // now that the pot and pages are absent.
    let owed = recorded_principal(&mut context, fixture.ledger(beta.record)).await;
    reclaimed += close_exact(
        &mut context,
        fixture.close_candidate(beta.id, false),
        fixture.keeper.pubkey(),
        owed,
        373,
    )
    .await;
    let owed = recorded_principal(&mut context, fixture.ledger(alpha.record)).await;
    reclaimed += close_exact(
        &mut context,
        fixture.close_candidate(alpha.id, true),
        fixture.keeper.pubkey(),
        owed,
        374,
    )
    .await;

    // The checkpoint closes (its record is gone, so no tail is needed), then
    // the root: epoch and window together, against the emptied registry.
    let owed = recorded_principal(&mut context, fixture.ledger(fixture.clear_work(alpha.id))).await;
    reclaimed += close_exact(
        &mut context,
        fixture.close_clear_work(alpha.id, false),
        fixture.keeper.pubkey(),
        owed,
        375,
    )
    .await;
    let owed = recorded_principal(&mut context, fixture.ledger(fixture.epoch_account)).await;
    reclaimed += close_exact(
        &mut context,
        fixture.close_epoch(&retained),
        fixture.keeper.pubkey(),
        owed,
        376,
    )
    .await;

    // Every close path refuses its double: the accounts no longer exist, so
    // each attempt dies on the vanished target's role.
    for (instruction, nonce) in [
        (fixture.close_page(&live_order), 392u32),
        (fixture.close_pot(), 393),
        (fixture.close_candidate(beta.id, false), 394),
        (fixture.close_clear_work(alpha.id, false), 395),
        (fixture.close_epoch(&retained), 396),
        (
            fixture.close_reservation(res_a, fixture.owners[a].key.pubkey()),
            397,
        ),
    ] {
        let twice = send(&mut context, &[instruction], None, nonce).await;
        assert_eq!(custom(twice.0), ClutchError::WrongProgramOwner as u32);
    }

    /* ------------------------------------------------------------------ */
    /* The residual is exactly the declared-permanent set.                 */
    /* ------------------------------------------------------------------ */
    for address in &machinery {
        assert!(
            account(&mut context, *address).await.is_none(),
            "{address} is closed"
        );
    }
    // The declared-permanent residual: the sealed batch-policy artifact.
    let residual = lamports_of(&mut context, fixture.policy_account).await;
    assert_eq!(residual, rent_exempt(64));
    // Owner state and market infrastructure stand untouched.
    for owner in &fixture.owners {
        assert!(account(&mut context, owner.position).await.is_some());
    }
    assert!(account(&mut context, fixture.market_account)
        .await
        .is_some());

    // Conservation to the lamport: everything the machinery held is either
    // reclaimed principal or measured burn — and the burn is exactly the two
    // injected donations.
    let burned = lamports_of(&mut context, sink_address()).await - sink_before;
    assert_eq!(inventory, reclaimed + burned);
    assert_eq!(burned, RESERVATION_DONATION + RECEIPT_DONATION);

    eprintln!(
        "TerminalClosure headline (CLEARED epoch): machinery held {inventory} lamports; \
         reclaimed {reclaimed} to the exact recorded payers; burned {burned} at the incinerator; \
         residual (declared-permanent policy artifact) {residual}"
    );
}

/// The lapsed twin: freeze, no verified candidate, lapse — then release every
/// reservation, close all machinery, and tolerate exactly the unregistered
/// candidate the design says is unclosable.
#[tokio::test]
async fn lapsed_epoch_releases_and_closes_with_the_unregistered_residual() {
    let (mut context, fixture) = start().await;
    let a = 0usize;
    let b = 1usize;
    let orders = [
        (a, fixture.single(&fixture.owners[a], 1, 0, 0, 8, 3_000)),
        (b, fixture.single(&fixture.owners[b], 2, 0, 1, 8, 2_000)),
    ];
    build_frozen_book(&mut context, &fixture, &orders, &[]).await;
    let (epoch, book, reservations) = frozen_state(&mut context, &fixture).await;
    assert_eq!(book.len, 2);

    // One candidate, submitted WITHOUT its funding ledger and never walked:
    // the unregistered, unverified specimen.
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[..4].copy_from_slice(&[2_500, 2_500, 2_500, 2_500]);
    let gamma = plan_submission(
        &fixture,
        &epoch,
        &book,
        prices,
        0,
        PairingWitnessV1::empty(),
    );
    submit_seal(&mut context, &fixture, &gamma, None, &[], false, 100).await;

    // Release before terminality refuses: the epoch is FROZEN, not lapsed.
    let early = send(
        &mut context,
        &[fixture.release_lapsed(&fixture.owners[a], reservations[0], 2)],
        Some(&fixture.owners[a].key),
        150,
    )
    .await;
    assert_eq!(custom(early.0), ClutchError::NotActive as u32);

    context
        .warp_to_slot(FREEZE_DEADLINE + CANDIDATE_WINDOW_SLOTS)
        .unwrap();
    let (result, _) = send(&mut context, &[fixture.finalize(&[gamma.id])], None, 160).await;
    result.unwrap();
    let epoch_now =
        EpochAccount::decode(&bytes_of(&mut context, fixture.epoch_account).await).unwrap();
    assert_eq!(epoch_now.phase, EPOCH_PHASE_LAPSED);

    let machinery = vec![
        fixture.epoch_account,
        fixture.window_account,
        fixture.page,
        reservations[0],
        reservations[1],
        fixture.ledger(fixture.epoch_account),
        fixture.ledger(fixture.page),
    ];
    let mut inventory = 0u64;
    for address in &machinery {
        inventory += lamports_of(&mut context, *address).await;
    }
    let stranded =
        lamports_of(&mut context, gamma.record).await + lamports_of(&mut context, gamma.feed).await;
    let sink_before = lamports_of(&mut context, sink_address()).await;
    let mut reclaimed = 0u64;

    // Every lapsed ACTIVE reservation releases, owner-signed, whole envelope.
    for (index, owner_index) in [(0usize, a), (1, b)] {
        let owner = &fixture.owners[owner_index];
        let before = read_position(&mut context, owner.position).await;
        let held = read_reservation(&mut context, reservations[index]).await;
        assert_eq!(held.state, RESERVATION_STATE_ACTIVE);
        let (result, _) = send(
            &mut context,
            &[fixture.release_lapsed(owner, reservations[index], 2)],
            Some(&owner.key),
            170 + index as u32,
        )
        .await;
        result.unwrap();
        let after = read_position(&mut context, owner.position).await;
        assert_eq!(
            after.reserved_cash_atoms,
            before.reserved_cash_atoms - held.remaining_cash_atoms
        );
        for outcome in 0..MAX_OUTCOMES {
            assert_eq!(
                after.internal[outcome],
                before.internal[outcome] + held.remaining_internal[outcome]
            );
        }
    }

    // The unregistered candidate is unclosable by design: no ledger exists,
    // no payer is guessed, the close refuses.
    let unregistered = send(
        &mut context,
        &[fixture.close_candidate(gamma.id, false)],
        None,
        180,
    )
    .await;
    assert_eq!(
        custom(unregistered.0),
        ClutchError::WrongProgramOwner as u32
    );

    // Page, reservation archives, and the root close in dependency order —
    // the root tolerating exactly the unregistered registry member.
    let owed = recorded_principal(&mut context, fixture.ledger(fixture.page)).await;
    reclaimed += close_exact(
        &mut context,
        fixture.close_page(&[reservations[0], reservations[1]]),
        fixture.keeper.pubkey(),
        owed,
        181,
    )
    .await;
    for (index, owner_index) in [(0usize, a), (1, b)] {
        reclaimed += close_exact(
            &mut context,
            fixture.close_reservation(
                reservations[index],
                fixture.owners[owner_index].key.pubkey(),
            ),
            fixture.owners[owner_index].key.pubkey(),
            rent_exempt(RESERVATION_ACCOUNT_BYTES),
            182 + index as u32,
        )
        .await;
    }
    let owed = recorded_principal(&mut context, fixture.ledger(fixture.epoch_account)).await;
    reclaimed += close_exact(
        &mut context,
        fixture.close_epoch(&[gamma.id]),
        fixture.keeper.pubkey(),
        owed,
        184,
    )
    .await;

    // The residual: the declared-permanent policy artifact, plus exactly the
    // unregistered candidate pair the design records as unclosable.
    for address in &machinery {
        assert!(account(&mut context, *address).await.is_none());
    }
    assert!(account(&mut context, gamma.record).await.is_some());
    assert!(account(&mut context, gamma.feed).await.is_some());
    let burned = lamports_of(&mut context, sink_address()).await - sink_before;
    assert_eq!(inventory, reclaimed + burned);
    assert_eq!(burned, 0);

    eprintln!(
        "TerminalClosure headline (LAPSED epoch): machinery held {inventory} lamports; \
         reclaimed {reclaimed}; burned {burned}; unregistered candidate pair stands at \
         {stranded} lamports by design"
    );
}
