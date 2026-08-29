use std::{
    collections::BTreeMap,
    thread,
    time::{Duration, Instant},
};

#[path = "founding_submission_journal.rs"]
pub(crate) mod founding_submission_journal;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityEntryV1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingLedgerV2, MAX_DEPENDENCIES_PER_CAPABILITY,
    controller_funding_checkpoint::{
        CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1, CONTROLLER_FUNDING_CUSTODY_ABORT_ANCHOR_DOMAIN_V1,
        CONTROLLER_FUNDING_CUSTODY_LADDER_DIGEST_DOMAIN_V1,
        ControllerFundingCheckpointDerivationV1, ControllerFundingCheckpointPhaseV1,
        ControllerFundingCheckpointV1, ControllerFundingControllerV1,
    },
    funding_ledger_bytes_v2,
};
use dclutch_claims_svm::{
    founding_v5::{
        ClaimsFoundingAggregateSeedsV5, ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5,
    },
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        liability_basis_vector_width_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyVaultSeedsV1, FoundingPrestateStageV1,
    OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1, PROJECTED_CUSTODY_STATE_BYTES_V2,
    PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCallerRoleV1, ProjectedCustodyCallerSeedsV1,
    ProjectedCustodyOperationV1, ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1,
    ProjectedCustodyStateV2, SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
};
use dclutch_direct_codec::{
    COMPILED_DIRECT_RELEASE_ID_V1, execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3,
};
use dclutch_market_core_codec::{
    Action, CoreState, FOUND_ACCOUNT_COUNT_V3, FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3,
    FOUND_RENT_SYSVAR_INDEX_V3, FoundingIntentV5, GenericFoundingRequestV1, GenericFoundingStageV1,
    Identity, MarketCoreStateSeedsV2, MarketIdentity, PROJECT_FOUND_ACCOUNT_COUNT_V2, Phase,
    ProjectFoundReceiptV2, ProjectFoundRequestV2, Readiness, Request,
    SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES, SeriesFoundingPermitSeedsV1,
    generic_founding_funding_list_id_v1,
};
use dclutch_market_founding_v1_operator::{
    authenticate_generic_market_founding_artifact_v1, construct_generic_founding_root_selection_v1,
    construct_generic_market_founding_plan_v1,
};
use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product_runtime_v2::{
    ContentId as ProductContentId, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, CompiledProductRecordsV2, FinalizedRecordObservationV2,
    ProductCompilationInputV2, compile_product_records_v2,
    found::{
        FinalizedReferenceObservationV2, FoundProjectionStateV2, FoundStateV2,
        build_found_instruction_v2, project_found_v2,
    },
    lifecycle_rent_v2::{LifecycleRentCreateStateV2, build_lifecycle_rent_create_v2},
    publication::{RecordPublicationContentV1, derive_record_addresses_v1},
};
use dclutch_pyth_svm::{PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1, PythSponsoredPushReleaseV1};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleRentCreditV2,
};
use dclutch_resolution_codec::{
    FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1, PreMarketFundingAbortRequestV1,
    PreMarketFundingRequestV2, pre_market_funding_ledger_account_digest_v1,
    pre_market_funding_prestate_digest_v1,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, ManipulationFloorV1,
    PROVIDER_RELEASE_SCHEMA_ID_V1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, ProviderReleaseV1,
    PythAdapterConfigV1, RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryPolicyV2,
    SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1,
    SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialV3, SourceSpecV1,
    WINDOW_SPEC_SCHEMA_ID_V1,
};
use dclutch_token_svm::{
    ACCOUNT_BYTES, AccountState, CollateralAdapterReleaseV1, MINT_BYTES, Mint,
    TOKEN_2022_PROGRAM_ID, TokenAccount,
};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::state::AddressLookupTable;
use solana_sdk::{
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::{AddressLookupTableAccount, VersionedMessage, v0},
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::VersionedTransaction,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{create_account, transfer};

use dclutch_versioned_message_operator::{
    Observation, ObservedAccount, build_lookup_table_creation_v1, build_lookup_table_freeze,
};

use crate::{
    Error, Result,
    direct_market::{
        DirectMarketCompilerInputV1, attach_direct_market_capability_v1,
        validate_direct_market_capability_v1,
    },
    funding_readiness::{
        FundingReadinessCoordinatesV1, FundingReadinessInstructionPlanV1, FundingReadinessPlanV1,
        FundingReadinessPrepayV1, FundingReadinessRecordCoordinatesV1,
        FundingReadinessRoutedPlanV1, funding_readiness_routing_addresses_v1,
        plan_funding_readiness_from_rpc_v1, plan_funding_readiness_with_routing_from_rpc_v1,
    },
    model::{AccountEvidence, MarketRunInput, SuccessorPlan, TransactionEvidence},
    plan::{hex, hex32, pubkey},
    rpc::{FOUNDING_HEAP_FRAME_BYTES, Rpc, RpcAccount, account_evidence, bounded_instructions},
    runtime::{PublishedRecord, decode_hex, publish_product_graph, publish_record, record},
    seed::{KeyForge, role},
};

use founding_submission_journal::{
    FoundingFinalizationV1, FoundingPreSendProjectionV1, FoundingSubmissionBindingV1,
    FoundingSubmissionJournalV1, FoundingSubmissionOperationV1, FoundingSubmissionPhaseV1,
    FoundingSubmissionPlanV1, FoundingSubmissionRecoveryV1,
    authenticate_bound_founding_submission_prefix_v1, authenticate_founding_packet_fresh_v1,
    authenticate_founding_submission_v1, dispatch_founding_submission_v1,
    finalize_founding_submission_v1, founding_submission_finalized_poststates_v1,
    founding_submission_message_v1, founding_submission_packet_v1,
    founding_submission_recovery_payload_v1, founding_submission_recovery_v1,
    plan_founding_submission_v1, prepare_founding_submission_v1, submit_founding_submission_v1,
    visit_founding_pre_send_boundary_v1,
};

/// The captured Pyth `PriceUpdateV2` account body this demo Market resolves
/// against. It is one of the eleven provenance-pinned artifacts
/// `dclutch-successor-validator` verifies before it starts, and the launcher
/// loads the receiver and router ELFs beside it, so the bytes here and the
/// programs on the chain come from one pinned set.
const FIXTURE_PRICE_UPDATE: &[u8] =
    include_bytes!("../../../../../fixtures/pyth/local-upgraded-2026-08-22/price-update.account");

/// The demo Market's terminal window width, in seconds.
///
/// Not one instant: TWIN measured that a window pinned to a single second is
/// answered only when a publication happens to land on it, and Pyth's SOL/USD
/// cadence is nearer five minutes. Three hundred seconds is one cadence of
/// margin on a period that ends at the publication the market is about.
const TERMINAL_WINDOW_WIDTH_SECONDS: i64 = 300;

/// How stale the captured publication may be, measured against the CLUSTER's
/// clock rather than against the window.
///
/// This is the fixture's shelf life and not a market's staleness tolerance.
/// The captured publication instant is frozen; a validator's clock is
/// wall-clock; so the quantity this bounds grows by 86,400 every day the
/// fixture is not recaptured. One year is a stated bound with a tripwire
/// behind it: the journey campaign refuses once the fixture outlives it, so
/// the number can never be quietly widened instead of the fixture being
/// refreshed. A Market resolving against a live feed states seconds here.
const FIXTURE_SHELF_LIFE_SECONDS: u32 = 31_536_000;

pub(crate) const REMAINING_OPEN_SEAM: &str = "The campaign publishes the complete authenticated Product and Source graph, creates the lifecycle credit, projects the future Market through Core Found37, prepares the two controller-owned FundingLedgerV2 accounts and their checkpoint through DCLTCFQ1, stages projected custody through DCLTPCB2, and then opens the Market atomically through DCLTGMF3. The opening transaction locks the Hoard, creates Core and Claims state, realizes custody, consumes the one-shot permit, and commits Open last. Compute evidence must be remeasured for these split routes; the runner does not reuse pre-split founding measurements.";

/// The one local-only participant allocation. It is test liquidity, not
/// founding principal, and therefore never enters a Hoard or a principal cap.
pub(crate) const LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1: u64 = 100_000_000;
/// Local-prepare role that owns the fixture Token-2022 account.
pub(crate) const LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1: &str = "participant";
/// Local-prepare role whose key is the fixture Token-2022 account address.
pub(crate) const LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1: &str = "direct-buyer";

/// Exact finalized receipt for the local-only participant allocation.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LocalParticipantFixtureLiquidityEvidenceV1 {
    pub(crate) source_token_account: String,
    pub(crate) source_owner: String,
    pub(crate) quantity_atoms: u64,
    pub(crate) founding_collateral_atoms: u64,
    pub(crate) total_supply_atoms: u64,
    pub(crate) mint: String,
    pub(crate) mint_authority_removed: bool,
    pub(crate) transaction_signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) compute_units_consumed: u64,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct MarketExecutionEvidence {
    pub(crate) completed: Vec<String>,
    pub(crate) accounts: BTreeMap<String, AccountEvidence>,
    pub(crate) founding_custody_context: String,
    pub(crate) direct_selected_manifest_entry_index: u16,
    /// Projected by campaign.rs at `execution.localParticipantFixtureLiquidity`;
    /// skipped here so the report has one JSON coordinate for the receipt.
    #[serde(skip)]
    pub(crate) local_participant_fixture_liquidity:
        Option<LocalParticipantFixtureLiquidityEvidenceV1>,
}

/// Public identities that shape founding but never sign a campaign packet.
///
/// Keeping them separate from [`KeyForge`] prevents a public driver from
/// demanding private-key files for roles whose secret material is never used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FoundingActorsV1 {
    pub(crate) founder: Pubkey,
    pub(crate) substituted_founder: Pubkey,
}

impl FoundingActorsV1 {
    pub(crate) fn new(founder: Pubkey, substituted_founder: Pubkey) -> Result<Self> {
        if founder == substituted_founder {
            return Err(Error::new(
                "founder and substituted-founder identities must be distinct",
            ));
        }
        Ok(Self {
            founder,
            substituted_founder,
        })
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MarketExecutionCheckpointV1 {
    pub(crate) schema: String,
    pub(crate) market: String,
    #[serde(rename = "foundingCustodyContext")]
    pub(crate) founding_custody_context: String,
    #[serde(rename = "directSelectedManifestEntryIndex")]
    pub(crate) direct_selected_manifest_entry_index: u16,
    pub(crate) direct_capability_root: String,
    pub(crate) direct_trading_funding_ledger: String,
    pub(crate) expiry_slot: u64,
    pub(crate) found_record: String,
    pub(crate) lock_record: String,
    #[serde(rename = "localParticipantFixtureLiquidity")]
    pub(crate) local_participant_fixture_liquidity:
        Option<LocalParticipantFixtureLiquidityEvidenceV1>,
    pub(crate) accounts: BTreeMap<String, AccountEvidence>,
    pub(crate) completed: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Dcltpcb2RecoveryPayloadV1 {
    schema: String,
    checkpoint: MarketExecutionCheckpointV1,
    completion_accounts: BTreeMap<String, String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Dcltcfq1RecoveryPayloadV1 {
    schema: String,
    checkpoint: MarketExecutionCheckpointV1,
    completion_accounts: BTreeMap<String, String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FundingReadinessRecoveryPayloadV1 {
    schema: String,
    operation: FoundingSubmissionOperationV1,
    market: String,
    source_state: String,
    funding_ledger: String,
    beneficiary: String,
    activation_receipt: String,
    expected_next_route: String,
}

pub(crate) const DCLTCFQ1_RECOVERY_PAYLOAD_SCHEMA_V1: &str =
    "dclutch-market-dcltcfq1-recovery-payload-v1";
pub(crate) const DCLTCFQ1_PREPARED_CHECKPOINT_SCHEMA_V1: &str =
    "dclutch-market-dcltcfq1-prepared-checkpoint-v1";
pub(crate) const DCLTPCB2_RECOVERY_PAYLOAD_SCHEMA_V1: &str =
    "dclutch-market-dcltpcb2-recovery-payload-v1";
pub(crate) const DCLTPCB2_CHECKPOINT_SCHEMA_V1: &str = "dclutch-market-dcltpcb2-checkpoint-v1";
pub(crate) const FUNDING_READINESS_RECOVERY_PAYLOAD_SCHEMA_V1: &str =
    "dclutch-market-funding-readiness-recovery-payload-v1";

/// The campaign report owns the filesystem and its exclusive lease; this
/// adapter lets each split founding send advance its embedded journal without
/// giving `market.rs` a second file format or lock implementation.
pub(crate) struct FoundingSubmissionRecorderV1<'a> {
    binding: FoundingSubmissionBindingV1,
    journals: &'a mut BTreeMap<FoundingSubmissionOperationV1, FoundingSubmissionJournalV1>,
    persist: &'a mut dyn FnMut(&[FoundingSubmissionJournalV1]) -> Result<()>,
}

impl<'a> FoundingSubmissionRecorderV1<'a> {
    pub(crate) fn new(
        binding: FoundingSubmissionBindingV1,
        journals: &'a mut BTreeMap<FoundingSubmissionOperationV1, FoundingSubmissionJournalV1>,
        persist: &'a mut dyn FnMut(&[FoundingSubmissionJournalV1]) -> Result<()>,
    ) -> Result<Self> {
        let ordered = journals.values().cloned().collect::<Vec<_>>();
        authenticate_bound_founding_submission_prefix_v1(&binding, &ordered)?;
        Ok(Self {
            binding,
            journals,
            persist,
        })
    }

    fn current(
        &self,
        operation: FoundingSubmissionOperationV1,
    ) -> Option<&FoundingSubmissionJournalV1> {
        self.journals.get(&operation)
    }

    fn write(&mut self, journal: FoundingSubmissionJournalV1) -> Result<()> {
        authenticate_founding_submission_v1(&self.binding, &journal)?;
        let mut ordered = self
            .journals
            .values()
            .filter(|existing| existing.operation != journal.operation)
            .cloned()
            .collect::<Vec<_>>();
        ordered.push(journal.clone());
        ordered.sort_by_key(|row| row.operation);
        authenticate_bound_founding_submission_prefix_v1(&self.binding, &ordered)?;
        self.journals.insert(journal.operation, journal);
        (self.persist)(&ordered)
    }

    fn post_fsync_pre_send(
        &mut self,
        journal: &FoundingSubmissionJournalV1,
    ) -> Result<FoundingPreSendProjectionV1> {
        visit_founding_pre_send_boundary_v1(&self.binding, journal, &mut |_| Ok(()))
    }
}

fn compile_current_founding_message_v1(
    label: &str,
    payer: Pubkey,
    instructions: &[Instruction],
    observation: Observation,
    tables: &[ObservedAccount],
    heap_frame_bytes: Option<u32>,
    blockhash: Hash,
) -> Result<VersionedMessage> {
    let bounded = bounded_instructions(instructions, heap_frame_bytes)
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    let plan = dclutch_versioned_message_operator::compile_v0_message_with_optional_tables(
        payer,
        &bounded,
        solana_hash::Hash::new_from_array(blockhash.to_bytes()),
        observation,
        tables,
    )
    .map_err(|error| Error::new(format!("{label}: v0 message compilation: {error:?}")))?;
    Ok(plan.message)
}

fn authenticate_resolved_founding_message_v1(
    operation: FoundingSubmissionOperationV1,
    message: &VersionedMessage,
    tables: &[ObservedAccount],
) -> Result<()> {
    let mut resolved = std::collections::BTreeSet::new();
    let (static_keys, lookups) = match message {
        VersionedMessage::Legacy(message) => (message.account_keys.as_slice(), &[][..]),
        VersionedMessage::V0(message) => (
            message.account_keys.as_slice(),
            message.address_table_lookups.as_slice(),
        ),
    };
    if static_keys.iter().any(|key| !resolved.insert(*key)) {
        return Err(Error::new(
            "founding compiled message duplicated a static account",
        ));
    }
    let mut used_tables = std::collections::BTreeSet::new();
    for lookup in lookups {
        if !used_tables.insert(lookup.account_key) {
            return Err(Error::new(
                "founding compiled message duplicated a lookup table",
            ));
        }
        let table = tables
            .iter()
            .find(|table| table.key == lookup.account_key)
            .ok_or_else(|| Error::new("founding compiled message omitted its lookup body"))?;
        let decoded = AddressLookupTable::deserialize(&table.data)
            .map_err(|error| Error::new(format!("founding lookup table: {error:?}")))?;
        for index in lookup
            .writable_indexes
            .iter()
            .chain(&lookup.readonly_indexes)
        {
            let address = decoded
                .addresses
                .get(usize::from(*index))
                .ok_or_else(|| Error::new("founding lookup index exceeded its exact body"))?;
            if !resolved.insert(*address) {
                return Err(Error::new(
                    "founding resolved accounts aliased a static or loaded account",
                ));
            }
        }
    }
    if resolved.len() != operation.exact_unique_accounts() {
        return Err(Error::new(format!(
            "{} resolved account count changed: expected {}, observed {}",
            operation.label(),
            operation.exact_unique_accounts(),
            resolved.len()
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_current_founding_intent_v1(
    label: &str,
    operation: FoundingSubmissionOperationV1,
    instructions: &[Instruction],
    expected_signers: &[Pubkey],
    observation: Observation,
    tables: &[ObservedAccount],
    resolved_accounts_sha256: &str,
    prestate_addresses: &[Pubkey],
    completion_addresses: &[Pubkey],
    recovery_payload: &[u8],
    heap_frame_bytes: Option<u32>,
    binding: &FoundingSubmissionBindingV1,
    current: &FoundingSubmissionJournalV1,
) -> Result<()> {
    let persisted = founding_submission_message_v1(binding, current)?;
    let blockhash = match &persisted {
        VersionedMessage::Legacy(message) => message.recent_blockhash,
        VersionedMessage::V0(message) => message.recent_blockhash,
    };
    let recomputed = compile_current_founding_message_v1(
        label,
        binding.payer,
        instructions,
        observation,
        tables,
        heap_frame_bytes,
        blockhash,
    )?;
    authenticate_resolved_founding_message_v1(operation, &recomputed, tables)?;
    let expected_signers = expected_signers
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let expected_prestate_accounts = prestate_addresses
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let expected_completion_accounts = completion_addresses
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if current.operation != operation
        || recomputed.serialize() != persisted.serialize()
        || current.expected_signers != expected_signers
        || current.resolved_accounts_sha256 != resolved_accounts_sha256
        || current.prestate_accounts != expected_prestate_accounts
        || current.completion_accounts != expected_completion_accounts
        || current.completion_contract_sha256
            != founding_completion_contract_v1(operation, completion_addresses)?
        || founding_submission_recovery_payload_v1(binding, current)? != recovery_payload
    {
        return Err(Error::new(format!(
            "{} durable journal does not match the freshly derived instruction, routing, signer, prestate, completion, or recovery intent",
            operation.label()
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn send_durable_founding_v1(
    rpc: &mut Rpc,
    label: &str,
    operation: FoundingSubmissionOperationV1,
    instructions: &[Instruction],
    signers: &[&Keypair],
    observation: Observation,
    tables: &[ObservedAccount],
    resolved_accounts_sha256: String,
    prestate_addresses: &[Pubkey],
    completion_addresses: &[Pubkey],
    recovery_payload: Vec<u8>,
    heap_frame_bytes: Option<u32>,
    recorder: &mut FoundingSubmissionRecorderV1<'_>,
    authenticate_completion: &mut dyn FnMut(&mut Rpc) -> Result<()>,
) -> Result<TransactionEvidence> {
    let payer = signers
        .first()
        .ok_or_else(|| Error::new("durable founding submission omitted payer"))?;
    let expected_signers = signers
        .iter()
        .map(|signer| signer.pubkey())
        .collect::<Vec<_>>();
    let expected_prestate = founding_account_set_digest_v1(rpc, prestate_addresses)?;
    let completion_contract_sha256 =
        founding_completion_contract_v1(operation, completion_addresses)?;

    if recorder.current(operation).is_none() {
        let latest = rpc.call(
            "getLatestBlockhash",
            &serde_json::json!([{"commitment":"finalized"}]),
        )?;
        let value = latest
            .get("value")
            .ok_or_else(|| Error::new("founding getLatestBlockhash omitted value"))?;
        let blockhash = value
            .get("blockhash")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::new("founding getLatestBlockhash omitted blockhash"))?
            .parse::<Hash>()
            .map_err(|error| Error::new(format!("founding blockhash: {error}")))?;
        let last_valid_block_height = value
            .get("lastValidBlockHeight")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::new("founding blockhash omitted last-valid height"))?;
        let message = compile_current_founding_message_v1(
            label,
            payer.pubkey(),
            instructions,
            observation,
            tables,
            heap_frame_bytes,
            blockhash,
        )?;
        authenticate_resolved_founding_message_v1(operation, &message, tables)?;
        let message_bytes = message.serialize();
        let exact_fee_lamports = rpc
            .call(
                "getFeeForMessage",
                &serde_json::json!([BASE64.encode(&message_bytes), {"commitment":"finalized"}]),
            )?
            .get("value")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::new("founding getFeeForMessage omitted exact fee"))?;
        let journal = plan_founding_submission_v1(
            &recorder.binding,
            FoundingSubmissionPlanV1 {
                operation,
                message,
                last_valid_block_height,
                exact_fee_lamports,
                expected_signers: expected_signers.clone(),
                resolved_accounts_sha256: resolved_accounts_sha256.clone(),
                prestate_accounts: prestate_addresses.to_vec(),
                prestate_sha256: expected_prestate.clone(),
                completion_accounts: completion_addresses.to_vec(),
                completion_contract_sha256: completion_contract_sha256.clone(),
                recovery_payload: recovery_payload.clone(),
            },
        )?;
        // Key-free, self-authenticated Planned review is durable first.
        recorder.write(journal)?;
    }

    let current = recorder
        .current(operation)
        .ok_or_else(|| Error::new("durable founding journal disappeared before intent join"))?;
    authenticate_current_founding_intent_v1(
        label,
        operation,
        instructions,
        &expected_signers,
        observation,
        tables,
        &resolved_accounts_sha256,
        prestate_addresses,
        completion_addresses,
        &recovery_payload,
        heap_frame_bytes,
        &recorder.binding,
        current,
    )?;

    loop {
        let current = recorder
            .current(operation)
            .cloned()
            .ok_or_else(|| Error::new("durable founding journal disappeared"))?;
        match founding_submission_recovery_v1(&recorder.binding, &current)? {
            FoundingSubmissionRecoveryV1::SignOnce => {
                if current.prestate_sha256
                    != founding_account_set_digest_v1(rpc, prestate_addresses)?
                {
                    return Err(Error::new(format!(
                        "{} prestate changed after Planned review and before signing",
                        operation.label()
                    )));
                }
                let height = rpc
                    .call(
                        "getBlockHeight",
                        &serde_json::json!([{"commitment":"finalized"}]),
                    )?
                    .as_u64()
                    .ok_or_else(|| Error::new("founding block height was not u64"))?;
                authenticate_founding_packet_fresh_v1(&recorder.binding, &current, height)?;
                let message = founding_submission_message_v1(&recorder.binding, &current)?;
                let transaction =
                    VersionedTransaction::try_new(message, signers).map_err(|error| {
                        Error::new(format!("sign {} packet: {error}", operation.label()))
                    })?;
                let packet = bincode::serialize(&transaction).map_err(|error| {
                    Error::new(format!("serialize {} packet: {error}", operation.label()))
                })?;
                let prepared =
                    prepare_founding_submission_v1(&recorder.binding, &current, &packet)?;
                // Exact packet bytes and signature are fsynced before first send.
                recorder.write(prepared)?;
            }
            FoundingSubmissionRecoveryV1::BeginDispatch => {
                let height = rpc
                    .call(
                        "getBlockHeight",
                        &serde_json::json!([{"commitment":"finalized"}]),
                    )?
                    .as_u64()
                    .ok_or_else(|| Error::new("founding block height was not u64"))?;
                authenticate_founding_packet_fresh_v1(&recorder.binding, &current, height)?;
                if current.prestate_sha256
                    != founding_account_set_digest_v1(rpc, prestate_addresses)?
                {
                    return Err(Error::new(format!(
                        "{} Prepared prestate changed before dispatch; do not send or re-sign",
                        operation.label()
                    )));
                }
                // Dispatching is fsynced before the native pre-send seam. A
                // restart from it may use only the authenticated packet bytes.
                let dispatching = dispatch_founding_submission_v1(&recorder.binding, &current)?;
                recorder.write(dispatching)?;
            }
            FoundingSubmissionRecoveryV1::ResendIdenticalPacket => {
                let signature = current
                    .expected_signature
                    .as_deref()
                    .ok_or_else(|| Error::new("Dispatching founding journal omitted signature"))?
                    .parse::<Signature>()
                    .map_err(|error| {
                        Error::new(format!("Dispatching founding signature: {error}"))
                    })?;
                if let Some(finalized) = rpc.finalized_signed_packet(label, signature, false)? {
                    return finish_durable_founding_v1(
                        rpc,
                        finalized,
                        current,
                        recorder,
                        authenticate_completion,
                    );
                }
                let height = rpc
                    .call(
                        "getBlockHeight",
                        &serde_json::json!([{"commitment":"finalized"}]),
                    )?
                    .as_u64()
                    .ok_or_else(|| Error::new("founding block height was not u64"))?;
                authenticate_founding_packet_fresh_v1(&recorder.binding, &current, height)?;
                if current.prestate_sha256
                    != founding_account_set_digest_v1(rpc, prestate_addresses)?
                {
                    return Err(Error::new(format!(
                        "{} Dispatching recovery found changed prestate; poll the exact signature and do not resend",
                        operation.label()
                    )));
                }
                let packet = founding_submission_packet_v1(&recorder.binding, &current)?;
                // Native crash-test seam: Dispatching is already fsynced. A kill
                // here recovers by sending only these identical bytes/signature.
                let projection = recorder.post_fsync_pre_send(&current)?;
                if projection.signature != signature.to_string()
                    || projection.signed_packet_sha256 != hex(&Sha256::digest(&packet))
                {
                    return Err(Error::new(
                        "Dispatching founding pre-send projection changed packet or signature",
                    ));
                }
                let returned = rpc.submit_signed_packet_once(label, &packet, signature, false)?;
                let submitted = submit_founding_submission_v1(
                    &recorder.binding,
                    &current,
                    &returned.to_string(),
                )?;
                // A kill after the exact send but before this fsync leaves
                // Dispatching; recovery may duplicate only the same signature.
                // Once Submitted is fsynced, recovery is strictly poll-only.
                recorder.write(submitted)?;
            }
            FoundingSubmissionRecoveryV1::PollOnly => {
                let signature = current
                    .expected_signature
                    .as_deref()
                    .ok_or_else(|| Error::new("Submitted founding journal omitted signature"))?
                    .parse::<Signature>()
                    .map_err(|error| {
                        Error::new(format!("Submitted founding signature: {error}"))
                    })?;
                let deadline = Instant::now() + Duration::from_secs(300);
                while Instant::now() < deadline {
                    if let Some(finalized) = rpc.finalized_signed_packet(label, signature, false)? {
                        return finish_durable_founding_v1(
                            rpc,
                            finalized,
                            current,
                            recorder,
                            authenticate_completion,
                        );
                    }
                    thread::sleep(Duration::from_millis(250));
                }
                return Err(Error::new(format!(
                    "{} Submitted signature {signature} is not finalized; recovery is poll-only and opens no key or send path",
                    operation.label()
                )));
            }
            FoundingSubmissionRecoveryV1::Complete => {
                authenticate_completion(rpc)?;
                return authenticate_completed_founding_submission_v1(
                    rpc,
                    label,
                    &recorder.binding,
                    &current,
                );
            }
        }
    }
}

fn finish_durable_founding_v1(
    rpc: &mut Rpc,
    finalized: crate::rpc::FinalizedSignedPacketV1,
    submitted: FoundingSubmissionJournalV1,
    recorder: &mut FoundingSubmissionRecorderV1<'_>,
    authenticate_completion: &mut dyn FnMut(&mut Rpc) -> Result<()>,
) -> Result<TransactionEvidence> {
    if !matches!(
        submitted.phase,
        FoundingSubmissionPhaseV1::Dispatching | FoundingSubmissionPhaseV1::Submitted
    ) {
        return Err(Error::new(
            "founding finalization did not start from an ambiguous durable packet",
        ));
    }
    // A packet observed finalized from Dispatching is advanced through Submitted
    // locally first. That preserves the one adjacent phase grammar without a
    // second network send.
    let submitted = if submitted.phase == FoundingSubmissionPhaseV1::Dispatching {
        let signature = submitted
            .expected_signature
            .clone()
            .ok_or_else(|| Error::new("Dispatching founding journal omitted signature"))?;
        let next = submit_founding_submission_v1(&recorder.binding, &submitted, &signature)?;
        recorder.write(next.clone())?;
        next
    } else {
        submitted
    };
    authenticate_completion(rpc)?;
    let poststates = capture_founding_poststates_v1(rpc, &submitted)?;
    let packet_sha256 = hex(&Sha256::digest(&finalized.packet));
    let fee_lamports = finalized
        .evidence
        .fee_lamports
        .ok_or_else(|| Error::new("finalized founding transaction omitted fee"))?;
    let compute_units_consumed = finalized
        .evidence
        .compute_units_consumed
        .ok_or_else(|| Error::new("finalized founding transaction omitted compute units"))?;
    let next = finalize_founding_submission_v1(
        &recorder.binding,
        &submitted,
        FoundingFinalizationV1 {
            signature: finalized.evidence.signature.clone(),
            finalized_slot: finalized.evidence.slot,
            transaction_sha256: packet_sha256,
            fee_lamports,
            compute_units_consumed,
            completion_contract_sha256: submitted.completion_contract_sha256.clone(),
            poststates,
        },
    )?;
    recorder.write(next)?;
    Ok(finalized.evidence)
}

/// Derive the Finalized journal row for an already-finalized exact packet.
///
/// The campaign owns persistence and must fsync an adjacent Submitted row
/// before calling this helper. This helper performs no signing and no send;
/// it binds the chain packet and exact completion poststates into the next
/// journal row so that checkpoint materialization cannot race ahead of local
/// Finalized durability.
pub(crate) fn finalize_observed_founding_submission_v1(
    rpc: &mut Rpc,
    binding: &FoundingSubmissionBindingV1,
    submitted: &FoundingSubmissionJournalV1,
    finalized: &crate::rpc::FinalizedSignedPacketV1,
) -> Result<FoundingSubmissionJournalV1> {
    if submitted.phase != FoundingSubmissionPhaseV1::Submitted {
        return Err(Error::new(
            "observed founding finalization requires a durably Submitted journal",
        ));
    }
    let poststates = capture_founding_poststates_v1(rpc, submitted)?;
    let packet_sha256 = hex(&Sha256::digest(&finalized.packet));
    let fee_lamports = finalized
        .evidence
        .fee_lamports
        .ok_or_else(|| Error::new("finalized founding transaction omitted fee"))?;
    let compute_units_consumed = finalized
        .evidence
        .compute_units_consumed
        .ok_or_else(|| Error::new("finalized founding transaction omitted compute units"))?;
    finalize_founding_submission_v1(
        binding,
        submitted,
        FoundingFinalizationV1 {
            signature: finalized.evidence.signature.clone(),
            finalized_slot: finalized.evidence.slot,
            transaction_sha256: packet_sha256,
            fee_lamports,
            compute_units_consumed,
            completion_contract_sha256: submitted.completion_contract_sha256.clone(),
            poststates,
        },
    )
}

fn finalize_existing_founding_submission_v1(
    rpc: &mut Rpc,
    label: &str,
    operation: FoundingSubmissionOperationV1,
    recorder: &mut FoundingSubmissionRecorderV1<'_>,
    authenticate_completion: &mut dyn FnMut(&mut Rpc) -> Result<()>,
) -> Result<Option<TransactionEvidence>> {
    let Some(current) = recorder.current(operation).cloned() else {
        return Ok(None);
    };
    match founding_submission_recovery_v1(&recorder.binding, &current)? {
        FoundingSubmissionRecoveryV1::Complete => {
            authenticate_completion(rpc)?;
            authenticate_completed_founding_submission_v1(rpc, label, &recorder.binding, &current)
                .map(Some)
        }
        FoundingSubmissionRecoveryV1::ResendIdenticalPacket
        | FoundingSubmissionRecoveryV1::PollOnly => {
            let signature = current
                .expected_signature
                .as_deref()
                .ok_or_else(|| Error::new("ambiguous founding journal omitted signature"))?
                .parse::<Signature>()
                .map_err(|error| Error::new(format!("ambiguous founding signature: {error}")))?;
            let finalized = rpc
                .finalized_signed_packet(label, signature, false)?
                .ok_or_else(|| {
                    Error::new(format!(
                        "{} signature {signature} remains ambiguous; recovery is exact-signature poll-only",
                        operation.label()
                    ))
                })?;
            finish_durable_founding_v1(rpc, finalized, current, recorder, authenticate_completion)
                .map(Some)
        }
        FoundingSubmissionRecoveryV1::BeginDispatch => Err(Error::new(format!(
            "{} recovery reached Prepared before any dispatch after chain completion",
            operation.label()
        ))),
        FoundingSubmissionRecoveryV1::SignOnce => Err(Error::new(format!(
            "{} recovery reached an unsigned Planned journal after chain completion",
            operation.label()
        ))),
    }
}

fn capture_founding_poststates_v1(
    rpc: &mut Rpc,
    journal: &FoundingSubmissionJournalV1,
) -> Result<Vec<AccountEvidence>> {
    let mut addresses = journal
        .completion_accounts
        .iter()
        .map(|value| {
            value
                .parse::<Pubkey>()
                .map_err(|error| Error::new(format!("founding completion account: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.len() != journal.completion_accounts.len() {
        return Err(Error::new("founding completion account set was duplicated"));
    }
    addresses
        .into_iter()
        .map(|address| {
            rpc.required_account(address, "founding finalized poststate")
                .map(|account| account_evidence(address, &account))
        })
        .collect()
}

pub(crate) fn authenticate_completed_founding_submission_v1(
    rpc: &mut Rpc,
    label: &str,
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<TransactionEvidence> {
    let expected_poststates = founding_submission_finalized_poststates_v1(binding, journal)?;
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| Error::new("Finalized founding journal omitted signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("Finalized founding signature: {error}")))?;
    let finalized = rpc
        .finalized_signed_packet(label, signature, false)?
        .ok_or_else(|| Error::new("persisted finalized founding transaction disappeared"))?;
    let packet_sha256 = hex(&Sha256::digest(&finalized.packet));
    if journal.finalized_slot != Some(finalized.evidence.slot)
        || journal.transaction_sha256.as_deref() != Some(packet_sha256.as_str())
        || journal.fee_lamports != finalized.evidence.fee_lamports
        || journal.compute_units_consumed != finalized.evidence.compute_units_consumed
    {
        return Err(Error::new(
            "persisted finalized founding slot, packet, fee, or compute units changed from chain",
        ));
    }
    let observed_poststates = capture_founding_poststates_v1(rpc, journal)?;
    if observed_poststates != expected_poststates {
        return Err(Error::new(
            "finalized founding poststates changed from chain-derived evidence",
        ));
    }
    Ok(finalized.evidence)
}

/// Reopen one immutable finalized transaction after a later suffix has
/// legitimately changed its completion accounts.
///
/// The Finalized journal already captured those poststates at the adjacent
/// transition. Recovery therefore reauthenticates the journal and immutable
/// chain packet/metadata, but deliberately does not pretend the old
/// poststates should still be the live terminal state.
fn authenticate_historical_founding_transaction_v1(
    rpc: &mut Rpc,
    label: &str,
    operation: FoundingSubmissionOperationV1,
    recorder: &FoundingSubmissionRecorderV1<'_>,
) -> Result<TransactionEvidence> {
    let journal = recorder
        .current(operation)
        .ok_or_else(|| Error::new(format!("{} durable journal is absent", operation.label())))?;
    authenticate_founding_submission_v1(&recorder.binding, journal)?;
    if journal.operation != operation || journal.phase != FoundingSubmissionPhaseV1::Finalized {
        return Err(Error::new(format!(
            "{} historical recovery requires its exact Finalized journal",
            operation.label()
        )));
    }
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| Error::new("Finalized historical journal omitted signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("Finalized historical signature: {error}")))?;
    let finalized = rpc
        .finalized_signed_packet(label, signature, false)?
        .ok_or_else(|| Error::new("persisted historical transaction disappeared"))?;
    let packet_sha256 = hex(&Sha256::digest(&finalized.packet));
    if journal.finalized_slot != Some(finalized.evidence.slot)
        || journal.transaction_sha256.as_deref() != Some(packet_sha256.as_str())
        || journal.fee_lamports != finalized.evidence.fee_lamports
        || journal.compute_units_consumed != finalized.evidence.compute_units_consumed
    {
        return Err(Error::new(
            "persisted historical slot, packet, fee, or compute units changed from chain",
        ));
    }
    Ok(finalized.evidence)
}

fn finalized_founding_routing_table_keys_v1(
    recorder: &FoundingSubmissionRecorderV1<'_>,
    operation: FoundingSubmissionOperationV1,
) -> Result<Vec<Pubkey>> {
    let journal = recorder
        .current(operation)
        .ok_or_else(|| Error::new(format!("{} journal is absent", operation.label())))?;
    authenticate_founding_submission_v1(&recorder.binding, journal)?;
    if journal.phase != FoundingSubmissionPhaseV1::Finalized {
        return Err(Error::new(format!(
            "{} routing recovery requires its Finalized journal",
            operation.label()
        )));
    }
    let VersionedMessage::V0(message) = founding_submission_message_v1(&recorder.binding, journal)?
    else {
        return Err(Error::new(
            "founding routing recovery requires a v0 message",
        ));
    };
    let keys = message
        .address_table_lookups
        .iter()
        .map(|lookup| lookup.account_key)
        .collect::<Vec<_>>();
    let mut unique = keys.clone();
    unique.sort_unstable();
    unique.dedup();
    if keys.len() != 1 || unique != keys {
        return Err(Error::new(
            "founding routing recovery requires one exact nonaliased table",
        ));
    }
    Ok(keys)
}

pub(crate) fn founding_account_set_digest_v1(
    rpc: &mut Rpc,
    addresses: &[Pubkey],
) -> Result<String> {
    let mut keys = addresses.to_vec();
    keys.sort_unstable();
    keys.dedup();
    if keys.len() != addresses.len() || keys.is_empty() {
        return Err(Error::new(
            "founding prestate account set was empty or duplicated",
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/founding-submission-prestate/v1");
    for key in keys {
        hasher.update(key.as_ref());
        match rpc.account(key)? {
            None => hasher.update([0]),
            Some(account) => {
                hasher.update([1]);
                hasher.update(account.owner.as_ref());
                hasher.update(account.lamports.to_le_bytes());
                hasher.update([u8::from(account.executable)]);
                hasher.update((account.data.len() as u64).to_le_bytes());
                hasher.update(Sha256::digest(&account.data));
            }
        }
    }
    Ok(hex(&hasher.finalize()))
}

fn founding_completion_contract_v1(
    operation: FoundingSubmissionOperationV1,
    addresses: &[Pubkey],
) -> Result<String> {
    let mut keys = addresses.to_vec();
    keys.sort_unstable();
    keys.dedup();
    if keys.len() != addresses.len() || keys.is_empty() {
        return Err(Error::new(
            "founding completion account set was empty or duplicated",
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/founding-submission-completion/v1");
    hasher.update(operation.label().as_bytes());
    for key in keys {
        hasher.update(key.as_ref());
    }
    Ok(hex(&hasher.finalize()))
}

fn founding_instruction_account_digest_v1(payer: Pubkey, instruction: &Instruction) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/founding-submission-resolved-accounts/v1");
    hasher.update(payer.as_ref());
    hasher.update(instruction.program_id.as_ref());
    for account in &instruction.accounts {
        hasher.update(account.pubkey.as_ref());
        hasher.update([u8::from(account.is_signer), u8::from(account.is_writable)]);
    }
    hex(&hasher.finalize())
}

fn funding_readiness_instruction_digest_v1(
    payer: Pubkey,
    instructions: &[Instruction],
    routing_tables: &[ObservedAccount],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/funding-readiness-resolved-instructions/v1");
    hasher.update(payer.as_ref());
    hasher.update((instructions.len() as u64).to_le_bytes());
    for instruction in instructions {
        hasher.update(instruction.program_id.as_ref());
        hasher.update((instruction.accounts.len() as u64).to_le_bytes());
        for account in &instruction.accounts {
            hasher.update(account.pubkey.as_ref());
            hasher.update([u8::from(account.is_signer), u8::from(account.is_writable)]);
        }
        hasher.update((instruction.data.len() as u64).to_le_bytes());
        hasher.update(&instruction.data);
    }
    hasher.update((routing_tables.len() as u64).to_le_bytes());
    for table in routing_tables {
        hasher.update(table.key.as_ref());
        hasher.update(table.owner.as_ref());
        hasher.update(table.lamports.to_le_bytes());
        hasher.update([u8::from(table.executable)]);
        hasher.update((table.data.len() as u64).to_le_bytes());
        hasher.update(Sha256::digest(&table.data));
        hasher.update(table.observation.slot.to_le_bytes());
        hasher.update(table.observation.unix_timestamp.to_le_bytes());
    }
    hex(&hasher.finalize())
}

fn materialize_founding_checkpoint_v1(
    rpc: &mut Rpc,
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
    expected_operation: FoundingSubmissionOperationV1,
    expected_checkpoint_schema: &str,
    mut checkpoint: MarketExecutionCheckpointV1,
    completion_accounts: BTreeMap<String, String>,
) -> Result<MarketExecutionCheckpointV1> {
    authenticate_founding_submission_v1(binding, journal)?;
    if journal.operation != expected_operation
        || journal.phase != FoundingSubmissionPhaseV1::Finalized
        || checkpoint.schema != expected_checkpoint_schema
        || completion_accounts.is_empty()
    {
        return Err(Error::new(format!(
            "{} checkpoint recovery requires its exact Finalized journal and schema {}",
            expected_operation.label(),
            expected_checkpoint_schema,
        )));
    }
    let mut unique = completion_accounts
        .values()
        .map(|value| {
            value.parse::<Pubkey>().map_err(|error| {
                Error::new(format!(
                    "{} recovery account: {error}",
                    expected_operation.label()
                ))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    unique.sort_unstable();
    unique.dedup();
    let mut journal_accounts = journal
        .completion_accounts
        .iter()
        .map(|value| {
            value.parse::<Pubkey>().map_err(|error| {
                Error::new(format!(
                    "{} journal account: {error}",
                    expected_operation.label()
                ))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    journal_accounts.sort_unstable();
    if unique != journal_accounts
        || founding_completion_contract_v1(expected_operation, &unique)?
            != journal.completion_contract_sha256
    {
        return Err(Error::new(format!(
            "{} recovery account set changed from its completion contract",
            expected_operation.label()
        )));
    }
    let finalized_poststates = founding_submission_finalized_poststates_v1(binding, journal)?;
    let expected_by_address = finalized_poststates
        .into_iter()
        .map(|evidence| (evidence.address.clone(), evidence))
        .collect::<BTreeMap<_, _>>();
    if expected_by_address.len() != journal_accounts.len() {
        return Err(Error::new(format!(
            "{} finalized poststate set did not cover its completion contract",
            expected_operation.label()
        )));
    }
    for (label, address) in completion_accounts {
        if checkpoint.accounts.contains_key(&label) {
            return Err(Error::new(format!(
                "{} recovery attempted to overwrite checkpoint label {label}",
                expected_operation.label()
            )));
        }
        let address = address.parse::<Pubkey>().map_err(|error| {
            Error::new(format!(
                "{} recovery account: {error}",
                expected_operation.label()
            ))
        })?;
        let account = rpc.required_account(address, &label)?;
        let observed = account_evidence(address, &account);
        if expected_by_address.get(&address.to_string()) != Some(&observed) {
            return Err(Error::new(format!(
                "{} recovery account {label} changed after journal finalization",
                expected_operation.label()
            )));
        }
        checkpoint.accounts.insert(label, observed);
    }
    Ok(checkpoint)
}

/// Rebuild the already-finalized DCLTCFQ1 Prepared checkpoint from its
/// immutable Planned payload and the exact Finalized poststates.
pub(crate) fn materialize_dcltcfq1_checkpoint_v1(
    rpc: &mut Rpc,
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<MarketExecutionCheckpointV1> {
    let payload = founding_submission_recovery_payload_v1(binding, journal)?;
    let payload: Dcltcfq1RecoveryPayloadV1 = serde_json::from_slice(&payload)
        .map_err(|error| Error::new(format!("DCLTCFQ1 recovery payload: {error}")))?;
    if payload.schema != DCLTCFQ1_RECOVERY_PAYLOAD_SCHEMA_V1 {
        return Err(Error::new("DCLTCFQ1 recovery payload identity changed"));
    }
    materialize_founding_checkpoint_v1(
        rpc,
        binding,
        journal,
        FoundingSubmissionOperationV1::Dcltcfq1,
        DCLTCFQ1_PREPARED_CHECKPOINT_SCHEMA_V1,
        payload.checkpoint,
        payload.completion_accounts,
    )
}

/// Rebuild the already-finalized DCLTPCB2 checkpoint from its Planned payload
/// and the exact accounts now visible on chain. The normal checkpoint resume
/// subsequently re-derives every coordinate and runs the full projected-
/// Custody verifier before DCLTGMF3 can be constructed.
pub(crate) fn materialize_dcltpcb2_checkpoint_v1(
    rpc: &mut Rpc,
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<MarketExecutionCheckpointV1> {
    let payload = founding_submission_recovery_payload_v1(binding, journal)?;
    let payload: Dcltpcb2RecoveryPayloadV1 = serde_json::from_slice(&payload)
        .map_err(|error| Error::new(format!("DCLTPCB2 recovery payload: {error}")))?;
    if payload.schema != DCLTPCB2_RECOVERY_PAYLOAD_SCHEMA_V1 {
        return Err(Error::new("DCLTPCB2 recovery payload identity changed"));
    }
    materialize_founding_checkpoint_v1(
        rpc,
        binding,
        journal,
        FoundingSubmissionOperationV1::Dcltpcb2,
        DCLTPCB2_CHECKPOINT_SCHEMA_V1,
        payload.checkpoint,
        payload.completion_accounts,
    )
}

/// Publication facts selected by one already-authenticated Source graph.
///
/// This is the one semantic join used by both input validation and Registry
/// publication. In particular, the sponsored release is not a second caller
/// DTO: the SourceSpec selects the access profile, the ProviderRelease names
/// the release body, and this join proves all four immutable provider links
/// before the publisher can create any record.
struct SourcePublicationContractV1 {
    adapter_config_schema: [u8; 32],
    sponsored_release: Option<Vec<u8>>,
}

fn authenticate_source_publication_v1(
    input: &MarketRunInput,
) -> Result<SourcePublicationContractV1> {
    let source_spec_bytes = decode_hex(&input.source_spec_hex)?;
    let source_spec = SourceSpecV1::decode(&source_spec_bytes)
        .map_err(|error| Error::new(format!("SourceSpecV1: {error:?}")))?;
    if source_spec.to_bytes().as_slice() != source_spec_bytes {
        return Err(Error::new("SourceSpecV1 input was not canonical"));
    }
    let provider_release_bytes = decode_hex(&input.provider_release_hex)?;
    let adapter_config_bytes = decode_hex(&input.pyth_adapter_config_hex)?;
    if record_identity(&provider_release_bytes) != source_spec.provider_release_id().to_bytes() {
        return Err(Error::new(
            "the provider release body is not the one the source spec names",
        ));
    }
    if record_identity(&adapter_config_bytes) != source_spec.adapter_config_id().to_bytes() {
        return Err(Error::new(
            "the Pyth adapter configuration body is not the one the source spec names",
        ));
    }

    let sponsored_release_bytes = decode_hex(&input.pyth_sponsored_push_release_hex)?;
    match source_spec.access_profile() {
        SourceAccessProfile::PythTerminalOneTransaction => {
            if !sponsored_release_bytes.is_empty() {
                return Err(Error::new(
                    "a terminal Pyth source must not carry a sponsored push release",
                ));
            }
            Ok(SourcePublicationContractV1 {
                adapter_config_schema: PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
                sponsored_release: None,
            })
        }
        SourceAccessProfile::RelayedObservationRecord => {
            if !sponsored_release_bytes.is_empty() {
                return Err(Error::new(
                    "a relayed source must not carry a sponsored push release",
                ));
            }
            Ok(SourcePublicationContractV1 {
                adapter_config_schema: dclutch_registry_contract::ARTIFACT_RELEASE_SCHEMA_ID_V1,
                sponsored_release: None,
            })
        }
        SourceAccessProfile::PythSponsoredPushSnapshot => {
            if sponsored_release_bytes.is_empty() {
                return Err(Error::new(
                    "a sponsored push source must carry its exact release body",
                ));
            }
            let provider_release = ProviderReleaseV1::decode(&provider_release_bytes)
                .map_err(|error| Error::new(format!("ProviderReleaseV1: {error:?}")))?;
            if provider_release.to_bytes().as_slice() != provider_release_bytes {
                return Err(Error::new("ProviderReleaseV1 input was not canonical"));
            }
            let sponsored_release = PythSponsoredPushReleaseV1::decode(&sponsored_release_bytes)
                .map_err(|error| Error::new(format!("PythSponsoredPushReleaseV1: {error:?}")))?;
            if sponsored_release.to_bytes().as_slice() != sponsored_release_bytes {
                return Err(Error::new(
                    "PythSponsoredPushReleaseV1 input was not canonical",
                ));
            }
            let adapter_config = PythAdapterConfigV1::decode(&adapter_config_bytes)
                .map_err(|error| Error::new(format!("PythAdapterConfigV1: {error:?}")))?;
            if adapter_config.to_bytes().as_slice() != adapter_config_bytes {
                return Err(Error::new("PythAdapterConfigV1 input was not canonical"));
            }
            if provider_release.provider_deployment_release_id().to_bytes()
                != record_identity(&sponsored_release_bytes)
                || provider_release.provider_family_id().to_bytes()
                    != sponsored_release.provider_family_id()
                || provider_release.adapter_release_id().to_bytes()
                    != sponsored_release.adapter_id()
                || provider_release.decoding_rules_id().to_bytes()
                    != sponsored_release.price_update_codec_id()
                || provider_release.transport_profile_id().to_bytes()
                    != sponsored_release.transport_profile_id()
                || adapter_config.provider_feed_id() != sponsored_release.feed_id()
            {
                return Err(Error::new(
                    "the Source/Provider/adapter/sponsored release join changed",
                ));
            }
            Ok(SourcePublicationContractV1 {
                adapter_config_schema: PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
                sponsored_release: Some(sponsored_release_bytes),
            })
        }
        SourceAccessProfile::SharedObservationChild => Err(Error::new(
            "the source spec names an access profile this publisher has no adapter-config schema for: SharedObservationChild",
        )),
    }
}

pub(crate) fn validate_market_input(input: &MarketRunInput) -> Result<()> {
    if input.initial_collateral_atoms == 0
        || input.cut_denominator == 0
        || input.portfolio_denominator == 0
    {
        return Err(Error::new(
            "market input requires positive raw collateral and denominators",
        ));
    }
    if !matches!(
        input.local_participant_fixture_liquidity_atoms,
        0 | LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1
    ) {
        return Err(Error::new(format!(
            "local participant fixture liquidity must be absent or exactly \
             {LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1} atoms; hidden multipliers and \
             caller-chosen fixture supply are refused"
        )));
    }
    let cuts = input
        .cuts
        .iter()
        .map(|value| canonical_i128(value))
        .collect::<Result<Vec<_>>>()?;
    if input.coefficients.len()
        != cuts
            .len()
            .checked_add(2)
            .ok_or_else(|| Error::new("Product outcome width overflow"))?
    {
        return Err(Error::new(
            "portfolio coefficient width must equal cuts + failure + tails",
        ));
    }
    for value in [
        &input.product_id,
        &input.coordinate_domain_id,
        &input.result_unit_id,
        &input.claim_basis_id,
        &input.liability_basis_id,
        &input.representation_release_id,
        &input.mapping_release_id,
    ] {
        let _ = product_id(value)?;
    }
    for value in [
        &input.primary_source_spec_id,
        &input.window_spec_id,
        &input.statistic_spec_id,
        &input.failure_policy_release_id,
    ] {
        let _ = source_id(value)?;
    }
    // Three of those four identities name a record body this spec also
    // carries, and the identity of a finalized record IS the SHA-256 of its
    // body. Checking that here rather than trusting the pair is the whole
    // point of carrying the bodies: an identity that is not its body's digest
    // names a record that can never be published, which is the defect this
    // check exists to make impossible to reintroduce. The provider release and
    // adapter configuration are named from INSIDE the source spec rather than
    // at top level, so they are checked by decoding the source spec below.
    for (label, identity, body) in [
        (
            "primary source spec",
            &input.primary_source_spec_id,
            &input.source_spec_hex,
        ),
        ("window spec", &input.window_spec_id, &input.window_spec_hex),
        (
            "statistic spec",
            &input.statistic_spec_id,
            &input.statistic_spec_hex,
        ),
    ] {
        let bytes = decode_hex(body)?;
        if bytes.is_empty() {
            return Err(Error::new(format!(
                "the run spec names a {label} and carries no body for it"
            )));
        }
        if record_identity(&bytes) != source_id(identity)?.to_bytes() {
            return Err(Error::new(format!(
                "the {label} identity is not the SHA-256 of the {label} body this spec carries, so                  it names a finalized record that can never exist"
            )));
        }
    }
    let _ = authenticate_source_publication_v1(input)?;
    let recovery_bytes = decode_hex(&input.recovery_policy_hex)?;
    // Empty means the material carries NO recovery policy: the deliberate
    // section-12.8 demo shape, admitted on chain at e5b6923 and decided in
    // MAINNET_STATE_RELAY.md section 13. Non-empty keeps the original rule.
    if !recovery_bytes.is_empty() {
        let recovery = RecoveryPolicyV2::decode(&recovery_bytes)
            .map_err(|error| Error::new(format!("RecoveryPolicyV2: {error:?}")))?;
        if recovery.to_bytes().as_slice() != recovery_bytes {
            return Err(Error::new("RecoveryPolicyV2 input was not canonical"));
        }
    }
    let manifest = decode_hex(&input.capability_manifest_hex)?;
    let manifest = CapabilityManifestV1::decode(&manifest)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
    if manifest.entry_count() != 4 {
        return Err(Error::new(
            "capability manifest must contain one Direct entry and three Resolution companions",
        ));
    }
    validate_direct_market_capability_v1(input)?;
    // The Product's `liability_basis_id` is the semantic identity of a real
    // published `ProductBasisV3`. Founding reads that record and refuses any
    // Product whose declared liability basis is not the one it links, so the
    // run spec has to carry the exact record and not merely an opaque digest.
    let basis = decode_hex(&input.linked_basis_hex)?;
    let semantic = semantic_basis_identity_v3(&basis)?;
    if semantic != product_id(&input.liability_basis_id)?.to_bytes() {
        return Err(Error::new(
            "linked liability basis record is not the Product's declared liability basis",
        ));
    }
    Ok(())
}

/// Exact semantic identity of one canonical `ProductBasisV3` record.
///
/// The semantic preimage deliberately omits the Product and result-domain
/// links, so this identity exists before the Product that will declare it.
/// That omission is what makes the join acyclic rather than a fixed point.
pub(crate) fn semantic_basis_identity_v3(bytes: &[u8]) -> Result<[u8; 32]> {
    let preimage = semantic_basis_preimage_v3(bytes)
        .map_err(|error| Error::new(format!("ProductBasisV3: {error:?}")))?;
    let mut hasher = Sha256::new();
    hasher.update(SEMANTIC_BASIS_CONTENT_DOMAIN_V3);
    hasher.update(preimage.prefix());
    hasher.update(preimage.suffix());
    Ok(hasher.finalize().into())
}

/// Compile the exact categorical liability basis one Product outcome vector
/// determines.
///
/// `CategoricalQ1` is the only shape a categorical Product admits: unit payout
/// scale, no knots, no graded terms, and one basis claim per outcome. Nothing
/// here is a free parameter except the two acyclic links, and both are checked
/// against the compiled Product graph before publication.
pub(crate) fn compile_linked_basis_v3(
    product_id: [u8; 32],
    result_domain_id: [u8; 32],
    coordinate_domain_id: [u8; 32],
    result_unit_id: [u8; 32],
    evaluator_release_id: [u8; 32],
    outcome_count: usize,
) -> Result<Vec<u8>> {
    let basis_width =
        u32::try_from(outcome_count).map_err(|_| Error::new("Product outcome width overflow"))?;
    let width = basis_record_bytes_v3(BasisKindV3::CategoricalQ1, outcome_count, 0, 0)
        .map_err(|error| Error::new(format!("ProductBasisV3 width: {error:?}")))?;
    let mut bytes = vec![0_u8; width];
    compile_basis_v3(
        BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id,
            result_domain_id,
            coordinate_domain_id,
            result_unit_id,
            evaluator_release_id,
            basis_width,
            payout_scale: 1,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
        },
        &mut bytes,
    )
    .map_err(|error| Error::new(format!("canonical ProductBasisV3 compiler: {error:?}")))?;
    Ok(bytes)
}

#[derive(Clone)]
struct MarketRecords {
    realm: PublishedRecord,
    product: PublishedRecord,
    domain: PublishedRecord,
    portfolio: PublishedRecord,
    source: PublishedRecord,
    source_capacity_profile: PublishedRecord,
    manipulation_floor: Option<PublishedRecord>,
    recovery: Option<PublishedRecord>,
    manifest: PublishedRecord,
    basis: PublishedRecord,
    /// The five source-graph records both provider legs authenticate. They are
    /// published with the rest of the graph rather than left to a resolution
    /// campaign, because the Market's `SourceMaterialV2` NAMES them and a
    /// Market that names records nobody published is a Market that cannot
    /// resolve.
    source_spec: PublishedRecord,
    window_spec: PublishedRecord,
    statistic_spec: PublishedRecord,
    provider_release: PublishedRecord,
    adapter_config: PublishedRecord,
    sponsored_push_release: Option<PublishedRecord>,
    /// Exact Registry closure selected by the manifest's Direct entry.
    direct: BTreeMap<&'static str, PublishedRecord>,
    principal_cap_sets: u64,
}

struct FinalizedSnapshot {
    slot: u64,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

impl FinalizedSnapshot {
    fn observation(&self, key: Pubkey) -> Result<AccountObservationV2<'_>> {
        match self.accounts.get(&key) {
            Some(Some(account)) => Ok(AccountObservationV2 {
                slot: self.slot,
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: &account.data,
            }),
            Some(None) => Ok(AccountObservationV2 {
                slot: self.slot,
                key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: &[],
            }),
            None => Err(Error::new(format!("finalized snapshot omitted {key}"))),
        }
    }

    fn finalized_record(
        &self,
        rpc: &mut Rpc,
        pair: PublishedRecord,
    ) -> Result<FinalizedRecordObservationV2<'_>> {
        let raw = self.observation(pair.raw)?;
        let staging = self.observation(pair.staging)?;
        Ok(FinalizedRecordObservationV2 {
            raw,
            staging,
            raw_rent_minimum: rpc.minimum_balance(raw.data.len())?,
        })
    }
}

pub(crate) fn execute_found_market(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<MarketExecutionEvidence> {
    let actors = FoundingActorsV1::new(
        forge.keypair(role::FOUNDING_FOUNDER).pubkey(),
        forge.keypair(role::SUBSTITUTED_FOUNDER).pubkey(),
    )?;
    execute_found_market_with_checkpoint_and_journal(
        rpc,
        plan,
        input,
        payer,
        forge,
        actors,
        transactions,
        &mut |_| Ok(()),
        None,
    )
}

pub(crate) fn execute_found_market_with_checkpoint(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    transactions: &mut Vec<TransactionEvidence>,
    checkpoint: &mut dyn FnMut(&MarketExecutionCheckpointV1) -> Result<()>,
) -> Result<MarketExecutionEvidence> {
    let actors = FoundingActorsV1::new(
        forge.keypair(role::FOUNDING_FOUNDER).pubkey(),
        forge.keypair(role::SUBSTITUTED_FOUNDER).pubkey(),
    )?;
    execute_found_market_with_checkpoint_and_journal(
        rpc,
        plan,
        input,
        payer,
        forge,
        actors,
        transactions,
        checkpoint,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_found_market_with_checkpoint_and_journal(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    transactions: &mut Vec<TransactionEvidence>,
    checkpoint: &mut dyn FnMut(&MarketExecutionCheckpointV1) -> Result<()>,
    mut submission_recorder: Option<&mut FoundingSubmissionRecorderV1<'_>>,
) -> Result<MarketExecutionEvidence> {
    validate_market_input(input)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let created_collateral = create_real_collateral(
        rpc,
        payer,
        forge,
        token_program,
        input.collateral_display_decimals,
        input.initial_collateral_atoms,
        input.local_participant_fixture_liquidity_atoms,
        transactions,
    )?;
    let mint = created_collateral.mint;
    let collateral_wallet = created_collateral.wallet;
    let local_participant_fixture_liquidity =
        created_collateral.local_participant_fixture_liquidity;

    let release_set_digest = hex32(&plan.release_set_id)?;
    let founding_targets = derive_founding_targets(plan, input, mint)?;
    let (records, product_id) = publish_market_records(
        rpc,
        registry,
        input,
        mint,
        founding_targets.open_market,
        release_set_digest,
        payer,
        transactions,
    )?;
    let market_identity = MarketIdentity {
        market_id: identity([0xff; 32])?,
        realm_id: identity(records.realm.digest)?,
        product_record: identity(records.product.digest)?,
        product_id: identity(product_id.to_bytes())?,
        resolution_policy: identity(records.source.digest)?,
        capability_manifest: identity(records.manifest.digest)?,
        selected_release_set: identity(release_set_digest)?,
        registry_program: identity(registry.to_bytes())?,
        generation: input.generation,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
        &core,
    )
    .0;
    if market != founding_targets.found31_market {
        return Err(Error::new(
            "published Market graph moved from its pre-publication derivation",
        ));
    }
    let credit = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &input.generation.to_le_bytes(),
        ],
        &rent_program,
    )
    .0;
    let keys = found_snapshot_keys(plan, payer.pubkey(), market, credit, &records)?;
    let minimum_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .ok_or_else(|| Error::new("market execution had no finalized predecessor"))?;
    let pre_credit = finalized_snapshot(rpc, &keys, minimum_slot)?;
    let projection_state =
        projection_state(rpc, plan, &pre_credit, payer.pubkey(), market, &records)?;
    let projection = project_found_v2(input.generation, projection_state)
        .map_err(|error| Error::new(format!("chain-derived Found projection: {error:?}")))?;
    if projection.market_address != market {
        return Err(Error::new(
            "Found projection changed the discovered Market address",
        ));
    }
    let create = build_lifecycle_rent_create_v2(
        &projection,
        LifecycleRentCreateStateV2 {
            payer: pre_credit.observation(payer.pubkey())?,
            credit_destination: pre_credit.observation(credit)?,
            refund_wallet: pre_credit.observation(payer.pubkey())?,
            rent_program: pre_credit.observation(rent_program)?,
            system_program: pre_credit.observation(system_program::ID)?,
            rent: pre_credit.observation(sysvar::rent::ID)?,
        },
    )
    .map_err(|error| Error::new(format!("chain-derived RentV2 Create: {error:?}")))?;
    transactions.push(rpc.send(
        "create Market-scoped lifecycle RentCreditV2",
        std::slice::from_ref(&create.instruction),
        payer,
    )?);
    let credit_account = rpc.required_account(credit, "created lifecycle RentCreditV2")?;
    let credit_state = LifecycleRentCreditV2::decode(&credit_account.data)
        .map_err(|error| Error::new(format!("created RentV2 state: {error:?}")))?;
    if credit_account.owner != rent_program
        || credit_account.executable
        || credit_account.data.len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || credit_state != create.state
        || credit_account.lamports < create.rent_debit
    {
        return Err(Error::new(
            "RentV2 transaction poststate differed from its checked plan",
        ));
    }

    let post_credit = finalized_snapshot(
        rpc,
        &keys,
        transactions
            .last()
            .map(|transaction| transaction.slot)
            .ok_or_else(|| Error::new("RentV2 transaction omitted finalized slot"))?,
    )?;
    let state = found_state(
        rpc,
        plan,
        &post_credit,
        payer.pubkey(),
        market,
        credit,
        &records,
    )?;
    let found = build_found_instruction_v2(input.generation, state)
        .map_err(|error| Error::new(format!("chain-derived Found37: {error:?}")))?;
    // The canonical 31-account Found frame does not fit the 1,232-byte legacy
    // packet with its keys inline. Routing is table data, never authority: the
    // shared versioned-message operator owns table admission and geometry.
    let (routing, tables) = publish_routing_table(
        rpc,
        payer,
        "Found37",
        std::slice::from_ref(&found.instruction),
        transactions,
    )?;
    let mut hostile = found.instruction.clone();
    hostile
        .accounts
        .get_mut(2)
        .ok_or_else(|| Error::new("Found37 omitted RentCredit coordinate"))?
        .pubkey = payer.pubkey();
    transactions.push(rpc.send_v0_expected_failure(
        "Found37 refuses substituted lifecycle credit",
        &[hostile],
        payer,
        routing,
        &tables,
    )?);
    if rpc.account(market)?.is_some() {
        return Err(Error::new("hostile Found37 left a Market account"));
    }

    // Routing data is not authority. The Market address is derived from the
    // immutable identity, so substituting it under an attacker-chosen table
    // must refuse and must roll the whole multi-instruction transaction back
    // to a fee-only debit.
    let rollback_recipient = crate::seed::fresh_probe_address();
    let substituted_market_key = crate::seed::fresh_probe_address();
    let mut substituted_market = found.instruction.clone();
    substituted_market
        .accounts
        .get_mut(1)
        .ok_or_else(|| Error::new("Found37 omitted the Market coordinate"))?
        .pubkey = substituted_market_key;
    let rolled_back = rpc.send_v0_expected_failure(
        "Found37 refuses a substituted Market coordinate and rolls the transaction back",
        &[
            transfer(&payer.pubkey(), &rollback_recipient, 1),
            substituted_market,
        ],
        payer,
        routing,
        &tables,
    )?;
    // The rollback property is read from the refused transaction's OWN
    // balance record — one atomic statement the chain wrote — rather than
    // separate before/after account reads that race a load-balanced
    // endpoint's replicas. It also covers every account the transaction
    // touched, the probe recipient included, not just three of them.
    let recipient_exists = rpc.account(rollback_recipient)?.is_some();
    let market_exists = rpc.account(market)?.is_some();
    let fee_only = rolled_back.fee_only_balance_change;
    if recipient_exists || market_exists || fee_only != Some(true) {
        return Err(Error::new(format!(
            "refused Found37 did not roll its whole transaction back to a fee-only debit: \
             recipient_exists={recipient_exists} market_exists={market_exists} \
             fee_only_balance_change={fee_only:?} (rolled-back signature {})",
            rolled_back.signature
        )));
    }

    transactions.push(rolled_back);
    transactions.push(rpc.send_v0(
        "create canonical Found37 Market",
        &[found.instruction],
        payer,
        routing,
        &tables,
    )?);
    let market_account = rpc.required_account(market, "Found37 Market")?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Found37 Market state: {error:?}")))?;
    if market_account.owner != core
        || market_account.executable
        || market_state.phase != Phase::Founding
        || market_state.identity != found.market_identity
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
    {
        return Err(Error::new(
            "Found37 transaction poststate differed from its checked plan",
        ));
    }

    let mut accounts = BTreeMap::new();
    for (label, key) in [
        ("collateral_mint", mint),
        ("collateral_wallet", collateral_wallet),
        ("lifecycle_rent_credit", credit),
        ("market", market),
        ("realm_record", records.realm.raw),
        ("product_record", records.product.raw),
        ("result_domain_record", records.domain.raw),
        ("portfolio_record", records.portfolio.raw),
        ("source_material_record", records.source.raw),
        ("capability_manifest_record", records.manifest.raw),
        ("source_spec_record", records.source_spec.raw),
        (
            "source_capacity_profile_record",
            records.source_capacity_profile.raw,
        ),
        ("window_spec_record", records.window_spec.raw),
        ("statistic_spec_record", records.statistic_spec.raw),
        ("provider_release_record", records.provider_release.raw),
        ("pyth_adapter_config_record", records.adapter_config.raw),
        ("linked_liability_basis_record", records.basis.raw),
    ] {
        let account = rpc.required_account(key, label)?;
        accounts.insert(label.into(), account_evidence(key, &account));
    }
    if let Some(record) = records.sponsored_push_release {
        let account = rpc.required_account(record.raw, "Pyth sponsored push release record")?;
        accounts.insert(
            "pyth_sponsored_push_release_record".into(),
            account_evidence(record.raw, &account),
        );
    }
    if let Some(fixture) = &local_participant_fixture_liquidity {
        let source = fixture
            .source_token_account
            .parse()
            .map_err(|_| Error::new("local participant fixture source is not a public key"))?;
        let account =
            rpc.required_account(source, "local participant fixture collateral source")?;
        accounts.insert(
            "local_participant_fixture_source".into(),
            account_evidence(source, &account),
        );
    }
    if let Some(floor) = records.manipulation_floor {
        let account = rpc.required_account(floor.raw, "manipulation_floor_record")?;
        accounts.insert(
            "manipulation_floor_record".into(),
            account_evidence(floor.raw, &account),
        );
    }
    // A no-recovery material published no recovery record, and the evidence
    // says so by absence rather than by a placeholder address.
    if let Some(recovery) = &records.recovery {
        let account = rpc.required_account(recovery.raw, "recovery_policy_record")?;
        accounts.insert(
            "recovery_policy_record".into(),
            account_evidence(recovery.raw, &account),
        );
    }
    for (&label, record) in &records.direct {
        let account = rpc.required_account(record.raw, label)?;
        accounts.insert(label.into(), account_evidence(record.raw, &account));
    }
    let mut completed = vec![
            "created an exact Token-2022 collateral Mint and funded raw-atom wallet with ephemeral local keys".into(),
            "transaction-published and finalized the canonical Realm/Product/Source/Recovery/Manifest graph".into(),
            "derived Market and lifecycle-credit coordinates from one finalized pre-credit projection".into(),
            "created and reacquired the exact Market-scoped LifecycleRentCreditV2".into(),
            "proved Found37 rejects a substituted lifecycle credit".into(),
            "proved a substituted Market coordinate under attacker-chosen routing refuses and rolls the whole transaction back to a fee-only debit".into(),
            "routed the oversized 31-account Found frame through a finalized address lookup table as a packet-safe v0 transaction".into(),
            "created and verified the canonical Founding Market through the chain-derived Found37 operator".into(),
        ];
    // The generic founding runs at its own generation against a Market that
    // does not exist yet: every projected-Custody stage asserts the inverse of
    // a live Market, so the Found37 Market above cannot be reused.
    let mut founding_custody_context = None;
    // The success driver owns exactly one opening ladder. SourceAbort is a
    // separately named validation lane with its own terminal evidence; it must
    // never make an otherwise-complete public/private founding wait for an
    // expiry or append cleanup transactions to the success receipt.
    for lane in SUCCESS_PRESTATE_LANES_V1 {
        let context = execute_projected_custody_bootstrap(
            rpc,
            plan,
            input,
            &records,
            market_identity,
            product_id,
            mint,
            collateral_wallet,
            market,
            lane,
            None,
            payer,
            forge,
            actors,
            transactions,
            &mut accounts,
            &mut completed,
            local_participant_fixture_liquidity.as_ref(),
            checkpoint,
            submission_recorder.as_deref_mut(),
        )?;
        if lane == PrestateLaneV1::Founding {
            founding_custody_context = Some(context);
        }
    }
    Ok(MarketExecutionEvidence {
        completed,
        accounts,
        founding_custody_context: hex(&founding_custody_context
            .ok_or_else(|| Error::new("founding lane omitted its custody context"))?),
        direct_selected_manifest_entry_index: input
            .direct_capability
            .as_ref()
            .ok_or_else(|| Error::new("founding evidence omitted its Direct payload"))?
            .selected_manifest_entry_index,
        local_participant_fixture_liquidity,
    })
}

struct RecoveredFoundingContextV1 {
    records: MarketRecords,
    coordinates: FoundingCoordinates,
    identity_template: MarketIdentity,
    product: ProductContentId,
    mint: Pubkey,
    collateral_wallet: Pubkey,
    found31_market: Pubkey,
    founder: Pubkey,
    found_record: Pubkey,
    lock_record: Pubkey,
    claim_count: u32,
}

fn checkpoint_account(checkpoint: &MarketExecutionCheckpointV1, label: &str) -> Result<Pubkey> {
    let address = checkpoint
        .accounts
        .get(label)
        .ok_or_else(|| Error::new(format!("DCLTPCB2 checkpoint omitted {label}")))?
        .address
        .as_str();
    pubkey(address)
}

fn authenticate_checkpoint_record_graph(
    rpc: &mut Rpc,
    checkpoint: &MarketExecutionCheckpointV1,
) -> Result<()> {
    let records = checkpoint
        .accounts
        .iter()
        .filter(|(label, _)| label.ends_with("_record"))
        .collect::<Vec<_>>();
    if records.len() < 12 {
        return Err(Error::new(
            "DCLTPCB2 checkpoint omitted the complete published Market record graph",
        ));
    }
    for (label, expected) in records {
        let address = pubkey(&expected.address)?;
        let account = rpc.required_account(address, label)?;
        if account_evidence(address, &account) != *expected {
            return Err(Error::new(format!(
                "DCLTPCB2 checkpoint record {label} changed after its finalized checkpoint"
            )));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_founding_checkpoint_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    transactions: &mut Vec<TransactionEvidence>,
    checkpoint: &MarketExecutionCheckpointV1,
    consume_role_keys: bool,
) -> Result<RecoveredFoundingContextV1> {
    if !matches!(
        checkpoint.schema.as_str(),
        DCLTCFQ1_PREPARED_CHECKPOINT_SCHEMA_V1 | DCLTPCB2_CHECKPOINT_SCHEMA_V1
    ) {
        return Err(Error::new(
            "founding checkpoint is not the resumable DCLTPCB2 checkpoint schema",
        ));
    }
    validate_market_input(input)?;
    authenticate_checkpoint_record_graph(rpc, checkpoint)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    // Reissue the same persisted role indices the original attempt consumed.
    // No fresh or seeded key is admitted on devnet; campaign.rs has already
    // loaded every required role from an explicit file.
    let (mint, wallet) = if consume_role_keys {
        (
            forge.keypair(role::COLLATERAL_MINT).pubkey(),
            forge.keypair(role::COLLATERAL_WALLET).pubkey(),
        )
    } else {
        (
            forge.peek_pubkey(role::COLLATERAL_MINT)?,
            forge.peek_pubkey(role::COLLATERAL_WALLET)?,
        )
    };
    let expected_supply = input
        .initial_collateral_atoms
        .checked_add(input.local_participant_fixture_liquidity_atoms)
        .ok_or_else(|| Error::new("checkpoint collateral supply overflow"))?;
    if checkpoint_account(checkpoint, "collateral_mint")? != mint
        || checkpoint_account(checkpoint, "collateral_wallet")? != wallet
    {
        return Err(Error::new(
            "DCLTPCB2 checkpoint belongs to different collateral role keys",
        ));
    }
    let mint_account = rpc.required_account(mint, "checkpoint collateral Mint")?;
    let wallet_account = rpc.required_account(wallet, "checkpoint collateral wallet")?;
    let parsed_mint = Mint::parse(&mint_account.data)
        .map_err(|error| Error::new(format!("checkpoint collateral Mint: {error:?}")))?;
    let parsed_wallet = TokenAccount::parse(&wallet_account.data)
        .map_err(|error| Error::new(format!("checkpoint collateral wallet: {error:?}")))?;
    if mint_account.owner != token_program
        || wallet_account.owner != token_program
        || parsed_mint.decimals != input.collateral_display_decimals
        || parsed_mint.supply != expected_supply
        || !parsed_mint.is_initialized
        || !parsed_mint.mint_authority.is_none()
        || !parsed_mint.freeze_authority.is_none()
        || parsed_wallet.mint != mint.to_bytes()
        || parsed_wallet.owner != payer.pubkey().to_bytes()
        || parsed_wallet.state != AccountState::Initialized
    {
        return Err(Error::new(
            "DCLTPCB2 checkpoint collateral roles no longer match the exact founding assets",
        ));
    }
    match (
        input.local_participant_fixture_liquidity_atoms,
        checkpoint.local_participant_fixture_liquidity.as_ref(),
    ) {
        (0, None) => {}
        (LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1, Some(receipt)) => {
            let (source, owner) = if consume_role_keys {
                (
                    forge
                        .keypair(LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1)
                        .pubkey(),
                    forge
                        .keypair(LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1)
                        .pubkey(),
                )
            } else {
                (
                    forge.peek_pubkey(LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1)?,
                    forge.peek_pubkey(LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1)?,
                )
            };
            let source_account = rpc.required_account(
                source,
                "checkpoint local participant fixture collateral source",
            )?;
            let parsed_source = TokenAccount::parse(&source_account.data).map_err(|error| {
                Error::new(format!("checkpoint participant fixture source: {error:?}"))
            })?;
            if receipt.source_token_account != source.to_string()
                || receipt.source_owner != owner.to_string()
                || receipt.quantity_atoms != LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1
                || receipt.founding_collateral_atoms != input.initial_collateral_atoms
                || receipt.total_supply_atoms != expected_supply
                || receipt.mint != mint.to_string()
                || !receipt.mint_authority_removed
                || receipt.transaction_signature.parse::<Signature>().is_err()
                || receipt.finalized_slot == 0
                || receipt.compute_units_consumed == 0
                || source_account.owner != token_program
                || parsed_source.mint != mint.to_bytes()
                || parsed_source.owner != owner.to_bytes()
                || parsed_source.amount != LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1
                || parsed_source.state != AccountState::Initialized
                || !parsed_source.delegate.is_none()
                || parsed_source.delegated_amount != 0
                || !parsed_source.native_reserve.is_none()
                || !parsed_source.close_authority.is_none()
            {
                return Err(Error::new(
                    "DCLTPCB2 checkpoint local participant fixture liquidity changed",
                ));
            }
        }
        _ => {
            return Err(Error::new(
                "DCLTPCB2 checkpoint fixture-liquidity presence differs from its Market input",
            ));
        }
    }

    let targets = derive_founding_targets(plan, input, mint)?;
    if pubkey(&checkpoint.market)? != targets.open_market {
        return Err(Error::new(
            "DCLTPCB2 checkpoint names another derived Open Market",
        ));
    }

    // Every raw record was authenticated immediately above. The canonical
    // publisher is reused solely to reconstruct the one MarketRecords owner;
    // a complete raw graph makes every publication step Complete. A write here
    // would therefore be an invariant failure, never an admitted repair.
    let transaction_count = transactions.len();
    let (records, product) = publish_market_records(
        rpc,
        registry,
        input,
        mint,
        targets.open_market,
        hex32(&plan.release_set_id)?,
        payer,
        transactions,
    )?;
    if transactions.len() != transaction_count {
        return Err(Error::new(
            "DCLTPCB2 checkpoint reconstruction attempted to republish its record graph",
        ));
    }

    let identity_template = MarketIdentity {
        market_id: identity([0xff; 32])?,
        realm_id: identity(records.realm.digest)?,
        product_record: identity(records.product.digest)?,
        product_id: identity(product.to_bytes())?,
        resolution_policy: identity(records.source.digest)?,
        capability_manifest: identity(records.manifest.digest)?,
        selected_release_set: identity(hex32(&plan.release_set_id)?)?,
        registry_program: identity(registry.to_bytes())?,
        generation: input.generation,
    };
    let found31_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity_template).as_slices(),
        &core,
    )
    .0;
    if found31_market != targets.found31_market {
        return Err(Error::new(
            "DCLTPCB2 checkpoint reconstruction moved the Found37 Market",
        ));
    }

    let beneficiary = if consume_role_keys {
        let beneficiary = forge.keypair(role::FOUNDING_BENEFICIARY).pubkey();
        let _projection_witness = forge.keypair(role::FOUNDING_PROJECTION_WITNESS);
        beneficiary
    } else {
        forge.peek_pubkey(role::FOUNDING_BENEFICIARY)?
    };
    let coordinates = derive_founding_coordinates(
        rpc,
        plan,
        input,
        &records,
        identity_template,
        product,
        mint,
        payer.pubkey(),
        actors.founder,
        beneficiary,
        PrestateLaneV1::Founding.generation(input)?,
        checkpoint.expiry_slot,
    )?;
    if consume_role_keys {
        let _source_funder = forge.keypair(role::FOUNDING_SOURCE_FUNDER);
    }

    let trading = pubkey(&plan.trading.program_id)?;
    let trading_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == trading)
        .ok_or_else(|| Error::new("reconstructed DCLTPCB2 omitted Trading FundingLedgerV2"))?;
    let root = Pubkey::new_from_array(coordinates.found.capability_root().to_bytes());
    if coordinates.market != targets.open_market
        || hex(&coordinates.context) != checkpoint.founding_custody_context
        || coordinates.capability_entry_index != checkpoint.direct_selected_manifest_entry_index
        || root.to_string() != checkpoint.direct_capability_root
        || trading_ledger.address.to_string() != checkpoint.direct_trading_funding_ledger
    {
        return Err(Error::new(
            "reconstructed DCLTPCB2 coordinates differ from the durable checkpoint",
        ));
    }

    let found_raw = coordinates
        .found
        .encode()
        .map_err(|error| Error::new(format!("checkpoint founding artifact: {error:?}")))?;
    let lock_raw = coordinates
        .lock
        .encode()
        .map_err(|error| Error::new(format!("checkpoint terminal Lock: {error:?}")))?;
    let found_record =
        derive_raw_request_record_v1(registry, "generic-founding-artifact", &found_raw)?;
    let lock_record =
        derive_raw_request_record_v1(registry, "projected-custody-terminal-lock", &lock_raw)?;
    if found_record.raw.to_string() != checkpoint.found_record
        || lock_record.raw.to_string() != checkpoint.lock_record
        || rpc
            .required_account(found_record.raw, "checkpoint founding artifact")?
            .data
            != found_raw
        || rpc
            .required_account(lock_record.raw, "checkpoint terminal Lock")?
            .data
            != lock_raw
    {
        return Err(Error::new(
            "DCLTPCB2 checkpoint request records differ from their reconstructed bytes",
        ));
    }

    let claim_count = u32::try_from(input.cuts.len().saturating_add(2))
        .map_err(|_| Error::new("checkpoint Product outcome width overflow"))?;
    Ok(RecoveredFoundingContextV1 {
        records,
        coordinates,
        identity_template,
        product,
        mint,
        collateral_wallet: wallet,
        found31_market,
        founder: actors.founder,
        found_record: found_record.raw,
        lock_record: lock_record.raw,
        claim_count,
    })
}

fn authenticate_prepared_resume_assets_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    checkpoint: &MarketExecutionCheckpointV1,
    context: &RecoveredFoundingContextV1,
) -> Result<()> {
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    // Reauthentication is read-only with respect to the persisted role
    // issuance sequence. The resumed executor below must still consume the
    // exact index-0 keys named by DCLTCFQ1; advancing here would silently move
    // every signer/PDA in the suffix to index 1.
    let (beneficiary, source_funder, projection_witness) = prepared_resume_role_pubkeys_v1(forge)?;
    for (label, key) in [
        ("collateral_wallet", context.collateral_wallet),
        ("founding_source_funder", source_funder),
        ("founding_projection_witness", projection_witness),
        (
            "founding_prepared_lifecycle_rent_credit",
            context.coordinates.credit,
        ),
    ] {
        let expected = checkpoint.accounts.get(label).ok_or_else(|| {
            Error::new(format!(
                "Prepared checkpoint omitted exact {label} evidence"
            ))
        })?;
        let account = rpc.required_account(key, label)?;
        if expected.address != key.to_string() || *expected != account_evidence(key, &account) {
            return Err(Error::new(format!(
                "Prepared checkpoint {label} changed before suffix resume"
            )));
        }
    }
    let source_account = rpc.required_account(source_funder, "founding_source_funder")?;
    let source = TokenAccount::parse(&source_account.data)
        .map_err(|error| Error::new(format!("Prepared source funder: {error:?}")))?;
    let wallet_account = rpc.required_account(context.collateral_wallet, "collateral_wallet")?;
    let wallet = TokenAccount::parse(&wallet_account.data)
        .map_err(|error| Error::new(format!("Prepared collateral wallet: {error:?}")))?;
    let expected_wallet_atoms = input
        .initial_collateral_atoms
        .checked_sub(context.coordinates.lock.amount)
        .ok_or_else(|| Error::new("Prepared wallet principal arithmetic underflow"))?;
    if source_account.owner != token_program
        || source.mint != context.mint.to_bytes()
        || source.owner != beneficiary.to_bytes()
        || source.amount != context.coordinates.lock.amount
        || source.state != AccountState::Initialized
        || !source.delegate.is_none()
        || source.delegated_amount != 0
        || !source.native_reserve.is_none()
        || !source.close_authority.is_none()
        || wallet.owner != payer.pubkey().to_bytes()
        || wallet.amount != expected_wallet_atoms
    {
        return Err(Error::new(
            "Prepared principal supplier or collateral wallet changed before suffix resume",
        ));
    }
    let witness = rpc.required_account(projection_witness, "founding_projection_witness")?;
    if witness.owner != system_program::ID
        || witness.executable
        || !witness.data.is_empty()
        || witness.lamports != rpc.minimum_balance(STATE_BYTES)?
    {
        return Err(Error::new(
            "Prepared projection witness no longer holds the exact retained Market rent",
        ));
    }
    authenticate_controller_funding_checkpoint_v1(
        rpc,
        plan,
        &context.coordinates,
        payer.pubkey(),
        ControllerFundingCheckpointPhaseV1::Prepared,
    )?;
    Ok(())
}

fn prepared_resume_role_pubkeys_v1(forge: &KeyForge) -> Result<(Pubkey, Pubkey, Pubkey)> {
    Ok((
        forge.peek_pubkey(role::FOUNDING_BENEFICIARY)?,
        forge.peek_pubkey(role::FOUNDING_SOURCE_FUNDER)?,
        forge.peek_pubkey(role::FOUNDING_PROJECTION_WITNESS)?,
    ))
}

/// Resume after DCLTCFQ1 finalized but before DCLTPCB2 was planned. The
/// durable checkpoint reconstructs every prefix coordinate and exact mutable
/// balance; this route enters at DCLTPCB2 and never recreates collateral,
/// records, Found37, the RentCredit, the supplier, or DCLTCFQ1.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resume_found_market_from_prepared_checkpoint(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    transactions: &mut Vec<TransactionEvidence>,
    prepared_checkpoint: &MarketExecutionCheckpointV1,
    checkpoint: &mut dyn FnMut(&MarketExecutionCheckpointV1) -> Result<()>,
    mut submission_recorder: Option<&mut FoundingSubmissionRecorderV1<'_>>,
) -> Result<MarketExecutionEvidence> {
    if prepared_checkpoint.schema != DCLTCFQ1_PREPARED_CHECKPOINT_SCHEMA_V1 {
        return Err(Error::new(
            "Prepared suffix resume requires the DCLTCFQ1 checkpoint schema",
        ));
    }
    let context = reconstruct_founding_checkpoint_v1(
        rpc,
        plan,
        input,
        payer,
        forge,
        actors,
        transactions,
        prepared_checkpoint,
        false,
    )?;
    authenticate_prepared_resume_assets_v1(
        rpc,
        plan,
        input,
        payer,
        forge,
        prepared_checkpoint,
        &context,
    )?;
    if rpc.finalized_slot()? > prepared_checkpoint.expiry_slot {
        return Err(Error::new(
            "the Prepared controller-funding checkpoint expired before DCLTPCB2 resume",
        ));
    }
    let mut accounts = prepared_checkpoint.accounts.clone();
    let mut completed = prepared_checkpoint.completed.clone();
    let founding_context = execute_projected_custody_bootstrap(
        rpc,
        plan,
        input,
        &context.records,
        context.identity_template,
        context.product,
        context.mint,
        context.collateral_wallet,
        context.found31_market,
        PrestateLaneV1::Founding,
        Some(prepared_checkpoint),
        payer,
        forge,
        actors,
        transactions,
        &mut accounts,
        &mut completed,
        prepared_checkpoint
            .local_participant_fixture_liquidity
            .as_ref(),
        checkpoint,
        submission_recorder.as_deref_mut(),
    )?;
    Ok(MarketExecutionEvidence {
        completed,
        accounts,
        founding_custody_context: hex(&founding_context),
        direct_selected_manifest_entry_index: prepared_checkpoint
            .direct_selected_manifest_entry_index,
        local_participant_fixture_liquidity: prepared_checkpoint
            .local_participant_fixture_liquidity
            .clone(),
    })
}

/// Resume only the suffix whose durable checkpoint proves DCLTPCB2 complete.
/// No collateral, publication, Found37, or principal-funding prefix is replayed.
pub(crate) fn resume_found_market_from_checkpoint(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    transactions: &mut Vec<TransactionEvidence>,
    checkpoint: &MarketExecutionCheckpointV1,
    mut submission_recorder: Option<&mut FoundingSubmissionRecorderV1<'_>>,
) -> Result<MarketExecutionEvidence> {
    if checkpoint.schema != DCLTPCB2_CHECKPOINT_SCHEMA_V1 {
        return Err(Error::new(
            "custody-staged resume requires the DCLTPCB2 checkpoint schema",
        ));
    }
    let context = reconstruct_founding_checkpoint_v1(
        rpc,
        plan,
        input,
        payer,
        forge,
        actors,
        transactions,
        checkpoint,
        true,
    )?;
    if rpc.finalized_slot()? > checkpoint.expiry_slot {
        return Err(Error::new(
            "the checkpointed DCLTPCB2 founding prestate expired before resume; use its explicit abort route rather than replaying principal",
        ));
    }
    authenticate_bootstrap_poststate(
        rpc,
        &context.coordinates,
        pubkey(&plan.custody.program_id)?,
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        context.mint,
        context.coordinates.lock.amount,
    )?;
    if let Some(recorder) = submission_recorder.as_deref_mut() {
        let coordinates = &context.coordinates;
        let custody = pubkey(&plan.custody.program_id)?;
        let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        let mint = context.mint;
        let principal = coordinates.lock.amount;
        let mut completion = |rpc: &mut Rpc| {
            authenticate_bootstrap_poststate(
                rpc,
                coordinates,
                custody,
                token_program,
                mint,
                principal,
            )
        };
        if let Some(evidence) = finalize_existing_founding_submission_v1(
            rpc,
            "stage projected custody against prepared controller funding (DCLTPCB2)",
            FoundingSubmissionOperationV1::Dcltpcb2,
            recorder,
            &mut completion,
        )? {
            transactions.push(evidence);
        }
    }
    let mut accounts = checkpoint.accounts.clone();
    let mut completed = checkpoint.completed.clone();
    execute_generic_market_founding(
        rpc,
        plan,
        input,
        &context.records,
        &context.coordinates,
        context.product,
        context.mint,
        targets_found31(plan, input, context.mint)?,
        actors,
        context.found_record,
        context.lock_record,
        context.claim_count,
        payer,
        transactions,
        &mut accounts,
        &mut completed,
        submission_recorder.as_deref_mut(),
    )?;
    Ok(MarketExecutionEvidence {
        completed,
        accounts,
        founding_custody_context: checkpoint.founding_custody_context.clone(),
        direct_selected_manifest_entry_index: checkpoint.direct_selected_manifest_entry_index,
        local_participant_fixture_liquidity: checkpoint.local_participant_fixture_liquidity.clone(),
    })
}

fn targets_found31(plan: &SuccessorPlan, input: &MarketRunInput, mint: Pubkey) -> Result<Pubkey> {
    Ok(derive_founding_targets(plan, input, mint)?.found31_market)
}

/// Rebuild full caller-consumable evidence after the atomic DCLTGMF3 packet
/// finalized but the process died before the campaign report update.
pub(crate) fn recover_completed_market_from_checkpoint(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    transactions: &mut Vec<TransactionEvidence>,
    checkpoint: &MarketExecutionCheckpointV1,
    mut submission_recorder: Option<&mut FoundingSubmissionRecorderV1<'_>>,
) -> Result<MarketExecutionEvidence> {
    if checkpoint.schema != DCLTPCB2_CHECKPOINT_SCHEMA_V1 {
        return Err(Error::new(
            "completed founding recovery requires the DCLTPCB2 checkpoint schema",
        ));
    }
    let context = reconstruct_founding_checkpoint_v1(
        rpc,
        plan,
        input,
        payer,
        forge,
        actors,
        transactions,
        checkpoint,
        true,
    )?;
    let poststate = derive_founding_poststate_expectation_v1(
        plan,
        &context.coordinates,
        context.founder,
        context.claim_count,
    )?;
    authenticate_open_market_poststate_v1(
        rpc,
        &context.coordinates,
        &poststate,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.claims.program_id)?,
        pubkey(&plan.custody.program_id)?,
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        context.mint,
    )?;
    if let Some(recorder) = submission_recorder.as_deref_mut() {
        let coordinates = &context.coordinates;
        let core = pubkey(&plan.core.program_id)?;
        let claims = pubkey(&plan.claims.program_id)?;
        let custody = pubkey(&plan.custody.program_id)?;
        let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        let mint = context.mint;
        let mut completion = |rpc: &mut Rpc| {
            authenticate_open_market_poststate_v1(
                rpc,
                coordinates,
                &poststate,
                core,
                claims,
                custody,
                token_program,
                mint,
            )
        };
        if let Some(evidence) = finalize_existing_founding_submission_v1(
            rpc,
            "found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF3)",
            FoundingSubmissionOperationV1::Dcltgmf3,
            recorder,
            &mut completion,
        )? {
            transactions.push(evidence);
        }
    }
    let mut accounts = checkpoint.accounts.clone();
    for (label, key) in [
        ("founding_market", context.coordinates.market),
        ("claims_aggregate", poststate.aggregate),
        ("founder_position", poststate.position),
        ("claims_admission", poststate.admission),
        ("founding_hoard_vault_open", context.coordinates.hoard_vault),
        (
            "founding_normal_custody_replay",
            context.coordinates.projected_replay,
        ),
    ] {
        let account = rpc.required_account(key, label)?;
        accounts.insert(label.into(), account_evidence(key, &account));
    }
    let mut completed = checkpoint.completed.clone();
    completed.push(
        "recovered finalized DCLTGMF3 poststate from the durable DCLTPCB2 checkpoint after process interruption".into(),
    );
    completed.push(
        "executed DCLTGMF3: the Market is OPEN, with the Claims liability aggregate, the founder Position, the admission record, and a Hoard holding the exact collateral".into(),
    );
    let routing_table_keys = finalized_founding_routing_table_keys_v1(
        submission_recorder
            .as_deref()
            .ok_or_else(|| Error::new("completed public recovery omitted its journal owner"))?,
        FoundingSubmissionOperationV1::Dcltgmf3,
    )?;
    let minimum_slot = rpc.finalized_slot()?;
    execute_funding_readiness_suffix_v1(
        rpc,
        plan,
        &context.records,
        &context.coordinates,
        payer,
        transactions,
        &mut accounts,
        &mut completed,
        minimum_slot,
        &routing_table_keys,
        submission_recorder.as_deref_mut(),
    )?;
    Ok(MarketExecutionEvidence {
        completed,
        accounts,
        founding_custody_context: checkpoint.founding_custody_context.clone(),
        direct_selected_manifest_entry_index: checkpoint.direct_selected_manifest_entry_index,
        local_participant_fixture_liquidity: checkpoint.local_participant_fixture_liquidity.clone(),
    })
}

/// Every record body one market input compiles to, before anything touches a
/// chain.
///
/// One author for two consumers: `publish_market_records` publishes exactly
/// these bytes, and `derive_founding_targets` digests exactly these bytes to
/// know where the founding will land — so the detector and the executor cannot
/// disagree about what the input means without one of them failing loudly.
struct CompiledMarketBodiesV1 {
    compiled: CompiledProductRecordsV2,
    semantic_product_id: ProductContentId,
    realm: Vec<u8>,
    product: [u8; PRODUCT_RECORD_BYTES_V2],
    domain: Vec<u8>,
    portfolio: Vec<u8>,
    source: Vec<u8>,
    source_capacity_profile: Vec<u8>,
    manipulation_floor: Vec<u8>,
    principal_cap_sets: u64,
    /// Empty means the material carries no recovery policy (the deliberate
    /// §12.8 demo shape) and no recovery record is published.
    recovery: Vec<u8>,
    manifest: Vec<u8>,
    product_digest: [u8; 32],
    domain_digest: [u8; 32],
}

/// Compile one market input's record bodies, with no RPC and no side effects.
fn compile_market_bodies(
    registry: Pubkey,
    input: &MarketRunInput,
    collateral_mint: Pubkey,
) -> Result<CompiledMarketBodiesV1> {
    let cuts = input
        .cuts
        .iter()
        .map(|value| canonical_i128(value))
        .collect::<Result<Vec<_>>>()?;
    let outcome_count = cuts
        .len()
        .checked_add(2)
        .ok_or_else(|| Error::new("Product outcome width overflow"))?;
    if input.coefficients.len() != outcome_count {
        return Err(Error::new(
            "portfolio coefficient width must equal cuts + failure + tails",
        ));
    }
    let semantic_product_id = product_id(&input.product_id)?;
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![
        0_u8;
        result_domain_record_bytes(cuts.len()).map_err(|error| Error::new(
            format!("result-domain width: {error:?}")
        ))?
    ];
    let mut portfolio = vec![
        0_u8;
        portfolio_record_bytes(outcome_count).map_err(|error| Error::new(
            format!("portfolio width: {error:?}")
        ))?
    ];
    let compiled = compile_product_records_v2(
        registry,
        ProductCompilationInputV2 {
            product_id: semantic_product_id,
            coordinate_domain_id: product_id(&input.coordinate_domain_id)?,
            result_unit_id: product_id(&input.result_unit_id)?,
            claim_basis_id: product_id(&input.claim_basis_id)?,
            liability_basis_id: product_id(&input.liability_basis_id)?,
            representation_release_id: product_id(&input.representation_release_id)?,
            mapping_release_id: product_id(&input.mapping_release_id)?,
            cut_denominator: input.cut_denominator,
            cuts: &cuts,
            portfolio_denominator: input.portfolio_denominator,
            coefficients: &input.coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .map_err(|error| Error::new(format!("canonical Product compiler: {error:?}")))?;
    let product_digest: [u8; 32] = Sha256::digest(product).into();
    let domain_digest: [u8; 32] = Sha256::digest(&domain).into();

    let recovery_bytes = decode_hex(&input.recovery_policy_hex)?;
    let recovery_link = if recovery_bytes.is_empty() {
        // The no-recovery material: a silent provider walks the funded
        // Primary -> Exhausted -> FailureCommitted path to the Product's own
        // pre-disclosed failure outcome (MAINNET_STATE_RELAY.md section 13).
        None
    } else {
        let recovery = RecoveryPolicyV2::decode(&recovery_bytes)
            .map_err(|error| Error::new(format!("RecoveryPolicyV2: {error:?}")))?;
        if recovery.to_bytes().as_slice() != recovery_bytes {
            return Err(Error::new("RecoveryPolicyV2 input was not canonical"));
        }
        let recovery_digest: [u8; 32] = Sha256::digest(&recovery_bytes).into();
        Some(
            SourceContentId::new(recovery_digest)
                .map_err(|error| Error::new(format!("Recovery digest: {error:?}")))?,
        )
    };
    let source_spec_bytes = decode_hex(&input.source_spec_hex)?;
    let source_spec = SourceSpecV1::decode(&source_spec_bytes)
        .map_err(|error| Error::new(format!("SourceSpecV1: {error:?}")))?;
    let source_spec_digest: [u8; 32] = Sha256::digest(&source_spec_bytes).into();
    if source_spec_digest != source_id(&input.primary_source_spec_id)?.to_bytes() {
        return Err(Error::new(
            "SourceSpecV1 body does not own the selected identity",
        ));
    }
    let source_spec_identity = SourceContentId::new(source_spec_digest)
        .map_err(|error| Error::new(format!("SourceSpec identity: {error:?}")))?;
    let source_capacity_profile = decode_hex(&input.source_capacity_profile_hex)?;
    let capacity = SourceCapacityProfileV1::decode(&source_capacity_profile)
        .map_err(|error| Error::new(format!("SourceCapacityProfileV1: {error:?}")))?;
    let capacity_digest: [u8; 32] = Sha256::digest(&source_capacity_profile).into();
    if source_spec.capacity_profile_id().to_bytes() != capacity_digest {
        return Err(Error::new(
            "SourceSpecV1 selects a different capacity profile",
        ));
    }
    let market_collateral_unit = SourceContentId::new(collateral_mint.to_bytes())
        .map_err(|error| Error::new(format!("Realm collateral unit: {error:?}")))?;
    let manipulation_floor_template = decode_hex(&input.manipulation_floor_hex)?;
    let template_floor = if manipulation_floor_template.is_empty() {
        None
    } else {
        Some(
            ManipulationFloorV1::decode(&manipulation_floor_template)
                .map_err(|error| Error::new(format!("ManipulationFloorV1 template: {error:?}")))?,
        )
    };
    let manipulation_floor = template_floor.map(|template| {
        ManipulationFloorV1::new(
            template.basis(),
            source_spec_identity,
            source_spec.adapter_config_id(),
            market_collateral_unit,
            template.derivation_release_id(),
            template.floor_atoms(),
        )
    });
    let source_for_floor = |floor: ManipulationFloorV1| -> Result<SourceMaterialV3> {
        Ok(SourceMaterialV3::bounded_by_floor(
            SourceContentId::new(product_digest)
                .map_err(|error| Error::new(format!("Product digest: {error:?}")))?,
            source_spec_identity,
            source_id(&input.window_spec_id)?,
            source_id(&input.statistic_spec_id)?,
            recovery_link,
            source_id(&input.failure_policy_release_id)?,
            SourceContentId::new(Sha256::digest(floor.to_bytes()).into())
                .map_err(|error| Error::new(format!("floor identity: {error:?}")))?,
        ))
    };
    let source = match manipulation_floor {
        Some(floor) => source_for_floor(floor)?,
        None => SourceMaterialV3::explicitly_unbounded(
            SourceContentId::new(product_digest)
                .map_err(|error| Error::new(format!("Product digest: {error:?}")))?,
            source_spec_identity,
            source_id(&input.window_spec_id)?,
            source_id(&input.statistic_spec_id)?,
            recovery_link,
            source_id(&input.failure_policy_release_id)?,
        ),
    };
    let linked_basis_bytes = decode_hex(&input.linked_basis_hex)?;
    let basis = ProductBasisV3::decode(&linked_basis_bytes)
        .map_err(|error| Error::new(format!("ProductBasisV3: {error:?}")))?;
    let authenticated_floor = match manipulation_floor {
        Some(floor) => Some((
            SourceContentId::new(Sha256::digest(floor.to_bytes()).into())
                .map_err(|error| Error::new(format!("floor identity: {error:?}")))?,
            floor,
        )),
        None => None,
    };
    let principal_cap_sets = source
        .derive_principal_cap_sets(
            source_spec_identity,
            source_spec,
            SourceContentId::new(capacity_digest)
                .map_err(|error| Error::new(format!("capacity identity: {error:?}")))?,
            capacity,
            authenticated_floor,
            market_collateral_unit,
            basis.payout_scale(),
        )
        .map_err(|error| Error::new(format!("Source principal cap: {error:?}")))?
        .to_sets();
    if principal_cap_sets == 0 {
        return Err(Error::new(
            "Source policy projected an absent principal cap",
        ));
    }
    let manipulation_floor = manipulation_floor
        .map(|floor| floor.to_bytes().to_vec())
        .unwrap_or_default();
    let source = source.to_bytes();
    let source_digest: [u8; 32] = Sha256::digest(source).into();
    let template_source_digest: Option<[u8; 32]> = match template_floor {
        Some(floor) => Some(Sha256::digest(source_for_floor(floor)?.to_bytes()).into()),
        None => None,
    };
    let mut manifest = decode_hex(&input.capability_manifest_hex)?;
    if let Some(template_digest) = template_source_digest
        && template_digest != source_digest
    {
        let original = CapabilityManifestV1::decode(&manifest)
            .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
        let mut matches = 0_usize;
        let mut rebuilt = Vec::with_capacity(usize::from(original.entry_count()));
        for index in 0..original.entry_count() {
            let entry = original
                .entry(index)
                .map_err(|error| Error::new(format!("capability entry {index}: {error:?}")))?;
            let config_id = if entry.config_id().to_bytes() == template_digest {
                matches += 1;
                CapabilityContentId::new(source_digest)
                    .map_err(|error| Error::new(format!("Source config identity: {error:?}")))?
            } else {
                entry.config_id()
            };
            let mut dependencies = [0_u8; MAX_DEPENDENCIES_PER_CAPABILITY];
            for position in 0..usize::from(entry.dependency_count()) {
                dependencies[position] = entry.dependency(position).map_err(|error| {
                    Error::new(format!(
                        "capability dependency {index}/{position}: {error:?}"
                    ))
                })?;
            }
            rebuilt.push(
                CapabilityEntryV1::new(
                    entry.kind_id(),
                    entry.release_id(),
                    config_id,
                    entry.capacity_profile_id(),
                    entry.child_schema_id(),
                    entry.child_derivation_id(),
                    entry.activation_policy(),
                    entry.activation_deadline_slot(),
                    entry.dependency_count(),
                    dependencies,
                    entry.funding_quote(),
                )
                .map_err(|error| {
                    Error::new(format!("rebuilt capability entry {index}: {error:?}"))
                })?,
            );
        }
        if matches != 1 {
            return Err(Error::new(
                "bounded Source material must own exactly one capability config",
            ));
        }
        CapabilityManifestV1::encode_into(&rebuilt, &mut manifest)
            .map_err(|error| Error::new(format!("rebuilt capability manifest: {error:?}")))?;
    }
    let decoded_manifest = CapabilityManifestV1::decode(&manifest)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
    if decoded_manifest.as_bytes() != manifest.as_slice() || decoded_manifest.entry_count() < 3 {
        return Err(Error::new(
            "capability manifest was noncanonical or omitted the three Resolution funding entries",
        ));
    }
    let adapter = CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer();
    let realm = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: collateral_mint.to_bytes(),
        collateral_adapter_release_id: Sha256::digest(adapter.to_bytes()).into(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .map_err(|error| Error::new(format!("canonical collateral Realm: {error:?}")))?
    .to_bytes();
    Ok(CompiledMarketBodiesV1 {
        compiled,
        semantic_product_id,
        realm: realm.to_vec(),
        product,
        domain,
        portfolio,
        source: source.to_vec(),
        source_capacity_profile,
        manipulation_floor,
        principal_cap_sets,
        recovery: recovery_bytes,
        manifest,
        product_digest,
        domain_digest,
    })
}

#[cfg(test)]
pub(crate) fn native_composition_bodies_for_test(
    registry: Pubkey,
    input: &MarketRunInput,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> {
    let compiled = compile_market_bodies(registry, input, Pubkey::new_from_array([0x7b; 32]))?;
    Ok((
        compiled.product.to_vec(),
        compiled.domain,
        compiled.portfolio,
        decode_hex(&input.linked_basis_hex)?,
    ))
}

fn publish_market_records(
    rpc: &mut Rpc,
    registry: Pubkey,
    input: &MarketRunInput,
    collateral_mint: Pubkey,
    terminal_market: Pubkey,
    release_set: [u8; 32],
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(MarketRecords, ProductContentId)> {
    let source_publication = authenticate_source_publication_v1(input)?;
    let outcome_count = input
        .cuts
        .len()
        .checked_add(2)
        .ok_or_else(|| Error::new("Product outcome width overflow"))?;
    let CompiledMarketBodiesV1 {
        compiled,
        semantic_product_id,
        realm,
        product,
        domain,
        portfolio,
        source,
        source_capacity_profile,
        manipulation_floor: manipulation_floor_bytes,
        principal_cap_sets,
        recovery: recovery_bytes,
        manifest,
        product_digest: _,
        domain_digest,
    } = compile_market_bodies(registry, input, collateral_mint)?;

    let hostile_wallet = Some(crate::seed::fresh_probe_address());
    let realm = publish_record(
        rpc,
        registry,
        payer,
        REALM_SCHEMA_RELEASE_ID_V1,
        &realm,
        hostile_wallet,
        transactions,
    )?;
    let product_body = product.clone();
    let domain_body = domain.clone();
    let portfolio_body = portfolio.clone();
    let (product, domain, portfolio) = publish_product_graph(
        rpc,
        registry,
        payer,
        compiled,
        &product,
        &domain,
        &portfolio,
        transactions,
    )?;
    let source = publish_record(
        rpc,
        registry,
        payer,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        &source,
        None,
        transactions,
    )?;
    let recovery = if recovery_bytes.is_empty() {
        None
    } else {
        Some(publish_record(
            rpc,
            registry,
            payer,
            RECOVERY_POLICY_SCHEMA_ID_V2,
            &recovery_bytes,
            None,
            transactions,
        )?)
    };
    let mut direct = BTreeMap::new();
    let linked_basis_bytes = decode_hex(&input.linked_basis_hex)?;
    for record in crate::direct_market::direct_publication_records_v1(
        input,
        dclutch_representation_composition_v3_operator::native_categorical_v1::NativeCategoricalCompositionInputV1 {
            market: terminal_market.to_bytes(),
            release_set,
            product_record_bytes: &product_body,
            result_domain_bytes: &domain_body,
            portfolio_bytes: &portfolio_body,
            product_basis_bytes: &linked_basis_bytes,
        },
    )? {
        let published = publish_record(
            rpc,
            registry,
            payer,
            record.schema,
            &record.body,
            None,
            transactions,
        )?;
        if direct.insert(record.label, published).is_some() {
            return Err(Error::new("Direct publication repeated an evidence label"));
        }
    }
    let manifest = publish_record(
        rpc,
        registry,
        payer,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest,
        None,
        transactions,
    )?;
    // The five source-graph records. `validate_market_input` has already
    // checked that each identity the material names IS the SHA-256 of the body
    // published here, so `publish_record` cannot land a record at an address
    // the Market does not point at.
    let source_spec = publish_record(
        rpc,
        registry,
        payer,
        SOURCE_SPEC_SCHEMA_ID_V1,
        &decode_hex(&input.source_spec_hex)?,
        None,
        transactions,
    )?;
    let source_capacity_profile = publish_record(
        rpc,
        registry,
        payer,
        SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
        &source_capacity_profile,
        None,
        transactions,
    )?;
    let manipulation_floor = if manipulation_floor_bytes.is_empty() {
        None
    } else {
        Some(publish_record(
            rpc,
            registry,
            payer,
            MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            &manipulation_floor_bytes,
            None,
            transactions,
        )?)
    };
    let window_spec = publish_record(
        rpc,
        registry,
        payer,
        WINDOW_SPEC_SCHEMA_ID_V1,
        &decode_hex(&input.window_spec_hex)?,
        None,
        transactions,
    )?;
    let statistic_spec = publish_record(
        rpc,
        registry,
        payer,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        &decode_hex(&input.statistic_spec_hex)?,
        None,
        transactions,
    )?;
    let provider_release = publish_record(
        rpc,
        registry,
        payer,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        &decode_hex(&input.provider_release_hex)?,
        None,
        transactions,
    )?;
    let adapter_config = publish_record(
        rpc,
        registry,
        payer,
        source_publication.adapter_config_schema,
        &decode_hex(&input.pyth_adapter_config_hex)?,
        None,
        transactions,
    )?;
    let sponsored_push_release = match source_publication.sponsored_release {
        Some(body) => Some(publish_record(
            rpc,
            registry,
            payer,
            PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1,
            &body,
            None,
            transactions,
        )?),
        None => None,
    };
    // Founding reads the liability basis the Product declares. Both of the
    // record's links are checked against the graph that was just compiled, so
    // a basis belonging to a different Product cannot be published as this
    // Market's.
    let basis_bytes = linked_basis_bytes;
    let basis_state = ProductBasisV3::decode(&basis_bytes)
        .map_err(|error| Error::new(format!("ProductBasisV3: {error:?}")))?;
    let outcome_width =
        u32::try_from(outcome_count).map_err(|_| Error::new("Product outcome width overflow"))?;
    if semantic_basis_identity_v3(&basis_bytes)?
        != product_id(&input.liability_basis_id)?.to_bytes()
        || basis_state.product_id() != semantic_product_id.to_bytes()
        || basis_state.result_domain_id() != domain_digest
        || basis_state.basis_width() != outcome_width
        || basis_state.payout_scale() != 1
        || basis_state.kind() != BasisKindV3::CategoricalQ1
    {
        return Err(Error::new(
            "linked liability basis record did not bind the compiled Product graph",
        ));
    }
    let basis = publish_record(
        rpc,
        registry,
        payer,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        &basis_bytes,
        None,
        transactions,
    )?;
    Ok((
        MarketRecords {
            realm,
            product,
            domain,
            portfolio,
            source,
            source_capacity_profile,
            manipulation_floor,
            recovery,
            manifest,
            basis,
            source_spec,
            window_spec,
            statistic_spec,
            provider_release,
            adapter_config,
            sponsored_push_release,
            direct,
            principal_cap_sets,
        },
        semantic_product_id,
    ))
}

/// Where one market input's founding lands on a chain, derived offline.
///
/// Everything here is a function of the plan, the input, and the collateral
/// mint's public key — no RPC, no side effects. It exists so the campaign
/// driver's founding stage can READ the chain before writing it: the bodies
/// come from the same `compile_market_bodies` the publisher uses, and the
/// identity completion is the same move `derive_founding_coordinates` makes,
/// so the detector and the executor answer from one derivation.
pub(crate) struct FoundingTargetsV1 {
    pub(crate) collateral_mint: Pubkey,
    /// The realm record's raw address — the first account the founding ever
    /// publishes, so its existence separates "untouched" from "started".
    pub(crate) realm_record: Pubkey,
    /// The canonical Found37 Market at the input's own generation.
    pub(crate) found31_market: Pubkey,
    /// The DCLTGMF3 Market at generation + 1 — the one that ends Open, which
    /// is the product of the whole founding.
    pub(crate) open_market: Pubkey,
    /// The abort-lane Market at generation + 2 (staged and unwound; its
    /// existence mid-run still marks a founding in progress).
    pub(crate) abort_market: Pubkey,
    /// The Open Market's completed identity, for the poststate comparison.
    pub(crate) open_market_identity: MarketIdentity,
}

pub(crate) fn derive_founding_targets(
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    collateral_mint: Pubkey,
) -> Result<FoundingTargetsV1> {
    derive_founding_targets_inner(
        pubkey(&plan.registry.program_id)?,
        pubkey(&plan.core.program_id)?,
        hex32(&plan.release_set_id)?,
        input,
        collateral_mint,
    )
}

fn derive_founding_targets_inner(
    registry: Pubkey,
    core: Pubkey,
    release_set: [u8; 32],
    input: &MarketRunInput,
    collateral_mint: Pubkey,
) -> Result<FoundingTargetsV1> {
    validate_market_input(input)?;
    let bodies = compile_market_bodies(registry, input, collateral_mint)?;
    let (realm_record, _, _) = derive_record_addresses_v1(
        registry,
        RecordPublicationContentV1 {
            schema_release_id: REALM_SCHEMA_RELEASE_ID_V1,
            content: &bodies.realm,
        },
    )
    .map_err(|error| Error::new(format!("derive realm record address: {error:?}")))?;
    let template = MarketIdentity {
        market_id: identity([0xff; 32])?,
        realm_id: identity(record_identity(&bodies.realm))?,
        product_record: identity(bodies.product_digest)?,
        product_id: identity(bodies.semantic_product_id.to_bytes())?,
        resolution_policy: identity(record_identity(&bodies.source))?,
        capability_manifest: identity(record_identity(&bodies.manifest))?,
        selected_release_set: identity(release_set)?,
        registry_program: identity(registry.to_bytes())?,
        generation: input.generation,
    };
    let market_at = |generation: u64| -> Result<(Pubkey, MarketIdentity)> {
        let seeded = MarketIdentity {
            generation,
            ..template
        };
        let market =
            Pubkey::find_program_address(&MarketCoreStateSeedsV2::new(seeded).as_slices(), &core).0;
        // `market_id` is not one of the nine seeds, so the address is derived
        // with a placeholder there and completed afterwards — required not to
        // have moved, exactly as `derive_founding_coordinates` requires.
        let completed = MarketIdentity {
            market_id: identity_of(market.to_bytes())?,
            ..seeded
        };
        if Pubkey::find_program_address(&MarketCoreStateSeedsV2::new(completed).as_slices(), &core)
            .0
            != market
        {
            return Err(Error::new(
                "the Market address moved when its own identity was completed",
            ));
        }
        Ok((market, completed))
    };
    let (found31_market, _) = market_at(input.generation)?;
    let (open_market, open_market_identity) =
        market_at(PrestateLaneV1::Founding.generation(input)?)?;
    let (abort_market, _) = market_at(PrestateLaneV1::SourceAbort.generation(input)?)?;
    Ok(FoundingTargetsV1 {
        collateral_mint,
        realm_record,
        found31_market,
        open_market,
        abort_market,
        open_market_identity,
    })
}

/// What the chain holds at one derived Open-Market coordinate.
pub(crate) enum OpenMarketObservationV1 {
    /// No account.
    Absent,
    /// The account is exactly the Open Market this input founds: Core-owned,
    /// `STATE_BYTES` wide, `Phase::Open`, readiness consumed, identity equal —
    /// the market-account core of `authenticate_open_market_poststate_v1`.
    Open,
    /// An account exists that is not that, described.
    Other(String),
}

pub(crate) fn observe_open_market(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    targets: &FoundingTargetsV1,
) -> Result<OpenMarketObservationV1> {
    let core = pubkey(&plan.core.program_id)?;
    let Some(account) = rpc.account(targets.open_market)? else {
        return Ok(OpenMarketObservationV1::Absent);
    };
    if account.owner != core || account.executable || account.data.len() != STATE_BYTES {
        return Ok(OpenMarketObservationV1::Other(format!(
            "an account exists at the derived Market address {} that is not a Core Market: \
             owner {}, {} bytes, executable {}",
            targets.open_market,
            account.owner,
            account.data.len(),
            account.executable
        )));
    }
    let state = CoreState::decode(&account.data)
        .map_err(|error| Error::new(format!("derived Market account state: {error:?}")))?;
    if state.identity != targets.open_market_identity {
        return Ok(OpenMarketObservationV1::Other(format!(
            "a Core Market exists at {} whose identity is not the one this input derives",
            targets.open_market
        )));
    }
    if state.phase != Phase::Open || state.readiness != Readiness::Consumed {
        return Ok(OpenMarketObservationV1::Other(format!(
            "the Market at {} carries this input's identity but is not Open-with-consumed-\
             readiness: phase {:?}, readiness {:?}",
            targets.open_market, state.phase, state.readiness
        )));
    }
    Ok(OpenMarketObservationV1::Open)
}

/// Publish one finalized address lookup table covering an oversized frame.
///
/// Only non-signer coordinates and the invoked Program are routed; the fee
/// payer and every signer stay in the message's static key list. The table is
/// authority-owned so its rent stays recoverable, and it is never frozen.
pub(crate) fn publish_routing_table(
    rpc: &mut Rpc,
    payer: &Keypair,
    label: &str,
    instructions: &[Instruction],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(Observation, Vec<ObservedAccount>)> {
    let addresses = canonical_routing_addresses_v1(payer.pubkey(), instructions);
    let recent_slot = rpc.finalized_slot()?;
    let plan =
        build_lookup_table_creation_v1(payer.pubkey(), payer.pubkey(), recent_slot, &addresses)
            .map_err(|error| Error::new(format!("{label} routing table plan: {error:?}")))?;
    transactions.push(rpc.send(
        &format!("create {label} routing address lookup table"),
        std::slice::from_ref(&plan.create),
        payer,
    )?);
    for (index, extension) in plan.extensions.iter().enumerate() {
        transactions.push(rpc.send(
            &format!("extend {label} routing table page {index}"),
            std::slice::from_ref(extension),
            payer,
        )?);
    }
    let extended_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .ok_or_else(|| Error::new("routing table publication omitted a finalized slot"))?;
    // A table is only usable strictly after the slot that last extended it.
    let minimum_slot = extended_slot
        .checked_add(1)
        .ok_or_else(|| Error::new("routing table slot overflow"))?;
    await_finalized_slot(rpc, minimum_slot)?;
    let (observation, tables) =
        rpc.finalized_observed_accounts(&[plan.lookup_table], minimum_slot)?;
    Ok((observation, tables))
}

fn publish_frozen_founding_routing_table_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    label: &str,
    instructions: &[Instruction],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(Observation, Vec<ObservedAccount>)> {
    let addresses = canonical_routing_addresses_v1(payer.pubkey(), instructions);
    let recent_slot = rpc.finalized_slot()?;
    let plan =
        build_lookup_table_creation_v1(payer.pubkey(), payer.pubkey(), recent_slot, &addresses)
            .map_err(|error| Error::new(format!("{label} frozen routing plan: {error:?}")))?;
    transactions.push(rpc.send(
        &format!("create {label} frozen routing address lookup table"),
        std::slice::from_ref(&plan.create),
        payer,
    )?);
    for (index, extension) in plan.extensions.iter().enumerate() {
        transactions.push(rpc.send(
            &format!("extend {label} frozen routing table page {index}"),
            std::slice::from_ref(extension),
            payer,
        )?);
    }
    transactions.push(rpc.send(
        &format!("freeze {label} routing table after its one complete extension plan"),
        std::slice::from_ref(&build_lookup_table_freeze(
            plan.lookup_table,
            payer.pubkey(),
        )),
        payer,
    )?);
    let frozen_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .ok_or_else(|| Error::new("frozen routing publication omitted a finalized slot"))?;
    let minimum_slot = frozen_slot
        .checked_add(1)
        .ok_or_else(|| Error::new("frozen routing activation slot overflow"))?;
    await_finalized_slot(rpc, minimum_slot)?;
    let (observation, tables) =
        rpc.finalized_observed_accounts(&[plan.lookup_table], minimum_slot)?;
    let table = tables
        .first()
        .ok_or_else(|| Error::new("frozen routing observation omitted its table"))?;
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|error| Error::new(format!("frozen routing table bytes: {error:?}")))?;
    if table.owner != solana_address_lookup_table_interface::program::ID
        || table.executable
        || decoded.meta.authority.is_some()
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= observation.slot
        || decoded.addresses.as_ref() != plan.addresses.as_slice()
    {
        return Err(Error::new(
            "founding routing table was not exact, frozen, active, and activated",
        ));
    }
    Ok((observation, tables))
}

fn await_finalized_slot(rpc: &mut Rpc, minimum_slot: u64) -> Result<()> {
    // 300 seconds, not 60: a co-tenant laptop under several concurrent SBF
    // builds can stall finalization past a minute while the validator is
    // healthy (observed 2026-08-27, a full founding lost at the abort lane's
    // wait). A genuinely wedged validator still dies here, just later.
    for _ in 0..3_000 {
        if rpc.finalized_slot()? >= minimum_slot {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(Error::new(
        "validator did not finalize a slot after the routing table extension",
    ))
}

struct CreatedCollateralV1 {
    mint: Pubkey,
    wallet: Pubkey,
    local_participant_fixture_liquidity: Option<LocalParticipantFixtureLiquidityEvidenceV1>,
}

fn authenticate_collateral_supply_partition_v1(
    founding_collateral_atoms: u64,
    fixture_liquidity_atoms: u64,
    observed_supply_atoms: u64,
    observed_founding_wallet_atoms: u64,
    observed_fixture_source_atoms: Option<u64>,
    mint_authority_removed: bool,
) -> Result<u64> {
    let expected_supply = founding_collateral_atoms
        .checked_add(fixture_liquidity_atoms)
        .ok_or_else(|| Error::new("collateral fixture supply overflow"))?;
    if observed_supply_atoms != expected_supply
        || observed_founding_wallet_atoms != founding_collateral_atoms
        || observed_fixture_source_atoms
            != (fixture_liquidity_atoms != 0).then_some(fixture_liquidity_atoms)
        || !mint_authority_removed
    {
        return Err(Error::new(
            "collateral supply is not the exact founding/fixture partition with mint authority removed",
        ));
    }
    Ok(expected_supply)
}

fn create_real_collateral(
    rpc: &mut Rpc,
    payer: &Keypair,
    forge: &KeyForge,
    token_program: Pubkey,
    decimals: u8,
    atoms: u64,
    local_participant_fixture_liquidity_atoms: u64,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<CreatedCollateralV1> {
    if atoms == 0 {
        return Err(Error::new("initial collateral raw atoms must be positive"));
    }
    let mint = forge.keypair(role::COLLATERAL_MINT);
    let wallet = forge.keypair(role::COLLATERAL_WALLET);
    let total_supply_atoms = atoms
        .checked_add(local_participant_fixture_liquidity_atoms)
        .ok_or_else(|| Error::new("collateral fixture supply overflow"))?;
    let local_fixture = if local_participant_fixture_liquidity_atoms == 0 {
        None
    } else {
        if local_participant_fixture_liquidity_atoms != LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1
        {
            return Err(Error::new("local participant fixture amount changed"));
        }
        Some((
            forge.keypair(LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1),
            forge
                .keypair(LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1)
                .pubkey(),
        ))
    };
    let mint_rent = rpc.minimum_balance(MINT_BYTES)?;
    let wallet_rent = rpc.minimum_balance(ACCOUNT_BYTES)?;
    let mut initialize_mint = Vec::with_capacity(70);
    initialize_mint.extend_from_slice(&[20, decimals]);
    initialize_mint.extend_from_slice(payer.pubkey().as_ref());
    initialize_mint.extend_from_slice(&0_u32.to_le_bytes());
    initialize_mint.extend_from_slice(&[0_u8; 32]);
    let mut initialize_wallet = Vec::with_capacity(33);
    initialize_wallet.push(18);
    initialize_wallet.extend_from_slice(payer.pubkey().as_ref());
    let mint_to_checked = |amount: u64| {
        let mut data = Vec::with_capacity(10);
        data.push(14);
        data.extend_from_slice(&amount.to_le_bytes());
        data.push(decimals);
        data
    };
    let mut remove_authority = Vec::with_capacity(38);
    remove_authority.extend_from_slice(&[6, 0]);
    remove_authority.extend_from_slice(&0_u32.to_le_bytes());
    remove_authority.extend_from_slice(&[0_u8; 32]);
    let mut instructions = vec![
        create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            mint_rent,
            MINT_BYTES as u64,
            &token_program,
        ),
        Instruction {
            program_id: token_program,
            accounts: vec![AccountMeta::new(mint.pubkey(), false)],
            data: initialize_mint,
        },
        create_account(
            &payer.pubkey(),
            &wallet.pubkey(),
            wallet_rent,
            ACCOUNT_BYTES as u64,
            &token_program,
        ),
        Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(wallet.pubkey(), false),
                AccountMeta::new_readonly(mint.pubkey(), false),
            ],
            data: initialize_wallet,
        },
        Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(mint.pubkey(), false),
                AccountMeta::new(wallet.pubkey(), false),
                AccountMeta::new_readonly(payer.pubkey(), true),
            ],
            data: mint_to_checked(atoms),
        },
    ];
    if let Some((source, owner)) = &local_fixture {
        let mut initialize_source = Vec::with_capacity(33);
        initialize_source.push(18);
        initialize_source.extend_from_slice(owner.as_ref());
        instructions.extend([
            create_account(
                &payer.pubkey(),
                &source.pubkey(),
                wallet_rent,
                ACCOUNT_BYTES as u64,
                &token_program,
            ),
            Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(source.pubkey(), false),
                    AccountMeta::new_readonly(mint.pubkey(), false),
                ],
                data: initialize_source,
            },
            Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(mint.pubkey(), false),
                    AccountMeta::new(source.pubkey(), false),
                    AccountMeta::new_readonly(payer.pubkey(), true),
                ],
                data: mint_to_checked(local_participant_fixture_liquidity_atoms),
            },
        ]);
    }
    instructions.push(Instruction {
        program_id: token_program,
        accounts: vec![
            AccountMeta::new(mint.pubkey(), false),
            AccountMeta::new_readonly(payer.pubkey(), true),
        ],
        data: remove_authority,
    });
    let mut signers = vec![&mint, &wallet];
    if let Some((source, _)) = &local_fixture {
        signers.push(source);
    }
    let transaction = rpc.send_with_signers(
        if local_fixture.is_some() {
            "create immutable Token-2022 collateral plus explicit local participant fixture liquidity"
        } else {
            "create real Token-2022 collateral and raw-atom wallet"
        },
        &instructions,
        payer,
        &signers,
    )?;
    let mint_account = rpc.required_account(mint.pubkey(), "collateral Mint")?;
    let wallet_account = rpc.required_account(wallet.pubkey(), "collateral token wallet")?;
    let parsed_mint = Mint::parse(&mint_account.data)
        .map_err(|error| Error::new(format!("collateral Mint: {error:?}")))?;
    let parsed_wallet = TokenAccount::parse(&wallet_account.data)
        .map_err(|error| Error::new(format!("collateral wallet: {error:?}")))?;
    if mint_account.owner != token_program
        || wallet_account.owner != token_program
        || !parsed_mint.freeze_authority.is_none()
        || !parsed_mint.is_initialized
        || parsed_mint.decimals != decimals
        || parsed_wallet.mint != mint.pubkey().to_bytes()
        || parsed_wallet.owner != payer.pubkey().to_bytes()
        || parsed_wallet.state != AccountState::Initialized
        || !parsed_wallet.delegate.is_none()
        || !parsed_wallet.native_reserve.is_none()
        || !parsed_wallet.close_authority.is_none()
    {
        return Err(Error::new(
            "real Token-2022 collateral poststate refused exact base profile",
        ));
    }
    let local_participant_fixture_liquidity = match local_fixture {
        None => {
            authenticate_collateral_supply_partition_v1(
                atoms,
                0,
                parsed_mint.supply,
                parsed_wallet.amount,
                None,
                parsed_mint.mint_authority.is_none(),
            )?;
            None
        }
        Some((source, owner)) => {
            let source_account = rpc.required_account(
                source.pubkey(),
                "local participant fixture collateral source",
            )?;
            let parsed_source = TokenAccount::parse(&source_account.data).map_err(|error| {
                Error::new(format!("local participant fixture source: {error:?}"))
            })?;
            if source_account.owner != token_program
                || parsed_source.mint != mint.pubkey().to_bytes()
                || parsed_source.owner != owner.to_bytes()
                || parsed_source.amount != local_participant_fixture_liquidity_atoms
                || parsed_source.state != AccountState::Initialized
                || !parsed_source.delegate.is_none()
                || parsed_source.delegated_amount != 0
                || !parsed_source.native_reserve.is_none()
                || !parsed_source.close_authority.is_none()
            {
                return Err(Error::new(
                    "local participant fixture source refused exact immutable base-token profile",
                ));
            }
            authenticate_collateral_supply_partition_v1(
                atoms,
                local_participant_fixture_liquidity_atoms,
                parsed_mint.supply,
                parsed_wallet.amount,
                Some(parsed_source.amount),
                parsed_mint.mint_authority.is_none(),
            )?;
            Some(LocalParticipantFixtureLiquidityEvidenceV1 {
                source_token_account: source.pubkey().to_string(),
                source_owner: owner.to_string(),
                quantity_atoms: local_participant_fixture_liquidity_atoms,
                founding_collateral_atoms: atoms,
                total_supply_atoms,
                mint: mint.pubkey().to_string(),
                mint_authority_removed: parsed_mint.mint_authority.is_none(),
                transaction_signature: transaction.signature.clone(),
                finalized_slot: transaction.slot,
                compute_units_consumed: transaction.compute_units_consumed.ok_or_else(|| {
                    Error::new("local participant fixture transaction omitted compute units")
                })?,
            })
        }
    };
    transactions.push(transaction);
    Ok(CreatedCollateralV1 {
        mint: mint.pubkey(),
        wallet: wallet.pubkey(),
        local_participant_fixture_liquidity,
    })
}

fn found_snapshot_keys(
    plan: &SuccessorPlan,
    payer: Pubkey,
    market: Pubkey,
    credit: Pubkey,
    records: &MarketRecords,
) -> Result<Vec<Pubkey>> {
    let registry_artifact = record(plan, "registry_artifact_release")?;
    let rent_artifact = record(plan, "rent_artifact_release")?;
    let floor = manipulation_floor_pair(pubkey(&plan.registry.program_id)?, records);
    let keys = vec![
        payer,
        market,
        credit,
        pubkey(&plan.rent_credit.program_id)?,
        records.realm.raw,
        records.realm.staging,
        records.product.raw,
        records.product.staging,
        records.domain.raw,
        records.domain.staging,
        records.portfolio.raw,
        records.portfolio.staging,
        records.basis.raw,
        records.basis.staging,
        records.source.raw,
        records.source.staging,
        records.source_spec.raw,
        records.source_spec.staging,
        records.source_capacity_profile.raw,
        records.source_capacity_profile.staging,
        floor.0,
        floor.1,
        records.manifest.raw,
        records.manifest.staging,
        pubkey(&plan.activation)?,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        pubkey(&plan.registry.program_id)?,
        sysvar::rent::ID,
        system_program::ID,
        pubkey(&plan.infrastructure_profile.address)?,
        registry_artifact.0,
        registry_artifact.1,
        pubkey(&plan.registry.programdata_id)?,
        rent_artifact.0,
        rent_artifact.1,
        pubkey(&plan.rent_credit.programdata_id)?,
    ];
    authenticate_found_snapshot_coordinates_v3(&keys, records.manifest.raw)?;
    Ok(keys)
}

fn authenticate_found_snapshot_coordinates_v3(
    keys: &[Pubkey],
    capability_manifest_raw: Pubkey,
) -> Result<()> {
    if keys.len() != FOUND_ACCOUNT_COUNT_V3
        || keys.get(FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3) != Some(&capability_manifest_raw)
        || keys.get(FOUND_RENT_SYSVAR_INDEX_V3) != Some(&sysvar::rent::ID)
    {
        return Err(Error::new(
            "ordinary Found37 capability-manifest coordinate drifted",
        ));
    }
    Ok(())
}

/// Complete ordinary ProjectFound V2 graph with runtime Rent supplied by Core.
fn ordinary_project_found_snapshot_keys_v2(
    plan: &SuccessorPlan,
    payer: Pubkey,
    market: Pubkey,
    credit: Pubkey,
    records: &MarketRecords,
) -> Result<Vec<Pubkey>> {
    let mut keys = found_snapshot_keys(plan, payer, market, credit, records)?;
    keys.remove(FOUND_RENT_SYSVAR_INDEX_V3);
    if keys.len() != PROJECT_FOUND_ACCOUNT_COUNT_V2 {
        return Err(Error::new(
            "ordinary ProjectFound36 runtime-sysvar erasure drifted",
        ));
    }
    Ok(keys)
}

/// The compact ProjectedFound V2 prefix consumed inside DCLTGMF3.
///
/// Realm, SourceMaterial, SourceSpec, capacity profile, manipulation floor,
/// and linked basis were authenticated when Custody created the projection.
/// Re-presenting those finalized Registry pairs here would create no new
/// authority and is precisely the lock wall this route removes.
fn projected_found_snapshot_keys_v2(
    plan: &SuccessorPlan,
    payer: Pubkey,
    market: Pubkey,
    credit: Pubkey,
    records: &MarketRecords,
) -> Result<Vec<Pubkey>> {
    let registry_artifact = record(plan, "registry_artifact_release")?;
    let rent_artifact = record(plan, "rent_artifact_release")?;
    Ok(vec![
        payer,
        market,
        credit,
        pubkey(&plan.rent_credit.program_id)?,
        records.product.raw,
        records.product.staging,
        records.domain.raw,
        records.domain.staging,
        records.portfolio.raw,
        records.portfolio.staging,
        records.manifest.raw,
        records.manifest.staging,
        pubkey(&plan.activation)?,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        pubkey(&plan.registry.program_id)?,
        // The runtime-owned Rent sysvar is elided from the PROJECTED frame:
        // Core's `FoundAccounts::parse_project` (rent_elided) reads 24
        // accounts with registry_program followed directly by system.
        // Including it here made the assembled generic founding frame one
        // account wider than the 125-pinned spec and refused every founding.
        system_program::ID,
        pubkey(&plan.infrastructure_profile.address)?,
        registry_artifact.0,
        registry_artifact.1,
        pubkey(&plan.registry.programdata_id)?,
        rent_artifact.0,
        rent_artifact.1,
        pubkey(&plan.rent_credit.programdata_id)?,
    ])
}

fn manipulation_floor_pair(registry: Pubkey, records: &MarketRecords) -> (Pubkey, Pubkey) {
    records.manipulation_floor.map_or_else(
        || {
            let absent = [0_u8; 32];
            (
                Pubkey::find_program_address(
                    &[
                        RAW_RECORD_PDA_SEED_V1,
                        &MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
                        &absent,
                    ],
                    &registry,
                )
                .0,
                Pubkey::find_program_address(
                    &[
                        STAGING_CURSOR_PDA_SEED_V1,
                        &MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
                        &absent,
                    ],
                    &registry,
                )
                .0,
            )
        },
        |record| (record.raw, record.staging),
    )
}

fn finalized_snapshot(
    rpc: &mut Rpc,
    keys: &[Pubkey],
    minimum_slot: u64,
) -> Result<FinalizedSnapshot> {
    let mut ordered = keys.to_vec();
    ordered.sort();
    if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::new("Found snapshot address set contained aliases"));
    }
    let (slot, values) = rpc.finalized_accounts(keys, minimum_slot)?;
    let accounts = keys.iter().copied().zip(values).collect::<BTreeMap<_, _>>();
    Ok(FinalizedSnapshot { slot, accounts })
}

fn projection_state<'a>(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    snapshot: &'a FinalizedSnapshot,
    payer: Pubkey,
    market: Pubkey,
    records: &MarketRecords,
) -> Result<FoundProjectionStateV2<'a>> {
    let registry_artifact = record(plan, "registry_artifact_release")?;
    let rent_artifact = record(plan, "rent_artifact_release")?;
    let floor = manipulation_floor_pair(pubkey(&plan.registry.program_id)?, records);
    let record_observation =
        |rpc: &mut Rpc, published: PublishedRecord| snapshot.finalized_record(rpc, published);
    Ok(FoundProjectionStateV2 {
        payer: snapshot.observation(payer)?,
        market: snapshot.observation(market)?,
        rent_program: snapshot.observation(pubkey(&plan.rent_credit.program_id)?)?,
        realm: FinalizedReferenceObservationV2 {
            schema_id: REALM_SCHEMA_RELEASE_ID_V1,
            record: record_observation(rpc, records.realm)?,
        },
        product: record_observation(rpc, records.product)?,
        result_domain: record_observation(rpc, records.domain)?,
        portfolio: record_observation(rpc, records.portfolio)?,
        linked_basis: record_observation(rpc, records.basis)?,
        source_material: FinalizedReferenceObservationV2 {
            schema_id: SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            record: record_observation(rpc, records.source)?,
        },
        source_spec: FinalizedReferenceObservationV2 {
            schema_id: SOURCE_SPEC_SCHEMA_ID_V1,
            record: record_observation(rpc, records.source_spec)?,
        },
        capacity_profile: FinalizedReferenceObservationV2 {
            schema_id: SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
            record: record_observation(rpc, records.source_capacity_profile)?,
        },
        manipulation_floor: FinalizedReferenceObservationV2 {
            schema_id: MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            record: snapshot_record(rpc, snapshot, floor)?,
        },
        capability_manifest: FinalizedReferenceObservationV2 {
            schema_id: CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            record: record_observation(rpc, records.manifest)?,
        },
        activation_cache: snapshot.observation(pubkey(&plan.activation)?)?,
        core_program: snapshot.observation(pubkey(&plan.core.program_id)?)?,
        core_programdata: snapshot.observation(pubkey(&plan.core.programdata_id)?)?,
        registry_program: snapshot.observation(pubkey(&plan.registry.program_id)?)?,
        rent: snapshot.observation(sysvar::rent::ID)?,
        system_program: snapshot.observation(system_program::ID)?,
        infrastructure_profile: snapshot
            .observation(pubkey(&plan.infrastructure_profile.address)?)?,
        registry_artifact: snapshot_record(rpc, snapshot, registry_artifact)?,
        registry_programdata: snapshot.observation(pubkey(&plan.registry.programdata_id)?)?,
        rent_artifact: snapshot_record(rpc, snapshot, rent_artifact)?,
        rent_programdata: snapshot.observation(pubkey(&plan.rent_credit.programdata_id)?)?,
    })
}

fn found_state<'a>(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    snapshot: &'a FinalizedSnapshot,
    payer: Pubkey,
    market: Pubkey,
    credit: Pubkey,
    records: &MarketRecords,
) -> Result<FoundStateV2<'a>> {
    let projection = projection_state(rpc, plan, snapshot, payer, market, records)?;
    Ok(FoundStateV2 {
        payer: projection.payer,
        market: projection.market,
        rent_credit: snapshot.observation(credit)?,
        rent_program: projection.rent_program,
        realm: projection.realm,
        product: projection.product,
        result_domain: projection.result_domain,
        portfolio: projection.portfolio,
        linked_basis: projection.linked_basis,
        source_material: projection.source_material,
        source_spec: projection.source_spec,
        capacity_profile: projection.capacity_profile,
        manipulation_floor: projection.manipulation_floor,
        capability_manifest: projection.capability_manifest,
        activation_cache: projection.activation_cache,
        core_program: projection.core_program,
        core_programdata: projection.core_programdata,
        registry_program: projection.registry_program,
        rent: projection.rent,
        system_program: projection.system_program,
        infrastructure_profile: projection.infrastructure_profile,
        registry_artifact: projection.registry_artifact,
        registry_programdata: projection.registry_programdata,
        rent_artifact: projection.rent_artifact,
        rent_programdata: projection.rent_programdata,
    })
}

fn snapshot_record<'a>(
    rpc: &mut Rpc,
    snapshot: &'a FinalizedSnapshot,
    pair: (Pubkey, Pubkey),
) -> Result<FinalizedRecordObservationV2<'a>> {
    let raw = snapshot.observation(pair.0)?;
    Ok(FinalizedRecordObservationV2 {
        raw,
        staging: snapshot.observation(pair.1)?,
        raw_rent_minimum: rpc.minimum_balance(raw.data.len())?,
    })
}

fn product_id(value: &str) -> Result<ProductContentId> {
    ProductContentId::new(hex32(value)?)
        .map_err(|error| Error::new(format!("Product content ID: {error:?}")))
}

fn source_id(value: &str) -> Result<SourceContentId> {
    SourceContentId::new(hex32(value)?)
        .map_err(|error| Error::new(format!("Source content ID: {error:?}")))
}

/// One source identity from raw bytes rather than from a hex string.
pub(crate) fn source_content(bytes: [u8; 32]) -> Result<SourceContentId> {
    SourceContentId::new(bytes).map_err(|error| Error::new(format!("Source content ID: {error:?}")))
}

/// The SHA-256 a finalized record's address is derived from.
pub(crate) fn record_identity(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn identity(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|error| Error::new(format!("Market identity: {error:?}")))
}

fn canonical_i128(value: &str) -> Result<i128> {
    let parsed = value
        .parse::<i128>()
        .map_err(|error| Error::new(format!("cut numerator {value:?}: {error}")))?;
    if parsed.to_string() != value {
        return Err(Error::new(
            "cut numerators must use canonical decimal spelling",
        ));
    }
    Ok(parsed)
}

// ---------------------------------------------------------------------------
// DCLTPCB2 - projected Custody staging against prepared controller ledgers.
//
// Four Custody transitions in one rollback domain: Initialize, OpenHoard,
// OpenSourceCompartment, and the FundingState staging Trading performs itself.
// Every coordinate below is derived from facts this campaign already
// established on chain - the published record graph, the activated release
// set, the live Rent sysvar - or from a PDA seed order owned by the program
// that will re-derive it. The runner authors no digest.
// ---------------------------------------------------------------------------

/// Domain separating this campaign's founding action contexts.
const FOUNDING_CONTEXT_DOMAIN_V1: &[u8] = b"dclutch/local-campaign/founding-context/v1";

/// Sole top-level projected-Custody founding-bootstrap instruction.
///
/// Owned by `programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs`
/// (`PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V1`); restated here because a localhost
/// host utility does not depend on an SBF program crate. The route carries no
/// payload, so these eight bytes are the whole instruction data.
const PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V2: [u8; 8] = *b"DCLTPCB2";

/// Exact projected-Custody bootstrap frame width before the funding tail.
///
/// 78 physical accounts plus the instructions sysvar the route presents to its
/// own entrypoint. `entrypoint_adapter::admit_heap_frame_v1` re-derives the
/// transaction's heap grant from that sysvar and scans the instruction's own
/// account list to find it, so a frame without it keeps the compile-time
/// 32 KiB ceiling this route was measured exhausting at stage three.
const PROJECTED_CUSTODY_BOOTSTRAP_COMMON_ACCOUNTS_V2: usize = 84;
const PROJECTED_CUSTODY_BOOTSTRAP_CHECKPOINT_V2: usize = 84;
const PROJECTED_CUSTODY_BOOTSTRAP_RESOLUTION_LEDGER_V2: usize = 85;
const PROJECTED_CUSTODY_BOOTSTRAP_TRADING_LEDGER_V2: usize = 86;
const PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNTS_V2: usize = 87;
const PROJECTED_CUSTODY_BOOTSTRAP_COMPLETE_KEYS_V2: usize = 60;
const DEVNET_ACCOUNT_LOCK_LIMIT_V1: usize = 64;

const CONTROLLER_FUNDING_PREPARE_MAGIC_V1: [u8; 8] = *b"DCLTCFQ1";
const CONTROLLER_FUNDING_PREPARE_ACCOUNTS_V1: usize = 48;
const CONTROLLER_FUNDING_PREPARE_COMPLETE_KEYS_V1: usize = 49;
const CONTROLLER_FUNDING_PREPARE_FUNDING_SOURCE_V1: usize = 11;
const CONTROLLER_FUNDING_PREPARE_FOUND_START_V1: usize = 12;
const CONTROLLER_FUNDING_PREPARE_FOUND_RENT_CREDIT_V1: usize =
    CONTROLLER_FUNDING_PREPARE_FOUND_START_V1 + 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompiledMessageGeometryV1 {
    complete_keys: usize,
    required_signatures: usize,
    static_keys: usize,
    loaded_writable: usize,
    loaded_readonly: usize,
    message_bytes: usize,
    packet_bytes: usize,
}

fn dcltpcb2_completion_lines_v1(geometry: CompiledMessageGeometryV1) -> Vec<String> {
    vec![
        "derived one generic founding's complete coordinate set - Market, credit, action context, Custody compartments, capability root, prepared controller checkpoint, and FundingLedgerV2 list - from the finalized record graph alone".into(),
        "proved DCLTPCB2 admits only the terminal Lock request its founding determines".into(),
        "proved a reordered controller-ledger pair refuses and rolls the whole bootstrap back to a fee-only debit".into(),
        "executed DCLTPCB2: projected replay at SourceFunded, empty Hoard vault, funded source compartment, and a CustodyStaged checkpoint while both prepared FundingLedgerV2 accounts remained byte- and lamport-exact".into(),
        format!(
            "compiled the exact bounded DCLTPCB2 transaction with payer, three ComputeBudget declarations, and its canonical address table: {} complete keys ({} static, {} writable loaded, {} readonly loaded), {} signatures, {} message bytes, and {} fully signed packet bytes; +4 distinct keys reaches 64 and +5 reaches the refused 65-key boundary",
            geometry.complete_keys,
            geometry.static_keys,
            geometry.loaded_writable,
            geometry.loaded_readonly,
            geometry.required_signatures,
            geometry.message_bytes,
            geometry.packet_bytes,
        ),
    ]
}

/// Compile the exact bounded transaction shape used by the successor runner.
///
/// This intentionally consumes the finished instruction rather than restating
/// its reference count. The canonical table selection is the same one
/// `publish_routing_table` uses, and the ComputeBudget prefix comes from the
/// same builder `Rpc::send_v0_on_founding_heap_with_signers` calls.
fn projected_bootstrap_compiled_geometry_v2(
    payer: Pubkey,
    instruction: &Instruction,
) -> Result<CompiledMessageGeometryV1> {
    let mut addresses = Vec::new();
    let mut push = |key: Pubkey| {
        if key != payer && !addresses.contains(&key) {
            addresses.push(key);
        }
    };
    push(instruction.program_id);
    for meta in &instruction.accounts {
        if !meta.is_signer {
            push(meta.pubkey);
        }
    }
    let routing = build_lookup_table_creation_v1(payer, payer, 1, &addresses)
        .map_err(|error| Error::new(format!("DCLTPCB2 census table: {error:?}")))?;
    let bounded = bounded_instructions(std::slice::from_ref(instruction), Some(256_u32 * 1024))?;
    let message = v0::Message::try_compile(
        &payer,
        &bounded,
        &[AddressLookupTableAccount {
            key: routing.lookup_table,
            addresses: routing.addresses,
        }],
        Hash::new_from_array([0x42; 32]),
    )
    .map_err(|error| Error::new(format!("DCLTPCB2 census compile: {error}")))?;
    let static_keys = message.account_keys.len();
    let loaded_writable = message
        .address_table_lookups
        .iter()
        .map(|lookup| lookup.writable_indexes.len())
        .sum::<usize>();
    let loaded_readonly = message
        .address_table_lookups
        .iter()
        .map(|lookup| lookup.readonly_indexes.len())
        .sum::<usize>();
    let complete_keys = static_keys
        .checked_add(loaded_writable)
        .and_then(|value| value.checked_add(loaded_readonly))
        .ok_or_else(|| Error::new("DCLTPCB2 complete-key census overflow"))?;
    let required_signatures = usize::from(message.header.num_required_signatures);
    let versioned_message = VersionedMessage::V0(message);
    let message_bytes = versioned_message.serialize().len();
    let packet_bytes = bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default(); required_signatures],
        message: versioned_message,
    })
    .map_err(|error| Error::new(format!("DCLTPCB2 packet serialization: {error}")))?
    .len();
    Ok(CompiledMessageGeometryV1 {
        complete_keys,
        required_signatures,
        static_keys,
        loaded_writable,
        loaded_readonly,
        message_bytes,
        packet_bytes,
    })
}

fn append_distinct_census_accounts_v1(instruction: &Instruction, count: usize) -> Instruction {
    let mut expanded = instruction.clone();
    let mut counter = 0_u64;
    while expanded.accounts.len() < instruction.accounts.len().saturating_add(count) {
        let mut hasher = Sha256::new();
        hasher.update(b"dclutch/census/dcltpcb2/distinct-key-v1");
        hasher.update(counter.to_le_bytes());
        counter = counter.saturating_add(1);
        let key = Pubkey::new_from_array(hasher.finalize().into());
        if key != expanded.program_id && !expanded.accounts.iter().any(|meta| meta.pubkey == key) {
            expanded
                .accounts
                .push(AccountMeta::new_readonly(key, false));
        }
    }
    expanded
}

fn authenticate_cleanup_compiled_census_v1(
    payer: Pubkey,
    instruction: &Instruction,
    base: CompiledMessageGeometryV1,
) -> Result<()> {
    let admitted = projected_bootstrap_compiled_geometry_v2(
        payer,
        &append_distinct_census_accounts_v1(instruction, CONTROLLER_FUNDING_CLEANUP_CENSUS_PADDING_V1),
    )?;
    let refused = projected_bootstrap_compiled_geometry_v2(
        payer,
        &append_distinct_census_accounts_v1(instruction, CONTROLLER_FUNDING_CLEANUP_CENSUS_PADDING_V1 + 1),
    )?;
    if base.complete_keys != CONTROLLER_FUNDING_CLEANUP_COMPLETE_KEYS_V1
        || admitted.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1
        || refused.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1
    {
        return Err(Error::new(format!(
            "controller cleanup census refused: base {}, +45 {}, +46 {}",
            base.complete_keys, admitted.complete_keys, refused.complete_keys,
        )));
    }
    Ok(())
}

/// Every coordinate one generic founding determines, derived once.
struct FoundingCoordinates {
    generation: u64,
    identity: MarketIdentity,
    market: Pubkey,
    credit: Pubkey,
    hoard_vault: Pubkey,
    source_vault: Pubkey,
    source_replay: Pubkey,
    projected_replay: Pubkey,
    custody_authority: Pubkey,
    controller_funding_checkpoint: Pubkey,
    context: [u8; 32],
    principal_cap_sets: u64,
    capability_entry_count: u16,
    capability_entry_index: u16,
    funding_ledgers: Vec<FoundingFundingLedgerV2>,
    found: GenericFoundingRequestV1,
    lock: ProjectedCustodyRequestV1,
}

#[derive(Clone, Debug)]
struct FoundingFundingLedgerV2 {
    address: Pubkey,
    controller: Pubkey,
    selected_mask: u16,
    bytes: Vec<u8>,
    required_lamports: u64,
}

fn manifest_required_union_v1(entry_count: u16) -> Result<u16> {
    if entry_count == 0 || entry_count > u16::BITS as u16 {
        return Err(Error::new(
            "capability manifest entry mask width is invalid",
        ));
    }
    if entry_count == u16::BITS as u16 {
        Ok(u16::MAX)
    } else {
        1_u16
            .checked_shl(u32::from(entry_count))
            .and_then(|bound| bound.checked_sub(1))
            .ok_or_else(|| Error::new("capability manifest entry mask overflow"))
    }
}

fn direct_founding_controller_masks_v1(
    manifest: CapabilityManifestV1<'_>,
    resolution_release: [u8; 32],
) -> Result<(u16, [u16; 2])> {
    let mut direct_index = None;
    let mut resolution_mask = 0_u16;
    for entry_index in 0..manifest.entry_count() {
        let entry = manifest
            .entry(entry_index)
            .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
        let bit = 1_u16
            .checked_shl(u32::from(entry_index))
            .ok_or_else(|| Error::new("capability entry mask overflow"))?;
        if entry.kind_id().to_bytes() == DIRECT_SUCCESSOR_KIND_ID_V3 {
            if direct_index.replace(entry_index).is_some()
                || entry.release_id().to_bytes() == resolution_release
            {
                return Err(Error::new(
                    "the founding manifest must contain exactly one non-Resolution Direct entry",
                ));
            }
        } else if entry.release_id().to_bytes() == resolution_release {
            resolution_mask |= bit;
        } else {
            return Err(Error::new(format!(
                "manifest companion entry {entry_index} does not name the exact activated Resolution release"
            )));
        }
    }
    let direct_index = direct_index
        .ok_or_else(|| Error::new("the founding manifest omitted its Direct capability entry"))?;
    let trading_mask = 1_u16
        .checked_shl(u32::from(direct_index))
        .ok_or_else(|| Error::new("Direct capability entry mask overflow"))?;
    let required_union = manifest_required_union_v1(manifest.entry_count())?;
    if manifest.entry_count() != 4
        || resolution_mask.count_ones() != 3
        || resolution_mask & trading_mask != 0
        || resolution_mask | trading_mask != required_union
    {
        return Err(Error::new(
            "Direct founding requires one Direct entry and three exact Resolution companions",
        ));
    }
    Ok((direct_index, [resolution_mask, trading_mask]))
}

/// Derive one founding's complete coordinate set from the finalized graph.
///
/// The order is forced and acyclic. The Custody vault, replay, and projection
/// addresses depend only on the Market, release set, and action context. The
/// FundingState addresses depend only on the authenticated manifest. The
/// artifact's capability root is derived last, from a root-free preimage, by
/// the shared operator - hashing the finished artifact instead would require
/// the artifact to carry the address of a PDA whose seeds already contain that
/// hash.
#[allow(clippy::too_many_arguments)]
fn derive_founding_coordinates(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    records: &MarketRecords,
    identity_template: MarketIdentity,
    product: ProductContentId,
    mint: Pubkey,
    payer: Pubkey,
    founder: Pubkey,
    beneficiary: Pubkey,
    generation: u64,
    expiry_slot: u64,
) -> Result<FoundingCoordinates> {
    let core = pubkey(&plan.core.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let release_set = hex32(&plan.release_set_id)?;

    // The generic founding creates its own Market. It cannot reuse the one
    // Found37 already created: every projected-Custody stage asserts the
    // inverse of a live Market, and Core's own projection requires the Market
    // account vacant. A distinct generation is a distinct, still-vacant PDA.
    let identity = MarketIdentity {
        generation,
        ..identity_template
    };
    let market =
        Pubkey::find_program_address(&MarketCoreStateSeedsV2::new(identity).as_slices(), &core).0;
    // `market_id` is not one of the nine seeds, so the template carries a
    // placeholder there and the address is derived without it. The Core state
    // Found writes carries the real address, and this campaign has to commit to
    // that state's digest two stages before it exists, so the placeholder is
    // replaced here and the derivation is required not to have moved.
    let identity = MarketIdentity {
        market_id: identity_of(market.to_bytes())?,
        ..identity
    };
    if Pubkey::find_program_address(&MarketCoreStateSeedsV2::new(identity).as_slices(), &core).0
        != market
    {
        return Err(Error::new(
            "the Market address moved when its own identity was completed",
        ));
    }
    let credit = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &rent_program,
    )
    .0;

    // The action context namespaces this founding under its own Market and
    // generation, so two foundings can never share a permit, a caller PDA, or
    // a Custody compartment.
    let mut hasher = Sha256::new();
    hasher.update(FOUNDING_CONTEXT_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(market.as_ref());
    hasher.update(generation.to_le_bytes());
    hasher.update(release_set);
    let context: [u8; 32] = hasher.finalize().into();
    let context_digest: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(PROJECTED_HOARD_CONTEXT_DOMAIN_V1);
        hasher.update(context);
        hasher.finalize().into()
    };
    if context_digest == context {
        return Err(Error::new("founding context collided with its own digest"));
    }

    let market_bytes = market.to_bytes();
    let hoard_seeds = CustodyVaultSeedsV1::new(
        market_bytes,
        release_set,
        context_digest,
        CompartmentV1::HoardPrincipal,
    );
    let hoard_vault = Pubkey::find_program_address(&hoard_seeds.as_slices(), &custody).0;
    // Settlement is the compartment the request codec admits for a founding
    // source; None, External, and HoardPrincipal are refused outright.
    let source_seeds = CustodyVaultSeedsV1::new(
        market_bytes,
        release_set,
        context,
        CompartmentV1::Settlement,
    );
    let source_vault = Pubkey::find_program_address(&source_seeds.as_slices(), &custody).0;
    // Both coordinates are the ordinary replay namespace, whose seeds carry the
    // executing role. The founding is Trading throughout: `open_source_compartment`
    // mints a Trading-role source replay, and `RealizeAndClose` rewrites the
    // projection in place as the Market's Trading-role live replay.
    let source_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(market_bytes, release_set, CallerRoleV1::Trading, context)
            .as_slices(),
        &custody,
    )
    .0;
    let projected_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market_bytes,
            release_set,
            CallerRoleV1::Trading,
            context_digest,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market_bytes, &release_set],
        &custody,
    )
    .0;

    // One canonical controller-subset FundingLedgerV2 per nonempty controller
    // mask. The physical list is ordered by each mask's lowest manifest bit;
    // the generic founding artifact commits to these physical addresses, not
    // to a caller-authored logical count.
    let manifest_bytes = decode_hex(&input.capability_manifest_hex)?;
    let manifest = CapabilityManifestV1::decode(&manifest_bytes)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
    let manifest_id = CapabilityContentId::new(records.manifest.digest)
        .map_err(|error| Error::new(format!("manifest identity: {error:?}")))?;
    if hex32(&plan.trading.semantic_release_id)? != COMPILED_DIRECT_RELEASE_ID_V1 {
        return Err(Error::new(
            "the activated Trading artifact does not name the compiled Direct controller release",
        ));
    }
    let resolution = pubkey(&plan.resolution.program_id)?;
    let resolution_release = hex32(&plan.resolution.semantic_release_id)?;
    let (capability_entry_index, [resolution_mask, trading_mask]) =
        direct_founding_controller_masks_v1(manifest, resolution_release)?;
    let mut subsets = vec![(resolution_mask, resolution), (trading_mask, trading)];
    subsets.sort_by_key(|(mask, _)| mask.trailing_zeros());
    let mut funding_ledgers = Vec::with_capacity(subsets.len());
    let mut funding_identities = Vec::new();
    for (selected_mask, controller) in subsets {
        let slot_count = u16::try_from(selected_mask.count_ones())
            .map_err(|_| Error::new("funding-ledger slot count overflow"))?;
        let mut bytes = vec![
            0_u8;
            funding_ledger_bytes_v2(slot_count).map_err(|error| Error::new(
                format!("FundingLedgerV2 width: {error:?}")
            ))?
        ];
        FundingLedgerV2::initialize(&mut bytes, manifest_id, manifest, selected_mask)
            .map_err(|error| Error::new(format!("FundingLedgerV2 initialization: {error:?}")))?;
        let ledger = FundingLedgerV2::decode(&bytes)
            .map_err(|error| Error::new(format!("FundingLedgerV2: {error:?}")))?;
        let required_lamports = rpc
            .minimum_balance(bytes.len())?
            .checked_add(
                ledger
                    .authenticate(manifest_id, manifest)
                    .and_then(|value| value.remaining_native_lamports_total())
                    .map_err(|error| {
                        Error::new(format!("FundingLedgerV2 native custody: {error:?}"))
                    })?,
            )
            .ok_or_else(|| Error::new("funding-ledger prepayment overflow"))?;
        let derivation = CapabilityFundingLedgerDerivationV2::new(
            controller.to_bytes(),
            market_bytes,
            generation,
            manifest_id,
            ledger,
        )
        .map_err(|error| Error::new(format!("funding derivation: {error:?}")))?;
        let address = Pubkey::find_program_address(&derivation.seed_components(), &controller).0;
        funding_ledgers.push(FoundingFundingLedgerV2 {
            address,
            controller,
            selected_mask,
            bytes,
            required_lamports,
        });
        funding_identities.push(identity_of(address.to_bytes())?);
    }
    let funding_list_id = generic_founding_funding_list_id_v1(&funding_identities)
        .map_err(|error| Error::new(format!("funding list identity: {error:?}")))?;

    // The complete-set quantity times the basis scale is the Hoard principal.
    // A categorical Product's payout scale is one, and Core refuses any
    // artifact whose basis scale differs from the published basis record's.
    let basis_scale = 1_u64;
    let quantity = input
        .initial_collateral_atoms
        .checked_div(2)
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::new("collateral supply cannot fund a founding"))?;
    let principal_cap_sets = records.principal_cap_sets;
    let market_rent = rpc.minimum_balance(STATE_BYTES)?;
    let permit_rent = rpc.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)?;

    let funding_count = u8::try_from(funding_ledgers.len())
        .map_err(|_| Error::new("capability funding width overflow"))?;
    let template = GenericFoundingRequestV1::new(
        GenericFoundingStageV1::FoundAndPermit,
        funding_count,
        identity_of(release_set)?,
        identity_of(market_bytes)?,
        identity_of([1; 32])?,
        identity_of(context)?,
        identity_of(founder.to_bytes())?,
        identity_of(beneficiary.to_bytes())?,
        identity_of(source_vault.to_bytes())?,
        identity_of(hoard_vault.to_bytes())?,
        identity_of(projected_replay.to_bytes())?,
        funding_list_id,
        generation,
        quantity,
        basis_scale,
        expiry_slot,
        market_rent,
        permit_rent,
        GENERIC_FOUNDING_PROJECTED_RESULTING_REVISION_V1,
        capability_entry_index,
    )
    .map_err(|error| Error::new(format!("founding artifact template: {error:?}")))?;
    let selection =
        construct_generic_founding_root_selection_v1(trading, template, &manifest_bytes)
            .map_err(|error| Error::new(format!("capability-root selection: {error:?}")))?;
    let found = selection.request;

    // The projection receipt is a pure function of the coordinates above, so
    // the request can commit to it without simulating the CPI Custody will run.
    let projection_receipt_digest = project_found_receipt_digest_v1(
        market_bytes,
        generation,
        records,
        product,
        mint,
        token_program,
        release_set,
        rent_program,
        principal_cap_sets,
    )?;

    let lock = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
        caller_role: ProjectedCallerRoleV1::TradingCapability,
        market: market_bytes,
        generation,
        realm: records.realm.digest,
        product_record: records.product.digest,
        product: product.to_bytes(),
        source: records.source.digest,
        release_set,
        projection_receipt_digest,
        parent_capability_root: found.capability_root().to_bytes(),
        context_digest,
        caller_program: trading.to_bytes(),
        payer: payer.to_bytes(),
        core_program: core.to_bytes(),
        rent_program: rent_program.to_bytes(),
        // Not the payer. `OpenSourceCompartment` requires the principal's owner
        // to sign while remaining non-writable and the creation payer to be
        // writable, and Solana grants privileges per key; the same split is
        // what `credit.refund_wallet() == found.beneficiary()` and
        // `lock.refund_owner == found.beneficiary()` already required.
        refund_owner: beneficiary.to_bytes(),
        rent_credit: credit.to_bytes(),
        hoard_vault: hoard_vault.to_bytes(),
        funding_source_vault: source_vault.to_bytes(),
        funding_source_context: context,
        funding_source_compartment: CompartmentV1::Settlement,
        mint: mint.to_bytes(),
        token_program: token_program.to_bytes(),
        collateral_release: collateral_adapter_release_id(),
        expiry_slot,
        expected_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1,
        resulting_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1
            .checked_add(1)
            .ok_or_else(|| Error::new("terminal revision overflow"))?,
        amount: found
            .hoard_principal()
            .map_err(|error| Error::new(format!("Hoard principal: {error:?}")))?,
        state_rent_lamports: rpc.minimum_balance(PROJECTED_CUSTODY_STATE_BYTES_V2)?,
        vault_rent_lamports: rpc.minimum_balance(ACCOUNT_BYTES)?,
        funding_source_replay_revision: SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
        funding_source_state_rent_lamports: rpc.minimum_balance(CUSTODY_REPLAY_BYTES_V1)?,
        funding_source_vault_rent_lamports: rpc.minimum_balance(ACCOUNT_BYTES)?,
    };
    // Refuse here rather than inside Custody: the ladder is only derivable
    // from a terminal request the codec already accepts.
    let _ = lock
        .encode()
        .map_err(|error| Error::new(format!("terminal Lock request: {error:?}")))?;
    if lock.resulting_revision.checked_add(1) != Some(found.projected_resulting_revision()) {
        return Err(Error::new(
            "founding artifact and terminal Lock disagree about the Realize revision",
        ));
    }
    let controller_funding_checkpoint = Pubkey::find_program_address(
        &ControllerFundingCheckpointDerivationV1::new(
            release_set,
            market_bytes,
            generation,
            records.manifest.digest,
            funding_list_id.to_bytes(),
        )
        .map_err(|error| Error::new(format!("controller funding checkpoint: {error:?}")))?
        .seed_components(),
        &trading,
    )
    .0;

    Ok(FoundingCoordinates {
        generation,
        identity,
        market,
        credit,
        hoard_vault,
        source_vault,
        source_replay,
        projected_replay,
        custody_authority,
        controller_funding_checkpoint,
        context,
        principal_cap_sets,
        capability_entry_count: manifest.entry_count(),
        capability_entry_index,
        funding_ledgers,
        found,
        lock,
    })
}

/// Exact projected-Custody terminal revision for the four-stage ladder.
///
/// Initialize reaches one, OpenHoard two, OpenSourceCompartment three, the
/// terminal Lock four, and Core's Realize stage five. Core refuses any artifact
/// whose declared revision is not exactly the Realize poststate.
const GENERIC_FOUNDING_PROJECTED_RESULTING_REVISION_V1: u64 = 5;

/// Digest of the `ProjectFoundReceiptV2` Core will return during Initialize.
///
/// Derived, not simulated: every field of the receipt is a coordinate this
/// campaign already established, and none of it is a slot, a bump, a balance,
/// or anything else that only execution could supply.
#[allow(clippy::too_many_arguments)]
fn project_found_receipt_digest_v1(
    market: [u8; 32],
    generation: u64,
    records: &MarketRecords,
    product: ProductContentId,
    mint: Pubkey,
    token_program: Pubkey,
    release_set: [u8; 32],
    rent_program: Pubkey,
    principal_cap_sets: u64,
) -> Result<[u8; 32]> {
    let found_request = Request::administrative(Action::Found, generation, identity_of(market)?);
    let found_bytes = found_request
        .encode()
        .map_err(|error| Error::new(format!("canonical Core Found request: {error:?}")))?;
    let receipt = ProjectFoundReceiptV2::new(
        identity_of(market)?,
        generation,
        identity_of(records.realm.digest)?,
        identity_of(mint.to_bytes())?,
        identity_of(token_program.to_bytes())?,
        identity_of(collateral_adapter_release_id())?,
        identity_of(records.product.digest)?,
        identity_of(product.to_bytes())?,
        identity_of(records.source.digest)?,
        identity_of(release_set)?,
        identity_of(rent_program.to_bytes())?,
        principal_cap_sets,
        Sha256::digest(found_bytes).into(),
    )
    .map_err(|error| Error::new(format!("ProjectFoundReceiptV2: {error:?}")))?;
    let bytes = receipt
        .encode()
        .map_err(|error| Error::new(format!("ProjectFound receipt encoding: {error:?}")))?;
    Ok(Sha256::digest(bytes).into())
}

/// The Realm-selected collateral adapter release this campaign publishes.
fn collateral_adapter_release_id() -> [u8; 32] {
    Sha256::digest(
        CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer().to_bytes(),
    )
    .into()
}

fn identity_of(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|error| Error::new(format!("identity: {error:?}")))
}

/// Build the separate exact controller-funding preparation frame.
#[allow(clippy::too_many_arguments)]
fn build_controller_funding_prepare_v1(
    plan: &SuccessorPlan,
    coordinates: &FoundingCoordinates,
    records: &MarketRecords,
    found_raw: Pubkey,
    lock_raw: Pubkey,
    funding_source: Pubkey,
    projection_witness: Pubkey,
) -> Result<Instruction> {
    let trading = pubkey(&plan.trading.program_id)?;
    let trading_programdata = pubkey(&plan.trading.programdata_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let resolution_programdata = pubkey(&plan.resolution.programdata_id)?;
    let resolution_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == resolution)
        .ok_or_else(|| Error::new("controller-funding prepare omitted Resolution ledger"))?;
    let trading_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == trading)
        .ok_or_else(|| Error::new("controller-funding prepare omitted Trading ledger"))?;
    let project_found = ProjectFoundRequestV2::new(Request::administrative(
        Action::Found,
        coordinates.generation,
        identity_of(coordinates.market.to_bytes())?,
    ))
    .map_err(|error| Error::new(format!("controller-funding ProjectFound: {error:?}")))?;
    let request = PreMarketFundingRequestV2 {
        project_found,
        manifest: records.manifest.digest,
        selected_mask: resolution_ledger.selected_mask,
        funding_source: funding_source.to_bytes(),
        ledger: resolution_ledger.address.to_bytes(),
        prestate_digest: pre_market_funding_prestate_digest_v1(
            resolution_ledger.address.to_bytes(),
            system_program::ID.to_bytes(),
            0,
            0,
        ),
        expected_project_found_receipt_digest: coordinates.lock.projection_receipt_digest,
    }
    .encode()
    .map_err(|error| Error::new(format!("controller-funding Resolution request: {error:?}")))?;
    let caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            hex32(&plan.release_set_id)?,
            coordinates.market.to_bytes(),
            ExecutionRoleV1::Trading,
            records.manifest.digest,
            Sha256::digest(request).into(),
        )
        .map_err(|error| Error::new(format!("controller-funding authority: {error:?}")))?
        .as_slices(),
        &trading,
    )
    .0;

    let mut accounts = vec![
        AccountMeta::new_readonly(found_raw, false),
        AccountMeta::new_readonly(lock_raw, false),
        AccountMeta::new_readonly(sysvar::instructions::ID, false),
        AccountMeta::new_readonly(resolution, false),
        AccountMeta::new_readonly(resolution_programdata, false),
        AccountMeta::new_readonly(caller_authority, false),
        AccountMeta::new_readonly(trading, false),
        AccountMeta::new_readonly(trading_programdata, false),
        AccountMeta::new(resolution_ledger.address, false),
        AccountMeta::new(trading_ledger.address, false),
        AccountMeta::new(coordinates.controller_funding_checkpoint, false),
        AccountMeta::new(funding_source, true),
    ];
    for (index, key) in ordinary_project_found_snapshot_keys_v2(
        plan,
        projection_witness,
        coordinates.market,
        coordinates.credit,
        records,
    )?
    .into_iter()
    .enumerate()
    {
        accounts.push(match index {
            0 => AccountMeta::new(key, true),
            2 => AccountMeta::new(key, false),
            _ => AccountMeta::new_readonly(key, false),
        });
    }
    authenticate_controller_funding_prepare_frame_v1(
        &accounts,
        funding_source,
        projection_witness,
    )?;
    Ok(Instruction {
        program_id: trading,
        accounts,
        data: CONTROLLER_FUNDING_PREPARE_MAGIC_V1.to_vec(),
    })
}

fn authenticate_controller_funding_prepare_frame_v1(
    accounts: &[AccountMeta],
    funding_source: Pubkey,
    projection_witness: Pubkey,
) -> Result<()> {
    let source = accounts
        .get(CONTROLLER_FUNDING_PREPARE_FUNDING_SOURCE_V1)
        .ok_or_else(|| Error::new("DCLTCFQ1 omitted its distinct funding source"))?;
    let found_payer = accounts
        .get(CONTROLLER_FUNDING_PREPARE_FOUND_START_V1)
        .ok_or_else(|| Error::new("DCLTCFQ1 omitted its ProjectFound payer"))?;
    let rent_credit = accounts
        .get(CONTROLLER_FUNDING_PREPARE_FOUND_RENT_CREDIT_V1)
        .ok_or_else(|| Error::new("DCLTCFQ1 omitted its ProjectFound RentCredit"))?;
    if accounts.len() != CONTROLLER_FUNDING_PREPARE_ACCOUNTS_V1
        || source.pubkey != funding_source
        || !source.is_signer
        || !source.is_writable
        || found_payer.pubkey != projection_witness
        || !found_payer.is_signer
        || !found_payer.is_writable
        || source.pubkey == found_payer.pubkey
        || rent_credit.is_signer
        || !rent_credit.is_writable
    {
        return Err(Error::new(
            "assembled DCLTCFQ1 frame changed its source, ProjectFound payer, RentCredit, or 48-account geometry",
        ));
    }
    Ok(())
}

fn authenticate_controller_funding_checkpoint_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: &FoundingCoordinates,
    funding_source: Pubkey,
    expected_phase: ControllerFundingCheckpointPhaseV1,
) -> Result<ControllerFundingCheckpointV1> {
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let checkpoint_account = rpc.required_account(
        coordinates.controller_funding_checkpoint,
        "controller funding checkpoint",
    )?;
    if checkpoint_account.owner != trading
        || checkpoint_account.data.len() != CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1
        || checkpoint_account.lamports
            != rpc.minimum_balance(CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1)?
    {
        return Err(Error::new(
            "controller funding checkpoint owner, width, or Rent changed",
        ));
    }
    let checkpoint = ControllerFundingCheckpointV1::decode(&checkpoint_account.data)
        .map_err(|error| Error::new(format!("controller funding checkpoint: {error:?}")))?;
    let input = checkpoint.input_ref();
    let resolution_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == resolution)
        .ok_or_else(|| Error::new("checkpoint verifier omitted Resolution ledger"))?;
    let trading_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == trading)
        .ok_or_else(|| Error::new("checkpoint verifier omitted Trading ledger"))?;
    for ledger in [resolution_ledger, trading_ledger] {
        let account = rpc.required_account(ledger.address, "controller funding ledger")?;
        if account.owner != ledger.controller
            || account.lamports != ledger.required_lamports
            || account.data != ledger.bytes
        {
            return Err(Error::new(
                "controller funding ledger differs from its exact initial Pending poststate",
            ));
        }
    }
    let expected_ladder_digest = if expected_phase == ControllerFundingCheckpointPhaseV1::Prepared {
        [0; 32]
    } else {
        controller_funding_custody_ladder_digest_v1(rpc, coordinates)?
    };
    let resolution_ledger_digest: [u8; 32] = Sha256::digest(&resolution_ledger.bytes).into();
    let trading_ledger_digest: [u8; 32] = Sha256::digest(&trading_ledger.bytes).into();
    if checkpoint.phase() != expected_phase
        || input.release_set != hex32(&plan.release_set_id)?
        || input.market != coordinates.market.to_bytes()
        || input.generation != coordinates.generation
        || input.funding_list != coordinates.found.funding_list_id().to_bytes()
        || input.resolution_ledger != resolution_ledger.address.to_bytes()
        || input.resolution_ledger_digest != resolution_ledger_digest
        || input.trading_ledger != trading_ledger.address.to_bytes()
        || input.trading_ledger_digest != trading_ledger_digest
        || input.funding_source != funding_source.to_bytes()
        || input.rent_credit != coordinates.credit.to_bytes()
        || input.project_found_receipt_digest != coordinates.lock.projection_receipt_digest
        || input.expiry_slot != coordinates.lock.expiry_slot
        || checkpoint.custody_ladder_digest() != expected_ladder_digest
    {
        return Err(Error::new(
            "controller funding checkpoint changed identity, ledger, refund, or phase facts",
        ));
    }
    Ok(checkpoint)
}

fn controller_funding_custody_ladder_digest_v1(
    rpc: &mut Rpc,
    coordinates: &FoundingCoordinates,
) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    hasher.update(CONTROLLER_FUNDING_CUSTODY_LADDER_DIGEST_DOMAIN_V1);
    for key in [
        coordinates.projected_replay,
        coordinates.hoard_vault,
        coordinates.source_vault,
        coordinates.source_replay,
    ] {
        let account = rpc.required_account(key, "controller funding Custody ladder")?;
        hasher.update(key.as_ref());
        hasher.update(account.owner.as_ref());
        hasher.update(account.lamports.to_le_bytes());
        hasher.update((account.data.len() as u64).to_le_bytes());
        hasher.update(&account.data);
    }
    Ok(hasher.finalize().into())
}

fn authenticate_controller_funding_cleanup_checkpoint_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: &FoundingCoordinates,
    funding_source: Pubkey,
    expected_phase: ControllerFundingCheckpointPhaseV1,
) -> Result<ControllerFundingCheckpointV1> {
    if !matches!(
        expected_phase,
        ControllerFundingCheckpointPhaseV1::CustodyAborted
            | ControllerFundingCheckpointPhaseV1::PreparedFirstLedgerClosed
            | ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed
    ) {
        return Err(Error::new(
            "cleanup checkpoint verifier received a non-cleanup phase",
        ));
    }
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let checkpoint_account = rpc.required_account(
        coordinates.controller_funding_checkpoint,
        "controller funding cleanup checkpoint",
    )?;
    if checkpoint_account.owner != trading
        || checkpoint_account.data.len() != CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1
        || checkpoint_account.lamports
            != rpc.minimum_balance(CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1)?
    {
        return Err(Error::new(
            "controller funding cleanup checkpoint owner, width, or Rent changed",
        ));
    }
    let checkpoint = ControllerFundingCheckpointV1::decode(&checkpoint_account.data)
        .map_err(|error| Error::new(format!("controller funding cleanup checkpoint: {error:?}")))?;
    let input = checkpoint.input_ref();
    if checkpoint.phase() != expected_phase
        || checkpoint.revision() != expected_phase as u64
        || input.release_set != hex32(&plan.release_set_id)?
        || input.market != coordinates.market.to_bytes()
        || input.generation != coordinates.generation
        || input.funding_list != coordinates.found.funding_list_id().to_bytes()
        || input.funding_source != funding_source.to_bytes()
        || input.rent_credit != coordinates.credit.to_bytes()
        || input.project_found_receipt_digest != coordinates.lock.projection_receipt_digest
        || input.expiry_slot != coordinates.lock.expiry_slot
    {
        return Err(Error::new(
            "controller funding cleanup checkpoint changed identity, refund, or phase facts",
        ));
    }
    let resolution_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == resolution)
        .ok_or_else(|| Error::new("cleanup checkpoint omitted Resolution ledger"))?;
    let trading_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == trading)
        .ok_or_else(|| Error::new("cleanup checkpoint omitted Trading ledger"))?;
    for (controller, ledger) in [
        (ControllerFundingControllerV1::Resolution, resolution_ledger),
        (ControllerFundingControllerV1::Trading, trading_ledger),
    ] {
        let should_be_closed = matches!(
            expected_phase,
            ControllerFundingCheckpointPhaseV1::PreparedFirstLedgerClosed
                | ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed
        ) && checkpoint.canonical_first_controller() == controller;
        let observed = rpc.account(ledger.address)?;
        if should_be_closed {
            if observed.is_some_and(|account| account.lamports != 0 || !account.data.is_empty()) {
                return Err(Error::new(
                    "cleanup checkpoint left the canonical first ledger live",
                ));
            }
        } else if observed.is_none_or(|account| {
            account.owner != ledger.controller
                || account.lamports != ledger.required_lamports
                || account.data != ledger.bytes
        }) {
            return Err(Error::new(
                "cleanup checkpoint remaining ledger differs from exact Pending prestate",
            ));
        }
    }
    let cleanup = checkpoint
        .cleanup()
        .ok_or_else(|| Error::new("cleanup checkpoint omitted persisted cleanup evidence"))?;
    if cleanup.transition_slot() <= input.expiry_slot
        || cleanup.prior_checkpoint_digest() == [0; 32]
        || (matches!(
            expected_phase,
            ControllerFundingCheckpointPhaseV1::PreparedFirstLedgerClosed
                | ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed
        ) && (cleanup.first_controller() != Some(checkpoint.canonical_first_controller())
            || cleanup.first_mask()
                != checkpoint.controller_mask(checkpoint.canonical_first_controller())
            || cleanup.first_ledger_prestate_digest() == [0; 32]
            || cleanup.first_ledger_closed_digest() == [0; 32]
            || cleanup.first_close_receipt_digest() == [0; 32]
            || cleanup.remaining_ledger_prestate_digest() == [0; 32]))
    {
        return Err(Error::new(
            "controller funding cleanup checkpoint omitted exact transition evidence",
        ));
    }
    Ok(checkpoint)
}

/// Build the exact 88-account `DCLTPCB2` frame.
///
/// Privileges are the outer's own assertion, not a guess: the route refuses a
/// frame that under-privileges an account rather than failing opaquely inside
/// Custody. Exactly the projected state, the two vaults, the source replay, the
/// funding tail, and the Custody rent payer are writable; exactly the payer and
/// the principal `refund_owner` sign, and the three caller PDAs sign only
/// inside the CPI, under `invoke_signed`.
#[allow(clippy::too_many_arguments)]
fn build_projected_custody_bootstrap_v2(
    plan: &SuccessorPlan,
    coordinates: &FoundingCoordinates,
    records: &MarketRecords,
    found_raw: Pubkey,
    lock_raw: Pubkey,
    projection_witness: Pubkey,
    payer: Pubkey,
    beneficiary: Pubkey,
    funder: Pubkey,
    mint: Pubkey,
) -> Result<Instruction> {
    let trading = pubkey(&plan.trading.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let cache = pubkey(&plan.activation)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let trading_programdata = pubkey(&plan.trading.programdata_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    let stages = [
        (
            FoundingPrestateStageV1::Initialize,
            ProjectedCustodyOperationV1::Initialize,
        ),
        (
            FoundingPrestateStageV1::OpenHoard,
            ProjectedCustodyOperationV1::OpenHoard,
        ),
        (
            FoundingPrestateStageV1::OpenSourceCompartment,
            ProjectedCustodyOperationV1::OpenSourceCompartment,
        ),
    ];
    let mut callers = Vec::with_capacity(stages.len());
    for (stage, operation) in stages {
        let request = coordinates
            .lock
            .founding_prestate_stage_v1(stage)
            .map_err(|error| Error::new(format!("founding prestate {operation:?}: {error:?}")))?;
        let raw = request
            .encode()
            .map_err(|error| Error::new(format!("prestate encoding {operation:?}: {error:?}")))?;
        let digest: [u8; 32] = Sha256::digest(raw).into();
        let seeds = ProjectedCustodyCallerSeedsV1::new(request, digest);
        callers.push(Pubkey::find_program_address(&seeds.as_slices(), &trading).0);
    }

    let mut accounts = Vec::with_capacity(PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNTS_V2);
    accounts.push(AccountMeta::new_readonly(found_raw, false));
    accounts.push(AccountMeta::new_readonly(lock_raw, false));
    accounts.push(AccountMeta::new_readonly(sysvar::instructions::ID, false));
    accounts.push(AccountMeta::new_readonly(custody, false));

    // Initialize: the seven shared projected coordinates, four Initialize
    // coordinates, then Core's exact ProjectFound36 sub-frame verbatim.
    let common = |caller: Pubkey| {
        vec![
            AccountMeta::new_readonly(caller, false),
            AccountMeta::new(coordinates.projected_replay, false),
            AccountMeta::new_readonly(cache, false),
            AccountMeta::new_readonly(registry, false),
            AccountMeta::new_readonly(trading, false),
            AccountMeta::new_readonly(trading_programdata, false),
            AccountMeta::new_readonly(coordinates.credit, false),
        ]
    };
    accounts.extend(common(
        *callers
            .first()
            .ok_or_else(|| Error::new("Initialize caller authority missing"))?,
    ));
    accounts.push(AccountMeta::new_readonly(core, false));
    accounts.push(AccountMeta::new(payer, true));
    accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    for key in ordinary_project_found_snapshot_keys_v2(
        plan,
        projection_witness,
        coordinates.market,
        coordinates.credit,
        records,
    )? {
        accounts.push(AccountMeta::new_readonly(key, false));
    }

    // OpenHoard.
    accounts.extend(common(
        *callers
            .get(1)
            .ok_or_else(|| Error::new("OpenHoard caller authority missing"))?,
    ));
    accounts.push(AccountMeta::new(coordinates.hoard_vault, false));
    accounts.push(AccountMeta::new_readonly(
        coordinates.custody_authority,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(mint, false));
    accounts.push(AccountMeta::new_readonly(token_program, false));
    accounts.push(AccountMeta::new(payer, true));
    accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    accounts.push(AccountMeta::new_readonly(coordinates.market, false));

    // OpenSourceCompartment.
    accounts.extend(common(*callers.get(2).ok_or_else(|| {
        Error::new("OpenSourceCompartment caller authority missing")
    })?));
    accounts.push(AccountMeta::new(coordinates.source_vault, false));
    accounts.push(AccountMeta::new(coordinates.source_replay, false));
    accounts.push(AccountMeta::new_readonly(
        coordinates.custody_authority,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(mint, false));
    accounts.push(AccountMeta::new_readonly(token_program, false));
    accounts.push(AccountMeta::new(funder, false));
    accounts.push(AccountMeta::new_readonly(beneficiary, true));
    accounts.push(AccountMeta::new(payer, true));
    accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    accounts.push(AccountMeta::new_readonly(coordinates.market, false));

    if accounts.len() != PROJECTED_CUSTODY_BOOTSTRAP_COMMON_ACCOUNTS_V2 {
        return Err(Error::new(
            "assembled DCLTPCB2 common frame did not have 84 accounts",
        ));
    }
    let resolution = pubkey(&plan.resolution.program_id)?;
    let resolution_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == resolution)
        .ok_or_else(|| Error::new("DCLTPCB2 omitted the Resolution subset ledger"))?;
    let trading_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == trading)
        .ok_or_else(|| Error::new("DCLTPCB2 omitted the Trading subset ledger"))?;
    let trading_mask = 1_u16
        .checked_shl(u32::from(coordinates.capability_entry_index))
        .ok_or_else(|| Error::new("Direct capability entry mask overflow"))?;
    let resolution_mask =
        manifest_required_union_v1(coordinates.capability_entry_count)? ^ trading_mask;
    if coordinates.funding_ledgers.len() != 2
        || resolution_ledger.selected_mask != resolution_mask
        || trading_ledger.selected_mask != trading_mask
    {
        return Err(Error::new(
            "Direct DCLTPCB2 requires the selected Direct bit and its exact Resolution complement",
        ));
    }
    accounts.push(AccountMeta::new(
        coordinates.controller_funding_checkpoint,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(resolution_ledger.address, false));
    accounts.push(AccountMeta::new_readonly(trading_ledger.address, false));
    if accounts.len() != PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNTS_V2
        || accounts[PROJECTED_CUSTODY_BOOTSTRAP_CHECKPOINT_V2].pubkey
            != coordinates.controller_funding_checkpoint
        || accounts[PROJECTED_CUSTODY_BOOTSTRAP_RESOLUTION_LEDGER_V2].pubkey
            != resolution_ledger.address
        || accounts[PROJECTED_CUSTODY_BOOTSTRAP_TRADING_LEDGER_V2].pubkey != trading_ledger.address
    {
        return Err(Error::new(
            "assembled DCLTPCB2 frame did not match its exact ABI",
        ));
    }
    Ok(Instruction {
        program_id: trading,
        accounts,
        data: PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V2.to_vec(),
    })
}

/// Schema namespacing this campaign's readonly raw-request records.
///
/// `DCLTPCB2` and `DCLTGMF3` carry no economic payload: every economic byte travels in
/// readonly data accounts. Publishing them as ordinary content-addressed
/// Registry records means their addresses are a function of their bytes, so a
/// substituted request is a substituted address and the outer's own frame
/// checks catch it.
fn raw_request_schema_v1(role: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/local-campaign/raw-request-schema/v1");
    hasher.update([0]);
    hasher.update(role.as_bytes());
    hasher.finalize().into()
}

/// Derive a raw-request Record's complete canonical coordinate without writing it.
///
/// DCLTGMF3 uses this before any pre-funding or publication so the final
/// compiled-message lock census is an admission guard, not post-spend evidence.
fn derive_raw_request_record_v1(
    registry: Pubkey,
    role: &str,
    content: &[u8],
) -> Result<PublishedRecord> {
    let schema = raw_request_schema_v1(role);
    let (raw, staging, digest) = derive_record_addresses_v1(
        registry,
        RecordPublicationContentV1 {
            schema_release_id: schema,
            content,
        },
    )
    .map_err(|error| Error::new(format!("derive {role} record address: {error:?}")))?;
    Ok(PublishedRecord {
        schema,
        digest,
        raw,
        staging,
    })
}

fn require_published_record_matches_derivation_v1(
    role: &str,
    expected: PublishedRecord,
    published: PublishedRecord,
) -> Result<()> {
    if published != expected {
        return Err(Error::new(format!(
            "published {role} record coordinate/digest differed from its pre-mutation derivation"
        )));
    }
    Ok(())
}

/// Expiry geometry for the one SourceAbort proof lane.
///
/// Public/devnet keeps the shipped 900/64 policy byte-for-byte. The shorter
/// policy is selected only by the exact 100,000,000-atom owned-loopback fixture
/// marker that campaign admission already refuses on devnet. It budgets the
/// observed 32-slot local finality window across all sixteen finalized
/// transaction barriers before the pre-expiry refusal, plus two full windows:
/// one staging reserve and one refusal-dispatch reserve. Runtime guards refuse
/// safely if a loaded validator consumes either reserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceAbortExpiryPolicyV1 {
    PublicDevnet,
    OwnedLoopback,
}

impl SourceAbortExpiryPolicyV1 {
    const LOCAL_FINALITY_WINDOW_SLOTS_V1: u64 = 32;
    const LOCAL_PRE_EXPIRY_FINALIZED_BARRIERS_V1: u64 = 16;
    const LOCAL_RESERVE_WINDOWS_V1: u64 = 2;
    const LOCAL_POST_STAGE_FINALIZED_BARRIERS_V1: u64 = 4;

    fn from_input(input: &MarketRunInput) -> Result<Self> {
        match input.local_participant_fixture_liquidity_atoms {
            0 => Ok(Self::PublicDevnet),
            LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1 => Ok(Self::OwnedLoopback),
            _ => Err(Error::new(
                "SourceAbort expiry policy received a noncanonical local fixture quantity",
            )),
        }
    }

    const fn expiry_slots(self) -> u64 {
        match self {
            Self::PublicDevnet => 900,
            Self::OwnedLoopback => {
                Self::LOCAL_FINALITY_WINDOW_SLOTS_V1
                    * (Self::LOCAL_PRE_EXPIRY_FINALIZED_BARRIERS_V1
                        + Self::LOCAL_RESERVE_WINDOWS_V1)
            }
        }
    }

    const fn minimum_staging_margin_slots(self) -> u64 {
        match self {
            Self::PublicDevnet => 64,
            Self::OwnedLoopback => {
                Self::LOCAL_FINALITY_WINDOW_SLOTS_V1
                    * (Self::LOCAL_POST_STAGE_FINALIZED_BARRIERS_V1 + 1)
            }
        }
    }

    const fn minimum_pre_expiry_refusal_margin_slots(self) -> u64 {
        match self {
            Self::PublicDevnet => 0,
            Self::OwnedLoopback => Self::LOCAL_FINALITY_WINDOW_SLOTS_V1 * 2,
        }
    }
}

fn require_expiry_margin_v1(
    stage: &str,
    current_slot: u64,
    expiry_slot: u64,
    minimum_margin_slots: u64,
) -> Result<()> {
    let guarded_slot = current_slot
        .checked_add(minimum_margin_slots)
        .ok_or_else(|| Error::new(format!("{stage} expiry-margin arithmetic overflowed")))?;
    if guarded_slot >= expiry_slot {
        return Err(Error::new(format!(
            "{stage} reached slot {current_slot} with expiry at {expiry_slot}: fewer than the required {minimum_margin_slots} margin slots remain"
        )));
    }
    Ok(())
}

/// What a staged projected-Custody prestate is being staged *for*.
///
/// Both lanes run the identical four-stage `DCLTPCB2` ladder. They differ in
/// the generation they occupy, how long their founding stays satisfiable, and
/// which of the two exits out of `SourceFunded` they then take.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrestateLaneV1 {
    /// Stage the prestate and found the Market atomically through `DCLTGMF3`.
    Founding,
    /// Stage the prestate, let it expire, and unwind it through `DCLTPCA1`.
    ///
    /// This lane exists because the abort is a SAFETY route, and a safety route
    /// with no execution evidence is the one you least want to discover is
    /// broken at the moment somebody needs it.
    SourceAbort,
}

/// The public/private success receipt has one and only one projected-Custody
/// lane. Expiry cleanup is driven by its separately named validation command
/// and therefore never appears as an incidental suffix here.
const SUCCESS_PRESTATE_LANES_V1: [PrestateLaneV1; 1] = [PrestateLaneV1::Founding];

impl PrestateLaneV1 {
    /// The generation this lane's Market occupies.
    ///
    /// Every projected-Custody stage asserts the inverse of a live Market and
    /// Core's projection requires the Market vacant, so each lane needs its own
    /// still-vacant Market PDA - and therefore its own generation, distinct
    /// from Found37's and from the other lane's.
    fn generation(self, input: &MarketRunInput) -> Result<u64> {
        let offset = match self {
            Self::Founding => 1,
            Self::SourceAbort => 2,
        };
        input
            .generation
            .checked_add(offset)
            .ok_or_else(|| Error::new("founding generation overflow"))
    }

    /// How long this lane's founding stays satisfiable, in slots.
    ///
    /// The founding lane wants an expiry it will never reach. The abort lane
    /// wants the opposite and is squeezed from both sides: `initialize` refuses
    /// once `current_slot > expiry_slot`, so the expiry must outlast staging;
    /// and every slot past that is dead waiting. Staging costs about thirteen
    /// transactions at roughly thirty-two slots of finality each, so ~420
    /// slots; public/devnet retains 900. Owned loopback uses the separately
    /// authenticated fixture policy above so repeated private validation does
    /// not spend hundreds of dead slots after the rollback proof. The runner
    /// does not assume the arithmetic held: it checks both dispatch margins and
    /// waits for the real slot after.
    fn expiry_slots(self, input: &MarketRunInput) -> Result<u64> {
        match self {
            Self::Founding => Ok(500_000),
            Self::SourceAbort => Ok(SourceAbortExpiryPolicyV1::from_input(input)?.expiry_slots()),
        }
    }

    /// Slots that must remain before expiry for staging to be worth attempting.
    fn minimum_staging_margin_slots(self, input: &MarketRunInput) -> Result<u64> {
        match self {
            Self::Founding => Ok(0),
            Self::SourceAbort => {
                Ok(SourceAbortExpiryPolicyV1::from_input(input)?.minimum_staging_margin_slots())
            }
        }
    }

    /// Prefix for this lane's account-evidence keys.
    ///
    /// Both lanes stage the same shaped accounts at different generations, so
    /// they must not overwrite one another's evidence. The abort lane's are
    /// additionally CLOSED by the end of the run, and evidence that silently
    /// swapped one lane's live account for the other's closed one would be
    /// worse than no evidence.
    const fn evidence_prefix(self) -> &'static str {
        match self {
            Self::Founding => "founding",
            Self::SourceAbort => "abort",
        }
    }

    /// The label its honest `DCLTPCB2` transaction carries.
    const fn prestate_label(self) -> &'static str {
        match self {
            Self::Founding => {
                "stage projected custody against prepared controller funding (DCLTPCB2)"
            }
            Self::SourceAbort => {
                "stage projected custody against prepared controller funding for the expiry abort (DCLTPCB2)"
            }
        }
    }
}

/// Create the projected-Custody founding prestate on a real validator.
///
/// This is `DCLTPCB2`: Custody and controller funding in one rollback domain against a Market
/// that does not exist yet. It leaves the projected replay at `SourceFunded`,
/// an empty Hoard vault, a funded source compartment, and one prepaid
/// `FundingStateV1` per capability-manifest entry - exactly the prestate the
/// founding outer's Lock stage consumes and nothing else.
#[allow(clippy::too_many_arguments)]
fn execute_projected_custody_bootstrap(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    records: &MarketRecords,
    identity_template: MarketIdentity,
    product: ProductContentId,
    mint: Pubkey,
    collateral_wallet: Pubkey,
    found31_market: Pubkey,
    lane: PrestateLaneV1,
    prepared_checkpoint: Option<&MarketExecutionCheckpointV1>,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    transactions: &mut Vec<TransactionEvidence>,
    accounts: &mut BTreeMap<String, AccountEvidence>,
    completed: &mut Vec<String>,
    local_participant_fixture_liquidity: Option<&LocalParticipantFixtureLiquidityEvidenceV1>,
    checkpoint: &mut dyn FnMut(&MarketExecutionCheckpointV1) -> Result<()>,
    mut submission_recorder: Option<&mut FoundingSubmissionRecorderV1<'_>>,
) -> Result<[u8; 32]> {
    let registry = pubkey(&plan.registry.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    // The principal supplier is not the rent payer. Custody requires the
    // funding source's owner to sign and to be non-writable, while the rent
    // payer must be writable, and a transaction grants privileges per key.
    let beneficiary = forge.keypair(role::FOUNDING_BENEFICIARY);
    let projection_witness = forge.keypair(role::FOUNDING_PROJECTION_WITNESS);
    let market_rent = rpc.minimum_balance(STATE_BYTES)?;
    let resume_prepared = prepared_checkpoint.is_some();
    if resume_prepared && lane != PrestateLaneV1::Founding {
        return Err(Error::new(
            "only the founding lane admits a DCLTCFQ1 Prepared checkpoint",
        ));
    }
    let expiry_slot = match prepared_checkpoint {
        Some(checkpoint) => checkpoint.expiry_slot,
        None => rpc
            .finalized_slot()?
            .checked_add(lane.expiry_slots(input)?)
            .ok_or_else(|| Error::new("founding expiry slot overflow"))?,
    };

    let coordinates = derive_founding_coordinates(
        rpc,
        plan,
        input,
        records,
        identity_template,
        product,
        mint,
        payer.pubkey(),
        actors.founder,
        beneficiary.pubkey(),
        lane.generation(input)?,
        expiry_slot,
    )?;
    let principal = coordinates.lock.amount;
    let projection_witness_lamports = market_rent;

    // One Token-2022 account owned by the party the principal is refundable to.
    let source_funder = forge.keypair(role::FOUNDING_SOURCE_FUNDER);
    if !resume_prepared {
        let wallet_rent = rpc.minimum_balance(ACCOUNT_BYTES)?;
        let mut initialize_wallet = Vec::with_capacity(33);
        initialize_wallet.push(18);
        initialize_wallet.extend_from_slice(beneficiary.pubkey().as_ref());
        let mut transfer_checked = Vec::with_capacity(10);
        transfer_checked.push(12);
        transfer_checked.extend_from_slice(&principal.to_le_bytes());
        transfer_checked.push(input.collateral_display_decimals);
        transactions.push(rpc.send_with_signers(
            "fund the founding principal supplier and its rent-capacity witness",
            &[
                transfer(
                    &payer.pubkey(),
                    &projection_witness.pubkey(),
                    projection_witness_lamports,
                ),
                create_account(
                    &payer.pubkey(),
                    &source_funder.pubkey(),
                    wallet_rent,
                    ACCOUNT_BYTES as u64,
                    &token_program,
                ),
                Instruction {
                    program_id: token_program,
                    accounts: vec![
                        AccountMeta::new(source_funder.pubkey(), false),
                        AccountMeta::new_readonly(mint, false),
                    ],
                    data: initialize_wallet,
                },
                Instruction {
                    program_id: token_program,
                    accounts: vec![
                        AccountMeta::new(collateral_wallet, false),
                        AccountMeta::new_readonly(mint, false),
                        AccountMeta::new(source_funder.pubkey(), false),
                        AccountMeta::new_readonly(payer.pubkey(), true),
                    ],
                    data: transfer_checked,
                },
            ],
            payer,
            &[&source_funder],
        )?);
    }

    // The founding generation's own lifecycle credit, refundable to the
    // beneficiary the artifact names.
    let claim_count = if resume_prepared {
        u32::try_from(input.cuts.len().saturating_add(2))
            .map_err(|_| Error::new("checkpoint Product outcome width overflow"))?
    } else {
        let keys = found_snapshot_keys(
            plan,
            projection_witness.pubkey(),
            coordinates.market,
            coordinates.credit,
            records,
        )?;
        let mut snapshot_keys = keys.clone();
        for extra in [payer.pubkey(), beneficiary.pubkey()] {
            if !snapshot_keys.contains(&extra) {
                snapshot_keys.push(extra);
            }
        }
        let minimum_slot = transactions
            .last()
            .map(|transaction| transaction.slot)
            .ok_or_else(|| Error::new("founding stage had no finalized predecessor"))?;
        let snapshot = finalized_snapshot(rpc, &snapshot_keys, minimum_slot)?;
        let projection_state = projection_state(
            rpc,
            plan,
            &snapshot,
            projection_witness.pubkey(),
            coordinates.market,
            records,
        )?;
        let projection = project_found_v2(coordinates.generation, projection_state)
            .map_err(|error| Error::new(format!("chain-derived founding projection: {error:?}")))?;
        if projection.market_address != coordinates.market {
            return Err(Error::new(
                "founding projection changed the derived Market address",
            ));
        }
        let claim_count = projection.outcome_count;
        if usize::try_from(claim_count) != Ok(input.cuts.len().saturating_add(2)) {
            return Err(Error::new(
                "the published Product's outcome width disagrees with the run spec's cut list",
            ));
        }
        let create = build_lifecycle_rent_create_v2(
            &projection,
            LifecycleRentCreateStateV2 {
                payer: snapshot.observation(payer.pubkey())?,
                credit_destination: snapshot.observation(coordinates.credit)?,
                refund_wallet: snapshot.observation(beneficiary.pubkey())?,
                rent_program: snapshot.observation(rent_program)?,
                system_program: snapshot.observation(system_program::ID)?,
                rent: snapshot.observation(sysvar::rent::ID)?,
            },
        )
        .map_err(|error| Error::new(format!("chain-derived founding RentV2 Create: {error:?}")))?;
        transactions.push(rpc.send(
            "create the founding generation's lifecycle RentCreditV2",
            std::slice::from_ref(&create.instruction),
            payer,
        )?);
        claim_count
    };

    // Publish the artifact and the terminal Lock request as content-addressed
    // readonly records, plus one non-terminal prestate the bootstrap must
    // refuse, so the substitution below is a real request rather than a fake.
    let found_raw_bytes = coordinates
        .found
        .encode()
        .map_err(|error| Error::new(format!("founding artifact encoding: {error:?}")))?;
    let lock_raw_bytes = coordinates
        .lock
        .encode()
        .map_err(|error| Error::new(format!("terminal Lock encoding: {error:?}")))?;
    let open_hoard_bytes = coordinates
        .lock
        .founding_prestate_stage_v1(FoundingPrestateStageV1::OpenHoard)
        .and_then(|request| request.encode())
        .map_err(|error| Error::new(format!("OpenHoard prestate encoding: {error:?}")))?;
    let found_record = if resume_prepared {
        derive_raw_request_record_v1(registry, "generic-founding-artifact", &found_raw_bytes)?
    } else {
        publish_record(
            rpc,
            registry,
            payer,
            raw_request_schema_v1("generic-founding-artifact"),
            &found_raw_bytes,
            None,
            transactions,
        )?
    };
    let lock_record = if resume_prepared {
        derive_raw_request_record_v1(registry, "projected-custody-terminal-lock", &lock_raw_bytes)?
    } else {
        publish_record(
            rpc,
            registry,
            payer,
            raw_request_schema_v1("projected-custody-terminal-lock"),
            &lock_raw_bytes,
            None,
            transactions,
        )?
    };
    if let Some(checkpoint) = prepared_checkpoint
        && (checkpoint.found_record != found_record.raw.to_string()
            || checkpoint.lock_record != lock_record.raw.to_string())
    {
        return Err(Error::new(
            "Prepared checkpoint found/Lock records changed during suffix reconstruction",
        ));
    }
    // Published only where it is used: it exists to be substituted into the
    // founding lane's hostile case, and publishing it costs three transactions.
    let open_hoard_record = if lane == PrestateLaneV1::Founding && !resume_prepared {
        Some(publish_record(
            rpc,
            registry,
            payer,
            raw_request_schema_v1("projected-custody-non-terminal"),
            &open_hoard_bytes,
            None,
            transactions,
        )?)
    } else {
        None
    };

    let controller_funding_prepare = build_controller_funding_prepare_v1(
        plan,
        &coordinates,
        records,
        found_record.raw,
        lock_record.raw,
        payer.pubkey(),
        projection_witness.pubkey(),
    )?;
    let controller_funding_prepare_geometry =
        projected_bootstrap_compiled_geometry_v2(payer.pubkey(), &controller_funding_prepare)?;
    let controller_funding_prepare_admitted = projected_bootstrap_compiled_geometry_v2(
        payer.pubkey(),
        &append_distinct_census_accounts_v1(&controller_funding_prepare, 15),
    )?;
    let controller_funding_prepare_refused = projected_bootstrap_compiled_geometry_v2(
        payer.pubkey(),
        &append_distinct_census_accounts_v1(&controller_funding_prepare, 16),
    )?;
    if controller_funding_prepare_geometry.complete_keys
        != CONTROLLER_FUNDING_PREPARE_COMPLETE_KEYS_V1
        || controller_funding_prepare_admitted.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1
        || controller_funding_prepare_refused.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1
    {
        return Err(Error::new(format!(
            "DCLTCFQ1 census refused: base {} keys, +15 {} keys, +16 {} keys; expected exactly {}, {}, {}",
            controller_funding_prepare_geometry.complete_keys,
            controller_funding_prepare_admitted.complete_keys,
            controller_funding_prepare_refused.complete_keys,
            CONTROLLER_FUNDING_PREPARE_COMPLETE_KEYS_V1,
            DEVNET_ACCOUNT_LOCK_LIMIT_V1,
            DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1,
        )));
    }
    let recovering_prepare = lane == PrestateLaneV1::Founding
        && submission_recorder.as_deref().is_some_and(|recorder| {
            recorder
                .current(FoundingSubmissionOperationV1::Dcltcfq1)
                .is_some()
        });
    if !recovering_prepare {
        for (label, key) in coordinates
            .funding_ledgers
            .iter()
            .map(|ledger| ("controller funding ledger", ledger.address))
            .chain(std::iter::once((
                "controller funding checkpoint",
                coordinates.controller_funding_checkpoint,
            )))
        {
            if rpc.account(key)?.is_some() {
                return Err(Error::new(format!(
                    "{label} existed before its DCLTCFQ1 preparation"
                )));
            }
        }
    }
    let prepare_route = if resume_prepared {
        None
    } else {
        Some(publish_routing_table(
            rpc,
            payer,
            "DCLTCFQ1",
            std::slice::from_ref(&controller_funding_prepare),
            transactions,
        )?)
    };
    let prepare_accounts = coordinates
        .funding_ledgers
        .iter()
        .map(|ledger| ledger.address)
        .chain(std::iter::once(coordinates.controller_funding_checkpoint))
        .chain(std::iter::once(payer.pubkey()))
        .chain(std::iter::once(projection_witness.pubkey()))
        .chain(std::iter::once(coordinates.credit))
        .collect::<Vec<_>>();
    let prepare_honest = if resume_prepared {
        None
    } else {
        let (prepare_routing, prepare_tables) = prepare_route
            .as_ref()
            .ok_or_else(|| Error::new("fresh DCLTCFQ1 omitted its routing table"))?;
        Some(match submission_recorder.as_deref_mut() {
            Some(recorder) if lane == PrestateLaneV1::Founding => {
                // The supplier transfer happened before this journal. Refresh the
                // one previously-captured mutable wallet and add the supplier's
                // exact Token-2022 poststate so Prepared recovery can prove that
                // no principal-funding prefix is replayed.
                for (label, key) in [
                    ("collateral_wallet", collateral_wallet),
                    ("founding_source_funder", source_funder.pubkey()),
                ] {
                    let account = rpc.required_account(key, label)?;
                    accounts.insert(label.into(), account_evidence(key, &account));
                }
                let root = Pubkey::new_from_array(coordinates.found.capability_root().to_bytes());
                let trading = pubkey(&plan.trading.program_id)?;
                let trading_ledger = coordinates
                    .funding_ledgers
                    .iter()
                    .find(|ledger| ledger.controller == trading)
                    .ok_or_else(|| {
                        Error::new("DCLTCFQ1 recovery omitted Trading FundingLedgerV2")
                    })?;
                let mut recovery_accounts = BTreeMap::new();
                for (index, funding) in coordinates.funding_ledgers.iter().enumerate() {
                    recovery_accounts.insert(
                        format!("founding_prepared_funding_ledger_v2_{index}"),
                        funding.address.to_string(),
                    );
                }
                recovery_accounts.insert(
                    "founding_controller_funding_checkpoint".into(),
                    coordinates.controller_funding_checkpoint.to_string(),
                );
                recovery_accounts.insert(
                    "founding_controller_funding_source".into(),
                    payer.pubkey().to_string(),
                );
                recovery_accounts.insert(
                    "founding_projection_witness".into(),
                    projection_witness.pubkey().to_string(),
                );
                recovery_accounts.insert(
                    "founding_prepared_lifecycle_rent_credit".into(),
                    coordinates.credit.to_string(),
                );
                let mut recovery_completed = completed.clone();
                recovery_completed.push(format!(
                "executed DCLTCFQ1: exact Resolution and Trading Pending ledgers plus their Prepared checkpoint; compiled {} complete keys, {} signatures, {} message bytes, {} packet bytes",
                controller_funding_prepare_geometry.complete_keys,
                controller_funding_prepare_geometry.required_signatures,
                controller_funding_prepare_geometry.message_bytes,
                controller_funding_prepare_geometry.packet_bytes,
            ));
                let recovery_payload = serde_json::to_vec(&Dcltcfq1RecoveryPayloadV1 {
                    schema: DCLTCFQ1_RECOVERY_PAYLOAD_SCHEMA_V1.into(),
                    checkpoint: MarketExecutionCheckpointV1 {
                        schema: DCLTCFQ1_PREPARED_CHECKPOINT_SCHEMA_V1.into(),
                        market: coordinates.market.to_string(),
                        founding_custody_context: hex(&coordinates.context),
                        direct_selected_manifest_entry_index: coordinates.capability_entry_index,
                        direct_capability_root: root.to_string(),
                        direct_trading_funding_ledger: trading_ledger.address.to_string(),
                        expiry_slot,
                        found_record: found_record.raw.to_string(),
                        lock_record: lock_record.raw.to_string(),
                        local_participant_fixture_liquidity: local_participant_fixture_liquidity
                            .cloned(),
                        accounts: accounts.clone(),
                        completed: recovery_completed,
                    },
                    completion_accounts: recovery_accounts,
                })?;
                let mut completion = |rpc: &mut Rpc| {
                    authenticate_controller_funding_checkpoint_v1(
                        rpc,
                        plan,
                        &coordinates,
                        payer.pubkey(),
                        ControllerFundingCheckpointPhaseV1::Prepared,
                    )
                    .map(|_| ())
                };
                send_durable_founding_v1(
                    rpc,
                    "prepare exact controller funding ledgers and checkpoint (DCLTCFQ1)",
                    FoundingSubmissionOperationV1::Dcltcfq1,
                    std::slice::from_ref(&controller_funding_prepare),
                    &[payer, &projection_witness],
                    *prepare_routing,
                    prepare_tables,
                    founding_instruction_account_digest_v1(
                        payer.pubkey(),
                        &controller_funding_prepare,
                    ),
                    &prepare_accounts,
                    &prepare_accounts,
                    recovery_payload,
                    Some(FOUNDING_HEAP_FRAME_BYTES),
                    recorder,
                    &mut completion,
                )?
            }
            Some(_) | None => rpc.send_v0_on_founding_heap_with_signers(
                "prepare exact controller funding ledgers and checkpoint (DCLTCFQ1)",
                std::slice::from_ref(&controller_funding_prepare),
                payer,
                &[&projection_witness],
                *prepare_routing,
                prepare_tables,
            )?,
        })
    };
    if let Some(prepare_honest) = prepare_honest {
        transactions.push(prepare_honest);
    }
    authenticate_controller_funding_checkpoint_v1(
        rpc,
        plan,
        &coordinates,
        payer.pubkey(),
        ControllerFundingCheckpointPhaseV1::Prepared,
    )?;
    if !resume_prepared
        && lane == PrestateLaneV1::Founding
        && let Some(recorder) = submission_recorder.as_deref_mut()
    {
        let journal = recorder
            .current(FoundingSubmissionOperationV1::Dcltcfq1)
            .cloned()
            .ok_or_else(|| Error::new("DCLTCFQ1 finalized without its durable journal"))?;
        let prepared = materialize_dcltcfq1_checkpoint_v1(rpc, &recorder.binding, &journal)?;
        // The Finalized journal was fsynced inside send_durable_founding_v1;
        // this callback atomically persists the strictly later checkpoint.
        checkpoint(&prepared)?;
    }
    if !resume_prepared {
        let prepared_abort = build_controller_funding_cleanup_v1(
            rpc,
            plan,
            records,
            &coordinates,
            ControllerFundingCheckpointPhaseV1::Prepared,
            CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1,
        )?;
        let prepared_abort_geometry =
            projected_bootstrap_compiled_geometry_v2(payer.pubkey(), &prepared_abort)?;
        authenticate_cleanup_compiled_census_v1(
            payer.pubkey(),
            &prepared_abort,
            prepared_abort_geometry,
        )?;
        let (prepared_abort_routing, prepared_abort_tables) = publish_routing_table(
            rpc,
            payer,
            "DCLTCF1A",
            std::slice::from_ref(&prepared_abort),
            transactions,
        )?;
        let prepared_abort_probe = crate::seed::fresh_probe_address();
        let refused_prepared_abort = rpc.send_v0_expected_failure_with_signers(
            "DCLTCF1A refuses to close the first controller ledger before checkpoint expiry",
            &[
                transfer(&payer.pubkey(), &prepared_abort_probe, 1),
                prepared_abort,
            ],
            payer,
            &[],
            prepared_abort_routing,
            &prepared_abort_tables,
        )?;
        if rpc.account(prepared_abort_probe)?.is_some()
            || refused_prepared_abort.fee_only_balance_change != Some(true)
        {
            return Err(Error::new(
                "pre-expiry DCLTCF1A did not roll its whole transaction back",
            ));
        }
        transactions.push(refused_prepared_abort);
        authenticate_controller_funding_checkpoint_v1(
            rpc,
            plan,
            &coordinates,
            payer.pubkey(),
            ControllerFundingCheckpointPhaseV1::Prepared,
        )?;
        completed.push(format!(
        "executed DCLTCFQ1: exact Resolution and Trading Pending ledgers plus their Prepared checkpoint; compiled {} complete keys, {} signatures, {} message bytes, {} packet bytes",
        controller_funding_prepare_geometry.complete_keys,
        controller_funding_prepare_geometry.required_signatures,
        controller_funding_prepare_geometry.message_bytes,
        controller_funding_prepare_geometry.packet_bytes,
    ));
        completed.push(format!(
        "proved DCLTCF1A refuses before expiry with full rollback; compiled {} complete keys, {} signatures, {} message bytes, {} packet bytes",
        prepared_abort_geometry.complete_keys,
        prepared_abort_geometry.required_signatures,
        prepared_abort_geometry.message_bytes,
        prepared_abort_geometry.packet_bytes,
        ));
    }

    let bootstrap = build_projected_custody_bootstrap_v2(
        plan,
        &coordinates,
        records,
        found_record.raw,
        lock_record.raw,
        projection_witness.pubkey(),
        payer.pubkey(),
        beneficiary.pubkey(),
        source_funder.pubkey(),
        mint,
    )?;
    let bootstrap_geometry = projected_bootstrap_compiled_geometry_v2(payer.pubkey(), &bootstrap)?;
    let admitted_boundary = projected_bootstrap_compiled_geometry_v2(
        payer.pubkey(),
        &append_distinct_census_accounts_v1(&bootstrap, 4),
    )?;
    let refused_boundary = projected_bootstrap_compiled_geometry_v2(
        payer.pubkey(),
        &append_distinct_census_accounts_v1(&bootstrap, 5),
    )?;
    if bootstrap_geometry.complete_keys != PROJECTED_CUSTODY_BOOTSTRAP_COMPLETE_KEYS_V2
        || bootstrap_geometry.complete_keys > DEVNET_ACCOUNT_LOCK_LIMIT_V1
        || admitted_boundary.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1
        || refused_boundary.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1
    {
        return Err(Error::new(format!(
            "DCLTPCB2 census refused: base {} keys, +4 {} keys, +5 {} keys; expected exactly {}, {}, {}",
            bootstrap_geometry.complete_keys,
            admitted_boundary.complete_keys,
            refused_boundary.complete_keys,
            PROJECTED_CUSTODY_BOOTSTRAP_COMPLETE_KEYS_V2,
            DEVNET_ACCOUNT_LOCK_LIMIT_V1,
            DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1,
        )));
    }
    let (routing, tables) = publish_routing_table(
        rpc,
        payer,
        "DCLTPCB2",
        std::slice::from_ref(&bootstrap),
        transactions,
    )?;

    let created = [
        ("projected_custody_replay", coordinates.projected_replay),
        ("hoard_vault", coordinates.hoard_vault),
        ("source_vault", coordinates.source_vault),
        ("source_replay", coordinates.source_replay),
    ];
    let recovering_stage = lane == PrestateLaneV1::Founding
        && submission_recorder.as_deref().is_some_and(|recorder| {
            recorder
                .current(FoundingSubmissionOperationV1::Dcltpcb2)
                .is_some()
        });
    let refuse_left_nothing = |rpc: &mut Rpc, label: &str| -> Result<()> {
        for (name, key) in created {
            if rpc.account(key)?.is_some() {
                return Err(Error::new(format!("{label} left a {name} account")));
            }
        }
        authenticate_controller_funding_checkpoint_v1(
            rpc,
            plan,
            &coordinates,
            payer.pubkey(),
            ControllerFundingCheckpointPhaseV1::Prepared,
        )?;
        Ok(())
    };
    if !recovering_stage {
        refuse_left_nothing(rpc, "the founding prestate")?;
    }

    // Both bootstrap hostile cases belong to the founding lane. Re-running
    // them for the abort lane's prestate would cost two transactions and
    // 700,000 compute units to re-prove a coordinate that has not moved.
    if lane == PrestateLaneV1::Founding && !resume_prepared && !recovering_stage {
        // The bootstrap admits exactly one request: the terminal Lock this
        // founding determines. A well-formed non-terminal prestate is refused.
        let mut non_terminal = bootstrap.clone();
        non_terminal
            .accounts
            .get_mut(1)
            .ok_or_else(|| Error::new("bootstrap omitted its terminal Lock coordinate"))?
            .pubkey = open_hoard_record
            .ok_or_else(|| Error::new("the founding lane omitted its non-terminal record"))?
            .raw;
        transactions.push(rpc.send_v0_on_founding_heap_expected_failure_with_signers(
            "DCLTPCB2 refuses a non-terminal projected-Custody request",
            &[non_terminal],
            payer,
            &[&beneficiary],
            routing,
            &tables,
        )?);
        refuse_left_nothing(rpc, "the refused non-terminal bootstrap")?;

        // The FundingState tail is the manifest binding. Reordering it derives an
        // address the manifest entry at that position does not name, and the whole
        // four-stage bootstrap must roll back with no funded account left behind.
        if coordinates.funding_ledgers.len() >= 2 {
            let mut reordered = bootstrap.clone();
            let first = PROJECTED_CUSTODY_BOOTSTRAP_RESOLUTION_LEDGER_V2;
            let second = PROJECTED_CUSTODY_BOOTSTRAP_TRADING_LEDGER_V2;
            reordered.accounts.swap(first, second);
            let rollback_recipient = crate::seed::fresh_probe_address();
            let rolled_back = rpc.send_v0_on_founding_heap_expected_failure_with_signers(
                "DCLTPCB2 refuses a reordered FundingLedgerV2 tail and rolls the transaction back",
                &[transfer(&payer.pubkey(), &rollback_recipient, 1), reordered],
                payer,
                &[&beneficiary],
                routing,
                &tables,
            )?;
            let fee_only = rolled_back.fee_only_balance_change;
            if rpc.account(rollback_recipient)?.is_some() || fee_only != Some(true) {
                return Err(Error::new(format!(
                    "refused bootstrap did not roll its whole transaction back to a fee-only \
                     debit: fee_only_balance_change={fee_only:?}",
                )));
            }
            transactions.push(rolled_back);
            refuse_left_nothing(rpc, "the refused reordered bootstrap")?;
        }
    }

    // The staging transaction has to land before the prestate's own expiry, or
    // Custody's `initialize` refuses outright. Say so here rather than let a
    // slow validator turn into an opaque refusal three transactions later.
    if !recovering_stage {
        let staged_at = rpc.finalized_slot()?;
        require_expiry_margin_v1(
            "projected-Custody staging",
            staged_at,
            expiry_slot,
            lane.minimum_staging_margin_slots(input)?,
        )?;
    }

    let honest = if lane == PrestateLaneV1::Founding {
        match submission_recorder.as_deref_mut() {
            Some(recorder) => {
                let mut prestate_addresses =
                    created.iter().map(|(_, key)| *key).collect::<Vec<_>>();
                prestate_addresses.extend(
                    coordinates
                        .funding_ledgers
                        .iter()
                        .map(|funding| funding.address),
                );
                prestate_addresses.push(coordinates.controller_funding_checkpoint);
                prestate_addresses.push(source_funder.pubkey());
                let root = Pubkey::new_from_array(coordinates.found.capability_root().to_bytes());
                let trading = pubkey(&plan.trading.program_id)?;
                let trading_ledger = coordinates
                    .funding_ledgers
                    .iter()
                    .find(|ledger| ledger.controller == trading)
                    .ok_or_else(|| {
                        Error::new("founding recovery omitted Trading FundingLedgerV2")
                    })?;
                let mut recovery_accounts = BTreeMap::new();
                for (label, key) in created {
                    recovery_accounts.insert(format!("founding_{label}"), key.to_string());
                }
                for (index, funding) in coordinates.funding_ledgers.iter().enumerate() {
                    recovery_accounts.insert(
                        format!("founding_funding_ledger_v2_{index}"),
                        funding.address.to_string(),
                    );
                }
                // The Direct capability root is a CHECKPOINT COORDINATE (the
                // checkpoint field below carries it), not a DCLTPCB2 product:
                // since f581af6b root/replay setup is permissionless
                // first-use, so no account exists at this address until the
                // Direct exterior first runs. Requiring it here (90b6bf7a)
                // made every complete founding refuse its own poststate.
                recovery_accounts.insert(
                    "direct_trading_funding_ledger".into(),
                    trading_ledger.address.to_string(),
                );
                recovery_accounts.insert(
                    "founding_lifecycle_rent_credit".into(),
                    coordinates.credit.to_string(),
                );
                recovery_accounts.insert(
                    "founding_source_funder_after_dcltpcb2".into(),
                    source_funder.pubkey().to_string(),
                );
                let mut completion_addresses = recovery_accounts
                    .values()
                    .map(|value| {
                        value.parse::<Pubkey>().map_err(|error| {
                            Error::new(format!("founding recovery account: {error}"))
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                completion_addresses.sort_unstable();
                completion_addresses.dedup();
                let mut recovery_completed = completed.clone();
                recovery_completed.extend(dcltpcb2_completion_lines_v1(bootstrap_geometry));
                let recovery_payload = serde_json::to_vec(&Dcltpcb2RecoveryPayloadV1 {
                    schema: "dclutch-market-dcltpcb2-recovery-payload-v1".into(),
                    checkpoint: MarketExecutionCheckpointV1 {
                        schema: "dclutch-market-dcltpcb2-checkpoint-v1".into(),
                        market: coordinates.market.to_string(),
                        founding_custody_context: hex(&coordinates.context),
                        direct_selected_manifest_entry_index: coordinates.capability_entry_index,
                        direct_capability_root: root.to_string(),
                        direct_trading_funding_ledger: trading_ledger.address.to_string(),
                        expiry_slot,
                        found_record: found_record.raw.to_string(),
                        lock_record: lock_record.raw.to_string(),
                        local_participant_fixture_liquidity: local_participant_fixture_liquidity
                            .cloned(),
                        accounts: accounts.clone(),
                        completed: recovery_completed,
                    },
                    completion_accounts: recovery_accounts,
                })?;
                let resolved_accounts_sha256 =
                    founding_instruction_account_digest_v1(payer.pubkey(), &bootstrap);
                let mut completion = |rpc: &mut Rpc| {
                    authenticate_bootstrap_poststate(
                        rpc,
                        &coordinates,
                        custody,
                        token_program,
                        mint,
                        principal,
                    )
                };
                send_durable_founding_v1(
                    rpc,
                    lane.prestate_label(),
                    FoundingSubmissionOperationV1::Dcltpcb2,
                    std::slice::from_ref(&bootstrap),
                    &[payer, &beneficiary],
                    routing,
                    &tables,
                    resolved_accounts_sha256,
                    &prestate_addresses,
                    &completion_addresses,
                    recovery_payload,
                    Some(FOUNDING_HEAP_FRAME_BYTES),
                    recorder,
                    &mut completion,
                )?
            }
            None => rpc.send_v0_on_founding_heap_with_signers(
                lane.prestate_label(),
                std::slice::from_ref(&bootstrap),
                payer,
                &[&beneficiary],
                routing,
                &tables,
            )?,
        }
    } else {
        rpc.send_v0_on_founding_heap_with_signers(
            lane.prestate_label(),
            std::slice::from_ref(&bootstrap),
            payer,
            &[&beneficiary],
            routing,
            &tables,
        )?
    };
    transactions.push(honest);

    authenticate_bootstrap_poststate(rpc, &coordinates, custody, token_program, mint, principal)?;
    authenticate_controller_funding_checkpoint_v1(
        rpc,
        plan,
        &coordinates,
        payer.pubkey(),
        ControllerFundingCheckpointPhaseV1::CustodyStaged,
    )?;
    let prefix = lane.evidence_prefix();
    for (label, key) in created {
        let account = rpc.required_account(key, label)?;
        accounts.insert(format!("{prefix}_{label}"), account_evidence(key, &account));
    }
    for (index, funding) in coordinates.funding_ledgers.iter().enumerate() {
        let account = rpc.required_account(funding.address, "staged FundingLedgerV2")?;
        accounts.insert(
            format!("{prefix}_funding_ledger_v2_{index}"),
            account_evidence(funding.address, &account),
        );
    }
    if lane == PrestateLaneV1::Founding {
        let root = Pubkey::new_from_array(coordinates.found.capability_root().to_bytes());
        // Under transaction publication the Direct capability root is created
        // permissionlessly on FIRST USE by the Direct exterior, so at founding
        // time its absence is the expected state, not missing evidence. The
        // checkpoint carries the coordinate either way; the account row joins
        // the evidence only where a genesis pre-created the root.
        if let Some(root_account) = rpc.account(root)? {
            accounts.insert(
                "direct_capability_root".into(),
                account_evidence(root, &root_account),
            );
        }
        let trading = pubkey(&plan.trading.program_id)?;
        let trading_ledger = coordinates
            .funding_ledgers
            .iter()
            .find(|ledger| ledger.controller == trading)
            .ok_or_else(|| Error::new("founding evidence omitted the Trading FundingLedgerV2"))?;
        let ledger_account =
            rpc.required_account(trading_ledger.address, "direct_trading_funding_ledger")?;
        accounts.insert(
            "direct_trading_funding_ledger".into(),
            account_evidence(trading_ledger.address, &ledger_account),
        );
    }
    let credit_account = rpc.required_account(coordinates.credit, "staged lifecycle credit")?;
    accounts.insert(
        format!("{prefix}_lifecycle_rent_credit"),
        account_evidence(coordinates.credit, &credit_account),
    );
    let staged_source = rpc.required_account(source_funder.pubkey(), "staged source funder")?;
    accounts.insert(
        format!("{prefix}_source_funder_after_dcltpcb2"),
        account_evidence(source_funder.pubkey(), &staged_source),
    );
    completed.extend(dcltpcb2_completion_lines_v1(bootstrap_geometry));

    // This checkpoint means DCLTPCB2 is COMPLETE, not merely intended. It is
    // written only after the exact SourceFunded prestate, both controller
    // ledgers, record graph, and rent credit have been reacquired from the
    // finalized chain. A restart can therefore resume the still-atomic
    // DCLTGMF3 suffix without replaying any principal movement.
    if lane == PrestateLaneV1::Founding {
        let trading = pubkey(&plan.trading.program_id)?;
        let trading_ledger = coordinates
            .funding_ledgers
            .iter()
            .find(|ledger| ledger.controller == trading)
            .ok_or_else(|| Error::new("founding checkpoint omitted the Trading FundingLedgerV2"))?;
        checkpoint(&MarketExecutionCheckpointV1 {
            schema: "dclutch-market-dcltpcb2-checkpoint-v1".into(),
            market: coordinates.market.to_string(),
            founding_custody_context: hex(&coordinates.context),
            direct_selected_manifest_entry_index: coordinates.capability_entry_index,
            direct_capability_root: Pubkey::new_from_array(
                coordinates.found.capability_root().to_bytes(),
            )
            .to_string(),
            direct_trading_funding_ledger: trading_ledger.address.to_string(),
            expiry_slot,
            found_record: found_record.raw.to_string(),
            lock_record: lock_record.raw.to_string(),
            local_participant_fixture_liquidity: local_participant_fixture_liquidity.cloned(),
            accounts: accounts.clone(),
            completed: completed.clone(),
        })?;
    }

    // The prestate is complete. What it is for is the lane's business.
    match lane {
        // Everything from here is one transaction.
        PrestateLaneV1::Founding => execute_generic_market_founding(
            rpc,
            plan,
            input,
            records,
            &coordinates,
            product,
            mint,
            found31_market,
            actors,
            found_record.raw,
            lock_record.raw,
            claim_count,
            payer,
            transactions,
            accounts,
            completed,
            submission_recorder.as_deref_mut(),
        ),
        PrestateLaneV1::SourceAbort => execute_source_abort_v1(
            rpc,
            plan,
            records,
            &coordinates,
            expiry_slot,
            lock_record.raw,
            &beneficiary,
            source_funder.pubkey(),
            mint,
            payer,
            transactions,
            accounts,
            completed,
            SourceAbortExpiryPolicyV1::from_input(input)?,
        ),
    }?;
    Ok(coordinates.context)
}

/// Sole top-level projected-Custody founding-abort instruction.
///
/// Owned by `programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs`
/// (`PROJECTED_CUSTODY_ABORT_MAGIC_V1`); restated here because a localhost host
/// utility does not depend on an SBF program crate.
const PROJECTED_CUSTODY_ABORT_MAGIC_V1: [u8; 8] = *b"DCLTPCA1";

/// Exact `DCLTPCA1` frame width: one raw request, Custody's checked Loader
/// pair, and Custody's own frame.
const PROJECTED_CUSTODY_ABORT_PREFIX_ACCOUNTS_V1: usize = 19;
const CONTROLLER_FUNDING_ABORT_ACCOUNTS_V1: usize = 17;
const PROJECTED_CUSTODY_ABORT_ACCOUNTS_V1: usize =
    PROJECTED_CUSTODY_ABORT_PREFIX_ACCOUNTS_V1 + CONTROLLER_FUNDING_ABORT_ACCOUNTS_V1;
const PROJECTED_CUSTODY_ABORT_COMPLETE_KEYS_V1: usize = 33;
// 19 until 5ca145e8 de-aliased the DCLTCFQ1 funding source into the payer,
// removing one distinct key from the cleanup frame.
const CONTROLLER_FUNDING_CLEANUP_COMPLETE_KEYS_V1: usize = 18;
/// Derived from the two pinned numbers so the exact lock-limit proof
/// (base + padding == 64, one more refuses) cannot lag the frame the way a
/// literal did when 5ca145e8 de-aliased the funding source.
const CONTROLLER_FUNDING_CLEANUP_CENSUS_PADDING_V1: usize =
    DEVNET_ACCOUNT_LOCK_LIMIT_V1 - CONTROLLER_FUNDING_CLEANUP_COMPLETE_KEYS_V1;
const CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1: [u8; 8] = *b"DCLTCF1A";
const CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1: [u8; 8] = *b"DCLTCF2A";

/// The three durable mutations in an expired staged-founding recovery.
///
/// These labels are intentionally not founding-success labels. A caller may
/// resume this suffix only from the onchain ControllerFundingCheckpoint phase,
/// and no partial prefix is consumer evidence for an Open Market.
#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SourceAbortRecoveryOperationV1 {
    Custody,
    ControllerFirst,
    ControllerTerminal,
}

impl SourceAbortRecoveryOperationV1 {
    pub(crate) const ORDERED: [Self; 3] = [
        Self::Custody,
        Self::ControllerFirst,
        Self::ControllerTerminal,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Custody => "source-abort-custody-v1",
            Self::ControllerFirst => "source-abort-controller-first-v1",
            Self::ControllerTerminal => "source-abort-controller-terminal-v1",
        }
    }

    pub(crate) const fn expected_complete_keys(self) -> usize {
        match self {
            Self::Custody => PROJECTED_CUSTODY_ABORT_COMPLETE_KEYS_V1,
            Self::ControllerFirst | Self::ControllerTerminal => {
                CONTROLLER_FUNDING_CLEANUP_COMPLETE_KEYS_V1
            }
        }
    }
}

#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SourceAbortRecoveryPhaseV1 {
    CustodyStaged,
    CustodyAborted,
    CustodyFirstLedgerClosed,
    Complete,
}

impl SourceAbortRecoveryPhaseV1 {
    pub(crate) const fn next_operation(self) -> Option<SourceAbortRecoveryOperationV1> {
        match self {
            Self::CustodyStaged => Some(SourceAbortRecoveryOperationV1::Custody),
            Self::CustodyAborted => Some(SourceAbortRecoveryOperationV1::ControllerFirst),
            Self::CustodyFirstLedgerClosed => {
                Some(SourceAbortRecoveryOperationV1::ControllerTerminal)
            }
            Self::Complete => None,
        }
    }
}

/// Exact pre-mutation quantities needed to prove that the abort returned
/// principal to its immutable supplier and rent to its immutable credit.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SourceAbortRecoveryBaselineV1 {
    pub(crate) market: String,
    pub(crate) controller_funding_checkpoint: String,
    pub(crate) funding_ledgers: Vec<String>,
    pub(crate) destination: String,
    pub(crate) destination_before_atoms: u64,
    pub(crate) principal_atoms: u64,
    pub(crate) lifecycle_rent_credit: String,
    pub(crate) lifecycle_rent_credit_before_lamports: u64,
    pub(crate) controller_funding_source: String,
    pub(crate) controller_funding_source_before_lamports: u64,
    pub(crate) controller_rent_refund_lamports: u64,
    pub(crate) controller_native_refund_lamports: u64,
    pub(crate) beneficiary: String,
    pub(crate) expiry_slot: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceAbortRecoveryPlanV1 {
    pub(crate) phase: SourceAbortRecoveryPhaseV1,
    pub(crate) operation: Option<SourceAbortRecoveryOperationV1>,
    pub(crate) instruction: Option<Instruction>,
    pub(crate) beneficiary: Pubkey,
    pub(crate) complete_keys: Option<usize>,
    pub(crate) message_bytes: Option<usize>,
    pub(crate) packet_bytes: Option<usize>,
}

fn source_abort_context_from_checkpoint_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    checkpoint: &MarketExecutionCheckpointV1,
) -> Result<RecoveredFoundingContextV1> {
    if checkpoint.schema != DCLTPCB2_CHECKPOINT_SCHEMA_V1 {
        return Err(Error::new(
            "SourceAbort recovery requires the finalized DCLTPCB2 checkpoint schema",
        ));
    }
    reconstruct_founding_checkpoint_v1(
        rpc,
        plan,
        input,
        payer,
        forge,
        actors,
        &mut Vec::new(),
        checkpoint,
        false,
    )
}

pub(crate) fn capture_source_abort_recovery_baseline_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    checkpoint: &MarketExecutionCheckpointV1,
) -> Result<SourceAbortRecoveryBaselineV1> {
    let context = source_abort_context_from_checkpoint_v1(
        rpc, plan, input, payer, forge, actors, checkpoint,
    )?;
    let coordinates = &context.coordinates;
    let custody = pubkey(&plan.custody.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    authenticate_bootstrap_poststate(
        rpc,
        coordinates,
        custody,
        token_program,
        context.mint,
        coordinates.lock.amount,
    )?;
    let checkpoint_account = rpc.required_account(
        coordinates.controller_funding_checkpoint,
        "staged controller funding checkpoint",
    )?;
    let controller_checkpoint = ControllerFundingCheckpointV1::decode(&checkpoint_account.data)
        .map_err(|error| Error::new(format!("staged controller funding checkpoint: {error:?}")))?;
    if checkpoint_account.owner != pubkey(&plan.trading.program_id)?
        || controller_checkpoint.phase() != ControllerFundingCheckpointPhaseV1::CustodyStaged
    {
        return Err(Error::new(
            "SourceAbort baseline may be captured only from exact CustodyStaged state",
        ));
    }
    if rpc.finalized_slot()? <= checkpoint.expiry_slot {
        return Err(Error::new(
            "SourceAbort baseline refuses while the staged founding remains satisfiable",
        ));
    }
    let destination = forge.peek_pubkey(role::FOUNDING_SOURCE_FUNDER)?;
    let beneficiary = forge.peek_pubkey(role::FOUNDING_BENEFICIARY)?;
    let source_account = rpc.required_account(destination, "abort refund destination")?;
    let parsed_source = TokenAccount::parse(&source_account.data)
        .map_err(|error| Error::new(format!("abort refund destination: {error:?}")))?;
    if source_account.owner != token_program || parsed_source.owner != beneficiary.to_bytes() {
        return Err(Error::new(
            "SourceAbort supplier account no longer names the immutable beneficiary",
        ));
    }
    let controller_funding_source =
        Pubkey::new_from_array(controller_checkpoint.input_ref().funding_source);
    let controller_rent_refund_lamports = coordinates
        .funding_ledgers
        .iter()
        .try_fold(
            rpc.minimum_balance(CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1)?,
            |total, ledger| total.checked_add(rpc.minimum_balance(ledger.bytes.len()).ok()?),
        )
        .ok_or_else(|| Error::new("controller funding Rent refund overflow"))?;
    let controller_native_refund_lamports = coordinates
        .funding_ledgers
        .iter()
        .try_fold(0_u64, |total, ledger| {
            ledger
                .required_lamports
                .checked_sub(rpc.minimum_balance(ledger.bytes.len()).ok()?)
                .and_then(|value| total.checked_add(value))
        })
        .ok_or_else(|| Error::new("controller funding native refund overflow"))?;
    Ok(SourceAbortRecoveryBaselineV1 {
        market: coordinates.market.to_string(),
        controller_funding_checkpoint: coordinates.controller_funding_checkpoint.to_string(),
        funding_ledgers: coordinates
            .funding_ledgers
            .iter()
            .map(|ledger| ledger.address.to_string())
            .collect(),
        destination: destination.to_string(),
        destination_before_atoms: parsed_source.amount,
        principal_atoms: coordinates.lock.amount,
        lifecycle_rent_credit: coordinates.credit.to_string(),
        lifecycle_rent_credit_before_lamports: rpc
            .required_account(coordinates.credit, "abort lifecycle credit")?
            .lamports,
        controller_funding_source: controller_funding_source.to_string(),
        controller_funding_source_before_lamports: rpc
            .required_account(
                controller_funding_source,
                "controller funding refund source",
            )?
            .lamports,
        controller_rent_refund_lamports,
        controller_native_refund_lamports,
        beneficiary: beneficiary.to_string(),
        expiry_slot: checkpoint.expiry_slot,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_source_abort_recovery_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    checkpoint: &MarketExecutionCheckpointV1,
    baseline: &SourceAbortRecoveryBaselineV1,
) -> Result<SourceAbortRecoveryPlanV1> {
    let context = source_abort_context_from_checkpoint_v1(
        rpc, plan, input, payer, forge, actors, checkpoint,
    )?;
    let coordinates = &context.coordinates;
    let destination = pubkey(&baseline.destination)?;
    let beneficiary = pubkey(&baseline.beneficiary)?;
    let controller_funding_source = pubkey(&baseline.controller_funding_source)?;
    if baseline.market != coordinates.market.to_string()
        || baseline.controller_funding_checkpoint
            != coordinates.controller_funding_checkpoint.to_string()
        || baseline.funding_ledgers
            != coordinates
                .funding_ledgers
                .iter()
                .map(|ledger| ledger.address.to_string())
                .collect::<Vec<_>>()
        || baseline.lifecycle_rent_credit != coordinates.credit.to_string()
        || baseline.principal_atoms != coordinates.lock.amount
        || baseline.expiry_slot != checkpoint.expiry_slot
        || forge.peek_pubkey(role::FOUNDING_SOURCE_FUNDER)? != destination
        || forge.peek_pubkey(role::FOUNDING_BENEFICIARY)? != beneficiary
    {
        return Err(Error::new(
            "SourceAbort recovery baseline differs from reconstructed founding coordinates",
        ));
    }
    let checkpoint_account = rpc.account(coordinates.controller_funding_checkpoint)?;
    let phase = match checkpoint_account {
        Some(account) => {
            let controller_checkpoint = ControllerFundingCheckpointV1::decode(&account.data)
                .map_err(|error| Error::new(format!("SourceAbort checkpoint: {error:?}")))?;
            if account.owner != pubkey(&plan.trading.program_id)? || account.executable {
                return Err(Error::new("SourceAbort checkpoint owner changed"));
            }
            match controller_checkpoint.phase() {
                ControllerFundingCheckpointPhaseV1::CustodyStaged => {
                    if rpc.finalized_slot()? <= checkpoint.expiry_slot {
                        return Err(Error::new(
                            "SourceAbort refuses while the staged founding remains satisfiable",
                        ));
                    }
                    authenticate_bootstrap_poststate(
                        rpc,
                        coordinates,
                        pubkey(&plan.custody.program_id)?,
                        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
                        context.mint,
                        coordinates.lock.amount,
                    )?;
                    SourceAbortRecoveryPhaseV1::CustodyStaged
                }
                ControllerFundingCheckpointPhaseV1::CustodyAborted => {
                    authenticate_source_abort_custody_poststate_v1(
                        rpc,
                        coordinates,
                        destination,
                        baseline.destination_before_atoms,
                        baseline.principal_atoms,
                    )?;
                    authenticate_controller_funding_cleanup_checkpoint_v1(
                        rpc,
                        plan,
                        coordinates,
                        controller_funding_source,
                        ControllerFundingCheckpointPhaseV1::CustodyAborted,
                    )?;
                    SourceAbortRecoveryPhaseV1::CustodyAborted
                }
                ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed => {
                    authenticate_source_abort_custody_poststate_v1(
                        rpc,
                        coordinates,
                        destination,
                        baseline.destination_before_atoms,
                        baseline.principal_atoms,
                    )?;
                    authenticate_controller_funding_cleanup_checkpoint_v1(
                        rpc,
                        plan,
                        coordinates,
                        controller_funding_source,
                        ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed,
                    )?;
                    SourceAbortRecoveryPhaseV1::CustodyFirstLedgerClosed
                }
                other => {
                    return Err(Error::new(format!(
                        "SourceAbort checkpoint phase {other:?} is not one of its exact suffix phases"
                    )));
                }
            }
        }
        None => {
            authenticate_source_abort_poststate_v1(
                rpc,
                coordinates,
                destination,
                baseline.destination_before_atoms,
                baseline.lifecycle_rent_credit_before_lamports,
                baseline.principal_atoms,
                controller_funding_source,
                baseline.controller_funding_source_before_lamports,
                baseline.controller_rent_refund_lamports,
                baseline.controller_native_refund_lamports,
            )?;
            SourceAbortRecoveryPhaseV1::Complete
        }
    };
    let instruction = match phase {
        SourceAbortRecoveryPhaseV1::CustodyStaged => Some(build_projected_custody_abort_v1(
            rpc,
            plan,
            &context.records,
            coordinates,
            context.lock_record,
            beneficiary,
            destination,
            context.mint,
        )?),
        SourceAbortRecoveryPhaseV1::CustodyAborted => Some(build_controller_funding_cleanup_v1(
            rpc,
            plan,
            &context.records,
            coordinates,
            ControllerFundingCheckpointPhaseV1::CustodyAborted,
            CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1,
        )?),
        SourceAbortRecoveryPhaseV1::CustodyFirstLedgerClosed => {
            Some(build_controller_funding_cleanup_v1(
                rpc,
                plan,
                &context.records,
                coordinates,
                ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed,
                CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1,
            )?)
        }
        SourceAbortRecoveryPhaseV1::Complete => None,
    };
    let operation = phase.next_operation();
    let geometry = instruction
        .as_ref()
        .map(|instruction| projected_bootstrap_compiled_geometry_v2(payer.pubkey(), instruction))
        .transpose()?;
    if let (Some(operation), Some(instruction), Some(geometry)) =
        (operation, instruction.as_ref(), geometry)
    {
        if geometry.complete_keys != operation.expected_complete_keys() {
            return Err(Error::new(format!(
                "{} compiled {} complete keys, expected {}",
                operation.label(),
                geometry.complete_keys,
                operation.expected_complete_keys()
            )));
        }
        match operation {
            SourceAbortRecoveryOperationV1::Custody => {
                let admitted = projected_bootstrap_compiled_geometry_v2(
                    payer.pubkey(),
                    &append_distinct_census_accounts_v1(instruction, 31),
                )?;
                let refused = projected_bootstrap_compiled_geometry_v2(
                    payer.pubkey(),
                    &append_distinct_census_accounts_v1(instruction, 32),
                )?;
                if admitted.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1
                    || refused.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1
                {
                    return Err(Error::new("DCLTPCA1 exterior 64/65 census changed"));
                }
            }
            SourceAbortRecoveryOperationV1::ControllerFirst
            | SourceAbortRecoveryOperationV1::ControllerTerminal => {
                authenticate_cleanup_compiled_census_v1(payer.pubkey(), instruction, geometry)?;
            }
        }
    }
    Ok(SourceAbortRecoveryPlanV1 {
        phase,
        operation,
        instruction,
        beneficiary,
        complete_keys: geometry.map(|value| value.complete_keys),
        message_bytes: geometry.map(|value| value.message_bytes),
        packet_bytes: geometry.map(|value| value.packet_bytes),
    })
}

/// Unwind an expired founding's funded source compartment on a real validator.
///
/// `OpenSourceCompartment` puts real collateral under a projected authority
/// against a Market that does not exist, and the only way forward is the Lock
/// stage of an atomic founding whose Core Found and Open stages both refuse an
/// expired artifact. So this is the route that decides whether a founder who
/// stages a prestate and does not found in time gets their principal back or
/// loses it permanently.
#[allow(clippy::too_many_arguments)]
fn execute_source_abort_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    records: &MarketRecords,
    coordinates: &FoundingCoordinates,
    expiry_slot: u64,
    lock_raw_account: Pubkey,
    beneficiary: &Keypair,
    destination: Pubkey,
    mint: Pubkey,
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
    accounts: &mut BTreeMap<String, AccountEvidence>,
    completed: &mut Vec<String>,
    expiry_policy: SourceAbortExpiryPolicyV1,
) -> Result<()> {
    let custody = pubkey(&plan.custody.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let principal = coordinates.lock.amount;

    let abort = build_projected_custody_abort_v1(
        rpc,
        plan,
        records,
        coordinates,
        lock_raw_account,
        beneficiary.pubkey(),
        destination,
        mint,
    )?;
    let abort_geometry = projected_bootstrap_compiled_geometry_v2(payer.pubkey(), &abort)?;
    let abort_admitted = projected_bootstrap_compiled_geometry_v2(
        payer.pubkey(),
        &append_distinct_census_accounts_v1(&abort, 31),
    )?;
    let abort_refused = projected_bootstrap_compiled_geometry_v2(
        payer.pubkey(),
        &append_distinct_census_accounts_v1(&abort, 32),
    )?;
    if abort_geometry.complete_keys != PROJECTED_CUSTODY_ABORT_COMPLETE_KEYS_V1
        || abort_admitted.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1
        || abort_refused.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1
    {
        return Err(Error::new(format!(
            "DCLTPCA1 census refused: base {}, +31 {}, +32 {}",
            abort_geometry.complete_keys, abort_admitted.complete_keys, abort_refused.complete_keys,
        )));
    }
    let (routing, tables) = publish_routing_table(
        rpc,
        payer,
        "DCLTPCA1",
        std::slice::from_ref(&abort),
        transactions,
    )?;

    // BEFORE expiry the abort must refuse, and this is the boundary that
    // matters most: while the founding is still satisfiable, the authority over
    // funded principal may not be destroyed. A route that let anyone unwind a
    // live founding would be a worse bug than the one it fixes.
    let staged_at = rpc.finalized_slot()?;
    require_expiry_margin_v1(
        "the pre-expiry DCLTPCA1 rollback probe",
        staged_at,
        expiry_slot,
        expiry_policy.minimum_pre_expiry_refusal_margin_slots(),
    )?;

    let rollback_recipient = crate::seed::fresh_probe_address();
    let refused = rpc.send_v0_expected_failure_with_signers(
        "DCLTPCA1 refuses to abort a funded source before expiry",
        &[
            transfer(&payer.pubkey(), &rollback_recipient, 1),
            abort.clone(),
        ],
        payer,
        &[beneficiary],
        routing,
        &tables,
    )?;
    let fee_only = refused.fee_only_balance_change;
    if rpc.account(rollback_recipient)?.is_some() || fee_only != Some(true) {
        return Err(Error::new(format!(
            "the refused pre-expiry abort did not roll its whole transaction back to a fee-only \
             debit: fee_only_balance_change={fee_only:?}",
        )));
    }
    transactions.push(refused);
    authenticate_bootstrap_poststate(rpc, coordinates, custody, token_program, mint, principal)?;

    // Now let it expire. The wait is the point of the test.
    await_finalized_slot(
        rpc,
        expiry_slot
            .checked_add(1)
            .ok_or_else(|| Error::new("expiry slot overflow"))?,
    )?;

    let credit_before = rpc
        .required_account(coordinates.credit, "abort lifecycle credit")?
        .lamports;
    let checkpoint_before = rpc.required_account(
        coordinates.controller_funding_checkpoint,
        "staged controller funding checkpoint",
    )?;
    let checkpoint = ControllerFundingCheckpointV1::decode(&checkpoint_before.data)
        .map_err(|error| Error::new(format!("staged controller funding checkpoint: {error:?}")))?;
    let controller_funding_source = Pubkey::new_from_array(checkpoint.input_ref().funding_source);
    let controller_funding_source_before = rpc
        .required_account(
            controller_funding_source,
            "controller funding refund source",
        )?
        .lamports;
    let controller_rent_refund = coordinates
        .funding_ledgers
        .iter()
        .try_fold(
            rpc.minimum_balance(CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1)?,
            |total, ledger| total.checked_add(rpc.minimum_balance(ledger.bytes.len()).ok()?),
        )
        .ok_or_else(|| Error::new("controller funding Rent refund overflow"))?;
    let controller_native_refund = coordinates
        .funding_ledgers
        .iter()
        .try_fold(0_u64, |total, ledger| {
            ledger
                .required_lamports
                .checked_sub(rpc.minimum_balance(ledger.bytes.len()).ok()?)
                .and_then(|value| total.checked_add(value))
        })
        .ok_or_else(|| Error::new("controller funding native refund overflow"))?;
    let destination_before = token_amount(rpc, destination, "abort refund destination")?;
    transactions.push(rpc.send_v0_with_signers(
        "unwind an expired founding's funded source compartment (DCLTPCA1)",
        std::slice::from_ref(&abort),
        payer,
        &[beneficiary],
        routing,
        &tables,
    )?);
    authenticate_source_abort_custody_poststate_v1(
        rpc,
        coordinates,
        destination,
        destination_before,
        principal,
    )?;
    authenticate_controller_funding_cleanup_checkpoint_v1(
        rpc,
        plan,
        coordinates,
        controller_funding_source,
        ControllerFundingCheckpointPhaseV1::CustodyAborted,
    )?;

    let cleanup_step1 = build_controller_funding_cleanup_v1(
        rpc,
        plan,
        records,
        coordinates,
        ControllerFundingCheckpointPhaseV1::CustodyAborted,
        CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1,
    )?;
    let cleanup_step1_geometry =
        projected_bootstrap_compiled_geometry_v2(payer.pubkey(), &cleanup_step1)?;
    authenticate_cleanup_compiled_census_v1(
        payer.pubkey(),
        &cleanup_step1,
        cleanup_step1_geometry,
    )?;
    let (cleanup_step1_routing, cleanup_step1_tables) = publish_routing_table(
        rpc,
        payer,
        "DCLTCF1A",
        std::slice::from_ref(&cleanup_step1),
        transactions,
    )?;
    transactions.push(rpc.send_v0_with_signers(
        "close the canonical first expired controller ledger (DCLTCF1A)",
        std::slice::from_ref(&cleanup_step1),
        payer,
        &[],
        cleanup_step1_routing,
        &cleanup_step1_tables,
    )?);
    authenticate_controller_funding_cleanup_checkpoint_v1(
        rpc,
        plan,
        coordinates,
        controller_funding_source,
        ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed,
    )?;

    let cleanup_step2 = build_controller_funding_cleanup_v1(
        rpc,
        plan,
        records,
        coordinates,
        ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed,
        CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1,
    )?;
    let cleanup_step2_geometry =
        projected_bootstrap_compiled_geometry_v2(payer.pubkey(), &cleanup_step2)?;
    authenticate_cleanup_compiled_census_v1(
        payer.pubkey(),
        &cleanup_step2,
        cleanup_step2_geometry,
    )?;
    let (cleanup_step2_routing, cleanup_step2_tables) = publish_routing_table(
        rpc,
        payer,
        "DCLTCF2A",
        std::slice::from_ref(&cleanup_step2),
        transactions,
    )?;
    transactions.push(rpc.send_v0_with_signers(
        "close the remaining expired controller ledger and checkpoint (DCLTCF2A)",
        std::slice::from_ref(&cleanup_step2),
        payer,
        &[],
        cleanup_step2_routing,
        &cleanup_step2_tables,
    )?);
    authenticate_source_abort_poststate_v1(
        rpc,
        coordinates,
        destination,
        destination_before,
        credit_before,
        principal,
        controller_funding_source,
        controller_funding_source_before,
        controller_rent_refund,
        controller_native_refund,
    )?;
    let credit_account = rpc.required_account(coordinates.credit, "abort lifecycle credit")?;
    accounts.insert(
        "abort_lifecycle_rent_credit".into(),
        account_evidence(coordinates.credit, &credit_account),
    );
    let refunded = rpc.required_account(destination, "abort refund destination")?;
    accounts.insert(
        "abort_refunded_principal_wallet".into(),
        account_evidence(destination, &refunded),
    );
    completed.push(
        "proved DCLTPCA1 refuses to unwind a funded source compartment while its founding is still satisfiable, and rolls the whole transaction back to a fee-only debit".into(),
    );
    completed.push(format!(
        "authenticated {:?} SourceAbort expiry geometry: {} slots from coordinate derivation, at least {} slots before staging, and at least {} slots before the pre-expiry rollback probe",
        expiry_policy,
        expiry_policy.expiry_slots(),
        expiry_policy.minimum_staging_margin_slots(),
        expiry_policy.minimum_pre_expiry_refusal_margin_slots(),
    ));
    completed.push(
        "executed DCLTPCA1 after expiry: the source principal is back with the party that supplied it, and the source vault, source replay, empty Hoard vault, and projection are all closed to the lifecycle credit".into(),
    );
    completed.push(format!(
        "compiled the exact staged DCLTPCA1 with {} complete keys, {} signatures, {} message bytes, and {} packet bytes; DCLTCF1A with {} complete keys and {} packet bytes; and DCLTCF2A with {} complete keys and {} packet bytes",
        abort_geometry.complete_keys,
        abort_geometry.required_signatures,
        abort_geometry.message_bytes,
        abort_geometry.packet_bytes,
        cleanup_step1_geometry.complete_keys,
        cleanup_step1_geometry.packet_bytes,
        cleanup_step2_geometry.complete_keys,
        cleanup_step2_geometry.packet_bytes,
    ));
    Ok(())
}

/// Build the exact 36-account staged `DCLTPCA1` frame.
fn build_projected_custody_abort_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    records: &MarketRecords,
    coordinates: &FoundingCoordinates,
    lock_raw_account: Pubkey,
    beneficiary: Pubkey,
    destination: Pubkey,
    mint: Pubkey,
) -> Result<Instruction> {
    let trading = pubkey(&plan.trading.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let custody_programdata = pubkey(&plan.custody.programdata_id)?;
    let cache = pubkey(&plan.activation)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let trading_programdata = pubkey(&plan.trading.programdata_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    // The caller PDA is single-use and derived from the abort request itself,
    // which is the terminal Lock with one field changed. Nothing else can sign
    // it, including the founding's own Lock caller.
    let request = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::AbortSourceAndClose,
        ..coordinates.lock
    };
    let raw = request
        .encode()
        .map_err(|error| Error::new(format!("abort request encoding: {error:?}")))?;
    let digest: [u8; 32] = Sha256::digest(raw).into();
    let caller = Pubkey::find_program_address(
        &ProjectedCustodyCallerSeedsV1::new(request, digest).as_slices(),
        &trading,
    )
    .0;

    let mut accounts = vec![
        AccountMeta::new_readonly(lock_raw_account, false),
        AccountMeta::new_readonly(custody, false),
        AccountMeta::new_readonly(custody_programdata, false),
        // Custody's own sixteen-account abort frame.
        AccountMeta::new_readonly(caller, false),
        AccountMeta::new(coordinates.projected_replay, false),
        AccountMeta::new_readonly(cache, false),
        AccountMeta::new_readonly(registry, false),
        AccountMeta::new_readonly(trading, false),
        AccountMeta::new_readonly(trading_programdata, false),
        AccountMeta::new(coordinates.credit, false),
        AccountMeta::new(coordinates.source_vault, false),
        AccountMeta::new(coordinates.source_replay, false),
        AccountMeta::new(coordinates.hoard_vault, false),
        AccountMeta::new(destination, false),
        // The principal's owner signs and stays non-writable, exactly as it had
        // to when it supplied the principal.
        AccountMeta::new_readonly(beneficiary, true),
        AccountMeta::new_readonly(coordinates.custody_authority, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(coordinates.market, false),
    ];
    if accounts.len() != PROJECTED_CUSTODY_ABORT_PREFIX_ACCOUNTS_V1 {
        return Err(Error::new(
            "assembled Custody abort prefix did not match its exact width",
        ));
    }
    accounts.extend(build_controller_funding_abort_accounts_v1(
        rpc,
        plan,
        records,
        coordinates,
        ControllerFundingCheckpointPhaseV1::CustodyStaged,
    )?);
    if accounts.len() != PROJECTED_CUSTODY_ABORT_ACCOUNTS_V1 {
        return Err(Error::new(
            "assembled staged abort frame did not match 36 accounts",
        ));
    }
    Ok(Instruction {
        program_id: trading,
        accounts,
        data: PROJECTED_CUSTODY_ABORT_MAGIC_V1.to_vec(),
    })
}

fn build_controller_funding_abort_accounts_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    records: &MarketRecords,
    coordinates: &FoundingCoordinates,
    expected_phase: ControllerFundingCheckpointPhaseV1,
) -> Result<Vec<AccountMeta>> {
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let checkpoint_account = rpc.required_account(
        coordinates.controller_funding_checkpoint,
        "controller funding checkpoint",
    )?;
    let checkpoint = ControllerFundingCheckpointV1::decode(&checkpoint_account.data)
        .map_err(|error| Error::new(format!("controller funding checkpoint: {error:?}")))?;
    if checkpoint_account.owner != trading || checkpoint.phase() != expected_phase {
        return Err(Error::new(
            "controller funding abort observed a foreign checkpoint owner or phase",
        ));
    }
    let input = checkpoint.input_ref();
    let resolution_ledger_key = Pubkey::new_from_array(input.resolution_ledger);
    let trading_ledger_key = Pubkey::new_from_array(input.trading_ledger);
    let resolution_should_be_closed = matches!(
        expected_phase,
        ControllerFundingCheckpointPhaseV1::PreparedFirstLedgerClosed
            | ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed
    ) && checkpoint.canonical_first_controller()
        == ControllerFundingControllerV1::Resolution;
    let resolution_ledger = match rpc.account(resolution_ledger_key)? {
        Some(account) => account,
        None if resolution_should_be_closed => RpcAccount {
            lamports: 0,
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        },
        None => {
            return Err(Error::new(
                "controller funding Resolution ledger disappeared before its cleanup step",
            ));
        }
    };
    if resolution_should_be_closed {
        if resolution_ledger.owner != system_program::ID
            || resolution_ledger.lamports != 0
            || !resolution_ledger.data.is_empty()
        {
            return Err(Error::new(
                "controller funding Resolution ledger differed from exact closed state",
            ));
        }
    } else if resolution_ledger.owner != resolution
        || resolution_ledger.lamports == 0
        || resolution_ledger.data.is_empty()
    {
        return Err(Error::new(
            "controller funding Resolution ledger differed from live Pending state",
        ));
    }
    let ledger_account_digest = pre_market_funding_ledger_account_digest_v1(
        resolution_ledger_key.to_bytes(),
        resolution_ledger.owner.to_bytes(),
        resolution_ledger.lamports,
        &resolution_ledger.data,
    );
    let checkpoint_digest: [u8; 32] = Sha256::digest(&checkpoint_account.data).into();
    let caller = if expected_phase == ControllerFundingCheckpointPhaseV1::CustodyStaged {
        // No Resolution ledger close is authorized until Custody has committed
        // SourceAbort and advanced the semantic checkpoint to phase 3. Bind
        // the first cleanup transaction to the exact phase-2 checkpoint with
        // a Trading-owned, non-signing anchor instead of encoding a forbidden
        // phase-2 Resolution abort packet.
        Pubkey::find_program_address(
            &[
                CONTROLLER_FUNDING_CUSTODY_ABORT_ANCHOR_DOMAIN_V1,
                coordinates.controller_funding_checkpoint.as_ref(),
                &checkpoint_digest,
            ],
            &trading,
        )
        .0
    } else {
        let request = PreMarketFundingAbortRequestV1 {
            checkpoint_phase: checkpoint.phase() as u8,
            checkpoint_revision: checkpoint.revision(),
            release_set: input.release_set,
            checkpoint: coordinates.controller_funding_checkpoint.to_bytes(),
            checkpoint_digest,
            market: input.market,
            generation: input.generation,
            manifest: input.manifest,
            funding_list: input.funding_list,
            selected_mask: input.resolution_mask,
            ledger: input.resolution_ledger,
            ledger_account_digest,
            funding_source: input.funding_source,
            rent_credit: input.rent_credit,
            expiry_slot: input.expiry_slot,
        }
        .encode()
        .map_err(|error| Error::new(format!("controller funding abort request: {error:?}")))?;
        Pubkey::find_program_address(
            &CallerAuthoritySeedsV1::from_bytes(
                input.release_set,
                input.market,
                ExecutionRoleV1::Trading,
                input.manifest,
                Sha256::digest(request).into(),
            )
            .map_err(|error| Error::new(format!("controller funding abort authority: {error:?}")))?
            .as_slices(),
            &trading,
        )
        .0
    };
    let accounts = vec![
        AccountMeta::new_readonly(caller, false),
        AccountMeta::new_readonly(trading, false),
        AccountMeta::new_readonly(pubkey(&plan.trading.programdata_id)?, false),
        AccountMeta::new_readonly(resolution, false),
        AccountMeta::new_readonly(pubkey(&plan.resolution.programdata_id)?, false),
        AccountMeta::new(coordinates.controller_funding_checkpoint, false),
        AccountMeta::new(resolution_ledger_key, false),
        AccountMeta::new(Pubkey::new_from_array(input.funding_source), false),
        AccountMeta::new(Pubkey::new_from_array(input.rent_credit), false),
        AccountMeta::new_readonly(pubkey(&plan.activation)?, false),
        AccountMeta::new_readonly(pubkey(&plan.registry.program_id)?, false),
        AccountMeta::new_readonly(records.manifest.raw, false),
        AccountMeta::new_readonly(records.manifest.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new(trading_ledger_key, false),
    ];
    if accounts.len() != CONTROLLER_FUNDING_ABORT_ACCOUNTS_V1 {
        return Err(Error::new("controller funding abort suffix width changed"));
    }
    Ok(accounts)
}

fn build_controller_funding_cleanup_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    records: &MarketRecords,
    coordinates: &FoundingCoordinates,
    expected_phase: ControllerFundingCheckpointPhaseV1,
    instruction_data: [u8; 8],
) -> Result<Instruction> {
    let accounts = build_controller_funding_abort_accounts_v1(
        rpc,
        plan,
        records,
        coordinates,
        expected_phase,
    )?;
    Ok(Instruction {
        program_id: pubkey(&plan.trading.program_id)?,
        accounts,
        data: instruction_data.to_vec(),
    })
}

/// Read one Token-2022 account's balance.
fn token_amount(rpc: &mut Rpc, key: Pubkey, label: &str) -> Result<u64> {
    let account = rpc.required_account(key, label)?;
    Ok(TokenAccount::parse(&account.data)
        .map_err(|error| Error::new(format!("{label}: {error:?}")))?
        .amount)
}

/// Require the abort to have returned the principal and closed all four.
fn authenticate_source_abort_custody_poststate_v1(
    rpc: &mut Rpc,
    coordinates: &FoundingCoordinates,
    destination: Pubkey,
    destination_before: u64,
    principal: u64,
) -> Result<()> {
    let expected_destination = destination_before
        .checked_add(principal)
        .ok_or_else(|| Error::new("source-abort token refund overflow"))?;
    if token_amount(rpc, destination, "abort refund destination")? != expected_destination {
        return Err(Error::new(
            "the abort did not return exactly the source principal to its supplier",
        ));
    }
    for (label, key) in [
        ("projected replay", coordinates.projected_replay),
        ("source vault", coordinates.source_vault),
        ("source replay", coordinates.source_replay),
        ("Hoard vault", coordinates.hoard_vault),
    ] {
        if let Some(account) = rpc.account(key)?
            && (account.lamports != 0 || !account.data.is_empty())
        {
            return Err(Error::new(format!("the abort left the {label} behind")));
        }
    }
    Ok(())
}

fn authenticate_source_abort_poststate_v1(
    rpc: &mut Rpc,
    coordinates: &FoundingCoordinates,
    destination: Pubkey,
    destination_before: u64,
    credit_before: u64,
    principal: u64,
    controller_funding_source: Pubkey,
    controller_funding_source_before: u64,
    controller_rent_refund: u64,
    controller_native_refund: u64,
) -> Result<()> {
    authenticate_source_abort_custody_poststate_v1(
        rpc,
        coordinates,
        destination,
        destination_before,
        principal,
    )?;
    for (label, key) in coordinates
        .funding_ledgers
        .iter()
        .map(|ledger| ("controller funding ledger", ledger.address))
        .chain(std::iter::once((
            "controller funding checkpoint",
            coordinates.controller_funding_checkpoint,
        )))
    {
        if rpc
            .account(key)?
            .is_some_and(|account| account.lamports != 0 || !account.data.is_empty())
        {
            return Err(Error::new(format!("the abort left the {label} behind")));
        }
    }
    let funding_source_after = rpc
        .required_account(
            controller_funding_source,
            "controller funding refund source",
        )?
        .lamports;
    if funding_source_after
        != controller_funding_source_before
            .checked_add(controller_native_refund)
            .ok_or_else(|| Error::new("controller funding native refund overflow"))?
    {
        return Err(Error::new(
            "the abort did not return exact controller native principal to its original source",
        ));
    }
    // And their rent went where it was always going, exactly.
    let credit_after = rpc
        .required_account(coordinates.credit, "abort lifecycle credit")?
        .lamports;
    let expected = credit_before
        .checked_add(coordinates.lock.funding_source_vault_rent_lamports)
        .and_then(|value| value.checked_add(coordinates.lock.funding_source_state_rent_lamports))
        .and_then(|value| value.checked_add(coordinates.lock.vault_rent_lamports))
        .and_then(|value| value.checked_add(coordinates.lock.state_rent_lamports))
        .and_then(|value| value.checked_add(controller_rent_refund))
        .ok_or_else(|| Error::new("abort rent arithmetic overflow"))?;
    if credit_after != expected {
        return Err(Error::new(format!(
            "the abort credited {credit_after} to the lifecycle credit, not the exact {expected} its four closures owe"
        )));
    }
    Ok(())
}

/// Reacquire and check the exact prestate the founding outer will consume.
#[allow(clippy::too_many_arguments)]
fn authenticate_bootstrap_poststate(
    rpc: &mut Rpc,
    coordinates: &FoundingCoordinates,
    custody: Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
    principal: u64,
) -> Result<()> {
    let replay = rpc.required_account(coordinates.projected_replay, "projected Custody replay")?;
    let state = ProjectedCustodyStateV2::decode(&replay.data)
        .map_err(|error| Error::new(format!("projected Custody state: {error:?}")))?;
    if replay.owner != custody
        || replay.data.len() != PROJECTED_CUSTODY_STATE_BYTES_V2
        || state.phase != ProjectedCustodyPhaseV1::SourceFunded
        || state.next_revision != OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1
        || state.locked_amount != principal
    {
        return Err(Error::new(
            "DCLTPCB2 poststate did not rest at the funded source compartment",
        ));
    }
    let hoard = rpc.required_account(coordinates.hoard_vault, "founding Hoard vault")?;
    let hoard_state = TokenAccount::parse(&hoard.data)
        .map_err(|error| Error::new(format!("Hoard vault: {error:?}")))?;
    if hoard.owner != token_program
        || hoard_state.mint != mint.to_bytes()
        || hoard_state.amount != 0
        || hoard_state.state != AccountState::Initialized
    {
        return Err(Error::new(
            "founding Hoard vault poststate differed from its checked plan",
        ));
    }
    let source = rpc.required_account(coordinates.source_vault, "founding source vault")?;
    let source_state = TokenAccount::parse(&source.data)
        .map_err(|error| Error::new(format!("source vault: {error:?}")))?;
    if source.owner != token_program
        || source_state.mint != mint.to_bytes()
        || source_state.amount != principal
        || source_state.state != AccountState::Initialized
    {
        return Err(Error::new(
            "founding source vault did not hold exactly the founding principal",
        ));
    }
    let source_replay =
        rpc.required_account(coordinates.source_replay, "founding source replay")?;
    if source_replay.owner != custody || source_replay.data.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(Error::new(
            "founding source replay poststate differed from its checked plan",
        ));
    }
    for (index, ledger) in coordinates.funding_ledgers.iter().enumerate() {
        let account = rpc.required_account(ledger.address, "founding FundingLedgerV2")?;
        let funding = FundingLedgerV2::decode(&account.data)
            .map_err(|error| Error::new(format!("FundingLedgerV2 {index}: {error:?}")))?;
        if account.owner != ledger.controller
            || account.data != ledger.bytes
            || account.lamports != ledger.required_lamports
            || funding.selected_mask() != ledger.selected_mask
        {
            return Err(Error::new(format!(
                "FundingLedgerV2 {index} poststate differed from its checked plan"
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// DCLTGMF3 - the compact atomic generic Market founding.
//
// Lock -> Found/permit -> Realize -> Claims FoundingV5 -> Core Open, five
// stages in one rollback domain. The outer carries an eight-byte discriminator
// followed by five canonical caller bumps; every economic byte travels in four
// readonly content-addressed request records, so a substituted request is a
// substituted address. The bumps are invocation evidence, not semantic truth.
//
// Nothing below is a runner choice. The Lock and Realize receipts the founding
// commits to are produced by running the Custody kernel's own transitions over
// the exact prestate bytes DCLTPCB2 left on chain; the permit intent and the
// Claims request are the same values Core will rebuild inside the Found stage,
// assembled from the same authenticated coordinates in the same order.
// ---------------------------------------------------------------------------

/// Sole top-level generic Market founding instruction.
///
/// Owned by `programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs`
/// (`GENERIC_MARKET_FOUNDING_MAGIC_V3`); restated here because a localhost host
/// utility does not depend on an SBF program crate.
const GENERIC_MARKET_FOUNDING_MAGIC_V3: [u8; 8] = *b"DCLTGMF3";
const GENERIC_MARKET_FOUNDING_CALLER_BUMP_COUNT_V3: usize = 5;
const GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3: usize =
    8 + GENERIC_MARKET_FOUNDING_CALLER_BUMP_COUNT_V3;

/// Exact `DCLTGMF3` frame width before the founding's FundingLedgerV2 tail.
///
/// Four readonly requests, the instructions sysvar the heap-frame admission
/// reads back, then Lock (14), Found (26 fixed + tail + 15 suffix), Realize
/// (12), Claims (31), Open (21), and the durable funding checkpoint. The total is
/// `GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 + physical_funding_count`.
const GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3: usize = 125;
const GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3: usize = 2;
const GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3: usize = 12;
const GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3: usize = 58;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GenericMarketFoundingLockExpectationV3 {
    frame_digest: [u8; 32],
}

#[derive(Clone, Debug)]
struct PreparedGenericMarketFoundingV3 {
    instruction: Instruction,
    lock_expectation: GenericMarketFoundingLockExpectationV3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompleteLockCensusV1 {
    complete_keys: usize,
    required_signatures: usize,
    static_keys: usize,
    loaded_writable: usize,
    loaded_readonly: usize,
    key_privilege_digest: [u8; 32],
}

fn exact_instruction_frame_digest_v1(instruction: &Instruction) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/successor/exact-instruction-frame/v1");
    hasher.update(instruction.program_id.as_ref());
    hasher.update(
        u64::try_from(instruction.accounts.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for meta in &instruction.accounts {
        hasher.update(meta.pubkey.as_ref());
        hasher.update([u8::from(meta.is_signer), u8::from(meta.is_writable)]);
    }
    hasher.update(
        u64::try_from(instruction.data.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(&instruction.data);
    hasher.finalize().into()
}

fn canonical_routing_addresses_v1(payer: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    let push = |key: Pubkey, addresses: &mut Vec<Pubkey>| {
        if key != payer && !addresses.contains(&key) {
            addresses.push(key);
        }
    };
    for instruction in instructions {
        push(instruction.program_id, &mut addresses);
        for meta in &instruction.accounts {
            if !meta.is_signer {
                push(meta.pubkey, &mut addresses);
            }
        }
    }
    addresses
}

fn compiled_complete_lock_census_v1(
    payer: Pubkey,
    instruction: &Instruction,
) -> Result<CompleteLockCensusV1> {
    let addresses = canonical_routing_addresses_v1(payer, std::slice::from_ref(instruction));
    let routing = build_lookup_table_creation_v1(payer, payer, 1, &addresses)
        .map_err(|error| Error::new(format!("DCLTGMF3 census table: {error:?}")))?;
    let bounded = bounded_instructions(std::slice::from_ref(instruction), Some(256_u32 * 1024))?;
    let table = AddressLookupTableAccount {
        key: routing.lookup_table,
        addresses: routing.addresses,
    };
    let message = v0::Message::try_compile(
        &payer,
        &bounded,
        std::slice::from_ref(&table),
        Hash::new_from_array([0x43; 32]),
    )
    .map_err(|error| Error::new(format!("DCLTGMF3 census compile: {error}")))?;

    let required_signatures = usize::from(message.header.num_required_signatures);
    let readonly_signed = usize::from(message.header.num_readonly_signed_accounts);
    let readonly_unsigned = usize::from(message.header.num_readonly_unsigned_accounts);
    let static_keys = message.account_keys.len();
    let writable_signed = required_signatures
        .checked_sub(readonly_signed)
        .ok_or_else(|| Error::new("DCLTGMF3 signed privilege census underflow"))?;
    let writable_unsigned_end = static_keys
        .checked_sub(readonly_unsigned)
        .ok_or_else(|| Error::new("DCLTGMF3 unsigned privilege census underflow"))?;
    let mut resolved = Vec::new();
    for (index, key) in message.account_keys.iter().copied().enumerate() {
        let signer = index < required_signatures;
        let writable = if signer {
            index < writable_signed
        } else {
            index < writable_unsigned_end
        };
        resolved.push((key, signer, writable, 0_u8));
    }
    let mut loaded_writable = 0_usize;
    let mut loaded_readonly = 0_usize;
    for lookup in &message.address_table_lookups {
        if lookup.account_key != table.key {
            return Err(Error::new(
                "DCLTGMF3 census selected an unknown lookup table",
            ));
        }
        for index in &lookup.writable_indexes {
            let key = table
                .addresses
                .get(usize::from(*index))
                .copied()
                .ok_or_else(|| Error::new("DCLTGMF3 writable lookup index was out of range"))?;
            resolved.push((key, false, true, 1_u8));
            loaded_writable = loaded_writable
                .checked_add(1)
                .ok_or_else(|| Error::new("DCLTGMF3 writable census overflow"))?;
        }
        for index in &lookup.readonly_indexes {
            let key = table
                .addresses
                .get(usize::from(*index))
                .copied()
                .ok_or_else(|| Error::new("DCLTGMF3 readonly lookup index was out of range"))?;
            resolved.push((key, false, false, 2_u8));
            loaded_readonly = loaded_readonly
                .checked_add(1)
                .ok_or_else(|| Error::new("DCLTGMF3 readonly census overflow"))?;
        }
    }

    let mut expected = BTreeMap::<Pubkey, (bool, bool)>::new();
    expected.insert(payer, (true, true));
    for bounded_instruction in &bounded {
        expected
            .entry(bounded_instruction.program_id)
            .or_insert((false, false));
        for meta in &bounded_instruction.accounts {
            expected
                .entry(meta.pubkey)
                .and_modify(|privilege| {
                    privilege.0 |= meta.is_signer;
                    privilege.1 |= meta.is_writable;
                })
                .or_insert((meta.is_signer, meta.is_writable));
        }
    }
    let mut unique = BTreeMap::new();
    for (key, signer, writable, class) in &resolved {
        if unique.insert(*key, (*signer, *writable, *class)).is_some() {
            return Err(Error::new(
                "DCLTGMF3 compiled complete-key list contained a duplicate",
            ));
        }
        if expected.get(key).copied() != Some((*signer, *writable)) {
            return Err(Error::new(
                "DCLTGMF3 compiled key privilege differed from the complete instruction union",
            ));
        }
    }
    if unique.len() != expected.len() {
        return Err(Error::new(
            "DCLTGMF3 compiled complete-key list omitted an instruction key",
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/successor/dcltgmf3-complete-lock-census/v1");
    hasher.update(
        u64::try_from(resolved.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (key, signer, writable, class) in &resolved {
        hasher.update(key.as_ref());
        hasher.update([u8::from(*signer), u8::from(*writable), *class]);
    }
    Ok(CompleteLockCensusV1 {
        complete_keys: resolved.len(),
        required_signatures,
        static_keys,
        loaded_writable,
        loaded_readonly,
        key_privilege_digest: hasher.finalize().into(),
    })
}

fn require_devnet_complete_key_limit_v1(census: CompleteLockCensusV1) -> Result<()> {
    if census.complete_keys > DEVNET_ACCOUNT_LOCK_LIMIT_V1 {
        return Err(Error::new(format!(
            "compiled transaction locks {} complete keys, above devnet's {}-key limit",
            census.complete_keys, DEVNET_ACCOUNT_LOCK_LIMIT_V1
        )));
    }
    Ok(())
}

fn authenticate_generic_market_founding_lock_census_v3(
    payer: Pubkey,
    prepared: &PreparedGenericMarketFoundingV3,
) -> Result<CompleteLockCensusV1> {
    let instruction = &prepared.instruction;
    let expected_frame = GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3
        .checked_add(GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3)
        .ok_or_else(|| Error::new("DCLTGMF3 expected frame overflow"))?;
    if instruction.data.len() != GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3
        || instruction.data.get(..8) != Some(GENERIC_MARKET_FOUNDING_MAGIC_V3.as_slice())
        || instruction.accounts.len() != expected_frame
        || instruction.accounts.iter().any(|meta| meta.is_signer)
        || instruction.accounts.iter().any(|meta| meta.pubkey == payer)
        || exact_instruction_frame_digest_v1(instruction) != prepared.lock_expectation.frame_digest
    {
        return Err(Error::new(
            "DCLTGMF3 exact frame/key/order/privilege expectation changed before compilation",
        ));
    }
    let census = compiled_complete_lock_census_v1(payer, instruction)?;
    require_devnet_complete_key_limit_v1(census)?;
    if census.complete_keys != GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3
        || census.required_signatures != 1
        || census.static_keys != 3
        || census.loaded_writable != GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3
        || census.loaded_readonly != 43
    {
        return Err(Error::new(format!(
            "DCLTGMF3 compiled lock census refused: {} complete, {} static, {} writable loaded, {} readonly loaded, {} signatures",
            census.complete_keys,
            census.static_keys,
            census.loaded_writable,
            census.loaded_readonly,
            census.required_signatures,
        )));
    }
    Ok(census)
}

/// Exact normal replay revision the founding's Realize stage commits.
///
/// Core pins this rather than accepting it: `build_permit_plan` writes
/// `normal_replay_revision: 1` into the intent, and Claims requires
/// `post_custody_revision` to equal it.
const FOUNDING_NORMAL_REPLAY_REVISION_V1: u64 = 1;

#[derive(Clone, Copy)]
struct FoundingPoststateExpectationV1 {
    permit: Pubkey,
    permit_bump: u8,
    aggregate: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    aggregate_width: usize,
    position_width: usize,
    principal: u64,
}

fn derive_founding_poststate_expectation_v1(
    plan: &SuccessorPlan,
    coordinates: &FoundingCoordinates,
    founder: Pubkey,
    claim_count: u32,
) -> Result<FoundingPoststateExpectationV1> {
    let core = pubkey(&plan.core.program_id)?;
    let claims_program = pubkey(&plan.claims.program_id)?;
    let permit_seeds = SeriesFoundingPermitSeedsV1::new(
        coordinates.found.release_set(),
        coordinates.found.market(),
        coordinates.found.context(),
    );
    let (permit, permit_bump) = Pubkey::find_program_address(&permit_seeds.as_slices(), &core);
    let aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(coordinates.market.to_bytes())
            .map_err(|error| Error::new(format!("Claims aggregate seeds: {error:?}")))?
            .as_slices(),
        &claims_program,
    )
    .0;
    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), founder.to_bytes())
            .map_err(|error| Error::new(format!("Claims position seeds: {error:?}")))?
            .as_slices(),
        &claims_program,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), founder.to_bytes())
            .map_err(|error| Error::new(format!("Claims admission seeds: {error:?}")))?
            .as_slices(),
        &claims_program,
    )
    .0;
    let aggregate_width =
        liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, claim_count)
            .map_err(|error| Error::new(format!("aggregate width: {error:?}")))?;
    let position_width =
        liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, claim_count)
            .map_err(|error| Error::new(format!("position width: {error:?}")))?;
    Ok(FoundingPoststateExpectationV1 {
        permit,
        permit_bump,
        aggregate,
        position,
        admission,
        aggregate_width,
        position_width,
        principal: coordinates.lock.amount,
    })
}

/// Every coordinate the founding outer determines beyond its prestate.
struct FoundingOuterV1 {
    found_raw: Vec<u8>,
    lock_raw: Vec<u8>,
    realize_raw: Vec<u8>,
    claims_raw: Vec<u8>,
    substituted_claims_raw: Vec<u8>,
    lock_caller: Pubkey,
    lock_caller_bump: u8,
    realize_caller: Pubkey,
    realize_caller_bump: u8,
    claims_caller: Pubkey,
    claims_caller_bump: u8,
    found_authority: Pubkey,
    found_authority_bump: u8,
    open_authority: Pubkey,
    open_authority_bump: u8,
    permit: Pubkey,
    aggregate: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    aggregate_width: usize,
    position_width: usize,
    market_rent: u64,
    permit_rent: u64,
}

/// Derive the founding outer's four requests and every PDA it signs through.
///
/// The order is forced and acyclic, and each step consumes only values the
/// previous ones produced:
///
/// 1. the Lock receipt, from the Custody kernel applied to the chain's own
///    `SourceFunded` projection and its normal source replay;
/// 2. the Realize request, which is the Lock request with exactly its operation
///    and its two revisions moved - the same derivation the outer's
///    `authenticate_projected_sequence` evaluates;
/// 3. the Realize receipt, which needs the candidate Core state the Found stage
///    will write, so that state is built here and cross-checked below;
/// 4. the permit intent, whose digest is a caller-PDA seed input for Claims;
/// 5. the Claims request, which carries that digest.
#[allow(clippy::too_many_arguments)]
fn derive_founding_outer_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    records: &MarketRecords,
    coordinates: &FoundingCoordinates,
    product: ProductContentId,
    founder: Pubkey,
    substituted_founder: Pubkey,
    claim_count: u32,
) -> Result<FoundingOuterV1> {
    let core = pubkey(&plan.core.program_id)?;
    let claims_program = pubkey(&plan.claims.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let release_set = hex32(&plan.release_set_id)?;
    let principal = coordinates.lock.amount;

    // The prestate is read back from the chain rather than modelled. These are
    // the exact bytes DCLTPCB2 left, and the kernel transitions below are the
    // ones Custody itself will run over them.
    let replay = rpc.required_account(coordinates.projected_replay, "projected Custody replay")?;
    let projection = ProjectedCustodyStateV2::decode(&replay.data)
        .map_err(|error| Error::new(format!("projected Custody state: {error:?}")))?;
    let source_replay_account =
        rpc.required_account(coordinates.source_replay, "founding source replay")?;
    let source_replay = CustodyReplayV1::decode(&source_replay_account.data)
        .map_err(|error| Error::new(format!("founding source replay: {error:?}")))?;

    let found_raw = coordinates
        .found
        .encode()
        .map_err(|error| Error::new(format!("founding artifact encoding: {error:?}")))?;
    let lock_raw = coordinates
        .lock
        .encode()
        .map_err(|error| Error::new(format!("terminal Lock encoding: {error:?}")))?;
    let lock_digest: [u8; 32] = Sha256::digest(lock_raw).into();

    let (locked, lock_receipt) = projection
        .lock_hoard_and_close_source(
            coordinates.lock,
            lock_digest,
            coordinates.source_replay.to_bytes(),
            source_replay,
            principal,
            0,
            0,
            principal,
            coordinates.lock.funding_source_vault_rent_lamports,
            coordinates.lock.funding_source_state_rent_lamports,
            coordinates.lock.rent_credit,
            true,
        )
        .map_err(|error| Error::new(format!("projected Lock transition: {error:?}")))?;
    let lock_receipt_bytes = lock_receipt
        .encode()
        .map_err(|error| Error::new(format!("Lock receipt encoding: {error:?}")))?;
    let lock_receipt_digest: [u8; 32] = Sha256::digest(lock_receipt_bytes).into();

    // Exactly the sequence the outer re-derives before its Realize CPI: the
    // terminal Lock request with its operation and its two revisions moved.
    let mut realize = coordinates.lock;
    realize.operation = ProjectedCustodyOperationV1::RealizeAndClose;
    realize.expected_revision = coordinates.lock.resulting_revision;
    realize.resulting_revision = coordinates
        .lock
        .resulting_revision
        .checked_add(1)
        .ok_or_else(|| Error::new("Realize revision overflow"))?;
    let realize_raw = realize
        .encode()
        .map_err(|error| Error::new(format!("Realize request encoding: {error:?}")))?;
    let realize_digest: [u8; 32] = Sha256::digest(realize_raw).into();

    // The candidate Core state the Found stage writes. Every field of it is
    // fixed by the kernel's `found`: the phase and readiness are constants, the
    // identity is the one this campaign already derived the Market address
    // from, and the rent beneficiary is the founding generation's credit. It is
    // cross-checked against the chain in `authenticate_core_state_encoding_v1`
    // before anything commits to its digest.
    let market_state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Prepaid,
        terminal_winner: 0,
        identity: coordinates.identity,
        outstanding_capabilities: 0,
        principal_cap_sets: coordinates.principal_cap_sets,
        rent_beneficiary: identity_of(coordinates.credit.to_bytes())?,
        terminal_receipt: None,
    };
    let market_state_bytes = market_state
        .encode()
        .map_err(|error| Error::new(format!("candidate Core state: {error:?}")))?;
    let market_state_digest: [u8; 32] = Sha256::digest(market_state_bytes).into();

    let realize_receipt = locked
        .realize_and_close_ref(
            &realize,
            realize_digest,
            &market_state,
            market_state_digest,
            principal,
            coordinates.lock.rent_credit,
        )
        .map_err(|error| Error::new(format!("projected Realize transition: {error:?}")))?;
    if realize_receipt.resulting_revision != coordinates.found.projected_resulting_revision() {
        return Err(Error::new(
            "derived Realize receipt did not reach the artifact's projected revision",
        ));
    }
    let realize_receipt_bytes = realize_receipt
        .encode()
        .map_err(|error| Error::new(format!("Realize receipt encoding: {error:?}")))?;
    let realize_receipt_digest: [u8; 32] = Sha256::digest(realize_receipt_bytes).into();

    // One semantic owner for the final coordinates. Completed-crash recovery
    // calls the same helper against finalized Open state; it never tries to
    // replay the SourceFunded kernel state that DCLTGMF3 consumed and closed.
    let poststate =
        derive_founding_poststate_expectation_v1(plan, coordinates, founder, claim_count)?;
    let permit = poststate.permit;
    let aggregate = poststate.aggregate;
    let position = poststate.position;
    let admission = poststate.admission;
    let aggregate_width = poststate.aggregate_width;
    let position_width = poststate.position_width;
    let aggregate_rent = rpc.minimum_balance(aggregate_width)?;
    let position_rent = rpc.minimum_balance(position_width)?;
    let admission_rent = rpc.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)?;

    let intent = FoundingIntentV5::new(
        poststate.permit_bump,
        coordinates.found.release_set(),
        coordinates.found.market(),
        identity_of(records.product.digest)?,
        identity_of(records.source.digest)?,
        coordinates.found.founder(),
        coordinates.found.context(),
        coordinates.found.capability_root(),
        coordinates.found.projected_replay(),
        coordinates.found.funding_source(),
        coordinates.found.hoard(),
        identity_of(realize_digest)?,
        identity_of(realize_receipt_digest)?,
        identity_of(trading.to_bytes())?,
        identity_of(claims_program.to_bytes())?,
        identity_of(coordinates.credit.to_bytes())?,
        coordinates.found.generation(),
        coordinates.found.quantity(),
        coordinates.found.basis_scale(),
        coordinates.found.expiry_slot(),
        realize_receipt.resulting_revision,
        FOUNDING_NORMAL_REPLAY_REVISION_V1,
    )
    .map_err(|error| Error::new(format!("founding permit intent: {error:?}")))?;
    let intent_bytes = intent
        .encode()
        .map_err(|error| Error::new(format!("founding intent encoding: {error:?}")))?;
    let intent_digest: [u8; 32] = Sha256::digest(intent_bytes).into();

    // Exactly the request Core compiles inside the Found stage and commits to
    // in the permit. Every observed lamport figure below is what the runner
    // will have transferred, and Core reads the same accounts to rebuild it: a
    // pre-funding that differs by one lamport moves the permit's digest and the
    // founding refuses at Claims.
    let claims_input = ClaimsFoundingRequestInputV5 {
        release_set,
        market: coordinates.market.to_bytes(),
        product_record_digest: records.product.digest,
        product_instance_id: product.to_bytes(),
        linked_basis_record_digest: records.basis.digest,
        semantic_basis_id: product_id(&input.liability_basis_id)?.to_bytes(),
        founder: founder.to_bytes(),
        founding_intent_digest: intent_digest,
        aggregate: aggregate.to_bytes(),
        position: position.to_bytes(),
        admission: admission.to_bytes(),
        hoard: coordinates.hoard_vault.to_bytes(),
        rent_credit: coordinates.credit.to_bytes(),
        rent_program: rent_program.to_bytes(),
        claims_program: claims_program.to_bytes(),
        trading_program: trading.to_bytes(),
        funding_source: coordinates.source_vault.to_bytes(),
        custody_replay: coordinates.projected_replay.to_bytes(),
        custody_request_digest: lock_digest,
        custody_receipt_digest: lock_receipt_digest,
        generation: coordinates.generation,
        claim_count,
        quantity: coordinates.found.quantity(),
        basis_scale: coordinates.found.basis_scale(),
        pre_source_amount: principal,
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: principal,
        pre_custody_revision: 0,
        post_custody_revision: FOUNDING_NORMAL_REPLAY_REVISION_V1,
        aggregate_rent_principal: aggregate_rent,
        position_rent_principal: position_rent,
        admission_rent_principal: admission_rent,
        observed_aggregate_lamports: aggregate_rent,
        observed_position_lamports: position_rent,
        observed_admission_lamports: admission_rent,
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    };
    let claims_raw = ClaimsFoundingRequestV5::new(claims_input)
        .map_err(|error| Error::new(format!("Claims founding request: {error:?}")))?
        .to_bytes()
        .to_vec();

    // The hostile request differs in exactly one coordinate: the founder whose
    // Position the founding mints. Everything else, including the permit intent
    // digest it carries, is the honest founding's. The outer's cross-request
    // join is the only thing between a substituted Claims record and a Position
    // minted to somebody else.
    let substituted_claims_raw = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
        founder: substituted_founder.to_bytes(),
        ..claims_input
    })
    .map_err(|error| Error::new(format!("substituted Claims request: {error:?}")))?
    .to_bytes()
    .to_vec();

    let claims_digest: [u8; 32] = Sha256::digest(&claims_raw).into();
    let (claims_caller, claims_caller_bump) = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            release_set,
            coordinates.market.to_bytes(),
            ExecutionRoleV1::Trading,
            intent_digest,
            claims_digest,
        )
        .map_err(|error| Error::new(format!("Claims caller seeds: {error:?}")))?
        .as_slices(),
        &trading,
    );

    let (lock_caller, lock_caller_bump) = Pubkey::find_program_address(
        &ProjectedCustodyCallerSeedsV1::new(coordinates.lock, lock_digest).as_slices(),
        &trading,
    );
    let (realize_caller, realize_caller_bump) = Pubkey::find_program_address(
        &ProjectedCustodyCallerSeedsV1::new(realize, realize_digest).as_slices(),
        &trading,
    );

    let selected = authenticate_generic_market_founding_artifact_v1(
        CapabilityContentId::new(Sha256::digest(found_raw).into())
            .map_err(|error| Error::new(format!("founding artifact identity: {error:?}")))?,
        &found_raw,
    )
    .map_err(|error| Error::new(format!("founding artifact: {error:?}")))?;
    let stages = construct_generic_market_founding_plan_v1(selected, trading, core)
        .map_err(|error| Error::new(format!("founding stage plan: {error:?}")))?;
    if stages.permit != permit {
        return Err(Error::new(
            "the founding permit the operator derived is not the one the intent commits to",
        ));
    }

    Ok(FoundingOuterV1 {
        found_raw: found_raw.to_vec(),
        lock_raw: lock_raw.to_vec(),
        realize_raw: realize_raw.to_vec(),
        claims_raw,
        substituted_claims_raw,
        lock_caller,
        lock_caller_bump,
        realize_caller,
        realize_caller_bump,
        claims_caller,
        claims_caller_bump,
        found_authority: stages.found_authority,
        found_authority_bump: stages.found_authority_bump,
        open_authority: stages.open_authority,
        open_authority_bump: stages.open_authority_bump,
        permit,
        aggregate,
        position,
        admission,
        aggregate_width,
        position_width,
        market_rent: coordinates.found.market_rent(),
        permit_rent: coordinates.found.permit_rent(),
    })
}

/// Prove the candidate Core state is encoded exactly the way the chain writes.
///
/// The founding commits to `sha256(CoreState)` two stages before that state
/// exists, so an encoding that differed from Core's by one byte would produce a
/// permit the Claims stage refuses and a failure with no visible cause. This
/// re-encodes the Found37 Market's own decoded state and requires the result to
/// be the bytes the chain is holding: one independent derivation of a value the
/// validator produced, not a read-back.
fn authenticate_core_state_encoding_v1(rpc: &mut Rpc, market: Pubkey) -> Result<()> {
    let account = rpc.required_account(market, "Found37 Market")?;
    let state = CoreState::decode(&account.data)
        .map_err(|error| Error::new(format!("Found37 Market state: {error:?}")))?;
    let encoded = state
        .encode()
        .map_err(|error| Error::new(format!("Found37 Market re-encoding: {error:?}")))?;
    if encoded.as_slice() != account.data.as_slice() {
        return Err(Error::new(
            "CoreState re-encoding did not reproduce the Market bytes the chain holds",
        ));
    }
    Ok(())
}

/// Build the exact 125 + physical-funding-count account `DCLTGMF3` frame.
///
/// Privileges are the outer's own assertion. Exactly twelve distinct keys are
/// writable — the projected replay, the rent credit, the Hoard vault, the
/// source vault, the source replay, the Found caller PDA, the Market, the
/// permit, the three Claims accounts, and the controller-funding checkpoint —
/// and **no account in the frame is a transaction-level signer**: every stage's
/// signer is a PDA the outer signs for under `invoke_signed`, so the fee payer
/// must be a key that appears nowhere in this list.
///
/// Writability is unioned per key at the end rather than per slot. Solana
/// grants privileges per key, so an account that must be writable in one stage
/// is writable in every stage that names it; the outer downgrades it back to
/// readonly in each child's metas, which is where the child's own
/// non-writability requirements are enforced.
#[allow(clippy::too_many_arguments)]
fn build_generic_market_founding_v3(
    plan: &SuccessorPlan,
    coordinates: &FoundingCoordinates,
    outer: &FoundingOuterV1,
    records: &MarketRecords,
    requests: [Pubkey; 4],
    founder: Pubkey,
    mint: Pubkey,
) -> Result<PreparedGenericMarketFoundingV3> {
    let trading = pubkey(&plan.trading.program_id)?;
    let trading_programdata = pubkey(&plan.trading.programdata_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let claims_programdata = pubkey(&plan.claims.programdata_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let custody_programdata = pubkey(&plan.custody.programdata_id)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let cache = pubkey(&plan.activation)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    let mut accounts: Vec<AccountMeta> = Vec::with_capacity(
        GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3
            .checked_add(coordinates.funding_ledgers.len())
            .ok_or_else(|| Error::new("founding frame width overflow"))?,
    );
    let push = |key: Pubkey, writable: bool, accounts: &mut Vec<AccountMeta>| {
        accounts.push(if writable {
            AccountMeta::new(key, false)
        } else {
            AccountMeta::new_readonly(key, false)
        });
    };

    // Four readonly content-addressed requests, then the instructions sysvar
    // the heap-frame admission reads the transaction's grant back out of.
    for request in requests {
        push(request, false, &mut accounts);
    }
    push(sysvar::instructions::ID, false, &mut accounts);

    // Lock: LockHoardAndCloseSource, 14 accounts.
    push(outer.lock_caller, false, &mut accounts);
    push(coordinates.projected_replay, true, &mut accounts);
    push(cache, false, &mut accounts);
    push(registry, false, &mut accounts);
    push(trading, false, &mut accounts);
    push(trading_programdata, false, &mut accounts);
    push(coordinates.credit, true, &mut accounts);
    push(coordinates.hoard_vault, true, &mut accounts);
    push(coordinates.source_vault, true, &mut accounts);
    push(coordinates.custody_authority, false, &mut accounts);
    push(mint, false, &mut accounts);
    push(token_program, false, &mut accounts);
    push(coordinates.source_replay, true, &mut accounts);
    push(coordinates.market, false, &mut accounts);

    // Found: Core's compact 24-account ProjectedFound V2 frame, then Trading,
    // this founding's FundingLedgerV2 tail, and the
    // fifteen-account suffix.
    for (index, key) in projected_found_snapshot_keys_v2(
        plan,
        outer.found_authority,
        coordinates.market,
        coordinates.credit,
        records,
    )?
    .into_iter()
    .enumerate()
    {
        // Index 0 is the payer and the Trading caller PDA in one slot; index 1
        // is the Market the stage creates.
        push(key, index < 2, &mut accounts);
    }
    push(trading, false, &mut accounts);
    push(trading_programdata, false, &mut accounts);
    for funding in &coordinates.funding_ledgers {
        push(funding.address, false, &mut accounts);
    }
    push(outer.permit, true, &mut accounts);
    push(coordinates.projected_replay, true, &mut accounts);
    push(coordinates.hoard_vault, true, &mut accounts);
    push(coordinates.source_vault, true, &mut accounts);
    push(coordinates.source_replay, true, &mut accounts);
    push(records.basis.raw, false, &mut accounts);
    push(records.basis.staging, false, &mut accounts);
    push(claims, false, &mut accounts);
    push(claims_programdata, false, &mut accounts);
    push(custody, false, &mut accounts);
    push(custody_programdata, false, &mut accounts);
    push(outer.aggregate, true, &mut accounts);
    push(outer.position, true, &mut accounts);
    push(outer.admission, true, &mut accounts);
    push(founder, false, &mut accounts);

    // Realize: RealizeAndClose, 12 accounts.
    push(outer.realize_caller, false, &mut accounts);
    push(coordinates.projected_replay, true, &mut accounts);
    push(cache, false, &mut accounts);
    push(registry, false, &mut accounts);
    push(trading, false, &mut accounts);
    push(trading_programdata, false, &mut accounts);
    push(coordinates.credit, true, &mut accounts);
    push(coordinates.hoard_vault, true, &mut accounts);
    push(coordinates.market, false, &mut accounts);
    push(coordinates.custody_authority, false, &mut accounts);
    push(mint, false, &mut accounts);
    push(token_program, false, &mut accounts);

    // Claims FoundingV5, 31 accounts.
    push(outer.claims_caller, false, &mut accounts);
    push(outer.permit, true, &mut accounts);
    push(outer.aggregate, true, &mut accounts);
    push(outer.position, true, &mut accounts);
    push(outer.admission, true, &mut accounts);
    push(coordinates.source_vault, true, &mut accounts);
    push(coordinates.hoard_vault, true, &mut accounts);
    push(coordinates.projected_replay, true, &mut accounts);
    push(records.basis.raw, false, &mut accounts);
    push(records.basis.staging, false, &mut accounts);
    push(records.product.raw, false, &mut accounts);
    push(records.product.staging, false, &mut accounts);
    push(records.domain.raw, false, &mut accounts);
    push(records.domain.staging, false, &mut accounts);
    push(records.portfolio.raw, false, &mut accounts);
    push(records.portfolio.staging, false, &mut accounts);
    push(system_program::ID, false, &mut accounts);
    push(coordinates.market, false, &mut accounts);
    push(cache, false, &mut accounts);
    push(registry, false, &mut accounts);
    push(claims, false, &mut accounts);
    push(claims_programdata, false, &mut accounts);
    push(core, false, &mut accounts);
    push(core_programdata, false, &mut accounts);
    push(trading, false, &mut accounts);
    push(trading_programdata, false, &mut accounts);
    push(custody, false, &mut accounts);
    push(custody_programdata, false, &mut accounts);
    push(founder, false, &mut accounts);
    push(coordinates.credit, true, &mut accounts);
    push(rent_program, false, &mut accounts);

    // Core Open, commit-last, 21 accounts.
    push(outer.open_authority, false, &mut accounts);
    push(coordinates.market, true, &mut accounts);
    push(outer.permit, true, &mut accounts);
    push(coordinates.credit, true, &mut accounts);
    push(rent_program, false, &mut accounts);
    push(cache, false, &mut accounts);
    push(registry, false, &mut accounts);
    push(trading, false, &mut accounts);
    push(trading_programdata, false, &mut accounts);
    push(claims, false, &mut accounts);
    push(claims_programdata, false, &mut accounts);
    push(custody, false, &mut accounts);
    push(custody_programdata, false, &mut accounts);
    push(core, false, &mut accounts);
    push(core_programdata, false, &mut accounts);
    push(coordinates.projected_replay, true, &mut accounts);
    push(coordinates.hoard_vault, true, &mut accounts);
    push(coordinates.source_vault, true, &mut accounts);
    push(outer.aggregate, true, &mut accounts);
    push(outer.position, true, &mut accounts);
    push(outer.admission, true, &mut accounts);

    // The durable CustodyStaged checkpoint is authenticated before Lock and
    // consumed only after the exact Open acknowledgement and unchanged
    // Pending controller ledgers. It is therefore the final physical account
    // in the outer frame and writable solely for its close-last transition.
    push(
        coordinates.controller_funding_checkpoint,
        true,
        &mut accounts,
    );

    let expected = GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3
        .checked_add(coordinates.funding_ledgers.len())
        .ok_or_else(|| Error::new("founding frame width overflow"))?;
    if accounts.len() != expected {
        return Err(Error::new(format!(
            "assembled founding frame did not match its exact width: assembled {}, expected {} ({} fixed + {} funding ledgers)",
            accounts.len(),
            expected,
            GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3,
            coordinates.funding_ledgers.len(),
        )));
    }

    // One key, one privilege: union writability so a key writable in any stage
    // is writable everywhere the message names it.
    let writable: Vec<Pubkey> = accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect();
    for meta in &mut accounts {
        if writable.contains(&meta.pubkey) {
            meta.is_writable = true;
        }
    }
    let mut distinct: Vec<Pubkey> = Vec::new();
    for meta in &accounts {
        if !distinct.contains(&meta.pubkey) {
            distinct.push(meta.pubkey);
        }
    }
    let mut distinct_writable: Vec<Pubkey> = Vec::new();
    for key in &writable {
        if !distinct_writable.contains(key) {
            distinct_writable.push(*key);
        }
    }
    // Transaction admission is checked from the compiled bounded-v0 message,
    // after payer, ComputeBudget, and canonical ALT placement are known. A
    // frame-only `distinct + 2` estimate is not an admission proof.
    if distinct_writable.len() != 12 {
        return Err(Error::new(format!(
            "the founding frame declared {} writable keys, not the twelve the outer requires",
            distinct_writable.len()
        )));
    }
    let mut data = Vec::with_capacity(GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3);
    data.extend_from_slice(&GENERIC_MARKET_FOUNDING_MAGIC_V3);
    data.extend_from_slice(&[
        outer.lock_caller_bump,
        outer.found_authority_bump,
        outer.realize_caller_bump,
        outer.claims_caller_bump,
        outer.open_authority_bump,
    ]);
    if data.len() != GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3 {
        return Err(Error::new("DCLTGMF3 caller-bump encoding width changed"));
    }
    let instruction = Instruction {
        program_id: trading,
        accounts,
        data,
    };
    Ok(PreparedGenericMarketFoundingV3 {
        lock_expectation: GenericMarketFoundingLockExpectationV3 {
            frame_digest: exact_instruction_frame_digest_v1(&instruction),
        },
        instruction,
    })
}

fn funding_readiness_coordinates_v1(
    plan: &SuccessorPlan,
    records: &MarketRecords,
    coordinates: &FoundingCoordinates,
) -> Result<FundingReadinessCoordinatesV1> {
    let resolution = pubkey(&plan.resolution.program_id)?;
    let funding_ledger = coordinates
        .funding_ledgers
        .iter()
        .find(|ledger| ledger.controller == resolution)
        .ok_or_else(|| Error::new("founding omitted the Resolution FundingLedgerV2"))?
        .address;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            coordinates.market.as_ref(),
            &coordinates.generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let activation_receipt = Pubkey::find_program_address(
        &[
            FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            coordinates.market.as_ref(),
            &coordinates.generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    Ok(FundingReadinessCoordinatesV1 {
        market: coordinates.market,
        source_material: FundingReadinessRecordCoordinatesV1 {
            raw: records.source.raw,
            staging: records.source.staging,
        },
        capability_manifest: FundingReadinessRecordCoordinatesV1 {
            raw: records.manifest.raw,
            staging: records.manifest.staging,
        },
        recovery_policy: records
            .recovery
            .map(|record| FundingReadinessRecordCoordinatesV1 {
                raw: record.raw,
                staging: record.staging,
            }),
        source_state,
        funding_ledger,
        beneficiary: coordinates.credit,
        activation_receipt,
    })
}

fn funding_readiness_compiled_geometry_v1(
    payer: Pubkey,
    instructions: &[Instruction],
    routing_tables: &[ObservedAccount],
) -> Result<CompiledMessageGeometryV1> {
    let bounded = bounded_instructions(instructions, None)?;
    let lookup_tables = routing_tables
        .iter()
        .map(|account| {
            let table = AddressLookupTable::deserialize(&account.data).map_err(|error| {
                Error::new(format!("funding-readiness routing table: {error:?}"))
            })?;
            Ok(AddressLookupTableAccount {
                key: account.key,
                addresses: table.addresses.to_vec(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let message = v0::Message::try_compile(
        &payer,
        &bounded,
        &lookup_tables,
        Hash::new_from_array([0x73; 32]),
    )
    .map_err(|error| Error::new(format!("funding-readiness v0 compile: {error}")))?;
    let static_keys = message.account_keys.len();
    let loaded_writable = message
        .address_table_lookups
        .iter()
        .map(|lookup| lookup.writable_indexes.len())
        .sum::<usize>();
    let loaded_readonly = message
        .address_table_lookups
        .iter()
        .map(|lookup| lookup.readonly_indexes.len())
        .sum::<usize>();
    let complete_keys = static_keys
        .checked_add(loaded_writable)
        .and_then(|value| value.checked_add(loaded_readonly))
        .ok_or_else(|| Error::new("funding-readiness complete-key census overflow"))?;
    let required_signatures = usize::from(message.header.num_required_signatures);
    let versioned_message = VersionedMessage::V0(message);
    let message_bytes = versioned_message.serialize().len();
    let packet_bytes = bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default(); required_signatures],
        message: versioned_message,
    })
    .map_err(|error| Error::new(format!("funding-readiness packet geometry: {error}")))?
    .len();
    Ok(CompiledMessageGeometryV1 {
        complete_keys,
        required_signatures,
        static_keys,
        loaded_writable,
        loaded_readonly,
        message_bytes,
        packet_bytes,
    })
}

fn append_distinct_funding_readiness_accounts_v1(
    instructions: &[Instruction],
    count: usize,
) -> Result<Vec<Instruction>> {
    let mut expanded = instructions.to_vec();
    let target = expanded
        .last_mut()
        .ok_or_else(|| Error::new("funding-readiness instruction set was empty"))?;
    let original = target.accounts.len();
    let mut counter = 0_u64;
    while target.accounts.len() < original.saturating_add(count) {
        let mut hasher = Sha256::new();
        hasher.update(b"dclutch/census/funding-readiness/distinct-key-v1");
        hasher.update(counter.to_le_bytes());
        let candidate = Pubkey::new_from_array(hasher.finalize().into());
        counter = counter
            .checked_add(1)
            .ok_or_else(|| Error::new("funding-readiness census counter overflow"))?;
        if candidate != target.program_id
            && !target.accounts.iter().any(|meta| meta.pubkey == candidate)
        {
            target
                .accounts
                .push(AccountMeta::new_readonly(candidate, false));
        }
    }
    Ok(expanded)
}

fn authenticate_funding_readiness_compiled_geometry_v1(
    payer: Pubkey,
    operation: FoundingSubmissionOperationV1,
    instructions: &[Instruction],
    observation: Observation,
    routing_tables: &[ObservedAccount],
) -> Result<CompiledMessageGeometryV1> {
    let base = funding_readiness_compiled_geometry_v1(payer, instructions, routing_tables)?;
    let compiled = dclutch_versioned_message_operator::compile_v0_message(
        payer,
        &bounded_instructions(instructions, None)?,
        solana_hash::Hash::new_from_array([0x73; 32]),
        observation,
        routing_tables,
    )
    .map_err(|error| {
        Error::new(format!(
            "{} routed v0 compile: {error:?}",
            operation.label()
        ))
    })?;
    authenticate_resolved_founding_message_v1(operation, &compiled.message, routing_tables)?;
    let plus_one = funding_readiness_compiled_geometry_v1(
        payer,
        &append_distinct_funding_readiness_accounts_v1(instructions, 1)?,
        routing_tables,
    )?;
    let plus_two = funding_readiness_compiled_geometry_v1(
        payer,
        &append_distinct_funding_readiness_accounts_v1(instructions, 2)?,
        routing_tables,
    )?;
    let admitted_delta = DEVNET_ACCOUNT_LOCK_LIMIT_V1
        .checked_sub(base.complete_keys)
        .ok_or_else(|| Error::new("funding-readiness base exceeds the 64-key ceiling"))?;
    let admitted = funding_readiness_compiled_geometry_v1(
        payer,
        &append_distinct_funding_readiness_accounts_v1(instructions, admitted_delta)?,
        routing_tables,
    )?;
    let refused = funding_readiness_compiled_geometry_v1(
        payer,
        &append_distinct_funding_readiness_accounts_v1(
            instructions,
            admitted_delta.saturating_add(1),
        )?,
        routing_tables,
    )?;
    if base.complete_keys != operation.exact_unique_accounts()
        || base.required_signatures != operation.exact_required_signatures()
        || plus_one.complete_keys != base.complete_keys.saturating_add(1)
        || plus_two.complete_keys != base.complete_keys.saturating_add(2)
        || admitted.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1
        || refused.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1
        || compiled.wire_bytes != base.packet_bytes
        || usize::from(compiled.required_signatures) != base.required_signatures
        || base.packet_bytes > 1_232
    {
        return Err(Error::new(format!(
            "{} compiled geometry refused: base {}/{}, +1 {}, +2 {}, boundary {}/{}",
            operation.label(),
            base.complete_keys,
            base.packet_bytes,
            plus_one.complete_keys,
            plus_two.complete_keys,
            admitted.complete_keys,
            refused.complete_keys,
        )));
    }
    Ok(base)
}

fn funding_readiness_instructions_v1(
    payer: Pubkey,
    protocol: Instruction,
    prepay: Option<FundingReadinessPrepayV1>,
) -> Vec<Instruction> {
    let mut instructions = Vec::with_capacity(2);
    if let Some(prepay) = prepay.filter(|value| value.lamports != 0) {
        instructions.push(transfer(&payer, &prepay.destination, prepay.lamports));
    }
    instructions.push(protocol);
    instructions
}

fn authenticate_funding_readiness_route_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
    minimum_slot: u64,
    expected: &str,
) -> Result<()> {
    let observed = plan_funding_readiness_from_rpc_v1(rpc, plan, coordinates, minimum_slot)?;
    if observed.route_name() != expected {
        return Err(Error::new(format!(
            "funding-readiness completion selected {} instead of {expected}",
            observed.route_name()
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_one_funding_readiness_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
    minimum_slot: u64,
    operation: FoundingSubmissionOperationV1,
    label: &str,
    expected_next_route: &str,
    instruction: Instruction,
    observation: Observation,
    routing_tables: &[ObservedAccount],
    prepay: Option<FundingReadinessPrepayV1>,
    protocol_writable: Vec<Pubkey>,
    completion_accounts: Vec<Pubkey>,
    payer: &Keypair,
    submission_recorder: Option<&mut FoundingSubmissionRecorderV1<'_>>,
) -> Result<(TransactionEvidence, CompiledMessageGeometryV1)> {
    let instructions = funding_readiness_instructions_v1(payer.pubkey(), instruction, prepay);
    let geometry = authenticate_funding_readiness_compiled_geometry_v1(
        payer.pubkey(),
        operation,
        &instructions,
        observation,
        routing_tables,
    )?;
    let mut prestate_accounts = protocol_writable;
    if let Some(prepay) = prepay
        && !prestate_accounts.contains(&prepay.destination)
    {
        prestate_accounts.push(prepay.destination);
    }
    prestate_accounts.sort_unstable();
    prestate_accounts.dedup();
    let recovery_payload = serde_json::to_vec(&FundingReadinessRecoveryPayloadV1 {
        schema: FUNDING_READINESS_RECOVERY_PAYLOAD_SCHEMA_V1.into(),
        operation,
        market: coordinates.market.to_string(),
        source_state: coordinates.source_state.to_string(),
        funding_ledger: coordinates.funding_ledger.to_string(),
        beneficiary: coordinates.beneficiary.to_string(),
        activation_receipt: coordinates.activation_receipt.to_string(),
        expected_next_route: expected_next_route.into(),
    })?;
    let mut completion = |rpc: &mut Rpc| {
        authenticate_funding_readiness_route_v1(
            rpc,
            plan,
            coordinates,
            minimum_slot,
            expected_next_route,
        )
    };
    let evidence = match submission_recorder {
        Some(recorder) => send_durable_founding_v1(
            rpc,
            label,
            operation,
            &instructions,
            &[payer],
            observation,
            routing_tables,
            funding_readiness_instruction_digest_v1(payer.pubkey(), &instructions, routing_tables),
            &prestate_accounts,
            &completion_accounts,
            recovery_payload,
            None,
            recorder,
            &mut completion,
        )?,
        None => {
            let evidence = rpc.send_v0(label, &instructions, payer, observation, routing_tables)?;
            completion(rpc)?;
            evidence
        }
    };
    Ok((evidence, geometry))
}

fn push_transaction_once_v1(
    transactions: &mut Vec<TransactionEvidence>,
    evidence: TransactionEvidence,
) {
    if !transactions
        .iter()
        .any(|existing| existing.signature == evidence.signature)
    {
        transactions.push(evidence);
    }
}

fn recover_readiness_prefix_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
    minimum_slot: u64,
    expected_route: &str,
    label: &str,
    operation: FoundingSubmissionOperationV1,
    recorder: &mut FoundingSubmissionRecorderV1<'_>,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    if transactions.iter().any(|evidence| {
        recorder
            .current(operation)
            .and_then(|journal| journal.expected_signature.as_ref())
            == Some(&evidence.signature)
    }) {
        return Ok(());
    }
    let phase = recorder
        .current(operation)
        .ok_or_else(|| Error::new(format!("{} durable journal is absent", operation.label())))?
        .phase;
    let evidence = if phase == FoundingSubmissionPhaseV1::Finalized {
        authenticate_historical_founding_transaction_v1(rpc, label, operation, recorder)?
    } else {
        let mut completion = |rpc: &mut Rpc| {
            authenticate_funding_readiness_route_v1(
                rpc,
                plan,
                coordinates,
                minimum_slot,
                expected_route,
            )
        };
        finalize_existing_founding_submission_v1(rpc, label, operation, recorder, &mut completion)?
            .ok_or_else(|| {
                Error::new(format!(
                    "{} advanced chain state had no recoverable durable journal",
                    operation.label()
                ))
            })?
    };
    push_transaction_once_v1(transactions, evidence);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_funding_readiness_suffix_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    records: &MarketRecords,
    founding: &FoundingCoordinates,
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
    accounts: &mut BTreeMap<String, AccountEvidence>,
    completed: &mut Vec<String>,
    minimum_slot: u64,
    routing_table_keys: &[Pubkey],
    mut submission_recorder: Option<&mut FoundingSubmissionRecorderV1<'_>>,
) -> Result<()> {
    let coordinates = funding_readiness_coordinates_v1(plan, records, founding)?;

    let FundingReadinessRoutedPlanV1 {
        plan: current,
        routing_tables,
    } = plan_funding_readiness_with_routing_from_rpc_v1(
        rpc,
        plan,
        coordinates,
        minimum_slot,
        routing_table_keys,
    )?;
    match current {
        FundingReadinessPlanV1::Create(FundingReadinessInstructionPlanV1 {
            report,
            prepay,
            accounts: sets,
            ..
        }) => {
            let (evidence, geometry) = execute_one_funding_readiness_v1(
                rpc,
                plan,
                coordinates,
                minimum_slot,
                FoundingSubmissionOperationV1::CoreFundingCreateV1,
                "core-funding-create-v1",
                "activate",
                report.instruction,
                report.observation,
                &routing_tables,
                prepay,
                sets.protocol_writable,
                sets.completion,
                payer,
                submission_recorder.as_deref_mut(),
            )?;
            push_transaction_once_v1(transactions, evidence);
            completed.push(format!(
                "executed core-funding-create-v1: {} complete keys, {} signatures, {} message bytes, {} packet bytes; +1/+2 are {}/{} and +{} reaches 64 while +{} refuses at 65",
                geometry.complete_keys,
                geometry.required_signatures,
                geometry.message_bytes,
                geometry.packet_bytes,
                geometry.complete_keys + 1,
                geometry.complete_keys + 2,
                DEVNET_ACCOUNT_LOCK_LIMIT_V1 - geometry.complete_keys,
                DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1 - geometry.complete_keys,
            ));
        }
        FundingReadinessPlanV1::Activate(_)
        | FundingReadinessPlanV1::Accept(_)
        | FundingReadinessPlanV1::Complete(_) => {
            let recorder = submission_recorder.as_deref_mut().ok_or_else(|| {
                Error::new("CreateFund chain state was advanced without a durable public journal")
            })?;
            recover_readiness_prefix_v1(
                rpc,
                plan,
                coordinates,
                minimum_slot,
                "activate",
                "core-funding-create-v1",
                FoundingSubmissionOperationV1::CoreFundingCreateV1,
                recorder,
                transactions,
            )?;
        }
    }

    let FundingReadinessRoutedPlanV1 {
        plan: current,
        routing_tables,
    } = plan_funding_readiness_with_routing_from_rpc_v1(
        rpc,
        plan,
        coordinates,
        minimum_slot,
        routing_table_keys,
    )?;
    match current {
        FundingReadinessPlanV1::Activate(FundingReadinessInstructionPlanV1 {
            report,
            prepay,
            accounts: sets,
            ..
        }) => {
            let (evidence, geometry) = execute_one_funding_readiness_v1(
                rpc,
                plan,
                coordinates,
                minimum_slot,
                FoundingSubmissionOperationV1::ResolutionFundingActivateV1,
                "resolution-funding-activate-v1",
                "accept",
                report.instruction,
                report.observation,
                &routing_tables,
                prepay,
                sets.protocol_writable,
                sets.completion,
                payer,
                submission_recorder.as_deref_mut(),
            )?;
            push_transaction_once_v1(transactions, evidence);
            completed.push(format!(
                "executed resolution-funding-activate-v1: {} complete keys, {} signatures, {} message bytes, {} packet bytes; +1/+2 are {}/{} and +{} reaches 64 while +{} refuses at 65",
                geometry.complete_keys,
                geometry.required_signatures,
                geometry.message_bytes,
                geometry.packet_bytes,
                geometry.complete_keys + 1,
                geometry.complete_keys + 2,
                DEVNET_ACCOUNT_LOCK_LIMIT_V1 - geometry.complete_keys,
                DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1 - geometry.complete_keys,
            ));
        }
        FundingReadinessPlanV1::Accept(_) | FundingReadinessPlanV1::Complete(_) => {
            let recorder = submission_recorder.as_deref_mut().ok_or_else(|| {
                Error::new("ActivateFund chain state was advanced without a durable public journal")
            })?;
            recover_readiness_prefix_v1(
                rpc,
                plan,
                coordinates,
                minimum_slot,
                "accept",
                "resolution-funding-activate-v1",
                FoundingSubmissionOperationV1::ResolutionFundingActivateV1,
                recorder,
                transactions,
            )?;
        }
        FundingReadinessPlanV1::Create(_) => {
            return Err(Error::new(
                "CreateFund finalized without selecting the adjacent Activate route",
            ));
        }
    }

    let FundingReadinessRoutedPlanV1 {
        plan: current,
        routing_tables,
    } = plan_funding_readiness_with_routing_from_rpc_v1(
        rpc,
        plan,
        coordinates,
        minimum_slot,
        routing_table_keys,
    )?;
    match current {
        FundingReadinessPlanV1::Accept(FundingReadinessInstructionPlanV1 {
            report,
            prepay,
            accounts: sets,
            ..
        }) => {
            if submission_recorder.as_deref().is_some_and(|recorder| {
                recorder
                    .current(FoundingSubmissionOperationV1::CoreFundingAcceptV1)
                    .is_some_and(|journal| journal.phase == FoundingSubmissionPhaseV1::Finalized)
            }) {
                let recorder = submission_recorder
                    .as_deref_mut()
                    .ok_or_else(|| Error::new("Accept recovery recorder disappeared"))?;
                recover_readiness_prefix_v1(
                    rpc,
                    plan,
                    coordinates,
                    minimum_slot,
                    "accept",
                    "core-funding-accept-v1",
                    FoundingSubmissionOperationV1::CoreFundingAcceptV1,
                    recorder,
                    transactions,
                )?;
            } else {
                let (evidence, geometry) = execute_one_funding_readiness_v1(
                    rpc,
                    plan,
                    coordinates,
                    minimum_slot,
                    FoundingSubmissionOperationV1::CoreFundingAcceptV1,
                    "core-funding-accept-v1",
                    "accept",
                    report.instruction,
                    report.observation,
                    &routing_tables,
                    prepay,
                    sets.protocol_writable,
                    sets.completion,
                    payer,
                    submission_recorder.as_deref_mut(),
                )?;
                push_transaction_once_v1(transactions, evidence);
                completed.push(format!(
                    "executed core-funding-accept-v1: {} complete keys, {} signatures, {} message bytes, {} packet bytes; +1/+2 are {}/{} and +{} reaches 64 while +{} refuses at 65",
                    geometry.complete_keys,
                    geometry.required_signatures,
                    geometry.message_bytes,
                    geometry.packet_bytes,
                    geometry.complete_keys + 1,
                    geometry.complete_keys + 2,
                    DEVNET_ACCOUNT_LOCK_LIMIT_V1 - geometry.complete_keys,
                    DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1 - geometry.complete_keys,
                ));
            }
        }
        FundingReadinessPlanV1::Complete(_) => {
            let recorder = submission_recorder.as_deref_mut().ok_or_else(|| {
                Error::new("Ready funding state omitted the durable Accept journal")
            })?;
            recover_readiness_prefix_v1(
                rpc,
                plan,
                coordinates,
                minimum_slot,
                "complete",
                "core-funding-accept-v1",
                FoundingSubmissionOperationV1::CoreFundingAcceptV1,
                recorder,
                transactions,
            )?;
        }
        FundingReadinessPlanV1::Create(_) | FundingReadinessPlanV1::Activate(_) => {
            return Err(Error::new(
                "ActivateFund finalized without selecting the adjacent Accept route",
            ));
        }
    }

    authenticate_funding_readiness_route_v1(rpc, plan, coordinates, minimum_slot, "accept")?;
    for (label, key) in [
        ("resolution_source_state", coordinates.source_state),
        ("resolution_funding_ledger", coordinates.funding_ledger),
        (
            "resolution_funding_activation_receipt",
            coordinates.activation_receipt,
        ),
    ] {
        let account = rpc.required_account(key, label)?;
        accounts.insert(label.into(), account_evidence(key, &account));
    }
    completed.push(
        "completed the post-Open V7 funding readiness suffix in exact order: core-funding-create-v1, resolution-funding-activate-v1, core-funding-accept-v1"
            .into(),
    );
    Ok(())
}

/// Found the Market atomically on a real validator: `DCLTGMF3`.
///
/// Five stages in one rollback domain against the prestate `DCLTPCB2` left.
/// The Market is created by the Found stage and Opened by the last, so this
/// single transaction is the whole distance between a projected-Custody
/// prestate and a live Market with a Claims aggregate, a founder Position, and
/// a Hoard holding the collateral.
#[allow(clippy::too_many_arguments)]
fn execute_generic_market_founding(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    records: &MarketRecords,
    coordinates: &FoundingCoordinates,
    product: ProductContentId,
    mint: Pubkey,
    found31_market: Pubkey,
    actors: FoundingActorsV1,
    found_raw_account: Pubkey,
    lock_raw_account: Pubkey,
    claim_count: u32,
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
    accounts: &mut BTreeMap<String, AccountEvidence>,
    completed: &mut Vec<String>,
    mut submission_recorder: Option<&mut FoundingSubmissionRecorderV1<'_>>,
) -> Result<()> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let claims_program = pubkey(&plan.claims.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    authenticate_core_state_encoding_v1(rpc, found31_market)?;
    let outer = derive_founding_outer_v1(
        rpc,
        plan,
        input,
        records,
        coordinates,
        product,
        actors.founder,
        actors.substituted_founder,
        claim_count,
    )?;

    // The founding artifact and the terminal Lock are the records the
    // bootstrap already published. Requiring the chain's bytes to equal the
    // ones this derivation consumed is what makes the join below a statement
    // about the accounts the outer will read, rather than about two independent
    // encodings that happen to agree.
    for (label, key, expected) in [
        ("founding artifact", found_raw_account, &outer.found_raw),
        ("terminal Lock request", lock_raw_account, &outer.lock_raw),
    ] {
        let account = rpc.required_account(key, label)?;
        if &account.data != expected {
            return Err(Error::new(format!(
                "the published {label} record is not the bytes the founding derived"
            )));
        }
    }

    // Admission comes before mutation. Derive all three still-vacant request
    // coordinates from their exact content, assemble the same frame the outer
    // will receive, and compile its complete bounded-v0 message now. A lock
    // drift must stop here, before the five rent transfers or any Registry/ALT
    // publication can be planned or sent.
    let expected_realize_record =
        derive_raw_request_record_v1(registry, "projected-custody-realize", &outer.realize_raw)?;
    let expected_claims_record =
        derive_raw_request_record_v1(registry, "claims-founding-v5", &outer.claims_raw)?;
    let expected_substituted_claims_record = derive_raw_request_record_v1(
        registry,
        "claims-founding-v5-substituted-founder",
        &outer.substituted_claims_raw,
    )?;
    let prepared_founding = build_generic_market_founding_v3(
        plan,
        coordinates,
        &outer,
        records,
        [
            found_raw_account,
            lock_raw_account,
            expected_realize_record.raw,
            expected_claims_record.raw,
        ],
        actors.founder,
        mint,
    )?;
    let initial_founding_census =
        authenticate_generic_market_founding_lock_census_v3(payer.pubkey(), &prepared_founding)?;
    let open_admitted = compiled_complete_lock_census_v1(
        payer.pubkey(),
        &append_distinct_census_accounts_v1(&prepared_founding.instruction, 6),
    )?;
    let open_refused = compiled_complete_lock_census_v1(
        payer.pubkey(),
        &append_distinct_census_accounts_v1(&prepared_founding.instruction, 7),
    )?;
    if open_admitted.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1
        || open_refused.complete_keys != DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1
        || require_devnet_complete_key_limit_v1(open_admitted).is_err()
        || require_devnet_complete_key_limit_v1(open_refused).is_ok()
    {
        return Err(Error::new(format!(
            "DCLTGMF3 boundary census refused: base {}, +6 {}, +7 {}",
            initial_founding_census.complete_keys,
            open_admitted.complete_keys,
            open_refused.complete_keys,
        )));
    }
    eprintln!(
        "campaign: DCLTGMF3 pre-mutation compiled-message lock census: {} complete keys ({} static, {} writable loaded, {} readonly loaded), digest {}",
        initial_founding_census.complete_keys,
        initial_founding_census.static_keys,
        initial_founding_census.loaded_writable,
        initial_founding_census.loaded_readonly,
        hex(&initial_founding_census.key_privilege_digest),
    );

    // Nothing in the protocol funds these five. Core allocates the Market and
    // the permit and Claims allocates its three accounts with `allocate` and
    // `assign` only, never a transfer, so each must already hold its rent. The
    // Market and the permit are checked for EXACT equality with their rent
    // minima, and all three Claims balances are folded byte-exactly into the
    // permit's committed Claims request, so this transaction is part of the
    // founding's authenticated prestate and not a convenience.
    let aggregate_rent = rpc.minimum_balance(outer.aggregate_width)?;
    let position_rent = rpc.minimum_balance(outer.position_width)?;
    let admission_rent = rpc.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)?;
    let prefunding = [
        (coordinates.market, outer.market_rent),
        (outer.permit, outer.permit_rent),
        (outer.aggregate, aggregate_rent),
        (outer.position, position_rent),
        (outer.admission, admission_rent),
    ];
    let observed_prefunding = prefunding
        .iter()
        .map(|(address, _)| rpc.account(*address))
        .collect::<Result<Vec<_>>>()?;
    if observed_prefunding.iter().all(Option::is_none) {
        transactions.push(
            rpc.send(
                "pre-fund the founding's five program-allocated accounts",
                &prefunding
                    .iter()
                    .map(|(address, lamports)| transfer(&payer.pubkey(), address, *lamports))
                    .collect::<Vec<_>>(),
                payer,
            )?,
        );
    } else if prefunding
        .iter()
        .zip(&observed_prefunding)
        .all(|((_, expected), account)| {
            account.as_ref().is_some_and(|account| {
                account.owner == system_program::ID
                    && !account.executable
                    && account.data.is_empty()
                    && account.lamports == *expected
            })
        })
    {
        eprintln!(
            "campaign: exact DCLTGMF3 pre-funding already finalized; resumed without a second debit"
        );
    } else {
        return Err(Error::new(
            "DCLTGMF3 pre-funding is partial or differs from the five exact rent principals; never top up or overwrite it",
        ));
    }
    authenticate_founding_prefunding_v1(rpc, &outer, coordinates.market)?;

    // The Realize and Claims requests join the founding artifact and terminal
    // Lock this campaign already published. A substituted request is a
    // substituted address, so the outer's frame checks catch it.
    let realize_record = publish_record(
        rpc,
        registry,
        payer,
        expected_realize_record.schema,
        &outer.realize_raw,
        None,
        transactions,
    )?;
    require_published_record_matches_derivation_v1(
        "projected-custody-realize",
        expected_realize_record,
        realize_record,
    )?;
    let claims_record = publish_record(
        rpc,
        registry,
        payer,
        expected_claims_record.schema,
        &outer.claims_raw,
        None,
        transactions,
    )?;
    require_published_record_matches_derivation_v1(
        "claims-founding-v5",
        expected_claims_record,
        claims_record,
    )?;
    let substituted_claims_record = publish_record(
        rpc,
        registry,
        payer,
        expected_substituted_claims_record.schema,
        &outer.substituted_claims_raw,
        None,
        transactions,
    )?;
    require_published_record_matches_derivation_v1(
        "claims-founding-v5-substituted-founder",
        expected_substituted_claims_record,
        substituted_claims_record,
    )?;

    // Recompile immediately before ALT publication. The exact instruction and
    // canonical record coordinates must still produce the byte-identical
    // complete-key/privilege digest admitted before any write.
    let founding_census =
        authenticate_generic_market_founding_lock_census_v3(payer.pubkey(), &prepared_founding)?;
    if founding_census != initial_founding_census {
        return Err(Error::new(
            "DCLTGMF3 pre-ALT lock census differed from its pre-mutation admission",
        ));
    }
    eprintln!(
        "campaign: DCLTGMF3 pre-ALT compiled-message lock census recheck: {} complete keys ({} static, {} writable loaded, {} readonly loaded), digest {}",
        founding_census.complete_keys,
        founding_census.static_keys,
        founding_census.loaded_writable,
        founding_census.loaded_readonly,
        hex(&founding_census.key_privilege_digest),
    );
    let founding = prepared_founding.instruction;
    let readiness_coordinates = funding_readiness_coordinates_v1(plan, records, coordinates)?;
    let readiness_routing = Instruction {
        // This instruction is never submitted. It gives the one pre-Open ALT
        // plan an exhaustive address scope for the three packet-heavy V7
        // suffixes without inventing another mutable routing lifecycle.
        program_id: system_program::ID,
        accounts: funding_readiness_routing_addresses_v1(plan, readiness_coordinates)?
            .into_iter()
            .map(|key| AccountMeta::new_readonly(key, false))
            .collect(),
        data: Vec::new(),
    };
    let (routing, tables) = publish_frozen_founding_routing_table_v1(
        rpc,
        payer,
        "DCLTGMF3",
        &[founding.clone(), readiness_routing],
        transactions,
    )?;

    // Every account the founding creates, in the state it must be in if and
    // only if the whole five-stage chain committed.
    let created = [
        ("founding_market", coordinates.market),
        ("founding_permit", outer.permit),
        ("claims_aggregate", outer.aggregate),
        ("founder_position", outer.position),
        ("claims_admission", outer.admission),
    ];
    let untouched = |rpc: &mut Rpc, label: &str| -> Result<()> {
        for (name, key) in created {
            let account = rpc.required_account(key, name)?;
            if account.owner != system_program::ID || !account.data.is_empty() {
                return Err(Error::new(format!("{label} allocated the {name} account")));
            }
        }
        let source = rpc.required_account(coordinates.source_vault, "founding source vault")?;
        let replay = rpc.required_account(coordinates.projected_replay, "projected replay")?;
        if source.owner != token_program || replay.data.len() != PROJECTED_CUSTODY_STATE_BYTES_V2 {
            return Err(Error::new(format!(
                "{label} moved the projected-Custody prestate"
            )));
        }
        Ok(())
    };
    untouched(rpc, "the founding prestate")?;

    // The founding-outer hostile case. The substituted Claims request names a
    // different founder and is otherwise byte-identical, so the Position and
    // the admission it would mint belong to somebody else. The outer's
    // cross-request join is the only thing that refuses it, and the refusal has
    // to roll back Lock, Found, Realize, and Claims together: a chain that
    // committed the Market and then refused would be worse than one that never
    // ran.
    let mut substituted = founding.clone();
    substituted
        .accounts
        .get_mut(3)
        .ok_or_else(|| Error::new("the founding frame omitted its Claims request"))?
        .pubkey = substituted_claims_record.raw;
    let rollback_recipient = crate::seed::fresh_probe_address();

    // The probe is EVIDENCE COLLECTION, not the protocol: the refusal it
    // proves was executed and verified on the local proof chain, and this
    // probe's transaction is the LARGEST send of the whole campaign (the real
    // founding frame plus a prepended transfer and its recipient). Measured on
    // devnet 2026-08-28: leaders would not land exactly this transaction — six
    // full blockhash lifetimes across paid and unpaid attempts, while every
    // smaller transaction of the same campaign landed in seconds. A probe the
    // cluster refuses to CARRY proves nothing about the protocol either way,
    // so an undeliverable probe is RECORDED as an evidence gap and the
    // founding — a strictly smaller transaction, and the thing this campaign
    // exists to do — proceeds fail-closed on its own delivery.
    match rpc.send_v0_on_founding_heap_expected_failure_with_signers(
        "DCLTGMF3 refuses a substituted Claims request and rolls the whole founding back",
        &[
            transfer(&payer.pubkey(), &rollback_recipient, 1),
            substituted,
        ],
        payer,
        &[],
        routing,
        &tables,
    ) {
        Ok(rolled_back) => {
            let fee_only = rolled_back.fee_only_balance_change;
            if rpc.account(rollback_recipient)?.is_some() || fee_only != Some(true) {
                return Err(Error::new(format!(
                    "refused founding did not roll its whole transaction back to a fee-only \
                     debit: fee_only_balance_change={fee_only:?}",
                )));
            }
            transactions.push(rolled_back);
            untouched(rpc, "the refused substituted-Claims founding")?;
            completed.push(
                "proved DCLTGMF3 refuses a substituted Claims request and rolls the whole \
                 founding back to a fee-only debit"
                    .into(),
            );
        }
        Err(error) if error.to_string().contains("dropped") => {
            eprintln!(
                "campaign: EVIDENCE GAP, recorded: the substituted-Claims hostile probe could \
                 not be landed by the cluster ({error}); the refusal it proves was executed on \
                 the local proof chain, and the founding itself proceeds on its own delivery"
            );
            completed.push(
                "EVIDENCE GAP: the substituted-Claims hostile probe was undeliverable on this \
                 cluster (the largest transaction of the campaign; leaders would not carry it); \
                 the refusal it proves is executed evidence on the local proof chain only"
                    .into(),
            );
        }
        Err(error) => return Err(error),
    }

    let poststate =
        derive_founding_poststate_expectation_v1(plan, coordinates, actors.founder, claim_count)?;
    let founding_label =
        "found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF3)";
    let honest = match submission_recorder.as_deref_mut() {
        Some(recorder) => {
            let mut prestate_addresses = created.iter().map(|(_, key)| *key).collect::<Vec<_>>();
            prestate_addresses.extend([coordinates.source_vault, coordinates.projected_replay]);
            let completion_addresses = vec![
                coordinates.market,
                outer.permit,
                outer.aggregate,
                outer.position,
                outer.admission,
                coordinates.hoard_vault,
                coordinates.projected_replay,
            ];
            let mut completion = |rpc: &mut Rpc| {
                authenticate_open_market_poststate_v1(
                    rpc,
                    coordinates,
                    &poststate,
                    core,
                    claims_program,
                    custody,
                    token_program,
                    mint,
                )
            };
            send_durable_founding_v1(
                rpc,
                founding_label,
                FoundingSubmissionOperationV1::Dcltgmf3,
                std::slice::from_ref(&founding),
                &[payer],
                routing,
                &tables,
                hex(&founding_census.key_privilege_digest),
                &prestate_addresses,
                &completion_addresses,
                b"dclutch-market-dcltgmf3-recovery-payload-v1".to_vec(),
                Some(FOUNDING_HEAP_FRAME_BYTES),
                recorder,
                &mut completion,
            )?
        }
        None => rpc.send_v0_on_founding_heap_with_signers(
            founding_label,
            &[founding],
            payer,
            &[],
            routing,
            &tables,
        )?,
    };
    let open_slot = honest.slot;
    transactions.push(honest);

    authenticate_open_market_poststate_v1(
        rpc,
        coordinates,
        &poststate,
        core,
        claims_program,
        custody,
        token_program,
        mint,
    )?;
    execute_funding_readiness_suffix_v1(
        rpc,
        plan,
        records,
        coordinates,
        payer,
        transactions,
        accounts,
        completed,
        open_slot,
        &tables.iter().map(|table| table.key).collect::<Vec<_>>(),
        submission_recorder.as_deref_mut(),
    )?;
    for (label, key) in [
        ("founding_market", coordinates.market),
        ("claims_aggregate", outer.aggregate),
        ("founder_position", outer.position),
        ("claims_admission", outer.admission),
        ("founding_hoard_vault_open", coordinates.hoard_vault),
        (
            "founding_normal_custody_replay",
            coordinates.projected_replay,
        ),
    ] {
        let account = rpc.required_account(key, label)?;
        accounts.insert(label.into(), account_evidence(key, &account));
    }
    completed.push(
        "pre-funded the five accounts the founding allocates but never funds - the Market, the one-shot Core permit, and the Claims aggregate, founder Position, and admission - to the exact rents the permit's committed Claims request re-derives".into(),
    );
    completed.push(
        "derived the founding's Lock and Realize receipts by running the Custody kernel's own transitions over the exact prestate DCLTPCB2 left on chain, and the permit intent and Claims request Core rebuilds inside the Found stage".into(),
    );
    // The substituted-Claims probe's own line is pushed at its send site,
    // where delivered-and-refused and undeliverable are told apart honestly.
    completed.push(
        "executed DCLTGMF3: the Market is OPEN, with the Claims liability aggregate, the founder Position, the admission record, and a Hoard holding the exact collateral".into(),
    );
    Ok(())
}

/// Require the five program-allocated accounts to hold exactly their rents.
fn authenticate_founding_prefunding_v1(
    rpc: &mut Rpc,
    outer: &FoundingOuterV1,
    market: Pubkey,
) -> Result<()> {
    for (label, key, lamports) in [
        ("founding Market", market, outer.market_rent),
        ("founding permit", outer.permit, outer.permit_rent),
        (
            "Claims aggregate",
            outer.aggregate,
            rpc.minimum_balance(outer.aggregate_width)?,
        ),
        (
            "founder Position",
            outer.position,
            rpc.minimum_balance(outer.position_width)?,
        ),
        (
            "Claims admission",
            outer.admission,
            rpc.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)?,
        ),
    ] {
        let account = rpc.required_account(key, label)?;
        if account.owner != system_program::ID
            || !account.data.is_empty()
            || account.executable
            || account.lamports != lamports
        {
            return Err(Error::new(format!(
                "{label} prestate was not the exact vacant rent-funded account the founding requires"
            )));
        }
    }
    Ok(())
}

/// Reacquire and check every poststate the Open gate claims.
#[allow(clippy::too_many_arguments)]
fn authenticate_open_market_poststate_v1(
    rpc: &mut Rpc,
    coordinates: &FoundingCoordinates,
    expected: &FoundingPoststateExpectationV1,
    core: Pubkey,
    claims_program: Pubkey,
    custody: Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
) -> Result<()> {
    let market = rpc.required_account(coordinates.market, "founded Market")?;
    let state = CoreState::decode(&market.data)
        .map_err(|error| Error::new(format!("founded Market state: {error:?}")))?;
    if market.owner != core
        || market.data.len() != STATE_BYTES
        || market.executable
        || state.phase != Phase::Open
        || state.readiness != Readiness::Consumed
        || state.identity != coordinates.identity
        || state.terminal_receipt.is_some()
        || state.rent_beneficiary.to_bytes() != coordinates.credit.to_bytes()
    {
        return Err(Error::new(
            "the founded Market did not reach the Open phase its founding claims",
        ));
    }

    // The one-shot permit is consumed by the commit-last Open stage and its
    // lamports return to the lifecycle credit. A permit that survived would be
    // a second founding.
    let permit = rpc.account(expected.permit)?;
    if permit.is_some_and(|account| {
        account.owner != system_program::ID || account.lamports != 0 || !account.data.is_empty()
    }) {
        return Err(Error::new(
            "the founding permit survived the Open stage that must consume it",
        ));
    }

    for (label, key, owner, width) in [
        (
            "Claims aggregate",
            expected.aggregate,
            claims_program,
            expected.aggregate_width,
        ),
        (
            "founder Position",
            expected.position,
            claims_program,
            expected.position_width,
        ),
        (
            "Claims admission",
            expected.admission,
            claims_program,
            PROTOCOL_POSITION_ADMISSION_BYTES_V2,
        ),
    ] {
        let account = rpc.required_account(key, label)?;
        if account.owner != owner
            || account.data.len() != width
            || account.data.iter().all(|byte| *byte == 0)
        {
            return Err(Error::new(format!(
                "{label} was not allocated, owned, and written by the Claims founding"
            )));
        }
    }

    let hoard = rpc.required_account(coordinates.hoard_vault, "founded Hoard")?;
    let hoard_state = TokenAccount::parse(&hoard.data)
        .map_err(|error| Error::new(format!("Hoard vault: {error:?}")))?;
    if hoard.owner != token_program
        || hoard_state.mint != mint.to_bytes()
        || hoard_state.amount != expected.principal
        || hoard_state.state != AccountState::Initialized
    {
        return Err(Error::new(
            "the Hoard does not hold exactly the founding principal",
        ));
    }

    // The source compartment is fully consumed: both accounts closed to the
    // lifecycle credit, and the projection rewritten in place as the normal
    // live replay the Market's ordinary Custody route will use.
    for (label, key) in [
        ("founding source vault", coordinates.source_vault),
        ("founding source replay", coordinates.source_replay),
    ] {
        if let Some(account) = rpc.account(key)?
            && (account.owner != system_program::ID
                || account.lamports != 0
                || !account.data.is_empty())
        {
            return Err(Error::new(format!(
                "{label} was not closed by the founding Lock stage"
            )));
        }
    }
    let replay = rpc.required_account(coordinates.projected_replay, "normal Custody replay")?;
    let normal = CustodyReplayV1::decode(&replay.data)
        .map_err(|error| Error::new(format!("normal Custody replay: {error:?}")))?;
    if replay.owner != custody
        || replay.data.len() != CUSTODY_REPLAY_BYTES_V1
        || normal.market != coordinates.market.to_bytes()
        || normal.generation != coordinates.generation
        || normal.open_vault_count != 1
        || normal.next_revision != FOUNDING_NORMAL_REPLAY_REVISION_V1
    {
        return Err(Error::new(
            "the projected replay was not realized into the Market's normal Custody replay",
        ));
    }
    if rpc
        .account(coordinates.controller_funding_checkpoint)?
        .is_some_and(|account| account.lamports != 0 || !account.data.is_empty())
    {
        return Err(Error::new(
            "the Open acknowledgement finalized without consuming the controller funding checkpoint",
        ));
    }
    for ledger in &coordinates.funding_ledgers {
        let account = rpc.required_account(ledger.address, "Open controller funding ledger")?;
        if account.owner != ledger.controller
            || account.lamports != ledger.required_lamports
            || account.data != ledger.bytes
        {
            return Err(Error::new(
                "Open changed a Pending controller funding ledger while consuming its checkpoint",
            ));
        }
    }
    Ok(())
}

/// Domain-separated preimage prefix for every demo semantic identifier.
///
/// A demo identifier is the SHA-256 of `domain || 00 || part || 00 || part...`.
/// It names a semantic spec; it is not a checked production release row and no
/// registry publishes it outside this local lab.
const DEMO_ID_DOMAIN_V1: &[u8] = b"dclutch/local-demo-market/v1";

pub(crate) fn demo_id(role: &str, parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DEMO_ID_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(role.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// Everything that distinguishes one Pyth-resolved range-protection Market
/// from another: which release row it binds, which feed account body it
/// derives from, where its terminal window lies, and what its band is.
///
/// One core (`pyth_market_input`) serves both the local demo (the captured
/// synthetic release, a window ending at the frozen fixture publication) and
/// the devnet flagship (the committed devnet row, a live window), so the two
/// cannot drift in graph shape — only in the facts this struct names.
#[derive(Clone, Copy)]
enum PythMarketProviderV1<'a> {
    Pull(&'a dclutch_pyth_svm::PythReleaseV1),
    Sponsored(PythSponsoredPushReleaseV1),
}

impl PythMarketProviderV1<'_> {
    fn adapter_id(self) -> [u8; 32] {
        match self {
            Self::Pull(release) => release.adapter_id(),
            Self::Sponsored(release) => release.adapter_id(),
        }
    }

    fn provider_family_id(self) -> [u8; 32] {
        match self {
            // Preserve the established terminal/relayed provider family byte
            // for byte. Sponsored releases own their provider family inside
            // the canonical release body.
            Self::Pull(_) => demo_id("provider-family/pyth", &[]),
            Self::Sponsored(release) => release.provider_family_id(),
        }
    }

    fn deployment_release_bytes(self) -> Vec<u8> {
        match self {
            Self::Pull(release) => release.to_bytes().to_vec(),
            Self::Sponsored(release) => release.to_bytes().to_vec(),
        }
    }

    fn price_update_codec_id(self) -> [u8; 32] {
        match self {
            Self::Pull(release) => release.price_update_codec_id(),
            Self::Sponsored(release) => release.price_update_codec_id(),
        }
    }

    fn transport_profile_id(self) -> [u8; 32] {
        match self {
            Self::Pull(release) => release.router_abi_id(),
            Self::Sponsored(release) => release.transport_profile_id(),
        }
    }

    const fn access_profile(self) -> SourceAccessProfile {
        match self {
            Self::Pull(_) => SourceAccessProfile::PythTerminalOneTransaction,
            Self::Sponsored(_) => SourceAccessProfile::PythSponsoredPushSnapshot,
        }
    }

    fn sponsored_release_hex(self) -> String {
        match self {
            Self::Pull(_) => String::new(),
            Self::Sponsored(release) => hex(&release.to_bytes()),
        }
    }

    fn authenticate_price_update(self, update: &dclutch_pyth_svm::FullPriceUpdateV2) -> Result<()> {
        if let Self::Sponsored(release) = self
            && (update.write_authority() != release.price_account()
                || update.feed_id() != release.feed_id()
                || update.publish_time() <= 0
                || update.posted_slot() == 0
                || update.prev_publish_time() > update.publish_time())
        {
            return Err(Error::new(
                "sponsored price body did not authenticate against the compiled release",
            ));
        }
        Ok(())
    }
}

pub(crate) struct PythMarketParamsV1<'a> {
    pub(crate) registry: Pubkey,
    release: PythMarketProviderV1<'a>,
    /// Domain-separation label folded into every demo-id: the synthetic
    /// fixture's local label for the lab, the cluster identity for devnet.
    pub(crate) label: [u8; 32],
    pub(crate) product_name: &'a str,
    pub(crate) coordinate_domain_name: &'a str,
    pub(crate) feed_label: &'a [u8],
    /// A `PriceUpdateV2` account body for the feed: the captured fixture
    /// locally, a live read of the push-oracle account for devnet. Supplies
    /// the feed id and exponent the adapter config binds.
    pub(crate) price_update: &'a [u8],
    pub(crate) window_start: i64,
    pub(crate) window_end: i64,
    pub(crate) max_age_seconds: u32,
    pub(crate) max_confidence_bps: u16,
    pub(crate) cut_denominator: u64,
    pub(crate) cuts: Vec<i128>,
    pub(crate) coefficients: Vec<u64>,
    pub(crate) generation: u64,
    pub(crate) local_participant_fixture_liquidity_atoms: u64,
}

/// Construct the canonical local demo Market: SOL/USD range protection.
///
/// The Product is a small categorical partition of USD-cents-per-SOL with cuts
/// at 12000 and 18000, so the three ordinary regions are "below 120.00",
/// "inside [120.00, 180.00)", and "at or above 180.00", followed by the
/// explicit failure outcome. The portfolio pays one unit of the liability
/// basis in either tail and nothing inside the range or on failure, which is
/// the payoff a holder buys as protection against SOL/USD leaving the band.
///
/// Every semantic identifier is a domain-separated digest naming the spec it
/// stands for, and the resolution identifiers additionally bind the captured
/// synthetic-local Pyth release from
/// `fixtures/pyth/local-upgraded-2026-08-22`. That release is a local lab
/// projection documented in `docs/evidence/PYTH_SYNTHETIC_RELEASE_V1.md`; it is
/// not a production provider release, and this Market is not a mainnet or
/// devnet product.
pub(crate) fn demo_market_input(
    registry: Pubkey,
    direct: DirectMarketCompilerInputV1<'_>,
) -> Result<MarketRunInput> {
    use dclutch_pyth_svm::{FullPriceUpdateV2, synthetic_fixture::local_validator_release_v1};

    // The release this Market resolves against is the LOCAL-VALIDATOR
    // projection, not the captured one. They differ in exactly two facts --
    // the local label and both Loader deployment slots -- and the difference
    // is the whole point: `solana-test-validator --upgradeable-program`
    // regenerates the receiver's and router's `ProgramData` headers with
    // deployment slot ZERO, and the captured row states devnet's slots. A
    // Market that named the captured row would be resolvable only on a chain
    // where those deployments happened, which is not this one.
    let fixture = local_validator_release_v1()
        .map_err(|error| Error::new(format!("local-validator Pyth release: {error:?}")))?;
    let update = FullPriceUpdateV2::parse(FIXTURE_PRICE_UPDATE)
        .map_err(|error| Error::new(format!("captured Pyth price update: {error:?}")))?;
    // The window is a real 300-second terminal period ENDING at the captured
    // publication (TWIN's finding: a window forced to one instant is a market
    // nobody can resolve), and `max_age_seconds` is the fixture's declared
    // shelf life, not a market parameter — see the shared core for both.
    pyth_market_input(
        PythMarketParamsV1 {
            registry,
            release: PythMarketProviderV1::Pull(fixture.release()),
            label: fixture.local_label(),
            product_name: "product/sol-usd-range-protection",
            coordinate_domain_name: "coordinate-domain/usd-cents-per-sol",
            feed_label: b"sol-usd",
            price_update: FIXTURE_PRICE_UPDATE,
            window_start: update
                .publish_time()
                .checked_sub(TERMINAL_WINDOW_WIDTH_SECONDS)
                .ok_or_else(|| Error::new("terminal window start underflowed"))?,
            window_end: update.publish_time(),
            max_age_seconds: FIXTURE_SHELF_LIFE_SECONDS,
            // The adapter's ceiling — the widest the type admits. A LAB setting:
            // this Market resolves against a single captured publication whose
            // confidence is whatever it was on the day it was captured, and
            // refusing it on confidence would be refusing the fixture rather than
            // testing the adapter. The devnet flagship states a real bound.
            max_confidence_bps: 10_000,
            cut_denominator: 100,
            cuts: vec![12_000, 18_000],
            coefficients: vec![1, 0, 1, 0],
            generation: 1,
            local_participant_fixture_liquidity_atoms: LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1,
        },
        direct,
    )
}

/// The devnet flagship's input: SOL/USD (or any Pyth feed) range protection,
/// bound to the committed devnet release row and a LIVE terminal window.
///
/// The window-width floor is the measured devnet SOL/USD cadence
/// (SMOKE-0 §1.2: p50 313 s over an 86-hour page, three live-confirmed gaps at
/// 313–314 s): four cadences ≈ 1,252 s gives ~98% coverage that at least one
/// publication lands inside the window, and a narrower window is a market that
/// fails for provider reasons, so it is refused here rather than founded.
pub(crate) struct DevnetPythMarketSpecV1<'a> {
    pub(crate) registry: Pubkey,
    /// A live `PriceUpdateV2` account body read off devnet for the feed this
    /// market is about (SOL/USD: `7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE`).
    pub(crate) price_update: &'a [u8],
    pub(crate) product_name: &'a str,
    pub(crate) coordinate_domain_name: &'a str,
    pub(crate) feed_label: &'a [u8],
    pub(crate) window_start: i64,
    pub(crate) window_width_seconds: u32,
    pub(crate) max_age_seconds: u32,
    pub(crate) cut_denominator: u64,
    pub(crate) cuts: Vec<i128>,
    pub(crate) coefficients: Vec<u64>,
    pub(crate) generation: u64,
}

/// Four measured 313-second cadences, the §12.3 guidance floor.
pub(crate) const DEVNET_MINIMUM_WINDOW_WIDTH_SECONDS: u32 = 1_252;

pub(crate) fn devnet_market_input(
    spec: DevnetPythMarketSpecV1<'_>,
    direct: DirectMarketCompilerInputV1<'_>,
) -> Result<MarketRunInput> {
    let window_end = devnet_window_end_v1(&spec)?;
    let release = dclutch_pyth_svm::devnet_release_v1()
        .map_err(|error| Error::new(format!("devnet Pyth release row: {error:?}")))?;
    pyth_market_input(
        PythMarketParamsV1 {
            registry: spec.registry,
            release: PythMarketProviderV1::Pull(&release),
            // The cluster identity is the devnet label: a devnet market's ids can
            // never collide with the lab's, whose label is the synthetic fixture's.
            label: release.cluster_id(),
            product_name: spec.product_name,
            coordinate_domain_name: spec.coordinate_domain_name,
            feed_label: spec.feed_label,
            price_update: spec.price_update,
            window_start: spec.window_start,
            window_end,
            max_age_seconds: spec.max_age_seconds,
            // A real bound for a live feed: 5% of price. Pyth's stated SOL/USD
            // confidence runs well under 0.1%, so this refuses only a genuinely
            // degenerate publication, not an ordinary one.
            max_confidence_bps: 500,
            cut_denominator: spec.cut_denominator,
            cuts: spec.cuts,
            coefficients: spec.coefficients,
            generation: spec.generation,
            local_participant_fixture_liquidity_atoms: 0,
        },
        direct,
    )
}

/// The credential-free devnet flagship. The update is a finalized read of the
/// official sponsored SOL/USD push account; no Hermes request or bearer token
/// participates in this graph. The compiled release binds the two programs,
/// both ProgramData accounts and slots, the Receiver config, feed, and exact
/// price account before the ordinary market compiler sees the body.
pub(crate) fn devnet_sponsored_market_input(
    spec: DevnetPythMarketSpecV1<'_>,
    direct: DirectMarketCompilerInputV1<'_>,
) -> Result<MarketRunInput> {
    let window_end = devnet_window_end_v1(&spec)?;
    let release = dclutch_pyth_svm::devnet_sponsored_sol_usd_release_v1()
        .map_err(|error| Error::new(format!("devnet sponsored Pyth release row: {error:?}")))?;
    pyth_market_input(
        PythMarketParamsV1 {
            registry: spec.registry,
            release: PythMarketProviderV1::Sponsored(release),
            label: release.cluster_id(),
            product_name: spec.product_name,
            coordinate_domain_name: spec.coordinate_domain_name,
            feed_label: spec.feed_label,
            price_update: spec.price_update,
            window_start: spec.window_start,
            window_end,
            max_age_seconds: spec.max_age_seconds,
            max_confidence_bps: 500,
            cut_denominator: spec.cut_denominator,
            cuts: spec.cuts,
            coefficients: spec.coefficients,
            generation: spec.generation,
            local_participant_fixture_liquidity_atoms: 0,
        },
        direct,
    )
}

fn devnet_window_end_v1(spec: &DevnetPythMarketSpecV1<'_>) -> Result<i64> {
    if spec.window_width_seconds < DEVNET_MINIMUM_WINDOW_WIDTH_SECONDS {
        return Err(Error::new(format!(
            "devnet terminal window width {} s is below the measured floor \
             {DEVNET_MINIMUM_WINDOW_WIDTH_SECONDS} s (four cadences of the measured 313 s SOL/USD \
             p50, ~98% coverage). A narrower window is a market that fails for provider reasons; \
             it is refused here rather than founded.",
            spec.window_width_seconds
        )));
    }
    spec.window_start
        .checked_add(i64::from(spec.window_width_seconds))
        .ok_or_else(|| Error::new("terminal window end overflowed"))
}

/// The shared Pyth range-protection graph compiler. See the two callers for
/// what each fact means in its context.
fn pyth_market_input(
    params: PythMarketParamsV1<'_>,
    direct: DirectMarketCompilerInputV1<'_>,
) -> Result<MarketRunInput> {
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_pyth_svm::FullPriceUpdateV2;
    use dclutch_source_contract::{
        CapacityEnvelope, ProviderReleaseV1, PythAdapterConfigV1, RECOVERY_POLICY_MAX_ATTEMPTS_V2,
        RecoveryAttemptV2, RoundingBoundary, SOURCE_FAILURE_POLICY_RELEASE_ID_V2,
        SourceCapacityProfileV1, SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind,
        WindowSpecV1,
    };

    let local_label = params.label;
    let adapter = params.release.adapter_id();
    let feed = params.feed_label;

    let product_identity = demo_id(params.product_name, &[&local_label]);
    let coordinate_domain = demo_id(params.coordinate_domain_name, &[]);
    let result_unit = demo_id("result-unit/usd-cents", &[]);
    let claim_basis = demo_id("claim-basis/unit-complete-set", &[]);
    let representation = demo_id("representation/categorical-fixed-width", &[]);
    let mapping = demo_id("mapping/scaled-integer-cut", &[&coordinate_domain]);
    // ------------------------------------------------------------ the source graph
    //
    // The four identities below used to be domain-separated demo digests, and
    // that made this Market UNRESOLVABLE in a way nothing refused. Both
    // provider legs authenticate the source spec, window spec and statistic
    // spec as FINALIZED REGISTRY RECORDS, and a finalized record lives at an
    // address derived from the hash of its own body -- so an identity that is
    // the digest of a SENTENCE names a record nobody can ever publish. The
    // Market could create and activate its Resolution funding, and then stop
    // forever, one step short of a certificate. Found by JRNY-2, which is the
    // first campaign to drive the funding ladder against a chain and then ask
    // what came next; the Pyth receiver and router were deployed and waiting
    // the whole time.
    //
    // So the graph is compiled here, its identities ARE its bodies' digests,
    // and the run spec carries the bodies for the same reason it already
    // carries `linked_basis_hex` rather than an opaque digest.
    let update = FullPriceUpdateV2::parse(params.price_update)
        .map_err(|error| Error::new(format!("Pyth price update body: {error:?}")))?;
    params.release.authenticate_price_update(&update)?;
    let capacity = SourceCapacityProfileV1::new(
        CapacityEnvelope::Measured,
        1,
        1,
        source_content(demo_id("capacity/terminal-verifier", &[&local_label]))?,
        source_content(demo_id("capacity/envelope-basis", &[&local_label]))?,
        256,
        0,
    )
    .map_err(|error| Error::new(format!("demo source capacity: {error:?}")))?;
    let capacity_id = source_content(record_identity(&capacity.to_bytes()))?;

    let pyth_release_bytes = params.release.deployment_release_bytes();
    // `adapter_release_id` is the PUBLISHED PYTH RELEASE's own `adapter_id`,
    // and getting here took two chain refusals and a contradiction worth
    // recording, because TWO LIVE READERS OF THIS ONE FIELD DISAGREE ABOUT WHAT
    // IT HOLDS:
    //
    //   * `PythProviderAdapterObligationV1::from_material_view`
    //     (dclutch-source-contract lib.rs) refuses anything that is not
    //     `PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1` -- the field is "which
    //     provider EXTENSION is this", a closed constant.
    //   * `authenticate_provider_release` (resolution-proof-sbf provider_v3.rs)
    //     refuses anything that is not `pyth_release.adapter_id()` -- the field
    //     is "which adapter release does this provider deployment carry".
    //
    // The two constants differ, so no `ProviderReleaseV1` satisfies both. The
    // live V3 provider route goes through the V2 obligation
    // (`from_authenticated_records`, which does NOT check the extension) and
    // then `authenticate_provider_release`, so the SECOND reading is the one a
    // chain enforces and this is its value. The V1 obligation is not on that
    // path.
    //
    // Measured, in order: `adapter_id` refused at the publisher below, which
    // was selecting the adapter-config schema off this field and taking V1's
    // reading; the extension constant then refused ON CHAIN with
    // ResolutionError::ProviderObservation (0x800A) after 1,070,265 CU. Both
    // refusals were correct given their own reading. The publisher now selects
    // on the source spec's access profile instead, which is a real extension
    // discriminator and is pinned by the same obligation that consumes it.
    let provider_release = ProviderReleaseV1::new(
        source_content(params.release.provider_family_id())?,
        source_content(adapter)?,
        source_content(record_identity(&pyth_release_bytes))?,
        source_content(params.release.price_update_codec_id())?,
        source_content(params.release.transport_profile_id())?,
    );
    let provider_release_bytes = provider_release.to_bytes();
    let provider_release_id = record_identity(&provider_release_bytes);

    // `max_confidence_bps` is the adapter's tolerance for the provider's own
    // stated confidence interval. Each caller states its own bound and why:
    // the lab passes the type's 10,000-bps ceiling (refusing the frozen
    // fixture on confidence would be refusing the fixture rather than testing
    // the adapter); the devnet flagship states a real live-feed bound.
    let adapter_config = PythAdapterConfigV1::new(
        update.feed_id(),
        update.exponent(),
        params.max_confidence_bps,
    )
    .map_err(|error| Error::new(format!("Pyth adapter configuration: {error:?}")))?;
    let adapter_config_bytes = adapter_config.to_bytes();
    let adapter_config_id = record_identity(&adapter_config_bytes);

    let source_unit = demo_id("source-unit/pyth-scaled-price", &[&adapter, feed]);
    let source_spec = SourceSpecV1::new(
        source_content(coordinate_domain)?,
        source_content(source_unit)?,
        source_content(provider_release_id)?,
        params.release.access_profile(),
        source_content(adapter_config_id)?,
        capacity_id,
    );
    let source_spec_bytes = source_spec.to_bytes();
    let primary_source = record_identity(&source_spec_bytes);

    // The §12.3 admission is TWO predicates over two different clocks, and
    // conflating them is what made the old fixture unusable:
    //
    //   observation_unix_seconds in [window.start, window.end]      -- what it is ABOUT
    //   publication_unix_seconds in [now - max_age, now + max_skew] -- how FRESH it is
    //
    // The first is a market parameter and each caller states its own window:
    // the lab a 300-second terminal period ending at the captured publication
    // (TWIN's finding: a window forced to one instant is answered only when a
    // publication happens to land on that exact second, and Pyth's SOL/USD
    // cadence is nearer five minutes); the devnet flagship a live window whose
    // width the cadence floor in `devnet_market_input` enforces.
    //
    // The second is NOT the same clock and must never be read as one. For the
    // frozen fixture `now - publication` is THE AGE OF THE FIXTURE, so the lab
    // states its declared shelf life (the journey campaign refuses the moment
    // the fixture outlives it); a live market states a real submission-latency
    // budget in seconds.
    let window = WindowSpecV1::new(
        source_content(primary_source)?,
        WindowKind::Terminal,
        params.window_start,
        params.window_end,
        params.max_age_seconds,
        1,
        source_content(demo_id("window-schedule/terminal-single-sample", &[]))?,
    )
    .map_err(|error| Error::new(format!("terminal window: {error:?}")))?;
    let window_bytes = window.to_bytes();
    let window = record_identity(&window_bytes);

    let statistic = StatisticSpecV1::new(
        source_content(source_unit)?,
        source_content(result_unit)?,
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_content(demo_id("statistic-evaluator/terminal-sample", &[]))?,
        capacity,
    )
    .map_err(|error| Error::new(format!("demo terminal statistic: {error:?}")))?;
    let statistic_bytes = statistic.to_bytes();
    let statistic = record_identity(&statistic_bytes);

    // The failure policy is a PROTOCOL release identity, not this campaign's
    // to name. It was a demo digest for the same reason the three above were,
    // and it had the same consequence.
    let failure_policy = SOURCE_FAILURE_POLICY_RELEASE_ID_V2;
    let recovery_allocation = demo_id("recovery/funding-allocation", &[&local_label]);
    let recovery_source = demo_id("recovery/secondary-source-spec", &[&adapter, feed]);
    let recovery_authority = demo_id("recovery/attempt-authority", &[&local_label]);
    let recovery_root = demo_id("recovery/policy-root", &[&local_label]);

    let cut_denominator = params.cut_denominator;
    let cuts: Vec<i128> = params.cuts.clone();
    let coefficients: Vec<u64> = params.coefficients.clone();
    let outcome_count = coefficients.len();

    // The liability basis is a real record, not a name. Its semantic identity
    // omits the Product and result-domain links, so it is derivable before the
    // Product that declares it exists; the record is then recompiled with both
    // real links and must keep the same identity.
    let evaluator_release = demo_id("liability-basis/categorical-unit-evaluator", &[]);
    let liability_basis = semantic_basis_identity_v3(&compile_linked_basis_v3(
        product_identity,
        product_identity,
        coordinate_domain,
        result_unit,
        evaluator_release,
        outcome_count,
    )?)?;

    let product = ProductCompilationInputV2 {
        product_id: ProductContentId::new(product_identity)
            .map_err(|error| Error::new(format!("demo Product ID: {error:?}")))?,
        coordinate_domain_id: ProductContentId::new(coordinate_domain)
            .map_err(|error| Error::new(format!("demo coordinate domain: {error:?}")))?,
        result_unit_id: ProductContentId::new(result_unit)
            .map_err(|error| Error::new(format!("demo result unit: {error:?}")))?,
        claim_basis_id: ProductContentId::new(claim_basis)
            .map_err(|error| Error::new(format!("demo claim basis: {error:?}")))?,
        liability_basis_id: ProductContentId::new(liability_basis)
            .map_err(|error| Error::new(format!("demo liability basis: {error:?}")))?,
        representation_release_id: ProductContentId::new(representation)
            .map_err(|error| Error::new(format!("demo representation: {error:?}")))?,
        mapping_release_id: ProductContentId::new(mapping)
            .map_err(|error| Error::new(format!("demo mapping: {error:?}")))?,
        cut_denominator,
        cuts: &cuts,
        portfolio_denominator: 1,
        coefficients: &coefficients,
    };
    let mut product_bytes = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![
        0_u8;
        result_domain_record_bytes(cuts.len()).map_err(|error| Error::new(
            format!("demo domain width: {error:?}")
        ))?
    ];
    let mut portfolio = vec![
        0_u8;
        portfolio_record_bytes(outcome_count).map_err(|error| Error::new(
            format!("demo portfolio width: {error:?}")
        ))?
    ];
    compile_product_records_v2(
        params.registry,
        product,
        &mut product_bytes,
        &mut domain,
        &mut portfolio,
    )
    .map_err(|error| Error::new(format!("demo Product compiler: {error:?}")))?;
    let product_digest: [u8; 32] = Sha256::digest(product_bytes).into();
    let domain_digest: [u8; 32] = Sha256::digest(&domain).into();
    let linked_basis = compile_linked_basis_v3(
        product_identity,
        domain_digest,
        coordinate_domain,
        result_unit,
        evaluator_release,
        outcome_count,
    )?;
    if semantic_basis_identity_v3(&linked_basis)? != liability_basis {
        return Err(Error::new(
            "linking the demo liability basis to its Product changed its semantic identity",
        ));
    }

    let attempt = RecoveryAttemptV2::new(
        SourceContentId::new(recovery_source)
            .map_err(|error| Error::new(format!("demo recovery source: {error:?}")))?,
        SourceContentId::new(recovery_authority)
            .map_err(|error| Error::new(format!("demo recovery authority: {error:?}")))?,
        2_000_000_000,
        SourceContentId::new(recovery_allocation)
            .map_err(|error| Error::new(format!("demo recovery allocation: {error:?}")))?,
    )
    .map_err(|error| Error::new(format!("demo recovery attempt: {error:?}")))?;
    let mut attempts = [None; RECOVERY_POLICY_MAX_ATTEMPTS_V2];
    attempts[0] = Some(attempt);
    let recovery = RecoveryPolicyV2::new(
        SourceContentId::new(recovery_root)
            .map_err(|error| Error::new(format!("demo recovery root: {error:?}")))?,
        attempts,
        1,
    )
    .map_err(|error| Error::new(format!("demo recovery policy: {error:?}")))?;
    let recovery_bytes = recovery.to_bytes();
    let recovery_digest: [u8; 32] = Sha256::digest(recovery_bytes).into();
    let material = SourceMaterialV3::explicitly_unbounded(
        SourceContentId::new(product_digest)
            .map_err(|error| Error::new(format!("demo Product digest: {error:?}")))?,
        SourceContentId::new(primary_source)
            .map_err(|error| Error::new(format!("demo primary source: {error:?}")))?,
        SourceContentId::new(window)
            .map_err(|error| Error::new(format!("demo window: {error:?}")))?,
        SourceContentId::new(statistic)
            .map_err(|error| Error::new(format!("demo statistic: {error:?}")))?,
        Some(
            SourceContentId::new(recovery_digest)
                .map_err(|error| Error::new(format!("demo recovery digest: {error:?}")))?,
        ),
        SourceContentId::new(failure_policy)
            .map_err(|error| Error::new(format!("demo failure policy: {error:?}")))?,
    );
    let material_digest: [u8; 32] = Sha256::digest(material.to_bytes()).into();

    let native = CompartmentFundingV1::native_lamports(1)
        .map_err(|error| Error::new(format!("demo funding: {error:?}")))?;
    let none = CompartmentFundingV1::not_applicable();
    let amounts = FundingAmountsV1::new(native, native, none, none, native, none, none)
        .map_err(|error| Error::new(format!("demo funding amounts: {error:?}")))?;
    let quote = FundingQuoteV1::new(amounts, None)
        .map_err(|error| Error::new(format!("demo funding quote: {error:?}")))?;
    // The companion release is projected from the exact authenticated plan
    // that produced the Direct compiler. A stale hard-coded V4 here once let
    // the read-only market compiler succeed and then made real founding refuse
    // only after collateral, records, RentCredit, ALT, and Found37 existed.
    let release = CapabilityContentId::new(direct.resolution_release)
        .map_err(|error| Error::new(format!("demo Resolution release: {error:?}")))?;
    let mut entries_input: Vec<([u8; 32], [u8; 32])> = vec![
        (
            demo_id("capability/resolve-primary", &[&local_label]),
            attempt.funding_allocation_id().to_bytes(),
        ),
        (
            demo_id("capability/recovery-policy", &[&local_label]),
            recovery_digest,
        ),
        (
            demo_id("capability/source-material", &[&local_label]),
            material_digest,
        ),
    ];
    // The manifest is canonical only when entries are strictly ordered by
    // capability-kind identity; the demo kinds are digests, so sort them.
    entries_input.sort_by_key(|entry| entry.0);
    let mut entries = Vec::new();
    for (index, (kind, config)) in entries_input.into_iter().enumerate() {
        let entry_index =
            u16::try_from(index).map_err(|_| Error::new("demo capability index overflow"))?;
        entries.push(
            CapabilityEntryV1::new(
                CapabilityContentId::new(kind)
                    .map_err(|error| Error::new(format!("demo capability kind: {error:?}")))?,
                release,
                CapabilityContentId::new(config)
                    .map_err(|error| Error::new(format!("demo capability config: {error:?}")))?,
                CapabilityContentId::new(demo_id("capability/capacity", &[&[entry_index as u8]]))
                    .map_err(|error| Error::new(format!("demo capability capacity: {error:?}")))?,
                CapabilityContentId::new(demo_id("capability/schema", &[]))
                    .map_err(|error| Error::new(format!("demo capability schema: {error:?}")))?,
                CapabilityContentId::new(demo_id("capability/derivation", &[])).map_err(
                    |error| Error::new(format!("demo capability derivation: {error:?}")),
                )?,
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quote,
            )
            .map_err(|error| Error::new(format!("demo capability entry: {error:?}")))?,
        );
    }
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest)
        .map_err(|error| Error::new(format!("demo capability manifest: {error:?}")))?;

    let mut input = MarketRunInput {
        generation: params.generation,
        collateral_display_decimals: 6,
        local_participant_fixture_liquidity_atoms: params.local_participant_fixture_liquidity_atoms,
        initial_collateral_atoms: 1_000_000_000,
        product_id: hex(&product_identity),
        coordinate_domain_id: hex(&coordinate_domain),
        result_unit_id: hex(&result_unit),
        claim_basis_id: hex(&claim_basis),
        liability_basis_id: hex(&liability_basis),
        representation_release_id: hex(&representation),
        mapping_release_id: hex(&mapping),
        cut_denominator,
        cuts: cuts.iter().map(|cut| cut.to_string()).collect(),
        portfolio_denominator: 1,
        coefficients,
        primary_source_spec_id: hex(&primary_source),
        window_spec_id: hex(&window),
        statistic_spec_id: hex(&statistic),
        failure_policy_release_id: hex(&failure_policy),
        source_spec_hex: hex(&source_spec_bytes),
        source_capacity_profile_hex: hex(&capacity.to_bytes()),
        manipulation_floor_hex: String::new(),
        window_spec_hex: hex(&window_bytes),
        statistic_spec_hex: hex(&statistic_bytes),
        provider_release_hex: hex(&provider_release_bytes),
        pyth_adapter_config_hex: hex(&adapter_config_bytes),
        pyth_sponsored_push_release_hex: params.release.sponsored_release_hex(),
        recovery_policy_hex: hex(&recovery_bytes),
        capability_manifest_hex: hex(&manifest),
        direct_capability: None,
        linked_basis_hex: hex(&linked_basis),
    };
    attach_direct_market_capability_v1(&mut input, direct)?;
    validate_market_input(&input)?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_found_snapshot_pins_manifest_and_runtime_rent_coordinates() {
        let manifest = Pubkey::new_unique();
        let mut keys = (0..FOUND_ACCOUNT_COUNT_V3)
            .map(|_| Pubkey::new_unique())
            .collect::<Vec<_>>();
        keys[FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3] = manifest;
        keys[FOUND_RENT_SYSVAR_INDEX_V3] = sysvar::rent::ID;
        authenticate_found_snapshot_coordinates_v3(&keys, manifest)
            .expect("canonical Found37 coordinates");

        let mut wrong_rent = keys.clone();
        wrong_rent[FOUND_RENT_SYSVAR_INDEX_V3] = Pubkey::new_unique();
        assert!(authenticate_found_snapshot_coordinates_v3(&wrong_rent, manifest).is_err());

        let mut missing_rent = keys;
        missing_rent.remove(FOUND_RENT_SYSVAR_INDEX_V3);
        assert!(authenticate_found_snapshot_coordinates_v3(&missing_rent, manifest).is_err());
    }

    fn sponsored_price_update_for_test() -> Vec<u8> {
        let release = dclutch_pyth_svm::devnet_sponsored_sol_usd_release_v1()
            .expect("compiled sponsored release");
        let mut body = FIXTURE_PRICE_UPDATE.to_vec();
        body[8..40].copy_from_slice(&release.price_account());
        body[41..73].copy_from_slice(&release.feed_id());
        body
    }

    fn sponsored_market_for_test() -> MarketRunInput {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("test Direct deployment widths"),
        );
        let price = sponsored_price_update_for_test();
        let update = dclutch_pyth_svm::FullPriceUpdateV2::parse(&price).expect("price update");
        devnet_sponsored_market_input(
            DevnetPythMarketSpecV1 {
                registry,
                price_update: &price,
                product_name: "product/sol-usd-sponsored-range-protection",
                coordinate_domain_name: "coordinate-domain/usd-cents-per-sol",
                feed_label: b"sol-usd-sponsored",
                window_start: update.publish_time() - 1_800,
                window_width_seconds: 1_800,
                max_age_seconds: 7_200,
                cut_denominator: 100,
                cuts: vec![12_000, 18_000],
                coefficients: vec![1, 0, 1, 0],
                generation: 1,
            },
            direct.compiler(),
        )
        .expect("sponsored market input")
    }

    #[test]
    fn sponsored_market_compiles_one_exact_source_provider_release_graph() {
        let input = sponsored_market_for_test();
        validate_market_input(&input).expect("canonical sponsored input");
        let publication = authenticate_source_publication_v1(&input).expect("publication contract");
        assert_eq!(
            publication.adapter_config_schema,
            PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1
        );
        assert_eq!(
            publication.sponsored_release.as_deref(),
            Some(
                decode_hex(&input.pyth_sponsored_push_release_hex)
                    .expect("published release")
                    .as_slice()
            )
        );
        let source = SourceSpecV1::decode(&decode_hex(&input.source_spec_hex).expect("source hex"))
            .expect("source");
        let provider = ProviderReleaseV1::decode(
            &decode_hex(&input.provider_release_hex).expect("provider hex"),
        )
        .expect("provider");
        let release_bytes =
            decode_hex(&input.pyth_sponsored_push_release_hex).expect("sponsored release hex");
        let release =
            PythSponsoredPushReleaseV1::decode(&release_bytes).expect("sponsored release");
        assert_eq!(
            source.access_profile(),
            SourceAccessProfile::PythSponsoredPushSnapshot
        );
        assert_eq!(
            provider.provider_deployment_release_id().to_bytes(),
            record_identity(&release_bytes)
        );
        assert_eq!(
            provider.provider_family_id().to_bytes(),
            release.provider_family_id()
        );
        assert_eq!(
            provider.adapter_release_id().to_bytes(),
            release.adapter_id()
        );
        assert_eq!(
            provider.decoding_rules_id().to_bytes(),
            release.price_update_codec_id()
        );
        assert_eq!(
            provider.transport_profile_id().to_bytes(),
            release.transport_profile_id()
        );
    }

    #[test]
    fn sponsored_market_refuses_absent_or_substituted_release_facts() {
        let canonical = sponsored_market_for_test();

        let mut absent = canonical.clone();
        absent.pyth_sponsored_push_release_hex.clear();
        assert!(validate_market_input(&absent).is_err());

        // These offsets are semantic fields of the fixed-layout release, not
        // fuzz bytes: Receiver ProgramData, push deployment slot, and feed.
        for (offset, label) in [(80, "ProgramData"), (568, "slot"), (272, "feed")] {
            let mut substituted = canonical.clone();
            let mut release =
                decode_hex(&substituted.pyth_sponsored_push_release_hex).expect("release hex");
            release[offset] ^= 1;
            substituted.pyth_sponsored_push_release_hex = hex(&release);
            assert!(
                validate_market_input(&substituted).is_err(),
                "substituted {label} must refuse"
            );
        }
    }

    #[test]
    fn sponsored_market_refuses_source_profile_substitution_and_terminal_stays_legacy() {
        let mut substituted = sponsored_market_for_test();
        let mut source = decode_hex(&substituted.source_spec_hex).expect("source hex");
        source[10] = SourceAccessProfile::PythTerminalOneTransaction as u8;
        substituted.source_spec_hex = hex(&source);
        substituted.primary_source_spec_id = hex(&record_identity(&source));
        assert!(validate_market_input(&substituted).is_err());

        let registry = Pubkey::new_from_array([0x42; 32]);
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("test Direct deployment widths"),
        );
        let terminal = demo_market_input(registry, direct.compiler()).expect("terminal input");
        assert!(terminal.pyth_sponsored_push_release_hex.is_empty());
        assert!(
            serde_json::to_value(&terminal)
                .expect("terminal JSON")
                .get("pyth_sponsored_push_release_hex")
                .is_none(),
            "the optional sponsored field must not change legacy serialized inputs"
        );
        validate_market_input(&terminal).expect("legacy terminal input");
    }

    #[test]
    fn source_abort_expiry_policy_is_exactly_owned_loopback_only() {
        let registry = Pubkey::new_unique();
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("test Direct deployment widths"),
        );
        let mut input = demo_market_input(registry, direct.compiler()).expect("local market");
        let local = SourceAbortExpiryPolicyV1::from_input(&input).expect("local policy");
        assert_eq!(local, SourceAbortExpiryPolicyV1::OwnedLoopback);
        assert_eq!(local.expiry_slots(), 576);
        assert_eq!(local.minimum_staging_margin_slots(), 160);
        assert_eq!(local.minimum_pre_expiry_refusal_margin_slots(), 64);

        input.local_participant_fixture_liquidity_atoms = 0;
        let public = SourceAbortExpiryPolicyV1::from_input(&input).expect("public policy");
        assert_eq!(public, SourceAbortExpiryPolicyV1::PublicDevnet);
        assert_eq!(public.expiry_slots(), 900);
        assert_eq!(public.minimum_staging_margin_slots(), 64);
        assert_eq!(public.minimum_pre_expiry_refusal_margin_slots(), 0);

        input.local_participant_fixture_liquidity_atoms =
            LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1 - 1;
        assert!(SourceAbortExpiryPolicyV1::from_input(&input).is_err());
    }

    #[test]
    fn success_policy_excludes_source_abort_and_prepared_auth_preserves_role_indices() {
        assert_eq!(SUCCESS_PRESTATE_LANES_V1, [PrestateLaneV1::Founding]);

        let forge = KeyForge::parse(
            Some("1111111111111111111111111111111111111111111111111111111111111111"),
            "http://127.0.0.1:8899",
        )
        .expect("loopback seeded forge");
        let expected = prepared_resume_role_pubkeys_v1(&forge).expect("read-only role projection");
        assert_eq!(
            forge.keypair(role::FOUNDING_BENEFICIARY).pubkey(),
            expected.0,
            "Prepared authentication must not advance the beneficiary role"
        );
        assert_eq!(
            forge.keypair(role::FOUNDING_SOURCE_FUNDER).pubkey(),
            expected.1,
            "Prepared authentication must not advance the source-funder role"
        );
        assert_eq!(
            forge.keypair(role::FOUNDING_PROJECTION_WITNESS).pubkey(),
            expected.2,
            "Prepared authentication must not advance the projection-witness role"
        );
    }

    #[test]
    fn founding_actor_owner_refuses_alias_without_requiring_secret_material() {
        let founder = Pubkey::new_from_array([0x41; 32]);
        let substituted = Pubkey::new_from_array([0x42; 32]);
        assert_eq!(
            FoundingActorsV1::new(founder, substituted).expect("distinct actors"),
            FoundingActorsV1 {
                founder,
                substituted_founder: substituted,
            }
        );
        assert!(
            FoundingActorsV1::new(founder, founder).is_err(),
            "the hostile founder identity must not alias the honest founder"
        );
    }

    #[test]
    fn owned_loopback_source_abort_preserves_both_pre_expiry_dispatch_margins() {
        let policy = SourceAbortExpiryPolicyV1::OwnedLoopback;
        let start = 1_000;
        let expiry = start + policy.expiry_slots();

        // Twelve finalized barriers precede DCLTPCB2 staging. Four more carry
        // the stage, the DCLTPCA1 table, and the hostile rollback proof to
        // finality. Both boundaries retain one full 32-slot reserve window.
        let before_stage = start + 12 * SourceAbortExpiryPolicyV1::LOCAL_FINALITY_WINDOW_SLOTS_V1;
        require_expiry_margin_v1(
            "staging",
            before_stage,
            expiry,
            policy.minimum_staging_margin_slots(),
        )
        .expect("measured local staging budget");
        let before_refusal = start + 15 * SourceAbortExpiryPolicyV1::LOCAL_FINALITY_WINDOW_SLOTS_V1;
        require_expiry_margin_v1(
            "rollback probe",
            before_refusal,
            expiry,
            policy.minimum_pre_expiry_refusal_margin_slots(),
        )
        .expect("measured local refusal-dispatch budget");

        assert!(
            require_expiry_margin_v1(
                "staging",
                expiry - policy.minimum_staging_margin_slots(),
                expiry,
                policy.minimum_staging_margin_slots(),
            )
            .is_err()
        );
        assert!(
            require_expiry_margin_v1(
                "rollback probe",
                expiry - policy.minimum_pre_expiry_refusal_margin_slots(),
                expiry,
                policy.minimum_pre_expiry_refusal_margin_slots(),
            )
            .is_err()
        );
        assert!(require_expiry_margin_v1("overflow", u64::MAX, expiry, 1).is_err());

        assert!(require_expiry_margin_v1("public", expiry - 1, expiry, 0).is_ok());
        assert!(require_expiry_margin_v1("public", expiry, expiry, 0).is_err());
    }

    fn direct_controller_manifest(
        direct_index: Option<usize>,
        other_foreign: Option<usize>,
    ) -> Vec<u8> {
        use dclutch_capability_contract::{
            ActivationPolicy, CAPABILITY_ENTRY_BYTES, CompartmentFundingV1, FundingAmountsV1,
            FundingQuoteV1, MANIFEST_HEADER_BYTES,
        };
        let none = CompartmentFundingV1::not_applicable();
        let quote = FundingQuoteV1::new(
            FundingAmountsV1::new(none, none, none, none, none, none, none).expect("amounts"),
            None,
        )
        .expect("quote");
        let mut entries = Vec::new();
        for index in 0_usize..4 {
            let byte = u8::try_from(index).expect("bounded index");
            let is_direct = direct_index == Some(index);
            entries.push(
                CapabilityEntryV1::new(
                    CapabilityContentId::new(if is_direct {
                        DIRECT_SUCCESSOR_KIND_ID_V3
                    } else if direct_index.is_some_and(|selected| index < selected) {
                        [0x10 + byte; 32]
                    } else {
                        [0x80 + byte; 32]
                    })
                    .expect("kind"),
                    CapabilityContentId::new(if is_direct || other_foreign == Some(index) {
                        [0x31; 32]
                    } else {
                        [0x30; 32]
                    })
                    .expect("release"),
                    CapabilityContentId::new([0x40 + byte; 32]).expect("config"),
                    CapabilityContentId::new([0x50 + byte; 32]).expect("capacity"),
                    CapabilityContentId::new([0x60; 32]).expect("schema"),
                    CapabilityContentId::new([0x70; 32]).expect("derivation"),
                    ActivationPolicy::RequiredAtFounding,
                    0,
                    0,
                    [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                    quote,
                )
                .expect("entry"),
            );
        }
        let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("manifest");
        bytes
    }

    #[test]
    fn successor_controller_masks_follow_direct_identity_at_every_position() {
        for direct_index in 0_usize..4 {
            let bytes = direct_controller_manifest(Some(direct_index), None);
            let direct_mask = 1_u16 << direct_index;
            assert_eq!(
                direct_founding_controller_masks_v1(
                    CapabilityManifestV1::decode(&bytes).expect("manifest"),
                    [0x30; 32],
                )
                .expect("partition"),
                (
                    u16::try_from(direct_index).expect("index"),
                    [0b1111 ^ direct_mask, direct_mask],
                ),
            );
        }
    }

    #[test]
    fn successor_controller_masks_refuse_missing_or_ambiguous_direct_ownership() {
        for bytes in [
            direct_controller_manifest(None, None),
            direct_controller_manifest(Some(3), Some(1)),
        ] {
            assert!(
                direct_founding_controller_masks_v1(
                    CapabilityManifestV1::decode(&bytes).expect("manifest"),
                    [0x30; 32],
                )
                .is_err()
            );
        }
    }

    #[test]
    fn compiled_demo_manifest_uses_exact_resolution_release_and_refuses_substitution() {
        let registry = Pubkey::new_unique();
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("test Direct deployment widths"),
        );
        let input = demo_market_input(registry, direct.compiler()).expect("demo market input");
        let bytes = decode_hex(&input.capability_manifest_hex).expect("manifest bytes");
        let manifest = CapabilityManifestV1::decode(&bytes).expect("manifest");
        let expected = dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V7;
        assert!(direct_founding_controller_masks_v1(manifest, expected).is_ok());
        assert!(
            direct_founding_controller_masks_v1(
                manifest,
                dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V5,
            )
            .is_err()
        );
        assert!(direct_founding_controller_masks_v1(manifest, [0x5a; 32]).is_err());
    }

    fn projected_bootstrap_census_fixture_v2() -> (Pubkey, Instruction) {
        let payer = Pubkey::new_from_array([1; 32]);
        let beneficiary = Pubkey::new_from_array([2; 32]);
        let program_id = Pubkey::new_from_array([3; 32]);
        let distinct = (0_u8..56)
            .map(|index| Pubkey::new_from_array([index.saturating_add(4); 32]))
            .collect::<Vec<_>>();
        let mut accounts = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(beneficiary, true),
        ];
        for (index, key) in distinct.iter().enumerate() {
            accounts.push(if index < 7 {
                AccountMeta::new(*key, false)
            } else {
                AccountMeta::new_readonly(*key, false)
            });
        }
        let unique_width = accounts.len();
        while accounts.len() < PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNTS_V2 {
            accounts.push(accounts[accounts.len() % unique_width].clone());
        }
        (
            payer,
            Instruction {
                program_id,
                accounts,
                data: PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V2.to_vec(),
            },
        )
    }

    fn generic_market_founding_census_fixture_v3() -> (Pubkey, PreparedGenericMarketFoundingV3) {
        let payer = Pubkey::new_from_array([0xf1; 32]);
        let program_id = Pubkey::new_from_array([0xf2; 32]);
        let distinct = (0_u8..55)
            .map(|index| Pubkey::new_from_array([index.saturating_add(1); 32]))
            .collect::<Vec<_>>();
        let mut accounts = distinct
            .iter()
            .enumerate()
            .map(|(index, key)| {
                if index < 12 {
                    AccountMeta::new(*key, false)
                } else {
                    AccountMeta::new_readonly(*key, false)
                }
            })
            .collect::<Vec<_>>();
        let unique_width = accounts.len();
        while accounts.len()
            < GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3
                + GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3
        {
            accounts.push(accounts[accounts.len() % unique_width].clone());
        }
        let mut data = GENERIC_MARKET_FOUNDING_MAGIC_V3.to_vec();
        data.extend_from_slice(&[1, 2, 3, 4, 5]);
        let instruction = Instruction {
            program_id,
            accounts,
            data,
        };
        let lock_expectation = GenericMarketFoundingLockExpectationV3 {
            frame_digest: exact_instruction_frame_digest_v1(&instruction),
        };
        (
            payer,
            PreparedGenericMarketFoundingV3 {
                instruction,
                lock_expectation,
            },
        )
    }

    fn controller_funding_prepare_census_fixture_v1() -> (Pubkey, Instruction) {
        let payer = Pubkey::new_from_array([0xc1; 32]);
        let projection_witness = Pubkey::new_from_array([0xc2; 32]);
        let program_id = Pubkey::new_from_array([0xc3; 32]);
        let mut accounts = (0_u8..CONTROLLER_FUNDING_PREPARE_ACCOUNTS_V1 as u8)
            .map(|index| {
                let key = Pubkey::new_from_array([index.saturating_add(1); 32]);
                if matches!(index, 8 | 9 | 10 | 11 | 12 | 14) {
                    AccountMeta::new(key, false)
                } else {
                    AccountMeta::new_readonly(key, false)
                }
            })
            .collect::<Vec<_>>();
        accounts[6].pubkey = program_id;
        accounts[CONTROLLER_FUNDING_PREPARE_FUNDING_SOURCE_V1] = AccountMeta::new(payer, true);
        accounts[CONTROLLER_FUNDING_PREPARE_FOUND_START_V1] =
            AccountMeta::new(projection_witness, true);
        (
            payer,
            Instruction {
                program_id,
                accounts,
                data: CONTROLLER_FUNDING_PREPARE_MAGIC_V1.to_vec(),
            },
        )
    }

    fn controller_funding_cleanup_census_fixture_v1(data: [u8; 8]) -> (Pubkey, Instruction) {
        let payer = Pubkey::new_from_array([0xd1; 32]);
        let program_id = Pubkey::new_from_array([0xd2; 32]);
        let mut accounts = (0_u8..CONTROLLER_FUNDING_ABORT_ACCOUNTS_V1 as u8)
            .map(|index| {
                let key = Pubkey::new_from_array([index.saturating_add(1); 32]);
                if matches!(index, 5 | 6 | 7 | 8 | 16) {
                    AccountMeta::new(key, false)
                } else {
                    AccountMeta::new_readonly(key, false)
                }
            })
            .collect::<Vec<_>>();
        accounts[1].pubkey = program_id;
        // The funding source is the payer since 5ca145e8 de-aliased it: one
        // coordinate references an already-counted key, so the frame carries
        // 18 distinct complete keys, not 19. Any in-bounds slot other than
        // the program alias at [1] models the same census; the guard keeps a
        // frame-width change from silently pushing the alias out of range.
        const PAYER_ALIAS_INDEX: usize = 11;
        assert!(PAYER_ALIAS_INDEX < CONTROLLER_FUNDING_ABORT_ACCOUNTS_V1 && PAYER_ALIAS_INDEX != 1);
        accounts[PAYER_ALIAS_INDEX] = AccountMeta::new(payer, true);
        (
            payer,
            Instruction {
                program_id,
                accounts,
                data: data.to_vec(),
            },
        )
    }

    fn staged_abort_census_fixture_v1() -> (Pubkey, Instruction) {
        let payer = Pubkey::new_from_array([0xe1; 32]);
        let beneficiary = Pubkey::new_from_array([0xe2; 32]);
        let program_id = Pubkey::new_from_array([0xe3; 32]);
        let distinct = (0_u8..31)
            .map(|index| Pubkey::new_from_array([index.saturating_add(1); 32]))
            .collect::<Vec<_>>();
        let mut accounts = distinct
            .iter()
            .enumerate()
            .map(|(index, key)| {
                if index < 11 {
                    AccountMeta::new(*key, false)
                } else {
                    AccountMeta::new_readonly(*key, false)
                }
            })
            .collect::<Vec<_>>();
        accounts[7].pubkey = program_id;
        accounts[14] = AccountMeta::new_readonly(beneficiary, true);
        let unique_width = accounts.len();
        while accounts.len() < PROJECTED_CUSTODY_ABORT_ACCOUNTS_V1 {
            accounts.push(accounts[accounts.len() % unique_width].clone());
        }
        (
            payer,
            Instruction {
                program_id,
                accounts,
                data: PROJECTED_CUSTODY_ABORT_MAGIC_V1.to_vec(),
            },
        )
    }

    fn funding_readiness_census_fixture_v1(
        account_count: usize,
        program_index: usize,
        system_index: Option<usize>,
        writable: &[usize],
        data_len: usize,
    ) -> (Pubkey, Instruction) {
        let payer = Pubkey::new_from_array([0xb1; 32]);
        let mut accounts = (0..account_count)
            .map(|index| {
                let key = Pubkey::new_from_array(
                    [u8::try_from(index + 1).expect("fixture key fits u8"); 32],
                );
                if writable.contains(&index) {
                    AccountMeta::new(key, false)
                } else {
                    AccountMeta::new_readonly(key, false)
                }
            })
            .collect::<Vec<_>>();
        let program_id = accounts[program_index].pubkey;
        if let Some(system_index) = system_index {
            accounts[system_index].pubkey = system_program::ID;
        }
        // Keep the destination writable exactly as the real optional System
        // prepay does; System itself is already in every canonical frame.
        accounts[program_index].is_writable = false;
        (
            payer,
            Instruction {
                program_id,
                accounts,
                data: vec![0x5a; data_len],
            },
        )
    }

    fn frozen_funding_readiness_table_v1(
        payer: Pubkey,
        instructions: &[Instruction],
    ) -> ObservedAccount {
        use std::borrow::Cow;

        use solana_address_lookup_table_interface::state::LookupTableMeta;

        let addresses = canonical_routing_addresses_v1(payer, instructions);
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                deactivation_slot: u64::MAX,
                last_extended_slot: 1,
                last_extended_slot_start_index: 0,
                authority: None,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: Observation {
                slot: 2,
                unix_timestamp: 1,
                finality: dclutch_versioned_message_operator::Finality::Finalized,
            },
            key: Pubkey::new_from_array([0xb2; 32]),
            owner: solana_address_lookup_table_interface::program::ID,
            lamports: 1,
            executable: false,
            data: table.serialize_for_tests().expect("table bytes"),
        }
    }

    #[test]
    fn funding_readiness_routed_geometry_pins_packet_and_64_65_walls() {
        for (
            operation,
            account_count,
            program_index,
            system_index,
            writable,
            data_len,
            expected_message_bytes,
            expected_packet_bytes,
        ) in [
            (
                FoundingSubmissionOperationV1::CoreFundingCreateV1,
                18,
                4,
                Some(15),
                &[1_usize, 12][..],
                72 + 280 + 224,
                852,
                917,
            ),
            (
                FoundingSubmissionOperationV1::ResolutionFundingActivateV1,
                20,
                5,
                Some(17),
                &[12_usize, 13, 14][..],
                440,
                720,
                785,
            ),
            (
                FoundingSubmissionOperationV1::CoreFundingAcceptV1,
                20,
                4,
                None,
                &[1_usize][..],
                72 + 280 + 224,
                808,
                873,
            ),
        ] {
            let (payer, instruction) = funding_readiness_census_fixture_v1(
                account_count,
                program_index,
                system_index,
                writable,
                data_len,
            );
            let destination = match operation {
                FoundingSubmissionOperationV1::CoreFundingCreateV1 => {
                    Some(instruction.accounts[12].pubkey)
                }
                FoundingSubmissionOperationV1::ResolutionFundingActivateV1 => {
                    Some(instruction.accounts[14].pubkey)
                }
                FoundingSubmissionOperationV1::CoreFundingAcceptV1 => None,
                _ => unreachable!("readiness fixture operation"),
            };
            let instructions = funding_readiness_instructions_v1(
                payer,
                instruction,
                destination.map(|destination| FundingReadinessPrepayV1 {
                    destination,
                    lamports: 1,
                }),
            );
            let table = frozen_funding_readiness_table_v1(payer, &instructions);
            let geometry = authenticate_funding_readiness_compiled_geometry_v1(
                payer,
                operation,
                &instructions,
                table.observation,
                std::slice::from_ref(&table),
            )
            .expect("routed geometry");
            assert_eq!(geometry.complete_keys, operation.exact_unique_accounts());
            assert_eq!(geometry.required_signatures, 1);
            assert_eq!(geometry.message_bytes, expected_message_bytes);
            assert_eq!(geometry.packet_bytes, expected_packet_bytes);
        }
    }

    #[test]
    fn durable_recovery_rejoins_fresh_instruction_routing_and_completion_intent() {
        let operation = FoundingSubmissionOperationV1::CoreFundingCreateV1;
        let (payer, instruction) =
            funding_readiness_census_fixture_v1(18, 4, Some(15), &[1_usize, 12], 72 + 280 + 224);
        let destination = instruction.accounts[12].pubkey;
        let instructions = funding_readiness_instructions_v1(
            payer,
            instruction,
            Some(FundingReadinessPrepayV1 {
                destination,
                lamports: 1,
            }),
        );
        let table = frozen_funding_readiness_table_v1(payer, &instructions);
        let blockhash = Hash::new_from_array([0x73; 32]);
        let message = compile_current_founding_message_v1(
            operation.label(),
            payer,
            &instructions,
            table.observation,
            std::slice::from_ref(&table),
            None,
            blockhash,
        )
        .expect("current message");
        let binding = FoundingSubmissionBindingV1::new(
            "devnet",
            crate::cluster::DEVNET_GENESIS_HASH,
            std::path::Path::new("/tmp/dclutch-readiness-intent.json"),
            "https://api.devnet.solana.com/",
            "11".repeat(32),
            "22".repeat(32),
            payer,
        )
        .expect("binding");
        let resolved_digest = "33".repeat(32);
        let prestate = vec![destination];
        let completion = vec![destination];
        let recovery = b"exact readiness recovery".to_vec();
        let journal = plan_founding_submission_v1(
            &binding,
            FoundingSubmissionPlanV1 {
                operation,
                message,
                last_valid_block_height: 900,
                exact_fee_lamports: 5_000,
                expected_signers: vec![payer],
                resolved_accounts_sha256: resolved_digest.clone(),
                prestate_accounts: prestate.clone(),
                prestate_sha256: "44".repeat(32),
                completion_accounts: completion.clone(),
                completion_contract_sha256: founding_completion_contract_v1(operation, &completion)
                    .expect("completion"),
                recovery_payload: recovery.clone(),
            },
        )
        .expect("journal");
        let authenticate = |instructions: &[Instruction],
                            resolved: &str,
                            completion: &[Pubkey],
                            recovery: &[u8]| {
            authenticate_current_founding_intent_v1(
                operation.label(),
                operation,
                instructions,
                &[payer],
                table.observation,
                std::slice::from_ref(&table),
                resolved,
                &prestate,
                completion,
                recovery,
                None,
                &binding,
                &journal,
            )
        };
        authenticate(&instructions, &resolved_digest, &completion, &recovery)
            .expect("exact current intent");

        let mut changed_instruction = instructions.clone();
        changed_instruction
            .last_mut()
            .expect("protocol instruction")
            .data[0] ^= 1;
        assert!(
            authenticate(
                &changed_instruction,
                &resolved_digest,
                &completion,
                &recovery
            )
            .is_err()
        );
        assert!(authenticate(&instructions, &"55".repeat(32), &completion, &recovery).is_err());
        assert!(
            authenticate(
                &instructions,
                &resolved_digest,
                &[Pubkey::new_unique()],
                &recovery
            )
            .is_err()
        );
        assert!(
            authenticate(
                &instructions,
                &resolved_digest,
                &completion,
                b"substituted recovery"
            )
            .is_err()
        );
    }

    #[test]
    fn controller_funding_prepare_compiler_census_pins_the_49_64_65_wall() {
        let (payer, prepare) = controller_funding_prepare_census_fixture_v1();
        let projection_witness = prepare.accounts[CONTROLLER_FUNDING_PREPARE_FOUND_START_V1].pubkey;
        authenticate_controller_funding_prepare_frame_v1(
            &prepare.accounts,
            payer,
            projection_witness,
        )
        .expect("canonical 48-account prepare frame");

        let mut aliased_source = prepare.accounts.clone();
        aliased_source[CONTROLLER_FUNDING_PREPARE_FUNDING_SOURCE_V1].pubkey = projection_witness;
        assert!(
            authenticate_controller_funding_prepare_frame_v1(
                &aliased_source,
                projection_witness,
                projection_witness,
            )
            .is_err(),
            "the controller funding source must not alias the ProjectFound payer"
        );

        let mut readonly_rent_credit = prepare.accounts.clone();
        readonly_rent_credit[CONTROLLER_FUNDING_PREPARE_FOUND_RENT_CREDIT_V1].is_writable = false;
        assert!(
            authenticate_controller_funding_prepare_frame_v1(
                &readonly_rent_credit,
                payer,
                projection_witness,
            )
            .is_err(),
            "Resolution must receive the ProjectFound RentCredit as writable"
        );

        let base = projected_bootstrap_compiled_geometry_v2(payer, &prepare).expect("base census");
        let admitted = projected_bootstrap_compiled_geometry_v2(
            payer,
            &append_distinct_census_accounts_v1(&prepare, 15),
        )
        .expect("64-key census");
        let refused = projected_bootstrap_compiled_geometry_v2(
            payer,
            &append_distinct_census_accounts_v1(&prepare, 16),
        )
        .expect("65-key census");
        assert_eq!(
            base.complete_keys,
            CONTROLLER_FUNDING_PREPARE_COMPLETE_KEYS_V1
        );
        assert_eq!(base.required_signatures, 2);
        assert_eq!(base.static_keys, 4);
        assert_eq!(base.loaded_writable, 4);
        assert_eq!(base.loaded_readonly, 41);
        assert_eq!(base.message_bytes, 333);
        assert_eq!(base.packet_bytes, 462);
        assert_eq!(admitted.complete_keys, DEVNET_ACCOUNT_LOCK_LIMIT_V1);
        assert_eq!(refused.complete_keys, DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1);
    }

    #[test]
    fn controller_cleanup_compiler_census_pins_both_18_64_65_walls() {
        for data in [
            CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1,
            CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1,
        ] {
            let (payer, instruction) = controller_funding_cleanup_census_fixture_v1(data);
            let base = projected_bootstrap_compiled_geometry_v2(payer, &instruction)
                .expect("cleanup base census");
            authenticate_cleanup_compiled_census_v1(payer, &instruction, base)
                .expect("cleanup boundary census");
            assert_eq!(base.complete_keys, CONTROLLER_FUNDING_CLEANUP_COMPLETE_KEYS_V1);
            assert_eq!(base.required_signatures, 1);
            assert_eq!(base.static_keys, 3);
            assert_eq!(base.loaded_writable, 5);
            assert_eq!(base.loaded_readonly, 10);
            assert_eq!(base.message_bytes, 240);
            assert_eq!(base.packet_bytes, 305);
        }
    }

    #[test]
    fn staged_abort_compiler_census_pins_the_33_64_65_wall() {
        let (payer, instruction) = staged_abort_census_fixture_v1();
        let base = projected_bootstrap_compiled_geometry_v2(payer, &instruction)
            .expect("staged abort base census");
        let admitted = projected_bootstrap_compiled_geometry_v2(
            payer,
            &append_distinct_census_accounts_v1(&instruction, 31),
        )
        .expect("64-key staged abort census");
        let refused = projected_bootstrap_compiled_geometry_v2(
            payer,
            &append_distinct_census_accounts_v1(&instruction, 32),
        )
        .expect("65-key staged abort census");
        assert_eq!(base.complete_keys, PROJECTED_CUSTODY_ABORT_COMPLETE_KEYS_V1);
        assert_eq!(base.required_signatures, 2);
        assert_eq!(base.static_keys, 4);
        assert_eq!(base.loaded_writable, 10);
        assert_eq!(base.loaded_readonly, 19);
        assert_eq!(base.message_bytes, 305);
        assert_eq!(base.packet_bytes, 434);
        assert_eq!(admitted.complete_keys, DEVNET_ACCOUNT_LOCK_LIMIT_V1);
        assert_eq!(refused.complete_keys, DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1);
    }

    #[test]
    fn projected_bootstrap_actual_compiler_census_pins_the_60_64_65_wall() {
        let (payer, base) = projected_bootstrap_census_fixture_v2();
        let base_geometry =
            projected_bootstrap_compiled_geometry_v2(payer, &base).expect("base census");
        let admitted = projected_bootstrap_compiled_geometry_v2(
            payer,
            &append_distinct_census_accounts_v1(&base, 4),
        )
        .expect("64-key census");
        let refused = projected_bootstrap_compiled_geometry_v2(
            payer,
            &append_distinct_census_accounts_v1(&base, 5),
        )
        .expect("65-key census");

        assert_eq!(base_geometry.complete_keys, 60);
        assert_eq!(base_geometry.required_signatures, 2);
        assert_eq!(base_geometry.static_keys, 4);
        assert_eq!(base_geometry.loaded_writable, 7);
        assert_eq!(base_geometry.loaded_readonly, 49);
        assert_eq!(base_geometry.message_bytes, 383);
        assert_eq!(base_geometry.packet_bytes, 512);
        assert_eq!(admitted.complete_keys, 64);
        assert_eq!(admitted.message_bytes, 391);
        assert_eq!(admitted.packet_bytes, 520);
        assert_eq!(refused.complete_keys, 65);
        assert_eq!(refused.message_bytes, 393);
        assert_eq!(refused.packet_bytes, 522);
    }

    #[test]
    fn generic_founding_final_compiler_census_pins_the_58_key_shape() {
        let (payer, prepared) = generic_market_founding_census_fixture_v3();
        let census = authenticate_generic_market_founding_lock_census_v3(payer, &prepared)
            .expect("canonical DCLTGMF3 census");
        assert_eq!(census.complete_keys, 58);
        assert_eq!(census.required_signatures, 1);
        assert_eq!(census.static_keys, 3);
        assert_eq!(census.loaded_writable, 12);
        assert_eq!(census.loaded_readonly, 43);
        assert_eq!(
            hex(&census.key_privilege_digest),
            "8fb27f15c8509350a0702a1c6e3208ade60d6c16b48bb6d324cc721a08186561"
        );
        assert_eq!(
            census,
            authenticate_generic_market_founding_lock_census_v3(payer, &prepared)
                .expect("deterministic census")
        );
    }

    #[test]
    fn generic_founding_complete_key_census_enforces_the_64_65_wall() {
        let (payer, prepared) = generic_market_founding_census_fixture_v3();
        let admitted = compiled_complete_lock_census_v1(
            payer,
            &append_distinct_census_accounts_v1(&prepared.instruction, 6),
        )
        .expect("64-key census");
        let refused = compiled_complete_lock_census_v1(
            payer,
            &append_distinct_census_accounts_v1(&prepared.instruction, 7),
        )
        .expect("65-key census");
        assert_eq!(admitted.complete_keys, DEVNET_ACCOUNT_LOCK_LIMIT_V1);
        assert!(require_devnet_complete_key_limit_v1(admitted).is_ok());
        assert_eq!(refused.complete_keys, DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1);
        assert!(require_devnet_complete_key_limit_v1(refused).is_err());
    }

    #[test]
    fn generic_founding_65_key_drift_plans_zero_writes_or_transactions() {
        let (payer, mut prepared) = generic_market_founding_census_fixture_v3();
        let width = prepared.instruction.accounts.len();
        for offset in 0_u8..7 {
            prepared.instruction.accounts[width - 1 - usize::from(offset)].pubkey =
                Pubkey::new_from_array([0xa0 + offset; 32]);
        }
        prepared.lock_expectation.frame_digest =
            exact_instruction_frame_digest_v1(&prepared.instruction);
        let drifted = compiled_complete_lock_census_v1(payer, &prepared.instruction)
            .expect("same-width 65-key census");
        assert_eq!(drifted.complete_keys, DEVNET_ACCOUNT_LOCK_LIMIT_V1 + 1);

        let mut planned_writes = Vec::<Pubkey>::new();
        let mut planned_transactions = Vec::<Instruction>::new();
        let admission = (|| -> Result<()> {
            authenticate_generic_market_founding_lock_census_v3(payer, &prepared)?;
            planned_writes.push(prepared.instruction.accounts[0].pubkey);
            planned_transactions.push(prepared.instruction.clone());
            Ok(())
        })();
        assert!(admission.is_err());
        assert!(planned_writes.is_empty());
        assert!(planned_transactions.is_empty());
    }

    #[test]
    fn generic_founding_census_refuses_substitution_duplicate_order_and_privilege_drift() {
        let (payer, prepared) = generic_market_founding_census_fixture_v3();

        let mut substituted = prepared.clone();
        substituted.instruction.accounts[0].pubkey = Pubkey::new_from_array([0xe1; 32]);
        assert!(authenticate_generic_market_founding_lock_census_v3(payer, &substituted).is_err());

        let mut duplicate = prepared.clone();
        let removed = duplicate.instruction.accounts[55].pubkey;
        let retained = duplicate.instruction.accounts[54].pubkey;
        for meta in &mut duplicate.instruction.accounts {
            if meta.pubkey == removed {
                meta.pubkey = retained;
            }
        }
        assert!(authenticate_generic_market_founding_lock_census_v3(payer, &duplicate).is_err());
        duplicate.lock_expectation.frame_digest =
            exact_instruction_frame_digest_v1(&duplicate.instruction);
        assert!(authenticate_generic_market_founding_lock_census_v3(payer, &duplicate).is_err());

        let mut reordered = prepared.clone();
        reordered.instruction.accounts.swap(0, 1);
        assert!(authenticate_generic_market_founding_lock_census_v3(payer, &reordered).is_err());

        let mut privilege = prepared.clone();
        privilege.instruction.accounts[12].is_writable = true;
        assert!(authenticate_generic_market_founding_lock_census_v3(payer, &privilege).is_err());
        privilege.lock_expectation.frame_digest =
            exact_instruction_frame_digest_v1(&privilege.instruction);
        assert!(authenticate_generic_market_founding_lock_census_v3(payer, &privilege).is_err());
    }

    #[test]
    fn founding_targets_derive_three_distinct_stable_markets_offline() {
        let registry = Pubkey::new_unique();
        let core = Pubkey::new_unique();
        let release_set = [7_u8; 32];
        let mint = Pubkey::new_unique();
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("test Direct deployment widths"),
        );
        let input = demo_market_input(registry, direct.compiler()).expect("demo market input");
        let first = derive_founding_targets_inner(registry, core, release_set, &input, mint)
            .expect("founding targets");
        let second = derive_founding_targets_inner(registry, core, release_set, &input, mint)
            .expect("founding targets again");
        // Deterministic: the detector must read the same coordinates on every
        // resumed run.
        assert_eq!(first.open_market, second.open_market);
        assert_eq!(first.realm_record, second.realm_record);
        // Three generations, three distinct still-vacant PDAs.
        assert_ne!(first.found31_market, first.open_market);
        assert_ne!(first.open_market, first.abort_market);
        assert_ne!(first.found31_market, first.abort_market);
        // The completed identity round-trips to the same address, and its
        // market_id is the address itself.
        assert_eq!(
            first.open_market_identity.market_id.to_bytes(),
            first.open_market.to_bytes()
        );
        // The mint is inside the Realm body, so a different collateral mint is
        // a different Realm record and a different Market.
        let other_mint = derive_founding_targets_inner(
            registry,
            core,
            release_set,
            &input,
            Pubkey::new_unique(),
        )
        .expect("founding targets, other mint");
        assert_ne!(other_mint.realm_record, first.realm_record);
        assert_ne!(other_mint.open_market, first.open_market);
    }

    #[test]
    fn cut_parser_refuses_noncanonical_integer_spellings() {
        assert_eq!(canonical_i128("-2").expect("canonical"), -2);
        assert_eq!(canonical_i128("0").expect("canonical"), 0);
        for value in ["+1", "01", "-0", " 1", "1 "] {
            assert!(canonical_i128(value).is_err(), "{value}");
        }
    }

    #[test]
    fn mint_instruction_shapes_are_exact_and_do_not_convert_raw_atoms() {
        let authority = Keypair::new();
        let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        let mint = Keypair::new();
        let wallet = Keypair::new();
        let atoms = 9_007_199_254_740_993_u64;
        let decimals = 255_u8;
        let mut mint_to = Vec::with_capacity(10);
        mint_to.push(14);
        mint_to.extend_from_slice(&atoms.to_le_bytes());
        mint_to.push(decimals);
        assert_eq!(mint_to.len(), 10);
        assert_eq!(&mint_to[1..9], &atoms.to_le_bytes());
        assert_eq!(mint_to[9], decimals);
        assert_ne!(authority.pubkey(), mint.pubkey());
        assert_ne!(authority.pubkey(), wallet.pubkey());
        assert_ne!(mint.pubkey(), wallet.pubkey());
        assert_ne!(token_program, system_program::ID);
    }

    #[test]
    fn local_fixture_supply_is_exact_separate_and_permanently_immutable() {
        let founding = 1_000_000_000;
        let fixture = LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1;
        assert_eq!(
            authenticate_collateral_supply_partition_v1(
                founding,
                fixture,
                1_100_000_000,
                founding,
                Some(fixture),
                true,
            )
            .expect("exact partition"),
            1_100_000_000
        );
        assert!(
            authenticate_collateral_supply_partition_v1(
                founding,
                fixture,
                1_200_000_000,
                founding,
                Some(fixture),
                true,
            )
            .is_err(),
            "a hidden supply multiplier must refuse"
        );
        assert!(
            authenticate_collateral_supply_partition_v1(
                founding,
                fixture,
                1_100_000_000,
                founding,
                Some(fixture),
                false,
            )
            .is_err(),
            "a retained mint authority must refuse"
        );
        assert!(
            authenticate_collateral_supply_partition_v1(
                founding,
                fixture,
                1_100_000_000,
                founding,
                Some(fixture + 1),
                true,
            )
            .is_err(),
            "fixture liquidity must not leak into founding principal or grow by one atom"
        );
    }
}
