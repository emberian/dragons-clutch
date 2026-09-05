//! Thin browser ABI over the extracted wallet-terminal payout INPUT derivation.
//!
//! This crate owns no layout, routing, PDA, or authority decision. It carries
//! strict JSON in, calls `dclutch_operator::wallet_terminal_input`, and carries
//! that derivation's own answer back out. The two addresses round one observes,
//! the frame round two observes, and the payout input itself are all the
//! operator's; nothing here recomputes one.
//!
//! WHY THIS EXISTS. Stage two — the payout manifest — reached the browser in
//! `eed52c57`, and `RedeemFlow` still says the reader must import the JSON that
//! `dclutch-local-successor-bootstrap wallet-terminal-payout-input` emits.
//! That command was the last one standing between a stranger and a redemption.
//! This is the seam over its three pure phases.
//!
//! THREE PHASES, TWO ROUNDS, AND NO CARRIED STATE. A boundary call cannot hold
//! a Rust value between invocations, so
//! [`build_wallet_terminal_payout_input_json_v1`] takes BOTH rounds and
//! re-derives the frame from round one rather than trusting a frame a client
//! carried back. That is not a cost; it is the property — a client cannot
//! substitute the frame between the round that named it and the round that
//! filled it.
//!
//! The web shell keeps everything this crate must never have: finalized RPC,
//! Wallet Standard, durable storage, and submission.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
};
use dclutch_market::STATE_BYTES;
use dclutch_operator::wallet_terminal_input::{
    ProtocolCoordinatesV1, RoutedRecordV1, TerminalPayoutRequestV1, TerminalRecordRoutingV1,
    TerminalRoutingTableV1,
    address_book::{
        derive_terminal_routing_table_v1, routing_round_one_addresses_v1,
        routing_round_three_addresses_v1, routing_round_two_addresses_v1,
    },
    associated_token_account_program_v1, associated_token_account_v1,
    complete_terminal_payout_input_v1, market_release_set_v1, route_terminal_payout_frame_v1,
};
use dclutch_operator::wallet_terminal_payout::{
    hex32, pubkey, snapshot_wire::parse_observed_snapshot_v1, wire::FinalizedSnapshotV1,
};
use serde::Deserialize;
use solana_program::pubkey::Pubkey;
use wasm_bindgen::prelude::*;

/// Exact JSON schema this boundary accepts for one payout-input request.
pub const REQUEST_FORMAT_V1: &str = "dclutch-wallet-terminal-payout-input-request-v1";
/// Exact JSON schema this boundary accepts for one observed round.
pub const SNAPSHOT_FORMAT_V1: &str = "dclutch-wallet-terminal-payout-input-snapshot-v1";
/// Exact JSON schema this boundary returns for an address list.
pub const ADDRESSES_FORMAT_V1: &str = "dclutch-wallet-terminal-payout-input-addresses-v1";

/// THE CANARY.
///
/// The browser must never write the Core Market width, the Claims aggregate or
/// Position header widths, or the aggregate's seed down. It reads them from
/// here, and these assertions fail the BUILD if an owner renames or resizes
/// one — which is the difference between a rename that goes red and a rename
/// that silently derives a DIFFERENT aggregate address, at which point the
/// derivation authenticates a real account belonging to nothing this reader
/// asked about.
const _: () = assert!(STATE_BYTES == 368);
const _: () = assert!(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 == 256);
const _: () = assert!(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 == 128);
const _: () = assert!(!LIABILITY_BASIS_MARKET_SEED_V2.is_empty());

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProgramsWireV1 {
    registry: String,
    core: String,
    claims: String,
    custody: String,
    resolution: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordWireV1 {
    digest: String,
    address: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordsWireV1 {
    realm: RecordWireV1,
    product: RecordWireV1,
    result_domain: RecordWireV1,
    portfolio: RecordWireV1,
    product_basis: RecordWireV1,
    composition_descriptor: RecordWireV1,
    composition_graph: RecordWireV1,
    composition_translation: RecordWireV1,
    composition_exposure: RecordWireV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RoutingWireV1 {
    founding_market: String,
    collateral_mint: String,
    token_program: String,
    records: RecordsWireV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PayoutWireV1 {
    market: String,
    owner: String,
    /// Absent means "the conventional destination".
    ///
    /// The protocol takes any token account the owner controls, so naming one
    /// is always allowed and always wins. Leaving it out asks the derivation to
    /// fill in the owner's associated token account for this Market's
    /// collateral mint, which it can only do once the address book has named
    /// that mint -- so the default is filled in by
    /// `derive_wallet_terminal_input_request_v1`, beside the book.
    #[serde(default)]
    recipient: Option<String>,
    claim_index: u32,
    /// Absent redeems the whole authenticated balance; the derivation decides
    /// what that is, and this boundary does not.
    #[serde(default)]
    quantity: Option<String>,
}

/// One payout-input request: the six coordinates, the address book, and what
/// the caller is asking for.
///
/// `deny_unknown_fields` is the load-bearing half: a request carrying a
/// coordinate this boundary does not forward must fail loudly rather than be
/// planned around.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestWireV1 {
    format: String,
    programs: ProgramsWireV1,
    /// Absent when the caller holds only a deployment table.
    ///
    /// The release set is the MARKET's choice, not the deployment's, so a
    /// browser reads it out of round one instead of writing one down. A caller
    /// that pins one (the CLI, from its plan) supplies it and keeps the
    /// two-source check.
    #[serde(default)]
    release_set: Option<String>,
    /// Absent while the address book is still being DERIVED.
    ///
    /// A browser has no campaign report, so it starts without one and calls
    /// [`derive_wallet_terminal_input_request_json_v1`], which hands back this
    /// same request with the book filled in. The routing shape is therefore
    /// never written down by a client: it is only ever carried.
    #[serde(default)]
    routing: Option<RoutingWireV1>,
    request: PayoutWireV1,
}

struct ProgramsV1 {
    registry: Pubkey,
    core: Pubkey,
    claims: Pubkey,
    custody: Pubkey,
    resolution: Pubkey,
}

struct DecodedRequestV1 {
    programs: ProgramsV1,
    recipient: Option<Pubkey>,
    release_set: Option<[u8; 32]>,
    routing: Option<TerminalRoutingTableV1>,
    request: TerminalPayoutRequestV1,
}

impl DecodedRequestV1 {
    /// The six coordinates, with the release set resolved against round one.
    ///
    /// A caller that pinned one keeps its two-source check; a caller that
    /// pinned none reads the Market's own, which is the sixth coordinate a
    /// deployment table does not carry.
    fn coordinates(
        &self,
        round_one: &FinalizedSnapshotV1,
    ) -> Result<ProtocolCoordinatesV1, String> {
        let release_set = match self.release_set {
            Some(value) => value,
            None => market_release_set_v1(self.programs.core, self.request.market, round_one)
                .map_err(|error| format!("payout input request refused: {error}"))?,
        };
        Ok(ProtocolCoordinatesV1 {
            registry: self.programs.registry,
            core: self.programs.core,
            claims: self.programs.claims,
            custody: self.programs.custody,
            resolution: self.programs.resolution,
            release_set,
        })
    }

    /// The caller's payout request, or the refusal that names what is missing.
    fn payout_request(&self) -> Result<TerminalPayoutRequestV1, String> {
        let recipient = self.recipient.ok_or_else(|| {
            "this payout input request names no recipient token account; derive one first, which \
             fills in the owner's associated token account for this Market's collateral mint, or \
             name one in the request"
                .to_string()
        })?;
        Ok(TerminalPayoutRequestV1 {
            recipient,
            ..self.request
        })
    }

    /// The address book, or the refusal that names why there is none.
    fn routing(&self) -> Result<&TerminalRoutingTableV1, String> {
        self.routing.as_ref().ok_or_else(|| {
            format!(
                "this payout input request carries no address book; derive one first with the \
                 round-two and round-three observations, or supply `routing` in the exact \
                 {REQUEST_FORMAT_V1}"
            )
        })
    }
}

fn key(value: &str, field: &str) -> Result<Pubkey, String> {
    pubkey(value).map_err(|_| format!("{field} is not a base58 public key"))
}

fn digest(value: &str, field: &str) -> Result<[u8; 32], String> {
    hex32(value).map_err(|_| format!("{field} is not 64 lowercase hex characters"))
}

fn record(wire: &RecordWireV1, field: &str) -> Result<RoutedRecordV1, String> {
    Ok(RoutedRecordV1 {
        digest: digest(&wire.digest, &format!("{field} digest"))?,
        address: key(&wire.address, &format!("{field} address"))?,
    })
}

fn decode_routing(wire: &RoutingWireV1) -> Result<TerminalRoutingTableV1, String> {
    let records = &wire.records;
    Ok(TerminalRoutingTableV1 {
        founding_market: key(&wire.founding_market, "founding Market")?,
        collateral_mint: key(&wire.collateral_mint, "collateral mint")?,
        token_program: key(&wire.token_program, "token program")?,
        records: TerminalRecordRoutingV1 {
            realm: record(&records.realm, "realm record")?,
            product: record(&records.product, "Product record")?,
            result_domain: record(&records.result_domain, "result-domain record")?,
            portfolio: record(&records.portfolio, "portfolio record")?,
            product_basis: record(&records.product_basis, "product-basis record")?,
            composition_descriptor: record(
                &records.composition_descriptor,
                "composition descriptor record",
            )?,
            composition_graph: record(&records.composition_graph, "composition graph record")?,
            composition_translation: record(
                &records.composition_translation,
                "composition translation record",
            )?,
            composition_exposure: record(
                &records.composition_exposure,
                "composition exposure record",
            )?,
        },
    })
}

fn parse_request(request_json: &str) -> Result<DecodedRequestV1, String> {
    let wire: RequestWireV1 = serde_json::from_str(request_json)
        .map_err(|error| format!("payout input request is not the exact accepted JSON: {error}"))?;
    if wire.format != REQUEST_FORMAT_V1 {
        return Err(format!(
            "payout input request format must be {REQUEST_FORMAT_V1}"
        ));
    }
    let routing = match &wire.routing {
        None => None,
        Some(routing) => Some(decode_routing(routing)?),
    };
    Ok(DecodedRequestV1 {
        programs: ProgramsV1 {
            registry: key(&wire.programs.registry, "Registry program")?,
            core: key(&wire.programs.core, "Core program")?,
            claims: key(&wire.programs.claims, "Claims program")?,
            custody: key(&wire.programs.custody, "Custody program")?,
            resolution: key(&wire.programs.resolution, "Resolution program")?,
        },
        release_set: match wire.release_set.as_deref() {
            None => None,
            Some(value) => Some(digest(value, "release set")?),
        },
        routing,
        recipient: match wire.request.recipient.as_deref() {
            None => None,
            Some(value) => Some(key(value, "recipient")?),
        },
        request: TerminalPayoutRequestV1 {
            market: key(&wire.request.market, "Market")?,
            owner: key(&wire.request.owner, "owner")?,
            // Placeholder only: every phase that READS the recipient goes
            // through `payout_request`, which refuses an absent one by name.
            recipient: key(&wire.request.owner, "owner")?,
            claim_index: wire.request.claim_index,
            quantity: match wire.request.quantity.as_deref() {
                None => None,
                Some(value) => Some(
                    value
                        .parse()
                        .map_err(|_| "payout quantity is not a u64".to_string())?,
                ),
            },
        },
    })
}

fn snapshot(snapshot_json: &str, round: &str) -> Result<FinalizedSnapshotV1, String> {
    parse_observed_snapshot_v1(snapshot_json, SNAPSHOT_FORMAT_V1)
        .map_err(|error| format!("{round}: {error}"))
}

fn addresses_json(addresses: &[Pubkey]) -> Result<String, String> {
    let listed: Vec<String> = addresses.iter().map(Pubkey::to_string).collect();
    serde_json::to_string(&serde_json::json!({
        "format": ADDRESSES_FORMAT_V1,
        "addresses": listed,
    }))
    .map_err(|error| format!("payout input addresses could not be serialized: {error}"))
}

/// PHASE ONE — the two addresses round one observes, in the derivation's order.
///
/// The Claims aggregate is a PDA of the Market under the deployment's Claims
/// program, so it is knowable before any read and shares the Market's round.
/// The caller reads exactly this list at one finalized floor; handing back the
/// derivation's own addresses is what keeps a second routing implementation
/// from existing.
pub fn wallet_terminal_input_round_one_addresses_json_v1(
    request_json: &str,
) -> Result<String, String> {
    let decoded = parse_request(request_json)?;
    // Round one needs no address book: both of its addresses come from the
    // deployment's own coordinates and the caller's Market. That is what lets a
    // browser with no campaign report start the sequence at all.
    // A supplied book still gets its Market checked against the caller's,
    // which is the check the CLI has always made before opening a socket.
    if let Some(routing) = decoded.routing.as_ref() {
        if routing.founding_market != decoded.request.market {
            return Err(
                "payout input request refused: terminal Market differed from exact founding campaign evidence"
                    .into(),
            );
        }
    }
    // THREE accounts, not two. The third is the owner's Claims admission
    // record, which is the only place on chain that names the linked-basis
    // record digest; phase one reads the first two and ignores the rest, so one
    // round serves both.
    let keys = routing_round_one_addresses_v1(
        &ProtocolCoordinatesV1 {
            registry: decoded.programs.registry,
            core: decoded.programs.core,
            claims: decoded.programs.claims,
            custody: decoded.programs.custody,
            resolution: decoded.programs.resolution,
            // Round one names no address that depends on the release set.
            release_set: [1; 32],
        },
        decoded.request.market,
        decoded.request.owner,
    )
    .map_err(|error| format!("payout input request refused: {error}"))?;
    addresses_json(&keys)
}

/// PHASE TWO — the frame round two observes, from round one's observations.
pub fn wallet_terminal_input_frame_addresses_json_v1(
    request_json: &str,
    round_one_json: &str,
) -> Result<String, String> {
    let decoded = parse_request(request_json)?;
    let round_one = snapshot(round_one_json, "payout input round one")?;
    let coordinates = decoded.coordinates(&round_one)?;
    let frame = route_terminal_payout_frame_v1(
        &coordinates,
        decoded.routing()?,
        &decoded.payout_request()?,
        &round_one,
    )
    .map_err(|error| format!("payout input frame refused: {error}"))?;
    addresses_json(&frame.addresses())
}

/// PHASE THREE — the exact payout input stage two consumes.
///
/// Takes both rounds and re-derives the frame from round one, so a client
/// cannot substitute the frame between the round that named it and the round
/// that filled it. Returns the derivation's own refusal text unchanged; this
/// boundary invents no reason of its own.
pub fn build_wallet_terminal_payout_input_json_v1(
    request_json: &str,
    round_one_json: &str,
    round_two_json: &str,
) -> Result<String, String> {
    let decoded = parse_request(request_json)?;
    let round_one = snapshot(round_one_json, "payout input round one")?;
    let round_two = snapshot(round_two_json, "payout input round two")?;
    let coordinates = decoded.coordinates(&round_one)?;
    let frame = route_terminal_payout_frame_v1(
        &coordinates,
        decoded.routing()?,
        &decoded.payout_request()?,
        &round_one,
    )
    .map_err(|error| format!("payout input frame refused: {error}"))?;
    let completed =
        complete_terminal_payout_input_v1(&frame, &round_two, &decoded.payout_request()?)
            .map_err(|error| format!("payout input refused: {error}"))?;
    serde_json::to_string(&completed.input)
        .map_err(|error| format!("payout input could not be serialized: {error}"))
}

/// PHASE ZERO, ROUND TWO — the three records round one's own digests address.
///
/// The realm, Product and product-basis records. Every address is a raw-record
/// PDA of a digest the CHAIN published — Core's `realm_id` and `product_record`
/// and the Claims aggregate's `basis_id` — so none of them comes from a
/// document, and a caller with only a deployment table can reach all three.
pub fn wallet_terminal_input_book_round_two_addresses_json_v1(
    request_json: &str,
    round_one_json: &str,
) -> Result<String, String> {
    let decoded = parse_request(request_json)?;
    let round_one = snapshot(round_one_json, "payout input round one")?;
    let coordinates = decoded.coordinates(&round_one)?;
    let keys = routing_round_two_addresses_v1(
        &coordinates,
        decoded.request.market,
        decoded.request.owner,
        &round_one,
    )
    .map_err(|error| format!("payout input address book refused: {error}"))?;
    addresses_json(&keys)
}

/// PHASE ZERO, ROUND THREE — the records round two's BYTES address.
///
/// The result-domain and portfolio digests are inside the Product record and
/// the price-gate digest inside the basis, so this round cannot merge with the
/// one before it. A basis that names no price gate returns two addresses; one
/// that names a certificate returns three, and the BASIS decides which.
pub fn wallet_terminal_input_book_round_three_addresses_json_v1(
    request_json: &str,
    round_one_json: &str,
    round_two_json: &str,
) -> Result<String, String> {
    let decoded = parse_request(request_json)?;
    let round_one = snapshot(round_one_json, "payout input round one")?;
    let round_two = snapshot(round_two_json, "payout input book round two")?;
    let coordinates = decoded.coordinates(&round_one)?;
    let keys = routing_round_three_addresses_v1(
        &coordinates,
        decoded.request.market,
        decoded.request.owner,
        &round_one,
        &round_two,
    )
    .map_err(|error| format!("payout input address book refused: {error}"))?;
    addresses_json(&keys)
}

/// PHASE ZERO — the same request, with its address book DERIVED and filled in.
///
/// Returns the caller's own request rather than a bare table, so the routing
/// shape is never written down by a client: it is only ever carried from here
/// to the phases that consume it. Seven rows come from chain pointers; the four
/// `terminal_composition_*` rows are recompiled by the function the founding
/// published them with, which authenticates the whole product graph on the way.
pub fn derive_wallet_terminal_input_request_json_v1(
    request_json: &str,
    round_one_json: &str,
    round_two_json: &str,
    round_three_json: &str,
) -> Result<String, String> {
    let decoded = parse_request(request_json)?;
    let round_one = snapshot(round_one_json, "payout input round one")?;
    let round_two = snapshot(round_two_json, "payout input book round two")?;
    let round_three = snapshot(round_three_json, "payout input book round three")?;
    let coordinates = decoded.coordinates(&round_one)?;
    let routing = derive_terminal_routing_table_v1(
        &coordinates,
        decoded.request.market,
        decoded.request.owner,
        &round_one,
        &round_two,
        &round_three,
    )
    .map_err(|error| format!("payout input address book refused: {error}"))?;
    let mut wire: serde_json::Value = serde_json::from_str(request_json)
        .map_err(|error| format!("payout input request is not the exact accepted JSON: {error}"))?;
    wire["routing"] = routing_wire_v1(&routing);
    if decoded.recipient.is_none() {
        // THE CONVENTIONAL DESTINATION, filled in only now because only now is
        // the collateral mint known. A caller that named one is untouched.
        wire["request"]["recipient"] = serde_json::json!(
            associated_token_account_v1(
                decoded.request.owner,
                routing.collateral_mint,
                routing.token_program,
            )
            .to_string()
        );
    }
    serde_json::to_string(&wire)
        .map_err(|error| format!("payout input request could not be serialized: {error}"))
}

/// The address book, in exactly the shape [`RoutingWireV1`] accepts.
///
/// The one place this crate WRITES the routing wire rather than reading it. A
/// round-trip test pins the two against each other, because a serializer and a
/// deserializer that drift are two wires wearing one name.
fn routing_wire_v1(routing: &TerminalRoutingTableV1) -> serde_json::Value {
    let row = |routed: &RoutedRecordV1| {
        serde_json::json!({
            "digest": dclutch_operator::wallet_terminal_payout::hex(&routed.digest),
            "address": routed.address.to_string(),
        })
    };
    let records = &routing.records;
    serde_json::json!({
        "foundingMarket": routing.founding_market.to_string(),
        "collateralMint": routing.collateral_mint.to_string(),
        "tokenProgram": routing.token_program.to_string(),
        "records": {
            "realm": row(&records.realm),
            "product": row(&records.product),
            "resultDomain": row(&records.result_domain),
            "portfolio": row(&records.portfolio),
            "productBasis": row(&records.product_basis),
            "compositionDescriptor": row(&records.composition_descriptor),
            "compositionGraph": row(&records.composition_graph),
            "compositionTranslation": row(&records.composition_translation),
            "compositionExposure": row(&records.composition_exposure),
        },
    })
}

/// Every address phase zero's round two observes. Browser entry point.
#[wasm_bindgen]
pub fn wallet_terminal_input_book_round_two_addresses_v1(
    request_json: &str,
    round_one_json: &str,
) -> Result<String, JsValue> {
    wallet_terminal_input_book_round_two_addresses_json_v1(request_json, round_one_json)
        .map_err(|e| JsValue::from_str(&e))
}

/// Every address phase zero's round three observes. Browser entry point.
#[wasm_bindgen]
pub fn wallet_terminal_input_book_round_three_addresses_v1(
    request_json: &str,
    round_one_json: &str,
    round_two_json: &str,
) -> Result<String, JsValue> {
    wallet_terminal_input_book_round_three_addresses_json_v1(
        request_json,
        round_one_json,
        round_two_json,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// The request with its address book derived. Browser entry point.
#[wasm_bindgen]
pub fn derive_wallet_terminal_input_request_v1(
    request_json: &str,
    round_one_json: &str,
    round_two_json: &str,
    round_three_json: &str,
) -> Result<String, JsValue> {
    derive_wallet_terminal_input_request_json_v1(
        request_json,
        round_one_json,
        round_two_json,
        round_three_json,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Every address round one observes. Browser entry point.
#[wasm_bindgen]
pub fn wallet_terminal_input_round_one_addresses_v1(request_json: &str) -> Result<String, JsValue> {
    wallet_terminal_input_round_one_addresses_json_v1(request_json)
        .map_err(|e| JsValue::from_str(&e))
}

/// Every address round two observes. Browser entry point.
#[wasm_bindgen]
pub fn wallet_terminal_input_frame_addresses_v1(
    request_json: &str,
    round_one_json: &str,
) -> Result<String, JsValue> {
    wallet_terminal_input_frame_addresses_json_v1(request_json, round_one_json)
        .map_err(|e| JsValue::from_str(&e))
}

/// Build the exact payout input. Browser entry point.
#[wasm_bindgen]
pub fn build_wallet_terminal_payout_input_v1(
    request_json: &str,
    round_one_json: &str,
    round_two_json: &str,
) -> Result<String, JsValue> {
    build_wallet_terminal_payout_input_json_v1(request_json, round_one_json, round_two_json)
        .map_err(|e| JsValue::from_str(&e))
}

/// The associated-token-account program the default destination derives under.
#[wasm_bindgen]
pub fn associated_token_account_program_id_v1() -> String {
    associated_token_account_program_v1().to_string()
}

/// The Core Market state width, read from its codec for the client to check.
#[wasm_bindgen]
pub fn core_state_bytes_v1() -> usize {
    STATE_BYTES
}

/// The Claims aggregate header width, read from Claims rather than written down.
#[wasm_bindgen]
pub fn liability_basis_market_header_bytes_v2() -> usize {
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2
}

/// The Claims Position header width, read from Claims rather than written down.
#[wasm_bindgen]
pub fn liability_basis_position_header_bytes_v2() -> usize {
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2
}

#[cfg(test)]
mod tests {
    use dclutch_operator::wallet_terminal_payout::{hex, wire::PlanInputV1};

    use super::*;

    fn request_wire(input: &PlanInputV1) -> String {
        let selected = dclutch_operator::wallet_terminal_payout::wire::SelectedInputV1::parse(
            input,
            dclutch_operator::wallet_terminal_payout::wire::LookupTableRequirementV1::Present,
        )
        .expect("fixture routes");
        let record = |digest: &str, address: Pubkey| serde_json::json!({ "digest": digest, "address": address.to_string() });
        serde_json::json!({
            "format": REQUEST_FORMAT_V1,
            "programs": {
                "registry": input.programs.registry,
                "core": input.programs.core,
                "claims": input.programs.claims,
                "custody": input.programs.custody,
                "resolution": input.programs.resolution,
            },
            "releaseSet": input.release_set,
            "routing": {
                "foundingMarket": input.market,
                "collateralMint": input.collateral_mint,
                "tokenProgram": input.token_program,
                "records": {
                    "realm": record(&input.records.realm, selected.realm.raw),
                    "product": record(&input.records.product, selected.product.raw),
                    "resultDomain": record(&input.records.result_domain, selected.result_domain.raw),
                    "portfolio": record(&input.records.portfolio, selected.portfolio.raw),
                    "productBasis": record(&input.records.product_basis, selected.product_basis.raw),
                    "compositionDescriptor": record(&input.records.composition_descriptor, selected.composition_descriptor.raw),
                    "compositionGraph": record(&input.records.composition_graph, selected.composition_graph.raw),
                    "compositionTranslation": record(&input.records.composition_translation, selected.composition_translation.raw),
                    "compositionExposure": record(&input.records.composition_exposure, selected.composition_exposure.raw),
                },
            },
            "request": {
                "market": input.market,
                "owner": input.owner,
                "recipient": input.recipient,
                "claimIndex": input.claim_index,
                "quantity": input.quantity,
            },
        })
        .to_string()
    }

    #[test]
    fn refuses_a_request_that_names_another_format() {
        let error = wallet_terminal_input_round_one_addresses_json_v1(r#"{"format":"other"}"#)
            .expect_err("another format must be refused");
        assert!(error.contains("exact accepted JSON") || error.contains(REQUEST_FORMAT_V1));
    }

    /// The boundary hands back the derivation's own round-one addresses.
    ///
    /// WAS: "round one is two accounts, never three". That was true while the
    /// address book arrived as a document. Deriving it needs a third — the
    /// owner's Claims admission record, the only account on chain that names
    /// the linked-basis RECORD digest — and the assertion moves with the
    /// behaviour rather than the sentence being deleted from under it. It is
    /// still ONE round: all three are addressable before any read.
    #[test]
    fn round_one_hands_back_the_derivations_own_three_addresses() {
        let input = dclutch_operator::wallet_terminal_payout::wire::tests::input();
        let listed = wallet_terminal_input_round_one_addresses_json_v1(&request_wire(&input))
            .expect("the fixture request routes");
        assert!(listed.contains(ADDRESSES_FORMAT_V1));
        let parsed: serde_json::Value = serde_json::from_str(&listed).expect("addresses are JSON");
        let addresses = parsed["addresses"].as_array().expect("address list");
        assert_eq!(
            addresses.len(),
            3,
            "Market, Claims aggregate, and the owner's admission record"
        );
        assert_eq!(addresses[0].as_str().unwrap(), input.market);
        assert_ne!(addresses[1].as_str().unwrap(), input.market);
        assert_ne!(
            addresses[2].as_str().unwrap(),
            addresses[1].as_str().unwrap()
        );
    }

    /// A routing table for another Market is refused before any read.
    #[test]
    fn a_request_whose_address_book_names_another_market_is_refused() {
        let input = dclutch_operator::wallet_terminal_payout::wire::tests::input();
        let mut wire: serde_json::Value =
            serde_json::from_str(&request_wire(&input)).expect("request is JSON");
        wire["routing"]["foundingMarket"] = serde_json::json!(input.recipient);
        let error = wallet_terminal_input_round_one_addresses_json_v1(&wire.to_string())
            .expect_err("a cross-Market address book must refuse");
        assert!(
            error.contains("exact founding campaign evidence"),
            "{error}"
        );
    }

    /// A request carrying a coordinate this boundary does not forward fails
    /// loudly rather than being planned around.
    #[test]
    fn refuses_a_request_carrying_an_unknown_coordinate() {
        let input = dclutch_operator::wallet_terminal_payout::wire::tests::input();
        let mut wire: serde_json::Value =
            serde_json::from_str(&request_wire(&input)).expect("request is JSON");
        wire["lookupTable"] = serde_json::json!(input.market);
        let error = wallet_terminal_input_round_one_addresses_json_v1(&wire.to_string())
            .expect_err("an unknown coordinate must refuse");
        assert!(error.contains("exact accepted JSON"), "{error}");
    }

    /// The shared snapshot decoder still refuses a mispaired observation here.
    #[test]
    fn refuses_an_observation_paired_with_another_address_slot() {
        let input = dclutch_operator::wallet_terminal_payout::wire::tests::input();
        let round_one = format!(
            r#"{{"format":"{SNAPSHOT_FORMAT_V1}","slot":"9","unixTimestamp":"1","keys":["11111111111111111111111111111112"],"accounts":[{{"key":"11111111111111111111111111111113","owner":"11111111111111111111111111111111","lamports":"1","executable":false,"dataBase64":""}}]}}"#
        );
        let error =
            wallet_terminal_input_frame_addresses_json_v1(&request_wire(&input), &round_one)
                .expect_err("a mispaired observation must be refused");
        assert!(error.contains("pairs an observation of"), "{error}");
    }

    /// A round-one snapshot offered as round two is refused by format name.
    #[test]
    fn refuses_a_snapshot_that_names_another_format() {
        let input = dclutch_operator::wallet_terminal_payout::wire::tests::input();
        let error = wallet_terminal_input_frame_addresses_json_v1(
            &request_wire(&input),
            &format!(
                r#"{{"format":"dclutch-wallet-terminal-payout-snapshot-v1","slot":"9","unixTimestamp":"1","keys":[],"accounts":[]}}"#
            ),
        )
        .expect_err("stage two's snapshot format must be refused here");
        assert!(error.contains(SNAPSHOT_FORMAT_V1), "{error}");
    }

    /// A request with no recipient refuses BY NAME rather than planning a
    /// payout into a placeholder.
    #[test]
    fn a_request_with_no_recipient_refuses_before_it_can_route() {
        let input = dclutch_operator::wallet_terminal_payout::wire::tests::input();
        let mut wire: serde_json::Value =
            serde_json::from_str(&request_wire(&input)).expect("request is JSON");
        wire["request"]["recipient"] = serde_json::Value::Null;
        let error = wallet_terminal_input_frame_addresses_json_v1(
            &wire.to_string(),
            &format!(
                r#"{{"format":"{SNAPSHOT_FORMAT_V1}","slot":"9","unixTimestamp":"1","keys":[],"accounts":[]}}"#
            ),
        )
        .expect_err("a request with no destination must refuse");
        assert!(
            error.contains("names no recipient token account"),
            "{error}"
        );
    }

    /// The default destination is the standard convention, derived under the
    /// program that declares it -- not a new addressing rule.
    #[test]
    fn the_default_destination_is_the_conventional_associated_token_account() {
        assert_eq!(
            associated_token_account_program_id_v1(),
            "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
            "the interface crate that declares the program is the only place this id lives"
        );
        let owner = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let token_program = Pubkey::new_unique();
        let derived = associated_token_account_v1(owner, mint, token_program);
        assert_ne!(derived, owner);
        // A different mint is a different destination: the default is per
        // collateral, not per wallet.
        assert_ne!(
            derived,
            associated_token_account_v1(owner, Pubkey::new_unique(), token_program)
        );
    }

    #[test]
    fn reports_the_widths_their_owners_state() {
        assert_eq!(core_state_bytes_v1(), 368);
        assert_eq!(liability_basis_market_header_bytes_v2(), 256);
        assert_eq!(liability_basis_position_header_bytes_v2(), 128);
        // The seed is a magic, not a width, and the browser never writes it
        // down either.
        assert_eq!(hex(LIABILITY_BASIS_MARKET_SEED_V2).len() % 2, 0);
    }
}
