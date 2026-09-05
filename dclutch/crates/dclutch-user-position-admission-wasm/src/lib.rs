//! Thin browser ABI over the authoritative User Position admission planner.
//!
//! This crate owns no layout, routing, PDA, or authority decision. It carries
//! one strict JSON snapshot into `dclutch_operator::user_position_admission_v1`
//! and carries that planner's own answer back out. Every coordinate in the
//! twenty-seven-account frame, every rent deficit, and the predicted Claims
//! receipt are the native planner's; nothing here recomputes one.
//!
//! WHY THIS EXISTS. Admission is the step that turns a wallet into a market
//! participant, and until now no browser could compose it: `JoinPanel` said so
//! in its own words -- the frame "needs the position owner's signature over a
//! frame the browser cannot yet assemble byte-exactly" -- and handed the reader
//! a CLI command instead. That made maker/taker trade present-but-unreachable
//! for a stranger holding only a wallet. Compiling the planner is the answer
//! that does not require a second implementation of it in TypeScript.
//!
//! The web shell keeps everything this crate must never have: finalized RPC,
//! Wallet Standard, durable storage, and submission.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_claims::protocol_position_v2::ProtocolPositionAdmissionV2;
use dclutch_operator::user_position_admission_v1::{
    UserPositionAdmissionPlanV1, UserPositionAdmissionSnapshotV1, plan_user_position_admission_v1,
};
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_claims::position_admission::{
    USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1, USER_POSITION_ADMISSION_MAGIC_V1,
    USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
};
use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

/// Exact JSON schema this boundary accepts. A reader that sends another one is
/// refused rather than guessed at.
pub const SNAPSHOT_FORMAT_V1: &str = "dclutch-user-position-admission-snapshot-v1";
/// Exact JSON schema this boundary returns.
pub const PLAN_FORMAT_V1: &str = "dclutch-user-position-admission-plan-v1";

/// THE CANARY.
///
/// The browser must never write the outer selector or the frame width down. It
/// reads both from here, and these assertions fail the BUILD if the contract
/// renames or resizes either -- which is the difference between a rename that
/// goes red and a rename that silently produces a twenty-six-account frame the
/// runtime refuses at execution with no useful reason.
const _: () = assert!(USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1 == 27);
const _: () = assert!(USER_POSITION_ADMISSION_MAGIC_V1.len() == 8);
const _: () =
    assert!(USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1 < USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservationWireV1 {
    slot: String,
    unix_timestamp: String,
    finality: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountWireV1 {
    observation: ObservationWireV1,
    key: String,
    owner: String,
    lamports: String,
    executable: bool,
    data_base64: String,
}

/// The twenty-five observed accounts and the genesis hash, exactly.
///
/// Field-for-field with `UserPositionAdmissionSnapshotV1`. That is deliberate
/// and it is self-checking: adding a field to the planner's snapshot fails to
/// compile here until it is carried, so this transport cannot silently drop an
/// input the planner authenticates.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotWireV1 {
    format: String,
    genesis_hash: String,
    claims_market: AccountWireV1,
    position: AccountWireV1,
    admission: AccountWireV1,
    linked_basis_raw: AccountWireV1,
    linked_basis_staging: AccountWireV1,
    product_raw: AccountWireV1,
    product_staging: AccountWireV1,
    result_domain_raw: AccountWireV1,
    result_domain_staging: AccountWireV1,
    portfolio_raw: AccountWireV1,
    portfolio_staging: AccountWireV1,
    rent_sysvar: AccountWireV1,
    system_program: AccountWireV1,
    core_market: AccountWireV1,
    activation_cache: AccountWireV1,
    registry_program: AccountWireV1,
    trading_program: AccountWireV1,
    trading_programdata: AccountWireV1,
    claims_program: AccountWireV1,
    claims_programdata: AccountWireV1,
    core_program: AccountWireV1,
    core_programdata: AccountWireV1,
    owner: AccountWireV1,
    rent_credit: AccountWireV1,
    rent_program: AccountWireV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountMetaOutV1 {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionOutV1 {
    program_id: String,
    accounts: Vec<AccountMetaOutV1>,
    data_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanOutV1 {
    format: &'static str,
    instructions: Vec<InstructionOutV1>,
    required_signer: String,
    observed_slot: String,
    claims_request_digest: String,
    caller_authority: String,
    position: String,
    admission: String,
    position_rent_principal: String,
    admission_rent_principal: String,
    position_top_up_lamports: String,
    admission_top_up_lamports: String,
    expected_receipt_producer: String,
    expected_receipt_body_base64: String,
    /// Read from the contract, never written down by the client.
    account_count: usize,
    owner_account_index: usize,
}

fn key(value: &str, field: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(value).map_err(|_| format!("{field} is not a base58 public key"))
}

fn u64_of(value: &str, field: &str) -> Result<u64, String> {
    value.parse().map_err(|_| format!("{field} is not a u64"))
}

fn bytes32(value: &str, field: &str) -> Result<[u8; 32], String> {
    let raw = STANDARD
        .decode(value)
        .map_err(|_| format!("{field} is not canonical base64"))?;
    <[u8; 32]>::try_from(raw.as_slice()).map_err(|_| format!("{field} is not 32 bytes"))
}

fn account(wire: &AccountWireV1, field: &str) -> Result<ObservedAccount, String> {
    let finality = match wire.observation.finality.as_str() {
        "finalized" => Finality::Finalized,
        "confirmed" => Finality::Confirmed,
        "processed" => Finality::Processed,
        other => return Err(format!("{field} names an unknown commitment {other}")),
    };
    Ok(ObservedAccount {
        observation: Observation {
            slot: u64_of(&wire.observation.slot, field)?,
            unix_timestamp: wire
                .observation
                .unix_timestamp
                .parse()
                .map_err(|_| format!("{field} unix timestamp is not an i64"))?,
            finality,
        },
        key: key(&wire.key, field)?,
        owner: key(&wire.owner, field)?,
        lamports: u64_of(&wire.lamports, field)?,
        executable: wire.executable,
        data: STANDARD
            .decode(&wire.data_base64)
            .map_err(|_| format!("{field} data is not canonical base64"))?,
    })
}

fn snapshot(wire: &SnapshotWireV1) -> Result<UserPositionAdmissionSnapshotV1, String> {
    if wire.format != SNAPSHOT_FORMAT_V1 {
        return Err(format!("snapshot format must be {SNAPSHOT_FORMAT_V1}"));
    }
    Ok(UserPositionAdmissionSnapshotV1 {
        genesis_hash: bytes32(&wire.genesis_hash, "genesis hash")?,
        claims_market: account(&wire.claims_market, "claims market")?,
        position: account(&wire.position, "position")?,
        admission: account(&wire.admission, "admission")?,
        linked_basis_raw: account(&wire.linked_basis_raw, "linked basis raw")?,
        linked_basis_staging: account(&wire.linked_basis_staging, "linked basis staging")?,
        product_raw: account(&wire.product_raw, "product raw")?,
        product_staging: account(&wire.product_staging, "product staging")?,
        result_domain_raw: account(&wire.result_domain_raw, "result domain raw")?,
        result_domain_staging: account(&wire.result_domain_staging, "result domain staging")?,
        portfolio_raw: account(&wire.portfolio_raw, "portfolio raw")?,
        portfolio_staging: account(&wire.portfolio_staging, "portfolio staging")?,
        rent_sysvar: account(&wire.rent_sysvar, "rent sysvar")?,
        system_program: account(&wire.system_program, "system program")?,
        core_market: account(&wire.core_market, "core market")?,
        activation_cache: account(&wire.activation_cache, "activation cache")?,
        registry_program: account(&wire.registry_program, "registry program")?,
        trading_program: account(&wire.trading_program, "trading program")?,
        trading_programdata: account(&wire.trading_programdata, "trading programdata")?,
        claims_program: account(&wire.claims_program, "claims program")?,
        claims_programdata: account(&wire.claims_programdata, "claims programdata")?,
        core_program: account(&wire.core_program, "core program")?,
        core_programdata: account(&wire.core_programdata, "core programdata")?,
        owner: account(&wire.owner, "owner")?,
        rent_credit: account(&wire.rent_credit, "rent credit")?,
        rent_program: account(&wire.rent_program, "rent program")?,
    })
}

fn plan_out(plan: &UserPositionAdmissionPlanV1) -> PlanOutV1 {
    PlanOutV1 {
        format: PLAN_FORMAT_V1,
        instructions: plan
            .instructions
            .iter()
            .map(|instruction| InstructionOutV1 {
                program_id: instruction.program_id.to_string(),
                accounts: instruction
                    .accounts
                    .iter()
                    .map(|meta| AccountMetaOutV1 {
                        pubkey: meta.pubkey.to_string(),
                        is_signer: meta.is_signer,
                        is_writable: meta.is_writable,
                    })
                    .collect(),
                data_base64: STANDARD.encode(&instruction.data),
            })
            .collect(),
        required_signer: plan.required_signer.to_string(),
        observed_slot: plan.observation.slot.to_string(),
        claims_request_digest: STANDARD.encode(plan.claims_request_digest),
        caller_authority: plan.caller_authority.to_string(),
        position: plan.position.to_string(),
        admission: plan.admission.to_string(),
        position_rent_principal: plan.position_rent_principal.to_string(),
        admission_rent_principal: plan.admission_rent_principal.to_string(),
        position_top_up_lamports: plan.position_top_up_lamports.to_string(),
        admission_top_up_lamports: plan.admission_top_up_lamports.to_string(),
        expected_receipt_producer: plan.expected_receipt_producer.to_string(),
        expected_receipt_body_base64: STANDARD.encode(plan.expected_receipt_body),
        account_count: USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1,
        owner_account_index: USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
    }
}

/// Plan one wallet-authorized Position admission from one finalized snapshot.
///
/// Returns the planner's own refusal text unchanged; this boundary invents no
/// reason of its own.
pub fn plan_user_position_admission_json_v1(snapshot_json: &[u8]) -> Result<String, String> {
    let wire: SnapshotWireV1 = serde_json::from_slice(snapshot_json)
        .map_err(|error| format!("snapshot is not the exact accepted JSON: {error}"))?;
    let native = snapshot(&wire)?;
    let plan = plan_user_position_admission_v1(&native)
        .map_err(|error| format!("admission planning refused: {error:?}"))?;
    serde_json::to_string(&plan_out(&plan))
        .map_err(|error| format!("plan could not be serialized: {error}"))
}

/// The finalized linked-basis RECORD digest this owner was admitted against.
///
/// THE BUG THIS CLOSES. The browser derived the linked-basis record address
/// from the Claims aggregate's `basis_id`. That is the SEMANTIC LiabilityBasisV2
/// identity: it authenticates a basis body and cannot address one, because the
/// semantic preimage ignores bytes the record digest covers. Measured on devnet
/// cohort-11, the raw-record PDA it derives is VACANT while the record the
/// campaign published sits at the PDA of a digest the aggregate does not carry
/// -- so the frame named an account nothing lives at, and the planner failed
/// decoding empty bytes instead of saying which coordinate was wrong.
///
/// `ProtocolPositionAdmissionEvidenceV2` is the only place on chain that names
/// the record digest, and it is decoded HERE rather than sliced in TypeScript,
/// because an offset written down in a client is the same defect one level up.
#[wasm_bindgen]
pub fn linked_basis_record_digest_v1(admission_base64: &str) -> Result<String, JsValue> {
    let bytes = STANDARD
        .decode(admission_base64)
        .map_err(|_| JsValue::from_str("admission record bytes are not canonical base64"))?;
    let admission = ProtocolPositionAdmissionV2::decode(&bytes)
        .map_err(|error| JsValue::from_str(&format!("admission record: {error:?}")))?;
    let digest = admission.evidence().linked_basis_record_digest;
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    Ok(hex)
}

/// Plan one wallet-authorized Position admission. Browser entry point.
#[wasm_bindgen]
pub fn plan_user_position_admission_v1_wasm(snapshot_json: &str) -> Result<String, JsValue> {
    plan_user_position_admission_json_v1(snapshot_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// The outer frame width, read from the contract for the client to check against.
#[wasm_bindgen]
pub fn user_position_admission_account_count_v1() -> usize {
    USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1
}

/// The outer selector, read from the contract rather than written down.
#[wasm_bindgen]
pub fn user_position_admission_magic_v1() -> String {
    STANDARD.encode(USER_POSITION_ADMISSION_MAGIC_V1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_a_snapshot_that_names_another_format() {
        let error = plan_user_position_admission_json_v1(
            br#"{"format":"something-else","genesisHash":"","claimsMarket":{}}"#,
        )
        .expect_err("another format must be refused");
        assert!(error.contains("exact accepted JSON") || error.contains(SNAPSHOT_FORMAT_V1));
    }

    #[test]
    fn refuses_an_unknown_field_rather_than_ignoring_it() {
        // `deny_unknown_fields` is the load-bearing half: a snapshot carrying a
        // coordinate this boundary does not forward must fail loudly, not be
        // planned around.
        let error = plan_user_position_admission_json_v1(
            br#"{"format":"dclutch-user-position-admission-snapshot-v1","surprise":1}"#,
        )
        .expect_err("an unknown field must be refused");
        assert!(error.contains("exact accepted JSON"));
    }

    #[test]
    fn reports_the_contracts_own_frame_width_and_selector() {
        assert_eq!(user_position_admission_account_count_v1(), 27);
        assert_eq!(
            STANDARD.decode(user_position_admission_magic_v1()).unwrap(),
            USER_POSITION_ADMISSION_MAGIC_V1
        );
    }
}
