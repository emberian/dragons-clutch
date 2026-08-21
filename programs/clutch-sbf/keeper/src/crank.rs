//! The crank ladder: decode the plane, decide the next due permissionless
//! action, and build the transaction that takes it.
//!
//! Everything here is derived from **committed account bytes**.  The keeper
//! holds no plan, no script, and no memory across restarts: the chain is the
//! only state, which is what makes a mid-walk kill and restart a no-op rather
//! than a resume protocol.
//!
//! ## Idempotence
//!
//! Every action is safe to re-attempt because the program refuses the replay,
//! and the keeper reads the refusal code as a completion signal rather than an
//! error.  The per-action benign set is declared next to the action that emits
//! it ([`Act::benign`]) and is never a wildcard: an unexpected code stops the
//! keeper.
//!
//! ## What is and is not permissionless
//!
//! Everything below is permissionless — the fee payer is the only signer —
//! **except** [`Intent::ReleaseTerminalReservation`] (tag 60), which is
//! owner-signed by design: value returns to an owner, so an owner must
//! authorize it.  The keeper takes that route only when an owner key was
//! explicitly supplied, and logs it `authority=owner-signed`.

use crate::pda::{Deriver, Pda};
use crate::quotes::{self, Quote};
use crate::rpc::Rpc;
use crate::wire::{self, Instruction, Message};
use clutch_batch::relation_v1_stream::{ClearWorkV1, FeedStatusV1};
use clutch_solana_layout::clearing::{
    self, CandidateFeedHeader, ClearWorkHeader, EpochWindowAccount, LegRef, PairingSlice,
    CLEAR_WORK_GROW_STEP, CLEAR_WORK_STATUS_COMPLETE,
};
use clutch_solana_layout::reservation::{
    canonical_reservation_id, ReservationAccount, RESERVATION_STATE_ACTIVE,
    RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED, RESERVATION_STATE_RELEASED,
};
use clutch_solana_layout::{
    account_len, canonical_epoch_id, CandidateRecord, EpochAccount, FinalPotAccount, Hash32, Intent,
    MarketAccount, OrderPageAccount, OrderRecord, OrderSlot, SettlementReceiptAccount,
    TermsAccount, CANDIDATE_STATUS_SELECTED, CANDIDATE_STATUS_VERIFIED, EPOCH_PHASE_CLEARED,
    EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED, EPOCH_PHASE_OPEN, MAX_ORDER_PAGES,
    RECEIPT_FLAG_SLICE_EXHAUSTED,
};
use solana_keypair::{Keypair, Signer};

// --- the already-done code table -----------------------------------------
//
// Every constant is `clutch_sbf::error::ClutchError`'s discriminant, named
// here so a log line can print the reason rather than a bare number.

/// `ClutchError::WrongProgramOwner`: the account is gone (System-owned), which
/// is what a second close of the same account finds.
pub const WRONG_PROGRAM_OWNER: u64 = 0x0004;
/// `ClutchError::MismatchedState`: identities, generations, or phases
/// disagreed — the shape a replayed settle or entitle earns once the receipt
/// is exhausted and both reservations are CONSUMED.
pub const MISMATCHED_STATE: u64 = 0x000b;
/// `ClutchError::NotActive`: the lifecycle phase forbids the transition, which
/// is what a second freeze or a second selection earns.
pub const NOT_ACTIVE: u64 = 0x0016;
/// `ClutchError::AlreadyInitialized`: the creation target already has bytes.
pub const ALREADY_INITIALIZED: u64 = 0x0040;

/// Static configuration: what plane this keeper cranks.
#[derive(Clone, Debug)]
pub struct Config {
    /// Program id, base58.
    pub program_id_b58: String,
    /// Program id bytes.
    pub program_id: [u8; 32],
    /// Realm namespace.
    pub realm: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// Epoch index within the market.
    pub epoch_index: u64,
    /// Frozen batch-policy digest this epoch clears under.
    pub policy: Hash32,
    /// Slots after the opening slot at which the keeper's own `InitEpoch`
    /// stamps the freeze deadline; ignored when the epoch already exists.
    pub freeze_deadline_slots: u64,
    /// Whether the keeper may open the epoch and its page itself.
    pub may_open: bool,
}

/// What the ladder decided.
#[derive(Debug)]
pub enum Step {
    /// Nothing is due; the reason is for the log, `wait_until` for the sleep.
    Idle {
        /// Why nothing is due.
        reason: String,
        /// Slot at which something will be.
        wait_until: Option<u64>,
    },
    /// The epoch and its window are gone: the lifecycle is over.
    Done,
    /// The epoch is terminal and every route the keeper may take is taken;
    /// what remains is blocked on something the keeper is not allowed to do.
    Blocked {
        /// What is standing, and why the keeper cannot move it.
        reason: String,
    },
    /// Take this action.
    Act(Box<Act>),
}

/// One built, signed transaction and the facts a log line needs.
#[derive(Debug)]
pub struct Act {
    /// Short action name, e.g. `FreezeEpoch`.
    pub name: String,
    /// Free-form detail: the item the action is about.
    pub detail: String,
    /// The W1 row this action spends against.
    pub quote: Quote,
    /// Refusal codes that mean "already done"; anything else is fatal.
    pub benign: Vec<u64>,
    /// Whether the fee payer is the only signer.
    pub permissionless: bool,
    /// The signed transaction.
    pub transaction: Vec<u8>,
}

/// One transaction's account groups and program instructions.
struct TxSpec {
    writable_signers: Vec<[u8; 32]>,
    readonly_signers: Vec<[u8; 32]>,
    writable: Vec<[u8; 32]>,
    readonly: Vec<[u8; 32]>,
    /// `(account roles in program order, instruction data)`.
    program: Vec<(Vec<[u8; 32]>, Vec<u8>)>,
    heap: bool,
    limit: u32,
}

/// Byte offset of `ReservationAccount::epoch` on the wire: the two header
/// bytes, then `reservation` and `market`.
const RESERVATION_EPOCH_OFFSET: usize = 2 + 32 + 32;

/// One live order of the frozen set, at its walk rank.
#[derive(Clone, Debug)]
struct Live {
    rank: u16,
    page_index: u16,
    record: OrderRecord,
    is_portfolio: bool,
    reservation: Pda,
    state: Option<u8>,
}

/// The decoded plane, refetched on every poll.
struct View {
    slot: u64,
    epoch: Option<EpochAccount>,
    window: Option<EpochWindowAccount>,
    pages: Vec<(u16, Pda, Option<OrderPageAccount>)>,
    live: Vec<Live>,
    pot: Option<FinalPotAccount>,
    pot_present: bool,
}

/// The addresses the plane is anchored on.
struct Wiring {
    market_account: Pda,
    terms_account: Pda,
    grid_account: Pda,
    policy_account: Pda,
    epoch: Pda,
    window: Pda,
}

/// The keeper itself.
pub struct Keeper {
    cfg: Config,
    rpc: Rpc,
    deriver: Deriver,
    keys: Vec<Keypair>,
    payer: [u8; 32],
    epoch_id: Hash32,
    wiring: Option<Wiring>,
    compute_budget: [u8; 32],
    system_program: [u8; 32],
    rent_sysvar: [u8; 32],
    clock_sysvar: [u8; 32],
    sink: [u8; 32],
}

impl Keeper {
    /// Bind a keeper to a plane.
    ///
    /// The first key is the fee payer; any further keys are owner identities
    /// the keeper may use for the one owner-signed route.
    ///
    /// # Errors
    /// Returns an error when no keypair was supplied or a frozen address does
    /// not decode.
    pub fn new(cfg: Config, rpc: Rpc, keys: Vec<Keypair>) -> Result<Self, String> {
        let payer = keys
            .first()
            .ok_or("the keeper needs at least its own fee-payer keypair")?
            .pubkey()
            .to_bytes();
        let deriver = Deriver::new(&cfg.program_id_b58);
        let epoch_id = canonical_epoch_id(cfg.market, cfg.epoch_index);
        Ok(Self {
            compute_budget: wire::base58_decode_32(wire::COMPUTE_BUDGET)?,
            system_program: wire::base58_decode_32(wire::SYSTEM_PROGRAM)?,
            rent_sysvar: wire::base58_decode_32(wire::RENT_SYSVAR)?,
            clock_sysvar: wire::base58_decode_32(wire::CLOCK_SYSVAR)?,
            sink: wire::base58_decode_32(wire::INCINERATOR)?,
            cfg,
            rpc,
            deriver,
            keys,
            payer,
            epoch_id,
            wiring: None,
        })
    }

    /// The epoch identity this keeper cranks.
    #[must_use]
    pub const fn epoch_id(&self) -> Hash32 {
        self.epoch_id
    }

    /// The keeper's fee-payer address, base58.
    #[must_use]
    pub fn payer_address(&self) -> String {
        wire::base58(&self.payer)
    }

    // --- wiring ----------------------------------------------------------

    fn wiring(&mut self) -> Result<&Wiring, String> {
        if self.wiring.is_none() {
            let market_account = self
                .deriver
                .find(&[clutch_sbf::seeds::SEED_MARKET, &self.cfg.realm.bytes(), &self.cfg.market.bytes()])?;
            let market_bytes = self
                .rpc
                .account(&market_account.address)?
                .ok_or_else(|| format!("market account {} is absent", market_account.address))?;
            let market = MarketAccount::decode(&market_bytes)
                .map_err(|error| format!("the market account did not decode: {error:?}"))?;
            let terms_account = self.deriver.find(&[
                clutch_sbf::seeds::SEED_TERMS,
                &market.realm.bytes(),
                &market.terms.bytes(),
            ])?;
            let terms_bytes = self
                .rpc
                .account(&terms_account.address)?
                .ok_or_else(|| format!("terms account {} is absent", terms_account.address))?;
            let terms = TermsAccount::decode(&terms_bytes)
                .map_err(|error| format!("the terms account did not decode: {error:?}"))?;
            let grid_account = self.deriver.find(&[
                clutch_sbf::seeds::SEED_GRID,
                &market.realm.bytes(),
                &terms.price_grid.bytes(),
            ])?;
            let policy_account = self.deriver.find(&[
                clutch_sbf::seeds::SEED_BATCH_POLICY,
                &self.epoch_id.bytes(),
                &self.cfg.policy.bytes(),
            ])?;
            let epoch = self.deriver.epoch(self.cfg.market, self.cfg.epoch_index)?;
            let window = self.deriver.window(self.cfg.market, self.cfg.epoch_index)?;
            self.wiring = Some(Wiring {
                market_account,
                terms_account,
                grid_account,
                policy_account,
                epoch,
                window,
            });
        }
        Ok(self.wiring.as_ref().expect("just populated"))
    }

    // --- view ------------------------------------------------------------

    fn view(&mut self) -> Result<View, String> {
        let slot = self.rpc.slot()?;
        let (epoch_address, window_address) = {
            let wiring = self.wiring()?;
            (wiring.epoch.address.clone(), wiring.window.address.clone())
        };
        let epoch = match self.rpc.account(&epoch_address)? {
            Some(bytes) => Some(
                EpochAccount::decode(&bytes)
                    .map_err(|error| format!("the epoch account did not decode: {error:?}"))?,
            ),
            None => None,
        };
        let window = match self.rpc.account(&window_address)? {
            Some(bytes) => Some(
                EpochWindowAccount::decode(&bytes)
                    .map_err(|error| format!("the epoch window did not decode: {error:?}"))?,
            ),
            None => None,
        };

        // Pages: a frozen epoch states its own count; an open one is probed,
        // because nothing on chain has committed to a page set yet.
        let all_pages =
            u16::try_from(MAX_ORDER_PAGES).expect("the frozen page cap is a small number");
        let probe = epoch.map_or(all_pages, |value| {
            if value.page_count == 0 {
                all_pages
            } else {
                value.page_count
            }
        });
        let mut pages = Vec::new();
        for index in 0..probe {
            let pda = self.deriver.page(self.epoch_id, index)?;
            let Some(bytes) = self.rpc.account(&pda.address)? else {
                pages.push((index, pda, None));
                break;
            };
            let decoded = OrderPageAccount::decode(&bytes)
                .map_err(|error| format!("order page {index} did not decode: {error:?}"))?;
            pages.push((index, pda, Some(decoded)));
        }
        while matches!(pages.last(), Some((_, _, None))) {
            pages.pop();
        }

        let live = self.live_orders(&pages)?;

        let pot_pda = self.deriver.pot(self.epoch_id)?;
        let pot_bytes = self.rpc.account(&pot_pda.address)?;
        let pot_present = pot_bytes.is_some();
        let pot = match pot_bytes {
            Some(bytes) if bytes.len() == account_len::FINAL_POT => Some(
                FinalPotAccount::decode(&bytes)
                    .map_err(|error| format!("the final pot did not decode: {error:?}"))?,
            ),
            _ => None,
        };

        Ok(View {
            slot,
            epoch,
            window,
            pages,
            live,
            pot,
            pot_present,
        })
    }

    /// Walk the page set in canonical order, collecting live records and their
    /// reservations at their walk ranks.
    ///
    /// This is the same vocabulary the relation projection uses: the rank
    /// counts records and never retirements, so a cancellation leaves the
    /// later ranks alone.
    fn live_orders(
        &mut self,
        pages: &[(u16, Pda, Option<OrderPageAccount>)],
    ) -> Result<Vec<Live>, String> {
        let mut out = Vec::new();
        let mut rank = 0_u16;
        for (page_index, _, page) in pages {
            let Some(page) = page else { continue };
            for slot in &page.orders[..usize::from(page.order_count)] {
                let (record, is_portfolio) = match slot {
                    OrderSlot::Single(record) => (*record, false),
                    OrderSlot::Portfolio(record) => (
                        OrderRecord {
                            owner: record.owner,
                            order_id: record.order_id,
                            outcome: 0,
                            side: record.side,
                            quantity: record.lots,
                            limit: record.limit_collateral_per_lot,
                            minimum_fill: 0,
                            flags: 0,
                            generation: record.generation,
                            expiry_epoch: record.expiry_epoch,
                        },
                        true,
                    ),
                    OrderSlot::Empty | OrderSlot::Tombstone(_) => continue,
                };
                let reservation_id = canonical_reservation_id(
                    self.cfg.market,
                    self.epoch_id,
                    record.owner,
                    0,
                    record.order_id,
                );
                let pda = self.deriver.reservation(reservation_id)?;
                let state = match self.rpc.account(&pda.address)? {
                    Some(bytes) if bytes.len() == clutch_solana_layout::reservation::RESERVATION_ACCOUNT_BYTES => {
                        Some(
                            ReservationAccount::decode(&bytes)
                                .map_err(|error| {
                                    format!("reservation {} did not decode: {error:?}", pda.address)
                                })?
                                .state,
                        )
                    }
                    _ => None,
                };
                out.push(Live {
                    rank,
                    page_index: *page_index,
                    record,
                    is_portfolio,
                    reservation: pda,
                    state,
                });
                rank += 1;
            }
        }
        Ok(out)
    }

    /// Every reservation account of this epoch, asked of the bank directly.
    ///
    /// Enumerating from the pages is only possible while the pages exist, and
    /// the close DAG deliberately removes a page *before* its reservation
    /// archives.  Asking the bank keeps the keeper able to finish a walk it
    /// did not start.
    fn reservation_archives(&self) -> Result<Vec<(String, ReservationAccount)>, String> {
        let rows = self.rpc.program_accounts(
            &self.cfg.program_id_b58,
            clutch_solana_layout::reservation::RESERVATION_ACCOUNT_BYTES,
            RESERVATION_EPOCH_OFFSET,
            &self.epoch_id.bytes(),
        )?;
        let mut out = Vec::with_capacity(rows.len());
        for (address, bytes) in rows {
            let decoded = ReservationAccount::decode(&bytes)
                .map_err(|error| format!("reservation {address} did not decode: {error:?}"))?;
            out.push((address, decoded));
        }
        Ok(out)
    }

    // --- the ladder ------------------------------------------------------

    /// Decide and build the next due action.
    ///
    /// # Errors
    /// Returns an error on any RPC or decode failure; a decode failure on a
    /// program-owned account is never swallowed.
    pub fn next(&mut self) -> Result<Step, String> {
        let view = self.view()?;
        let Some(epoch) = view.epoch else {
            return Ok(Step::Done);
        };
        let Some(window) = view.window else {
            return Ok(Step::Done);
        };
        match epoch.phase {
            EPOCH_PHASE_OPEN => self.open_phase(&view, &epoch, &window),
            EPOCH_PHASE_FROZEN => self.frozen_phase(&view, &epoch, &window),
            _ => self.terminal_phase(&view, &epoch, &window),
        }
    }

    /// Whether the epoch and page still need opening, before anything else.
    ///
    /// # Errors
    /// Returns an error on any RPC or decode failure.
    pub fn open_if_absent(&mut self) -> Result<Option<Box<Act>>, String> {
        if !self.cfg.may_open {
            return Ok(None);
        }
        let epoch_address = self.wiring()?.epoch.address.clone();
        if self.rpc.account(&epoch_address)?.is_none() {
            let slot = self.rpc.slot()?;
            return self.init_epoch(slot).map(Some);
        }
        let page = self.deriver.page(self.epoch_id, 0)?;
        if self.rpc.account(&page.address)?.is_none() {
            return self.init_page(&page).map(Some);
        }
        Ok(None)
    }

    fn open_phase(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        window: &EpochWindowAccount,
    ) -> Result<Step, String> {
        if view.slot < window.freeze_deadline_slot {
            return Ok(Step::Idle {
                reason: format!(
                    "epoch OPEN; the freeze deadline is slot {}",
                    window.freeze_deadline_slot
                ),
                wait_until: Some(window.freeze_deadline_slot),
            });
        }
        let _ = epoch;
        self.freeze_epoch(view).map(Step::Act)
    }

    fn frozen_phase(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        window: &EpochWindowAccount,
    ) -> Result<Step, String> {
        // Verification first: a retained candidate with a sealed feed and an
        // unfinished checkpoint is work that must land before the deadline,
        // because selection compares only VERIFIED records.
        for index in 0..usize::from(window.retained_count) {
            let candidate = window.retained[index];
            if let Some(act) = self.verify_candidate(view, epoch, candidate)? {
                return Ok(Step::Act(act));
            }
        }
        if view.slot < window.selection_deadline_slot {
            return Ok(Step::Idle {
                reason: format!(
                    "epoch FROZEN, {} candidate(s) retained; the selection deadline is slot {}",
                    window.retained_count, window.selection_deadline_slot
                ),
                wait_until: Some(window.selection_deadline_slot),
            });
        }
        self.finalize_selection(window).map(Step::Act)
    }

    /// Drive one retained candidate's clearing checkpoint to COMPLETE.
    fn verify_candidate(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        candidate: Hash32,
    ) -> Result<Option<Box<Act>>, String> {
        let record_pda = self.deriver.candidate(self.epoch_id, candidate)?;
        let Some(record_bytes) = self.rpc.account(&record_pda.address)? else {
            return Ok(None);
        };
        let record = CandidateRecord::decode(&record_bytes)
            .map_err(|error| format!("candidate record did not decode: {error:?}"))?;
        if matches!(
            record.status,
            CANDIDATE_STATUS_VERIFIED | CANDIDATE_STATUS_SELECTED
        ) {
            return Ok(None);
        }
        let feed_pda = self.deriver.candidate_feed(self.epoch_id, candidate)?;
        let Some(feed_bytes) = self.rpc.account(&feed_pda.address)? else {
            return Ok(None);
        };
        // A staging feed carries tag 25 and is the submitter's business, not
        // the keeper's: there is nothing to verify until it is sealed.
        let Ok(feed) = CandidateFeedHeader::decode(&feed_bytes) else {
            return Ok(None);
        };

        let work_pda = self.deriver.clear_work(self.epoch_id, candidate)?;
        let work_bytes = self.rpc.account(&work_pda.address)?;
        let Some(work_bytes) = work_bytes else {
            return self.init_clear_work(candidate, &work_pda).map(Some);
        };
        if clearing::clear_work_grow_stage_len(work_bytes.len()).is_ok() {
            return self
                .grow_clear_work(candidate, &work_pda, work_bytes.len())
                .map(Some);
        }
        let header = ClearWorkHeader::decode(&work_bytes)
            .map_err(|error| format!("the clearing checkpoint did not decode: {error:?}"))?;
        if header.status == CLEAR_WORK_STATUS_COMPLETE {
            return Ok(None);
        }
        let mut body = Box::new(ClearWorkV1::new());
        let region = clearing::clear_work_body(&work_bytes)
            .map_err(|error| format!("the checkpoint body did not borrow: {error:?}"))?;
        body.decode_into(region)
            .map_err(|error| format!("the checkpoint body did not decode: {error:?}"))?;
        if body.is_poisoned() {
            return Err(format!(
                "the clearing checkpoint for candidate {} is poisoned; a fresh checkpoint is the \
                 only way forward and the keeper will not silently discard evidence",
                wire::base58(&candidate.bytes())
            ));
        }
        // The idle body has never begun; `status` reports it Complete because
        // there is nothing to feed, so `is_idle` is what distinguishes it.
        let status = if body.is_idle() {
            FeedStatusV1::NeedOrders { pass: 1 }
        } else {
            body.status()
        };
        match status {
            FeedStatusV1::NeedOrders { pass } => {
                let consumed = if body.is_idle() {
                    0
                } else {
                    body.orders_consumed()
                };
                self.advance_clear_work(view, epoch, candidate, &feed_pda, &work_pda, pass, consumed)
                    .map(Some)
            }
            FeedStatusV1::NeedSlices => {
                let declared = feed.declared_slices().unwrap_or(0);
                let consumed = body.slices_consumed();
                self.advance_clear_slices(candidate, &feed_pda, &work_pda, declared, consumed)
                    .map(Some)
            }
            FeedStatusV1::Complete => self
                .complete_clear_work(candidate, &feed_pda, &work_pda, &record_pda)
                .map(Some),
        }
    }

    fn terminal_phase(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        window: &EpochWindowAccount,
    ) -> Result<Step, String> {
        let selected = window.selected_candidate;
        let cleared = epoch.phase == EPOCH_PHASE_CLEARED && selected != Hash32::ZERO;

        // The entitlement phase is open exactly while some reservation still
        // owns its envelope: entitlement is the ACTIVE to ENTITLED move and
        // consumption is the ENTITLED to CONSUMED one, so once no reservation
        // is in either state there is nothing left to freeze a pot for.
        //
        // Without this the pot's own close reopens the question: the DAG
        // removes the pot, the next poll finds it absent, and a keeper that
        // read absence as "the entitlement freeze is due" would recreate it
        // forever.  A page cannot close while any reservation is ACTIVE or
        // ENTITLED, and the pot cannot close before every page, so this
        // condition is false by the time the pot is ever removed.
        let entitlement_open = self.reservation_archives()?.iter().any(|(_, value)| {
            matches!(
                value.state,
                RESERVATION_STATE_ACTIVE | RESERVATION_STATE_ENTITLED
            )
        });

        // --- economic close: entitle, then consume -----------------------
        if cleared && entitlement_open {
            if !view.pot_present {
                return self.freeze_entitlement(selected).map(Step::Act);
            }
            if let Some(act) = self.entitle_or_settle(view, epoch, selected)? {
                return Ok(Step::Act(act));
            }
        }

        // --- owner-signed releases, only with an owner key ---------------
        if let Some(act) = self.release_reservations(view, epoch, window, selected)? {
            return Ok(Step::Act(act));
        }

        // --- rent close, in dependency order -----------------------------
        if let Some(step) = self.close_dag(view, epoch, window, selected, cleared)? {
            return Ok(step);
        }
        Ok(Step::Done)
    }

    // --- individual actions ----------------------------------------------

    fn init_epoch(&mut self, slot: u64) -> Result<Box<Act>, String> {
        let deadline = slot.saturating_add(self.cfg.freeze_deadline_slots);
        let wiring = self.wiring()?;
        let (market_account, terms_account, grid_account, policy_account, epoch, window) = (
            wiring.market_account.bytes,
            wiring.terms_account.bytes,
            wiring.grid_account.bytes,
            wiring.policy_account.bytes,
            wiring.epoch.bytes,
            wiring.window.bytes,
        );
        let ledger = self.deriver.funding_ledger(&epoch)?.bytes;
        let data = wire::request(
            0,
            &Intent::InitEpoch {
                market: self.cfg.market,
                epoch_index: self.cfg.epoch_index,
                policy: self.cfg.policy,
                freeze_deadline_slot: deadline,
            },
        );
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![epoch, window, ledger],
            readonly: vec![
                market_account,
                terms_account,
                grid_account,
                policy_account,
                self.system_program,
                self.rent_sysvar,
                self.clock_sysvar,
            ],
            program: vec![(
                vec![
                    self.payer,
                    market_account,
                    terms_account,
                    grid_account,
                    policy_account,
                    epoch,
                    window,
                    self.system_program,
                    self.rent_sysvar,
                    self.clock_sysvar,
                    ledger,
                ],
                data,
            )],
            heap: false,
            limit: quotes::ledgered(quotes::INIT_EPOCH, 1).limit_cu,
        };
        self.act(
            "InitEpoch",
            format!("epoch_index={} freeze_deadline={deadline}", self.cfg.epoch_index),
            quotes::ledgered(quotes::INIT_EPOCH, 1),
            vec![ALREADY_INITIALIZED],
            true,
            &spec,
        )
    }

    fn init_page(&mut self, page: &Pda) -> Result<Box<Act>, String> {
        let wiring = self.wiring()?;
        let (market_account, epoch) = (wiring.market_account.bytes, wiring.epoch.bytes);
        let ledger = self.deriver.funding_ledger(&page.bytes)?.bytes;
        let data = wire::request(
            0,
            &Intent::InitOrderPage {
                market: self.cfg.market,
                epoch: self.epoch_id,
                page_index: 0,
                page_count: 1,
            },
        );
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![page.bytes, ledger],
            readonly: vec![
                market_account,
                epoch,
                self.system_program,
                self.rent_sysvar,
            ],
            program: vec![(
                vec![
                    self.payer,
                    page.bytes,
                    market_account,
                    epoch,
                    self.system_program,
                    self.rent_sysvar,
                    ledger,
                ],
                data,
            )],
            heap: false,
            limit: quotes::ledgered(quotes::INIT_ORDER_PAGE, 1).limit_cu,
        };
        self.act(
            "InitOrderPage",
            "page_index=0 page_count=1".to_string(),
            quotes::ledgered(quotes::INIT_ORDER_PAGE, 1),
            vec![ALREADY_INITIALIZED],
            true,
            &spec,
        )
    }

    fn freeze_epoch(&mut self, view: &View) -> Result<Box<Act>, String> {
        let wiring = self.wiring()?;
        let (epoch, window) = (wiring.epoch.bytes, wiring.window.bytes);
        let page_keys: Vec<[u8; 32]> = view.pages.iter().map(|(_, pda, _)| pda.bytes).collect();
        if page_keys.is_empty() {
            return Err("the freeze needs at least one order page and none exists".to_string());
        }
        let mut keys = vec![epoch, window, self.clock_sysvar];
        keys.extend_from_slice(&page_keys);
        let mut writable = vec![epoch, window];
        writable.extend_from_slice(&page_keys);
        let quote = quotes::freeze_epoch(
            u16::try_from(page_keys.len()).map_err(|_| "impossible page count".to_string())?,
        );
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable,
            readonly: vec![self.clock_sysvar],
            program: vec![(
                keys,
                wire::request(
                    0,
                    &Intent::FreezeEpoch {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                    },
                ),
            )],
            heap: false,
            limit: quote.limit_cu,
        };
        self.act(
            "FreezeEpoch",
            format!("pages={}", page_keys.len()),
            quote,
            vec![NOT_ACTIVE],
            true,
            &spec,
        )
    }

    fn init_clear_work(&mut self, candidate: Hash32, work: &Pda) -> Result<Box<Act>, String> {
        let ledger = self.deriver.funding_ledger(&work.bytes)?.bytes;
        let mut program = vec![(
            vec![
                self.payer,
                work.bytes,
                self.system_program,
                self.rent_sysvar,
                ledger,
            ],
            wire::request(
                0,
                &Intent::InitClearWork {
                    market: self.cfg.market,
                    epoch: self.epoch_id,
                    candidate,
                },
            ),
        )];
        program.extend(self.grow_instructions(candidate, work, CLEAR_WORK_GROW_STEP));
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![work.bytes, ledger],
            readonly: vec![self.system_program, self.rent_sysvar],
            program,
            heap: false,
            limit: quotes::ledgered(quotes::INIT_CLEAR_WORK, 1).limit_cu,
        };
        self.act(
            "InitClearWork+Grow",
            format!("candidate={}", short(candidate)),
            quotes::ledgered(quotes::INIT_CLEAR_WORK, 1),
            vec![ALREADY_INITIALIZED],
            true,
            &spec,
        )
    }

    fn grow_clear_work(
        &mut self,
        candidate: Hash32,
        work: &Pda,
        current_len: usize,
    ) -> Result<Box<Act>, String> {
        let program = self.grow_instructions(candidate, work, current_len);
        if program.is_empty() {
            return Err("a half-grown checkpoint reported no remaining growth".to_string());
        }
        let steps = program.len();
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![work.bytes],
            readonly: Vec::new(),
            program,
            heap: false,
            limit: quotes::INIT_CLEAR_WORK.limit_cu,
        };
        self.act(
            "GrowClearWork",
            format!(
                "candidate={} from={current_len} steps={steps}",
                short(candidate)
            ),
            quotes::INIT_CLEAR_WORK,
            vec![ALREADY_INITIALIZED, MISMATCHED_STATE],
            true,
            &spec,
        )
    }

    /// Every remaining grow, each at the sequence the program derives from the
    /// account's length at the moment that instruction runs.
    fn grow_instructions(
        &self,
        candidate: Hash32,
        work: &Pda,
        mut len: usize,
    ) -> Vec<(Vec<[u8; 32]>, Vec<u8>)> {
        let mut out = Vec::new();
        while len < account_len::CLEAR_WORK {
            let sequence = (len / CLEAR_WORK_GROW_STEP) as u64;
            out.push((
                vec![work.bytes],
                wire::request(
                    sequence,
                    &Intent::GrowClearWork {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                        candidate,
                    },
                ),
            ));
            len = clearing::clear_work_grown_len(len);
        }
        out
    }

    #[allow(clippy::too_many_arguments)] // one argument per authenticated fact
    fn advance_clear_work(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        candidate: Hash32,
        feed: &Pda,
        work: &Pda,
        pass: u8,
        consumed: u16,
    ) -> Result<Box<Act>, String> {
        let remaining: Vec<&Live> = view
            .live
            .iter()
            .filter(|live| live.rank >= consumed)
            .collect();
        if remaining.is_empty() {
            return Err(format!(
                "pass {pass} has consumed every live order but the checkpoint still asks for orders"
            ));
        }
        // Pass 1 sweeps each pushed order's exact ACTIVE reservation; later
        // passes take none.
        let sweeping = pass == 1;
        let epoch_key = self.wiring()?.epoch.bytes;
        let page_key = view
            .pages
            .iter()
            .find(|(index, _, _)| *index == remaining[0].page_index)
            .map(|(_, pda, _)| pda.bytes)
            .ok_or("the walk cursor sits on a page the keeper could not find")?;
        let quote = quotes::advance_clear_work(pass, epoch.order_count);

        // Shrink the batch until the packet fits: the number of reservations
        // a pass-1 batch may carry is a *wire* bound, not a compute bound.
        let mut batch = remaining.len();
        loop {
            let sweep: Vec<[u8; 32]> = if sweeping {
                remaining[..batch]
                    .iter()
                    .map(|live| live.reservation.bytes)
                    .collect()
            } else {
                Vec::new()
            };
            let max_orders =
                u16::try_from(batch).map_err(|_| "impossible batch size".to_string())?;
            let mut keys = vec![epoch_key, feed.bytes, work.bytes, page_key];
            keys.extend_from_slice(&sweep);
            let mut readonly = vec![epoch_key, feed.bytes, page_key];
            readonly.extend_from_slice(&sweep);
            let spec = TxSpec {
                writable_signers: vec![self.payer],
                readonly_signers: Vec::new(),
                writable: vec![work.bytes],
                readonly,
                program: vec![(
                    keys,
                    wire::request(
                        0,
                        &Intent::AdvanceClearWork {
                            market: self.cfg.market,
                            epoch: self.epoch_id,
                            candidate,
                            max_orders,
                        },
                    ),
                )],
                heap: true,
                limit: quote.limit_cu,
            };
            match self.act(
                "AdvanceClearWork",
                format!(
                    "candidate={} pass={pass} from_rank={consumed} max_orders={max_orders}",
                    short(candidate)
                ),
                quote,
                vec![MISMATCHED_STATE],
                true,
                &spec,
            ) {
                Ok(act) => return Ok(act),
                Err(error) if error.starts_with("packet") && batch > 1 => {
                    batch /= 2;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn advance_clear_slices(
        &mut self,
        candidate: Hash32,
        feed: &Pda,
        work: &Pda,
        declared: u16,
        consumed: u16,
    ) -> Result<Box<Act>, String> {
        let remaining = declared.saturating_sub(consumed);
        if remaining == 0 {
            return Err("the slice pass has no declared slices left but still asks for them"
                .to_string());
        }
        let epoch_key = self.wiring()?.epoch.bytes;
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![work.bytes],
            readonly: vec![epoch_key, feed.bytes],
            program: vec![(
                vec![epoch_key, feed.bytes, work.bytes],
                wire::request(
                    0,
                    &Intent::AdvanceClearSlices {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                        candidate,
                        max_slices: remaining,
                    },
                ),
            )],
            heap: true,
            limit: quotes::ADVANCE_SLICES.limit_cu,
        };
        self.act(
            "AdvanceClearSlices",
            format!(
                "candidate={} from_slice={consumed} max_slices={remaining}",
                short(candidate)
            ),
            quotes::ADVANCE_SLICES,
            vec![MISMATCHED_STATE],
            true,
            &spec,
        )
    }

    fn complete_clear_work(
        &mut self,
        candidate: Hash32,
        feed: &Pda,
        work: &Pda,
        record: &Pda,
    ) -> Result<Box<Act>, String> {
        let epoch_key = self.wiring()?.epoch.bytes;
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![work.bytes, record.bytes],
            readonly: vec![epoch_key, feed.bytes],
            program: vec![(
                vec![epoch_key, feed.bytes, work.bytes, record.bytes],
                wire::request(
                    0,
                    &Intent::CompleteClearWork {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                        candidate,
                    },
                ),
            )],
            heap: true,
            limit: quotes::COMPLETE_CLEAR_WORK.limit_cu,
        };
        self.act(
            "CompleteClearWork",
            format!("candidate={}", short(candidate)),
            quotes::COMPLETE_CLEAR_WORK,
            vec![MISMATCHED_STATE],
            true,
            &spec,
        )
    }

    fn finalize_selection(&mut self, window: &EpochWindowAccount) -> Result<Box<Act>, String> {
        let (epoch_key, window_key) = {
            let wiring = self.wiring()?;
            (wiring.epoch.bytes, wiring.window.bytes)
        };
        let mut keys = vec![epoch_key, window_key, self.clock_sysvar];
        let mut writable = vec![epoch_key, window_key];
        let mut readonly = vec![self.clock_sysvar];
        let mut verified = 0_u8;
        for index in 0..usize::from(window.retained_count) {
            let candidate = window.retained[index];
            let record = self.deriver.candidate(self.epoch_id, candidate)?;
            let feed = self.deriver.candidate_feed(self.epoch_id, candidate)?;
            keys.push(record.bytes);
            keys.push(feed.bytes);
            writable.push(record.bytes);
            readonly.push(feed.bytes);
            if let Some(bytes) = self.rpc.account(&record.address)? {
                if CandidateRecord::decode(&bytes).is_ok_and(|value| {
                    value.status == CANDIDATE_STATUS_VERIFIED
                }) {
                    verified += 1;
                }
            }
        }
        let quote = if verified == 0 {
            quotes::FINALIZE_SELECTION_LAPSE
        } else {
            quotes::FINALIZE_SELECTION
        };
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable,
            readonly,
            program: vec![(
                keys,
                wire::request(
                    0,
                    &Intent::FinalizeSelection {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                    },
                ),
            )],
            heap: false,
            limit: quote.limit_cu,
        };
        self.act(
            "FinalizeSelection",
            format!(
                "retained={} verified={verified}{}",
                window.retained_count,
                if verified == 0 { " (honest lapse)" } else { "" }
            ),
            quote,
            vec![NOT_ACTIVE],
            true,
            &spec,
        )
    }

    fn freeze_entitlement(&mut self, selected: Hash32) -> Result<Box<Act>, String> {
        let epoch_key = self.wiring()?.epoch.bytes;
        let record = self.deriver.candidate(self.epoch_id, selected)?;
        let work = self.deriver.clear_work(self.epoch_id, selected)?;
        let pot = self.deriver.pot(self.epoch_id)?;
        let ledger = self.deriver.funding_ledger(&pot.bytes)?;
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![pot.bytes, ledger.bytes],
            readonly: vec![
                epoch_key,
                record.bytes,
                work.bytes,
                self.system_program,
                self.rent_sysvar,
            ],
            program: vec![(
                vec![
                    self.payer,
                    epoch_key,
                    record.bytes,
                    work.bytes,
                    pot.bytes,
                    self.system_program,
                    self.rent_sysvar,
                    ledger.bytes,
                ],
                wire::request(
                    0,
                    &Intent::FreezeEntitlement {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                        candidate: selected,
                    },
                ),
            )],
            heap: true,
            limit: quotes::ledgered(quotes::FREEZE_ENTITLEMENT, 1).limit_cu,
        };
        self.act(
            "FreezeEntitlement",
            format!("candidate={}", short(selected)),
            quotes::ledgered(quotes::FREEZE_ENTITLEMENT, 1),
            vec![ALREADY_INITIALIZED],
            true,
            &spec,
        )
    }

    /// Entitle the next unfrozen slice group, else consume the next entitled
    /// receipt group.
    fn entitle_or_settle(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        selected: Hash32,
    ) -> Result<Option<Box<Act>>, String> {
        if view.live.is_empty() {
            // The frozen page set is gone, so there is no walk-rank index to
            // resolve a slice's ends against.  A page can only close once
            // every one of its live records' reservations is past ACTIVE,
            // which is exactly the condition under which nothing remains to
            // entitle or to consume.
            return Ok(None);
        }
        let feed_pda = self.deriver.candidate_feed(self.epoch_id, selected)?;
        let Some(feed_bytes) = self.rpc.account(&feed_pda.address)? else {
            // The selected feed is gone, which only happens after its close;
            // there is nothing left to entitle or consume from it.
            return Ok(None);
        };
        let feed = CandidateFeedHeader::decode(&feed_bytes)
            .map_err(|error| format!("the selected feed did not decode: {error:?}"))?;
        let declared = feed.declared_slices().unwrap_or(0);
        let mut slices = Vec::with_capacity(usize::from(declared));
        for index in 0..declared {
            slices.push(
                clearing::slice_at(&feed_bytes, &feed, index)
                    .map_err(|error| format!("slice {index} did not decode: {error:?}"))?,
            );
        }
        let record_pda = self.deriver.candidate(self.epoch_id, selected)?;

        for index in 0..declared {
            let Some(coverage) = coverage_of(&slices, index) else {
                continue;
            };
            if coverage[0] != index {
                continue; // not the entry slice of its group
            }
            let receipt = self.deriver.receipt(self.epoch_id, selected, index)?;
            let receipt_bytes = self.rpc.account(&receipt.address)?;
            match receipt_bytes {
                None => {
                    // A receipt's absence is not evidence that its slice is
                    // due: the close DAG removes exhausted receipts while the
                    // pot is still present, and re-entitling one of those
                    // would mint an entitlement over an already-consumed
                    // envelope.  The reservations are the authority — the
                    // entitlement is exactly the ACTIVE to ENTITLED move.
                    let (buy_end, sell_end) = pair_ends(view, &slices, index)?;
                    if buy_end.state != Some(RESERVATION_STATE_ACTIVE)
                        || sell_end.state != Some(RESERVATION_STATE_ACTIVE)
                    {
                        continue;
                    }
                    return self
                        .entitle_slice(view, epoch, selected, &feed_pda, &record_pda, &coverage,
                                       &slices)
                        .map(Some);
                }
                Some(bytes) => {
                    let decoded = SettlementReceiptAccount::decode(&bytes).map_err(|error| {
                        format!("receipt {index} did not decode: {error:?}")
                    })?;
                    if decoded.consumed_flags & RECEIPT_FLAG_SLICE_EXHAUSTED == 0 {
                        return self
                            .settle_page(view, selected, &record_pda, &coverage, &slices, &decoded)
                            .map(Some);
                    }
                }
            }
        }
        Ok(None)
    }

    #[allow(clippy::too_many_arguments)] // one argument per authenticated fact
    fn entitle_slice(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        selected: Hash32,
        feed: &Pda,
        record: &Pda,
        coverage: &[u16],
        slices: &[PairingSlice],
    ) -> Result<Box<Act>, String> {
        let entry = coverage[0];
        let (buy_end, sell_end) = pair_ends(view, slices, entry)?;
        let portfolio = buy_end.is_portfolio && sell_end.is_portfolio;
        let epoch_key = self.wiring()?.epoch.bytes;
        let terms_key = self.wiring()?.terms_account.bytes;
        let pot = self.deriver.pot(self.epoch_id)?;
        let page_keys: Vec<[u8; 32]> = view
            .pages
            .iter()
            .take(usize::from(epoch.page_count.max(1)))
            .map(|(_, pda, _)| pda.bytes)
            .collect();

        let mut receipts = Vec::new();
        let mut ledgers = Vec::new();
        for index in coverage {
            let receipt = self.deriver.receipt(self.epoch_id, selected, *index)?;
            let ledger = self.deriver.funding_ledger(&receipt.bytes)?;
            receipts.push(receipt.bytes);
            ledgers.push(ledger.bytes);
        }

        let mut keys = vec![
            self.payer,
            epoch_key,
            record.bytes,
            feed.bytes,
            pot.bytes,
            self.system_program,
            self.rent_sysvar,
        ];
        keys.extend_from_slice(&page_keys);
        if portfolio {
            keys.push(terms_key);
        }
        keys.push(buy_end.reservation.bytes);
        keys.push(sell_end.reservation.bytes);
        keys.extend_from_slice(&receipts);
        keys.extend_from_slice(&ledgers);

        let mut writable = vec![buy_end.reservation.bytes, sell_end.reservation.bytes];
        writable.extend_from_slice(&receipts);
        writable.extend_from_slice(&ledgers);
        let mut readonly = vec![
            epoch_key,
            record.bytes,
            feed.bytes,
            pot.bytes,
            self.system_program,
            self.rent_sysvar,
        ];
        readonly.extend_from_slice(&page_keys);
        if portfolio {
            readonly.push(terms_key);
        }

        let base = if portfolio {
            quotes::ENTITLE_SLICE_PAIR
        } else {
            quotes::ENTITLE_SLICE_SINGLE
        };
        let quote = quotes::ledgered(
            base,
            u32::try_from(coverage.len()).map_err(|_| "impossible group size".to_string())?,
        );
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable,
            readonly,
            program: vec![(
                keys,
                wire::request(
                    0,
                    &Intent::EntitleSlice {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                        candidate: selected,
                        slice_index: entry,
                    },
                ),
            )],
            heap: false,
            limit: quote.limit_cu,
        };
        self.act(
            "EntitleSlice",
            format!(
                "slice={entry} group={} shape={}",
                coverage.len(),
                if portfolio { "portfolio-pair" } else { "single" }
            ),
            quote,
            vec![ALREADY_INITIALIZED, MISMATCHED_STATE],
            true,
            &spec,
        )
    }

    fn settle_page(
        &mut self,
        view: &View,
        selected: Hash32,
        record: &Pda,
        coverage: &[u16],
        slices: &[PairingSlice],
        receipt: &SettlementReceiptAccount,
    ) -> Result<Box<Act>, String> {
        let entry = coverage[0];
        let (buy_end, sell_end) = pair_ends(view, slices, entry)?;
        let portfolio = buy_end.is_portfolio && sell_end.is_portfolio;
        let epoch_key = self.wiring()?.epoch.bytes;
        let terms_key = self.wiring()?.terms_account.bytes;
        let buy_position = self.deriver.position(self.cfg.market, buy_end.record.owner)?;
        let sell_position = self.deriver.position(self.cfg.market, sell_end.record.owner)?;
        let mut receipts = Vec::new();
        for index in coverage {
            receipts.push(self.deriver.receipt(self.epoch_id, selected, *index)?.bytes);
        }

        let mut keys = vec![epoch_key, record.bytes];
        let mut readonly = vec![epoch_key, record.bytes];
        if portfolio {
            keys.push(terms_key);
            readonly.push(terms_key);
        }
        keys.push(buy_position.bytes);
        keys.push(sell_position.bytes);
        keys.push(buy_end.reservation.bytes);
        keys.push(sell_end.reservation.bytes);
        if portfolio {
            let page = view
                .pages
                .iter()
                .find(|(index, _, _)| *index == buy_end.page_index)
                .map(|(_, pda, _)| pda.bytes)
                .ok_or("the pair's page is not in view")?;
            keys.push(page);
            readonly.push(page);
            if sell_end.page_index != buy_end.page_index {
                let other = view
                    .pages
                    .iter()
                    .find(|(index, _, _)| *index == sell_end.page_index)
                    .map(|(_, pda, _)| pda.bytes)
                    .ok_or("the pair's second page is not in view")?;
                keys.push(other);
                readonly.push(other);
            }
        }
        keys.extend_from_slice(&receipts);

        let mut writable = vec![
            buy_position.bytes,
            sell_position.bytes,
            buy_end.reservation.bytes,
            sell_end.reservation.bytes,
        ];
        writable.extend_from_slice(&receipts);

        let quote = if portfolio {
            quotes::SETTLE_PAIR
        } else {
            quotes::SETTLE_SINGLE
        };
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable,
            readonly,
            program: vec![(
                keys,
                wire::request(
                    receipt.sequence,
                    &Intent::SettlePage {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                        page_index: buy_end.page_index,
                    },
                ),
            )],
            heap: false,
            limit: quote.limit_cu,
        };
        self.act(
            "SettlePage",
            format!(
                "slice={entry} group={} shape={} sequence={}",
                coverage.len(),
                if portfolio { "portfolio-pair" } else { "single" },
                receipt.sequence
            ),
            quote,
            vec![MISMATCHED_STATE],
            true,
            &spec,
        )
    }

    /// Owner-signed release of a standing ACTIVE reservation.
    ///
    /// This is the one route in the family that is not permissionless, and the
    /// keeper takes it only for owners whose key it was explicitly given.  An
    /// abandoned reservation with no supplied key is left standing, which is
    /// exactly the ratified `GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`
    /// residual rather than a keeper failure.
    fn release_reservations(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        window: &EpochWindowAccount,
        selected: Hash32,
    ) -> Result<Option<Box<Act>>, String> {
        let lapsed = epoch.phase == EPOCH_PHASE_LAPSED;
        for live in &view.live {
            if live.state != Some(RESERVATION_STATE_ACTIVE) {
                continue;
            }
            let Some(owner_index) = self.owner_key_index(live.record.owner) else {
                continue;
            };
            let epoch_key = self.wiring()?.epoch.bytes;
            let position = self.deriver.position(self.cfg.market, live.record.owner)?;
            let owner = self.keys[owner_index].pubkey().to_bytes();
            let mut keys = vec![owner, epoch_key, live.reservation.bytes, position.bytes];
            let mut readonly = vec![epoch_key];
            if !lapsed {
                let record = self.deriver.candidate(self.epoch_id, selected)?;
                let feed = self.deriver.candidate_feed(self.epoch_id, selected)?;
                let page = view
                    .pages
                    .iter()
                    .find(|(index, _, _)| *index == live.page_index)
                    .map(|(_, pda, _)| pda.bytes)
                    .ok_or("the reservation's page is not in view")?;
                keys.push(record.bytes);
                keys.push(feed.bytes);
                keys.push(page);
                readonly.push(record.bytes);
                readonly.push(feed.bytes);
                readonly.push(page);
            }
            let spec = TxSpec {
                writable_signers: vec![self.payer],
                readonly_signers: vec![owner],
                writable: vec![live.reservation.bytes, position.bytes],
                readonly,
                program: vec![(
                    keys,
                    wire::request(
                        live.record.generation.saturating_add(1),
                        &Intent::ReleaseTerminalReservation {
                            market: self.cfg.market,
                            epoch: self.epoch_id,
                        },
                    ),
                )],
                heap: false,
                limit: quotes::RELEASE_RESERVATION.limit_cu,
            };
            let _ = window;
            return self
                .act(
                    "ReleaseTerminalReservation",
                    format!(
                        "rank={} owner={} phase={}",
                        live.rank,
                        short(live.record.owner),
                        if lapsed { "LAPSED" } else { "CLEARED" }
                    ),
                    quotes::RELEASE_RESERVATION,
                    vec![MISMATCHED_STATE, WRONG_PROGRAM_OWNER],
                    false,
                    &spec,
                )
                .map(Some);
        }
        Ok(None)
    }

    /// The rent close, in the DAG's admitted order.
    fn close_dag(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        window: &EpochWindowAccount,
        selected: Hash32,
        cleared: bool,
    ) -> Result<Option<Step>, String> {
        let epoch_key = self.wiring()?.epoch.bytes;
        let window_key = self.wiring()?.window.bytes;

        // 61: exhausted receipts.
        if cleared {
            if let Some(act) = self.close_receipts(selected, epoch_key)? {
                return Ok(Some(Step::Act(act)));
            }
        }

        // 63: pages, once no live record's reservation is ACTIVE or ENTITLED.
        let archives = self.reservation_archives()?;
        let blocking: Vec<&(String, ReservationAccount)> = archives
            .iter()
            .filter(|(_, value)| {
                matches!(
                    value.state,
                    RESERVATION_STATE_ACTIVE | RESERVATION_STATE_ENTITLED
                )
            })
            .collect();
        if !blocking.is_empty() {
            return Ok(Some(Step::Blocked {
                reason: format!(
                    "{} reservation(s) still ACTIVE or ENTITLED ({}); release is owner-signed \
                     by design (tag 60) and no owner key was supplied for them — this is the \
                     recorded GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT residual, not a keeper \
                     failure",
                    blocking.len(),
                    blocking
                        .iter()
                        .map(|(address, _)| short_text(address))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            }));
        }
        if let Some(step) = self.close_pages(view, epoch_key)? {
            return Ok(Some(step));
        }
        if let Some(step) = self.close_reservation_archives(&archives, epoch_key)? {
            return Ok(Some(step));
        }
        if let Some(step) = self.close_pot(view, epoch, epoch_key)? {
            return Ok(Some(step));
        }

        // 66 then 65: checkpoints, then candidate pairs, non-selected first.
        let mut order: Vec<Hash32> = Vec::new();
        for index in 0..usize::from(window.retained_count) {
            let candidate = window.retained[index];
            if candidate != selected {
                order.push(candidate);
            }
        }
        if selected != Hash32::ZERO {
            order.push(selected);
        }
        for candidate in order {
            if let Some(act) = self.close_clear_work(candidate, epoch_key, selected, epoch)? {
                return Ok(Some(Step::Act(act)));
            }
            if let Some(act) = self.close_candidate(candidate, epoch_key, selected, epoch)? {
                return Ok(Some(Step::Act(act)));
            }
        }
        self.close_root(epoch, window, epoch_key, window_key)
    }

    /// Tag 63: one frozen page whose live records are all past ACTIVE.
    fn close_pages(&mut self, view: &View, epoch_key: [u8; 32]) -> Result<Option<Step>, String> {
        for (index, pda, page) in &view.pages {
            if page.is_none() {
                continue;
            }
            let ledger = self.deriver.funding_ledger(&pda.bytes)?;
            let live_keys: Vec<[u8; 32]> = view
                .live
                .iter()
                .filter(|live| live.page_index == *index)
                .map(|live| live.reservation.bytes)
                .collect();
            let mut keys = vec![
                epoch_key,
                pda.bytes,
                ledger.bytes,
                self.payer,
                self.sink,
            ];
            keys.extend_from_slice(&live_keys);
            let mut readonly = vec![epoch_key];
            readonly.extend_from_slice(&live_keys);
            let spec = TxSpec {
                writable_signers: vec![self.payer],
                readonly_signers: Vec::new(),
                writable: vec![pda.bytes, ledger.bytes, self.sink],
                readonly,
                program: vec![(
                    keys,
                    wire::request(
                        0,
                        &Intent::CloseGeneralPage {
                            market: self.cfg.market,
                            epoch: self.epoch_id,
                            page_index: *index,
                        },
                    ),
                )],
                heap: false,
                limit: quotes::CLOSE_PAGE.limit_cu,
            };
            return self
                .act(
                    "CloseGeneralPage",
                    format!("page_index={index} live_reservations={}", live_keys.len()),
                    quotes::CLOSE_PAGE,
                    vec![WRONG_PROGRAM_OWNER],
                    true,
                    &spec,
                )
                .map(|act| Some(Step::Act(act)));
        }
        Ok(None)
    }

    /// Tag 62: one reservation archive whose page is already absent.
    fn close_reservation_archives(
        &mut self,
        archives: &[(String, ReservationAccount)],
        epoch_key: [u8; 32],
    ) -> Result<Option<Step>, String> {
        // 62: reservation archives, once their page is absent.
        for (address, value) in archives {
            if !matches!(
                value.state,
                RESERVATION_STATE_RELEASED | RESERVATION_STATE_CONSUMED
            ) {
                continue;
            }
            let reservation = wire::base58_decode_32(address)?;
            let page = self.deriver.page(self.epoch_id, value.page_index)?;
            // The reservation's page must already be gone: that absence is
            // what makes the page's sweep proof final.
            if self.rpc.account(&page.address)?.is_some() {
                continue;
            }
            // The recipient is the reservation's stored `owner` **wallet**,
            // not that owner's Position PDA: the placement actor funded the
            // reservation's rent in the same transition that recorded it, so
            // the close pays the exact wallet the program re-derives from the
            // reservation's own bytes.
            let recipient = value.owner.bytes();
            let spec = TxSpec {
                writable_signers: vec![self.payer],
                readonly_signers: Vec::new(),
                writable: vec![reservation, recipient, self.sink],
                readonly: vec![epoch_key, page.bytes, self.rent_sysvar],
                program: vec![(
                    vec![
                        epoch_key,
                        reservation,
                        page.bytes,
                        recipient,
                        self.sink,
                        self.rent_sysvar,
                    ],
                    wire::request(
                        0,
                        &Intent::CloseGeneralReservation {
                            market: self.cfg.market,
                            epoch: self.epoch_id,
                        },
                    ),
                )],
                heap: false,
                limit: quotes::CLOSE_RESERVATION.limit_cu,
            };
            return self
                .act(
                    "CloseGeneralReservation",
                    format!(
                        "reservation={} owner={} state={}",
                        short_text(address),
                        short(value.owner),
                        value.state
                    ),
                    quotes::CLOSE_RESERVATION,
                    vec![WRONG_PROGRAM_OWNER],
                    true,
                    &spec,
                )
                .map(|act| Some(Step::Act(act)));
        }
        Ok(None)
    }

    /// Tag 64: the provably empty pot, once every page is absent.
    fn close_pot(
        &mut self,
        view: &View,
        epoch: &EpochAccount,
        epoch_key: [u8; 32],
    ) -> Result<Option<Step>, String> {
        // 64: the pot, once every page is absent.
        if view.pot_present {
            let pot = self.deriver.pot(self.epoch_id)?;
            let ledger = self.deriver.funding_ledger(&pot.bytes)?;
            let mut keys = vec![epoch_key, pot.bytes, ledger.bytes, self.payer, self.sink];
            let mut readonly = vec![epoch_key];
            for index in 0..epoch.page_count.max(1) {
                let page = self.deriver.page(self.epoch_id, index)?;
                keys.push(page.bytes);
                readonly.push(page.bytes);
            }
            let spec = TxSpec {
                writable_signers: vec![self.payer],
                readonly_signers: Vec::new(),
                writable: vec![pot.bytes, ledger.bytes, self.sink],
                readonly,
                program: vec![(
                    keys,
                    wire::request(
                        0,
                        &Intent::CloseGeneralPot {
                            market: self.cfg.market,
                            epoch: self.epoch_id,
                        },
                    ),
                )],
                heap: false,
                limit: quotes::CLOSE_POT.limit_cu,
            };
            let _ = &view.pot;
            return self
                .act(
                    "CloseGeneralPot",
                    "pot".to_string(),
                    quotes::CLOSE_POT,
                    vec![WRONG_PROGRAM_OWNER],
                    true,
                    &spec,
                )
                .map(|act| Some(Step::Act(act)));
        }
        Ok(None)
    }

    /// Tag 67: the root of the DAG.
    fn close_root(
        &mut self,
        epoch: &EpochAccount,
        window: &EpochWindowAccount,
        epoch_key: [u8; 32],
        window_key: [u8; 32],
    ) -> Result<Option<Step>, String> {
        // 67: the root.
        let ledger = self.deriver.funding_ledger(&epoch_key)?;
        let pot = self.deriver.pot(self.epoch_id)?;
        let mut keys = vec![
            epoch_key,
            window_key,
            ledger.bytes,
            self.payer,
            self.sink,
            pot.bytes,
        ];
        let mut readonly = vec![pot.bytes];
        for index in 0..epoch.page_count.max(1) {
            let page = self.deriver.page(self.epoch_id, index)?;
            keys.push(page.bytes);
            readonly.push(page.bytes);
        }
        for index in 0..usize::from(window.retained_count) {
            let candidate = window.retained[index];
            let record = self.deriver.candidate(self.epoch_id, candidate)?;
            let record_ledger = self.deriver.funding_ledger(&record.bytes)?;
            keys.push(record.bytes);
            keys.push(record_ledger.bytes);
            readonly.push(record.bytes);
            readonly.push(record_ledger.bytes);
        }
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![epoch_key, window_key, ledger.bytes, self.sink],
            readonly,
            program: vec![(
                keys,
                wire::request(
                    0,
                    &Intent::CloseGeneralEpoch {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                    },
                ),
            )],
            heap: false,
            limit: quotes::CLOSE_EPOCH.limit_cu,
        };
        self.act(
            "CloseGeneralEpoch",
            format!("retained={}", window.retained_count),
            quotes::CLOSE_EPOCH,
            vec![WRONG_PROGRAM_OWNER],
            true,
            &spec,
        )
        .map(|act| Some(Step::Act(act)))
    }

    fn close_receipts(
        &mut self,
        selected: Hash32,
        epoch_key: [u8; 32],
    ) -> Result<Option<Box<Act>>, String> {
        let feed_pda = self.deriver.candidate_feed(self.epoch_id, selected)?;
        let Some(feed_bytes) = self.rpc.account(&feed_pda.address)? else {
            return Ok(None);
        };
        let Ok(feed) = CandidateFeedHeader::decode(&feed_bytes) else {
            return Ok(None);
        };
        let declared = feed.declared_slices().unwrap_or(0);
        for index in 0..declared {
            let receipt = self.deriver.receipt(self.epoch_id, selected, index)?;
            let Some(bytes) = self.rpc.account(&receipt.address)? else {
                continue;
            };
            let decoded = SettlementReceiptAccount::decode(&bytes)
                .map_err(|error| format!("receipt {index} did not decode: {error:?}"))?;
            if decoded.consumed_flags & RECEIPT_FLAG_SLICE_EXHAUSTED == 0 {
                continue;
            }
            let ledger = self.deriver.funding_ledger(&receipt.bytes)?;
            let spec = TxSpec {
                writable_signers: vec![self.payer],
                readonly_signers: Vec::new(),
                writable: vec![receipt.bytes, ledger.bytes, self.sink],
                readonly: vec![epoch_key],
                program: vec![(
                    vec![
                        epoch_key,
                        receipt.bytes,
                        ledger.bytes,
                        self.payer,
                        self.sink,
                    ],
                    wire::request(
                        0,
                        &Intent::CloseGeneralReceipt {
                            market: self.cfg.market,
                            epoch: self.epoch_id,
                            candidate: selected,
                            slice_index: index,
                        },
                    ),
                )],
                heap: false,
                limit: quotes::CLOSE_RECEIPT.limit_cu,
            };
            return self
                .act(
                    "CloseGeneralReceipt",
                    format!("slice={index}"),
                    quotes::CLOSE_RECEIPT,
                    vec![WRONG_PROGRAM_OWNER],
                    true,
                    &spec,
                )
                .map(Some);
        }
        Ok(None)
    }

    fn close_clear_work(
        &mut self,
        candidate: Hash32,
        epoch_key: [u8; 32],
        selected: Hash32,
        epoch: &EpochAccount,
    ) -> Result<Option<Box<Act>>, String> {
        let work = self.deriver.clear_work(self.epoch_id, candidate)?;
        if self.rpc.account(&work.address)?.is_none() {
            return Ok(None);
        }
        let record = self.deriver.candidate(self.epoch_id, candidate)?;
        let ledger = self.deriver.funding_ledger(&work.bytes)?;
        let pot = self.deriver.pot(self.epoch_id)?;
        let record_standing = self.rpc.account(&record.address)?.is_some();
        let pot_present = self.rpc.account(&pot.address)?.is_some();
        // A standing SELECTED record demands one of two proofs that no
        // entitlement freeze can still want this checkpoint's verdict: the
        // live pot (the freeze already ran), or one absence slot per page
        // (no freeze is reachable at all).  An empty tail is neither, and the
        // program counts the absence slots, so it earns `AccountCount`.
        let selected_standing = record_standing && candidate == selected;
        let pot_proof = selected_standing && pot_present;
        let mut keys = vec![
            epoch_key,
            work.bytes,
            record.bytes,
            ledger.bytes,
            self.payer,
            self.sink,
        ];
        let mut readonly = vec![epoch_key, record.bytes];
        if pot_proof {
            keys.push(pot.bytes);
            readonly.push(pot.bytes);
        } else if selected_standing {
            for index in 0..epoch.page_count.max(1) {
                let page = self.deriver.page(self.epoch_id, index)?;
                keys.push(page.bytes);
                readonly.push(page.bytes);
            }
        }
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![work.bytes, ledger.bytes, self.sink],
            readonly,
            program: vec![(
                keys,
                wire::request(
                    0,
                    &Intent::CloseGeneralClearWork {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                        candidate,
                    },
                ),
            )],
            heap: false,
            limit: quotes::CLOSE_CLEAR_WORK.limit_cu,
        };
        self.act(
            "CloseGeneralClearWork",
            format!(
                "candidate={} pot_proof={pot_proof} page_absence={}",
                short(candidate),
                usize::from(!pot_proof && selected_standing) * usize::from(epoch.page_count.max(1))
            ),
            quotes::CLOSE_CLEAR_WORK,
            vec![WRONG_PROGRAM_OWNER],
            true,
            &spec,
        )
        .map(Some)
    }

    fn close_candidate(
        &mut self,
        candidate: Hash32,
        epoch_key: [u8; 32],
        selected: Hash32,
        epoch: &EpochAccount,
    ) -> Result<Option<Box<Act>>, String> {
        let record = self.deriver.candidate(self.epoch_id, candidate)?;
        if self.rpc.account(&record.address)?.is_none() {
            return Ok(None);
        }
        let feed = self.deriver.candidate_feed(self.epoch_id, candidate)?;
        let ledger = self.deriver.funding_ledger(&record.bytes)?;
        // A pair created without its optional funding ledger records no payer,
        // so its close refuses forever and no payer is ever guessed.  That is
        // the ratified `RENT.ACCOUNT_REFUND_UNOWNED` tolerance: the root close
        // proceeds past it and the pair stands as recorded residual.  Spinning
        // on it would turn a documented residual into a keeper hang.
        if self.rpc.account(&ledger.address)?.is_none() {
            return Ok(None);
        }
        let pot = self.deriver.pot(self.epoch_id)?;
        let is_selected = candidate == selected;
        let mut keys = vec![
            epoch_key,
            record.bytes,
            feed.bytes,
            ledger.bytes,
            self.payer,
            self.sink,
        ];
        let mut readonly = vec![epoch_key];
        if is_selected {
            let page = self.deriver.page(self.epoch_id, 0)?;
            keys.push(pot.bytes);
            keys.push(page.bytes);
            readonly.push(pot.bytes);
            readonly.push(page.bytes);
        }
        let _ = epoch;
        let spec = TxSpec {
            writable_signers: vec![self.payer],
            readonly_signers: Vec::new(),
            writable: vec![record.bytes, feed.bytes, ledger.bytes, self.sink],
            readonly,
            program: vec![(
                keys,
                wire::request(
                    0,
                    &Intent::CloseGeneralCandidate {
                        market: self.cfg.market,
                        epoch: self.epoch_id,
                        candidate,
                    },
                ),
            )],
            heap: false,
            limit: quotes::CLOSE_CANDIDATE.limit_cu,
        };
        self.act(
            "CloseGeneralCandidate",
            format!("candidate={} selected={is_selected}", short(candidate)),
            quotes::CLOSE_CANDIDATE,
            vec![WRONG_PROGRAM_OWNER],
            true,
            &spec,
        )
        .map(Some)
    }

    // --- helpers ----------------------------------------------------------

    fn owner_key_index(&self, owner: Hash32) -> Option<usize> {
        self.keys
            .iter()
            .position(|key| key.pubkey().to_bytes() == owner.bytes())
    }

    /// Serialize, size-check, and sign one action.
    fn act(
        &self,
        name: &str,
        detail: String,
        quote: Quote,
        benign: Vec<u64>,
        permissionless: bool,
        spec: &TxSpec,
    ) -> Result<Box<Act>, String> {
        let blockhash = self.rpc.blockhash()?;
        let mut readonly = spec.readonly.clone();
        readonly.push(self.cfg.program_id);
        readonly.push(self.compute_budget);
        // The four groups must be disjoint: a key that is writable anywhere
        // must not also appear read-only.
        readonly.retain(|key| !spec.writable.contains(key) && !spec.writable_signers.contains(key));
        readonly.dedup();
        let mut seen = Vec::new();
        readonly.retain(|key| {
            if seen.contains(key) {
                false
            } else {
                seen.push(*key);
                true
            }
        });
        let mut writable = Vec::new();
        for key in &spec.writable {
            if !writable.contains(key) && !spec.writable_signers.contains(key) {
                writable.push(*key);
            }
        }
        let message = Message::new(
            &spec.writable_signers,
            &spec.readonly_signers,
            &writable,
            &readonly,
        );
        let mut instructions = Vec::new();
        // The heap rider is not free: the profile measured its cost as a flat
        // surcharge on top of the walk rows, so it is added on top of the
        // quoted limit rather than eaten out of the row's headroom.
        let limit = if spec.heap {
            instructions.push(wire::heap_frame_instruction(
                &message,
                &self.compute_budget,
                wire::HEAP_FRAME_BYTES,
            ));
            spec.limit
                .saturating_add(quotes::HEAP_FRAME_SURCHARGE_CU)
                .min(quotes::TRANSACTION_CEILING_CU)
        } else {
            spec.limit
        };
        instructions.push(wire::compute_limit_instruction(
            &message,
            &self.compute_budget,
            limit,
        ));
        for (accounts, data) in &spec.program {
            instructions.push(Instruction {
                program_index: message.index(&self.cfg.program_id),
                accounts: message.indices(accounts),
                data: data.clone(),
            });
        }
        let mut transaction = wire::serialize(&message, &instructions, &blockhash);
        if transaction.len() > wire::PACKET_BUDGET_BYTES {
            return Err(format!(
                "packet budget exceeded for {name}: {} bytes over the {} the cluster wire admits",
                transaction.len(),
                wire::PACKET_BUDGET_BYTES
            ));
        }
        let refs: Vec<&Keypair> = self.keys.iter().collect();
        wire::sign(&mut transaction, &refs)?;
        Ok(Box::new(Act {
            name: name.to_string(),
            detail,
            quote,
            benign,
            permissionless,
            transaction,
        }))
    }
}

/// The two live orders one witness slice pairs.
fn pair_ends(view: &View, slices: &[PairingSlice], index: u16) -> Result<(Live, Live), String> {
    let slice = slices
        .get(usize::from(index))
        .ok_or("slice index past the declared witness")?;
    let (LegRef::Order(buy_rank), LegRef::Order(sell_rank)) = (slice.buy_ref, slice.sell_ref) else {
        return Err(format!(
            "slice {index} carries a virtual leg; the `VirtualPot` blocker stands and the keeper \
             will not fabricate a shape for it"
        ));
    };
    let find = |rank: u8| -> Result<Live, String> {
        view.live
            .iter()
            .find(|live| live.rank == u16::from(rank))
            .cloned()
            .ok_or_else(|| format!("no live order at walk rank {rank}"))
    };
    Ok((find(buy_rank)?, find(sell_rank)?))
}

/// Every slice index referencing exactly the same `(buy, sell)` order pair as
/// `index`, ascending.
///
/// This mirrors the program's `scan_witness`: the entry slice is the group's
/// first index, and a group is entitled and consumed atomically.  `None` means
/// the slice carries a virtual leg, which this keeper does not drive.
fn coverage_of(slices: &[PairingSlice], index: u16) -> Option<Vec<u16>> {
    let slice = slices.get(usize::from(index))?;
    let (LegRef::Order(buy), LegRef::Order(sell)) = (slice.buy_ref, slice.sell_ref) else {
        return None;
    };
    let mut out = Vec::new();
    for (at, candidate) in slices.iter().enumerate() {
        if candidate.buy_ref == LegRef::Order(buy) && candidate.sell_ref == LegRef::Order(sell) {
            out.push(u16::try_from(at).ok()?);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// The first eight base58 characters of an identity, for a log line.
fn short(value: Hash32) -> String {
    short_text(&wire::base58(&value.bytes()))
}

/// The first eight characters of an already-base58 address.
fn short_text(text: &str) -> String {
    text.chars().take(8).collect()
}

/// Whether a refusal code means the action was already done.
#[must_use]
pub fn benign_reason(code: u64) -> &'static str {
    match code {
        WRONG_PROGRAM_OWNER => "account already closed (System-owned)",
        MISMATCHED_STATE => "state already past this transition",
        NOT_ACTIVE => "lifecycle phase already past this transition",
        ALREADY_INITIALIZED => "creation target already exists",
        _ => "unclassified",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(buy: u8, sell: u8) -> PairingSlice {
        PairingSlice {
            buy_ref: LegRef::Order(buy),
            sell_ref: LegRef::Order(sell),
            outcome: 0,
            quantity: 1,
        }
    }

    #[test]
    fn a_single_crossing_is_its_own_group() {
        let slices = [order(1, 2), order(3, 4)];
        assert_eq!(coverage_of(&slices, 0), Some(vec![0]));
        assert_eq!(coverage_of(&slices, 1), Some(vec![1]));
    }

    #[test]
    fn every_slice_of_one_pair_collects_into_one_group_with_the_first_as_entry() {
        // The portfolio shape: one (buy, sell) pair across several outcomes.
        let slices = [order(0, 1), order(3, 4), order(0, 1), order(0, 1)];
        let group = coverage_of(&slices, 2).expect("the group exists");
        assert_eq!(group, vec![0, 2, 3]);
        assert_eq!(group[0], 0, "the entry slice is the group's first index");
        assert_eq!(coverage_of(&slices, 1), Some(vec![1]));
    }

    #[test]
    fn a_virtual_leg_is_refused_rather_than_guessed() {
        let slices = [PairingSlice {
            buy_ref: LegRef::Merge,
            sell_ref: LegRef::Order(1),
            outcome: 0,
            quantity: 1,
        }];
        assert_eq!(coverage_of(&slices, 0), None);
    }

    #[test]
    fn every_benign_code_has_a_reason() {
        for code in [
            WRONG_PROGRAM_OWNER,
            MISMATCHED_STATE,
            NOT_ACTIVE,
            ALREADY_INITIALIZED,
        ] {
            assert_ne!(benign_reason(code), "unclassified");
        }
        assert_eq!(benign_reason(0x0002), "unclassified");
    }
}
