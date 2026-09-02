//! Thin browser ABI over the extracted wallet-terminal payout INPUT derivation.
//!
//! This crate owns no layout, routing, PDA, or authority decision. It carries
//! strict JSON in, calls `dclutch_wallet_terminal_input_operator`, and carries
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

use dclutch_claims_svm::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
};
use dclutch_market_core_codec::STATE_BYTES;
use dclutch_wallet_terminal_input_operator::{
    ProtocolCoordinatesV1, RoutedRecordV1, TerminalPayoutRequestV1, TerminalRecordRoutingV1,
    TerminalRoutingTableV1, complete_terminal_payout_input_v1, route_terminal_payout_frame_v1,
    terminal_payout_round_one_addresses_v1,
};
use dclutch_wallet_terminal_payout_operator::{
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
    recipient: String,
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
    release_set: String,
    routing: RoutingWireV1,
    request: PayoutWireV1,
}

struct DecodedRequestV1 {
    coordinates: ProtocolCoordinatesV1,
    routing: TerminalRoutingTableV1,
    request: TerminalPayoutRequestV1,
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

fn parse_request(request_json: &str) -> Result<DecodedRequestV1, String> {
    let wire: RequestWireV1 = serde_json::from_str(request_json)
        .map_err(|error| format!("payout input request is not the exact accepted JSON: {error}"))?;
    if wire.format != REQUEST_FORMAT_V1 {
        return Err(format!(
            "payout input request format must be {REQUEST_FORMAT_V1}"
        ));
    }
    let records = &wire.routing.records;
    Ok(DecodedRequestV1 {
        coordinates: ProtocolCoordinatesV1 {
            registry: key(&wire.programs.registry, "Registry program")?,
            core: key(&wire.programs.core, "Core program")?,
            claims: key(&wire.programs.claims, "Claims program")?,
            custody: key(&wire.programs.custody, "Custody program")?,
            resolution: key(&wire.programs.resolution, "Resolution program")?,
            release_set: digest(&wire.release_set, "release set")?,
        },
        routing: TerminalRoutingTableV1 {
            founding_market: key(&wire.routing.founding_market, "founding Market")?,
            collateral_mint: key(&wire.routing.collateral_mint, "collateral mint")?,
            token_program: key(&wire.routing.token_program, "token program")?,
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
        },
        request: TerminalPayoutRequestV1 {
            market: key(&wire.request.market, "Market")?,
            owner: key(&wire.request.owner, "owner")?,
            recipient: key(&wire.request.recipient, "recipient")?,
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
    let keys = terminal_payout_round_one_addresses_v1(
        &decoded.coordinates,
        &decoded.routing,
        &decoded.request,
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
    let frame = route_terminal_payout_frame_v1(
        &decoded.coordinates,
        &decoded.routing,
        &decoded.request,
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
    let frame = route_terminal_payout_frame_v1(
        &decoded.coordinates,
        &decoded.routing,
        &decoded.request,
        &round_one,
    )
    .map_err(|error| format!("payout input frame refused: {error}"))?;
    let completed = complete_terminal_payout_input_v1(&frame, &round_two, &decoded.request)
        .map_err(|error| format!("payout input refused: {error}"))?;
    serde_json::to_string(&completed.input)
        .map_err(|error| format!("payout input could not be serialized: {error}"))
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
    use dclutch_wallet_terminal_payout_operator::{hex, wire::PlanInputV1};

    use super::*;

    fn request_wire(input: &PlanInputV1) -> String {
        let selected = dclutch_wallet_terminal_payout_operator::wire::SelectedInputV1::parse(
            input,
            dclutch_wallet_terminal_payout_operator::wire::LookupTableRequirementV1::Present,
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

    /// The boundary hands back the derivation's own two addresses.
    #[test]
    fn round_one_hands_back_the_derivations_own_two_addresses() {
        let input = dclutch_wallet_terminal_payout_operator::wire::tests::input();
        let listed = wallet_terminal_input_round_one_addresses_json_v1(&request_wire(&input))
            .expect("the fixture request routes");
        assert!(listed.contains(ADDRESSES_FORMAT_V1));
        let parsed: serde_json::Value = serde_json::from_str(&listed).expect("addresses are JSON");
        let addresses = parsed["addresses"].as_array().expect("address list");
        assert_eq!(addresses.len(), 2, "round one is two accounts, never three");
        assert_eq!(addresses[0].as_str().unwrap(), input.market);
        assert_ne!(addresses[1].as_str().unwrap(), input.market);
    }

    /// A routing table for another Market is refused before any read.
    #[test]
    fn a_request_whose_address_book_names_another_market_is_refused() {
        let input = dclutch_wallet_terminal_payout_operator::wire::tests::input();
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
        let input = dclutch_wallet_terminal_payout_operator::wire::tests::input();
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
        let input = dclutch_wallet_terminal_payout_operator::wire::tests::input();
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
        let input = dclutch_wallet_terminal_payout_operator::wire::tests::input();
        let error = wallet_terminal_input_frame_addresses_json_v1(
            &request_wire(&input),
            &format!(
                r#"{{"format":"dclutch-wallet-terminal-payout-snapshot-v1","slot":"9","unixTimestamp":"1","keys":[],"accounts":[]}}"#
            ),
        )
        .expect_err("stage two's snapshot format must be refused here");
        assert!(error.contains(SNAPSHOT_FORMAT_V1), "{error}");
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
