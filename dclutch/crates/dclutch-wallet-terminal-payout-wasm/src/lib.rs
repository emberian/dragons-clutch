//! Thin browser ABI over the extracted wallet-terminal payout derivation.
//!
//! This crate owns no layout, routing, PDA, or authority decision. It carries
//! strict JSON in, calls `dclutch_wallet_terminal_payout_operator`, and carries
//! that derivation's own answer back out. Every coordinate in the
//! thirty-six-account settlement frame, the lookup table geometry, and the
//! authenticated report are the operator's; nothing here recomputes one.
//!
//! WHY THIS EXISTS. `RedeemFlow` says "This browser never creates or completes
//! a payout plan" and asks the reader to import JSON that
//! `dclutch-local-successor-bootstrap` emits. Redemption is the last of the
//! three capabilities in "the browser can sign but cannot originate", and this
//! is the seam that lets the browser run the SAME derivation rather than grow a
//! second one in TypeScript.
//!
//! The web shell keeps everything this crate must never have: finalized RPC,
//! Wallet Standard, durable storage, and submission.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_claims::terminal_settlement_v3::{
    TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
    TERMINAL_SETTLEMENT_REQUEST_BYTES_V3,
};
use dclutch_wallet_terminal_payout_operator::{
    snapshot_wire::parse_observed_snapshot_v1,
    wire::{INPUT_FORMAT, LookupTableRequirementV1, PlanInputV1, SelectedInputV1, build_manifest},
};
use wasm_bindgen::prelude::*;

/// Exact JSON schema this boundary accepts for one observed snapshot.
pub const SNAPSHOT_FORMAT_V1: &str = "dclutch-wallet-terminal-payout-snapshot-v1";
/// Exact JSON schema this boundary returns for the address list.
pub const ADDRESSES_FORMAT_V1: &str = "dclutch-wallet-terminal-payout-addresses-v1";

/// THE CANARY.
///
/// The browser must never write the settlement frame width, the request width,
/// or the candidate domain down. It reads them from here, and these assertions
/// fail the BUILD if Claims renames or resizes one -- which is the difference
/// between a rename that goes red and a rename that silently produces a
/// thirty-five-account frame the runtime refuses at execution with no useful
/// reason a reader could act on.
const _: () = assert!(TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 == 36);
const _: () = assert!(TERMINAL_SETTLEMENT_REQUEST_BYTES_V3 == 640);
const _: () = assert!(!TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3.is_empty());

fn parse_input(input_json: &str) -> Result<PlanInputV1, String> {
    let input: PlanInputV1 = serde_json::from_str(input_json)
        .map_err(|error| format!("payout input is not the exact accepted JSON: {error}"))?;
    if input.format != INPUT_FORMAT {
        return Err(format!("payout input format must be {INPUT_FORMAT}"));
    }
    Ok(input)
}

/// Every address the derivation will authenticate, in its own order.
///
/// The browser reads exactly this list at one finalized floor. Handing the
/// caller the derivation's own address list -- rather than letting a client
/// assemble one -- is what keeps a second routing implementation from existing.
pub fn wallet_terminal_payout_addresses_json_v1(input_json: &str) -> Result<String, String> {
    let input = parse_input(input_json)?;
    let selected = SelectedInputV1::parse(&input, LookupTableRequirementV1::Present)
        .map_err(|error| format!("payout input refused: {error}"))?;
    let addresses: Vec<String> = selected
        .addresses()
        .into_iter()
        .map(|address| address.to_string())
        .collect();
    serde_json::to_string(&serde_json::json!({
        "format": ADDRESSES_FORMAT_V1,
        "addresses": addresses,
        "accountCount": TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
    }))
    .map_err(|error| format!("payout addresses could not be serialized: {error}"))
}

/// Build the authenticated payout manifest from one finalized snapshot.
///
/// Returns the derivation's own refusal text unchanged; this boundary invents
/// no reason of its own.
pub fn build_wallet_terminal_payout_manifest_json_v1(
    input_json: &str,
    snapshot_json: &str,
) -> Result<String, String> {
    let input = parse_input(input_json)?;
    // ONE DECODER, shared with stage one rather than copied: the format name is
    // this boundary's, the bytes-to-snapshot mapping and the mispairing
    // cross-check belong to the operator crate that owns the snapshot type.
    let snapshot = parse_observed_snapshot_v1(snapshot_json, SNAPSHOT_FORMAT_V1)
        .map_err(|error| error.to_string())?;

    let selected = SelectedInputV1::parse(&input, LookupTableRequirementV1::Present)
        .map_err(|error| format!("payout input refused: {error}"))?;
    let manifest = build_manifest(&selected, &snapshot)
        .map_err(|error| format!("payout manifest refused: {error}"))?;
    serde_json::to_string(&manifest)
        .map_err(|error| format!("payout manifest could not be serialized: {error}"))
}

/// Every address the derivation authenticates. Browser entry point.
#[wasm_bindgen]
pub fn wallet_terminal_payout_addresses_v1(input_json: &str) -> Result<String, JsValue> {
    wallet_terminal_payout_addresses_json_v1(input_json).map_err(|e| JsValue::from_str(&e))
}

/// Build the authenticated payout manifest. Browser entry point.
#[wasm_bindgen]
pub fn build_wallet_terminal_payout_manifest_v1(
    input_json: &str,
    snapshot_json: &str,
) -> Result<String, JsValue> {
    build_wallet_terminal_payout_manifest_json_v1(input_json, snapshot_json)
        .map_err(|e| JsValue::from_str(&e))
}

/// The settlement frame width, read from Claims for the client to check against.
#[wasm_bindgen]
pub fn terminal_settlement_account_count_v3() -> usize {
    TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3
}

/// The settlement request width, read from Claims rather than written down.
#[wasm_bindgen]
pub fn terminal_settlement_request_bytes_v3() -> usize {
    TERMINAL_SETTLEMENT_REQUEST_BYTES_V3
}

/// The candidate domain, read from Claims rather than written down.
#[wasm_bindgen]
pub fn terminal_settlement_candidate_domain_v3() -> String {
    STANDARD.encode(TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_an_input_that_names_another_format() {
        let error = build_wallet_terminal_payout_manifest_json_v1(r#"{"format":"other"}"#, "{}")
            .expect_err("another format must be refused");
        assert!(error.contains("exact accepted JSON") || error.contains(INPUT_FORMAT));
    }

    #[test]
    fn refuses_a_snapshot_whose_keys_and_observations_disagree() {
        // A width that changed between asking and answering is the shape of a
        // snapshot stitched from two reads, and the derivation must never be
        // handed one.
        let input =
            serde_json::to_string(&dclutch_wallet_terminal_payout_operator::wire::tests::input())
                .expect("fixture serializes");
        let snapshot = format!(
            r#"{{"format":"{SNAPSHOT_FORMAT_V1}","slot":"9","unixTimestamp":"1","accounts":[null],"keys":[]}}"#
        );
        let error = build_wallet_terminal_payout_manifest_json_v1(&input, &snapshot)
            .expect_err("a changed width must be refused");
        assert!(error.contains("1 keys") || error.contains("0 keys"));
    }

    #[test]
    fn refuses_an_observation_paired_with_another_address_slot() {
        // The one corruption a snapshot can suffer that still decodes cleanly
        // and still authenticates -- against the wrong account.
        let input =
            serde_json::to_string(&dclutch_wallet_terminal_payout_operator::wire::tests::input())
                .expect("fixture serializes");
        let snapshot = format!(
            r#"{{"format":"{SNAPSHOT_FORMAT_V1}","slot":"9","unixTimestamp":"1","keys":["11111111111111111111111111111112"],"accounts":[{{"key":"11111111111111111111111111111113","owner":"11111111111111111111111111111111","lamports":"1","executable":false,"dataBase64":""}}]}}"#
        );
        let error = build_wallet_terminal_payout_manifest_json_v1(&input, &snapshot)
            .expect_err("a mispaired observation must be refused");
        assert!(error.contains("pairs an observation of"));
    }

    #[test]
    fn reports_the_claims_own_frame_width_and_request_size() {
        assert_eq!(terminal_settlement_account_count_v3(), 36);
        assert_eq!(terminal_settlement_request_bytes_v3(), 640);
        assert_eq!(
            STANDARD
                .decode(terminal_settlement_candidate_domain_v3())
                .unwrap(),
            TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3
        );
    }

    #[test]
    fn hands_back_the_derivations_own_address_list() {
        // The browser reads exactly this and never assembles its own.
        let input =
            serde_json::to_string(&dclutch_wallet_terminal_payout_operator::wire::tests::input())
                .expect("fixture serializes");
        let addresses =
            wallet_terminal_payout_addresses_json_v1(&input).expect("the fixture input routes");
        assert!(addresses.contains(ADDRESSES_FORMAT_V1));
        assert!(addresses.contains("\"accountCount\":36"));
    }
}
