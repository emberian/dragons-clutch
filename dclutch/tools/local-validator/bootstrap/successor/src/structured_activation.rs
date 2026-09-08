//! Finalized observation boundary for Structured receipt activation.
//!
//! This module owns no account image and accepts no caller-supplied account
//! coordinates.  It joins the sealed founding report, exact selected-capability
//! market input, and one finalized RPC snapshot before a V6 lifecycle builder
//! may construct `ActivateReceipt`.

use std::path::Path;

use dclutch_claims::{
    liability_basis_state_v2::{LiabilityBasisMarketSeedsV2, LiabilityBasisMarketViewV2},
    rational::RationalReceiptMintSeedsV2,
    rational::{TokenBehaviorRecordAdmissionV2, authenticate_token_behavior_v2},
    rational_kernel::RepresentationDescriptorV2,
    rational_lifecycle::{LifecycleActionV2, LifecycleHeaderV2, LifecycleRequestV2},
};
use dclutch_custody::token_svm::{
    TOKEN_2022_CLOSEABLE_MINT_BYTES_V2, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};
use dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_market::{CoreState, Phase};
use dclutch_operator::{
    ObservedAccount,
    observation::{FinalizedRecordProof, authenticate_finalized_record},
    rational_lifecycle_hot::{
        RationalLifecycleHotInstructionV3, RationalLifecycleHotStateV3,
        RationalLifecycleSelectedBundleV6, RationalLifecycleSelectedSelectionV6,
        build_rational_lifecycle_selected_hot_instruction_v6,
        validate_rational_lifecycle_selected_bundle_v6,
    },
};
use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::system_program;

use crate::{
    Error, Result, campaign,
    cluster::ExpectedClusterV1,
    model::MarketRunInput,
    plan::{hex32, pubkey},
    rpc::{Rpc, RpcAccount, parse_json_without_duplicate_keys_v1},
};

/// The only report account labels receipt activation may consume.
const MARKET_LABEL_V1: &str = "founding_market";
const AGGREGATE_LABEL_V1: &str = "claims_aggregate";
// `StructuredSelectedReleaseV1::publication_records` emits ProgramSet,
// config, then seven artifacts in ProgramSet selector order. Receipt
// activation owns selector six, after the five representation bundles.
const CONFIG_RECORD_INDEX_V1: usize = 1;
const ACTIVATE_RECEIPT_RECORD_START_V1: usize = 2 + 5 * 7;
const ACTIVATION_RECORD_COUNT_V1: usize = 7;

/// One bounded, same-finalized-slot activation observation.
#[derive(Clone, Debug)]
pub(crate) struct StructuredActivateReceiptObservationV1 {
    /// Finalized slot shared by every account below.
    pub(crate) slot: u64,
    /// The Open Market selected by the sealed founding report.
    pub(crate) market: Pubkey,
    /// Current Core Market bytes.
    pub(crate) market_account: RpcAccount,
    /// Claims aggregate named by the sealed founding report.
    pub(crate) aggregate: Pubkey,
    /// Current Claims aggregate bytes.
    pub(crate) aggregate_account: RpcAccount,
}

/// Same-slot physical facts from which the only valid `ActivateReceipt`
/// lifecycle header can be derived.  This is deliberately account-shaped:
/// the host never accepts a caller-authored revision, rent floor, or receipt
/// balance as a substitute for the current chain body.
#[derive(Clone, Copy)]
pub(crate) struct ActivateReceiptHeaderObservationV1<'a> {
    pub(crate) market: Pubkey,
    pub(crate) market_account: &'a RpcAccount,
    pub(crate) claims: Pubkey,
    pub(crate) aggregate: Pubkey,
    pub(crate) aggregate_account: &'a RpcAccount,
    pub(crate) rent_credit: Pubkey,
    pub(crate) rent_credit_account: &'a RpcAccount,
    pub(crate) rent_program: Pubkey,
    /// `None` is the RPC spelling of a System-owned, zero-lamport PDA.
    pub(crate) receipt_mint_account: Option<&'a RpcAccount>,
    pub(crate) rent: &'a solana_program::rent::Rent,
    /// The authenticated Product-terminal width N. It is distinct from the
    /// descriptor's sparse representation width K.
    pub(crate) exposure_product_width: u32,
    pub(crate) descriptor: RepresentationDescriptorV2<'a>,
}

/// Derive the sole canonical header for a receipt activation from physical
/// finalized prestate.  The caller must later place the receipt PDA in the
/// same transaction's first System transfer, with this exact rent floor;
/// Claims then sees the declared prepayment rather than an invented balance.
pub(crate) fn build_activate_receipt_header_v1(
    observed: ActivateReceiptHeaderObservationV1<'_>,
) -> Result<LifecycleHeaderV2> {
    let core = CoreState::decode(&observed.market_account.data)
        .map_err(|error| Error::new(format!("Structured receipt Core Market: {error:?}")))?;
    if observed.market_account.owner == Pubkey::default()
        || core.phase != Phase::Open
        || core.identity.market_id.to_bytes() != observed.market.to_bytes()
        || observed.market_account.owner == observed.claims
    {
        return Err(Error::new(
            "Structured receipt Market prestate is not an open Core Market",
        ));
    }
    let aggregate = LiabilityBasisMarketViewV2::decode(&observed.aggregate_account.data)
        .map_err(|error| Error::new(format!("Structured receipt Claims aggregate: {error:?}")))?;
    let aggregate_seeds = LiabilityBasisMarketSeedsV2::new(observed.market.to_bytes())
        .map_err(|error| Error::new(format!("Structured receipt aggregate seeds: {error:?}")))?;
    let expected_aggregate =
        Pubkey::find_program_address(&aggregate_seeds.as_slices(), &observed.claims).0;
    if observed.aggregate != expected_aggregate
        || observed.aggregate_account.owner != observed.claims
        || aggregate.logical_market != observed.market.to_bytes()
        || aggregate.release_set != core.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != core.identity.registry_program.to_bytes()
        || aggregate.generation != core.identity.generation
        || aggregate.claim_count != observed.exposure_product_width
    {
        return Err(Error::new(
            "Structured receipt Claims aggregate differs from the Core/descriptor join",
        ));
    }
    let credit =
        LifecycleRentCreditV2::decode(&observed.rent_credit_account.data).map_err(|error| {
            Error::new(format!(
                "Structured receipt lifecycle RentCredit: {error:?}"
            ))
        })?;
    if observed.rent_credit_account.owner != observed.rent_program
        || core.rent_beneficiary.to_bytes() != observed.rent_credit.to_bytes()
        || credit.market().to_bytes() != observed.market.to_bytes()
        || credit.release_set().to_bytes() != core.identity.selected_release_set.to_bytes()
        || credit.generation() != core.identity.generation
    {
        return Err(Error::new(
            "Structured receipt lifecycle RentCredit differs from the Core Market",
        ));
    }
    let receipt_seeds = RationalReceiptMintSeedsV2::new(
        observed.descriptor.graph_digest(),
        observed.market.to_bytes(),
        core.identity.selected_release_set.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Structured receipt mint seeds: {error:?}")))?;
    let receipt_mint = Pubkey::find_program_address(&receipt_seeds.as_slices(), &observed.claims).0;
    receipt_seeds
        .authenticate_address(receipt_mint.to_bytes(), observed.descriptor.receipt_mint())
        .map_err(|error| Error::new(format!("Structured receipt descriptor mint: {error:?}")))?;
    let receipt_lamports = match observed.receipt_mint_account {
        Some(account) if account.owner == system_program::ID && account.data.is_empty() => {
            account.lamports
        }
        Some(_) => {
            return Err(Error::new(
                "Structured receipt Mint is neither absent nor a vacant System account",
            ));
        }
        None => 0,
    };
    let receipt_rent = observed
        .rent
        .minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2);
    Ok(LifecycleHeaderV2 {
        action: LifecycleActionV2::ActivateReceipt,
        release_set: core.identity.selected_release_set.to_bytes(),
        market: observed.market.to_bytes(),
        graph_id: observed.descriptor.graph_id(),
        descriptor_id: observed.descriptor.descriptor_id(),
        parent_context: [0; 32],
        representation_authority: observed.descriptor.representation_authority(),
        receipt_mint: receipt_mint.to_bytes(),
        token_program: observed.descriptor.token_program(),
        rent_credit: observed.rent_credit.to_bytes(),
        rent_program: observed.rent_program.to_bytes(),
        generation: core.identity.generation,
        expected_claims_market_revision: aggregate.revision,
        // The executor prepays this exact floor before Hot if the account is
        // absent; an already funded System PDA keeps its observed balance.
        observed_receipt_lamports: receipt_lamports.max(receipt_rent),
        receipt_rent_principal: receipt_rent,
        expected_receipt_supply: 0,
        outcome_count: observed.descriptor.outcome_count(),
        coordinate_count: 0,
        rent_credit_before: observed.rent_credit_account.lamports,
        rent_credit_after: observed.rent_credit_account.lamports,
    })
}

/// One Registry record that this driver re-derived from selected release bytes.
#[derive(Clone, Debug)]
pub(crate) struct FinalizedStructuredRecordV1 {
    /// Schema selected by the compiled artifact.
    pub(crate) schema: [u8; 32],
    /// Raw content-addressed Registry PDA.
    pub(crate) raw: Pubkey,
    /// Paired vacant staging cursor PDA.
    pub(crate) staging: Pubkey,
    /// Exact finalized semantic body.
    pub(crate) body: Vec<u8>,
}

/// The selected V6 receipt bundle after its every record was authenticated.
#[derive(Clone, Debug)]
pub(crate) struct StructuredActivateReceiptArtifactsV1 {
    /// Finalized slot shared by the selected release records.
    pub(crate) slot: u64,
    /// Decoded immutable Realm/release selection.  Its descriptor/Market join
    /// happens only when the per-Market representation descriptor is present.
    pub(crate) token_behavior_selection: TokenBehaviorSelectionV2,
    /// Exact selected V6 bundle for `ActivateReceipt`.
    pub(crate) bundle: RationalLifecycleSelectedBundleV6,
    /// Config record coordinate, retained for the Hot fixed frame.
    pub(crate) config: FinalizedStructuredRecordV1,
    /// Descriptor, Profile13, RequestProfile, Lifecycle, Strategy, Transition,
    /// and Effect in that exact order.
    pub(crate) bundle_records: [FinalizedStructuredRecordV1; ACTIVATION_RECORD_COUNT_V1],
}

/// Derive the exact selected records the `ActivateReceipt` route may read.
///
/// This is pure hostile input parsing. The separate snapshot authenticator
/// below accepts only this record plan, so callers may batch it with founding
/// records without reproducing selected-release ordering.
pub(crate) fn selected_activate_receipt_record_expectations_v1(
    market_input_bytes: &[u8],
) -> Result<[SelectedReleaseRecordV1; 8]> {
    let input: MarketRunInput = serde_json::from_value(
        parse_json_without_duplicate_keys_v1(market_input_bytes)
            .map_err(|error| Error::new(format!("Structured market input {error}")))?,
    )
    .map_err(|error| Error::new(format!("Structured market input shape: {error}")))?;
    crate::market::validate_market_input(&input).map_err(|error| {
        Error::new(format!(
            "Structured activation selected release input refused: {error}"
        ))
    })?;
    let selected = input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("market input omitted its selected capability"))?;
    if selected.family != "structured" {
        return Err(Error::new(
            "market input selected another capability family for Structured activation",
        ));
    }
    Ok([
        selected_record_at_v1(selected, CONFIG_RECORD_INDEX_V1, "config", "config")?,
        selected_record_at_v1(
            selected,
            ACTIVATE_RECEIPT_RECORD_START_V1,
            "ActivateReceipt descriptor",
            "descriptor",
        )?,
        selected_record_at_v1(
            selected,
            ACTIVATE_RECEIPT_RECORD_START_V1 + 1,
            "ActivateReceipt account profile",
            "account-profile",
        )?,
        selected_record_at_v1(
            selected,
            ACTIVATE_RECEIPT_RECORD_START_V1 + 2,
            "ActivateReceipt request profile",
            "request-profile",
        )?,
        selected_record_at_v1(
            selected,
            ACTIVATE_RECEIPT_RECORD_START_V1 + 3,
            "ActivateReceipt lifecycle policy",
            "lifecycle-policy",
        )?,
        selected_record_at_v1(
            selected,
            ACTIVATE_RECEIPT_RECORD_START_V1 + 4,
            "ActivateReceipt strategy",
            "strategy",
        )?,
        selected_record_at_v1(
            selected,
            ACTIVATE_RECEIPT_RECORD_START_V1 + 5,
            "ActivateReceipt transition",
            "transition",
        )?,
        selected_record_at_v1(
            selected,
            ACTIVATE_RECEIPT_RECORD_START_V1 + 6,
            "ActivateReceipt effect",
            "effect",
        )?,
    ])
}

/// Authenticate a selected receipt bundle from one caller-owned finalized
/// snapshot.  `live` must be raw/staging pairs in expectation order.
pub(crate) fn hydrate_selected_activate_receipt_artifacts_from_snapshot_v1(
    registry: Pubkey,
    expected: &[SelectedReleaseRecordV1; 8],
    slot: u64,
    live: &[ObservedAccount],
) -> Result<StructuredActivateReceiptArtifactsV1> {
    if slot == 0 || live.len() != expected.len() * 2 {
        return Err(Error::new(
            "Structured activation finalized record snapshot changed width",
        ));
    }
    let mut finalized = Vec::with_capacity(expected.len());
    for (index, expected) in expected.iter().enumerate() {
        let raw = live
            .get(index * 2)
            .ok_or_else(|| Error::new("Structured activation missing finalized raw record"))?;
        let staging = live
            .get(index * 2 + 1)
            .ok_or_else(|| Error::new("Structured activation missing finalized staging record"))?;
        let (raw_address, staging_address) =
            record_coordinates_v1(registry, expected.schema, &expected.body)?;
        if raw.key != raw_address || staging.key != staging_address || raw.data != expected.body {
            return Err(Error::new(format!(
                "Structured activation finalized {} differs from its selected release",
                expected.label
            )));
        }
        authenticate_finalized_record(
            registry,
            raw,
            &FinalizedRecordProof {
                schema_release_id: expected.schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(|error| {
            Error::new(format!(
                "Structured activation finalized {} record refused: {error:?}",
                expected.label
            ))
        })?;
        finalized.push(FinalizedStructuredRecordV1 {
            schema: expected.schema,
            raw: raw_address,
            staging: staging_address,
            body: raw.data.clone(),
        });
    }
    let config = finalized
        .first()
        .cloned()
        .ok_or_else(|| Error::new("Structured activation missing finalized config record"))?;
    let bundle_records = std::array::from_fn(|index| finalized[index + 1].clone());
    let token_behavior_selection = TokenBehaviorSelectionV2::decode(&config.body)
        .map_err(|error| Error::new(format!("Structured activation config: {error:?}")))?;
    let descriptor = bundle_records[0]
        .body
        .as_slice()
        .try_into()
        .map_err(|_| Error::new("Structured activation descriptor width differs"))?;
    let strategy = bundle_records[4]
        .body
        .as_slice()
        .try_into()
        .map_err(|_| Error::new("Structured activation strategy width differs"))?;
    let token_behavior_selection_bytes = config
        .body
        .as_slice()
        .try_into()
        .map_err(|_| Error::new("Structured activation config width differs"))?;
    let bundle = RationalLifecycleSelectedBundleV6 {
        action: LifecycleActionV2::ActivateReceipt,
        release_set: token_behavior_selection.release_set(),
        token_program: token_behavior_selection.token_program(),
        token_behavior_selection: token_behavior_selection_bytes,
        account_profile: bundle_records[1].body.clone(),
        request_profile: bundle_records[2].body.clone(),
        lifecycle_policy: bundle_records[3].body.clone(),
        strategy,
        transition: bundle_records[5].body.clone(),
        effect: bundle_records[6].body.clone(),
        descriptor,
    };
    validate_rational_lifecycle_selected_bundle_v6(&bundle)
        .map_err(|error| Error::new(format!("Structured activation selected bundle: {error:?}")))?;
    Ok(StructuredActivateReceiptArtifactsV1 {
        slot,
        token_behavior_selection,
        bundle,
        config,
        bundle_records,
    })
}

/// Reconstruct the `ActivateReceipt` V6 bundle from a dedicated finalized
/// snapshot. Batched consumers should use the expectation and snapshot helpers
/// above so they can include founding records in the same response.
pub(crate) fn hydrate_selected_activate_receipt_artifacts_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    market_input_bytes: &[u8],
    minimum_slot: u64,
) -> Result<StructuredActivateReceiptArtifactsV1> {
    let expected = selected_activate_receipt_record_expectations_v1(market_input_bytes)?;
    let addresses = expected
        .iter()
        .map(|record| record_coordinates_v1(registry, record.schema, &record.body))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flat_map(|(raw, staging)| [raw, staging])
        .collect::<Vec<_>>();
    let (observation, live) =
        rpc.finalized_observed_accounts_admitting_vacant(&addresses, minimum_slot)?;
    hydrate_selected_activate_receipt_artifacts_from_snapshot_v1(
        registry,
        &expected,
        observation.slot,
        &live,
    )
}

/// Build the canonical unsigned V6 `ActivateReceipt` instruction.
///
/// The caller supplies the same-finalized Hot frame and per-Market rational
/// descriptor it has already admitted. This function refuses another lifecycle
/// action before calling the selected operator, so the receipt route cannot be
/// repurposed into coordinate creation by a changed child byte string.
pub(crate) fn build_selected_activate_receipt_instruction_v1(
    artifacts: &StructuredActivateReceiptArtifactsV1,
    state: &RationalLifecycleHotStateV3<'_>,
    claims_child: &Instruction,
    representation_descriptor: RepresentationDescriptorV2<'_>,
    market_realm: [u8; 32],
) -> Result<RationalLifecycleHotInstructionV3> {
    let request = LifecycleRequestV2::decode(&claims_child.data)
        .map_err(|error| Error::new(format!("Structured activation Claims child: {error:?}")))?;
    if request.header().action != LifecycleActionV2::ActivateReceipt
        || request.header().coordinate_count != 0
    {
        return Err(Error::new(
            "Structured activation accepts only an ActivateReceipt child with no coordinates",
        ));
    }
    let config_digest: [u8; 32] = Sha256::digest(&artifacts.config.body).into();
    let token_behavior = authenticate_token_behavior_v2(
        representation_descriptor,
        market_realm,
        &artifacts.config.body,
        TokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: artifacts.config.schema,
            selected_content_digest: config_digest,
            finalized_content_digest: config_digest,
            recomputed_content_digest: config_digest,
            record_authenticated: true,
            market_realm_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured activation token behavior: {error:?}")))?;
    if token_behavior.selection() != artifacts.token_behavior_selection {
        return Err(Error::new(
            "Structured activation config changed after finalized record authentication",
        ));
    }
    build_rational_lifecycle_selected_hot_instruction_v6(
        state,
        claims_child,
        RationalLifecycleSelectedSelectionV6 {
            bundle: &artifacts.bundle,
            authenticated_token_behavior: token_behavior,
            representation_descriptor,
        },
    )
    .map_err(|error| {
        Error::new(format!(
            "Structured activation selected operator: {error:?}"
        ))
    })
}

#[derive(Debug)]
pub(crate) struct SelectedReleaseRecordV1 {
    pub(crate) label: String,
    pub(crate) schema: [u8; 32],
    pub(crate) body: Vec<u8>,
}

fn selected_record_at_v1(
    selected: &crate::model::SelectedCapabilityV1,
    index: usize,
    role: &str,
    compiled_label: &str,
) -> Result<SelectedReleaseRecordV1> {
    let record = selected.records.get(index).ok_or_else(|| {
        Error::new(format!(
            "Structured activation missing selected {role} record"
        ))
    })?;
    let expected_label = format!(
        "structured_{index:02}_{}_record",
        compiled_label.replace('-', "_")
    );
    if record.label != expected_label {
        return Err(Error::new(format!(
            "Structured activation selected {role} record label differs from compiled Structured release"
        )));
    }
    let schema = hex32(&record.schema_hex)?;
    let body = crate::runtime::decode_hex(&record.body_hex)?;
    if body.is_empty() {
        return Err(Error::new(format!(
            "Structured activation selected {role} record is empty"
        )));
    }
    Ok(SelectedReleaseRecordV1 {
        label: record.label.clone(),
        schema,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_selected_capability_payload_keeps_its_activation_deadline() {
        let selected = crate::model::SelectedCapabilityV1 {
            family: "structured".into(),
            program_set_hex: "aa".into(),
            selected_descriptor_hex: "bb".into(),
            config_hex: "cc".into(),
            publication_hex: "dd".into(),
            records: vec![crate::model::SelectedCapabilityRecordV1 {
                label: "structured_00_descriptor_record".into(),
                schema_hex: "11".repeat(32),
                body_hex: "22".into(),
            }],
            activation_deadline_slot: 42,
            root_rent_minimum_lamports: 1,
            creation_principal_lamports: 7,
            selected_manifest_entry_index: 3,
        };
        let encoded = serde_json::to_value(&selected).expect("complete selected payload encodes");
        let decoded: crate::model::SelectedCapabilityV1 =
            serde_json::from_value(encoded).expect("complete selected payload parses");
        assert_eq!(decoded.family, "structured");
        assert_eq!(decoded.activation_deadline_slot, 42);
        assert_eq!(decoded.creation_principal_lamports, 7);
    }

    #[test]
    fn missing_compiled_receipt_record_names_its_role() {
        let selected = crate::model::SelectedCapabilityV1 {
            family: "structured".into(),
            program_set_hex: String::new(),
            selected_descriptor_hex: String::new(),
            config_hex: String::new(),
            publication_hex: String::new(),
            records: Vec::new(),
            activation_deadline_slot: 1,
            root_rent_minimum_lamports: 1,
            creation_principal_lamports: 0,
            selected_manifest_entry_index: 0,
        };
        let error = selected_record_at_v1(
            &selected,
            ACTIVATE_RECEIPT_RECORD_START_V1,
            "ActivateReceipt descriptor",
            "descriptor",
        )
        .expect_err("missing compiled receipt descriptor must refuse");
        assert_eq!(
            error.to_string(),
            "Structured activation missing selected ActivateReceipt descriptor record"
        );
    }
}

pub(crate) fn record_coordinates_v1(
    registry: Pubkey,
    schema: [u8; 32],
    body: &[u8],
) -> Result<(Pubkey, Pubkey)> {
    let digest: [u8; 32] = Sha256::digest(body).into();
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).map_err(|error| {
            Error::new(format!("Structured activation record schema: {error:?}"))
        })?,
        ContentDigest::new(digest).map_err(|error| {
            Error::new(format!("Structured activation record digest: {error:?}"))
        })?,
    );
    let address = |seeds: RecordPdaSeedsV1| {
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
    Ok((
        address(key.raw_record_pda_seeds()),
        address(key.staging_cursor_pda_seeds()),
    ))
}

/// Hydrate the chain facts that precede `ActivateReceipt` construction.
///
/// The two persisted documents are checksum-bound by their callers before this
/// function receives them. The report is parsed by its semantic owner and the
/// Market input is hostile-decoded again to prove it still selects Structured.
/// The current bytes come from one finalized response and are compared to the
/// report's recorded byte hashes before being exposed.
pub(crate) fn hydrate_activate_receipt_v1(
    rpc: &mut Rpc,
    market_input_bytes: &[u8],
    campaign_report_bytes: &[u8],
    minimum_slot: u64,
) -> Result<StructuredActivateReceiptObservationV1> {
    let input: MarketRunInput = serde_json::from_value(
        parse_json_without_duplicate_keys_v1(market_input_bytes)
            .map_err(|error| Error::new(format!("Structured market input {error}")))?,
    )
    .map_err(|error| Error::new(format!("Structured market input shape: {error}")))?;
    let selected = input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("market input omitted its selected capability"))?;
    // `MarketRunInput` has already parsed this complete selected-capability
    // DTO. Re-decoding it through a one-field deny-unknown surrogate made a
    // newly canonical selected field look hostile; the parsed DTO remains the
    // sole schema authority here.
    if selected.family != "structured" {
        return Err(Error::new(
            "market input selected another capability family for Structured activation",
        ));
    }
    let evidence = campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
        campaign_report_bytes,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    let market_row = evidence
        .accounts
        .get(MARKET_LABEL_V1)
        .ok_or_else(|| Error::new("campaign report omitted founding_market"))?;
    let aggregate_row = evidence
        .accounts
        .get(AGGREGATE_LABEL_V1)
        .ok_or_else(|| Error::new("campaign report omitted claims_aggregate"))?;
    let market = pubkey(&market_row.address)?;
    let aggregate = pubkey(&aggregate_row.address)?;
    let (slot, accounts) = rpc.finalized_accounts(&[market, aggregate], minimum_slot)?;
    if accounts.len() != 2 {
        return Err(Error::new(
            "Structured activation finalized snapshot changed width",
        ));
    }
    let market_account = accounts[0]
        .clone()
        .ok_or_else(|| Error::new("Structured activation Market is absent"))?;
    let aggregate_account = accounts[1]
        .clone()
        .ok_or_else(|| Error::new("Structured activation Claims aggregate is absent"))?;
    require_report_digest(&market_account, &market_row.data_sha256, "Market")?;
    require_report_digest(
        &aggregate_account,
        &aggregate_row.data_sha256,
        "Claims aggregate",
    )?;
    let state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Structured activation Market: {error:?}")))?;
    if state.phase != Phase::Open {
        return Err(Error::new("Structured activation requires an Open Market"));
    }
    Ok(StructuredActivateReceiptObservationV1 {
        slot,
        market,
        market_account,
        aggregate,
        aggregate_account,
    })
}

fn require_report_digest(account: &RpcAccount, expected: &str, label: &str) -> Result<()> {
    let actual = Sha256::digest(&account.data);
    let actual = actual
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected {
        return Err(Error::new(format!(
            "Structured activation finalized {label} bytes differ from the sealed founding report"
        )));
    }
    Ok(())
}

/// Require a regular checksum-pinned document before opening a key or RPC.
pub(crate) fn read_pinned_structured_document_v1(
    path: &Path,
    expected_sha256: &str,
    label: &str,
) -> Result<Vec<u8>> {
    if !path.is_absolute()
        || expected_sha256.len() != 64
        || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(Error::new(format!(
            "Structured {label} path or SHA-256 is invalid"
        )));
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(Error::new(format!(
            "Structured {label} must be a regular file"
        )));
    }
    let bytes = std::fs::read(path)?;
    let actual = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected_sha256.to_ascii_lowercase() {
        return Err(Error::new(format!("Structured {label} SHA-256 differs")));
    }
    Ok(bytes)
}
