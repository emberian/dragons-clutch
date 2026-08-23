//! Observed account bytes, read through the frozen layout codecs.
//!
//! The browser is shown decoded state so that it never needs a parser of its
//! own. Program-owned fields below come out of `clutch_solana_layout`, the same
//! codecs the program writes through. The deliberately narrow Token-2022
//! projections admit base accounts/mints plus the Hoard's exact `ImmutableOwner`
//! shape, and read frozen base offsets only after exact owner, size, extension,
//! and initialization checks. Every
//! decoded `u64` crosses the JSON boundary as a canonical decimal string; the
//! static client is still an untrusted projection, but it cannot silently
//! round ledger integers.
//!
//! A known role is admitted only through one centralized schema record: exact
//! byte length, exact layout decoder (which checks tag and version), and the
//! expected owner class. Unknown roles and malformed known roles fail closed;
//! this module never turns them into a plausible-looking opaque record.

use crate::integer;
use clutch_solana_layout::artifact::decode_stage;
use clutch_solana_layout::clearing::{EpochWindowAccount, EPOCH_WINDOW_ACCOUNT_BYTES};
use clutch_solana_layout::reservation::{ReservationAccount, RESERVATION_ACCOUNT_BYTES};
use clutch_solana_layout::{
    account_len, order_id_rank, CandidateRecord, EpochAccount, FinalPotAccount, Hash32,
    HoardAccount, MarketAccount, OrderPageAccount, OrderSlot, PositionAccount, ResolutionAccount,
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
const TOKEN_ACCOUNT_BYTES: usize = 165;
const TOKEN_DELEGATE_OPTION_OFFSET: usize = 72;
const TOKEN_DELEGATE_OFFSET: usize = 76;
const TOKEN_ACCOUNT_STATE_OFFSET: usize = 108;
const TOKEN_NATIVE_OPTION_OFFSET: usize = 109;
const TOKEN_NATIVE_RESERVE_OFFSET: usize = 113;
const TOKEN_DELEGATED_AMOUNT_OFFSET: usize = 121;
const TOKEN_CLOSE_AUTHORITY_OPTION_OFFSET: usize = 129;
const TOKEN_CLOSE_AUTHORITY_OFFSET: usize = 133;
const TOKEN_IMMUTABLE_OWNER_ACCOUNT_BYTES: usize = 170;
const TOKEN_ACCOUNT_TYPE_OFFSET: usize = 165;
const TOKEN_ACCOUNT_TYPE_ACCOUNT: u8 = 2;
const TOKEN_IMMUTABLE_OWNER_EXTENSION: u16 = 7;
const TOKEN_MINT_BYTES: usize = 82;
const TOKEN_MINT_INITIALIZED_OFFSET: usize = 45;

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
        "generation": integer::u64_value(value.generation),
        "cash_atoms": integer::u64_value(value.cash_atoms),
        "reserved_cash_atoms": integer::u64_value(value.reserved_cash_atoms),
        "free_cash_atoms": integer::u64_value(
            value.cash_atoms.saturating_sub(value.reserved_cash_atoms)
        ),
        "internal": integer::u64_values(value.internal[..WIDE].iter().copied()),
        "close_state": value.close_state,
    }))
}

fn hoard(bytes: &[u8]) -> Option<Value> {
    let value = HoardAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "hoard",
        "collateral_atoms": integer::u64_value(value.collateral_atoms),
        "flags": value.flags
    }))
}

fn market(bytes: &[u8]) -> Option<Value> {
    let value = MarketAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "market",
        "outcome_count": value.outcome_count,
        "lifecycle": value.lifecycle,
        "collateral_cap": integer::u64_value(value.collateral_cap),
        "created_slot": integer::u64_value(value.created_slot),
    }))
}

fn supply(bytes: &[u8]) -> Option<Value> {
    let value = SupplyLedgerAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "supply",
        "generation": integer::u64_value(value.generation),
        "internal_supply": integer::u64_values(value.internal_supply[..WIDE].iter().copied()),
        "external_supply": integer::u64_values(value.external_supply[..WIDE].iter().copied()),
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
        "epoch_index": integer::u64_value(value.epoch_index),
        "phase": value.phase,
        "order_count": value.order_count,
        "owner_count": value.owner_count,
        "page_count": value.page_count,
        "price_scale": integer::u64_value(value.price_scale),
        "outcome_count": value.outcome_count,
        "basis_degree": value.basis_degree,
        "relation_version": value.relation_version,
    }))
}

fn window(bytes: &[u8]) -> Option<Value> {
    let value = EpochWindowAccount::decode(bytes).ok()?;
    Some(json!({
        "kind": "window",
        "freeze_deadline_slot": integer::u64_value(value.freeze_deadline_slot),
        "selection_deadline_slot": integer::u64_value(value.selection_deadline_slot),
        "selected_slot": integer::u64_value(value.selected_slot),
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
            "quantity": integer::u64_value(order.quantity),
            "limit": integer::u64_value(order.limit),
            "minimum_fill": integer::u64_value(order.minimum_fill),
            "generation": integer::u64_value(order.generation),
        })),
        OrderSlot::Portfolio(order) => Some(json!({
            "slot": index, "kind": "portfolio", "order_id": order_tag(order.order_id),
            "owner": tag(order.owner), "side": side(order.side),
            "active_len": order.active_len,
            "coefficients": integer::u64_values(
                order.coefficients[..usize::from(order.active_len).min(order.coefficients.len())]
                    .iter().copied()
            ),
            "lots": integer::u64_value(order.lots),
            "limit_collateral_per_lot": integer::u64_value(order.limit_collateral_per_lot),
            "minimum_fill_lots": integer::u64_value(order.minimum_fill_lots),
            "generation": integer::u64_value(order.generation),
        })),
        OrderSlot::Tombstone(retired) => Some(json!({
            "slot": index, "kind": "tombstone", "order_id": order_tag(retired.order_id),
            "owner": tag(retired.owner),
            "retired_generation": integer::u64_value(retired.retired_generation),
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
        "initial_cash_atoms": integer::u64_value(value.initial_cash_atoms),
        "remaining_cash_atoms": integer::u64_value(value.remaining_cash_atoms),
        "initial_internal": integer::u64_values(value.initial_internal[..width].iter().copied()),
        "remaining_internal": integer::u64_values(value.remaining_internal[..width].iter().copied()),
        "entitled_units": integer::u64_value(value.entitled_units),
        "consumed_units": integer::u64_value(value.consumed_units),
    }))
}

fn candidate(bytes: &[u8]) -> Option<Value> {
    let value = CandidateRecord::decode(bytes).ok()?;
    Some(json!({
        "kind": "candidate",
        "status": value.status,
        "prices": integer::u64_values(value.prices[..WIDE].iter().copied()),
        "virtual_split": integer::u64_value(value.virtual_split),
        "virtual_merge": integer::u64_value(value.virtual_merge),
        "distinct_owners": value.distinct_owners,
        "order_len": value.order_len,
        "submitted_slot": integer::u64_value(value.submitted_slot),
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
        "resolved_slot": integer::u64_value(value.resolved_slot),
        "feed_cursor": integer::u64_value(value.feed_cursor),
        "sealed_end_bucket_exclusive": integer::u64_value(value.sealed_end_bucket_exclusive),
        "repair_generation": integer::u64_value(value.repair_generation),
    }))
}

fn token(bytes: &[u8]) -> Option<Value> {
    if bytes.get(TOKEN_ACCOUNT_STATE_OFFSET).copied()? != 1
        || bytes.get(TOKEN_DELEGATE_OPTION_OFFSET..TOKEN_DELEGATE_OFFSET)? != [0; 4]
        || bytes.get(TOKEN_DELEGATE_OFFSET..TOKEN_ACCOUNT_STATE_OFFSET)? != [0; 32]
        || bytes.get(TOKEN_NATIVE_OPTION_OFFSET..TOKEN_NATIVE_RESERVE_OFFSET)? != [0; 4]
        || bytes.get(TOKEN_NATIVE_RESERVE_OFFSET..TOKEN_DELEGATED_AMOUNT_OFFSET)? != [0; 8]
        || u64_at(bytes, TOKEN_DELEGATED_AMOUNT_OFFSET)? != 0
        || bytes.get(TOKEN_CLOSE_AUTHORITY_OPTION_OFFSET..TOKEN_CLOSE_AUTHORITY_OFFSET)? != [0; 4]
        || bytes.get(TOKEN_CLOSE_AUTHORITY_OFFSET..TOKEN_ACCOUNT_BYTES)? != [0; 32]
    {
        return None;
    }
    Some(json!({
        "kind": "token",
        "amount": integer::u64_value(u64_at(bytes, TOKEN_AMOUNT_OFFSET)?)
    }))
}

fn mint(bytes: &[u8]) -> Option<Value> {
    if bytes.get(TOKEN_MINT_INITIALIZED_OFFSET).copied()? != 1 {
        return None;
    }
    Some(json!({
        "kind": "mint",
        "supply": integer::u64_value(u64_at(bytes, MINT_SUPPLY_OFFSET)?)
    }))
}

fn immutable_owner_token(bytes: &[u8]) -> Option<Value> {
    if bytes.get(TOKEN_ACCOUNT_TYPE_OFFSET).copied()? != TOKEN_ACCOUNT_TYPE_ACCOUNT
        || u16::from_le_bytes(bytes.get(166..168)?.try_into().ok()?)
            != TOKEN_IMMUTABLE_OWNER_EXTENSION
        || u16::from_le_bytes(bytes.get(168..170)?.try_into().ok()?) != 0
    {
        return None;
    }
    token(bytes)
}

/// Legacy artifact-stage view used by watch mode only.
fn stage(bytes: &[u8]) -> Option<Value> {
    let value = decode_stage(bytes).ok()?;
    Some(json!({
        "kind": "artifact-stage",
        "cursor": value.cursor,
        "created_slot": integer::u64_value(value.created_slot),
        "expires_slot": integer::u64_value(value.expires_slot),
        "staged_bytes": bytes.len(),
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerClass {
    ProtocolProgram,
    Token2022,
}

impl OwnerClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProtocolProgram => "protocol-program",
            Self::Token2022 => "token-2022",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Codec {
    Position,
    Hoard,
    Market,
    Supply,
    Epoch,
    Window,
    Page,
    Candidate,
    Pot,
    Resolution,
    Reservation,
    Receipt,
    Token,
    TokenImmutableOwner,
    Mint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleSchema {
    codec: Codec,
    pub name: &'static str,
    pub exact_len: usize,
    pub owner: OwnerClass,
    pub tagged: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedDecode {
    pub value: Value,
    pub schema: Value,
}

/// Resolve a daemon role to one exact account schema.
///
/// The role selects which frozen decoder is attempted, but it cannot make
/// arbitrary bytes pass: every program-owned decoder below checks its own
/// exact tag, version, length, reserved bytes, and canonical padding.
/// Token-2022 is intentionally restricted here to extension-free 165-byte
/// actor accounts, the Hoard's exact 170-byte `ImmutableOwner` shape, and
/// extension-free 82-byte mints. Every other extension-bearing account is a
/// client-schema refusal, not a claim that Token-2022 or the protocol cannot
/// support it.
#[allow(clippy::too_many_lines)] // one exhaustive role-to-schema registry
pub fn role_schema(role: &str) -> Result<RoleSchema, String> {
    let tail = role.rsplit('.').next().unwrap_or(role);
    let schema = match tail {
        "position" => RoleSchema {
            codec: Codec::Position,
            name: "position",
            exact_len: account_len::POSITION,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "hoard" => RoleSchema {
            codec: Codec::Hoard,
            name: "hoard",
            exact_len: account_len::HOARD,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "market" => RoleSchema {
            codec: Codec::Market,
            name: "market",
            exact_len: account_len::MARKET,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "supply" => RoleSchema {
            codec: Codec::Supply,
            name: "supply",
            exact_len: account_len::SUPPLY_LEDGER,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "epoch" => RoleSchema {
            codec: Codec::Epoch,
            name: "epoch",
            exact_len: account_len::EPOCH,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "window" => RoleSchema {
            codec: Codec::Window,
            name: "epoch-window",
            exact_len: EPOCH_WINDOW_ACCOUNT_BYTES,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "page" => RoleSchema {
            codec: Codec::Page,
            name: "order-page",
            exact_len: account_len::ORDER_PAGE,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "candidate" => RoleSchema {
            codec: Codec::Candidate,
            name: "candidate",
            exact_len: account_len::CANDIDATE,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "pot" => RoleSchema {
            codec: Codec::Pot,
            name: "final-pot",
            exact_len: account_len::FINAL_POT,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "resolution" => RoleSchema {
            codec: Codec::Resolution,
            name: "resolution",
            exact_len: account_len::RESOLUTION,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        "hoard-token" => RoleSchema {
            codec: Codec::TokenImmutableOwner,
            name: "token-2022-immutable-owner-account",
            exact_len: TOKEN_IMMUTABLE_OWNER_ACCOUNT_BYTES,
            owner: OwnerClass::Token2022,
            tagged: false,
        },
        "collateral" | "actor-collateral" => RoleSchema {
            codec: Codec::Token,
            name: "token-2022-base-account",
            exact_len: TOKEN_ACCOUNT_BYTES,
            owner: OwnerClass::Token2022,
            tagged: false,
        },
        _ if tail.starts_with("reservation-") => RoleSchema {
            codec: Codec::Reservation,
            name: "reservation",
            exact_len: RESERVATION_ACCOUNT_BYTES,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        _ if tail.starts_with("receipt-") => RoleSchema {
            codec: Codec::Receipt,
            name: "settlement-receipt",
            exact_len: account_len::SETTLEMENT_RECEIPT,
            owner: OwnerClass::ProtocolProgram,
            tagged: true,
        },
        _ if tail.starts_with("outcome-mint-") => RoleSchema {
            codec: Codec::Mint,
            name: "token-2022-base-mint",
            exact_len: TOKEN_MINT_BYTES,
            owner: OwnerClass::Token2022,
            tagged: false,
        },
        _ => return Err(format!("role {role} has no admitted account schema")),
    };
    Ok(schema)
}

/// Decode one account only after its centralized role schema admits it.
pub fn verified_by_role(role: &str, bytes: &[u8]) -> Result<VerifiedDecode, String> {
    let schema = role_schema(role)?;
    if bytes.len() != schema.exact_len {
        return Err(format!(
            "role {role} / {} has {} bytes; expected exactly {}",
            schema.name,
            bytes.len(),
            schema.exact_len
        ));
    }
    let decoded = match schema.codec {
        Codec::Position => position(bytes),
        Codec::Hoard => hoard(bytes),
        Codec::Market => market(bytes),
        Codec::Supply => supply(bytes),
        Codec::Epoch => epoch(bytes),
        Codec::Window => window(bytes),
        Codec::Page => page(bytes),
        Codec::Candidate => candidate(bytes),
        Codec::Pot => pot(bytes),
        Codec::Resolution => resolution(bytes),
        Codec::Reservation => reservation(bytes),
        Codec::Receipt => receipt(bytes),
        Codec::Token => token(bytes),
        Codec::TokenImmutableOwner => immutable_owner_token(bytes),
        Codec::Mint => mint(bytes),
    }
    .ok_or_else(|| {
        format!(
            "role {role} / {} failed its exact tag, version, layout, or initialization checks",
            schema.name
        )
    })?;
    let exact_len = u64::try_from(schema.exact_len)
        .map_err(|_| format!("role {role} schema length does not fit u64"))?;
    let account_schema = if schema.tagged {
        json!({
            "name": schema.name,
            "bytes": integer::u64_value(exact_len),
            "tag": bytes[0],
            "version": bytes[1],
        })
    } else {
        json!({
            "name": schema.name,
            "bytes": integer::u64_value(exact_len),
            "tag": Value::Null,
            "version": Value::Null,
        })
    };
    Ok(VerifiedDecode {
        value: decoded,
        schema: account_schema,
    })
}

/// Legacy watch-mode projection for committed-plan roles.
///
/// Watch compares bytes against its committed expectation before calling this
/// renderer, and its inventory includes intentionally opaque work accounts.
/// Trade V2 must use [`verified_by_role`] and never this compatibility view.
pub fn by_role(role: &str, bytes: &[u8]) -> Value {
    if role.rsplit('.').next().unwrap_or(role) == "policy-stage" {
        return stage(bytes).unwrap_or_else(|| {
            json!({"kind": "opaque", "head": short(bytes), "decode_refusal": "legacy policy-stage bytes failed the artifact-stage codec"})
        });
    }
    verified_by_role(role, bytes).map_or_else(
        |error| json!({"kind": "opaque", "head": short(bytes), "decode_refusal": error}),
        |decoded| decoded.value,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_with_no_schema_fails_closed() {
        assert!(role_schema("general.clear-work").is_err());
    }

    #[test]
    fn a_token_amount_is_read_only_from_an_exact_initialized_base_account() {
        let mut bytes = vec![0_u8; TOKEN_ACCOUNT_BYTES];
        bytes[TOKEN_ACCOUNT_STATE_OFFSET] = 1;
        bytes[TOKEN_AMOUNT_OFFSET..TOKEN_AMOUNT_OFFSET + 8].copy_from_slice(&49_u64.to_le_bytes());
        let decoded = verified_by_role("owner-b.collateral", &bytes)
            .expect("exact initialized Token-2022 base account decodes");
        assert_eq!(decoded.value["kind"], "token");
        assert_eq!(decoded.value["amount"], "49");
        assert_eq!(decoded.schema["bytes"], "165");
        assert!(verified_by_role("owner-b.collateral", &bytes[..164]).is_err());
        bytes[TOKEN_ACCOUNT_STATE_OFFSET] = 0;
        assert!(verified_by_role("owner-b.collateral", &bytes).is_err());
    }

    #[test]
    fn token_amounts_above_javascript_precision_remain_distinct() {
        for amount in [1_u64 << 53, (1_u64 << 53) + 1, u64::MAX] {
            let mut bytes = vec![0_u8; TOKEN_ACCOUNT_BYTES];
            bytes[TOKEN_ACCOUNT_STATE_OFFSET] = 1;
            bytes[TOKEN_AMOUNT_OFFSET..TOKEN_AMOUNT_OFFSET + 8]
                .copy_from_slice(&amount.to_le_bytes());
            assert_eq!(
                verified_by_role("owner-b.collateral", &bytes)
                    .expect("exact token account decodes")
                    .value["amount"],
                amount.to_string()
            );
        }
    }

    #[test]
    fn an_order_id_reads_as_its_canonical_rank() {
        assert_eq!(order_tag(clutch_solana_layout::canonical_order_id(3)), "#3");
        assert!(order_tag(Hash32::from_bytes([9_u8; 32])).starts_with("non-canonical"));
    }

    #[test]
    fn a_truncated_position_does_not_decode_into_a_lie() {
        assert!(verified_by_role("owner-b.position", &[0_u8; 4]).is_err());
    }

    #[test]
    fn role_schema_centralizes_owner_and_exact_length() {
        let position = role_schema("owner-b.position").expect("position role is known");
        assert_eq!(position.owner, OwnerClass::ProtocolProgram);
        assert_eq!(position.exact_len, account_len::POSITION);
        assert!(position.tagged);

        let token = role_schema("owner-b.collateral").expect("token role is known");
        assert_eq!(token.owner, OwnerClass::Token2022);
        assert_eq!(token.exact_len, TOKEN_ACCOUNT_BYTES);
        assert!(!token.tagged);
    }

    #[test]
    fn the_hoard_accepts_only_the_exact_immutable_owner_extension() {
        let mut bytes = vec![0_u8; TOKEN_IMMUTABLE_OWNER_ACCOUNT_BYTES];
        bytes[TOKEN_ACCOUNT_STATE_OFFSET] = 1;
        bytes[TOKEN_ACCOUNT_TYPE_OFFSET] = TOKEN_ACCOUNT_TYPE_ACCOUNT;
        bytes[166..168].copy_from_slice(&TOKEN_IMMUTABLE_OWNER_EXTENSION.to_le_bytes());
        assert!(verified_by_role("friday.hoard-token", &bytes).is_ok());

        let mut wrong_extension = bytes.clone();
        wrong_extension[166..168].copy_from_slice(&8_u16.to_le_bytes());
        assert!(verified_by_role("friday.hoard-token", &wrong_extension).is_err());
        assert!(verified_by_role("friday.hoard-token", &bytes[..169]).is_err());
    }

    #[test]
    fn token_accounts_require_the_canonical_unencumbered_base_shape() {
        for offset in [
            TOKEN_DELEGATE_OPTION_OFFSET,
            TOKEN_DELEGATE_OFFSET,
            TOKEN_NATIVE_OPTION_OFFSET,
            TOKEN_NATIVE_RESERVE_OFFSET,
            TOKEN_DELEGATED_AMOUNT_OFFSET,
            TOKEN_CLOSE_AUTHORITY_OPTION_OFFSET,
            TOKEN_CLOSE_AUTHORITY_OFFSET,
        ] {
            let mut bytes = vec![0_u8; TOKEN_ACCOUNT_BYTES];
            bytes[TOKEN_ACCOUNT_STATE_OFFSET] = 1;
            bytes[offset] = 1;
            assert!(verified_by_role("human.collateral", &bytes).is_err());
        }
    }

    #[test]
    fn legacy_watch_mode_keeps_policy_stage_decoding_but_trade_v2_refuses_the_role() {
        use clutch_solana_layout::artifact::{
            initialize_stage, ArtifactBinding, ArtifactKind, ArtifactStageHeader,
        };

        let binding = ArtifactBinding {
            kind: ArtifactKind::CollateralPolicy,
            context: Hash32::from_bytes([1; 32]),
            digest: Hash32::from_bytes([2; 32]),
            exact_len: u16::try_from(ArtifactKind::CollateralPolicy.exact_len()).unwrap(),
        };
        let header = ArtifactStageHeader {
            binding,
            funder: [3; 32],
            cursor: 0,
            created_slot: 7,
            expires_slot: 70,
            stored_bump: 254,
        };
        let mut bytes = vec![0_u8; header.account_len().unwrap()];
        initialize_stage(&mut bytes, &header).unwrap();
        let decoded = by_role("general.policy-stage", &bytes);
        assert_eq!(decoded["kind"], "artifact-stage");
        assert_eq!(decoded["created_slot"], "7");
        assert!(verified_by_role("general.policy-stage", &bytes).is_err());
    }
}
