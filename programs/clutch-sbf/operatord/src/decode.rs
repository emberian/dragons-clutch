//! Observed account bytes, read through the frozen layout codecs.
//!
//! The browser is shown decoded state so that it never needs a parser of its
//! own.  Every field below comes out of `clutch_solana_layout`, the same
//! codecs the program writes through — there is no second reading of the wire
//! format anywhere in this project's frontend.
//!
//! A role this daemon has no codec for is reported as `{"kind": "opaque"}`
//! with its length and leading bytes.  That is deliberately a visible gap and
//! not a silent one: an undecoded account must look undecoded.

use clutch_solana_layout::artifact::decode_stage;
use clutch_solana_layout::clearing::EpochWindowAccount;
use clutch_solana_layout::reservation::ReservationAccount;
use clutch_solana_layout::{
    CandidateRecord, EpochAccount, FinalPotAccount, Hash32, HoardAccount, MarketAccount,
    order_id_rank, OrderPageAccount, OrderSlot, PositionAccount, ResolutionAccount,
    SettlementReceiptAccount, SupplyLedgerAccount,
};
use serde_json::{json, Value};

/// How many outcome-indexed entries the bench renders.
///
/// The frozen arrays are `MAX_OUTCOMES` wide and every market this bench can
/// open is narrower.  Two was the general lane's width and reading exactly two
/// made an eight-outcome market render as a two-outcome one; eight is the
/// widest market the bench founds, and a narrower market's tail is zero
/// because the codecs require canonical padding.
const WIDE: usize = 8;

/// Token-2022 base account: `amount` is a little-endian u64 at byte 64.
const TOKEN_AMOUNT_OFFSET: usize = 64;
/// Token-2022 base mint: `supply` is a little-endian u64 at byte 36.
const MINT_SUPPLY_OFFSET: usize = 36;

fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset.checked_add(8)?)?;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}

fn short(bytes: &[u8]) -> String {
    clutch_sbf_harness::hex_encode(&bytes[..bytes.len().min(8)])
}

fn position(bytes: &[u8]) -> Option<Value> {
    let value = PositionAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "position",
        "generation": value.generation,
        "cash_atoms": value.cash_atoms,
        "reserved_cash_atoms": value.reserved_cash_atoms,
        "free_cash_atoms": value.cash_atoms.saturating_sub(value.reserved_cash_atoms),
        "internal": &value.internal[..WIDE],
        "close_state": value.close_state,
    }))
}

fn hoard(bytes: &[u8]) -> Option<Value> {
    let value = HoardAccount::decode(bytes).ok()?;
    Some(json!({"kind": "hoard", "collateral_atoms": value.collateral_atoms, "flags": value.flags}))
}

fn market(bytes: &[u8]) -> Option<Value> {
    let value = MarketAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "market",
        "outcome_count": value.outcome_count,
        "lifecycle": value.lifecycle,
        "collateral_cap": value.collateral_cap,
        "created_slot": value.created_slot,
    }))
}

fn supply(bytes: &[u8]) -> Option<Value> {
    let value = SupplyLedgerAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "supply",
        "generation": value.generation,
        "internal_supply": &value.internal_supply[..WIDE],
        "external_supply": &value.external_supply[..WIDE],
    }))
}

/// The frozen epoch, including the basis width it binds.
///
/// `basis_degree` is stamped by `InitEpoch` from the terms artifact and is
/// what tells a reader whether this epoch is a degree-0 two-outcome lane or a
/// degree-1 eight-knot clutch.  Leaving it and `outcome_count` out of the
/// decode made two very different markets render identically, which is the
/// kind of gap a bench exists to close.
fn epoch(bytes: &[u8]) -> Option<Value> {
    let value = EpochAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "epoch",
        "epoch_index": value.epoch_index,
        "phase": value.phase,
        "order_count": value.order_count,
        "owner_count": value.owner_count,
        "page_count": value.page_count,
        "price_scale": value.price_scale,
        "outcome_count": value.outcome_count,
        "basis_degree": value.basis_degree,
        "relation_version": value.relation_version,
    }))
}

fn window(bytes: &[u8]) -> Option<Value> {
    let value = EpochWindowAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "window",
        "freeze_deadline_slot": value.freeze_deadline_slot,
        "selection_deadline_slot": value.selection_deadline_slot,
        "selected_slot": value.selected_slot,
        "live_order_count": value.live_order_count,
        "retained_count": value.retained_count,
    }))
}

/// `side` is a one-byte discriminator in the frozen record; 0 is the buy end.
fn side(value: u8) -> &'static str {
    if value == 0 {
        "buy"
    } else {
        "sell"
    }
}

/// A 32-byte identity, abbreviated for a table cell.
///
/// The full identity is still available in the raw image; this is a label, and
/// it is spelled short enough that nobody mistakes it for the identity itself.
fn tag(value: Hash32) -> String {
    clutch_sbf_harness::hex_encode(&value.bytes()[..6])
}

/// An order identity, read the one way the layout crate admits reading one.
///
/// A canonical order id is a one-based *rank* in the trailing eight bytes with
/// every leading byte zero, so abbreviating it the way [`tag`] abbreviates an
/// owner prints `000000000000` for every order in the book — structurally
/// true and completely useless.  `order_id_rank` is documented as the only
/// admitted reading, so use it, and say so plainly when an id is not canonical
/// rather than printing a prefix that looks like one.
fn order_tag(value: Hash32) -> String {
    match order_id_rank(value) {
        Ok(rank) => format!("#{rank}"),
        Err(_) => format!("non-canonical {}", tag(value)),
    }
}

/// One occupied slot of an order page, as the book actually holds it.
fn slot(index: usize, entry: OrderSlot) -> Option<Value> {
    match entry {
        OrderSlot::Empty => None,
        OrderSlot::Single(order) => Some(json!({
            "slot": index, "kind": "single", "order_id": order_tag(order.order_id),
            "owner": tag(order.owner), "outcome": order.outcome, "side": side(order.side),
            "quantity": order.quantity, "limit": order.limit,
            "minimum_fill": order.minimum_fill, "generation": order.generation,
        })),
        OrderSlot::Portfolio(order) => Some(json!({
            "slot": index, "kind": "portfolio", "order_id": order_tag(order.order_id),
            "owner": tag(order.owner), "side": side(order.side),
            "active_len": order.active_len,
            "coefficients": &order.coefficients[..usize::from(order.active_len).min(order.coefficients.len())],
            "lots": order.lots, "limit_collateral_per_lot": order.limit_collateral_per_lot,
            "minimum_fill_lots": order.minimum_fill_lots, "generation": order.generation,
        })),
        OrderSlot::Tombstone(retired) => Some(json!({
            "slot": index, "kind": "tombstone", "order_id": order_tag(retired.order_id),
            "owner": tag(retired.owner), "retired_generation": retired.retired_generation,
        })),
    }
}

fn page(bytes: &[u8]) -> Option<Value> {
    let value = OrderPageAccount::decode(bytes).ok()?;
    let orders: Vec<Value> = value
        .orders
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| slot(index, *entry))
        .collect();
    Some(json!({
        "kind": "page",
        "page_index": value.page_index,
        "order_count": value.order_count,
        "tombstone_count": value.tombstone_count,
        "set_order_count": value.set_order_count,
        "frozen": value.frozen,
        "orders": orders,
    }))
}

/// One reservation, including the partial-fill lane's entitlement counters.
///
/// `entitled_units` and `consumed_units` are the v3 fields the entitlement
/// seam stamps and every later slice re-derives; without them a partially
/// consumed order and a fully consumed one look the same in the bench, which
/// is precisely the distinction partial fill introduced.  The Egg envelopes
/// are shown across the whole active width rather than the first two outcomes,
/// because an eight-outcome market has eight of them.
fn reservation(bytes: &[u8]) -> Option<Value> {
    let value = ReservationAccount::decode(bytes).ok()?;
    let width = usize::from(value.outcome_count).min(value.initial_internal.len());
    Some(json!({
        "kind": "reservation",
        "state": value.state,
        "order_id": order_tag(value.order_id),
        "owner": tag(value.owner),
        "side": side(value.side),
        "order_kind": value.order_kind,
        "outcome_count": value.outcome_count,
        "initial_cash_atoms": value.initial_cash_atoms,
        "remaining_cash_atoms": value.remaining_cash_atoms,
        "initial_internal": &value.initial_internal[..width],
        "remaining_internal": &value.remaining_internal[..width],
        "entitled_units": value.entitled_units,
        "consumed_units": value.consumed_units,
    }))
}

fn candidate(bytes: &[u8]) -> Option<Value> {
    let value = CandidateRecord::decode(bytes).ok()?;
    Some(json!({
        "kind": "candidate",
        "status": value.status,
        "prices": &value.prices[..WIDE],
        "virtual_split": value.virtual_split,
        "virtual_merge": value.virtual_merge,
        "distinct_owners": value.distinct_owners,
        "order_len": value.order_len,
        "submitted_slot": value.submitted_slot,
    }))
}

fn receipt(bytes: &[u8]) -> Option<Value> {
    let value = SettlementReceiptAccount::decode(bytes).ok()?;
    Some(json!({"kind": "receipt", "flags": value.flags}))
}

fn pot(bytes: &[u8]) -> Option<Value> {
    let value = FinalPotAccount::decode(bytes).ok()?;
    Some(json!({"kind": "pot", "phase": value.phase}))
}

fn resolution(bytes: &[u8]) -> Option<Value> {
    let value = ResolutionAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "resolution",
        "payout_index": value.payout_index,
        "resolved_slot": value.resolved_slot,
        "feed_cursor": value.feed_cursor,
        "sealed_end_bucket_exclusive": value.sealed_end_bucket_exclusive,
        "repair_generation": value.repair_generation,
    }))
}

/// The artifact transport's staging header: how much of a chunked policy
/// artifact has landed, and when the stage lapses.
fn stage(bytes: &[u8]) -> Option<Value> {
    let value = decode_stage(bytes).ok()?;
    Some(json!({
        "kind": "artifact-stage",
        "cursor": value.cursor,
        "created_slot": value.created_slot,
        "expires_slot": value.expires_slot,
        "staged_bytes": bytes.len(),
    }))
}

fn token(bytes: &[u8]) -> Option<Value> {
    Some(json!({"kind": "token", "amount": u64_at(bytes, TOKEN_AMOUNT_OFFSET)?}))
}

fn mint(bytes: &[u8]) -> Option<Value> {
    Some(json!({"kind": "mint", "supply": u64_at(bytes, MINT_SUPPLY_OFFSET)?}))
}

/// Decode one reloaded account by the plan role it was compared under.
///
/// Roles are the plan's own vocabulary (`owner-b.position`,
/// `general.reservation-3`, `general-market.hoard-token`), so the match is on
/// the role suffix rather than on a guessed discriminator byte.
pub fn by_role(role: &str, bytes: &[u8]) -> Value {
    let tail = role.rsplit('.').next().unwrap_or(role);
    let decoded = match tail {
        "position" => position(bytes),
        "hoard" => hoard(bytes),
        "market" => market(bytes),
        "supply" => supply(bytes),
        "epoch" => epoch(bytes),
        "window" => window(bytes),
        "page" => page(bytes),
        "candidate" => candidate(bytes),
        "pot" => pot(bytes),
        "resolution" => resolution(bytes),
        "policy-stage" => stage(bytes),
        "hoard-token" | "collateral" | "actor-collateral" => token(bytes),
        _ if tail.starts_with("reservation-") => reservation(bytes),
        _ if tail.starts_with("receipt-") => receipt(bytes),
        _ if tail.starts_with("outcome-mint-") => mint(bytes),
        _ => None,
    };
    decoded.unwrap_or_else(|| json!({"kind": "opaque", "head": short(bytes)}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_with_no_codec_is_visibly_opaque() {
        let value = by_role("general.clear-work", &[1, 2, 3, 4]);
        assert_eq!(value["kind"], "opaque");
        assert_eq!(value["head"], "01020304");
    }

    #[test]
    fn a_token_amount_is_read_at_the_frozen_offset() {
        let mut bytes = vec![0_u8; 165];
        bytes[TOKEN_AMOUNT_OFFSET..TOKEN_AMOUNT_OFFSET + 8].copy_from_slice(&49_u64.to_le_bytes());
        let value = by_role("general-market.hoard-token", &bytes);
        assert_eq!(value["kind"], "token");
        assert_eq!(value["amount"], 49);
    }

    #[test]
    fn an_order_id_reads_as_its_canonical_rank() {
        assert_eq!(
            order_tag(clutch_solana_layout::canonical_order_id(3)),
            "#3"
        );
        assert!(order_tag(Hash32::from_bytes([9_u8; 32])).starts_with("non-canonical"));
    }

    #[test]
    fn a_truncated_position_does_not_decode_into_a_lie() {
        assert_eq!(by_role("owner-b.position", &[0_u8; 4])["kind"], "opaque");
    }
}
