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
use crate::integer;
use crate::quantize::{belief_on_ladder, resolution_weights, PAYOUT_DENOMINATOR};
use crate::rpc;
use clutch_batch::relation_v1::{
    canonical_candidate, canonical_pairing, BookV1, LegRefV1, PairingWitnessV1, RelationDomainV1,
};
use clutch_batch_policy_identity::general_clearing_v1::GENERAL_CLEARING_POLICY_V1;
use clutch_client_contract::settlement::{classify_direct_settlement, SettlementProjection};
use clutch_sbf_harness::Pda;
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
    pub reservation_address: String,
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
    latest: BTreeMap<String, rpc::AccountEnvelope>,
}

struct WatchedAccount {
    role: String,
    address: String,
    address_binding: &'static str,
    token_binding: Option<TokenBinding>,
}

#[derive(Clone, Copy)]
struct TokenBinding {
    mint: [u8; 32],
    authority: [u8; 32],
}

struct ValidatedEnvelope {
    role_schema: decode::RoleSchema,
    decoded: decode::VerifiedDecode,
}

struct PreparedSnapshot {
    states: Vec<Value>,
    next_latest: BTreeMap<String, rpc::AccountEnvelope>,
}

const GRAPH_SNAPSHOT_V2_SCHEMA: &str =
    "dragons-clutch/operator/graph-root-bracketed-account-snapshot/v2";

fn watched_pda(role: String, pda: &Pda, address_binding: &'static str) -> Result<WatchedAccount> {
    watched_derived(role, &pda.address, &pda.bytes, address_binding)
}

fn watched_derived(
    role: String,
    address: &str,
    bytes: &[u8; 32],
    address_binding: &'static str,
) -> Result<WatchedAccount> {
    if clutch_sbf_harness::base58_of(bytes) != address {
        return Err(format!("role {role} has a PDA byte/address mismatch").into());
    }
    decode::role_schema(&role).map_err(|error| format!("role {role}: {error}"))?;
    Ok(WatchedAccount {
        role,
        address: address.to_string(),
        address_binding,
        token_binding: None,
    })
}

fn watched_token(
    role: String,
    pda: &Pda,
    address_binding: &'static str,
    mint: [u8; 32],
    authority: [u8; 32],
) -> Result<WatchedAccount> {
    let mut watched = watched_pda(role, pda, address_binding)?;
    let role_schema = decode::role_schema(&watched.role)
        .map_err(|error| format!("role {}: {error}", watched.role))?;
    if role_schema.owner != decode::OwnerClass::Token2022 {
        return Err(format!("role {} is not a Token-2022 role", watched.role).into());
    }
    watched.token_binding = Some(TokenBinding { mint, authority });
    Ok(watched)
}

fn validate_envelope(
    watched: &WatchedAccount,
    envelope: &rpc::AccountEnvelope,
    program_owner: &str,
    token_owner: &str,
) -> Result<ValidatedEnvelope> {
    let role = &watched.role;
    let role_schema = decode::role_schema(role).map_err(|error| format!("role {role}: {error}"))?;
    let expected_owner = match role_schema.owner {
        decode::OwnerClass::ProtocolProgram => program_owner,
        decode::OwnerClass::Token2022 => token_owner,
    };
    if envelope.owner != expected_owner {
        return Err(format!(
            "role {role} owner {} does not match expected {} owner {expected_owner}",
            envelope.owner,
            role_schema.owner.label(),
        )
        .into());
    }
    if envelope.executable {
        return Err(format!("state role {role} is unexpectedly executable").into());
    }
    let decoded = decode::verified_by_role(role, &envelope.data)
        .map_err(|error| format!("role {role}: {error}"))?;
    if let Some(binding) = watched.token_binding {
        if envelope.data.get(..32) != Some(binding.mint.as_slice()) {
            return Err(format!(
                "role {role} token mint does not match the Friday collateral mint"
            )
            .into());
        }
        if envelope.data.get(32..64) != Some(binding.authority.as_slice()) {
            return Err(
                format!("role {role} token authority does not match its expected bearer").into(),
            );
        }
    }
    Ok(ValidatedEnvelope {
        role_schema,
        decoded,
    })
}

fn prepare_snapshot(
    live: &Live,
    watched: &[WatchedAccount],
    snapshot: &rpc::GraphSnapshotV2,
    family: &str,
    accepted: bool,
    program_owner: &str,
    token_owner: &str,
) -> Result<PreparedSnapshot> {
    let mut next_latest = live.latest.clone();
    let mut states = Vec::with_capacity(watched.len());
    for entry in watched {
        let observed = snapshot
            .accounts
            .get(&entry.address)
            .ok_or_else(|| format!("snapshot omitted watched role {}", entry.role))?;
        let Some(envelope) = observed else {
            let was_present = live.latest.contains_key(&entry.role);
            replace_latest(&mut next_latest, &entry.role, None);
            states.push(json!({
                "type": "state",
                "snapshot_schema": GRAPH_SNAPSHOT_V2_SCHEMA,
                "context_slot": integer::u64_value(snapshot.context_slot),
                "ordinal": live.ordinal,
                "role": entry.role,
                "address": entry.address,
                "address_binding": entry.address_binding,
                "present": false,
                "decoded": Value::Null,
                "validation": "explicitly absent in the shared-context batch; any prior projection is removed only if the complete snapshot is admitted",
            }));
            if was_present || role_is_mandatory(live, &entry.role, family, accepted) {
                return Err(format!(
                    "{} role {} is absent at snapshot context slot {}",
                    if was_present {
                        "previously present"
                    } else {
                        "mandatory"
                    },
                    entry.role,
                    snapshot.context_slot
                )
                .into());
            }
            continue;
        };
        let validated = validate_envelope(entry, envelope, program_owner, token_owner)?;
        states.push(json!({
            "type": "state",
            "snapshot_schema": GRAPH_SNAPSHOT_V2_SCHEMA,
            "context_slot": integer::u64_value(snapshot.context_slot),
            "ordinal": live.ordinal,
            "role": entry.role,
            "address": entry.address,
            "address_binding": entry.address_binding,
            "present": true,
            "owner": envelope.owner,
            "owner_class": validated.role_schema.owner.label(),
            "executable": envelope.executable,
            "lamports": integer::u64_value(envelope.lamports),
            "bytes": integer::u64_value(u64::try_from(envelope.data.len())?),
            "account_schema": validated.decoded.schema,
            "decoded": validated.decoded.value,
            "validation": "expected derived address + expected owner + non-executable + exact schema + role-specific token mint/authority join + shared batch context + stable graph root",
        }));
        replace_latest(&mut next_latest, &entry.role, Some(envelope.clone()));
    }
    Ok(PreparedSnapshot {
        states,
        next_latest,
    })
}

fn replace_latest(
    latest: &mut BTreeMap<String, rpc::AccountEnvelope>,
    role: &str,
    account: Option<rpc::AccountEnvelope>,
) {
    if let Some(account) = account {
        latest.insert(role.to_string(), account);
    } else {
        latest.remove(role);
    }
}

fn role_is_mandatory(live: &Live, role: &str, family: &str, accepted: bool) -> bool {
    match role {
        "friday.market" | "friday.hoard" | "friday.supply" | "friday.hoard-token"
        | "human.collateral" | "bot.collateral" => live.ordinal >= 1,
        "human.position" => live.ordinal >= 2,
        "bot.position" => live.ordinal >= 3,
        "friday.epoch" | "friday.window" => live.ordinal >= 6,
        "friday.page" => live.ordinal >= 7,
        "friday.pot" => {
            matches!(live.phase, "cleared" | "settled") || (accepted && family == "Entitlement")
        }
        _ if role.starts_with("friday.reservation-") => accepted && family == "PlaceOrder",
        _ => false,
    }
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
            .ok_or("confirmation carries no unsigned slot")?;
        let accepted = error.is_null();
        self.publish(&json!({
            "type": "step",
            "ordinal": ordinal,
            "name": name,
            "family": family,
            "state": if accepted { "accepted" } else { "refused" },
            "confirmation": status.get("confirmationStatus").and_then(Value::as_str)
                .unwrap_or("unknown"),
            "slot": integer::u64_value(slot),
            "cu": integer::optional_u64(rpc::compute_units(&self.url, &signature)),
            "bytes": signed.len(),
            "signature": signature,
            "error": error,
            "refusal_code": rpc::custom_error_code(&error)
                .map(|code| format!("Custom({code:#06x})")),
        }));
        self.sweep(live, family, accepted)?;
        if accepted {
            Ok(json!({
                "ok": true,
                "integer_transport": integer::TRANSPORT,
                "ordinal": ordinal,
                "signature": signature,
                "slot": integer::u64_value(slot)
            }))
        } else {
            Ok(json!({
                "ok": false,
                "integer_transport": integer::TRANSPORT,
                "ordinal": ordinal, "signature": signature,
                "detail": format!("the bank refused {name}: {error}"),
            }))
        }
    }

    /// Reload every account this session watches as one graph-bracketed V2 snapshot.
    #[allow(clippy::too_many_lines)] // one fail-closed snapshot validation boundary
    fn sweep(&self, live: &mut Live, family: &str, accepted: bool) -> Result<()> {
        let f = &self.friday;
        let mut watched = vec![
            watched_pda("friday.market".into(), &f.market, "canonical protocol PDA")?,
            watched_pda("friday.hoard".into(), &f.hoard, "canonical protocol PDA")?,
            watched_pda("friday.supply".into(), &f.supply, "canonical protocol PDA")?,
            watched_token(
                "friday.hoard-token".into(),
                &f.hoard_token,
                "canonical protocol PDA owned by Token-2022",
                f.shared.collateral_mint.bytes,
                f.hoard_authority.bytes,
            )?,
            watched_pda("friday.epoch".into(), &f.epoch, "canonical protocol PDA")?,
            watched_pda("friday.window".into(), &f.window, "canonical protocol PDA")?,
            watched_pda("friday.page".into(), &f.page, "canonical protocol PDA")?,
            watched_pda("friday.pot".into(), &f.pot, "canonical protocol PDA")?,
        ];
        for actor in &f.actors {
            watched.push(watched_pda(
                format!("{}.position", actor.role),
                &actor.position,
                "canonical protocol PDA",
            )?);
            watched.push(watched_token(
                format!("{}.collateral", actor.role),
                &actor.token,
                "fixture-derived Token-2022 account address",
                f.shared.collateral_mint.bytes,
                actor.key,
            )?);
        }
        for order in &live.orders {
            watched.push(watched_derived(
                format!("friday.reservation-{}", order.rank),
                &order.reservation_address,
                &order.reservation,
                "canonical reservation PDA",
            )?);
        }
        let root = f.market.address.clone();
        let children: Vec<String> = watched
            .iter()
            .filter(|entry| entry.address != root)
            .map(|entry| entry.address.clone())
            .collect();
        let snapshot = rpc::graph_snapshot_v2(&self.url, &root, &children)?;
        let program_owner = &f.shared.program.address;
        let token_owner = clutch_sbf_harness::base58_of(&f.shared.token_program);
        let prepared = prepare_snapshot(
            live,
            &watched,
            &snapshot,
            family,
            accepted,
            program_owner,
            &token_owner,
        )?;
        let account_count = u64::try_from(prepared.states.len())?;
        let snapshot_event = json!({
            "type": "account-snapshot-v2",
            "schema": GRAPH_SNAPSHOT_V2_SCHEMA,
            "ordinal": live.ordinal,
            "context_slot": integer::u64_value(snapshot.context_slot),
            "attempts": integer::u64_value(u64::try_from(snapshot.attempts)?),
            "root_role": "friday.market",
            "root_address": snapshot.root,
            "account_count": integer::u64_value(account_count),
            "states": prepared.states,
            "consistency": "child accounts share one getMultipleAccounts context; an unchanged complete Market envelope brackets that batch",
            "boundary": "the bracket proves only an unchanged root envelope, not whole-graph immutability; release ProgramData and ELF identity are not authenticated by this schema",
        });
        live.latest = prepared.next_latest;
        self.publish(&snapshot_event);
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
            let Some(account) = live.latest.get(&role) else {
                pending.push(role);
                continue;
            };
            let Ok(position) = PositionAccount::decode(&account.data) else {
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
                "cash": integer::u64_value(position.cash_atoms),
                "reserved": integer::u64_value(position.reserved_cash_atoms),
                "eggs": integer::u64_values(position.internal[..8].iter().copied()),
            }));
        }
        for (role, account) in &live.latest {
            if !role.contains("reservation-") {
                continue;
            }
            let Ok(reservation) = ReservationAccount::decode(&account.data) else {
                continue;
            };
            for (index, held) in eggs.iter_mut().enumerate() {
                *held += reservation.remaining_internal[index];
            }
        }
        let supply = live
            .latest
            .get("friday.supply")
            .and_then(|account| SupplyLedgerAccount::decode(&account.data).ok());
        let locked = live
            .latest
            .get("friday.hoard")
            .and_then(|account| clutch_solana_layout::HoardAccount::decode(&account.data).ok())
            .map(|hoard| hoard.collateral_atoms);
        let custody = live
            .latest
            .get("friday.hoard-token")
            .and_then(|account| account.data.get(64..72))
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
            "cash_total": integer::u64_value(cash),
            "reserved_total": integer::u64_value(reserved),
            "eggs": integer::u64_values(eggs),
            "locked": integer::optional_u64(locked),
            "custody": integer::optional_u64(custody),
            "endowed_total": integer::u64_value(live.endowed_total),
            "split_total": integer::u64_value(live.split_total),
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
                    "rank": integer::u64_value(order.rank),
                    "owner": order.owner_role, "kind": order.kind,
                    "outcome": order.outcome,
                    "side": if order.side == 0 { "buy" } else { "sell" },
                    "quantity": integer::u64_value(order.quantity),
                    "limit": integer::u64_value(order.limit),
                    "retired": order.retired,
                })
            })
            .collect();
        json!({
            "type": "session",
            "integer_transport": integer::TRANSPORT,
            "phase": live.phase,
            "epoch_index": integer::u64_value(EPOCH_INDEX),
            "freeze_deadline_slot": integer::u64_value(live.freeze_deadline),
            "next_rank": integer::u64_value(live.next_rank),
            "human_belief": live.human_belief.as_ref().map(|values| {
                integer::u64_values(values.iter().copied())
            }),
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
        let reservation_pda = self.friday.reservation(Hash32::from_bytes(owner), id);
        let reservation = reservation_pda.bytes;
        live.orders.push(Placed {
            rank,
            owner_role,
            owner,
            id,
            reservation,
            reservation_address: reservation_pda.address,
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

    /// Add collateral: the person deposits more atoms into pooled custody.
    ///
    /// The founding sequence already endowed both actors so there is a market
    /// to trade; this is the same transition, on demand, for when a person
    /// wants more room than the session opened them with.
    pub fn endow(&self, amount: u64) -> Result<Value> {
        let mut live = self.lock();
        if live.phase != "open" {
            return Ok(refused(&format!(
                "funding is admitted while the book is open; it is {}",
                live.phase
            )));
        }
        if amount == 0 {
            return Ok(refused("an endowment of nothing moves nothing"));
        }
        let owner = self.friday.actor("human").ok_or("no human actor")?.key;
        let sequence = live.sequences.get(&owner).copied().unwrap_or(0);
        let outcome = self.submit(
            &mut live,
            &format!("friday-endow-{amount}"),
            "Funding",
            &builders::endow(&self.friday, owner, sequence, amount),
        )?;
        if outcome.get("ok").and_then(Value::as_bool) == Some(true) {
            live.sequences.insert(owner, sequence + 1);
            live.endowed_total += amount;
            // Re-publish: the strip's `endowed_total` moved, so the identities
            // it renders are about a different total than a tick ago.
            self.publish(&self.conservation(&live));
        }
        Ok(outcome)
    }

    /// Lock complete sets: the person turns cash into one Egg on every active
    /// outcome, which is what a sell order's envelope is drawn from.
    pub fn split(&self, quantity: u64) -> Result<Value> {
        let mut live = self.lock();
        if live.phase != "open" {
            return Ok(refused(&format!(
                "funding is admitted while the book is open; it is {}",
                live.phase
            )));
        }
        if quantity == 0 {
            return Ok(refused("a split of nothing locks nothing"));
        }
        let owner = self.friday.actor("human").ok_or("no human actor")?.key;
        let sequence = live.sequences.get(&owner).copied().unwrap_or(0);
        let outcome = self.submit(
            &mut live,
            &format!("friday-split-{quantity}"),
            "Funding",
            &builders::split(&self.friday, owner, sequence, quantity),
        )?;
        if outcome.get("ok").and_then(Value::as_bool) == Some(true) {
            live.sequences.insert(owner, sequence + 1);
            live.split_total += quantity;
            self.publish(&self.conservation(&live));
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
        let reservation_pda = self.friday.reservation(Hash32::from_bytes(human), id);
        let reservation = reservation_pda.bytes;
        live.orders.push(Placed {
            rank,
            owner_role: "human",
            owner: human,
            id,
            reservation,
            reservation_address: reservation_pda.address,
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
        /* One proposal slot per knot of the basis; the automaton's quoted
         * vector is what defines how many knots there are. */
        let proposals: Vec<Value> = (0..self.bot.quoted.len())
            .filter_map(|index| {
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
                /* Invert the automaton's own book-former: I take the side its
                 * resting quote leaves open, at my own value, when my value
                 * crosses its limit.
                 *
                 * The comparison is against the limit actually on the page,
                 * not against what the automaton would quote today.  They are
                 * the same number for an opening quote and this bench never
                 * lets them diverge — but a preview of what crosses must read
                 * the book, or it is a preview of something else. */
                let theirs = &resting.limit;
                let side = match (resting.side, mine.cmp(theirs)) {
                    (1, std::cmp::Ordering::Greater) => 0,
                    (0, std::cmp::Ordering::Less) => 1,
                    _ => return None,
                };
                Some(json!({
                    "outcome": index,
                    "side": if side == 0 { "buy" } else { "sell" },
                    "quantity": integer::u64_value(self.bot.size),
                    "limit": integer::u64_value(mine),
                    "crosses_rank": integer::u64_value(resting.rank),
                    "their_limit": integer::u64_value(*theirs),
                }))
            })
            .collect();
        json!({
            "ok": true,
            "integer_transport": integer::TRANSPORT,
            "label": "MODEL-ONLY",
            "note": "nothing here has been submitted; the bank has committed none of it",
            "belief": integer::u64_values(belief),
            "ladder_step": integer::u64_value(LADDER_STEP),
            "proposed": proposals,
        })
    }

    /// Place every order the painter proposed, and remember the belief the
    /// auto-crank will use as one candidate-coordinate input.
    pub fn paint(&self, weights: &[u64]) -> Result<Value> {
        let proposal = self.propose(weights);
        if proposal.get("ok").and_then(Value::as_bool) != Some(true) {
            return Ok(proposal);
        }
        let belief = integer::field_u64_values(&proposal, "belief")?;
        {
            let mut live = self.lock();
            live.human_belief = Some(belief.clone());
        }
        self.publish(&json!({
            "type": "belief",
            "label": "MODEL-ONLY",
            "belief": integer::u64_values(belief.iter().copied()),
            "proposed": proposal["proposed"].clone(),
        }));
        let mut placed = Vec::new();
        let mut skipped = Vec::new();
        for entry in proposal["proposed"].as_array().unwrap_or(&Vec::new()) {
            let outcome = u8::try_from(integer::field_u64(entry, "outcome")?)?;
            let side = u8::from(entry["side"].as_str() == Some("sell"));
            let quantity = integer::field_u64(entry, "quantity")?;
            let limit = integer::field_u64(entry, "limit")?;
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
            "ok": true,
            "integer_transport": integer::TRANSPORT,
            "belief": integer::u64_values(belief),
            "placed": placed,
            "skipped": skipped,
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
                    "type": "clock",
                    "slot": integer::u64_value(now),
                    "target": integer::u64_value(target),
                    "reason": reason,
                    "remaining": integer::u64_value(target.saturating_sub(now)),
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
            .map(|account| account.data.clone())
            .ok_or("the frozen epoch was never reloaded")?;
        let page_bytes = live
            .latest
            .get("friday.page")
            .map(|account| account.data.clone())
            .ok_or("the frozen page was never reloaded")?;
        let epoch = EpochAccount::decode(&epoch_bytes)
            .map_err(|error| format!("the frozen epoch does not decode: {error:?}"))?;
        let projection = project(&page_bytes)?;
        let book = &projection.book;
        let reservations: Vec<[u8; 32]> = projection
            .identities
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

        let (prices, basis) = self.candidate_plan(&live, &domain, book)?;
        self.stage(&format!(
            "the pre-submit candidate plan is {prices:?}, derived from {basis}"
        ));
        let candidate = canonical_candidate(&domain, book, &prices, 0, 0)
            .map_err(|error| format!("no canonical candidate for the candidate plan: {error:?}"))?;
        let witness = canonical_pairing(&domain, book, &candidate)
            .map_err(|error| format!("the candidate has no canonical pairing: {error:?}"))?;
        let settlement_plan = classify_direct_settlement(&SettlementProjection {
            epoch_page_count: epoch.page_count,
            epoch_order_count: epoch.order_count,
            outcome_count: epoch.outcome_count,
            price_scale: epoch.price_scale,
            page: &projection.header,
            book,
            identities: &projection.identities,
            candidate: &candidate,
            witness: &witness,
        })
        .map_err(|error| error.to_string())?;
        let groups = settlement_plan.groups();
        let fills = candidate.fills[..book.len as usize].to_vec();
        self.publish(&json!({
            "type": "candidate-plan",
            "schema": "dragons-clutch/operator/candidate-plan/v1",
            "prices": integer::u64_values(
                prices[..usize::from(OUTCOMES)].iter().copied()
            ),
            "price_basis": basis,
            "fills": integer::u64_values(fills.iter().copied()),
            "slices": witness.len,
            "virtual_split": integer::u64_value(candidate.virtual_split),
            "virtual_merge": integer::u64_value(candidate.virtual_merge),
            "label": "MODEL-ONLY PRE-SUBMIT CANDIDATE PLAN",
            "boundary": "constructed before SubmitCandidate; not bank-accepted, verified, selected, or cleared",
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
            &builders::complete_clear_work(f, id, &[]),
        )?;

        let target = live.freeze_slot + CANDIDATE_WINDOW_SLOTS;
        self.stage(&format!(
            "candidate verified; waiting for the selection window to close at slot {target}"
        ));
        let mut tick = |now: u64, target: u64, reason: &str| {
            self.publish(&json!({
                "type": "clock",
                "slot": integer::u64_value(now),
                "target": integer::u64_value(target),
                "reason": reason,
                "remaining": integer::u64_value(target.saturating_sub(now)),
            }));
        };
        rpc::wait_for_slot(&self.url, target, "candidate window", &mut tick)?;
        self.stage(
            "selection window closed; finalizing the best candidate verified by the deadline",
        );
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
        self.stage(&format!(
            "entitling {} pairing group(s) of the selected candidate",
            groups.len()
        ));
        let mut legs = Vec::new();
        for group in groups {
            let buy = reservations
                .get(usize::from(group.buy()))
                .copied()
                .ok_or("a slice names an order outside the frozen book")?;
            let sell_side = reservations
                .get(usize::from(group.sell()))
                .copied()
                .ok_or("a slice names an order outside the frozen book")?;
            self.expect(
                &mut live,
                &format!("friday-entitle-slice-{}", group.slice()),
                "Entitlement",
                &builders::entitle_slice(
                    f,
                    &builders::Entitle {
                        candidate: id,
                        slice_index: group.slice(),
                        buy_reservation: buy,
                        sell_reservation: sell_side,
                        pair_receipts: None,
                    },
                ),
            )?;
            let buyer = self.position_of(&live, group.buy(), &reservations)?;
            let seller = self.position_of(&live, group.sell(), &reservations)?;
            legs.push((buy, sell_side, buyer, seller));
        }

        self.stage("settling every entitled group");
        for (group, (buy, sell_side, buyer, seller)) in groups.iter().zip(&legs) {
            self.expect(
                &mut live,
                &format!("friday-settle-slice-{}", group.slice()),
                "Settlement",
                &builders::settle_page(
                    f,
                    &builders::Settle {
                        candidate: id,
                        /* One-based, as the exhibit's settle loop numbers
                         * them: the request sequence is the settlement's own
                         * ordinal, not the slice index. */
                        sequence: 1 + u64::from(group.slice()),
                        buyer_position: *buyer,
                        seller_position: *seller,
                        buy_reservation: *buy,
                        sell_reservation: *sell_side,
                        slice_index: group.slice(),
                        pair_receipts: None,
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

    /// The pre-submit candidate plan, and how its coordinates were chosen.
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
    fn candidate_plan(
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
                        "type": "candidate-trial",
                        "schema": "dragons-clutch/operator/candidate-trial/v1",
                        "basis": basis, "taken": true,
                        "prices": integer::u64_values(
                            prices[..usize::from(OUTCOMES)].iter().copied()
                        ),
                        "attempts": attempts,
                    }));
                    return Ok((prices, basis));
                }
                Err(error) => {
                    let refusal = format!("{error:?}");
                    self.publish(&json!({
                        "type": "candidate-trial",
                        "schema": "dragons-clutch/operator/candidate-trial/v1",
                        "basis": basis, "taken": false,
                        "prices": integer::u64_values(
                            prices[..usize::from(OUTCOMES)].iter().copied()
                        ),
                        "refusal": refusal,
                    }));
                    attempts.push(format!("{basis}: {refusal}"));
                }
            }
        }
        Err(format!(
            "no stated coordinate constructs a canonical candidate for this book ({})",
            attempts.join("; ")
        )
        .into())
    }

    /// The session's opening banner rows.
    pub fn identity(&self) -> Value {
        let f = &self.friday;
        json!({
            "mode": "trade",
            "integer_transport": integer::TRANSPORT,
            "market": clutch_sbf_harness::hex_encode(&f.market_id.bytes()),
            "market_account": f.market.address,
            "terms": clutch_sbf_harness::hex_encode(&f.terms_value.terms.bytes()),
            "basis_degree": f.terms_value.basis_degree,
            "knot_count": f.terms_value.knot_count,
            "knots_cents": crate::friday::KNOT_CENTS.iter()
                .copied().map(integer::u128_value).collect::<Vec<_>>(),
            "outcome_count": OUTCOMES,
            "price_scale": integer::u64_value(PRICE_SCALE),
            "ladder_step": integer::u64_value(LADDER_STEP),
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
            "integer_transport": integer::TRANSPORT,
            "cents": integer::u64_value(cents),
            "denominator": integer::u64_value(PAYOUT_DENOMINATOR),
            "weights": integer::u64_values(weights),
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
    json!({
        "ok": false,
        "integer_transport": integer::TRANSPORT,
        "detail": detail
    })
}

fn identity(label: &str, observed: u64, expected: u64) -> Value {
    json!({
        "label": label,
        "observed": integer::u64_value(observed),
        "expected": integer::u64_value(expected),
        "ok": observed == expected,
    })
}

/// Project the frozen page into the relation's book.
///
/// The second return is the live orders' identities in walk order, which is
/// the order every clear-walk account list is indexed by: the caller resolves
/// each to the reservation address it already holds, rather than re-deriving
/// a program address it has derived once already.
struct PageProjection {
    header: stream::OrderPageHeader,
    book: BookV1,
    identities: Vec<Hash32>,
}

fn project(page: &[u8]) -> Result<PageProjection> {
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
    Ok(PageProjection {
        header,
        book,
        identities,
    })
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
            "no stated coordinate constructed a candidate for the book: {}",
            refusals.join("; ")
        );
    }

    /// The gate's own book: the automaton's eight opening quotes, the person's
    /// three ticket orders, and the four the painter adds on the knots the
    /// person is not already resting on.
    ///
    /// This is the exact shape `scripts/run_operator_trade.sh` drives, pinned
    /// here so the candidate-construction question is answered in milliseconds
    /// rather than after a ten-minute validator run.
    #[test]
    fn a_stated_coordinate_constructs_the_bench_candidate() {
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
        assert!(witness.len > 0, "the candidate plan must pair something");
        assert_eq!(
            basis, "automaton",
            "the midpoint is priced out by the person's $160 ticket, so the \
             automaton's own belief is the coordinate that constructs the candidate"
        );
    }

    #[test]
    fn snapshot_roles_bind_addresses_owners_executable_bits_and_exact_schemas() {
        let bytes = [7_u8; 32];
        let address = clutch_sbf_harness::base58_of(&bytes);
        let position = watched_derived(
            "human.position".to_string(),
            &address,
            &bytes,
            "test-derived",
        )
        .expect("matching address bytes and known role are admitted");
        assert_eq!(position.address, address);
        assert!(watched_derived(
            "human.position".to_string(),
            "11111111111111111111111111111111",
            &bytes,
            "test-derived",
        )
        .is_err());

        let watched = watched_derived(
            "human.collateral".to_string(),
            &address,
            &bytes,
            "test-derived",
        )
        .unwrap();

        let mut token_data = vec![0_u8; 165];
        token_data[108] = 1;
        let token = rpc::AccountEnvelope {
            data: token_data,
            owner: "token-owner".to_string(),
            executable: false,
            lamports: 1,
        };
        validate_envelope(&watched, &token, "program-owner", "token-owner")
            .expect("exact initialized token account with the expected owner is admitted");

        let mut wrong_owner = token.clone();
        wrong_owner.owner = "attacker".to_string();
        assert!(
            validate_envelope(&watched, &wrong_owner, "program-owner", "token-owner",).is_err()
        );
        let mut executable = token.clone();
        executable.executable = true;
        assert!(validate_envelope(&watched, &executable, "program-owner", "token-owner",).is_err());
        let mut truncated = token;
        truncated.data.pop();
        assert!(validate_envelope(&watched, &truncated, "program-owner", "token-owner",).is_err());
    }

    #[test]
    fn token_roles_bind_the_exact_collateral_mint_and_bearer() {
        let address_bytes = [7_u8; 32];
        let address = clutch_sbf_harness::base58_of(&address_bytes);
        let mut watched = watched_derived(
            "human.collateral".to_string(),
            &address,
            &address_bytes,
            "test-derived",
        )
        .unwrap();
        watched.token_binding = Some(TokenBinding {
            mint: [11; 32],
            authority: [12; 32],
        });
        let mut data = clutch_sbf_harness::token_account_bytes([11; 32], [12; 32], 99);
        let envelope = rpc::AccountEnvelope {
            data: data.clone(),
            owner: "token-owner".to_string(),
            executable: false,
            lamports: 1,
        };
        validate_envelope(&watched, &envelope, "program-owner", "token-owner").unwrap();

        data[0] ^= 1;
        let wrong_mint = rpc::AccountEnvelope {
            data,
            ..envelope.clone()
        };
        assert!(validate_envelope(&watched, &wrong_mint, "program-owner", "token-owner").is_err());
        let mut wrong_authority = envelope.clone();
        wrong_authority.data[32] ^= 1;
        assert!(
            validate_envelope(&watched, &wrong_authority, "program-owner", "token-owner").is_err()
        );
    }

    #[test]
    fn an_explicit_absence_removes_a_prior_live_projection() {
        let role = "human.collateral";
        let account = rpc::AccountEnvelope {
            data: vec![1],
            owner: "token-owner".to_string(),
            executable: false,
            lamports: 1,
        };
        let mut latest = BTreeMap::new();
        replace_latest(&mut latest, role, Some(account));
        assert!(latest.contains_key(role));
        replace_latest(&mut latest, role, None);
        assert!(!latest.contains_key(role));
    }

    #[test]
    fn a_late_snapshot_refusal_preserves_the_entire_prior_projection() {
        fn watched(role: &str, address: &str) -> WatchedAccount {
            WatchedAccount {
                role: role.to_string(),
                address: address.to_string(),
                address_binding: "test-derived",
                token_binding: Some(TokenBinding {
                    mint: [11; 32],
                    authority: [12; 32],
                }),
            }
        }
        fn token() -> rpc::AccountEnvelope {
            rpc::AccountEnvelope {
                data: clutch_sbf_harness::token_account_bytes([11; 32], [12; 32], 99),
                owner: "token-owner".to_string(),
                executable: false,
                lamports: 1,
            }
        }

        let first = watched("first.collateral", "first-address");
        let later = watched("later.collateral", "later-address");
        let previous = token();
        let mut latest = BTreeMap::new();
        latest.insert(later.role.clone(), previous.clone());
        let live = Live {
            phase: "open",
            ordinal: 0,
            next_rank: 1,
            sequences: BTreeMap::new(),
            orders: Vec::new(),
            human_belief: None,
            freeze_deadline: 0,
            freeze_slot: 0,
            endowed_total: 0,
            split_total: 0,
            latest,
        };
        let snapshot = rpc::GraphSnapshotV2 {
            context_slot: 10,
            attempts: 1,
            root: first.address.clone(),
            accounts: BTreeMap::from([
                (first.address.clone(), Some(token())),
                (later.address.clone(), None),
            ]),
        };

        assert!(prepare_snapshot(
            &live,
            &[first, later],
            &snapshot,
            "test",
            false,
            "program-owner",
            "token-owner",
        )
        .is_err());
        assert_eq!(live.latest.len(), 1);
        assert_eq!(live.latest["later.collateral"], previous);
        assert!(!live.latest.contains_key("first.collateral"));
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
