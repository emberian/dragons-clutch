//! Transport for the same General V5 producer used by `dclutch general plan`.
//!
//! This module accepts a bounded RPC observation and returns an unsigned plan.
//! Route grammar, account joins, lifecycle derivation and transaction bytes
//! remain owned by `dclutch_operator::general_successor`.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_operator::general_successor as producer;
use dclutch_versioned_message_operator::{Finality, Observation, ObservedAccount};
use serde::{Deserialize, Serialize};
use solana_hash::Hash;
use solana_program::pubkey::Pubkey;
use std::collections::BTreeSet;
use wasm_bindgen::prelude::*;

/// Provisional transport bound, separate from protocol/account size admission.
/// Lift by streaming the observation corpus while preserving the producer's
/// atomic slot and exact-account-set checks; see `CLIFF_DOCTRINE_V1.md`.
pub const MAX_GENERAL_TRANSPORT_JSON_BYTES_V1: usize = 32 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanInputV1 {
    format: String,
    route: String,
    snapshot_slot: String,
    snapshot_unix_timestamp: String,
    recent_blockhash: String,
    accounts: Vec<AccountV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountV1 {
    address: String,
    owner: String,
    lamports: String,
    executable: bool,
    data_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RouteAccountsV1 {
    format: &'static str,
    minimum_finalized_slot: String,
    addresses: Vec<String>,
}

fn key(value: &str, label: &str) -> Result<Pubkey, String> {
    let key: Pubkey = value.parse().map_err(|error| format!("{label}: {error}"))?;
    if key.to_string() != value {
        return Err(format!("{label} is not canonical base58"));
    }
    Ok(key)
}

fn unsigned(value: &str, label: &str) -> Result<u64, String> {
    let parsed: u64 = value.parse().map_err(|error| format!("{label}: {error}"))?;
    if parsed.to_string() != value {
        return Err(format!("{label} is not canonical u64"));
    }
    Ok(parsed)
}

/// Ask the retained native route parser for its exact finalized account set.
pub fn general_route_accounts_json_v1(route: &str) -> Result<String, String> {
    let route = producer::parse_route_v1(route.as_bytes()).map_err(|error| error.to_string())?;
    let result = RouteAccountsV1 {
        format: "dclutch-general-route-accounts-v1",
        minimum_finalized_slot: route.minimum_finalized_slot().to_string(),
        addresses: route
            .snapshot_addresses()
            .map_err(|error| error.to_string())?
            .iter()
            .map(ToString::to_string)
            .collect(),
    };
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

/// Construct the same V5 document the public CLI writes from observed accounts.
pub fn plan_general_json_v1(source: &str) -> Result<String, String> {
    if source.len() > MAX_GENERAL_TRANSPORT_JSON_BYTES_V1 {
        return Err("General transport input exceeds 32 MiB".to_owned());
    }
    let input: PlanInputV1 =
        serde_json::from_str(source).map_err(|error| format!("General transport JSON: {error}"))?;
    if input.format != "dclutch-general-plan-input-v1" {
        return Err("General transport format differs".to_owned());
    }
    let route =
        producer::parse_route_v1(input.route.as_bytes()).map_err(|error| error.to_string())?;
    let expected = route
        .snapshot_addresses()
        .map_err(|error| error.to_string())?;
    if input.accounts.len() != expected.len() {
        return Err("General observation account count differs from the native route".to_owned());
    }
    let slot = unsigned(&input.snapshot_slot, "snapshotSlot")?;
    if slot < route.minimum_finalized_slot() {
        return Err("General snapshot precedes the route floor".to_owned());
    }
    let unix_timestamp: i64 = input
        .snapshot_unix_timestamp
        .parse()
        .map_err(|error| format!("snapshotUnixTimestamp: {error}"))?;
    if unix_timestamp.to_string() != input.snapshot_unix_timestamp {
        return Err("snapshotUnixTimestamp is not canonical i64".to_owned());
    }
    let recent_blockhash: Hash = input
        .recent_blockhash
        .parse()
        .map_err(|error| format!("recentBlockhash: {error}"))?;
    if recent_blockhash.to_string() != input.recent_blockhash {
        return Err("recentBlockhash is not canonical base58".to_owned());
    }
    let expected: BTreeSet<Pubkey> = expected.into_iter().collect();
    let mut seen = BTreeSet::new();
    let mut accounts = Vec::with_capacity(input.accounts.len());
    for account in input.accounts {
        let address = key(&account.address, "account address")?;
        if !expected.contains(&address) || !seen.insert(address) {
            return Err("General observation substituted or duplicated an account".to_owned());
        }
        let data = BASE64
            .decode(&account.data_base64)
            .map_err(|error| format!("account data: {error}"))?;
        if BASE64.encode(&data) != account.data_base64 {
            return Err("account data is not canonical base64".to_owned());
        }
        accounts.push(ObservedAccount {
            observation: Observation {
                slot,
                unix_timestamp,
                finality: Finality::Finalized,
            },
            key: address,
            owner: key(&account.owner, "account owner")?,
            lamports: unsigned(&account.lamports, "account lamports")?,
            executable: account.executable,
            data,
        });
    }
    let plan = producer::produce_plan_v5(&route, accounts, recent_blockhash)
        .map_err(|error| error.to_string())?;
    let bytes = producer::encode_plan_v5(&plan).map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

/// WASM export for route-directed account acquisition.
#[wasm_bindgen]
pub fn general_route_accounts_v1(route: &str) -> Result<String, JsValue> {
    general_route_accounts_json_v1(route).map_err(|error| JsValue::from_str(&error))
}

/// WASM export for canonical General V5 construction.
#[wasm_bindgen]
pub fn plan_general_v1(source: &str) -> Result<String, JsValue> {
    plan_general_json_v1(source).map_err(|error| JsValue::from_str(&error))
}

#[cfg(test)]
mod tests;
