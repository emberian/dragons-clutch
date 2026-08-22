//! Shared harness for the **scale campaigns** (roadmap Phase S4).
//!
//! The existing general-plane suites each carry their own copy of the fixture,
//! and each pins one small shape: an eleven-tick grid, a one-page book, three
//! or four owners, a handful of slices.  That is what makes the W1 quote table
//! a table of *small-book observations* rather than of measured maxima.
//!
//! This module is the one parameterised builder those campaigns share, so a
//! campaign file states only its shape — tick count, page count, owner count,
//! order plan — and the wire it drives is the same wire every other suite
//! drives, from the same encoders the program decodes with.
//!
//! Three deliberate choices:
//!
//! * **Every created account carries its funding ledger.**  The keeper lane's
//!   finding is that a ledgered creation is a *different, heavier* shape than
//!   the row W1 quotes, and a keeper has no choice about sending it — an
//!   unledgered creation is unclosable by design.  So [`Plane::ledgered`]
//!   defaults on and the campaigns measure what a keeper actually sends.
//! * **Every CU observation is labelled and printed.**  [`Meter`] prints the
//!   established `"<label> CU: <units>"` grammar with a stable
//!   `scale.<campaign>/<route>` label, and its closing table names the worst
//!   route against the 1.4 M ceiling.
//! * **Nothing here models the protocol.**  Every account image is written by
//!   `clutch-solana-layout`'s own encoders; every candidate coordinate comes
//!   from `clutch-batch`'s own `canonical_candidate`/`canonical_pairing`.  A
//!   divergence is a divergence inside one crate.
//!
//! Claim plane: SBF-EXECUTED (bank), no promotion.

#![allow(dead_code)]

use {
    clutch_batch::relation_v1::{
        canonical_candidate, canonical_pairing, BookV1, LegRefV1, PairingSliceV1, PairingWitnessV1,
        RelationDomainV1,
    },
    clutch_batch_policy_identity::{
        batch_policy_digest, canonical_batch_policy_bytes,
        general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
    },
    clutch_sbf::{instructions::artifact::CLOCK_SYSVAR_ID, seeds},
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id,
        clearing::{CandidateFeedHeader, LegRef, PairingSlice},
        projection::{project_slot, OwnerInterner},
        reservation::{canonical_reservation_id, ReservationAccount},
        stream, CandidateFeedChunk, CandidateRecord, EpochAccount, Hash32, Intent, MarketAccount,
        OrderRecord, OrderSlot, PayoutVectorBytes, PortfolioRecord, PositionAccount,
        PriceGridAccount, TermsAccount, FEED_FILLS_PER_CHUNK, FEED_SLICES_PER_CHUNK,
        MAX_GRID_TICKS, MAX_KNOTS, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, MAX_PAYOUTS,
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
    solana_program_test::{ProgramTest, ProgramTestContext},
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

/// The per-transaction compute ceiling every campaign measures against.
pub const CU_CEILING: u64 = 1_400_000;
/// The limit each transaction requests (nonce-perturbed for uniqueness).
pub const CU_LIMIT: u32 = 1_400_000;
/// The heap frame the boxed `ClearWorkV1` walk needs.
pub const HEAP_FRAME: u32 = 262_144;
/// Wallet endowment: the keeper funds a whole epoch's machinery.
pub const WALLET: u64 = 500_000_000_000;

/* ---------------------------------------------------------------------- */
/* Primitives                                                              */
/* ---------------------------------------------------------------------- */

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

pub fn encode<F>(len: usize, writer: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, clutch_solana_layout::CodecError>,
{
    let mut bytes = vec![0; len];
    assert_eq!(writer(&mut bytes).unwrap(), len);
    bytes
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

pub fn program_account(data: Vec<u8>) -> Account {
    Account {
        lamports: rent_exempt(data.len()),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

/// Compose one campaign route name.  A campaign that drives a single plane
/// passes an empty phase; a multi-plane campaign passes the plane's name, so
/// its rows stay distinguishable in one log.
pub fn route(phase: &str, name: &str) -> String {
    if phase.is_empty() {
        name.to_string()
    } else {
        format!("{phase}.{name}")
    }
}

/* ---------------------------------------------------------------------- */
/* The compute meter                                                       */
/* ---------------------------------------------------------------------- */

/// Every labelled CU observation a campaign makes, printed as it is taken and
/// tabled at the end against the 1.4 M ceiling.
pub struct Meter {
    campaign: &'static str,
    rows: Vec<(String, u64)>,
}

impl Meter {
    pub fn new(campaign: &'static str) -> Self {
        Self {
            campaign,
            rows: Vec::new(),
        }
    }

    /// Record and print one observation in the harness log grammar.
    pub fn record(&mut self, route: &str, units: u64) -> u64 {
        let label = format!("scale.{}/{route}", self.campaign);
        eprintln!("{label} CU: {units}");
        self.rows.push((label, units));
        units
    }

    /// The heaviest observation taken so far, if any.
    pub fn worst(&self) -> Option<(&str, u64)> {
        self.rows
            .iter()
            .max_by_key(|(_, units)| *units)
            .map(|(label, units)| (label.as_str(), *units))
    }

    /// Print the campaign's closing table and assert every route fit.
    pub fn finish(&self) {
        eprintln!(
            "scale.{} ROUTES: {} observations",
            self.campaign,
            self.rows.len()
        );
        if let Some((label, units)) = self.worst() {
            eprintln!(
                "scale.{} WORST route={label} cu={units} ceiling={CU_CEILING} headroom={}",
                self.campaign,
                CU_CEILING - units
            );
            assert!(
                units < CU_CEILING,
                "{label} at {units} CU does not fit the {CU_CEILING} ceiling"
            );
        }
    }
}

/* ---------------------------------------------------------------------- */
/* The laboratory bank                                                     */
/* ---------------------------------------------------------------------- */

/// One book participant: a signing wallet and the identity it owns positions
/// under.
pub struct Owner {
    pub key: Keypair,
    pub id: Hash32,
}

/// The shape of one market's immutable plane.
#[derive(Clone, Copy, Debug)]
pub struct MarketSpec {
    /// Distinguishing byte of the canonical market identity.
    pub market_byte: u8,
    /// Active outcomes.
    pub outcomes: u8,
    /// Active grid ticks, `2..=MAX_GRID_TICKS`.
    pub tick_count: u8,
    /// Spacing between ticks; tick `i` is `i * spacing`.
    pub tick_spacing: u64,
    /// Exact integer price scale.
    pub price_scale: u64,
    /// Free cash every position starts with.
    pub start_cash: u64,
    /// Eggs every position starts with, on every active outcome.
    pub start_eggs: u64,
}

impl Default for MarketSpec {
    fn default() -> Self {
        Self {
            market_byte: 0x3c,
            outcomes: 4,
            tick_count: 11,
            tick_spacing: 500,
            price_scale: 10_000,
            start_cash: 1_000_000,
            start_eggs: 1_000,
        }
    }
}

/// One market's immutable plane, installed at genesis.
#[derive(Clone, Debug)]
pub struct Market {
    pub market: Hash32,
    pub realm: Hash32,
    pub profile: Hash32,
    pub outcomes: u8,
    pub price_scale: u64,
    pub tick_count: u8,
    pub tick_spacing: u64,
    pub market_account: Address,
    pub terms_account: Address,
    pub terms_digest: Hash32,
    pub grid_account: Address,
    pub grid_digest: Hash32,
    /// One Position address per Lab owner index.
    pub positions: Vec<Address>,
    pub start_cash: u64,
    pub start_eggs: u64,
}

impl Market {
    /// The exact limit value of a grid tick index.
    pub fn tick(&self, index: u8) -> u64 {
        assert!(index < self.tick_count, "tick {index} is not on this grid");
        u64::from(index) * self.tick_spacing
    }

    /// The highest grid tick, which is the deepest `tick_of` scan a placement
    /// can force.
    pub fn top_tick(&self) -> u64 {
        self.tick(self.tick_count - 1)
    }
}

/// The genesis builder: one bank, any number of markets, epochs, and owners.
pub struct Lab {
    test: ProgramTest,
    pub keeper: Keypair,
    pub owners: Vec<Owner>,
}

impl Lab {
    /// A bank carrying the real program ELF, one keeper wallet, and
    /// `owner_count` funded owner wallets.
    pub fn new(owner_count: usize) -> Self {
        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
        let keeper = Keypair::new();
        test.add_account(keeper.pubkey(), system_slot(WALLET));
        let mut owners = Vec::with_capacity(owner_count);
        for _ in 0..owner_count {
            let key = Keypair::new();
            let id = Hash32::from_bytes(key.pubkey().to_bytes());
            test.add_account(key.pubkey(), system_slot(WALLET));
            owners.push(Owner { key, id });
        }
        Self {
            test,
            keeper,
            owners,
        }
    }

    fn add_state(&mut self, address: Address, data: Vec<u8>) {
        self.test.add_account(address, program_account(data));
    }

    /// Install one market's grid, terms, market record, and every owner's
    /// Position.
    pub fn market(&mut self, spec: MarketSpec) -> Market {
        let realm = h(0x61);
        let profile = h(0x62);
        let feed = h(0x63);
        let market = h(spec.market_byte);
        assert!(spec.tick_count >= 2 && usize::from(spec.tick_count) <= MAX_GRID_TICKS);
        assert!(
            u64::from(spec.tick_count - 1) * spec.tick_spacing <= spec.price_scale,
            "the top tick must not exceed the price scale"
        );

        let mut ticks = [0u64; MAX_GRID_TICKS];
        for (index, slot) in ticks
            .iter_mut()
            .enumerate()
            .take(usize::from(spec.tick_count))
        {
            *slot = index as u64 * spec.tick_spacing;
        }
        let mut grid = PriceGridAccount {
            grid: Hash32::ZERO,
            realm,
            price_scale: spec.price_scale,
            tick_count: spec.tick_count,
            ticks,
            stored_bump: 0,
            flags: 0,
        };
        grid.grid = grid.recomputed_grid_id().unwrap();
        let (grid_address, grid_bump) =
            pda(seeds::SEED_GRID, &[&realm.bytes(), &grid.grid.bytes()]);
        grid.stored_bump = grid_bump;

        let mut terms = general_terms(realm, profile, feed, spec.outcomes);
        terms.price_grid = grid.grid;
        terms.terms = terms.recomputed_terms_digest().unwrap();
        let (terms_address, terms_bump) =
            pda(seeds::SEED_TERMS, &[&realm.bytes(), &terms.terms.bytes()]);
        terms.stored_bump = terms_bump;

        let (market_address, market_bump) =
            pda(seeds::SEED_MARKET, &[&realm.bytes(), &market.bytes()]);
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        for (index, slot) in outcomes
            .iter_mut()
            .enumerate()
            .take(usize::from(spec.outcomes))
        {
            *slot = canonical_outcome_id(market, index as u8);
        }
        let market_state = MarketAccount {
            market,
            realm,
            profile,
            terms: terms.terms,
            outcome_count: spec.outcomes,
            lifecycle: 0,
            stored_bump: market_bump,
            hoard_bump: 0,
            outcomes,
            feed,
            collateral_cap: terms.collateral_cap,
            created_slot: 0,
            reserved: Hash32::ZERO,
        };

        self.add_state(
            market_address,
            encode(account_len::MARKET, |out| market_state.encode(out)),
        );
        self.add_state(
            terms_address,
            encode(account_len::TERMS, |out| terms.encode(out)),
        );
        self.add_state(
            grid_address,
            encode(account_len::PRICE_GRID, |out| grid.encode(out)),
        );

        // Egg balances live only on the market's active outcomes: the
        // portfolio seam's canonical-padding rule refuses anything beyond.
        let mut start_eggs = [0u64; MAX_OUTCOMES];
        start_eggs[..usize::from(spec.outcomes)].fill(spec.start_eggs);
        let owner_ids: Vec<Hash32> = self.owners.iter().map(|owner| owner.id).collect();
        let mut positions = Vec::with_capacity(owner_ids.len());
        for id in owner_ids {
            let (position_address, position_bump) =
                pda(seeds::SEED_POSITION, &[&market.bytes(), &id.bytes()]);
            let position = PositionAccount {
                market,
                owner: id,
                generation: 0,
                internal: start_eggs,
                cash_atoms: spec.start_cash,
                reserved_cash_atoms: 0,
                stored_bump: position_bump,
                close_state: 0,
            };
            self.add_state(
                position_address,
                encode(account_len::POSITION, |out| position.encode(out)),
            );
            positions.push(position_address);
        }

        Market {
            market,
            realm,
            profile,
            outcomes: spec.outcomes,
            price_scale: spec.price_scale,
            tick_count: spec.tick_count,
            tick_spacing: spec.tick_spacing,
            market_account: market_address,
            terms_account: terms_address,
            terms_digest: terms.terms,
            grid_account: grid_address,
            grid_digest: grid.grid,
            positions,
            start_cash: spec.start_cash,
            start_eggs: spec.start_eggs,
        }
    }

    /// Derive one epoch's plane over an installed market and seal its frozen
    /// batch-policy artifact at the canonical per-epoch address.
    pub fn epoch(
        &mut self,
        market: &Market,
        epoch_index: u64,
        page_count: u16,
        freeze_deadline: u64,
    ) -> Plane {
        let epoch_id = canonical_epoch_id(market.market, epoch_index);
        let policy_digest =
            Hash32::from_bytes(batch_policy_digest(&GENERAL_CLEARING_POLICY_V1).unwrap().0);
        let (policy_address, _) = pda(
            seeds::SEED_BATCH_POLICY,
            &[&epoch_id.bytes(), &policy_digest.bytes()],
        );
        self.add_state(
            policy_address,
            canonical_batch_policy_bytes(&GENERAL_CLEARING_POLICY_V1)
                .unwrap()
                .to_vec(),
        );
        let (epoch_address, _) = pda(
            seeds::SEED_EPOCH,
            &[&market.market.bytes(), &epoch_index.to_le_bytes()],
        );
        let (window_address, _) = pda(
            seeds::SEED_EPOCH_WINDOW,
            &[&market.market.bytes(), &epoch_index.to_le_bytes()],
        );
        let pages: Vec<Address> = (0..page_count)
            .map(|index| pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &index.to_le_bytes()]).0)
            .collect();
        Plane {
            market: market.market,
            outcomes: market.outcomes,
            price_scale: market.price_scale,
            epoch_index,
            epoch_id,
            policy_digest,
            policy_account: policy_address,
            epoch_account: epoch_address,
            window_account: window_address,
            pages,
            market_account: market.market_account,
            terms_account: market.terms_account,
            grid_account: market.grid_account,
            positions: market.positions.clone(),
            freeze_deadline,
            ledgered: true,
        }
    }

    /// Start the bank; the owners come back so campaigns can sign with them.
    pub async fn start(self) -> (ProgramTestContext, Keypair, Vec<Owner>) {
        let Lab {
            test,
            keeper,
            owners,
        } = self;
        (test.start_with_context().await, keeper, owners)
    }
}

/// A terms artifact widened to `outcomes`, the general plane's shape.
pub fn general_terms(realm: Hash32, profile: Hash32, feed: Hash32, outcomes: u8) -> TermsAccount {
    let mut terms = fixture_terms(realm, profile, feed);
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    for outcome in 0..usize::from(outcomes) {
        let mut weights = [0; MAX_OUTCOMES];
        weights[outcome] = 1;
        payouts[outcome] = PayoutVectorBytes {
            denominator: 1,
            weights,
        };
        payout_map[outcome] = outcome as u8;
    }
    terms.outcome_count = outcomes;
    terms.payout_count = outcomes;
    terms.payouts = payouts;
    terms.payout_map = payout_map;
    // Degree-0 count rule: `outcome_count - 1` strictly increasing boundaries.
    let mut knots = [0u128; MAX_KNOTS];
    for (index, knot) in knots.iter_mut().enumerate().take(usize::from(outcomes) - 1) {
        *knot = index as u128 + 1;
    }
    terms.knot_count = outcomes - 1;
    terms.knots = knots;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    terms
}

/* ---------------------------------------------------------------------- */
/* One epoch's wire                                                        */
/* ---------------------------------------------------------------------- */

/// One general epoch's addresses and every instruction that drives it.
#[derive(Clone, Debug)]
pub struct Plane {
    pub market: Hash32,
    pub outcomes: u8,
    pub price_scale: u64,
    pub epoch_index: u64,
    pub epoch_id: Hash32,
    pub policy_digest: Hash32,
    pub policy_account: Address,
    pub epoch_account: Address,
    pub window_account: Address,
    pub pages: Vec<Address>,
    pub market_account: Address,
    pub terms_account: Address,
    pub grid_account: Address,
    pub positions: Vec<Address>,
    pub freeze_deadline: u64,
    /// Whether every creating instruction carries its funding ledger — the
    /// shape a keeper must actually send, since an unledgered creation is
    /// unclosable by design.
    pub ledgered: bool,
}

impl Plane {
    pub fn page_count(&self) -> u16 {
        self.pages.len() as u16
    }

    /// The extra derivation attempts of the three accounts `InitEpoch` names.
    ///
    /// A **lower bound**: the instruction derives at least these three, and
    /// may derive more this harness cannot enumerate.
    ///
    /// `find_program_address` counts a bump down from 255 and pays one
    /// `create_program_address` (≈1,500 CU) per attempt, so a route's cost
    /// carries a term proportional to `255 - bump` for every address the
    /// program derives.  Campaigns print it beside the CU so a quote model
    /// can carry the quantum instead of averaging it away.
    pub fn epoch_derivation_attempts(&self) -> u32 {
        let epoch = pda(
            seeds::SEED_EPOCH,
            &[&self.market.bytes(), &self.epoch_index.to_le_bytes()],
        )
        .1;
        let window = pda(
            seeds::SEED_EPOCH_WINDOW,
            &[&self.market.bytes(), &self.epoch_index.to_le_bytes()],
        )
        .1;
        let ledger = pda(
            seeds::SEED_GENERAL_FUNDING,
            &[&self.epoch_account.to_bytes()],
        )
        .1;
        u32::from(255 - epoch) + u32::from(255 - window) + u32::from(255 - ledger)
    }

    pub fn ledger(&self, target: Address) -> Address {
        pda(seeds::SEED_GENERAL_FUNDING, &[&target.to_bytes()]).0
    }

    pub fn reservation(&self, owner: Hash32, order_id: Hash32) -> Address {
        let id = canonical_reservation_id(self.market, self.epoch_id, owner, 0, order_id);
        pda(seeds::SEED_RESERVATION, &[&id.bytes()]).0
    }

    /// The extra `create_program_address` attempts `find_program_address` pays
    /// across every address one placement derives — the reservation, the
    /// target page, and the owner's Position — each `255 - bump`.
    pub fn placement_derivation_attempts(&self, owner: Hash32, order_id: Hash32) -> u32 {
        let id = canonical_reservation_id(self.market, self.epoch_id, owner, 0, order_id);
        let reservation = pda(seeds::SEED_RESERVATION, &[&id.bytes()]).1;
        let page = pda(
            seeds::SEED_PAGE,
            &[&self.epoch_id.bytes(), &0u16.to_le_bytes()],
        )
        .1;
        let position = pda(
            seeds::SEED_POSITION,
            &[&self.market.bytes(), &owner.bytes()],
        )
        .1;
        u32::from(255 - reservation) + u32::from(255 - page) + u32::from(255 - position)
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

    fn push_ledger(&self, metas: &mut Vec<AccountMeta>, target: Address) {
        if self.ledgered {
            metas.push(AccountMeta::new(self.ledger(target), false));
        }
    }

    pub fn init_epoch(&self, payer: Address) -> Instruction {
        let mut metas = vec![
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
        self.push_ledger(&mut metas, self.epoch_account);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitEpoch {
                    market: self.market,
                    epoch_index: self.epoch_index,
                    policy: self.policy_digest,
                    freeze_deadline_slot: self.freeze_deadline,
                },
            ),
            metas,
        )
    }

    pub fn init_page(&self, payer: Address, page_index: u16) -> Instruction {
        let page = self.pages[usize::from(page_index)];
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(page, false),
            AccountMeta::new_readonly(self.market_account, false),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        self.push_ledger(&mut metas, page);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitOrderPage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index,
                    page_count: self.page_count(),
                },
            ),
            metas,
        )
    }

    /// One placement.  `sequence` is the target page's current order count,
    /// which is the slot the record lands in.
    pub fn place(
        &self,
        signer: Address,
        position: Address,
        page_index: u16,
        sequence: u64,
        slot: OrderSlot,
    ) -> Instruction {
        let reservation = self.reservation(slot.owner(), slot.order_id());
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::PlaceOrder {
                    market: self.market,
                    epoch: self.epoch_id,
                    max_fee_atoms: 0,
                    slot,
                },
            ),
            vec![
                AccountMeta::new(signer, true),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(self.grid_account, false),
                AccountMeta::new(self.pages[usize::from(page_index)], false),
                AccountMeta::new(position, false),
                AccountMeta::new(reservation, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
        )
    }

    pub fn cancel(
        &self,
        signer: Address,
        owner: Hash32,
        position: Address,
        page_index: u16,
        order_id: Hash32,
        generation: u64,
    ) -> Instruction {
        let reservation = self.reservation(owner, order_id);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                generation,
                Intent::CancelOrder {
                    market: self.market,
                    epoch: self.epoch_id,
                    owner,
                    order_id,
                    generation,
                },
            ),
            vec![
                AccountMeta::new_readonly(signer, true),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new(self.pages[usize::from(page_index)], false),
                AccountMeta::new(position, false),
                AccountMeta::new(reservation, false),
            ],
        )
    }

    pub fn freeze(&self) -> Instruction {
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
                Intent::FreezeEpoch {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            metas,
        )
    }

    pub fn submit(
        &self,
        payer: Address,
        submission: &Submission,
        declared_slices: Option<u16>,
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.window_account, false),
            AccountMeta::new(submission.record, false),
            AccountMeta::new(submission.feed, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(clock_address(), false),
        ];
        self.push_ledger(&mut metas, submission.record);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::SubmitCandidate {
                    market: self.market,
                    epoch: self.epoch_id,
                    prices: submission.prices,
                    virtual_split: submission.virtual_split,
                    virtual_merge: submission.virtual_merge,
                    honored_aon_mask: submission.honored_aon_mask,
                    declared_slices,
                    weighted_direct_volume: submission.claims.0,
                    limit_surplus_price_units: submission.claims.1,
                    distinct_owners: submission.claims.2,
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
                Intent::WriteCandidateFeed {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate: submission.id,
                    chunk,
                },
            ),
            vec![AccountMeta::new(submission.feed, false)],
        )
    }

    pub fn seal(
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
                Intent::SealCandidate {
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
                Intent::FinalizeSelection {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            metas,
        )
    }

    pub fn init_clear_work(&self, payer: Address, candidate: Hash32) -> Instruction {
        let work = self.clear_work(candidate);
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(work, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        self.push_ledger(&mut metas, work);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitClearWork {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            metas,
        )
    }

    pub fn grow_clear_work(&self, candidate: Hash32, sequence: u64) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::GrowClearWork {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            vec![AccountMeta::new(self.clear_work(candidate), false)],
        )
    }

    /// One walk batch over the page at the checkpoint's current cursor.
    pub fn advance(
        &self,
        candidate: Hash32,
        page_index: u16,
        max_orders: u16,
        reservations: &[Address],
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new(self.clear_work(candidate), false),
            AccountMeta::new_readonly(self.pages[usize::from(page_index)], false),
        ];
        for reservation in reservations {
            metas.push(AccountMeta::new_readonly(*reservation, false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::AdvanceClearWork {
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
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::AdvanceClearSlices {
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

    pub fn complete(&self, candidate: Hash32) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::CompleteClearWork {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            vec![
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new_readonly(self.candidate_feed(candidate), false),
                AccountMeta::new(self.clear_work(candidate), false),
                AccountMeta::new(self.candidate_record(candidate), false),
            ],
        )
    }

    pub fn freeze_entitlement(&self, payer: Address, candidate: Hash32) -> Instruction {
        let pot = self.pot();
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new_readonly(self.clear_work(candidate), false),
            AccountMeta::new(pot, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        self.push_ledger(&mut metas, pot);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::FreezeEntitlement {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
            metas,
        )
    }

    /// The fixed prefix plus the **complete** frozen page set, in index order.
    fn entitle_prefix(&self, payer: Address, candidate: Hash32) -> Vec<AccountMeta> {
        let mut metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.epoch_account, false),
            AccountMeta::new_readonly(self.candidate_record(candidate), false),
            AccountMeta::new_readonly(self.candidate_feed(candidate), false),
            AccountMeta::new_readonly(self.pot(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        for page in &self.pages {
            metas.push(AccountMeta::new_readonly(*page, false));
        }
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
        let receipt = self.receipt(candidate, slice_index);
        let mut metas = self.entitle_prefix(payer, candidate);
        metas.push(AccountMeta::new(buy_reservation, false));
        metas.push(AccountMeta::new(sell_reservation, false));
        metas.push(AccountMeta::new(receipt, false));
        self.push_ledger(&mut metas, receipt);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::EntitleSlice {
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
        metas.push(AccountMeta::new_readonly(self.terms_account, false));
        metas.push(AccountMeta::new(buy_reservation, false));
        metas.push(AccountMeta::new(sell_reservation, false));
        for slice in receipt_slices {
            metas.push(AccountMeta::new(self.receipt(candidate, *slice), false));
        }
        if self.ledgered {
            for slice in receipt_slices {
                metas.push(AccountMeta::new(
                    self.ledger(self.receipt(candidate, *slice)),
                    false,
                ));
            }
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::EntitleSlice {
                    market: self.market,
                    epoch: self.epoch_id,
                    candidate,
                    slice_index,
                },
            ),
            metas,
        )
    }

    /// The seven-account direct-slice consumption.  `page_index` is the buy
    /// end's page, which the wire coordinate must name honestly.
    #[allow(clippy::too_many_arguments)] // one argument per account role
    pub fn settle_single(
        &self,
        candidate: Hash32,
        buyer_position: Address,
        seller_position: Address,
        buy_reservation: Address,
        sell_reservation: Address,
        slice_index: u16,
        page_index: u16,
    ) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                u64::from(slice_index) + 1,
                Intent::SettlePage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index,
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

    /// The same, plus the epoch's final pot for a completing end that realizes
    /// rounding residue.
    #[allow(clippy::too_many_arguments)] // one argument per account role
    pub fn settle_single_potted(
        &self,
        candidate: Hash32,
        buyer_position: Address,
        seller_position: Address,
        buy_reservation: Address,
        sell_reservation: Address,
        slice_index: u16,
        page_index: u16,
    ) -> Instruction {
        let mut instruction = self.settle_single(
            candidate,
            buyer_position,
            seller_position,
            buy_reservation,
            sell_reservation,
            slice_index,
            page_index,
        );
        instruction
            .accounts
            .push(AccountMeta::new(self.pot(), false));
        instruction
    }

    /// The portfolio full-pair consumption: one page when both ends share it,
    /// two otherwise, then the pair's receipts.
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
        buy_page: u16,
        sell_page: u16,
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
        metas.push(AccountMeta::new_readonly(
            self.pages[usize::from(buy_page)],
            false,
        ));
        if sell_page != buy_page {
            metas.push(AccountMeta::new_readonly(
                self.pages[usize::from(sell_page)],
                false,
            ));
        }
        for slice in receipt_slices {
            metas.push(AccountMeta::new(self.receipt(candidate, *slice), false));
        }
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::SettlePage {
                    market: self.market,
                    epoch: self.epoch_id,
                    page_index: buy_page,
                },
            ),
            metas,
        )
    }

    /// Owner-signed post-terminal release of a zero-fill reservation.
    pub fn release_cleared(
        &self,
        signer: Address,
        reservation: Address,
        position: Address,
        selected: Hash32,
        sequence: u64,
        page_index: u16,
    ) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::ReleaseTerminalReservation {
                    market: self.market,
                    epoch: self.epoch_id,
                },
            ),
            vec![
                AccountMeta::new_readonly(signer, true),
                AccountMeta::new_readonly(self.epoch_account, false),
                AccountMeta::new(reservation, false),
                AccountMeta::new(position, false),
                AccountMeta::new_readonly(self.candidate_record(selected), false),
                AccountMeta::new_readonly(self.candidate_feed(selected), false),
                AccountMeta::new_readonly(self.pages[usize::from(page_index)], false),
            ],
        )
    }
}

/* ---------------------------------------------------------------------- */
/* Order records                                                           */
/* ---------------------------------------------------------------------- */

pub fn single(
    owner: Hash32,
    rank: u64,
    outcome: u8,
    side: u8,
    quantity: u64,
    limit: u64,
    epoch_index: u64,
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
        expiry_epoch: epoch_index,
    })
}

pub fn portfolio(
    owner: Hash32,
    rank: u64,
    side: u8,
    coefficients_head: &[u64],
    lots: u64,
    limit_collateral_per_lot: u64,
    epoch_index: u64,
) -> OrderSlot {
    let mut coefficients = [0u64; MAX_OUTCOMES];
    coefficients[..coefficients_head.len()].copy_from_slice(coefficients_head);
    OrderSlot::Portfolio(PortfolioRecord {
        owner,
        order_id: canonical_order_id(rank),
        side,
        active_len: coefficients_head.len() as u8,
        flags: 0,
        coefficients,
        lots,
        limit_collateral_per_lot,
        minimum_fill_lots: 0,
        generation: 1,
        expiry_epoch: epoch_index,
    })
}

/* ---------------------------------------------------------------------- */
/* Transactions                                                            */
/* ---------------------------------------------------------------------- */

pub async fn send(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
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
    let mut all_signers = vec![&context.payer];
    all_signers.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        &all,
        Some(&context.payer.pubkey()),
        &all_signers,
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
pub async fn send_walk(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signers: &[&Keypair],
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
    let mut all_signers = vec![&context.payer];
    all_signers.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        &[heap, budget, instruction],
        Some(&context.payer.pubkey()),
        &all_signers,
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

pub async fn read_position(context: &mut ProgramTestContext, address: Address) -> PositionAccount {
    PositionAccount::decode(&bytes_of(context, address).await).unwrap()
}

pub async fn read_reservation(
    context: &mut ProgramTestContext,
    address: Address,
) -> ReservationAccount {
    ReservationAccount::decode(&bytes_of(context, address).await).unwrap()
}

pub async fn read_record(
    context: &mut ProgramTestContext,
    plane: &Plane,
    candidate: Hash32,
) -> CandidateRecord {
    CandidateRecord::decode(&bytes_of(context, plane.candidate_record(candidate)).await).unwrap()
}

/* ---------------------------------------------------------------------- */
/* The book plan and the frozen state                                      */
/* ---------------------------------------------------------------------- */

/// One order to place: which Lab owner signs it, and the record.
pub type PlannedOrder = (usize, OrderSlot);

/// Place a whole plan across the plane's pages, retire the named ranks, and
/// freeze at the deadline.  Returns every placement's CU, in plan order, so a
/// campaign can label the shapes it cares about.
#[allow(clippy::too_many_arguments)] // one argument per campaign coordinate
pub async fn build_frozen_book(
    context: &mut ProgramTestContext,
    plane: &Plane,
    keeper: &Keypair,
    owners: &[Owner],
    orders: &[PlannedOrder],
    cancels: &[u64],
    meter: &mut Meter,
    label: &str,
    nonce: u32,
) -> Vec<u64> {
    let keeper_key = keeper.pubkey();
    let (result, units) = send(context, &[plane.init_epoch(keeper_key)], &[keeper], nonce).await;
    result.unwrap();
    meter.record(&route(label, "init_epoch_ledgered"), units);

    for page_index in 0..plane.page_count() {
        let (result, units) = send(
            context,
            &[plane.init_page(keeper_key, page_index)],
            &[keeper],
            nonce + 1 + u32::from(page_index),
        )
        .await;
        result.unwrap();
        if page_index == 0 {
            meter.record(&route(label, "init_order_page_ledgered"), units);
        }
    }

    let per_page = MAX_ORDERS_PER_PAGE as u64;
    let mut placements = Vec::with_capacity(orders.len());
    let mut seen_single = false;
    let mut seen_portfolio = false;
    for (index, (owner_index, slot)) in orders.iter().enumerate() {
        let rank = index as u64 + 1;
        let page_index = ((rank - 1) / per_page) as u16;
        let sequence = (rank - 1) % per_page;
        let owner = &owners[*owner_index];
        let (result, units) = send(
            context,
            &[plane.place(
                owner.key.pubkey(),
                plane.positions[*owner_index],
                page_index,
                sequence,
                *slot,
            )],
            &[&owner.key],
            nonce + 10 + index as u32,
        )
        .await;
        result.unwrap();
        placements.push(units);
        match slot {
            OrderSlot::Portfolio(_) if !seen_portfolio => {
                seen_portfolio = true;
                meter.record(&route(label, "place_order_portfolio"), units);
            }
            OrderSlot::Single(_) if !seen_single => {
                seen_single = true;
                meter.record(&route(label, "place_order_single"), units);
            }
            _ => {}
        }
    }
    if let Some((worst, units)) = placements
        .iter()
        .enumerate()
        .max_by_key(|(_, units)| **units)
    {
        meter.record(
            &route(label, &format!("place_order_worst_rank{}", worst + 1)),
            *units,
        );
    }

    for (at, rank) in cancels.iter().enumerate() {
        let page_index = ((*rank - 1) / per_page) as u16;
        let owner_index = orders[*rank as usize - 1].0;
        let owner = &owners[owner_index];
        let (result, units) = send(
            context,
            &[plane.cancel(
                owner.key.pubkey(),
                owner.id,
                plane.positions[owner_index],
                page_index,
                canonical_order_id(*rank),
                2,
            )],
            &[&owner.key],
            nonce + 200 + at as u32,
        )
        .await;
        result.unwrap();
        if at == 0 {
            meter.record(&route(label, "cancel_order"), units);
        }
    }

    context.warp_to_slot(plane.freeze_deadline).unwrap();
    let (result, units) = send(context, &[plane.freeze()], &[], nonce + 250).await;
    result.unwrap();
    meter.record(
        &route(
            label,
            &format!(
                "freeze_epoch_{}pages_{}orders",
                plane.page_count(),
                orders.len()
            ),
        ),
        units,
    );
    placements
}

/// The frozen epoch, the projected host book, and the live orders' walk-order
/// reservations and page indexes.
pub struct Frozen {
    pub epoch: EpochAccount,
    pub book: BookV1,
    /// One entry per live order, in walk (live-rank) order.
    pub reservations: Vec<Address>,
    /// The page each live order lives on, in the same order.
    pub live_pages: Vec<u16>,
    /// The owner identity of each live order, in the same order.
    pub live_owners: Vec<Hash32>,
    pub page_bytes: Vec<Vec<u8>>,
}

pub async fn frozen_state(context: &mut ProgramTestContext, plane: &Plane) -> Frozen {
    let epoch = EpochAccount::decode(&bytes_of(context, plane.epoch_account).await).unwrap();
    let mut page_bytes = Vec::with_capacity(plane.pages.len());
    for page in &plane.pages {
        page_bytes.push(bytes_of(context, *page).await);
    }
    let mut book = BookV1::empty();
    let mut interner = OwnerInterner::new();
    let mut reservations = Vec::new();
    let mut live_pages = Vec::new();
    let mut live_owners = Vec::new();
    let mut live = 0u16;
    for (page_index, bytes) in page_bytes.iter().enumerate() {
        let header = stream::OrderPageHeader::decode(bytes).unwrap();
        let mut cursor = stream::OrderSlotCursor::new(bytes).unwrap();
        let mut index = 0usize;
        while index < header.order_count as usize {
            let slot = cursor.next_slot().unwrap().unwrap();
            index += 1;
            if let Some(order) = project_slot(&slot, live as u64 + 1, &mut interner).unwrap() {
                book.orders[live as usize] = order;
                reservations.push(plane.reservation(slot.owner(), slot.order_id()));
                live_pages.push(page_index as u16);
                live_owners.push(slot.owner());
                live += 1;
            }
        }
    }
    book.len = live as u8;
    Frozen {
        epoch,
        book,
        reservations,
        live_pages,
        live_owners,
        page_bytes,
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

/// One candidate's host-computed coordinates and its account addresses.
pub struct Submission {
    pub id: Hash32,
    pub prices: [u64; MAX_OUTCOMES],
    pub virtual_split: u64,
    pub virtual_merge: u64,
    pub honored_aon_mask: u64,
    pub fills: Vec<u64>,
    pub witness: PairingWitnessV1,
    pub record: Address,
    pub feed: Address,
    /// The admission claim `SubmitCandidate` carries: weighted direct volume,
    /// limit surplus, distinct owners.  Admission compares *claims*; only
    /// verification replaces them with measured components.  Campaigns that
    /// exercise the retained registry set this; every other campaign leaves
    /// it at the honest zeros.
    pub claims: (i128, u128, u16),
}

/// Compute one candidate's canonical coordinates host-side.
///
/// With `witness = None` the relation's own `canonical_pairing` supplies the
/// decomposition, which is the only tractable way to state a witness for a
/// book of dozens of orders.
pub fn plan_submission(
    plane: &Plane,
    epoch: &EpochAccount,
    book: &BookV1,
    prices: [u64; MAX_OUTCOMES],
    imbalance: i64,
    witness: Option<PairingWitnessV1>,
) -> Submission {
    let domain = zero_sentinel_domain(epoch);
    let candidate = canonical_candidate(&domain, book, &prices, imbalance, 0).unwrap();
    let witness = witness.unwrap_or_else(|| canonical_pairing(&domain, book, &candidate).unwrap());
    let mut shell = CandidateFeedHeader {
        candidate: Hash32::ZERO,
        epoch: plane.epoch_id,
        market: plane.market,
        order_set: epoch.order_set,
        prices,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        honored_aon_mask: candidate.honored_aon_mask,
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        claimed_digest: 0,
        churn: 0,
        declared_slices: 0,
        distinct_owners: 0,
        order_len: book.len,
        outcome_count: plane.outcomes,
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
        honored_aon_mask: candidate.honored_aon_mask,
        fills: candidate.fills[..book.len as usize].to_vec(),
        witness,
        record: plane.candidate_record(id),
        feed: plane.candidate_feed(id),
        claims: (0, 0, 0),
    }
}

/// The two live-order indexes one witness slice pairs.
///
/// Panics on a virtual leg: the entitlement seam refuses those outright (the
/// VirtualPot ranked blocker), so a campaign that reaches one has mis-planned
/// its book rather than found a gap.
pub fn slice_ends(witness: &PairingWitnessV1, index: u16) -> (usize, usize) {
    let slice = witness.slices[usize::from(index)];
    let live = |leg: LegRefV1| match leg {
        LegRefV1::Order(at) => usize::from(at),
        other => panic!("slice {index} names the virtual leg {other:?}"),
    };
    (live(slice.buy_ref), live(slice.sell_ref))
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

/// Drive one submission through the staged wire: create, chunked content,
/// seal against the current registry.  Returns the seal's result and CU.
#[allow(clippy::too_many_arguments)] // one argument per wire coordinate
pub async fn submit_seal(
    context: &mut ProgramTestContext,
    plane: &Plane,
    keeper: &Keypair,
    submission: &Submission,
    declared: Option<u16>,
    retained: &[Hash32],
    displaced_feed: Option<Address>,
    meter: &mut Meter,
    label: &str,
    nonce: u32,
) -> (Result<(), TransactionError>, u64) {
    let payer = keeper.pubkey();
    let (result, units) = send(
        context,
        &[plane.submit(payer, submission, declared)],
        &[keeper],
        nonce,
    )
    .await;
    result.unwrap();
    meter.record(&route(label, "submit_candidate_ledgered"), units);

    let mut written = 0u64;
    for chunk_fills in submission.fills.chunks(FEED_FILLS_PER_CHUNK) {
        let mut fills = [0u64; FEED_FILLS_PER_CHUNK];
        fills[..chunk_fills.len()].copy_from_slice(chunk_fills);
        let (result, units) = send(
            context,
            &[plane.write_chunk(
                submission,
                written,
                CandidateFeedChunk::Fills {
                    count: chunk_fills.len() as u8,
                    fills,
                },
            )],
            &[],
            nonce + 1 + written as u32,
        )
        .await;
        result.unwrap();
        meter.record(
            &route(label, &format!("write_feed_fills_x{}", chunk_fills.len())),
            units,
        );
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
            &[plane.write_chunk(
                submission,
                written,
                CandidateFeedChunk::Slices {
                    count: chunk_slices.len() as u8,
                    slices,
                },
            )],
            &[],
            nonce + 100 + written as u32,
        )
        .await;
        result.unwrap();
        meter.record(
            &route(label, &format!("write_feed_slices_x{}", chunk_slices.len())),
            units,
        );
        written += chunk_slices.len() as u64;
    }

    let (result, units) = send(
        context,
        &[plane.seal(submission, retained, displaced_feed)],
        &[],
        nonce + 300,
    )
    .await;
    if result.is_ok() {
        meter.record(
            &route(label, &format!("seal_candidate_{}retained", retained.len())),
            units,
        );
    }
    (result, units)
}

/// Create the checkpoint at its canonical PDA: init plus the four grows.
pub async fn create_checkpoint(
    context: &mut ProgramTestContext,
    plane: &Plane,
    keeper: &Keypair,
    candidate: Hash32,
    meter: &mut Meter,
    label: &str,
    nonce: u32,
) {
    let payer = keeper.pubkey();
    let (result, units) = send(
        context,
        &[
            plane.init_clear_work(payer, candidate),
            plane.grow_clear_work(candidate, 1),
            plane.grow_clear_work(candidate, 2),
            plane.grow_clear_work(candidate, 3),
            plane.grow_clear_work(candidate, 4),
        ],
        &[keeper],
        nonce,
    )
    .await;
    result.unwrap();
    meter.record(
        &route(label, "init_clear_work_plus_4_grows_ledgered"),
        units,
    );
}

/// Walk one sealed candidate to its verdict: pass 1 page by page (with the
/// reservation sweep), the declared slices, pass 2, and the completing close.
#[allow(clippy::too_many_arguments)] // one argument per walk coordinate
pub async fn walk_to_verdict(
    context: &mut ProgramTestContext,
    plane: &Plane,
    keeper: &Keypair,
    submission: &Submission,
    frozen: &Frozen,
    slice_batch: u16,
    meter: &mut Meter,
    label: &str,
    nonce: u32,
) {
    create_checkpoint(context, plane, keeper, submission.id, meter, label, nonce).await;
    let mut step = nonce + 1;

    for page_index in 0..plane.page_count() {
        let batch: Vec<Address> = frozen
            .reservations
            .iter()
            .zip(frozen.live_pages.iter())
            .filter(|(_, page)| **page == page_index)
            .map(|(reservation, _)| *reservation)
            .collect();
        let (result, units) = send_walk(
            context,
            plane.advance(
                submission.id,
                page_index,
                MAX_ORDERS_PER_PAGE as u16,
                &batch,
            ),
            &[],
            step,
        )
        .await;
        result.unwrap();
        meter.record(
            &route(
                label,
                &format!("advance_pass1_page{page_index}_{}orders", batch.len()),
            ),
            units,
        );
        step += 1;
    }

    let mut done = 0u16;
    while done < submission.witness.len {
        let batch = slice_batch.min(submission.witness.len - done);
        let (result, units) = send_walk(
            context,
            plane.advance_slices(submission.id, batch),
            &[],
            step,
        )
        .await;
        result.unwrap();
        meter.record(&route(label, &format!("advance_slices_x{batch}")), units);
        step += 1;
        done += batch;
    }

    for page_index in 0..plane.page_count() {
        let (result, units) = send_walk(
            context,
            plane.advance(submission.id, page_index, MAX_ORDERS_PER_PAGE as u16, &[]),
            &[],
            step,
        )
        .await;
        result.unwrap();
        meter.record(
            &route(label, &format!("advance_pass2_page{page_index}")),
            units,
        );
        step += 1;
    }

    let (result, units) = send_walk(context, plane.complete(submission.id), &[], step).await;
    result.unwrap();
    meter.record(&route(label, "complete_clear_work"), units);
}

/* ---------------------------------------------------------------------- */
/* Conservation                                                            */
/* ---------------------------------------------------------------------- */

/// Every owner's free cash, summed across the book.
pub async fn owner_cash(context: &mut ProgramTestContext, positions: &[Address]) -> u64 {
    let mut total = 0u64;
    for position in positions {
        total += read_position(context, *position).await.cash_atoms;
    }
    total
}

/// Every owner's encumbered cash, summed across the book.
pub async fn owner_reserved_cash(context: &mut ProgramTestContext, positions: &[Address]) -> u64 {
    let mut total = 0u64;
    for position in positions {
        total += read_position(context, *position).await.reserved_cash_atoms;
    }
    total
}

/// Every Egg on one outcome the book can account for: what the Positions hold
/// plus what the live reservations still own.
///
/// Admission moves a sell's Eggs *out* of its Position and into its
/// reservation, so only this sum is conserved at every transaction boundary.
pub async fn book_eggs(
    context: &mut ProgramTestContext,
    positions: &[Address],
    reservations: &[Address],
    outcome: usize,
) -> u64 {
    let mut total = 0u64;
    for position in positions {
        total += read_position(context, *position).await.internal[outcome];
    }
    for address in reservations {
        total += read_reservation(context, *address).await.remaining_internal[outcome];
    }
    total
}
