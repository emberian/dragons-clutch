//! The general-epoch bank harness the `vpot_*` campaigns drive.
//!
//! Copied from `entitled_clearing.rs`'s scaffolding and extended with the
//! four seam-plane accounts a virtual leg needs — the Hoard, the reference
//! kernel aggregate, the two-term supply ledger, and the Hoard's Token-2022
//! account — because a virtual leg is the one settlement that changes the
//! market's outstanding supply, and every account that truth lives in has to
//! be in the list.
//!
//! Items are `pub` because a `tests/` submodule is compiled once per test
//! binary that declares it; unused ones are expected and allowed.
#![allow(dead_code, unused_imports)]
use {
    clutch_batch::relation_v1::{
        canonical_candidate, LegRefV1, PairingSliceV1, PairingWitnessV1, RelationDomainV1,
    },
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
    },
    clutch_kernel::{BasisMode, PayoutSet, PayoutVector},
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
        HoardAccount, MarketAccount, OrderRecord, OrderSlot, PortfolioRecord, PositionAccount,
        PriceGridAccount, SettlementReceiptAccount, SupplyLedgerAccount, CANDIDATE_STATUS_SELECTED,
        EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED, FEED_FILLS_PER_CHUNK,
        FEED_SLICES_PER_CHUNK, MAX_GRID_TICKS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
        POT_PHASE_CLOSED, RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
        RECEIPT_FLAG_SLICE_EXHAUSTED,
    },
    clutch_solana_reference::{KernelAccount, KERNEL_ACCOUNT_LEN},
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_terms, layout_request, request_heap_frame_data,
        token_account_bytes, BASE_TOKEN_ACCOUNT_LEN, COMPUTE_BUDGET, PROGRAM_ID, RENT_SYSVAR,
        SYSTEM_PROGRAM, TOKEN_2022,
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

pub const PRICE_SCALE: u64 = 10_000;
pub const EPOCH_INDEX: u64 = 7;
pub const FREEZE_DEADLINE: u64 = 500;
pub const CU_LIMIT: u32 = 1_400_000;
pub const HEAP_FRAME: u32 = 262_144;
pub const WALLET: u64 = 5_000_000_000;
pub const OUTCOMES: u8 = 4;
/// Every position starts with this much cash and this many Eggs per outcome.
pub const START_CASH: u64 = 1_000_000;
pub const START_EGGS: u64 = 1_000;
/// Complete sets this market's collateral cap leaves room to mint.
///
/// Deliberately small: a campaign that mints more than this must refuse on
/// `ClutchError::CollateralCap`, which is the seam plane's own rule reaching
/// the clearing plane's mint unchanged.
pub const MINT_HEADROOM: u64 = 8;

pub fn h(byte: u8) -> Hash32 {
    Hash32::from_bytes([byte; 32])
}

pub fn pda(prefix: &[u8], suffixes: &[&[u8]]) -> (Address, u8) {
    let mut all = Vec::with_capacity(1 + suffixes.len());
    all.push(prefix);
    all.extend_from_slice(suffixes);
    Address::find_program_address(&all, &PROGRAM_ID)
}

pub fn rent_exempt(len: usize) -> u64 {
    solana_rent::Rent::default().minimum_balance(len).max(1)
}

pub fn clock_address() -> Address {
    Address::new_from_array(CLOCK_SYSVAR_ID.to_bytes())
}

/// One program-owned account image, for the hostile substitutions that write
/// bytes no instruction could have produced.
pub fn program_account(data: Vec<u8>) -> Account {
    Account {
        lamports: rent_exempt(data.len()),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn encode<F>(len: usize, writer: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, clutch_solana_layout::CodecError>,
{
    let mut bytes = vec![0; len];
    assert_eq!(writer(&mut bytes).unwrap(), len);
    bytes
}

pub fn add_state(test: &mut ProgramTest, address: Address, data: Vec<u8>) {
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

pub fn system_slot(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    }
}

/// The T2-5 zero-sentinel domain, verbatim, from the frozen epoch.
pub fn zero_sentinel_domain(epoch: &EpochAccount) -> RelationDomainV1 {
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

pub struct Owner {
    pub key: Keypair,
    pub id: Hash32,
    pub position: Address,
}

pub struct Fixture {
    pub market: Hash32,
    pub epoch_id: Hash32,
    pub policy_digest: Hash32,
    pub market_account: Address,
    pub terms_account: Address,
    pub grid_account: Address,
    pub policy_account: Address,
    pub epoch_account: Address,
    pub window_account: Address,
    pub page: Address,
    pub owners: Vec<Owner>,
    /// The Hoard collateral-accounting account.
    pub hoard_account: Address,
    /// The reference-only kernel aggregate.
    pub kernel_account: Address,
    /// The market-wide two-term supply ledger.
    pub supply_account: Address,
    /// The Hoard's Token-2022 collateral account, mirrored and never moved.
    pub hoard_token_account: Address,
}

/// One candidate's host-computed coordinates and its account addresses.
pub struct Submission {
    pub id: Hash32,
    pub prices: [u64; MAX_OUTCOMES],
    pub virtual_split: u64,
    pub virtual_merge: u64,
    pub fills: Vec<u64>,
    pub witness: PairingWitnessV1,
    pub record: Address,
    pub feed: Address,
}

impl Fixture {
    pub fn reservation(&self, owner: Hash32, order_id: Hash32) -> Address {
        let id = canonical_reservation_id(self.market, self.epoch_id, owner, 0, order_id);
        pda(seeds::SEED_RESERVATION, &[&id.bytes()]).0
    }

    pub fn candidate_feed(&self, candidate: Hash32) -> Address {
        pda(
            seeds::SEED_CANDIDATE_FEED,
            &[&self.epoch_id.bytes(), &candidate.bytes()],
        )
        .0
    }

    pub fn candidate_record(&self, candidate: Hash32) -> Address {
        pda(
            seeds::SEED_CANDIDATE,
            &[&self.epoch_id.bytes(), &candidate.bytes()],
        )
        .0
    }

    pub fn clear_work(&self, candidate: Hash32) -> Address {
        pda(
            seeds::SEED_CLEAR_WORK,
            &[&self.epoch_id.bytes(), &candidate.bytes()],
        )
        .0
    }

    pub fn pot(&self) -> Address {
        pda(seeds::SEED_POT, &[&self.epoch_id.bytes()]).0
    }

    pub fn receipt(&self, candidate: Hash32, slice_index: u16) -> Address {
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

    pub fn init_epoch(&self, payer: Address) -> Instruction {
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

    pub fn init_page(&self, payer: Address) -> Instruction {
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

    pub fn place(&self, owner: &Owner, sequence: u64, slot: OrderSlot) -> Instruction {
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

    pub fn cancel(&self, owner: &Owner, order_id: Hash32, generation: u64) -> Instruction {
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

    pub fn freeze(&self) -> Instruction {
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

    pub fn submit(
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

    pub fn write_chunk(
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

    pub fn seal(&self, submission: &Submission, _retained: &[Hash32]) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.window_account, false),
            AccountMeta::new(submission.feed, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        assert_eq!(metas.len(), SEAL_CANDIDATE_FIXED_ACCOUNT_COUNT);
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

    pub fn finalize(&self, retained: &[Hash32]) -> Instruction {
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

    pub fn init_clear_work(&self, payer: Address, candidate: Hash32) -> Instruction {
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

    pub fn grow_clear_work(&self, candidate: Hash32, sequence: u64) -> Instruction {
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

    pub fn advance(
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

    pub fn advance_slices(&self, candidate: Hash32, max_slices: u16) -> Instruction {
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

    pub fn complete(&self, candidate: Hash32, retained: &[Hash32]) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new(self.clear_work(candidate), false),
            AccountMeta::new(self.candidate_record(candidate), false),
            AccountMeta::new(self.window_account, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        assert_eq!(metas.len(), COMPLETE_CLEAR_WORK_ACCOUNT_COUNT);
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

    pub fn freeze_entitlement(&self, payer: Address, candidate: Hash32) -> Instruction {
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
    pub fn entitle_prefix(&self, payer: Address, candidate: Hash32) -> Vec<AccountMeta> {
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

    pub fn entitle_single(
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

    pub fn entitle_pair(
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
    pub fn settle_single(
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
    pub fn settle_single_potted(
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

    /// The virtual-leg shape: one real end, the pot, and the five accounts
    /// one pooled complete-set mint authenticates.
    pub fn settle_virtual(
        &self,
        candidate: Hash32,
        sequence: u64,
        position: Address,
        reservation: Address,
        slice_index: u16,
    ) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new(position, false),
            AccountMeta::new(reservation, false),
            AccountMeta::new(self.receipt(candidate, slice_index), false),
            AccountMeta::new(self.pot(), false),
            AccountMeta::new_readonly(self.market_account, false),
            AccountMeta::new(self.hoard_account, false),
            AccountMeta::new(self.kernel_account, false),
            AccountMeta::new(self.supply_account, false),
            AccountMeta::new_readonly(self.hoard_token_account, false),
        ];
        assert_eq!(metas.len(), orders_batch::SETTLE_VIRTUAL_ACCOUNT_COUNT);
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

    /// One virtual-slice entitlement: the single real end's reservation and
    /// the one creatable receipt.
    pub fn entitle_virtual(
        &self,
        payer: Address,
        candidate: Hash32,
        slice_index: u16,
        reservation: Address,
    ) -> Instruction {
        let mut metas = self.entitle_prefix(payer, candidate);
        metas.push(AccountMeta::new_readonly(self.page, false));
        metas.push(AccountMeta::new(reservation, false));
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

    #[allow(clippy::too_many_arguments)] // one argument per account role
    pub fn settle_pair(
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
    pub fn release_cleared(
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

    pub fn single(
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

    pub fn portfolio(
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
pub fn general_terms(
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

pub async fn start() -> (ProgramTestContext, Fixture) {
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
    /* The shared fixture's 1000-atom cap predates a market whose positions
     * open holding claims: four positions times `START_EGGS` is already four
     * times it, so a mint would refuse on the cap before it could refuse on
     * anything the campaign is about.  The cap is still real and still
     * checked — `MINT_HEADROOM` above the opening backing, so a candidate
     * that tried to mint past it would refuse. */
    terms.collateral_cap = 4 * START_EGGS + MINT_HEADROOM;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    let (terms_address, terms_bump) =
        pda(seeds::SEED_TERMS, &[&realm.bytes(), &terms.terms.bytes()]);
    terms.stored_bump = terms_bump;

    let (market_address, market_bump) = pda(seeds::SEED_MARKET, &[&realm.bytes(), &market.bytes()]);
    let (hoard_address, hoard_bump) = pda(seeds::SEED_HOARD, &[&market.bytes()]);
    let (kernel_address, _) = pda(seeds::SEED_KERNEL, &[&market.bytes()]);
    let (supply_address, supply_bump) = pda(seeds::SEED_SUPPLY, &[&market.bytes()]);
    let (hoard_authority_address, _) = pda(seeds::SEED_HOARD_AUTHORITY, &[&market.bytes()]);
    let (hoard_token_address, _) = pda(seeds::SEED_HOARD_TOKEN, &[&market.bytes()]);
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
        hoard_bump,
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

    /* The seam plane the general-epoch harness never needed until a virtual
     * leg made the market's outstanding supply move.  Every position starts
     * holding `START_EGGS` on every active outcome, so the ledgers that
     * account for those claims have to say so: the two-term closure is
     * `internal_supply + external_supply == kernel.total_supply` per outcome,
     * and the Active `DerivedBasis` collateral requirement is
     * `max_i total_supply[i]`.  The Hoard's token balance covers the whole
     * pool — every position's cash as well as the complete-set backing —
     * which is what makes a mint a reclassification rather than a creation. */
    let owner_count = 4u64;
    let backing = owner_count * START_EGGS;
    let mut ledger_terms = [0u64; MAX_OUTCOMES];
    ledger_terms[..OUTCOMES as usize].fill(backing);
    add_state(
        &mut test,
        hoard_address,
        encode(account_len::HOARD, |out| {
            HoardAccount {
                market,
                realm,
                authority: Hash32::from_bytes(hoard_authority_address.to_bytes()),
                collateral_atoms: backing,
                stored_bump: hoard_bump,
                flags: 0,
            }
            .encode(out)
        }),
    );
    let kernel_bytes = {
        let mut out = vec![0u8; KERNEL_ACCOUNT_LEN];
        KernelAccount {
            market,
            phase: 0,
            basis_mode: BasisMode::DerivedBasis,
            resolved_payout: 0,
            payouts: unit_payout_set(),
            total_supply: ledger_terms,
        }
        .encode(&mut out)
        .unwrap();
        out
    };
    add_state(&mut test, kernel_address, kernel_bytes);
    add_state(
        &mut test,
        supply_address,
        encode(account_len::SUPPLY_LEDGER, |out| {
            SupplyLedgerAccount {
                market,
                realm,
                generation: 0,
                outcome_count: OUTCOMES,
                internal_supply: ledger_terms,
                external_supply: [0; MAX_OUTCOMES],
                stored_bump: supply_bump,
                flags: 0,
            }
            .encode(out)
        }),
    );
    test.add_account(
        hoard_token_address,
        Account {
            lamports: rent_exempt(BASE_TOKEN_ACCOUNT_LEN),
            data: token_account_bytes(
                // The mirror reads a balance; the collateral mint's identity
                // is the collateral leg's business and no leg runs here.
                Address::new_from_array([0xc1_u8; 32]),
                hoard_authority_address,
                backing + owner_count * START_CASH,
            ),
            owner: TOKEN_2022,
            executable: false,
            rent_epoch: 0,
        },
    );

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
        hoard_account: hoard_address,
        kernel_account: kernel_address,
        supply_account: supply_address,
        hoard_token_account: hoard_token_address,
    };
    (test.start_with_context().await, fixture)
}

/// A degenerate unit payout set over the fixture's active outcomes.
///
/// `BasisMode::DerivedBasis` makes the Active collateral requirement
/// `max_i total_supply[i]` regardless of the preset vectors, so the set is
/// here to satisfy the kernel's shape validation and nothing else.
pub fn unit_payout_set() -> PayoutSet {
    let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
    for (index, slot) in vectors.iter_mut().enumerate().take(OUTCOMES as usize) {
        let mut weights = [0u64; MAX_OUTCOMES];
        weights[index] = 1;
        *slot = PayoutVector::new(1, weights);
    }
    PayoutSet::new(OUTCOMES, OUTCOMES, vectors)
}

pub async fn send(
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

pub async fn send_walk(
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

pub async fn account(context: &mut ProgramTestContext, address: Address) -> Option<Account> {
    context.banks_client.get_account(address).await.unwrap()
}

pub async fn bytes_of(context: &mut ProgramTestContext, address: Address) -> Vec<u8> {
    account(context, address).await.unwrap().data
}

pub async fn snapshot(context: &mut ProgramTestContext, addresses: &[Address]) -> Vec<Vec<u8>> {
    let mut all = Vec::with_capacity(addresses.len());
    for address in addresses {
        all.push(bytes_of(context, *address).await);
    }
    all
}

pub fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

/// Place one book plan (with optional retirements), then freeze at the
/// deadline.
pub async fn build_frozen_book(
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
pub async fn frozen_state(
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
pub fn plan_submission(
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
pub async fn submit_seal(
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
pub async fn walk_to_verdict(
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
    let (result, _) = send_walk(context, fixture.complete(submission.id, &[]), nonce + 4).await;
    result.unwrap();
}

/// One explicit witness slice.
pub fn slice(buy: u8, sell_ref: LegRefV1, outcome: u8, quantity: u64) -> PairingSliceV1 {
    PairingSliceV1 {
        buy_ref: LegRefV1::Order(buy),
        sell_ref,
        outcome,
        quantity,
    }
}

/// Pack explicit slices into a canonical-padded witness.
pub fn witness_of(slices: &[PairingSliceV1]) -> PairingWitnessV1 {
    let mut witness = PairingWitnessV1::empty();
    witness.slices[..slices.len()].copy_from_slice(slices);
    witness.len = slices.len() as u16;
    witness
}

pub async fn read_reservation(
    context: &mut ProgramTestContext,
    address: Address,
) -> ReservationAccount {
    ReservationAccount::decode(&bytes_of(context, address).await).unwrap()
}

/// Every owner's free-plus-reserved cash, summed across the book.
pub async fn owner_cash(context: &mut ProgramTestContext, fixture: &Fixture) -> u64 {
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
pub async fn book_eggs(
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

pub async fn read_position(context: &mut ProgramTestContext, address: Address) -> PositionAccount {
    PositionAccount::decode(&bytes_of(context, address).await).unwrap()
}
