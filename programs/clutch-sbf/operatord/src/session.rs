//! Trade mode: one person, one automaton, one epoch, one bank.
//!
//! The watch-mode walk replays a pregenerated plan and compares every reload
//! against an expectation the harness computed offline.  Nothing here is
//! pregenerated: the book is whatever the person and the automaton actually
//! placed, so there is no offline expectation to compare against and the
//! evidence is exactly what it says it is — *the bank accepted this, and here
//! are the account bytes it wrote*.  Every number the bench shows after a
//! submission is decoded from a reload; every number it shows before one is
//! labelled MODEL-ONLY.
//!
//! The claim plane is unchanged from the general-clearing lane and widened in
//! no direction: signed, confirmed, committed sequential execution on a local
//! `solana-test-validator`, from a genesis-assisted prestate, against an ELF
//! built with `--features non-production-mock-source`.

use crate::bot::{Bot, Quote};
use crate::builders;
use crate::bus::Bus;
use crate::decode;
use crate::friday::{Friday, EPOCH_INDEX, LADDER_STEP, OUTCOMES, PRICE_SCALE};
use crate::quantize::{belief_on_ladder, resolution_weights, PAYOUT_DENOMINATOR};
use crate::rpc;
use clutch_batch::relation_v1::{
    canonical_candidate, canonical_pairing, BookV1, LegRefV1, PairingWitnessV1, RelationDomainV1,
};
use clutch_batch_policy_identity::general_clearing_v1::GENERAL_CLEARING_POLICY_V1;
use clutch_solana_layout::clearing::{
    CandidateFeedHeader, LegRef, PairingSlice, CANDIDATE_WINDOW_SLOTS,
};
use clutch_solana_layout::projection::{project_slot, OwnerInterner};
use clutch_solana_layout::reservation::ReservationAccount;
use clutch_solana_layout::{
    canonical_order_id, stream, CandidateFeedChunk, EpochAccount, Hash32, OrderRecord, OrderSlot,
    PortfolioRecord, PositionAccount, SupplyLedgerAccount, FEED_FILLS_PER_CHUNK,
    FEED_SLICES_PER_CHUNK, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES,
};
use serde_json::{json, Value};
use solana_keypair::Keypair;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// How far ahead of the epoch's opening the freeze deadline is placed.
pub const FREEZE_WINDOW_SLOTS_DEFAULT: u64 = 400;

/// One order this session placed, remembered so the book can be named.
#[derive(Clone)]
pub struct Placed {
    pub rank: u64,
    pub owner_role: &'static str,
    pub owner: [u8; 32],
    pub id: Hash32,
    pub reservation: [u8; 32],
    pub kind: &'static str,
    pub outcome: u8,
    pub side: u8,
    pub quantity: u64,
    pub limit: u64,
    pub retired: bool,
}

struct Live {
    phase: &'static str,
    ordinal: usize,
    next_rank: u64,
    /// Replay sequence already consumed, per owner, on the Endow/Split plane.
    sequences: BTreeMap<[u8; 32], u64>,
    orders: Vec<Placed>,
    /// The person's painted belief, once they have painted one.
    human_belief: Option<Vec<u64>>,
    freeze_deadline: u64,
    freeze_slot: u64,
    endowed_total: u64,
    split_total: u64,
    latest: BTreeMap<String, Vec<u8>>,
}

pub struct Session {
    pub friday: Friday,
    pub bot: Bot,
    url: String,
    bus: Arc<Bus>,
    keypairs: Vec<Keypair>,
    freeze_window: u64,
    inner: Mutex<Live>,
}

impl Session {
    pub fn new(
        friday: Friday,
        bus: Arc<Bus>,
        keypairs: Vec<Keypair>,
        url: String,
        freeze_window: u64,
    ) -> Self {
        Self {
            bot: Bot::model_e(LADDER_STEP),
            friday,
            url,
            bus,
            keypairs,
            freeze_window,
            inner: Mutex::new(Live {
                phase: "booting",
                ordinal: 0,
                next_rank: 1,
                sequences: BTreeMap::new(),
                orders: Vec::new(),
                human_belief: None,
                freeze_deadline: 0,
                freeze_slot: 0,
                endowed_total: 0,
                split_total: 0,
                latest: BTreeMap::new(),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Live> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /* ------------------------------------------------------------------ */
    /* Submission                                                          */
    /* ------------------------------------------------------------------ */

    /// Sign, submit, confirm, reload, decode, publish.
    ///
    /// One transaction in, one `step` row and a full decoded-state sweep out.
    /// A refusal is reported with the bank's own error and is *not* a fault:
    /// a person can ask the venue for something it will not do, and the point
    /// of the bench is that the refusal is visible and exact.
    fn submit(&self, live: &mut Live, name: &str, family: &str, unsigned: &[u8]) -> Result<Value> {
        live.ordinal += 1;
        let ordinal = live.ordinal;
        self.publish(&json!({
            "type": "step", "ordinal": ordinal, "name": name, "family": family,
            "state": "inflight",
        }));
        let refs: Vec<&Keypair> = self.keypairs.iter().collect();
        let signed = rpc::sign_transaction(unsigned, rpc::latest_blockhash(&self.url)?, &refs)?;
        let signature = rpc::submit(&self.url, &signed)?;
        let status = rpc::await_confirmation(&self.url, &signature)?;
        let error = status.get("err").cloned().unwrap_or(Value::Null);
        let slot = status
            .get("slot")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let accepted = error.is_null();
        self.publish(&json!({
            "type": "step",
            "ordinal": ordinal,
            "name": name,
            "family": family,
            "state": if accepted { "accepted" } else { "refused" },
            "confirmation": status.get("confirmationStatus").and_then(Value::as_str)
                .unwrap_or("unknown"),
            "slot": slot,
            "cu": rpc::compute_units(&self.url, &signature),
            "bytes": signed.len(),
            "signature": signature,
            "error": error,
            "refusal_code": rpc::custom_error_code(&error)
                .map(|code| format!("Custom({code:#06x})")),
        }));
        self.sweep(live)?;
        if accepted {
            Ok(json!({"ok": true, "ordinal": ordinal, "signature": signature, "slot": slot}))
        } else {
            Ok(json!({
                "ok": false, "ordinal": ordinal, "signature": signature,
                "detail": format!("the bank refused {name}: {error}"),
            }))
        }
    }

    /// Reload every account this session watches and publish the decoded image.
    fn sweep(&self, live: &mut Live) -> Result<()> {
        let f = &self.friday;
        let mut roles: Vec<(String, String)> = vec![
            ("friday.market".into(), f.market.address.clone()),
            ("friday.hoard".into(), f.hoard.address.clone()),
            ("friday.supply".into(), f.supply.address.clone()),
            ("friday.hoard-token".into(), f.hoard_token.address.clone()),
            ("friday.epoch".into(), f.epoch.address.clone()),
            ("friday.window".into(), f.window.address.clone()),
            ("friday.page".into(), f.page.address.clone()),
            ("friday.pot".into(), f.pot.address.clone()),
        ];
        for actor in &f.actors {
            roles.push((
                format!("{}.position", actor.role),
                actor.position.address.clone(),
            ));
            roles.push((
                format!("{}.collateral", actor.role),
                actor.token.address.clone(),
            ));
        }
        for order in &live.orders {
            roles.push((
                format!("friday.reservation-{}", order.rank),
                clutch_sbf_harness::base58_of(&order.reservation),
            ));
        }
        for (role, address) in roles {
            let Some(bytes) = rpc::account_bytes(&self.url, &address)? else {
                continue;
            };
            self.publish(&json!({
                "type": "state",
                "ordinal": live.ordinal,
                "role": role,
                "address": address,
                "bytes": bytes.len(),
                "decoded": decode::by_role(&role, &bytes),
            }));
            live.latest.insert(role, bytes);
        }
        self.publish(&self.conservation(live));
        self.publish(&self.book_event(live));
        Ok(())
    }

    /// The value plane, re-derived from the bytes this session has observed.
    fn conservation(&self, live: &Live) -> Value {
        let mut cash = 0_u64;
        let mut reserved = 0_u64;
        let mut eggs = [0_u64; 8];
        let mut rows = Vec::new();
        let mut pending = Vec::new();
        for actor in &self.friday.actors {
            let role = format!("{}.position", actor.role);
            let Some(bytes) = live.latest.get(&role) else {
                pending.push(role);
                continue;
            };
            let Ok(position) = PositionAccount::decode(bytes) else {
                pending.push(role);
                continue;
            };
            cash += position.cash_atoms;
            reserved += position.reserved_cash_atoms;
            for (index, held) in eggs.iter_mut().enumerate() {
                *held += position.internal[index];
            }
            rows.push(json!({
                "role": actor.role,
                "cash": position.cash_atoms,
                "reserved": position.reserved_cash_atoms,
                "eggs": position.internal[..8].to_vec(),
            }));
        }
        for (role, bytes) in &live.latest {
            if !role.contains("reservation-") {
                continue;
            }
            let Ok(reservation) = ReservationAccount::decode(bytes) else {
                continue;
            };
            for (index, held) in eggs.iter_mut().enumerate() {
                *held += reservation.remaining_internal[index];
            }
        }
        let supply = live
            .latest
            .get("friday.supply")
            .and_then(|bytes| SupplyLedgerAccount::decode(bytes).ok());
        let locked = live
            .latest
            .get("friday.hoard")
            .and_then(|bytes| clutch_solana_layout::HoardAccount::decode(bytes).ok())
            .map(|hoard| hoard.collateral_atoms);
        let custody = live
            .latest
            .get("friday.hoard-token")
            .and_then(|bytes| bytes.get(64..72))
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes);
        /* The Eggs a resting sell escrows have left the Position and live in
         * its Reservation; both are counted above, which is why this identity
         * is about the *plane* rather than about one account.  The independent
         * check is the SupplyLedger, which the Split seam writes and no order
         * ever touches: it must still say exactly what was locked. */
        let mut identities = Vec::new();
        if let (Some(locked), Some(custody)) = (locked, custody) {
            identities.push(identity(
                "position cash + locked backing == endowed",
                cash + locked,
                live.endowed_total,
            ));
            identities.push(identity(
                "pooled custody == endowed",
                custody,
                live.endowed_total,
            ));
        }
        if live.split_total > 0 {
            for outcome in [0_usize, usize::from(OUTCOMES) - 1] {
                identities.push(identity(
                    &format!("positions + reservations eggs[{outcome}] == complete sets locked"),
                    eggs[outcome],
                    live.split_total,
                ));
            }
        }
        if let Some(supply) = supply {
            for outcome in [0_usize, usize::from(OUTCOMES) - 1] {
                identities.push(identity(
                    &format!("internal supply[{outcome}] == complete sets locked"),
                    supply.internal_supply[outcome],
                    live.split_total,
                ));
            }
        }
        json!({
            "type": "conservation",
            "live": true,
            "complete": pending.is_empty() && locked.is_some() && custody.is_some(),
            "pending": pending,
            "rows": rows,
            "cash_total": cash,
            "reserved_total": reserved,
            "eggs": eggs,
            "locked": locked,
            "custody": custody,
            "endowed_total": live.endowed_total,
            "split_total": live.split_total,
            "identities": identities,
        })
    }

    #[allow(clippy::unused_self)]
    fn book_event(&self, live: &Live) -> Value {
        let orders: Vec<Value> = live
            .orders
            .iter()
            .map(|order| {
                json!({
                    "rank": order.rank, "owner": order.owner_role, "kind": order.kind,
                    "outcome": order.outcome,
                    "side": if order.side == 0 { "buy" } else { "sell" },
                    "quantity": order.quantity, "limit": order.limit,
                    "retired": order.retired,
                })
            })
            .collect();
        json!({
            "type": "session",
            "phase": live.phase,
            "epoch_index": EPOCH_INDEX,
            "freeze_deadline_slot": live.freeze_deadline,
            "next_rank": live.next_rank,
            "human_belief": live.human_belief,
            "orders": orders,
        })
    }

    fn publish(&self, event: &Value) {
        self.bus.publish(event);
    }

    fn stage(&self, text: &str) {
        self.publish(&json!({"type": "boot", "stage": "session", "text": text}));
        println!("[session] {text}");
    }

    /* ------------------------------------------------------------------ */
    /* Founding                                                            */
    /* ------------------------------------------------------------------ */

    /// Found the market, fund both actors, open the epoch, and let the
    /// automaton rest its opening book.
    pub fn found(&self, endow: u64, split: u64) -> Result<()> {
        let mut live = self.lock();
        let f = &self.friday;
        let human = f.actor("human").ok_or("no human actor")?.key;
        let bot = f.actor("bot").ok_or("no bot actor")?.key;

        self.stage("creating the eight-outcome degree-1 market");
        self.expect(
            &mut live,
            "friday-01-create-market",
            "Founding",
            &builders::create_market(f, human),
        )?;

        for (label, owner, role) in [
            ("friday-02-endow-human", human, "human"),
            ("friday-03-endow-bot", bot, "bot"),
        ] {
            self.stage(&format!("endowing {role}: {endow} collateral atoms"));
            let sequence = live.sequences.get(&owner).copied().unwrap_or(0);
            self.expect(
                &mut live,
                label,
                "Funding",
                &builders::endow(f, owner, sequence, endow),
            )?;
            live.sequences.insert(owner, sequence + 1);
            live.endowed_total += endow;
        }
        for (label, owner, role) in [
            ("friday-04-split-human", human, "human"),
            ("friday-05-split-bot", bot, "bot"),
        ] {
            self.stage(&format!("{role} locks {split} complete sets"));
            let sequence = live.sequences.get(&owner).copied().unwrap_or(0);
            self.expect(
                &mut live,
                label,
                "Funding",
                &builders::split(f, owner, sequence, split),
            )?;
            live.sequences.insert(owner, sequence + 1);
            live.split_total += split;
        }

        let now = rpc::current_slot(&self.url)?;
        let deadline = now + self.freeze_window;
        live.freeze_deadline = deadline;
        self.stage(&format!(
            "opening epoch {EPOCH_INDEX}; the freeze deadline is slot {deadline}"
        ));
        self.expect(
            &mut live,
            "friday-06-init-epoch",
            "Epoch",
            &builders::init_epoch(f, deadline),
        )?;
        self.expect(
            &mut live,
            "friday-07-init-page",
            "Epoch",
            &builders::init_page(f),
        )?;

        self.publish(&json!({"type": "bot", "disclosure": self.bot.disclosure()}));
        self.stage("the automaton rests its opening book");
        for quote in self.bot.opening_quotes() {
            self.rest(&mut live, "bot", bot, quote, "opening")?;
        }
        live.phase = "open";
        self.publish(&self.book_event(&live));
        Ok(())
    }

    /// Submit, and turn a refusal into an error: founding has no admitted
    /// refusals, and one would mean the fixture disagrees with the program.
    fn expect(&self, live: &mut Live, name: &str, family: &str, unsigned: &[u8]) -> Result<()> {
        let outcome = self.submit(live, name, family, unsigned)?;
        if outcome.get("ok").and_then(Value::as_bool) == Some(true) {
            return Ok(());
        }
        Err(outcome
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or(name)
            .to_string()
            .into())
    }

    /* ------------------------------------------------------------------ */
    /* The interactive plane                                               */
    /* ------------------------------------------------------------------ */

    /// How many order slots the single page this epoch was opened with still
    /// has.
    ///
    /// A tombstone keeps its slot: retiring an order frees its envelope, not
    /// its place on the page.  Saying so here, before a transaction is built,
    /// turns a bank refusal nobody can read into a sentence that names the
    /// frozen constant it comes from.
    fn page_room(live: &Live) -> usize {
        MAX_ORDERS_PER_PAGE.saturating_sub(live.orders.len())
    }

    fn rest(
        &self,
        live: &mut Live,
        owner_role: &'static str,
        owner: [u8; 32],
        quote: Quote,
        why: &str,
    ) -> Result<Value> {
        let rank = live.next_rank;
        let id = canonical_order_id(rank);
        let slot = OrderSlot::Single(OrderRecord {
            owner: Hash32::from_bytes(owner),
            order_id: id,
            outcome: quote.outcome,
            side: quote.side,
            quantity: quote.quantity,
            limit: quote.limit,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: EPOCH_INDEX,
        });
        let reservation = self.friday.reservation(Hash32::from_bytes(owner), id).bytes;
        live.orders.push(Placed {
            rank,
            owner_role,
            owner,
            id,
            reservation,
            kind: "single",
            outcome: quote.outcome,
            side: quote.side,
            quantity: quote.quantity,
            limit: quote.limit,
            retired: false,
        });
        live.next_rank += 1;
        let name = format!("friday-place-{rank}-{owner_role}-{why}");
        let outcome = self.submit(
            live,
            &name,
            "PlaceOrder",
            &builders::place_order(&self.friday, owner, rank - 1, slot),
        )?;
        if outcome.get("ok").and_then(Value::as_bool) != Some(true) {
            // A refused placement never rested: forget it, and give the rank
            // back so the page's ranks stay the contiguous claim the program
            // requires.
            live.orders.pop();
            live.next_rank -= 1;
        }
        Ok(outcome)
    }

    /// The person places one single-Egg order; the automaton answers it if it
    /// crosses.
    pub fn place_single(&self, outcome: u8, side: u8, quantity: u64, limit: u64) -> Result<Value> {
        let mut live = self.lock();
        if live.phase != "open" {
            return Ok(refused(&format!("the book is {}, not open", live.phase)));
        }
        if outcome >= OUTCOMES {
            return Ok(refused("this market has eight outcomes, numbered 0 to 7"));
        }
        if !limit.is_multiple_of(LADDER_STEP) || limit > PRICE_SCALE {
            return Ok(refused(&format!(
                "the frozen ladder admits multiples of {LADDER_STEP} up to {PRICE_SCALE}; \
                 {limit} is not one"
            )));
        }
        if Self::page_room(&live) == 0 {
            return Ok(refused(&format!(
                "this epoch was opened with one page and a page holds \
                 {MAX_ORDERS_PER_PAGE} orders; all of them are taken"
            )));
        }
        let human = self.friday.actor("human").ok_or("no human actor")?.key;
        let quote = Quote {
            outcome,
            side,
            quantity,
            limit,
        };
        let placed = self.rest(&mut live, "human", human, quote, "ticket")?;
        if placed.get("ok").and_then(Value::as_bool) != Some(true) {
            return Ok(placed);
        }
        let resting: Vec<Quote> = live
            .orders
            .iter()
            .filter(|order| order.owner_role == "bot" && !order.retired)
            .map(|order| Quote {
                outcome: order.outcome,
                side: order.side,
                quantity: order.quantity,
                limit: order.limit,
            })
            .collect();
        if let Some(answer) = self.bot.response_to(quote, &resting) {
            let bot = self.friday.actor("bot").ok_or("no bot actor")?.key;
            self.stage("the automaton answers a crossing order at its own value");
            self.rest(&mut live, "bot", bot, answer, "response")?;
        }
        self.publish(&self.book_event(&live));
        Ok(placed)
    }

    /// The person places one portfolio ticket: a coefficient vector, lots, and
    /// a per-lot collateral bound.
    pub fn place_portfolio(
        &self,
        coefficients: &[u64],
        side: u8,
        lots: u64,
        limit_per_lot: u64,
    ) -> Result<Value> {
        let mut live = self.lock();
        if live.phase != "open" {
            return Ok(refused(&format!("the book is {}, not open", live.phase)));
        }
        if coefficients.len() > usize::from(OUTCOMES) {
            return Ok(refused(
                "a portfolio ticket carries at most eight coefficients",
            ));
        }
        if Self::page_room(&live) == 0 {
            return Ok(refused(&format!(
                "this epoch was opened with one page and a page holds \
                 {MAX_ORDERS_PER_PAGE} orders; all of them are taken"
            )));
        }
        let human = self.friday.actor("human").ok_or("no human actor")?.key;
        let mut packed = [0_u64; MAX_OUTCOMES];
        packed[..coefficients.len()].copy_from_slice(coefficients);
        let active_len = u8::try_from(coefficients.len()).unwrap_or(OUTCOMES);
        let rank = live.next_rank;
        let id = canonical_order_id(rank);
        let slot = OrderSlot::Portfolio(PortfolioRecord {
            owner: Hash32::from_bytes(human),
            order_id: id,
            side,
            active_len,
            flags: 0,
            coefficients: packed,
            lots,
            limit_collateral_per_lot: limit_per_lot,
            minimum_fill_lots: 0,
            generation: 1,
            expiry_epoch: EPOCH_INDEX,
        });
        let reservation = self.friday.reservation(Hash32::from_bytes(human), id).bytes;
        live.orders.push(Placed {
            rank,
            owner_role: "human",
            owner: human,
            id,
            reservation,
            kind: "portfolio",
            outcome: u8::MAX,
            side,
            quantity: lots,
            limit: limit_per_lot,
            retired: false,
        });
        live.next_rank += 1;
        let name = format!("friday-place-{rank}-human-portfolio");
        let outcome = self.submit(
            &mut live,
            &name,
            "PlaceOrder",
            &builders::place_order(&self.friday, human, rank - 1, slot),
        )?;
        if outcome.get("ok").and_then(Value::as_bool) != Some(true) {
            live.orders.pop();
            live.next_rank -= 1;
        }
        self.publish(&self.book_event(&live));
        Ok(outcome)
    }

    /// Retire one of the person's own orders.
    pub fn cancel(&self, rank: u64) -> Result<Value> {
        let mut live = self.lock();
        if live.phase != "open" {
            return Ok(refused(&format!("the book is {}, not open", live.phase)));
        }
        let Some(index) = live
            .orders
            .iter()
            .position(|order| order.rank == rank && !order.retired)
        else {
            return Ok(refused(&format!("no live order is ranked {rank}")));
        };
        if live.orders[index].owner_role != "human" {
            return Ok(refused("only your own orders can be retired from here"));
        }
        let order = live.orders[index].clone();
        let name = format!("friday-cancel-{rank}");
        let outcome = self.submit(
            &mut live,
            &name,
            "CancelOrder",
            &builders::cancel_order(&self.friday, order.owner, order.id, 2),
        )?;
        if outcome.get("ok").and_then(Value::as_bool) == Some(true) {
            live.orders[index].retired = true;
        }
        self.publish(&self.book_event(&live));
        Ok(outcome)
    }

    /// The painter's proposal: a belief, quantized, inverted against the
    /// automaton's resting quotes.
    ///
    /// Every number this returns is MODEL-ONLY.  Nothing has been submitted;
    /// the person still has to place the list.
    pub fn propose(&self, weights: &[u64]) -> Value {
        let live = self.lock();
        let Some(belief) = belief_on_ladder(weights, LADDER_STEP) else {
            return refused("a belief needs at least one nonzero knot");
        };
        let proposals: Vec<Value> = self
            .bot
            .quoted
            .iter()
            .enumerate()
            .filter_map(|(index, theirs)| {
                let mine = *belief.get(index)?;
                /* A second order of yours on a knot the automaton quotes once
                 * cannot clear: the relation fills every *strict* order in
                 * full, so two 500-Egg buys against one 500-Egg offer is a
                 * `StrictUnderfill` for the whole candidate, not a partial
                 * fill.  The painter therefore proposes only where you are not
                 * already resting. */
                if live.orders.iter().any(|order| {
                    order.owner_role == "human"
                        && !order.retired
                        && usize::from(order.outcome) == index
                }) {
                    return None;
                }
                let resting = live.orders.iter().find(|order| {
                    order.owner_role == "bot"
                        && !order.retired
                        && usize::from(order.outcome) == index
                })?;
                // Invert the automaton's own book-former: I take the side its
                // resting quote leaves open, at my own value, when my value
                // crosses its limit.
                let side = match (resting.side, mine.cmp(theirs)) {
                    (1, std::cmp::Ordering::Greater) => 0,
                    (0, std::cmp::Ordering::Less) => 1,
                    _ => return None,
                };
                Some(json!({
                    "outcome": index,
                    "side": if side == 0 { "buy" } else { "sell" },
                    "quantity": self.bot.size,
                    "limit": mine,
                    "crosses_rank": resting.rank,
                    "their_limit": theirs,
                }))
            })
            .collect();
        json!({
            "ok": true,
            "label": "MODEL-ONLY",
            "note": "nothing here has been submitted; the bank has committed none of it",
            "belief": belief,
            "ladder_step": LADDER_STEP,
            "proposed": proposals,
        })
    }

    /// Place every order the painter proposed, and remember the belief the
    /// auto-crank will read the clearing price out of.
    pub fn paint(&self, weights: &[u64]) -> Result<Value> {
        let proposal = self.propose(weights);
        if proposal.get("ok").and_then(Value::as_bool) != Some(true) {
            return Ok(proposal);
        }
        let belief: Vec<u64> = proposal["belief"]
            .as_array()
            .map(|values| values.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();
        {
            let mut live = self.lock();
            live.human_belief = Some(belief.clone());
        }
        self.publish(&json!({
            "type": "belief",
            "label": "MODEL-ONLY",
            "belief": belief,
            "proposed": proposal["proposed"].clone(),
        }));
        let mut placed = Vec::new();
        let mut skipped = Vec::new();
        for entry in proposal["proposed"].as_array().unwrap_or(&Vec::new()) {
            let outcome = u8::try_from(entry["outcome"].as_u64().unwrap_or(0)).unwrap_or(0);
            let side = u8::from(entry["side"].as_str() == Some("sell"));
            let quantity = entry["quantity"].as_u64().unwrap_or(0);
            let limit = entry["limit"].as_u64().unwrap_or(0);
            let outcome_value = self.place_single(outcome, side, quantity, limit)?;
            if outcome_value.get("ok").and_then(Value::as_bool) == Some(true) {
                placed.push(outcome_value);
            } else {
                skipped.push(json!({
                    "outcome": outcome,
                    "detail": outcome_value.get("detail").cloned().unwrap_or(Value::Null),
                }));
            }
        }
        Ok(json!({
            "ok": true, "belief": belief, "placed": placed, "skipped": skipped,
        }))
    }

    /* ------------------------------------------------------------------ */
    /* Freeze, clear, settle                                               */
    /* ------------------------------------------------------------------ */

    /// Close the book at the deadline, then drive the epoch to settled.
    ///
    /// `FreezeEpoch` is clock-gated by the program, so "Freeze" here means
    /// *close as soon as the bank's own clock admits it* and the wait is the
    /// validator's real clock, ticked to the browser.
    pub fn freeze_and_settle(&self) -> Result<()> {
        {
            let mut live = self.lock();
            if live.phase != "open" {
                return Err(format!("the book is {}, not open", live.phase).into());
            }
            let target = live.freeze_deadline;
            let mut tick = |now: u64, target: u64, reason: &str| {
                self.publish(&json!({
                    "type": "clock", "slot": now, "target": target, "reason": reason,
                    "remaining": target.saturating_sub(now),
                }));
            };
            rpc::wait_for_slot(&self.url, target, "freeze deadline", &mut tick)?;
            self.stage("closing the book at the deadline");
            self.expect(
                &mut live,
                "friday-freeze-epoch",
                "Freeze",
                &builders::freeze_epoch(&self.friday),
            )?;
            live.phase = "frozen";
            live.freeze_slot = rpc::current_slot(&self.url)?;
            self.publish(&self.book_event(&live));
        }
        self.clear()
    }

    #[allow(clippy::too_many_lines)] // the clearing walk is one sequence
    fn clear(&self) -> Result<()> {
        let mut live = self.lock();
        let f = &self.friday;
        let epoch_bytes = live
            .latest
            .get("friday.epoch")
            .cloned()
            .ok_or("the frozen epoch was never reloaded")?;
        let page_bytes = live
            .latest
            .get("friday.page")
            .cloned()
            .ok_or("the frozen page was never reloaded")?;
        let epoch = EpochAccount::decode(&epoch_bytes)
            .map_err(|error| format!("the frozen epoch does not decode: {error:?}"))?;
        let (book, identities) = project(&page_bytes)?;
        let reservations: Vec<[u8; 32]> = identities
            .iter()
            .map(|id| {
                live.orders
                    .iter()
                    .find(|order| order.id == *id)
                    .map(|order| order.reservation)
                    .ok_or_else(|| {
                        "the frozen book holds an order this session never placed".into()
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        let domain = RelationDomainV1 {
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
        };

        let (prices, basis) = self.clearing_prices(&live, &domain, &book)?;
        self.stage(&format!(
            "the cleared vector is {prices:?}, taken as {basis}"
        ));
        let candidate = canonical_candidate(&domain, &book, &prices, 0, 0)
            .map_err(|error| format!("no canonical candidate at the cleared vector: {error:?}"))?;
        let witness = canonical_pairing(&domain, &book, &candidate)
            .map_err(|error| format!("the candidate has no canonical pairing: {error:?}"))?;
        let fills = candidate.fills[..book.len as usize].to_vec();
        self.publish(&json!({
            "type": "clearing",
            "prices": prices[..usize::from(OUTCOMES)].to_vec(),
            "price_basis": basis,
            "fills": fills,
            "slices": witness.len,
            "virtual_split": candidate.virtual_split,
            "virtual_merge": candidate.virtual_merge,
            "label": "MODEL-ONLY until the bank verifies it",
        }));

        let mut shell = CandidateFeedHeader {
            candidate: Hash32::ZERO,
            epoch: f.epoch_id,
            market: f.market_id,
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
            outcome_count: epoch.outcome_count,
            stored_bump: 0,
            flags: 0,
        };
        shell.candidate = shell
            .recomputed_candidate_digest()
            .map_err(|error| format!("the candidate shell does not digest: {error:?}"))?;
        let id = shell.candidate;
        let submission = builders::Submission {
            id,
            prices,
            virtual_split: candidate.virtual_split,
            virtual_merge: candidate.virtual_merge,
            record: f.candidate_record(id).bytes,
            feed: f.candidate_feed(id).bytes,
        };

        self.stage("submitting the candidate and its exact fill/slice feed");
        self.expect(
            &mut live,
            "friday-submit-candidate",
            "Selection",
            &builders::submit_candidate(f, &submission, Some(witness.len)),
        )?;
        let mut written = 0_u64;
        for chunk in fills.chunks(FEED_FILLS_PER_CHUNK) {
            let mut buffer = [0_u64; FEED_FILLS_PER_CHUNK];
            buffer[..chunk.len()].copy_from_slice(chunk);
            self.expect(
                &mut live,
                &format!("friday-write-fills-{written}"),
                "Selection",
                &builders::write_feed(
                    f,
                    &submission,
                    written,
                    &CandidateFeedChunk::Fills {
                        count: u8::try_from(chunk.len())?,
                        fills: buffer,
                    },
                ),
            )?;
            written += chunk.len() as u64;
        }
        for chunk in slices_of(&witness).chunks(FEED_SLICES_PER_CHUNK) {
            let mut buffer = [PairingSlice::PADDING; FEED_SLICES_PER_CHUNK];
            buffer[..chunk.len()].copy_from_slice(chunk);
            self.expect(
                &mut live,
                &format!("friday-write-slices-{written}"),
                "Selection",
                &builders::write_feed(
                    f,
                    &submission,
                    written,
                    &CandidateFeedChunk::Slices {
                        count: u8::try_from(chunk.len())?,
                        slices: buffer,
                    },
                ),
            )?;
            written += chunk.len() as u64;
        }
        self.expect(
            &mut live,
            "friday-seal-candidate",
            "Selection",
            &builders::seal_candidate(f, &submission),
        )?;

        self.stage("walking the candidate to a verdict through the streaming relation");
        self.expect(
            &mut live,
            "friday-init-clear-work",
            "ClearWalk",
            &builders::init_clear_work(f, id),
        )?;
        for sequence in 1..=4_u64 {
            self.expect(
                &mut live,
                &format!("friday-grow-clear-work-{sequence}"),
                "ClearWalk",
                &builders::grow_clear_work(f, id, sequence),
            )?;
        }
        let max_orders = u16::from(book.len);
        self.expect(
            &mut live,
            "friday-advance-pass-one",
            "ClearWalk",
            &builders::advance_clear_work(f, id, max_orders, &reservations),
        )?;
        self.expect(
            &mut live,
            "friday-advance-slices",
            "ClearWalk",
            &builders::advance_clear_slices(f, id, witness.len),
        )?;
        self.expect(
            &mut live,
            "friday-advance-pass-two",
            "ClearWalk",
            &builders::advance_clear_work(f, id, max_orders, &[]),
        )?;
        self.expect(
            &mut live,
            "friday-complete-clear-work",
            "ClearWalk",
            &builders::complete_clear_work(f, id),
        )?;

        let target = live.freeze_slot + CANDIDATE_WINDOW_SLOTS;
        let mut tick = |now: u64, target: u64, reason: &str| {
            self.publish(&json!({
                "type": "clock", "slot": now, "target": target, "reason": reason,
                "remaining": target.saturating_sub(now),
            }));
        };
        rpc::wait_for_slot(&self.url, target, "candidate window", &mut tick)?;
        self.expect(
            &mut live,
            "friday-finalize-selection",
            "Selection",
            &builders::finalize_selection(f, &[id]),
        )?;
        self.expect(
            &mut live,
            "friday-freeze-entitlement",
            "Entitlement",
            &builders::freeze_entitlement(f, id),
        )?;
        live.phase = "cleared";
        self.publish(&self.book_event(&live));

        /* Two passes, in the order the exhibit proved: every entitlement
         * first, then every settlement.  Entitlement moves an order's claim
         * on the selected witness; settlement moves the value.  Interleaving
         * them would be a third ordering nobody has evidence for. */
        let groups = group_slices(&witness, &book);
        self.stage(&format!(
            "entitling {} pairing group(s) of the selected candidate",
            groups.len()
        ));
        let mut legs = Vec::new();
        for group in &groups {
            let buy = reservations
                .get(usize::from(group.buy_index))
                .copied()
                .ok_or("a slice names an order outside the frozen book")?;
            let sell_side = reservations
                .get(usize::from(group.sell_index))
                .copied()
                .ok_or("a slice names an order outside the frozen book")?;
            let pair = if group.portfolio {
                Some(group.slices.as_slice())
            } else {
                None
            };
            self.expect(
                &mut live,
                &format!("friday-entitle-slice-{}", group.slices[0]),
                "Entitlement",
                &builders::entitle_slice(
                    f,
                    &builders::Entitle {
                        candidate: id,
                        slice_index: group.slices[0],
                        buy_reservation: buy,
                        sell_reservation: sell_side,
                        pair_receipts: pair,
                    },
                ),
            )?;
            let buyer = self.position_of(&live, group.buy_index, &reservations)?;
            let seller = self.position_of(&live, group.sell_index, &reservations)?;
            legs.push((buy, sell_side, buyer, seller));
        }

        self.stage("settling every entitled group");
        for (group, (buy, sell_side, buyer, seller)) in groups.iter().zip(&legs) {
            let pair = if group.portfolio {
                Some(group.slices.as_slice())
            } else {
                None
            };
            self.expect(
                &mut live,
                &format!("friday-settle-slice-{}", group.slices[0]),
                "Settlement",
                &builders::settle_page(
                    f,
                    &builders::Settle {
                        candidate: id,
                        /* One-based, as the exhibit's settle loop numbers
                         * them: the request sequence is the settlement's own
                         * ordinal, not the slice index. */
                        sequence: 1 + u64::from(group.slices[0]),
                        buyer_position: *buyer,
                        seller_position: *seller,
                        buy_reservation: *buy,
                        sell_reservation: *sell_side,
                        slice_index: group.slices[0],
                        pair_receipts: pair,
                    },
                ),
            )?;
        }
        live.phase = "settled";
        self.publish(&self.book_event(&live));
        self.publish(&json!({
            "type": "done",
            "verdict": "SETTLED",
            "scope": "SBF_EXECUTED",
            "promotion": "unpromoted",
        }));
        Ok(())
    }

    /// Which of the two positions a book index belongs to.
    fn position_of(&self, live: &Live, index: u8, reservations: &[[u8; 32]]) -> Result<[u8; 32]> {
        let reservation = reservations
            .get(usize::from(index))
            .ok_or("a slice names an order outside the frozen book")?;
        let order = live
            .orders
            .iter()
            .find(|order| order.reservation == *reservation)
            .ok_or("the frozen book holds an order this session never placed")?;
        let actor = self
            .friday
            .actors
            .iter()
            .find(|actor| actor.key == order.owner)
            .ok_or("an order names an owner this session does not hold")?;
        Ok(actor.position.bytes)
    }

    /// The cleared vector, and how it was chosen.
    ///
    /// This is **not** a solver and makes no optimality claim.  It is four
    /// stated coordinates, tried in a fixed published order, and the bench
    /// says which one the relation admitted and exactly how it refused the
    /// ones before it.
    ///
    /// Why these four, and why in this order:
    ///
    /// 1. *The midpoint of the two published beliefs.*  Both are on the limit
    ///    ladder and each sums to the price scale, so the midpoint does too.
    ///    At a knot where the two disagree it sits strictly between the two
    ///    limits, so both sides of that crossing are eligible; at a knot where
    ///    only one side quoted, it sits on the far side of that quote's limit,
    ///    which makes the unpaired order *ineligible* rather than a strict
    ///    order nobody can fill.  That second property is the load-bearing
    ///    one: the frozen allocation policy fills every strict order in full,
    ///    so an eligible order with no counterparty refuses the whole
    ///    candidate.
    /// 2. *The automaton's belief.*  Every one of its quotes sits exactly at
    ///    the price, which makes them marginal rather than strict — a book of
    ///    quotes nobody answered still clears.
    /// 3. *The person's belief*, for the same reason from the other side.
    /// 4. *The flat prior*, which is the coordinate that assumes nothing.
    fn clearing_prices(
        &self,
        live: &Live,
        domain: &RelationDomainV1,
        book: &BookV1,
    ) -> Result<([u64; MAX_OUTCOMES], &'static str)> {
        let bot = &self.bot.quoted;
        let human = live
            .human_belief
            .clone()
            .unwrap_or_else(|| vec![PRICE_SCALE / u64::from(OUTCOMES); usize::from(OUTCOMES)]);
        let mut midpoint = [0_u64; MAX_OUTCOMES];
        for (index, entry) in midpoint.iter_mut().enumerate().take(usize::from(OUTCOMES)) {
            *entry = u64::midpoint(
                bot.get(index).copied().unwrap_or(0),
                human.get(index).copied().unwrap_or(0),
            );
        }
        let mut only_bot = [0_u64; MAX_OUTCOMES];
        only_bot[..bot.len().min(MAX_OUTCOMES)]
            .copy_from_slice(&bot[..bot.len().min(MAX_OUTCOMES)]);
        let mut only_human = [0_u64; MAX_OUTCOMES];
        for (index, entry) in only_human
            .iter_mut()
            .enumerate()
            .take(usize::from(OUTCOMES))
        {
            *entry = human.get(index).copied().unwrap_or(0);
        }
        let mut flat = [0_u64; MAX_OUTCOMES];
        let share = PRICE_SCALE / u64::from(OUTCOMES);
        for entry in flat.iter_mut().take(usize::from(OUTCOMES)) {
            *entry = share;
        }
        let mut attempts = Vec::new();
        for (prices, basis) in [
            (midpoint, "the midpoint of the two published beliefs"),
            (only_bot, "the automaton's published belief"),
            (only_human, "your painted belief"),
            (flat, "the flat prior"),
        ] {
            match canonical_candidate(domain, book, &prices, 0, 0) {
                Ok(_) => {
                    self.publish(&json!({
                        "type": "clearing-attempt", "basis": basis, "taken": true,
                        "prices": prices[..usize::from(OUTCOMES)].to_vec(),
                        "attempts": attempts,
                    }));
                    return Ok((prices, basis));
                }
                Err(error) => {
                    let refusal = format!("{error:?}");
                    self.publish(&json!({
                        "type": "clearing-attempt", "basis": basis, "taken": false,
                        "prices": prices[..usize::from(OUTCOMES)].to_vec(),
                        "refusal": refusal,
                    }));
                    attempts.push(format!("{basis}: {refusal}"));
                }
            }
        }
        Err(format!(
            "no admitted price vector clears this book ({})",
            attempts.join("; ")
        )
        .into())
    }

    /// The session's opening banner rows.
    pub fn identity(&self) -> Value {
        let f = &self.friday;
        json!({
            "mode": "trade",
            "market": clutch_sbf_harness::hex_encode(&f.market_id.bytes()),
            "market_account": f.market.address,
            "terms": clutch_sbf_harness::hex_encode(&f.terms_value.terms.bytes()),
            "basis_degree": f.terms_value.basis_degree,
            "knot_count": f.terms_value.knot_count,
            "knots_cents": crate::friday::KNOT_CENTS.iter()
                .map(|cents| u64::try_from(*cents).unwrap_or(u64::MAX))
                .collect::<Vec<_>>(),
            "outcome_count": OUTCOMES,
            "price_scale": PRICE_SCALE,
            "ladder_step": LADDER_STEP,
            "statistic_id": f.terms_value.statistic_id,
            "edge_policy_id": f.terms_value.edge_policy_id,
            "actors": f.actors.iter().map(|actor| json!({
                "role": actor.role, "label": actor.label,
                "owner_id": clutch_sbf_harness::hex_encode(&actor.id.bytes()[..6]),
                "pubkey": clutch_sbf_harness::base58_of(&actor.key),
                "position": actor.position.address,
                "collateral": actor.token.address,
            })).collect::<Vec<_>>(),
        })
    }

    /// What a terminal statistic of `cents` would pay, per unit held on each
    /// knot: the degree-1 hats, largest-remainder quantized onto the payout
    /// denominator.
    ///
    /// This is the same rule `docs/site-plan/friday_clutch_check.py` prints
    /// the illustration's numbers with, and the unit test in `quantize.rs`
    /// pins its vectors against that script.  Nothing resolves in this
    /// session, so every number here is MODEL-ONLY and says so.
    #[allow(clippy::unused_self)]
    pub fn weights_at(&self, cents: u64) -> Value {
        let weights = resolution_weights(cents);
        json!({
            "ok": true,
            "label": "MODEL-ONLY",
            "note": "this session never resolves; these are the weights a terminal \
                     statistic here would carry, not a payout the bank has made",
            "cents": cents,
            "denominator": PAYOUT_DENOMINATOR,
            "weights": weights,
            "rule": "degree-1 open-clamped hats, largest-remainder quantized, \
                     lowest-index ties (EDGE-CLAMP-01, STAT-TERMINAL-01)",
        })
    }

    pub fn snapshot(&self) -> Value {
        let live = self.lock();
        self.book_event(&live)
    }

    /// Stop the session and say why, in the stream, where the bench reads it.
    ///
    /// A fault is a *daemon* failure — a builder that produced something the
    /// bank would not take, or an RPC that went away.  A refusal is not a
    /// fault and never lands here: the bank refusing a person's order is an
    /// answer, and it is rendered as one.
    pub fn fault(&self, text: &str) {
        {
            let mut live = self.lock();
            live.phase = "faulted";
        }
        self.publish(&json!({"type": "fault", "text": text}));
        eprintln!("session fault: {text}");
    }
}

fn refused(detail: &str) -> Value {
    json!({"ok": false, "detail": detail})
}

fn identity(label: &str, observed: u64, expected: u64) -> Value {
    json!({
        "label": label, "observed": observed, "expected": expected,
        "ok": observed == expected,
    })
}

/// Project the frozen page into the relation's book.
///
/// The second return is the live orders' identities in walk order, which is
/// the order every clear-walk account list is indexed by: the caller resolves
/// each to the reservation address it already holds, rather than re-deriving
/// a program address it has derived once already.
fn project(page: &[u8]) -> Result<(BookV1, Vec<Hash32>)> {
    let header = stream::OrderPageHeader::decode(page)
        .map_err(|error| format!("the frozen page header does not decode: {error:?}"))?;
    let mut cursor = stream::OrderSlotCursor::new(page)
        .map_err(|error| format!("the frozen page does not walk: {error:?}"))?;
    let mut book = BookV1::empty();
    let mut owners = OwnerInterner::new();
    let mut identities = Vec::new();
    let mut live = 0_u16;
    for _ in 0..header.order_count {
        let Some(next) = cursor.next_slot() else {
            break;
        };
        let slot = next.map_err(|error| format!("a populated slot does not decode: {error:?}"))?;
        let projected = project_slot(&slot, u64::from(live) + 1, &mut owners)
            .map_err(|error| format!("a live order does not project: {error:?}"))?;
        if let Some(order) = projected {
            book.orders[usize::from(live)] = order;
            identities.push(slot.order_id());
            live += 1;
        }
    }
    book.len = u8::try_from(live)?;
    Ok((book, identities))
}

/// One entitlement/settlement unit: every witness slice that shares a
/// buy/sell pair, and whether either leg is a portfolio order.
struct Group {
    buy_index: u8,
    sell_index: u8,
    portfolio: bool,
    slices: Vec<u16>,
}

fn group_slices(witness: &PairingWitnessV1, book: &BookV1) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for index in 0..usize::from(witness.len) {
        let slice = witness.slices[index];
        let (LegRefV1::Order(buy), LegRefV1::Order(sell)) = (slice.buy_ref, slice.sell_ref) else {
            continue;
        };
        let portfolio = is_portfolio(book, buy) || is_portfolio(book, sell);
        let cursor = u16::try_from(index).unwrap_or(u16::MAX);
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.buy_index == buy && group.sell_index == sell)
        {
            group.slices.push(cursor);
        } else {
            groups.push(Group {
                buy_index: buy,
                sell_index: sell,
                portfolio,
                slices: vec![cursor],
            });
        }
    }
    groups
}

fn is_portfolio(book: &BookV1, index: u8) -> bool {
    usize::from(index) < usize::from(book.len)
        && matches!(
            book.orders[usize::from(index)],
            clutch_batch::relation_v1::OrderV1::Portfolio(_)
        )
}

fn slices_of(witness: &PairingWitnessV1) -> Vec<PairingSlice> {
    (0..usize::from(witness.len))
        .map(|index| {
            let slice = witness.slices[index];
            let leg = |reference: LegRefV1| match reference {
                LegRefV1::Order(order) => LegRef::Order(order),
                LegRefV1::Split => LegRef::Split,
                LegRefV1::Merge => LegRef::Merge,
            };
            PairingSlice {
                buy_ref: leg(slice.buy_ref),
                sell_ref: leg(slice.sell_ref),
                outcome: slice.outcome,
                quantity: slice.quantity,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{canonical_order_id, OrderRecord, OrderSlot, RELATION_VERSION};

    fn domain(owner_count: u16) -> RelationDomainV1 {
        RelationDomainV1 {
            relation_version: RELATION_VERSION,
            market_id: 0,
            book_id: 0,
            epoch: EPOCH_INDEX,
            policy_id: 0,
            order_set_id: 0,
            outcome_count: OUTCOMES,
            owner_count,
            price_scale: PRICE_SCALE,
            remainder_seed: 0,
            policy: GENERAL_CLEARING_POLICY_V1,
        }
    }

    fn single(owner: u8, rank: u64, outcome: u8, side: u8, quantity: u64, limit: u64) -> OrderSlot {
        OrderSlot::Single(OrderRecord {
            owner: Hash32::from_bytes([owner; 32]),
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

    fn book_of(slots: &[OrderSlot]) -> BookV1 {
        let mut book = BookV1::empty();
        let mut owners = OwnerInterner::new();
        for (index, slot) in slots.iter().enumerate() {
            book.orders[index] = project_slot(slot, index as u64 + 1, &mut owners)
                .expect("the fixture order projects")
                .expect("the fixture order is live");
        }
        book.len = u8::try_from(slots.len()).expect("a page holds at most sixteen");
        book
    }

    fn prices_of(values: [u64; 8]) -> [u64; MAX_OUTCOMES] {
        let mut prices = [0_u64; MAX_OUTCOMES];
        prices[..8].copy_from_slice(&values);
        prices
    }

    /// Try the daemon's published ladder against a book, in order, and say
    /// which coordinate the relation admitted.
    fn ladder(book: &BookV1, bot: &[u64; 8], human: &[u64; 8]) -> (String, PairingWitnessV1) {
        let mut midpoint = [0_u64; 8];
        for index in 0..8 {
            midpoint[index] = u64::midpoint(bot[index], human[index]);
        }
        let mut refusals = Vec::new();
        for (values, basis) in [
            (midpoint, "midpoint"),
            (*bot, "automaton"),
            (*human, "painted"),
            ([1_250_u64; 8], "flat"),
        ] {
            let prices = prices_of(values);
            match canonical_candidate(&domain(2), book, &prices, 0, 0) {
                Ok(candidate) => {
                    let witness = canonical_pairing(&domain(2), book, &candidate)
                        .unwrap_or_else(|error| panic!("{basis} pairs: {error:?}"));
                    return (basis.to_string(), witness);
                }
                Err(error) => refusals.push(format!("{basis}: {error:?}")),
            }
        }
        panic!(
            "no stated coordinate cleared the book: {}",
            refusals.join("; ")
        );
    }

    /// The gate's own book: the automaton's eight opening quotes, the person's
    /// three ticket orders, and the four the painter adds on the knots the
    /// person is not already resting on.
    ///
    /// This is the exact shape `scripts/run_operator_trade.sh` drives, pinned
    /// here so the clearing question is answered in milliseconds rather than
    /// after a ten-minute validator run.
    #[test]
    fn a_stated_coordinate_clears_the_bench_book() {
        let slots = vec![
            single(1, 1, 0, 1, 500, 0),      // the automaton sells the $100 hat
            single(1, 2, 1, 1, 500, 200),    // sells the $120 hat
            single(1, 3, 2, 0, 500, 2_600),  // buys the $140 hat
            single(1, 4, 3, 0, 500, 6_000),  // buys the $160 hat
            single(1, 5, 4, 0, 500, 1_200),  // buys the $180 hat
            single(1, 6, 5, 1, 500, 0),      // sells the $200 hat
            single(1, 7, 6, 1, 500, 0),      // sells the $220 hat
            single(1, 8, 7, 1, 500, 0),      // sells the $240 hat
            single(2, 9, 3, 1, 500, 5_800),  // the person sells into the bid
            single(2, 10, 1, 0, 500, 400),   // lifts the $120 offer
            single(2, 11, 5, 0, 500, 200),   // an uncrossed low-ball
            single(2, 12, 0, 0, 500, 200),   // painted
            single(2, 13, 2, 1, 500, 1_600), // painted
            single(2, 14, 6, 0, 500, 400),   // painted
            single(2, 15, 7, 0, 500, 200),   // painted
        ];
        let book = book_of(&slots);
        let bot = [0_u64, 200, 2_600, 6_000, 1_200, 0, 0, 0];
        let painted = [200_u64, 400, 1_600, 3_400, 2_600, 1_200, 400, 200];
        let (basis, witness) = ladder(&book, &bot, &painted);
        assert!(witness.len > 0, "the cleared book must pair something");
        assert_eq!(
            basis, "automaton",
            "the midpoint is priced out by the person's $160 ticket, so the \
             automaton's own belief is the coordinate that clears"
        );
    }

    /// Two orders of yours against one quote of the automaton's cannot clear:
    /// the frozen allocation policy fills every strict order in full.  This is
    /// the refusal the painter's skip rule exists to avoid, pinned so the rule
    /// cannot be removed as redundant.
    #[test]
    fn doubling_up_on_one_knot_refuses_the_whole_candidate() {
        let slots = vec![
            single(1, 1, 1, 1, 500, 200),
            single(2, 2, 1, 0, 500, 400),
            single(2, 3, 1, 0, 500, 400),
        ];
        let book = book_of(&slots);
        let prices = prices_of([1_250, 1_250, 1_250, 1_250, 1_250, 1_250, 1_250, 1_250]);
        let refusal = canonical_candidate(&domain(2), &book, &prices, 0, 0)
            .expect_err("a doubled-up knot has no canonical candidate");
        assert!(format!("{refusal:?}").contains("StrictUnderfill"));
    }
}
