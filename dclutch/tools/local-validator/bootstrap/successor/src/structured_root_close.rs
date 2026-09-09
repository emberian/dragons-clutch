//! Finalized execution and exact readback for the Structured capability-root close.
//!
//! Campaign evidence is used only to locate the founded Market records and
//! funding ledgers. The selected release derives every family record, and the
//! operator reauthenticates the complete account graph before constructing the
//! only instruction this module submits.

use std::collections::BTreeMap;

use dclutch_market::{
    capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    capability_program::{CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1},
    realm::REALM_SCHEMA_RELEASE_ID_V1,
};
use dclutch_operator::{
    structured_selected_release_v1::{StructuredPublicationRecordV1, StructuredSelectedReleaseV1},
    terminal_retirement_v1::{
        DirectNativeCloseReportV1, NativeCapabilityCloseSnapshotV1, build_structured_root_close_v1,
        preflight_structured_root_close_caller_v1,
    },
    wallet_terminal_payout::wire::FinalizedSnapshotV1,
};
use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_versioned_message_operator::ObservedAccount;
use serde_json::json;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result,
    campaign::CampaignTerminalEvidenceV1,
    model::{SuccessorPlan, TransactionEvidence},
    plan::{hex, pubkey},
    rpc::{Rpc, RpcAccount},
    terminal_lifecycle::{
        authenticate_campaign_market_v1, finalized_snapshot, required_account, routed_record,
    },
    wallet_terminal::snapshot_from_rpc,
};

#[derive(Clone, Copy)]
struct RecordPairV1 {
    raw: Pubkey,
    staging: Pubkey,
}

/// Close the selected Structured root through Core and prove the exact native
/// poststate from a single finalized readback.
pub(crate) fn close_structured_capability_root_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    campaign: &CampaignTerminalEvidenceV1,
    selected_release: &StructuredSelectedReleaseV1,
    market: Pubkey,
    root: Pubkey,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<serde_json::Value> {
    authenticate_campaign_market_v1(campaign, market)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let realm = routed_record(
        campaign,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let manifest = routed_record(
        campaign,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let publications = selected_release
        .publication_records()
        .map_err(|error| Error::new(format!("Structured root-close publication: {error:?}")))?;
    let program_set = selected_record_v1(&publications, registry, "program-set")?;
    let config = selected_record_v1(&publications, registry, "config")?;
    let close_profile = selected_record_v1(&publications, registry, "root-close-account-profile")?;
    let close_effect = selected_record_v1(&publications, registry, "root-close-effect")?;
    let close_descriptor = selected_record_v1(&publications, registry, "root-close-descriptor")?;

    let selected_funding = campaign_pubkey_v1(campaign, "direct_trading_funding_ledger")?;
    let dependency_funding = resolution_funding_v1(campaign, selected_funding)?;
    let rent_credit = campaign_pubkey_v1(campaign, "founding_lifecycle_rent_credit")?;
    let coordinates = CloseCoordinatesV1 {
        market,
        realm: RecordPairV1 {
            raw: realm.raw,
            staging: realm.staging,
        },
        manifest: RecordPairV1 {
            raw: manifest.raw,
            staging: manifest.staging,
        },
        selected_funding,
        dependency_funding,
        root,
        activation_cache: pubkey(&plan.activation)?,
        core_program: pubkey(&plan.core.program_id)?,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        trading_program: pubkey(&plan.trading.program_id)?,
        trading_programdata: pubkey(&plan.trading.programdata_id)?,
        resolution_program: pubkey(&plan.resolution.program_id)?,
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
        registry_program: registry,
        program_set,
        config,
        close_profile,
        close_effect,
        close_descriptor,
        rent_program: pubkey(&plan.rent_credit.program_id)?,
        rent_credit,
    };

    // Discovery deliberately omits the request-bound caller. The root itself
    // supplies the selected manifest position used to order the two physical
    // ledgers; neither the campaign report nor this shell authors that fact.
    let discovery = finalized_snapshot(rpc, &coordinates.discovery_keys())?;
    let entry_index = structured_entry_index_v1(&discovery, root)?;
    let discovery_snapshot = close_snapshot_v1(&discovery, coordinates, entry_index, None)?;
    let preflight =
        preflight_structured_root_close_caller_v1(&discovery_snapshot).map_err(|error| {
            Error::new(format!("Structured root-close caller preflight: {error:?}"))
        })?;

    // Fetch the caller together with every preflight coordinate. The builder
    // therefore consumes one complete finalized graph after caller discovery,
    // and independently repeats every release, deployment, funding and root
    // authentication the preflight performed.
    let mut full_keys = coordinates.discovery_keys();
    full_keys.push(preflight.caller_authority);
    let full = finalized_snapshot_at_least_v1(rpc, &full_keys, discovery.observation.slot)?;
    let full_entry_index = structured_entry_index_v1(&full, root)?;
    let full_snapshot = close_snapshot_v1(
        &full,
        coordinates,
        full_entry_index,
        Some(preflight.caller_authority),
    )?;
    let report = build_structured_root_close_v1(&full_snapshot)
        .map_err(|error| Error::new(format!("Structured root-close build: {error:?}")))?;
    if report.caller_authority != preflight.caller_authority
        || report.role_request_digest != preflight.request_digest
        || report.closed_root != root
    {
        return Err(Error::new(
            "Structured root-close full snapshot changed its preflight identity or selected root",
        ));
    }

    let label = "close Structured capability root";
    let mut routing_transactions = Vec::new();
    let (routing_observation, tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        label,
        std::slice::from_ref(&report.instruction),
        &mut routing_transactions,
    )?;
    transactions.extend(routing_transactions);
    let sent = rpc.send_v0(
        label,
        std::slice::from_ref(&report.instruction),
        payer,
        routing_observation,
        &tables,
    )?;
    let sent_slot = sent.slot;
    let signature = sent.signature.clone();
    if let Some(error) = sent.error.as_ref() {
        return Err(Error::new(format!(
            "Structured capability root close refused on chain: {error}"
        )));
    }
    transactions.push(sent);

    authenticate_close_poststate_v1(rpc, &report, market, sent_slot)?;
    Ok(json!({
        "rootClosed": true,
        "signature": signature,
        "slot": sent_slot,
        "market": market.to_string(),
        "root": root.to_string(),
        "closedFundingLedger": report.closed_funding_ledger.to_string(),
        "preservedDependencyFundingLedgers": report
            .preserved_dependency_ledgers
            .iter()
            .map(|ledger| ledger.key.to_string())
            .collect::<Vec<_>>(),
        "rentCredit": report.rent_credit.to_string(),
        "rootRefundLamports": report.root_refund_lamports,
        "fundingRefundLamports": report.funding_refund_lamports,
        "rentCreditDeltaLamports": report.rent_credit_delta_lamports,
        "requestDigest": hex(&report.role_request_digest),
        "prestateSlot": report.observation.slot,
    }))
}

#[derive(Clone, Copy)]
struct CloseCoordinatesV1 {
    market: Pubkey,
    realm: RecordPairV1,
    manifest: RecordPairV1,
    selected_funding: Pubkey,
    dependency_funding: Pubkey,
    root: Pubkey,
    activation_cache: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
    trading_program: Pubkey,
    trading_programdata: Pubkey,
    resolution_program: Pubkey,
    resolution_programdata: Pubkey,
    registry_program: Pubkey,
    program_set: RecordPairV1,
    config: RecordPairV1,
    close_profile: RecordPairV1,
    close_effect: RecordPairV1,
    close_descriptor: RecordPairV1,
    rent_program: Pubkey,
    rent_credit: Pubkey,
}

impl CloseCoordinatesV1 {
    fn discovery_keys(self) -> Vec<Pubkey> {
        vec![
            self.market,
            self.realm.raw,
            self.realm.staging,
            self.manifest.raw,
            self.manifest.staging,
            self.selected_funding,
            self.dependency_funding,
            self.root,
            self.activation_cache,
            self.core_program,
            self.core_programdata,
            self.trading_program,
            self.trading_programdata,
            self.resolution_program,
            self.resolution_programdata,
            self.registry_program,
            sysvar::rent::ID,
            self.program_set.raw,
            self.program_set.staging,
            self.config.raw,
            self.config.staging,
            self.close_profile.raw,
            self.close_profile.staging,
            self.close_effect.raw,
            self.close_effect.staging,
            system_program::ID,
            self.close_descriptor.raw,
            self.close_descriptor.staging,
            self.rent_program,
            self.rent_credit,
        ]
    }
}

fn close_snapshot_v1(
    snapshot: &FinalizedSnapshotV1,
    coordinates: CloseCoordinatesV1,
    entry_index: u16,
    caller_authority: Option<Pubkey>,
) -> Result<NativeCapabilityCloseSnapshotV1> {
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("Structured root-close {label}: {error}")))
            .cloned()
    };
    let funding_ledgers = crate::market::ordered_funding_ledger_slice_v1(
        entry_index,
        (coordinates.selected_funding, "selected funding ledger"),
        (coordinates.dependency_funding, "dependency funding ledger"),
    )
    .into_iter()
    .map(|(key, label)| account(key, label))
    .collect::<Result<Vec<_>>>()?;
    Ok(NativeCapabilityCloseSnapshotV1 {
        market: account(coordinates.market, "Market")?,
        realm: account(coordinates.realm.raw, "Realm")?,
        realm_staging: account(coordinates.realm.staging, "Realm staging")?,
        manifest: account(coordinates.manifest.raw, "manifest")?,
        manifest_staging: account(coordinates.manifest.staging, "manifest staging")?,
        funding_ledgers,
        root: account(coordinates.root, "root")?,
        activation_cache: account(coordinates.activation_cache, "activation cache")?,
        core_program: account(coordinates.core_program, "Core program")?,
        core_programdata: account(coordinates.core_programdata, "Core ProgramData")?,
        trading_program: account(coordinates.trading_program, "Trading program")?,
        trading_programdata: account(coordinates.trading_programdata, "Trading ProgramData")?,
        resolution_program: account(coordinates.resolution_program, "Resolution program")?,
        resolution_programdata: account(
            coordinates.resolution_programdata,
            "Resolution ProgramData",
        )?,
        registry_program: account(coordinates.registry_program, "Registry program")?,
        rent_sysvar: account(sysvar::rent::ID, "Rent sysvar")?,
        caller_authority: caller_authority
            .map(|key| account(key, "request-bound caller"))
            .transpose()?,
        program_set: account(coordinates.program_set.raw, "ProgramSet")?,
        program_set_staging: account(coordinates.program_set.staging, "ProgramSet staging")?,
        config: account(coordinates.config.raw, "config")?,
        config_staging: account(coordinates.config.staging, "config staging")?,
        close_profile: account(coordinates.close_profile.raw, "close AccountProfile")?,
        close_profile_staging: account(
            coordinates.close_profile.staging,
            "close AccountProfile staging",
        )?,
        close_effect: account(coordinates.close_effect.raw, "close Effect")?,
        close_effect_staging: account(coordinates.close_effect.staging, "close Effect staging")?,
        system_program: account(system_program::ID, "System Program")?,
        close_descriptor: account(coordinates.close_descriptor.raw, "close descriptor")?,
        close_descriptor_staging: account(
            coordinates.close_descriptor.staging,
            "close descriptor staging",
        )?,
        rent_program: account(coordinates.rent_program, "Rent program")?,
        rent_credit: account(coordinates.rent_credit, "RentCredit")?,
    })
}

fn selected_record_v1(
    records: &[StructuredPublicationRecordV1<'_>],
    registry: Pubkey,
    label: &str,
) -> Result<RecordPairV1> {
    let record = records
        .iter()
        .find(|record| record.label == label)
        .ok_or_else(|| Error::new(format!("Structured selected release omitted {label}")))?;
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(record.schema)
            .map_err(|error| Error::new(format!("Structured {label} schema: {error:?}")))?,
        ContentDigest::new(record.content_id())
            .map_err(|error| Error::new(format!("Structured {label} content: {error:?}")))?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
        .0
    };
    Ok(RecordPairV1 {
        raw: derive(key.raw_record_pda_seeds()),
        staging: derive(key.staging_cursor_pda_seeds()),
    })
}

fn campaign_pubkey_v1(campaign: &CampaignTerminalEvidenceV1, label: &str) -> Result<Pubkey> {
    pubkey(&required_account(campaign, label)?.address)
}

fn resolution_funding_v1(
    campaign: &CampaignTerminalEvidenceV1,
    selected: Pubkey,
) -> Result<Pubkey> {
    let candidates = [
        "resolution_funding_ledger",
        "founding_funding_ledger_v2_0",
        "founding_funding_ledger_v2_1",
    ];
    for label in candidates {
        if let Some(row) = campaign.accounts.get(label) {
            let candidate = pubkey(&row.address)?;
            if candidate != selected {
                return Ok(candidate);
            }
        }
    }
    Err(Error::new(
        "Structured campaign report omitted the distinct Resolution funding ledger",
    ))
}

fn structured_entry_index_v1(snapshot: &FinalizedSnapshotV1, root: Pubkey) -> Result<u16> {
    let root = snapshot.required(root, "Structured capability root")?;
    let header = CapabilityRootHeaderV1::decode(
        root.data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or_else(|| Error::new("Structured capability root is truncated"))?,
    )
    .map_err(|error| Error::new(format!("Structured capability root header: {error:?}")))?;
    Ok(header.selection().entry_index())
}

fn finalized_snapshot_at_least_v1(
    rpc: &mut Rpc,
    keys: &[Pubkey],
    minimum_slot: u64,
) -> Result<FinalizedSnapshotV1> {
    let mut keys = keys.to_vec();
    keys.sort_unstable();
    keys.dedup();
    let (slot, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
    snapshot_from_rpc(slot, rpc.block_time(slot)?, &keys, values)
}

fn authenticate_close_poststate_v1(
    rpc: &mut Rpc,
    report: &DirectNativeCloseReportV1,
    market: Pubkey,
    minimum_slot: u64,
) -> Result<()> {
    let mut keys = vec![
        report.closed_root,
        report.closed_funding_ledger,
        market,
        report.rent_credit,
    ];
    keys.extend(
        report
            .preserved_dependency_ledgers
            .iter()
            .map(|ledger| ledger.key),
    );
    keys.sort_unstable();
    keys.dedup();
    let (slot, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
    if slot < minimum_slot || values.len() != keys.len() {
        return Err(Error::new(
            "Structured root-close poststate was not one complete finalized snapshot",
        ));
    }
    let accounts = keys
        .iter()
        .copied()
        .zip(values)
        .collect::<BTreeMap<Pubkey, Option<RpcAccount>>>();
    if accounts
        .get(&report.closed_root)
        .is_none_or(Option::is_some)
        || accounts
            .get(&report.closed_funding_ledger)
            .is_none_or(Option::is_some)
    {
        return Err(Error::new(
            "Structured root-close left the root or selected FundingLedger allocated",
        ));
    }
    require_exact_live_v1(
        &accounts,
        market,
        report.expected_market_owner,
        report.expected_market_lamports,
        false,
        &report.expected_market_data,
        "Market",
    )?;
    require_exact_live_v1(
        &accounts,
        report.rent_credit,
        report.expected_rent_credit_owner,
        report.expected_rent_credit_lamports,
        false,
        &report.expected_rent_credit_data,
        "RentCredit",
    )?;
    for dependency in &report.preserved_dependency_ledgers {
        require_exact_live_v1(
            &accounts,
            dependency.key,
            dependency.owner,
            dependency.lamports,
            dependency.executable,
            &dependency.data,
            "Resolution dependency FundingLedger",
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn require_exact_live_v1(
    accounts: &BTreeMap<Pubkey, Option<RpcAccount>>,
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    data: &[u8],
    label: &str,
) -> Result<()> {
    let account = accounts
        .get(&key)
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::new(format!("Structured root-close poststate omitted {label}")))?;
    if account.owner != owner
        || account.lamports != lamports
        || account.executable != executable
        || account.data != data
    {
        return Err(Error::new(format!(
            "Structured root-close {label} owner, lamports, executable bit, or bytes changed"
        )));
    }
    Ok(())
}
