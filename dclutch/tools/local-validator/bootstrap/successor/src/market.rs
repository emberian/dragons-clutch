use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    thread,
    time::{Duration, Instant},
};

#[path = "founding_submission_journal.rs"]
pub(crate) mod founding_submission_journal;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use dclutch_market::capability_manifest::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityEntryV1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingLedgerV2, MAX_DEPENDENCIES_PER_CAPABILITY,
    controller_funding_checkpoint::{
        CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1, CONTROLLER_FUNDING_CUSTODY_ABORT_ANCHOR_DOMAIN_V1,
        CONTROLLER_FUNDING_CUSTODY_LADDER_DIGEST_DOMAIN_V1,
        ControllerFundingCheckpointDerivationV1, ControllerFundingCheckpointPhaseV1,
        ControllerFundingCheckpointV1, ControllerFundingControllerV1,
    },
    derive_funded_rent_rate_v2, funding_ledger_bytes_v2,
};
use dclutch_claims::{
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
use dclutch_custody::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyVaultSeedsV1, FoundingPrestateStageV1,
    OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1, PROJECTED_CUSTODY_STATE_BYTES_V2,
    PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCallerRoleV1, ProjectedCustodyCallerSeedsV1,
    ProjectedCustodyOperationV1, ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1,
    ProjectedCustodyStateV2, SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
};
use dclutch_trading::COMPILED_DIRECT_RELEASE_ID_V1;
#[cfg(test)]
use dclutch_trading::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3;
use dclutch_market::{
    Action, CoreState, FOUND_ACCOUNT_COUNT_V3, FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3,
    FOUND_PRICE_GATE_ACCOUNT_COUNT_V3, FOUND_RENT_SYSVAR_INDEX_V3, FoundingIntentV5,
    GenericFoundingRequestV1, GenericFoundingStageV1, Identity, MarketCoreStateSeedsV2,
    MarketIdentity, PRODUCT_GRAPH_BUMP_COUNT, PROJECT_FOUND_ACCOUNT_COUNT_V2,
    PROJECT_FOUND_PRICE_GATE_ACCOUNT_COUNT_V2, Phase, ProductGraphBumpsV1, ProjectFoundReceiptV2,
    ProjectFoundRequestV2, Readiness, Request, SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES,
    SeriesFoundingPermitSeedsV1, StateBumpsV1, generic_founding_funding_list_id_v1,
};
use dclutch_market_founding_v1_operator::{
    authenticate_generic_market_founding_artifact_v1, construct_generic_founding_root_selection_v1,
    construct_generic_market_founding_plan_v1,
};
use dclutch_product::payoff::{
    price_gate_v1::verify_price_gate_v1,
    registry_v3::{GRADED_BASIS_RECORD_SCHEMA_ID_V3, PRICE_GATE_RECORD_SCHEMA_ID_V1},
    runtime_v3::{
        BasisInputV3, BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product::{
    ContentId as ProductContentId, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, CompiledProductRecordsV2, FinalizedRecordObservationV2, FoundingBandV1,
    FoundingBeliefV1, ProductCompilationInputV2, StatedPropositionV1,
    compile_interesting_product_records_v2,
    found::{
        FinalizedReferenceObservationV2, FoundProjectionStateV2, FoundStateV2,
        build_found_instruction_v2, project_found_v2,
    },
    lifecycle_rent_v2::{LifecycleRentCreateStateV2, build_lifecycle_rent_create_v2},
    publication::{RecordPublicationContentV1, derive_record_addresses_v1},
};
use dclutch_source::pyth::{PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1, PythSponsoredPushReleaseV1};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{
    CallerAuthoritySeedsV1, ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V2,
    ProtocolInfrastructureProfileV1, ProtocolInfrastructureProfileV2,
};
use dclutch_market::rent::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleRentCreditV2,
};
use dclutch_source::resolution::{
    FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1, PreMarketFundingAbortRequestV1,
    PreMarketFundingRequestV2, pre_market_funding_ledger_account_digest_v1,
    pre_market_funding_prestate_digest_v1,
};
use dclutch_source::{
    ContentId as SourceContentId, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, ManipulationFloorV1,
    PROVIDER_RELEASE_SCHEMA_ID_V1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, ProviderReleaseV1,
    PythAdapterConfigV1, RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryAttemptV2, RecoveryPolicyV2,
    SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1,
    SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialV3, SourceSpecV1,
    WINDOW_SPEC_SCHEMA_ID_V1,
};
use dclutch_custody::token_svm::{
    ACCOUNT_BYTES, AccountState, MINT_BYTES, Mint, TOKEN_2022_PROGRAM_ID, TokenAccount,
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
    canonical_route_lookup_addresses_v1,
};

use crate::{
    Error, Result,
    core_bump_projection::{CoreProductGraphProjectionV1, core_product_graph_projection_v1},
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
    model::{
        AccountEvidence, FoundingRouteV1, MarketRunInput, RecordPair, SuccessorPlan,
        TransactionEvidence,
    },
    plan::{hex, hex32, pubkey},
    rpc::{FOUNDING_HEAP_FRAME_BYTES, Rpc, RpcAccount, account_evidence, bounded_instructions},
    runtime::{PublishedRecord, decode_hex, publish_product_graph, publish_record, record},
    seed::{KeyForge, role},
};

use founding_submission_journal::{
    FoundingFinalizationV1, FoundingPreSendProjectionV1, FoundingSubmissionBindingV1,
    FoundingSubmissionJournalV1, FoundingSubmissionOperationV1, FoundingSubmissionPhaseV1,
    FoundingSubmissionPlanV1, FoundingSubmissionRecoveryV1, UnresolvedFeeMarkerV1,
    UnresolvedFeeResolutionV1, authenticate_bound_founding_submission_prefix_v1,
    authenticate_founding_packet_fresh_v1, authenticate_founding_submission_v1,
    dispatch_founding_submission_v1, finalize_founding_submission_v1,
    founding_submission_finalized_poststates_v1, founding_submission_message_v1,
    founding_submission_packet_v1, founding_submission_recovery_payload_v1,
    founding_submission_recovery_v1, mark_unresolved_founding_submission_v1,
    plan_founding_submission_v1, prepare_founding_submission_v1, submit_founding_submission_v1,
    visit_founding_pre_send_boundary_v1,
};

/// The captured Pyth `PriceUpdateV2` account body this demo Market resolves
/// against. It is one of the eleven provenance-pinned artifacts
/// `dclutch-successor-validator` verifies before it starts, and the launcher
/// loads the receiver and router ELFs beside it, so the bytes here and the
/// programs on the chain come from one pinned set.
pub(crate) const FIXTURE_PRICE_UPDATE: &[u8] =
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
    pub(crate) binding: FoundingSubmissionBindingV1,
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

    /// Every journal this founding has written, in canonical operation order.
    ///
    /// A completion re-authentication needs the WHOLE set, not one row: an
    /// earlier stage's completion account is only honestly absent when a later
    /// stage's own journal names it.
    pub(crate) fn ordered(&self) -> Vec<FoundingSubmissionJournalV1> {
        self.journals.values().cloned().collect()
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
    recovery_policy: bool,
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
    if resolved.len() != operation.exact_unique_accounts(recovery_policy) {
        return Err(Error::new(format!(
            "{} resolved account count changed: expected {}, observed {} (market {} a recovery policy)",
            operation.label(),
            operation.exact_unique_accounts(recovery_policy),
            resolved.len(),
            if recovery_policy {
                "carries"
            } else {
                "does not carry"
            },
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
    authenticate_resolved_founding_message_v1(
        operation,
        binding.market_has_recovery_policy,
        &recomputed,
        tables,
    )?;
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
        authenticate_resolved_founding_message_v1(
            operation,
            recorder.binding.market_has_recovery_policy,
            &message,
            tables,
        )?;
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
                park_dcltgmf3_chaos_boundary_v1(
                    &current,
                    crate::chaos_fault::BoundaryV1::DispatchingBeforeSend,
                )?;
                let returned = rpc.submit_signed_packet_once(label, &packet, signature, false)?;
                if crate::chaos_fault::is_armed_for_v1(
                    "dcltgmf3",
                    crate::chaos_fault::BoundaryV1::LandedBeforeFinalizationFsync,
                )? {
                    let mut landed = false;
                    for _ in 0..300 {
                        if rpc
                            .finalized_signed_packet(label, returned, false)?
                            .is_some()
                        {
                            landed = true;
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(400));
                    }
                    if !landed {
                        return Err(Error::new(
                            "DCLTGMF3 chaos target did not reach finalized history before its fault boundary",
                        ));
                    }
                    park_dcltgmf3_chaos_boundary_v1(
                        &current,
                        crate::chaos_fault::BoundaryV1::LandedBeforeFinalizationFsync,
                    )?;
                }
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
                let successors = recorder.ordered();
                return authenticate_completed_founding_submission_v1(
                    rpc,
                    label,
                    &recorder.binding,
                    &current,
                    &successors,
                );
            }
        }
    }
}

fn park_dcltgmf3_chaos_boundary_v1(
    journal: &FoundingSubmissionJournalV1,
    boundary: crate::chaos_fault::BoundaryV1,
) -> Result<()> {
    if journal.operation != FoundingSubmissionOperationV1::Dcltgmf3 {
        return Ok(());
    }
    let packet_sha256 = journal
        .signed_packet_sha256
        .as_deref()
        .ok_or_else(|| Error::new("DCLTGMF3 chaos boundary omitted packet digest"))?;
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| Error::new("DCLTGMF3 chaos boundary omitted signature"))?;
    crate::chaos_fault::park_if_armed_v1(
        &journal.cluster,
        "dcltgmf3",
        boundary,
        Path::new(&journal.evidence_path),
        &journal.intent_sha256,
        packet_sha256,
        signature,
    )
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

/// Sealing-time totality pass over the durable submission journals.
///
/// Every journal the campaign never observed finalize either RESOLVES against
/// the chain (a late confirmation through the same verified reader the
/// recovery path trusts) or gains an explicit unresolved-fee marker, so the
/// campaign's fee record accounts EVERY send and a ledger reads a named
/// two-point bound instead of a silent absence. The selseam-hold-01 founding
/// is the motivating evidence: resolution-funding-activate-v1 sat in phase
/// `submitted` with a null fee while the chain had charged its deterministic
/// 75,000 lamports, and nothing in the report said so.
///
/// Nothing here may abort a seal: every refusal degrades to the marker, and a
/// marker refusal leaves the journal exactly as it was. Only rows whose packet
/// may have reached the chain participate (Dispatching and Submitted --
/// Planned and Prepared rows were never sent and owe no fee). Returns whether
/// any row changed.
pub(crate) fn resolve_stranded_founding_submissions_v1(
    rpc: &mut Rpc,
    binding: &FoundingSubmissionBindingV1,
    journals: &mut BTreeMap<FoundingSubmissionOperationV1, FoundingSubmissionJournalV1>,
) -> bool {
    let stranded: Vec<FoundingSubmissionOperationV1> = journals
        .iter()
        .filter(|(_, journal)| {
            matches!(
                journal.phase,
                FoundingSubmissionPhaseV1::Dispatching | FoundingSubmissionPhaseV1::Submitted
            )
        })
        .map(|(operation, _)| *operation)
        .collect();
    let mut changed = false;
    for operation in stranded {
        let Some(journal) = journals.get(&operation).cloned() else {
            continue;
        };
        let label = operation.label();
        let resolved: Result<FoundingSubmissionJournalV1> = (|| {
            let signature = journal
                .expected_signature
                .as_deref()
                .ok_or_else(|| Error::new("stranded durable journal omitted its signature"))?
                .parse::<Signature>()
                .map_err(|error| Error::new(format!("stranded durable signature: {error}")))?;
            let finalized = rpc
                .finalized_signed_packet(label, signature, false)?
                .ok_or_else(|| Error::new("the chain does not serve the transaction at sealing"))?;
            // `finalize_observed_…` requires Submitted exactly; a stranded
            // Dispatching row whose packet nevertheless finalized advances
            // through Submitted locally first -- the same one-adjacent-phase
            // grammar `finish_durable_founding_v1` uses.
            let submitted = if journal.phase == FoundingSubmissionPhaseV1::Dispatching {
                submit_founding_submission_v1(binding, &journal, &signature.to_string())?
            } else {
                journal.clone()
            };
            finalize_observed_founding_submission_v1(rpc, binding, &submitted, &finalized)
        })();
        match resolved {
            Ok(next) => {
                eprintln!(
                    "campaign: {label} resolved at sealing; fee read from the chain's own transaction record"
                );
                journals.insert(operation, next);
                changed = true;
            }
            Err(error) => {
                let (verdict, checked_at_slot) = match journal
                    .expected_signature
                    .as_deref()
                    .unwrap_or_default()
                    .parse::<Signature>()
                {
                    Ok(signature) => rpc.late_signature_probe_v1(signature),
                    Err(_) => (crate::rpc::LateSignatureProbeV1::Refused, 0),
                };
                let marker = match verdict {
                    crate::rpc::LateSignatureProbeV1::StatusWithoutMetadata { slot } => {
                        UnresolvedFeeMarkerV1 {
                            resolution: UnresolvedFeeResolutionV1::ChainStatusOnly,
                            status_slot: Some(slot),
                            unresolved_fee_bound_lamports: journal.exact_fee_lamports,
                            checked_at_slot,
                        }
                    }
                    crate::rpc::LateSignatureProbeV1::Unserved => UnresolvedFeeMarkerV1 {
                        resolution: UnresolvedFeeResolutionV1::ChainUnserved,
                        status_slot: None,
                        unresolved_fee_bound_lamports: journal.exact_fee_lamports,
                        checked_at_slot,
                    },
                    crate::rpc::LateSignatureProbeV1::Refused => UnresolvedFeeMarkerV1 {
                        resolution: UnresolvedFeeResolutionV1::RpcRefused,
                        status_slot: None,
                        unresolved_fee_bound_lamports: journal.exact_fee_lamports,
                        checked_at_slot,
                    },
                };
                eprintln!(
                    "campaign: {label} could not resolve at sealing ({error}); writing the explicit unresolved-fee marker ({:?})",
                    marker.resolution
                );
                match mark_unresolved_founding_submission_v1(binding, &journal, marker) {
                    Ok(next) => {
                        journals.insert(operation, next);
                        changed = true;
                    }
                    Err(error) => eprintln!(
                        "campaign: {label} unresolved-fee marker refused ({error}); journal retained unchanged"
                    ),
                }
            }
        }
    }
    changed
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
            let successors = recorder.ordered();
            authenticate_completed_founding_submission_v1(
                rpc,
                label,
                &recorder.binding,
                &current,
                &successors,
            )
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

/// The recovery-to-complete step's schema name.
pub(crate) const RECOVERY_TO_COMPLETE_SCHEMA_V1: &str =
    "dclutch-successor-founding-recovery-to-complete-v1";

/// The separate recovery-to-complete step's own evidence.
///
/// `campaign.rs` has refused a report whose founding was recovered after a
/// crash for as long as the flag has existed, and its refusal names the owed
/// repair in its own words: *"a separate recovery-to-complete step must
/// reconstruct and authenticate execution.market before terminal use"*. This
/// is that step's output. It exists so the refusal can tell an UNREPAIRED
/// crash-recovered report -- still non-consumable, and the only thing the flag
/// used to be able to mean -- from a repaired one, without the consumer having
/// to trust a boolean.
///
/// Every row here is a fact the step read, not a fact it decided: the six
/// finalized signatures its own transaction projection must also carry, the
/// journal state digests the founding wrote at the time, and one disposition
/// per recorded poststate saying whether the chain still holds it unchanged,
/// a later stage advanced it, or a later stage consumed it.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecoveryToCompleteEvidenceV1 {
    pub(crate) schema: String,
    /// The durable DCLTPCB2 checkpoint the reconstruction started from.
    pub(crate) checkpoint_sha256: String,
    /// One row per founding submission journal, in canonical operation order.
    pub(crate) journals: Vec<RecoveredFoundingJournalV1>,
    /// The account labels the reconstructed `execution.market` carries.
    pub(crate) reconstructed_account_labels: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecoveredFoundingJournalV1 {
    pub(crate) operation: FoundingSubmissionOperationV1,
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) state_sha256: String,
    pub(crate) finalized_poststates_sha256: String,
    pub(crate) poststates: Vec<RecordedPoststateDispositionV1>,
}

/// Build the recovery-to-complete step's evidence from the journal set and
/// the chain, refusing by name when any reconstructed fact disagrees.
pub(crate) fn recovery_to_complete_evidence_v1(
    rpc: &mut Rpc,
    recorder: &FoundingSubmissionRecorderV1<'_>,
    checkpoint: &MarketExecutionCheckpointV1,
    market: &MarketExecutionEvidence,
) -> Result<RecoveryToCompleteEvidenceV1> {
    let journals = recorder.ordered();
    authenticate_bound_founding_submission_prefix_v1(&recorder.binding, &journals)?;
    if journals.len() != FoundingSubmissionOperationV1::ORDER.len() {
        return Err(Error::new(format!(
            "recovery-to-complete requires the founding's whole {}-operation journal set; this report carries {}",
            FoundingSubmissionOperationV1::ORDER.len(),
            journals.len()
        )));
    }
    let mut rows = Vec::with_capacity(journals.len());
    for journal in &journals {
        if journal.phase != FoundingSubmissionPhaseV1::Finalized {
            return Err(Error::new(format!(
                "recovery-to-complete found {} at {}, and only a Finalized founding has evidence to recover",
                journal.operation.label(),
                serde_json::to_string(&journal.phase)?
            )));
        }
        let poststates = authenticate_recorded_founding_poststates_v1(
            rpc,
            &recorder.binding,
            journal,
            &journals,
        )?;
        rows.push(RecoveredFoundingJournalV1 {
            operation: journal.operation,
            signature: journal.expected_signature.clone().ok_or_else(|| {
                Error::new(format!(
                    "recovery-to-complete found {} Finalized with no signature",
                    journal.operation.label()
                ))
            })?,
            finalized_slot: journal.finalized_slot.ok_or_else(|| {
                Error::new(format!(
                    "recovery-to-complete found {} Finalized with no slot",
                    journal.operation.label()
                ))
            })?,
            state_sha256: journal.state_sha256.clone(),
            finalized_poststates_sha256: journal.finalized_poststates_sha256.clone().ok_or_else(
                || {
                    Error::new(format!(
                        "recovery-to-complete found {} Finalized with no poststate digest",
                        journal.operation.label()
                    ))
                },
            )?,
            poststates,
        });
    }
    if market.accounts.is_empty() {
        return Err(Error::new(
            "recovery-to-complete reconstructed no Market accounts",
        ));
    }
    Ok(RecoveryToCompleteEvidenceV1 {
        schema: RECOVERY_TO_COMPLETE_SCHEMA_V1.to_owned(),
        checkpoint_sha256: hex(&Sha256::digest(serde_json::to_vec(checkpoint)?)),
        journals: rows,
        reconstructed_account_labels: market.accounts.keys().cloned().collect(),
    })
}

/// The first later founding stage whose own journal accounts for this address.
///
/// `consumed` narrows it to a stage that names the account and does not leave
/// it among its own completion accounts -- which is exactly the record that
/// says the account's absence now is this founding's own doing. Cohort-13's
/// DCLTCFQ1 Trading Pending ledger is the worked example: DCLTCFQ1 completes
/// it, DCLTPCB2 names it as a prestate and completes something else, so the
/// vacancy is a pass by the record and not evidence of tampering.
///
/// The prestate and completion lists are the stage's CONTRACT, not its account
/// list, so this alone is not enough -- see
/// [`later_founding_stage_writing_v1`], which asks the stage's own signed
/// message.
fn later_founding_stage_naming_v1(
    address: &str,
    later: &[&FoundingSubmissionJournalV1],
    consumed: bool,
) -> Option<FoundingSubmissionOperationV1> {
    later
        .iter()
        .copied()
        .find(|successor| {
            let retained = successor
                .completion_accounts
                .iter()
                .any(|value| value == address);
            let named = retained
                || successor
                    .prestate_accounts
                    .iter()
                    .any(|value| value == address);
            named && (!consumed || !retained)
        })
        .map(|successor| successor.operation)
}

/// Every account one journal's own signed message LOCKS WRITABLE.
///
/// The journal's contract lists are curated: DCLTGMF3 consumes DCLTPCB2's
/// `founding_source_replay` and names it in neither its prestate nor its
/// completion set, so a rule reading only those lists refuses a founding that
/// did exactly what it was supposed to (measured against cohort-13's own
/// journals, 2026-09-02). The message is the authority for what a transaction
/// could write; it is immutable, digest-pinned in the journal, and its loaded
/// addresses resolve through routing tables this founding itself FROZE. So
/// this is a fact the founding signed rather than an inference about it.
fn founding_message_writable_accounts_v1(
    rpc: &mut Rpc,
    journal: &FoundingSubmissionJournalV1,
) -> Result<BTreeSet<Pubkey>> {
    let bytes = BASE64
        .decode(&journal.message_base64)
        .map_err(|error| Error::new(format!("founding journal message base64: {error}")))?;
    let message: VersionedMessage = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("founding journal message: {error}")))?;
    let header = message.header();
    let statics = message.static_account_keys();
    let signers = usize::from(header.num_required_signatures);
    let readonly_signed = usize::from(header.num_readonly_signed_accounts);
    let readonly_unsigned = usize::from(header.num_readonly_unsigned_accounts);
    let mut writable = BTreeSet::new();
    for (index, key) in statics.iter().enumerate() {
        let is_writable = if index < signers {
            index < signers.saturating_sub(readonly_signed)
        } else {
            index < statics.len().saturating_sub(readonly_unsigned)
        };
        if is_writable {
            writable.insert(*key);
        }
    }
    for lookup in message.address_table_lookups().unwrap_or_default() {
        let account = rpc.required_account(lookup.account_key, "founding routing table")?;
        let table = AddressLookupTable::deserialize(&account.data)
            .map_err(|_| Error::new("founding routing table bytes were invalid"))?;
        for index in &lookup.writable_indexes {
            let key = table
                .addresses
                .get(usize::from(*index))
                .ok_or_else(|| Error::new("founding routing table index is out of range"))?;
            writable.insert(*key);
        }
    }
    Ok(writable)
}

/// The first later founding stage that LOCKED this address writable.
fn later_founding_stage_writing_v1(
    rpc: &mut Rpc,
    address: Pubkey,
    later: &[&FoundingSubmissionJournalV1],
) -> Result<Option<FoundingSubmissionOperationV1>> {
    for successor in later {
        if founding_message_writable_accounts_v1(rpc, successor)?.contains(&address) {
            return Ok(Some(successor.operation));
        }
    }
    Ok(None)
}

/// The founding stages that finalized AFTER one boundary, and the only
/// authority an authenticator has for excusing a live difference from what
/// that boundary required.
///
/// One owner for a rule two callers need. `authenticate_recorded_founding_
/// poststates_v1` needs it for a journal's recorded poststates, and
/// [`BoundaryRpcV1`] needs it for a boundary-time expectation the journal
/// never recorded at all -- cohort-13's Pending controller funding ledgers,
/// which live in the founding's frozen coordinates and in no journal's
/// poststate list.
///
/// What it can and cannot say is worth stating plainly, because it is weaker
/// than it looks and still sufficient: naming or locking an address writable
/// proves a later stage COULD have moved it, not that it did. That is the same
/// strength the recorded-poststate rule has carried since `00793136`, and it
/// is the strongest claim the record supports -- the founding's own signed
/// messages and contract lists are the whole evidence base after the fact.
pub(crate) struct LaterFoundingStagesV1 {
    boundary: FoundingSubmissionOperationV1,
    journals: Vec<FoundingSubmissionJournalV1>,
}

impl LaterFoundingStagesV1 {
    /// Every Finalized journal strictly after `boundary`, each authenticated
    /// against the binding before it is allowed to excuse anything.
    pub(crate) fn authenticated(
        binding: &FoundingSubmissionBindingV1,
        boundary: FoundingSubmissionOperationV1,
        journals: &[FoundingSubmissionJournalV1],
    ) -> Result<Self> {
        let mut later = Vec::new();
        for successor in journals {
            if successor.operation <= boundary
                || successor.phase != FoundingSubmissionPhaseV1::Finalized
            {
                continue;
            }
            authenticate_founding_submission_v1(binding, successor)?;
            later.push(successor.clone());
        }
        Ok(Self {
            boundary,
            journals: later,
        })
    }

    fn rows(&self) -> Vec<&FoundingSubmissionJournalV1> {
        self.journals.iter().collect()
    }

    /// The first later stage that accounts for `address`: one whose contract
    /// lists name it, or -- because those lists are curated and not an account
    /// list -- one whose own signed message locks it writable, resolved
    /// through the routing tables this founding froze.
    fn owner_of(
        &self,
        rpc: &mut Rpc,
        address: Pubkey,
        consumed: bool,
    ) -> Result<Option<FoundingSubmissionOperationV1>> {
        let rows = self.rows();
        let text = address.to_string();
        Ok(
            match later_founding_stage_naming_v1(&text, &rows, consumed) {
                Some(operation) => Some(operation),
                None => later_founding_stage_writing_v1(rpc, address, &rows)?,
            },
        )
    }
}

/// An RPC connection an authenticator reads through when the boundary it
/// describes may already be in the PAST.
///
/// Cohort-13 paid for this distinction twice inside one commit. An
/// authenticator named for a transaction's poststate states an invariant about
/// ONE transaction's boundary, and every live read it makes silently
/// re-evaluates that invariant at whatever time the process happens to run.
/// While the caller is the transaction's own driver those two times coincide,
/// which is why nothing caught it for as long as the reconstruction path did
/// not exist. It does now.
///
/// So a read here must say which of two classes it is in, and the method name
/// is the declaration:
///
/// - `permanent_*` -- a fact no later founding stage can move. A closed
///   account stays closed, a consumed permit stays consumed, an allocated
///   Claims record is never deallocated, a Market that reached Open does not
///   leave Open by a funding stage.
/// - `boundary_*` -- a BOUNDARY-TIME expectation, which a later stage may
///   legitimately have superseded. A difference is excused only by a NAMED
///   later owner, and refused by name otherwise.
///
/// `at_boundary` is the live-time constructor and behaves exactly as a bare
/// `&mut Rpc` did: with no later stages, every boundary difference refuses in
/// the invariant's own words.
pub(crate) struct BoundaryRpcV1<'a> {
    rpc: &'a mut Rpc,
    later: Option<&'a LaterFoundingStagesV1>,
}

impl<'a> BoundaryRpcV1<'a> {
    /// The caller IS the boundary: nothing can have come after it yet.
    pub(crate) fn at_boundary(rpc: &'a mut Rpc) -> Self {
        Self { rpc, later: None }
    }

    /// The boundary is in the past, and these are the stages that finalized
    /// after it.
    pub(crate) fn after_boundary(rpc: &'a mut Rpc, later: &'a LaterFoundingStagesV1) -> Self {
        Self {
            rpc,
            later: Some(later),
        }
    }

    /// A fact no later founding stage can move.
    fn permanent_account(&mut self, address: Pubkey) -> Result<Option<RpcAccount>> {
        self.rpc.account(address)
    }

    /// A fact no later founding stage can move, whose account must exist.
    fn permanent_required_account(&mut self, address: Pubkey, label: &str) -> Result<RpcAccount> {
        self.rpc.required_account(address, label)
    }

    /// A boundary-time expectation: `holds` is the invariant as of the
    /// boundary, and `refusal` is the sentence that invariant refuses in.
    ///
    /// The invariant is kept, not weakened. What changes is the clock it is
    /// read against: at the boundary the live bytes ARE the boundary bytes, so
    /// a difference is the refusal; after it, a difference is the refusal only
    /// when no later stage of this same founding owns the address.
    fn boundary_account(
        &mut self,
        address: Pubkey,
        label: &str,
        holds: impl Fn(&RpcAccount) -> bool,
        refusal: &str,
    ) -> Result<()> {
        let observed = self.rpc.account(address)?;
        let consumed = observed.is_none();
        if observed.as_ref().is_some_and(holds) {
            return Ok(());
        }
        let Some(later) = self.later else {
            return Err(if consumed {
                Error::new(format!("missing {label} account {address}"))
            } else {
                Error::new(refusal.to_owned())
            });
        };
        match later.owner_of(self.rpc, address, consumed)? {
            Some(_) => Ok(()),
            None => Err(Error::new(format!(
                "{refusal}, and no founding stage after {} names or writes {label} {address}",
                later.boundary.label()
            ))),
        }
    }
}

/// What the chain now says about one poststate the journal recorded, and the
/// journal-held reason it may honestly differ.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecordedPoststateDispositionV1 {
    pub(crate) address: String,
    /// `unchanged`, `advanced-by-<operation>`, or `consumed-by-<operation>`.
    pub(crate) disposition: String,
}

/// Re-authenticate a Finalized journal's completion poststates against THE
/// RECORD, not against a live re-read.
///
/// `capture_founding_poststates_v1` is right at capture time and wrong at
/// recovery time, and the difference cost cohort-13 a landed founding's whole
/// evidence block on 2026-09-02: it calls `required_account` over every
/// completion account, and a founding's own later stages CONSUME earlier
/// stages' completion accounts by design -- DCLTCFQ1's Trading Pending ledger
/// `Q9zc5g4f…` is a prestate of DCLTPCB2 and a completion account of nothing
/// after it. So a COMPLETED founding could never be resumed: the resume
/// demanded a prestate the run it was resuming had destroyed on purpose.
///
/// The journal already holds what the account was at the adjacent transition
/// -- owner, lamports, data length and both digests -- and
/// `authenticate_founding_submission_v1` has already pinned that record to the
/// journal's state digest. So the question here is not "is the account still
/// there" but "does every difference from the record have a named later owner
/// in this same journal set". An absence with one is a pass BY THAT RECORD; an
/// absence without one is still a refusal, and so is an unexplained change.
pub(crate) fn authenticate_recorded_founding_poststates_v1(
    rpc: &mut Rpc,
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
    successors: &[FoundingSubmissionJournalV1],
) -> Result<Vec<RecordedPoststateDispositionV1>> {
    let recorded = founding_submission_finalized_poststates_v1(binding, journal)?;
    let later = LaterFoundingStagesV1::authenticated(binding, journal.operation, successors)?;
    let mut dispositions = Vec::with_capacity(recorded.len());
    for row in &recorded {
        let address = row
            .address
            .parse::<Pubkey>()
            .map_err(|error| Error::new(format!("recorded founding poststate: {error}")))?;
        let disposition = match rpc.account(address)? {
            Some(account) if &account_evidence(address, &account) == row => "unchanged".to_owned(),
            Some(_) => {
                let owner = later.owner_of(rpc, address, false)?.ok_or_else(|| {
                    Error::new(format!(
                        "founding finalized poststate {address} changed and no later {} stage names or writes it",
                        journal.operation.label()
                    ))
                })?;
                format!("advanced-by-{}", owner.label())
            }
            None => {
                let owner = later.owner_of(rpc, address, true)?.ok_or_else(|| {
                    Error::new(format!(
                        "founding finalized poststate {address} is vacant and no later {} stage consumed it",
                        journal.operation.label()
                    ))
                })?;
                format!("consumed-by-{}", owner.label())
            }
        };
        dispositions.push(RecordedPoststateDispositionV1 {
            address: row.address.clone(),
            disposition,
        });
    }
    Ok(dispositions)
}

pub(crate) fn authenticate_completed_founding_submission_v1(
    rpc: &mut Rpc,
    label: &str,
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
    successors: &[FoundingSubmissionJournalV1],
) -> Result<TransactionEvidence> {
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
    authenticate_recorded_founding_poststates_v1(rpc, binding, journal, successors)?;
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
                adapter_config_schema: dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1,
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

/// The alternative-source records a market's funded ordered ladder publishes.
///
/// One semantic join, used by input validation and by the publisher, for the
/// same reason `SourcePublicationContractV1` is: the ladder's records are
/// authenticated against the attempt that names them BEFORE anything is
/// created, so a rung can never name a record no publisher will land, and a
/// publisher can never land a record at an address the ladder does not point
/// at.
struct RecoveryLadderPublicationV1 {
    /// Canonical `RecoveryPolicyV2` body, empty for a market with no ladder.
    policy: Vec<u8>,
    /// One `(SourceSpecV1, PythAdapterConfigV1)` body pair per attempt, in
    /// ladder order.
    rungs: Vec<(Vec<u8>, Vec<u8>)>,
}

impl RecoveryLadderPublicationV1 {
    /// Resolution-controller compartments this market's founding must fund.
    ///
    /// FOUNDING FUNDS EVERY RUNG. `core_effect::authenticate_funding_entries`
    /// walks attempt `k` to the manifest entry at `recovery_entry_index + k`,
    /// then the exhaustion entry configured by the policy digest and the
    /// failure entry configured by the material -- so a policy of `n` attempts
    /// wants `n + 2` entries. A market with no ladder wants three: the failure
    /// compartment plus the two structural companions that stand in for the
    /// rungs nobody bought, which is `0.max(1) + 2` and is why the honest
    /// no-recovery founding keeps the count it has always had.
    fn controller_entries(&self) -> usize {
        self.rungs.len().max(1).saturating_add(2)
    }
}

/// Authenticate one market's ladder against the primary source it substitutes.
///
/// A RUNG SUBSTITUTES A SOURCE AND SUBSTITUTES NOTHING ELSE, which is the rule
/// `SourceMaterialV3::validate_recovery_source_graph` enforces on chain, and
/// every conjunct below is that rule asked one layer earlier so a founding that
/// would refuse refuses OFFLINE. The window, the statistic, the failure policy,
/// the capacity profile and the provider release stay the market's; the source
/// spec and the adapter configuration are the rung's own.
fn authenticate_recovery_ladder_publication_v1(
    input: &MarketRunInput,
    primary: SourceSpecV1,
    primary_source_spec_id: [u8; 32],
) -> Result<RecoveryLadderPublicationV1> {
    let policy_bytes = decode_hex(&input.recovery_policy_hex)?;
    // Empty means the material carries NO recovery policy: the deliberate
    // section-12.8 demo shape, admitted on chain at e5b6923 and decided in
    // MAINNET_STATE_RELAY.md section 13.
    if policy_bytes.is_empty() {
        if !input.recovery_source_records.is_empty() {
            return Err(Error::new(
                "the run spec carries alternative source records and no recovery policy: nothing \
                 would name them, and a record no attempt names is a record no market resolves \
                 against",
            ));
        }
        return Ok(RecoveryLadderPublicationV1 {
            policy: Vec::new(),
            rungs: Vec::new(),
        });
    }
    let policy = RecoveryPolicyV2::decode(&policy_bytes)
        .map_err(|error| Error::new(format!("RecoveryPolicyV2: {error:?}")))?;
    if policy.to_bytes().as_slice() != policy_bytes {
        return Err(Error::new("RecoveryPolicyV2 input was not canonical"));
    }
    // `RecoveryPolicyV2::validate_capacity_profile`'s producer, asked at the
    // founding rather than at the capture: a ladder running under a capacity
    // profile the market did not publish is a ladder the founding did not
    // price, and the recovery join refuses it on chain with `SourceMaterial`.
    policy
        .validate_capacity_profile(primary.capacity_profile_id())
        .map_err(|error| {
            Error::new(format!(
                "the recovery policy declares a capacity profile this market did not publish: \
                 {error:?}"
            ))
        })?;
    let declared = usize::from(policy.attempt_count());
    if input.recovery_source_records.len() != declared {
        return Err(Error::new(format!(
            "the recovery policy funds {declared} attempts and the run spec carries {} \
             alternative source record pairs; an attempt whose spec nobody publishes is a rung a \
             market can be advanced onto and never answered on",
            input.recovery_source_records.len()
        )));
    }
    let mut rungs = Vec::with_capacity(declared);
    for (index, records) in input.recovery_source_records.iter().enumerate() {
        let rung = u8::try_from(index).map_err(|_| Error::new("recovery rung index overflow"))?;
        let attempt = policy
            .attempt(rung)
            .map_err(|error| Error::new(format!("recovery attempt {rung}: {error:?}")))?;
        let spec_bytes = decode_hex(&records.source_spec_hex)?;
        let spec = SourceSpecV1::decode(&spec_bytes)
            .map_err(|error| Error::new(format!("rung {rung} SourceSpecV1: {error:?}")))?;
        if spec.to_bytes().as_slice() != spec_bytes {
            return Err(Error::new(format!(
                "rung {rung} SourceSpecV1 input was not canonical"
            )));
        }
        let spec_id = record_identity(&spec_bytes);
        if spec_id != attempt.source_spec_id().to_bytes() {
            return Err(Error::new(format!(
                "rung {rung} carries a SourceSpec body whose digest is not the source spec the \
                 attempt names, so the attempt names a finalized record that can never exist"
            )));
        }
        if spec_id == primary_source_spec_id {
            return Err(Error::new(format!(
                "rung {rung} names the market's PRIMARY source: a ladder whose alternative is the \
                 feed that already went silent buys nothing"
            )));
        }
        // The five conjuncts a rung may not move. `validate_recovery_source_graph`
        // replaces exactly one edge of the primary graph -- the source -- and a
        // rung that moved its unit, its domain, its capacity, its access profile
        // or its provider release would be substituting the QUESTION rather than
        // the answerer.
        if spec.unit_id() != primary.unit_id()
            || spec.domain_id() != primary.domain_id()
            || spec.capacity_profile_id() != primary.capacity_profile_id()
            || spec.access_profile() != primary.access_profile()
            || spec.provider_release_id() != primary.provider_release_id()
        {
            return Err(Error::new(format!(
                "rung {rung} substitutes more than a source: its unit, coordinate domain, \
                 capacity profile, access profile and provider release must all be the market's \
                 own, and a rung differs from the primary in its adapter configuration alone"
            )));
        }
        if spec.provider_release_id() != attempt.provider_release_id() {
            return Err(Error::new(format!(
                "rung {rung} names a provider release its own SourceSpec does not select"
            )));
        }
        let adapter_bytes = decode_hex(&records.pyth_adapter_config_hex)?;
        let adapter = PythAdapterConfigV1::decode(&adapter_bytes)
            .map_err(|error| Error::new(format!("rung {rung} PythAdapterConfigV1: {error:?}")))?;
        if adapter.to_bytes().as_slice() != adapter_bytes {
            return Err(Error::new(format!(
                "rung {rung} PythAdapterConfigV1 input was not canonical"
            )));
        }
        if record_identity(&adapter_bytes) != spec.adapter_config_id().to_bytes() {
            return Err(Error::new(format!(
                "rung {rung} carries an adapter configuration that is not the one its SourceSpec \
                 names"
            )));
        }
        rungs.push((spec_bytes, adapter_bytes));
    }
    Ok(RecoveryLadderPublicationV1 {
        policy: policy_bytes,
        rungs,
    })
}

/// ONE INDEX NAMES THE WHOLE RUN, so the run has to be adjacent.
///
/// `core_effect::authenticate_funding_entries` pays attempt `k` from the
/// manifest entry at `recovery_entry_index + k`, and the off-chain operator's
/// `select_resolution_funding_entries` finds only attempt ZERO's entry by its
/// allocation and derives the rest by that adjacency. A manifest is canonical
/// only when its entries are strictly ordered by capability-kind identity, and
/// the demo kinds are digests, so the ORDER a founding gets is a fact about
/// hashes rather than about the ladder. For a one-attempt ladder that is
/// vacuous. For a wider one it is a real constraint, and this is where a
/// founding that would refuse on chain -- after collateral, records, RentCredit
/// and an ALT already exist -- refuses offline instead.
///
/// PROVISIONAL BOUND, with its lifting plan named: a wider ladder whose kind
/// digests do not happen to sort into ladder order cannot be founded by this
/// producer today. Lifting it means authoring the rung kind identities so their
/// canonical order IS the run's -- which is a choice about how a demo capability
/// kind is derived, not a protocol change -- and it belongs to whoever founds
/// the first two-rung market.
fn authenticate_ladder_entry_adjacency_v1(
    input: &MarketRunInput,
    manifest: CapabilityManifestV1<'_>,
) -> Result<()> {
    let policy_bytes = decode_hex(&input.recovery_policy_hex)?;
    if policy_bytes.is_empty() {
        return Ok(());
    }
    let policy = RecoveryPolicyV2::decode(&policy_bytes)
        .map_err(|error| Error::new(format!("RecoveryPolicyV2: {error:?}")))?;
    let mut first: Option<u16> = None;
    for rung in 0..policy.attempt_count() {
        let attempt = policy
            .attempt(rung)
            .map_err(|error| Error::new(format!("recovery attempt {rung}: {error:?}")))?;
        let allocation = attempt.funding_allocation_id().to_bytes();
        let mut found: Option<u16> = None;
        let mut index = 0_u16;
        while index < manifest.entry_count() {
            let entry = manifest
                .entry(index)
                .map_err(|error| Error::new(format!("capability entry {index}: {error:?}")))?;
            if entry.config_id().to_bytes() == allocation {
                if found.replace(index).is_some() {
                    return Err(Error::new(format!(
                        "two capability entries are configured by rung {rung}'s funding \
                         allocation; one identity is one compartment, and a ladder whose rung \
                         names two would be paid twice for a leg it enters once"
                    )));
                }
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::new("capability entry index overflow"))?;
        }
        let at = found.ok_or_else(|| {
            Error::new(format!(
                "no capability entry is configured by rung {rung}'s funding allocation: the \
                 founding would sell a leg nothing paid for"
            ))
        })?;
        match first {
            None => first = Some(at),
            Some(base) => {
                let expected = base
                    .checked_add(u16::from(rung))
                    .ok_or_else(|| Error::new("recovery entry index overflow"))?;
                if at != expected {
                    return Err(Error::new(format!(
                        "rung {rung}'s compartment sits at manifest entry {at} and the run that \
                         starts at {base} requires {expected}: one recovery_entry_index names the \
                         whole run, so the compartments must be adjacent in ladder order"
                    )));
                }
            }
        }
    }
    Ok(())
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
    let primary_spec_bytes = decode_hex(&input.source_spec_hex)?;
    let primary_spec = SourceSpecV1::decode(&primary_spec_bytes)
        .map_err(|error| Error::new(format!("SourceSpecV1: {error:?}")))?;
    let ladder = authenticate_recovery_ladder_publication_v1(
        input,
        primary_spec,
        record_identity(&primary_spec_bytes),
    )?;
    let manifest = decode_hex(&input.capability_manifest_hex)?;
    let manifest = CapabilityManifestV1::decode(&manifest)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
    // ONE SELECTED TRADE ENTRY PLUS THE RESOLUTION COMPARTMENTS THE LADDER
    // COSTS. This was a hard four, which is the right number for a market with
    // no ladder and the wrong one for every market that buys a rung: founding
    // funds every rung, so a policy of `n` attempts wants `n + 2` controller
    // entries. `n = 1` is still four, so the one-alternative founding and the
    // no-recovery founding both keep the count they had.
    let expected_entries = ladder.controller_entries().saturating_add(1);
    if usize::from(manifest.entry_count()) != expected_entries {
        return Err(Error::new(format!(
            "capability manifest must contain one selected trade entry and {} Resolution \
             compartments -- one per funded rung, then exhaustion, then failure -- and carries {}",
            ladder.controller_entries(),
            manifest.entry_count()
        )));
    }
    authenticate_ladder_entry_adjacency_v1(input, manifest)?;
    match (&input.direct_capability, &input.selected_capability) {
        (Some(_), None) => validate_direct_market_capability_v1(input)?,
        (None, Some(_)) => {
            crate::selected_capability::validate_selected_capability_input_v1(input)?;
        }
        _ => {
            return Err(Error::new(
                "market input must carry exactly one selected-capability closure: the Direct \
                 closure or a family-neutral closure",
            ));
        }
    }
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
            // Exempt by proof: degree 0 and 1 need no price gate,
            // and a digest offered alongside one is refused.
            price_gate_certificate_digest: [0_u8; 32],
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
    /// The exact capability-manifest bytes that were PUBLISHED, which are not
    /// always the bytes the input declared: a market with bounded Source
    /// material has its manifest rebuilt so the Source entry names the compiled
    /// Source rather than the floor template's. Everything on chain — the
    /// record, the Market identity, and Core's own authentication — is bound to
    /// these bytes, so the founding artifact must be derived from them too.
    manifest_body: Vec<u8>,
    basis: PublishedRecord,
    price_gate: Option<PublishedRecord>,
    basis_scale: u64,
    /// Carried from the authenticated record, never recomputed here.
    basis_refunds_on_failure: bool,
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
    /// One `(SourceSpecV1, PythAdapterConfigV1)` published pair per funded
    /// rung, in ladder order. Empty for a market that bought no ladder, and
    /// empty is what the evidence then says -- by absence, exactly as the
    /// recovery policy record does.
    recovery_sources: Vec<(PublishedRecord, PublishedRecord)>,
    sponsored_push_release: Option<PublishedRecord>,
    /// Exact Registry closure selected by the manifest's one trade entry —
    /// Direct's typed record set, or a family-neutral closure's record list.
    direct: BTreeMap<String, PublishedRecord>,
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
    let authenticated_plan = authenticated_found_infrastructure_plan_v1(rpc, plan)?;
    let plan = &authenticated_plan;
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
    if let Some(price_gate) = records.price_gate {
        let account = rpc.required_account(price_gate.raw, "price-gate certificate record")?;
        accounts.insert(
            "price_gate_record".into(),
            account_evidence(price_gate.raw, &account),
        );
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
    for (label, record) in &records.direct {
        let account = rpc.required_account(record.raw, label)?;
        accounts.insert(label.clone(), account_evidence(record.raw, &account));
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
        direct_selected_manifest_entry_index:
            crate::selected_capability::selected_manifest_entry_index_v1(input)?,
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
    let authenticated_plan = authenticated_found_infrastructure_plan_v1(rpc, plan)?;
    let plan = &authenticated_plan;
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
    let authenticated_plan = authenticated_found_infrastructure_plan_v1(rpc, plan)?;
    let plan = &authenticated_plan;
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

/// The label DCLTCFQ1's honest transaction carries.
///
/// One author for two readers: the live send site and the reconstruction that
/// reads the same transaction back off chain hours later. They used to be one
/// literal and one absence, which is how the reconstruction came to project no
/// DCLTCFQ1 row at all.
const DCLTCFQ1_SUBMISSION_LABEL_V1: &str =
    "prepare exact controller funding ledgers and checkpoint (DCLTCFQ1)";

/// The founding stages the completed-founding reconstruction must project from
/// HISTORY, because nothing on that path re-sends them.
///
/// `campaign.rs`'s `authenticate_recovery_to_complete_v1` corroborates all six
/// journal signatures against the report's own `execution.transactions`, and it
/// is right to: an `execution` block that names journals no transaction row
/// backs is assertion rather than evidence. But the reconstruction deliberately
/// republishes nothing -- `reconstruct_founding_checkpoint_v1` refuses its own
/// republication by count -- so the two stages BEFORE Open had no owner that
/// would put them in the projection. DCLTGMF3 has one in
/// `finalize_existing_founding_submission_v1` and the three funding stages have
/// one in `execute_funding_readiness_suffix_v1`; these two had none, and a
/// recovered report was refused for a gap in its producer rather than a defect
/// in its founding.
///
/// They are READ BACK and reauthenticated, never transcribed:
/// `authenticate_historical_founding_transaction_v1` reparses the journal's
/// signature, refetches the finalized packet, and compares slot, packet digest,
/// fee and compute units against what the journal recorded. It deliberately
/// does not re-assert those stages' old poststates as live state -- three later
/// stages moved them by design, and saying so at the right boundary is
/// `RecoveredFoundingJournalV1::poststates`' job, not this projection's.
const RECONSTRUCTION_PROJECTED_HISTORY_V1: [(FoundingSubmissionOperationV1, &str); 2] = [
    (
        FoundingSubmissionOperationV1::Dcltcfq1,
        DCLTCFQ1_SUBMISSION_LABEL_V1,
    ),
    (
        FoundingSubmissionOperationV1::Dcltpcb2,
        PrestateLaneV1::Founding.prestate_label(),
    ),
];

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
) -> Result<(MarketExecutionEvidence, RecoveryToCompleteEvidenceV1)> {
    if checkpoint.schema != DCLTPCB2_CHECKPOINT_SCHEMA_V1 {
        return Err(Error::new(
            "completed founding recovery requires the DCLTPCB2 checkpoint schema",
        ));
    }
    let authenticated_plan = authenticated_found_infrastructure_plan_v1(rpc, plan)?;
    let plan = &authenticated_plan;
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
    // The Open boundary is HOURS in the past on this path, and three of this
    // founding's own stages finalized after it. So the Open verifier reads
    // through the later-stage resolver: its permanent facts are unaffected,
    // and its one boundary-time fact -- the Pending controller funding
    // ledgers -- is excused only by a named later owner.
    let later_than_open = LaterFoundingStagesV1::authenticated(
        &submission_recorder
            .as_deref()
            .ok_or_else(|| Error::new("completed founding recovery omitted its journal owner"))?
            .binding,
        FoundingSubmissionOperationV1::Dcltgmf3,
        &submission_recorder
            .as_deref()
            .ok_or_else(|| Error::new("completed founding recovery omitted its journal owner"))?
            .ordered(),
    )?;
    authenticate_open_market_poststate_v1(
        &mut BoundaryRpcV1::after_boundary(rpc, &later_than_open),
        &context.coordinates,
        &poststate,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.claims.program_id)?,
        pubkey(&plan.custody.program_id)?,
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        context.mint,
    )?;
    if let Some(recorder) = submission_recorder.as_deref_mut() {
        // Canonical order, and before DCLTGMF3: the projection a reader sees is
        // the founding's own sequence, not the order recovery happened to
        // rebuild it in.
        for (operation, label) in RECONSTRUCTION_PROJECTED_HISTORY_V1 {
            let evidence =
                authenticate_historical_founding_transaction_v1(rpc, label, operation, recorder)?;
            push_transaction_once_v1(transactions, evidence);
        }
        let coordinates = &context.coordinates;
        let core = pubkey(&plan.core.program_id)?;
        let claims = pubkey(&plan.claims.program_id)?;
        let custody = pubkey(&plan.custody.program_id)?;
        let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        let mint = context.mint;
        let later = &later_than_open;
        let mut completion = |rpc: &mut Rpc| {
            authenticate_open_market_poststate_v1(
                &mut BoundaryRpcV1::after_boundary(rpc, later),
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
    let evidence = MarketExecutionEvidence {
        completed,
        accounts,
        founding_custody_context: checkpoint.founding_custody_context.clone(),
        direct_selected_manifest_entry_index: checkpoint.direct_selected_manifest_entry_index,
        local_participant_fixture_liquidity: checkpoint.local_participant_fixture_liquidity.clone(),
    };
    // The reconstruction is not finished until it can say what it read. A
    // recovery that cannot produce its own evidence is a recovery nobody can
    // check, which is the state this whole step exists to end.
    let recovery = recovery_to_complete_evidence_v1(
        rpc,
        submission_recorder
            .as_deref()
            .ok_or_else(|| Error::new("recovery-to-complete omitted its journal owner"))?,
        checkpoint,
        &evidence,
    )?;
    Ok((evidence, recovery))
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
    basis: Vec<u8>,
    price_gate: Option<Vec<u8>>,
    basis_scale: u64,
    basis_refunds_on_failure: bool,
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

struct AuthenticatedMarketBasisV1 {
    body: Vec<u8>,
    price_gate: Option<Vec<u8>>,
    payout_scale: u64,
    /// Whether this record refunds ordinary holders on an oracle outage.
    ///
    /// Read from the decoded record through `ProductBasisV3::refunds_on_failure`,
    /// which is `categorical_refunds_on_failure_v3` -- the SOLE AUTHOR of the
    /// rule -- applied to the record's own kind, width and payout scale. The
    /// founding needs it because a refunding Market seats its failure column in
    /// an escrow whose two accounts the founder must pre-fund, and a host that
    /// spelled the rule a second time would eventually disagree with the
    /// program about which markets those are.
    refunds_on_failure: bool,
}

fn authenticate_market_basis_v1(
    input: &MarketRunInput,
    semantic_product_id: ProductContentId,
    domain_digest: [u8; 32],
    outcome_count: usize,
) -> Result<AuthenticatedMarketBasisV1> {
    let body = decode_hex(&input.linked_basis_hex)?;
    let basis = ProductBasisV3::decode(&body)
        .map_err(|error| Error::new(format!("ProductBasisV3: {error:?}")))?;
    basis
        .admit_selection_v3()
        .map_err(|error| Error::new(format!("ProductBasisV3 admission: {error:?}")))?;
    let width =
        u32::try_from(outcome_count).map_err(|_| Error::new("Product outcome width overflow"))?;
    if semantic_basis_identity_v3(&body)? != product_id(&input.liability_basis_id)?.to_bytes()
        || basis.product_id() != semantic_product_id.to_bytes()
        || basis.result_domain_id() != domain_digest
        || basis.basis_width() != width
    {
        return Err(Error::new(
            "linked liability basis record did not bind the compiled Product graph",
        ));
    }

    let offered = decode_hex(&input.price_gate_hex)?;
    let expected = basis.price_gate_certificate_digest_v3();
    let price_gate = if expected == [0; 32] {
        if !offered.is_empty() {
            return Err(Error::new(
                "an exempt ProductBasisV3 cannot carry a price-gate record",
            ));
        }
        None
    } else {
        if offered.is_empty() || record_identity(&offered) != expected {
            return Err(Error::new(
                "curved ProductBasisV3 requires its exact named price-gate record",
            ));
        }
        let degree = match basis.kind() {
            BasisKindV3::SplineDegree2To3 { degree, .. } => degree,
            _ => {
                return Err(Error::new(
                    "only the spline ProductBasisV3 family may name a price gate",
                ));
            }
        };
        verify_price_gate_v1(
            &basis,
            basis.knot_denominator(),
            basis.payout_scale(),
            degree,
            basis.basis_width(),
            &offered,
        )
        .map_err(|error| Error::new(format!("DCLTPGT1 admission: {error:?}")))?;
        Some(offered)
    };
    let payout_scale = basis.payout_scale();
    let refunds_on_failure = basis.refunds_on_failure();
    Ok(AuthenticatedMarketBasisV1 {
        refunds_on_failure,
        body,
        price_gate,
        payout_scale,
    })
}

/// Turn one authored belief into the compiler's, on the partition's denominator.
///
/// The belief is kept on the partition's own denominator rather than rescaled:
/// rescaling would silently measure a different market than the one being
/// compiled, which is the whole failure mode this gate exists to catch.
///
/// This is a MATCH ON KIND and never an exemption. A market that declares a
/// spot band is measured against a random walk exactly as it always was; a
/// market that declares a prior is measured against the prior it stated. There
/// is no third branch in which a market is not measured.
fn founding_belief_for(
    band: &crate::model::FoundingBandInputV1,
    cut_denominator: u64,
    label: &str,
) -> Result<(FoundingBeliefV1, u32)> {
    // Bounded by the compiler's own MAX_CELL_EX_ANTE_SHARE_BPS_V1, read rather
    // than restated. An author states the ceiling their product wants, at or
    // below the one this release enforces; `10_000` here until 2026-09-01 let
    // an author state a ceiling that switched the gate off.
    if band.max_cell_share_bps == 0
        || band.max_cell_share_bps
            > dclutch_product_runtime_v2_operator::MAX_CELL_EX_ANTE_SHARE_BPS_V1
    {
        return Err(Error::new(format!(
            "{label} founding_band/max_cell_share_bps: expected 1..={}",
            dclutch_product_runtime_v2_operator::MAX_CELL_EX_ANTE_SHARE_BPS_V1
        )));
    }
    let belief = match band.require_one_kind(label)? {
        crate::model::DeclaredBeliefKindV1::SpotBand {
            anchor,
            volatility_bps,
            window_slots,
            plausible_half_widths,
        } => {
            if plausible_half_widths == 0 {
                return Err(Error::new(format!(
                    "{label} founding_band/plausible_half_widths: expected at least one"
                )));
            }
            FoundingBeliefV1::SpotBand {
                band: FoundingBandV1 {
                    anchor,
                    denominator: cut_denominator,
                    volatility_bps,
                    window_slots,
                },
                plausible_half_widths,
            }
        }
        crate::model::DeclaredBeliefKindV1::StatedProposition {
            cell_probability_bps,
        } => FoundingBeliefV1::StatedProposition(StatedPropositionV1 {
            denominator: cut_denominator,
            cell_probability_bps,
        }),
    };
    Ok((belief, band.max_cell_share_bps))
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
    // THE REQUIREMENT IS UNCHANGED AND TOTAL: every market states a belief, and
    // there is no default. What moved is that it is now a match on the KIND of
    // belief rather than an assumption that every market's belief is a spot.
    // The relayed graduation market states a proposition; the SOL/USD markets
    // the release ladder founds state a spot band and take the identical path.
    let declared_band = input.founding_band.as_ref().ok_or_else(|| {
        Error::new(
            "founding_band is required to compile this market's partition: state \
             max_cell_share_bps, and EITHER a spot band (anchor, volatility_bps, \
             window_slots, plausible_half_widths) for a coordinate that moves OR \
             cell_probability_bps for a proposition. There is no default in \
             either kind -- the belief is an authoring input, and a partition \
             cannot be measured for degeneracy without the belief it is meant \
             to describe",
        )
    })?;
    let (belief, ceiling) =
        founding_belief_for(declared_band, input.cut_denominator, "market input")?;
    let (compiled, _quality) = compile_interesting_product_records_v2(
        registry,
        &belief,
        ceiling,
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
    let authenticated_basis =
        authenticate_market_basis_v1(input, semantic_product_id, domain_digest, outcome_count)?;

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
            authenticated_basis.payout_scale,
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
    let realm = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: collateral_mint.to_bytes(),
        collateral_adapter_release_id: collateral_adapter_release_id(),
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
        basis: authenticated_basis.body,
        price_gate: authenticated_basis.price_gate,
        basis_scale: authenticated_basis.payout_scale,
        basis_refunds_on_failure: authenticated_basis.refunds_on_failure,
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
    let primary_spec_bytes = decode_hex(&input.source_spec_hex)?;
    let recovery_rungs = authenticate_recovery_ladder_publication_v1(
        input,
        SourceSpecV1::decode(&primary_spec_bytes)
            .map_err(|error| Error::new(format!("SourceSpecV1: {error:?}")))?,
        record_identity(&primary_spec_bytes),
    )?
    .rungs;
    let CompiledMarketBodiesV1 {
        compiled,
        semantic_product_id,
        realm,
        product,
        domain,
        portfolio,
        basis: basis_bytes,
        price_gate: price_gate_bytes,
        basis_scale,
        basis_refunds_on_failure,
        source,
        source_capacity_profile,
        manipulation_floor: manipulation_floor_bytes,
        principal_cap_sets,
        recovery: recovery_bytes,
        manifest,
        product_digest: _,
        domain_digest: _,
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
    let mut direct: BTreeMap<String, PublishedRecord> = BTreeMap::new();
    let native_composition = dclutch_representation_composition_v3_operator::native_categorical_v1::NativeBasisCompositionInputV1 {
        market: terminal_market.to_bytes(),
        release_set,
        product_record_bytes: &product_body,
        result_domain_bytes: &domain_body,
        portfolio_bytes: &portfolio_body,
        product_basis_bytes: &basis_bytes,
        price_gate_bytes: price_gate_bytes.as_deref(),
    };
    if let Some(selected) = &input.selected_capability {
        // The family-neutral closure: its own record list, labels already
        // validated unique and `_record`-suffixed, plus the four terminal
        // composition records every market's terminal path requires.
        for record in &selected.records {
            let published = publish_record(
                rpc,
                registry,
                payer,
                hex32(&record.schema_hex)?,
                &decode_hex(&record.body_hex)?,
                None,
                transactions,
            )?;
            if direct.insert(record.label.clone(), published).is_some() {
                return Err(Error::new(
                    "selected-capability publication repeated an evidence label",
                ));
            }
        }
        let native = dclutch_representation_composition_v3_operator::native_categorical_v1::compile_native_basis_composition_v1(native_composition)
            .map_err(|error| Error::new(format!("native basis composition: {error:?}")))?;
        for (label, target) in [
            "terminal_composition_descriptor_record",
            "terminal_composition_graph_record",
            "terminal_composition_translation_record",
            "terminal_composition_exposure_record",
        ]
        .into_iter()
        .zip(native.publication_targets())
        {
            let published = publish_record(
                rpc,
                registry,
                payer,
                target.schema_id,
                target.bytes,
                None,
                transactions,
            )?;
            if direct.insert(label.to_string(), published).is_some() {
                return Err(Error::new(
                    "selected-capability publication repeated an evidence label",
                ));
            }
        }
    } else {
        let categorical_composition = dclutch_representation_composition_v3_operator::native_categorical_v1::NativeCategoricalCompositionInputV1 {
            market: native_composition.market,
            release_set: native_composition.release_set,
            product_record_bytes: native_composition.product_record_bytes,
            result_domain_bytes: native_composition.result_domain_bytes,
            portfolio_bytes: native_composition.portfolio_bytes,
            product_basis_bytes: native_composition.product_basis_bytes,
        };
        for record in
            crate::direct_market::direct_publication_records_v1(input, categorical_composition)?
        {
            let published = publish_record(
                rpc,
                registry,
                payer,
                record.schema,
                &record.body,
                None,
                transactions,
            )?;
            if direct.insert(record.label.to_string(), published).is_some() {
                return Err(Error::new("Direct publication repeated an evidence label"));
            }
        }
    }
    // Keep the published bytes, not just the record's coordinates: the
    // founding artifact's capability-root selection is derived from the
    // manifest, and deriving it from the input's DECLARED manifest instead of
    // this published one makes a bounded-Source-material market unfoundable.
    let manifest_body = manifest.clone();
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
    // THE LADDER'S OWN RECORDS. A rung's `SourceSpecV1` and the
    // `PythAdapterConfigV1` it names are finalized records like any other, and
    // the recovery join authenticates them exactly as the primary leg
    // authenticates the market's. They ride under the SAME two schemas as the
    // primary pair, because a rung's records are the same KIND of record --
    // what makes the rung an alternative is the confidence bound inside the
    // configuration, not a different schema.
    //
    // `authenticate_recovery_ladder_publication_v1` has already proved every
    // digest, so `publish_record` cannot land one at an address no attempt
    // points at.
    let mut recovery_sources = Vec::with_capacity(recovery_rungs.len());
    for (spec_bytes, adapter_bytes) in &recovery_rungs {
        let spec = publish_record(
            rpc,
            registry,
            payer,
            SOURCE_SPEC_SCHEMA_ID_V1,
            spec_bytes,
            None,
            transactions,
        )?;
        let adapter = publish_record(
            rpc,
            registry,
            payer,
            source_publication.adapter_config_schema,
            adapter_bytes,
            None,
            transactions,
        )?;
        recovery_sources.push((spec, adapter));
    }
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
    let basis = publish_record(
        rpc,
        registry,
        payer,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        &basis_bytes,
        None,
        transactions,
    )?;
    let price_gate = match price_gate_bytes {
        Some(bytes) => Some(publish_record(
            rpc,
            registry,
            payer,
            PRICE_GATE_RECORD_SCHEMA_ID_V1,
            &bytes,
            None,
            transactions,
        )?),
        None => None,
    };
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
            manifest_body,
            basis,
            price_gate,
            basis_scale,
            basis_refunds_on_failure,
            source_spec,
            window_spec,
            statistic_spec,
            provider_release,
            adapter_config,
            recovery_sources,
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

/// Publish one finalized, FROZEN address lookup table covering an oversized
/// frame, and read it back before anyone routes through it.
///
/// Only routable coordinates are carried; the fee payer, every signer and every
/// invoked program stay in the message's static key list, because no table can
/// move them.
///
/// # Why frozen, when frozen forfeits the rent
///
/// This function used to leave the table authority-owned, and said so: "the
/// table is authority-owned so its rent stays recoverable, and it is never
/// frozen." Nine call sites took that shape and two took the frozen one, which
/// is two answers to one question.
///
/// A mutable table is a second authority over the transaction. A v0 message
/// carries INDICES into the table, not addresses, and the account set is
/// resolved at execution from whatever the table holds THEN -- so an authority
/// that can still extend the table can change which accounts a message touches
/// after that message is signed. On this path the table authority and the
/// transaction payer are the same key, which makes it both halves of the route:
/// exactly the shape this protocol refuses everywhere else. That is a defect,
/// not a trade-off.
///
/// The rent is a price, and a smaller one than it looks. Nothing in this tree
/// ever deactivated or closed one of these tables -- `plan_lookup_table_retirement_v1`
/// exists in the operator and has no caller here -- so "recoverable" was
/// theoretical, and what freezing actually costs is the option nobody exercised.
/// The price is stated and pinned by
/// `a_frozen_routing_table_costs_one_rent_exempt_minimum_per_market`.
///
/// The readback is not decoration. It is what makes the freeze a fact rather
/// than an intention: the table must come back owned by the lookup-table
/// program, non-executable, with NO authority, not deactivating, activated
/// strictly before the observation slot, and holding exactly the address list
/// this function planned.
pub(crate) fn publish_routing_table(
    rpc: &mut Rpc,
    payer: &Keypair,
    label: &str,
    instructions: &[Instruction],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(Observation, Vec<ObservedAccount>)> {
    let addresses = canonical_routing_addresses_v1(payer.pubkey(), instructions)?;
    publish_routing_table_over_v1(rpc, payer, label, &addresses, transactions)
}

/// Publish one frozen routing table over an address set the CALLER states.
///
/// [`publish_routing_table`] derives its set from a probe compile of the
/// instructions, which is right for every founding and activation here. It is
/// not right for a General Hot frame: `compile_general_hot_v0` requires the
/// table to equal `canonical_general_lookup_addresses_v3` exactly, and only the
/// General operator can compute that. So the set becomes an argument and the
/// create/extend/freeze/readback discipline stays one function.
pub(crate) fn publish_routing_table_over_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    label: &str,
    addresses: &[Pubkey],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(Observation, Vec<ObservedAccount>)> {
    let recent_slot = rpc.finalized_slot()?;
    let plan =
        build_lookup_table_creation_v1(payer.pubkey(), payer.pubkey(), recent_slot, addresses)
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
    // Freezing AFTER the last extension is the whole ordering constraint: the
    // plan is one complete extension sequence, and nothing in this tree appends
    // to one of these tables later. A stage that needed to would have to freeze
    // after its own last append instead, and say so here.
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
    // A table is only usable strictly after the slot that last extended it.
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
        return Err(Error::new(format!(
            "{label} routing table was not exact, frozen, active, and activated"
        )));
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoundInfrastructureCoordinatesV1 {
    profile: Pubkey,
    registry_artifact_id: [u8; 32],
    registry_raw: Pubkey,
    registry_staging: Pubkey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FoundInfrastructureSelectionV1 {
    /// A cohort BORN at V2: initialization committed the genesis V2 and no
    /// succession has run or is planned.
    Genesis,
    PlannedSuccessor,
}

/// Which V2 profile does this founding authenticate against?
///
/// There used to be a third answer, `Predecessor`, taken whenever a plan had no
/// succession and no V2 stood on chain: it fed the 144-byte V1 at the V1 PDA
/// into the projection. After `2951b226` moved every Core reader to
/// "V2 only, and never a fallback" that arm could not produce a foundable
/// projection at all -- it was not dead code, it was the arm a genesis cohort
/// always took, and it always failed sixty transactions deep with a coarse
/// `AccountAuthority`. Measured on cohort-9 at the cost of two stranded
/// collateral mints. It is deleted rather than left beside this one, per
/// AGENTS.md: a superseded authority path does not survive its successor.
///
/// So a plan with no succession now REQUIRES its genesis V2 on chain. Absence
/// is not a fallback; it is the initialize stage not having run.
fn checked_found_infrastructure_selection_v1(
    plan: &SuccessorPlan,
    successor_profile_observed: bool,
) -> Result<FoundInfrastructureSelectionV1> {
    match (
        plan.infrastructure_succession.is_some(),
        successor_profile_observed,
    ) {
        (false, true) => Ok(FoundInfrastructureSelectionV1::Genesis),
        (false, false) => Err(Error::new(
            "Found requires the genesis V2 infrastructure profile at the V2 domain: this plan              carries no succession, so the cohort is born at V2, and Core authenticates the V2              profile and nothing else. The V2 PDA is vacant -- run the initialize stage.",
        )),
        (true, _) => Ok(FoundInfrastructureSelectionV1::PlannedSuccessor),
    }
}

/// Authenticate an observed born-at-V2 profile against the plan's own pin.
///
/// Nothing is taken on the account's word: the bytes must be exactly the pin
/// `prepare` derived, the pin must decode as a V2 that `born_at_v2()`, and the
/// address must be the V2 PDA under this plan's Core. A succeeded profile
/// carries real predecessor ids and fails `born_at_v2()` here, so this arm can
/// never quietly found against a cohort whose ceremony has run without its
/// succession plan.
fn checked_genesis_found_plan_v1(
    plan: &SuccessorPlan,
    profile_address: Pubkey,
    profile_account: &RpcAccount,
) -> Result<SuccessorPlan> {
    let core = pubkey(&plan.core.program_id)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let rent = pubkey(&plan.rent_credit.program_id)?;
    let expected_address =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;
    let pinned = decode_hex(&plan.genesis_infrastructure_profile.body_hex)?;
    let observed = ProtocolInfrastructureProfileV2::decode(&profile_account.data)
        .map_err(|error| Error::new(format!("genesis Found infrastructure: {error:?}")))?;
    if profile_address != expected_address
        || plan.genesis_infrastructure_profile.address != expected_address.to_string()
        || profile_account.owner != core
        || profile_account.executable
        || profile_account.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
        || profile_account.data != pinned
        || !observed.born_at_v2()
        || observed.registry().program().to_bytes() != registry.to_bytes()
        || observed.rent().program().to_bytes() != rent.to_bytes()
    {
        return Err(Error::new(
            "the observed genesis V2 infrastructure profile is not this plan's exact born-at-V2 pin",
        ));
    }
    // The founding path reads `plan.infrastructure_profile`, so the selected
    // plan points it at the V2 the chain actually authenticates. The V1 stays
    // exactly where initialization sealed it; nothing reads it again.
    let mut selected = plan.clone();
    selected.infrastructure_profile = plan.genesis_infrastructure_profile.clone();
    Ok(selected)
}

fn checked_successor_found_coordinates_v1(
    plan: &SuccessorPlan,
    profile_address: Pubkey,
    profile_account: &RpcAccount,
) -> Result<FoundInfrastructureCoordinatesV1> {
    let succession = plan.infrastructure_succession.as_ref().ok_or_else(|| {
        Error::new("successor Found infrastructure selection omitted its succession pin")
    })?;
    let core = pubkey(&plan.core.program_id)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let rent = pubkey(&plan.rent_credit.program_id)?;
    let expected_profile =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;
    let predecessor_bytes = decode_hex(&plan.infrastructure_profile.body_hex)?;
    let predecessor = ProtocolInfrastructureProfileV1::decode(&predecessor_bytes)
        .map_err(|error| Error::new(format!("Found predecessor infrastructure: {error:?}")))?;
    let successor = ProtocolInfrastructureProfileV2::decode(&profile_account.data)
        .map_err(|error| Error::new(format!("Found successor infrastructure: {error:?}")))?;
    if profile_address != expected_profile
        || profile_account.owner != core
        || profile_account.executable
        || profile_account.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
        || predecessor.registry().program().to_bytes() != registry.to_bytes()
        || predecessor.rent().program().to_bytes() != rent.to_bytes()
        || succession.predecessor_registry_artifact_release_id
            != hex(predecessor.registry().artifact_release().as_bytes())
        || succession.predecessor_rent_artifact_release_id
            != hex(predecessor.rent().artifact_release().as_bytes())
        || successor.registry().program() != predecessor.registry().program()
        || successor.rent() != predecessor.rent()
        || successor.predecessor_registry_artifact() != predecessor.registry().artifact_release()
        || successor.predecessor_rent_artifact() != predecessor.rent().artifact_release()
        || successor.registry().artifact_release() == predecessor.registry().artifact_release()
    {
        return Err(Error::new(
            "Found successor profile changed its V1 generation, program binding, or exact V2 account authority",
        ));
    }
    let registry_artifact_id = successor.registry().artifact_release().to_bytes();
    let registry_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &registry_artifact_id,
        ],
        &registry,
    )
    .0;
    let registry_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &registry_artifact_id,
        ],
        &registry,
    )
    .0;
    Ok(FoundInfrastructureCoordinatesV1 {
        profile: expected_profile,
        registry_artifact_id,
        registry_raw,
        registry_staging,
    })
}

fn checked_successor_found_plan_v1(
    plan: &SuccessorPlan,
    profile_account: &RpcAccount,
    coordinates: FoundInfrastructureCoordinatesV1,
    registry_raw: Pubkey,
    registry_raw_account: &RpcAccount,
    registry_staging: Pubkey,
    registry_staging_account: Option<&RpcAccount>,
) -> Result<SuccessorPlan> {
    let registry = pubkey(&plan.registry.program_id)?;
    let expected_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &coordinates.registry_artifact_id,
        ],
        &registry,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &coordinates.registry_artifact_id,
        ],
        &registry,
    )
    .0;
    let registry_raw_digest: [u8; 32] = Sha256::digest(&registry_raw_account.data).into();
    if registry_raw != expected_raw
        || registry_raw != coordinates.registry_raw
        || registry_staging != expected_staging
        || registry_staging != coordinates.registry_staging
        || registry_raw_account.owner != registry
        || registry_raw_account.executable
        || registry_raw_digest != coordinates.registry_artifact_id
        || registry_staging_account.is_some()
    {
        return Err(Error::new(
            "Found successor Registry artifact raw/staging coordinates or finalized body changed",
        ));
    }

    let mut selected = plan.clone();
    selected.infrastructure_profile.address = coordinates.profile.to_string();
    selected.infrastructure_profile.schema_id = hex(&PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V2);
    selected.infrastructure_profile.body_sha256 = hex(&Sha256::digest(&profile_account.data));
    selected.infrastructure_profile.body_hex = hex(&profile_account.data);
    selected.infrastructure_profile.registry_artifact_release_id =
        hex(&coordinates.registry_artifact_id);
    selected.records.insert(
        "registry_artifact_release".into(),
        RecordPair {
            raw: coordinates.registry_raw.to_string(),
            staging: coordinates.registry_staging.to_string(),
            schema_id: hex(&dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1),
            content_sha256: hex(&coordinates.registry_artifact_id),
            body_hex: hex(&registry_raw_account.data),
        },
    );
    Ok(selected)
}

fn authenticated_found_infrastructure_plan_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
) -> Result<SuccessorPlan> {
    let core = pubkey(&plan.core.program_id)?;
    let profile =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;
    let profile_account = rpc.account(profile)?;
    if checked_found_infrastructure_selection_v1(plan, profile_account.is_some())?
        == FoundInfrastructureSelectionV1::Genesis
    {
        let profile_account = profile_account.ok_or_else(|| {
            Error::new("genesis Found selection lost its observed V2 infrastructure profile")
        })?;
        return checked_genesis_found_plan_v1(plan, profile, &profile_account);
    }
    match crate::campaign::succession_state(rpc, plan)? {
        crate::campaign::StageStateV1::Complete => {}
        crate::campaign::StageStateV1::Absent => {
            return Err(Error::new(
                "Found requires the planned Registry succession before selecting V2 infrastructure",
            ));
        }
        crate::campaign::StageStateV1::Partial(detail) => {
            return Err(Error::new(format!(
                "Found refuses partially completed Registry succession: {detail}"
            )));
        }
        crate::campaign::StageStateV1::Conflict(detail) => {
            return Err(Error::new(format!(
                "Found refuses conflicting Registry succession: {detail}"
            )));
        }
    }
    let profile_account = profile_account.ok_or_else(|| {
        Error::new("Found completed succession omitted its successor V2 infrastructure profile")
    })?;
    let coordinates = checked_successor_found_coordinates_v1(plan, profile, &profile_account)?;
    let registry_raw_account = rpc.required_account(
        coordinates.registry_raw,
        "successor Registry artifact raw record",
    )?;
    let registry_staging_account = rpc.account(coordinates.registry_staging)?;
    checked_successor_found_plan_v1(
        plan,
        &profile_account,
        coordinates,
        coordinates.registry_raw,
        &registry_raw_account,
        coordinates.registry_staging,
        registry_staging_account.as_ref(),
    )
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
    let mut keys = vec![
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
    if let Some(price_gate) = records.price_gate {
        keys.push(price_gate.raw);
        keys.push(price_gate.staging);
    }
    authenticate_found_snapshot_coordinates_v3(
        &keys,
        records.manifest.raw,
        records
            .price_gate
            .map(|record| (record.raw, record.staging)),
    )?;
    Ok(keys)
}

fn authenticate_found_snapshot_coordinates_v3(
    keys: &[Pubkey],
    capability_manifest_raw: Pubkey,
    price_gate: Option<(Pubkey, Pubkey)>,
) -> Result<()> {
    let expected = if price_gate.is_some() {
        FOUND_PRICE_GATE_ACCOUNT_COUNT_V3
    } else {
        FOUND_ACCOUNT_COUNT_V3
    };
    if keys.len() != expected
        || keys.get(FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3) != Some(&capability_manifest_raw)
        || keys.get(FOUND_RENT_SYSVAR_INDEX_V3) != Some(&sysvar::rent::ID)
        || price_gate.is_some_and(|(raw, staging)| {
            keys.get(FOUND_ACCOUNT_COUNT_V3) != Some(&raw)
                || keys.get(FOUND_ACCOUNT_COUNT_V3 + 1) != Some(&staging)
        })
    {
        return Err(Error::new(
            "ordinary Found capability-manifest or optional price-gate coordinate drifted",
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
    let expected = if records.price_gate.is_some() {
        PROJECT_FOUND_PRICE_GATE_ACCOUNT_COUNT_V2
    } else {
        PROJECT_FOUND_ACCOUNT_COUNT_V2
    };
    if keys.len() != expected {
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
        price_gate: records
            .price_gate
            .map(|published| snapshot.finalized_record(rpc, published))
            .transpose()?,
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
        &append_distinct_census_accounts_v1(
            instruction,
            CONTROLLER_FUNDING_CLEANUP_CENSUS_PADDING_V1,
        ),
    )?;
    let refused = projected_bootstrap_compiled_geometry_v2(
        payer,
        &append_distinct_census_accounts_v1(
            instruction,
            CONTROLLER_FUNDING_CLEANUP_CENSUS_PADDING_V1 + 1,
        ),
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

/// Lowercase hexadecimal, for the diagnostic lines this module writes to the
/// run's own evidence. Not a wire encoding and not a parser's inverse.
fn lower_hex_v1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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

/// Split the founding manifest into its Resolution-companion mask and the one
/// selected trade entry the Trading controller funds.
///
/// The selected kind is the one the input's own capability closure derived —
/// Direct's, or a family-neutral closure's — so this census is
/// capability-neutral: exactly one entry of the selected kind whose release is
/// not the Resolution release, and three exact Resolution companions.
fn selected_founding_controller_masks_v1(
    manifest: CapabilityManifestV1<'_>,
    resolution_release: [u8; 32],
    selected_kind: [u8; 32],
) -> Result<(u16, [u16; 2])> {
    let mut selected_index = None;
    let mut resolution_mask = 0_u16;
    for entry_index in 0..manifest.entry_count() {
        let entry = manifest
            .entry(entry_index)
            .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
        let bit = 1_u16
            .checked_shl(u32::from(entry_index))
            .ok_or_else(|| Error::new("capability entry mask overflow"))?;
        if entry.kind_id().to_bytes() == selected_kind {
            if selected_index.replace(entry_index).is_some()
                || entry.release_id().to_bytes() == resolution_release
            {
                return Err(Error::new(
                    "the founding manifest must contain exactly one non-Resolution selected \
                     trade entry",
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
    let selected_index = selected_index
        .ok_or_else(|| Error::new("the founding manifest omitted its selected capability entry"))?;
    let trading_mask = 1_u16
        .checked_shl(u32::from(selected_index))
        .ok_or_else(|| Error::new("selected capability entry mask overflow"))?;
    let required_union = manifest_required_union_v1(manifest.entry_count())?;
    if manifest.entry_count() != 4
        || resolution_mask.count_ones() != 3
        || resolution_mask & trading_mask != 0
        || resolution_mask | trading_mask != required_union
    {
        return Err(Error::new(
            "founding requires one selected trade entry and three exact Resolution companions",
        ));
    }
    Ok((selected_index, [resolution_mask, trading_mask]))
}

/// Convert the founding fixture's collateral reserve into complete-set units.
///
/// The reserve policy owns one floor already: the lower half of the minted
/// collateral remains the founding budget, exactly as it did for categorical
/// `Q = 1`. ProductBasis owns the conversion from complete sets to collateral,
/// so that half must be an exact multiple of `Q`; accepting another floor here
/// would create a second rounding boundary and an unclassified remainder.
fn founding_quantity_v1(initial_collateral_atoms: u64, basis_scale: u64) -> Result<u64> {
    let founding_budget = initial_collateral_atoms
        .checked_div(2)
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::new("collateral supply cannot fund a founding"))?;
    let quantity = founding_budget
        .checked_div(basis_scale)
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::new("basis scale exceeds the founding collateral reserve"))?;
    if quantity.checked_mul(basis_scale) != Some(founding_budget) {
        return Err(Error::new(
            "founding collateral reserve is not exactly divisible by basis scale",
        ));
    }
    Ok(quantity)
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
    // The PUBLISHED manifest, never the declared one. A market with bounded
    // Source material has its manifest rebuilt before publication (the Source
    // entry's config id becomes the compiled Source's digest instead of the
    // floor template's), and the Market identity, the record, and Core's
    // `authenticate_derived_capability_root` are all bound to the rebuilt
    // bytes. Deriving the capability-root selection from
    // `input.capability_manifest_hex` here instead made every such market
    // unfoundable: Core rebuilt a different root and refused with a bare
    // `CoreSbfError::Reference` nine stages into an atomic founding, naming
    // none of this. Direct markets never saw it because an unbounded Source
    // never triggers the rebuild, so declared and published were the same
    // bytes.
    let manifest_bytes = records.manifest_body.clone();
    if record_identity(&manifest_bytes) != records.manifest.digest {
        return Err(Error::new(
            "the founding artifact's capability manifest is not the manifest this market \
             published; the derived capability root would refuse on chain",
        ));
    }
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
        selected_founding_controller_masks_v1(
            manifest,
            resolution_release,
            crate::selected_capability::selected_capability_kind_v1(input)?,
        )?;
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
        // The rate THIS cluster charges, read from the connection the founding is
        // about to run on. The ledger records what its own account is funded at,
        // so a later exactness check asks the founding's figure and not whatever
        // the sysvar says at the moment of the check.
        let funded_rent_rate = derive_funded_rent_rate_v2(
            rpc.minimum_balance(0)?,
            bytes.len(),
            rpc.minimum_balance(bytes.len())?,
        )
        .map_err(|error| Error::new(format!("funded rent rate: {error:?}")))?;
        FundingLedgerV2::initialize(
            &mut bytes,
            manifest_id,
            manifest,
            selected_mask,
            funded_rent_rate,
        )
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

    // The complete-set quantity times the authenticated basis scale is the
    // Hoard principal. Core and Claims independently recover this same value
    // from the finalized ProductBasisV3 record. The reserve policy remains
    // byte-identical at scale one: exactly the lower half of the fixture
    // collateral funds founding. At larger scales that same half must divide
    // exactly, rather than manufacturing a quantity whose principal exceeds
    // the Mint supply or hiding a remainder at the rounding boundary.
    let basis_scale = records.basis_scale;
    let quantity = founding_quantity_v1(input.initial_collateral_atoms, basis_scale)?;
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
///
/// One author, `crate::collateral_release`. A founding SELECTS the newest
/// release; a reader ADMITS any this tree implements. Spelling a constructor
/// here again is how the two questions get one answer.
fn collateral_adapter_release_id() -> [u8; 32] {
    crate::collateral_release::founded_collateral_adapter_release_id_v1()
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

/// The generation the OPEN Market occupies for one founding input.
///
/// This is the founding lane's generation — the same author
/// `derive_founding_targets` uses to place the DCLTGMF3 Open Market — exposed
/// so consumers that meet a LIVE Open Market can check the chain's identity
/// against the founding derivation instead of restating the offset.
pub(crate) fn open_market_generation_v1(input: &MarketRunInput) -> Result<u64> {
    PrestateLaneV1::Founding.generation(input)
}

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
                    DCLTCFQ1_SUBMISSION_LABEL_V1,
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
    let refused = rpc
        .send_v0_expected_failure_with_signers(
            "DCLTPCA1 refuses to abort a funded source before expiry",
            &[
                transfer(&payer.pubkey(), &rollback_recipient, 1),
                abort.clone(),
            ],
            payer,
            &[beneficiary],
            routing,
            &tables,
        )?
        // CustodySbfError::Expiry. The kernel distinguishes "too early" from every
        // other reason an abort can be refused, and this probe is about the clock
        // alone -- a selection or frame refusal here would mean the abort never
        // reached the expiry gate and the wait below proves nothing.
        .refusing(0x600B)?;
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
/// (12), Claims (33), Open (23), and the durable funding checkpoint. The total is
/// `GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 + physical_funding_count`.
///
/// Claims and Open each grew by two when the failure escrow was seated at
/// founding (decision 0025 item 2): the escrow's Position and admission, both
/// derived, written by Claims and hashed by Core's Open.
const GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3: usize = 129;
/// DCLTGMF3 account-frame revision with an appended DCLTPGT1 raw/staging pair.
const GENERIC_MARKET_FOUNDING_PRICE_GATE_FIXED_ACCOUNTS_V4: usize =
    GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 + 2;
const GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3: usize = 2;
/// Distinct writable keys the composed founding frame carries.
///
/// TWO AUTHORS IS ONE TOO MANY, and this constant is the survivor. Until
/// 2026-09-04 the composed route compared against a BARE LITERAL twelve while
/// the split route compared against this constant, so seating the failure
/// escrow -- which appends two writable accounts and nothing else -- moved one
/// and not the other, and six host tests refused a founding the program was
/// perfectly happy with. The escrow's contribution is now read from the
/// PROGRAM'S own declaration rather than counted again here, so the next frame
/// change moves this number by construction.
const GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3: usize =
    GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_BEFORE_ESCROW_V3
        + dclutch_claims::founding_v5::CLAIMS_FOUNDING_ESCROW_ACCOUNT_COUNT_V6;

/// Message keys no address-lookup table can move: the payer, the invoked
/// program and the ComputeBudget program.
const GENERIC_MARKET_FOUNDING_CENSUS_STATIC_KEYS_V3: usize = 3;

/// The twelve this frame carried before the failure escrow was seated: the
/// Found authority, the Market, the RentCredit, the permit, both Custody
/// replays, both vaults, the Claims aggregate, the founder Position, its
/// admission, and the controller funding checkpoint.
const GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_BEFORE_ESCROW_V3: usize = 12;
const GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3: usize = 60;
const GENERIC_MARKET_FOUNDING_PRICE_GATE_COMPLETE_KEYS_V4: usize =
    GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3 + 2;

/// Stage-1 split founding instruction: Lock, Found, Realize, Claims, permit
/// escrowed, no Open.
///
/// Owned by `programs/dclutch-trading-sbf/src/generic_founding_stages_v1.rs`
/// (`GENERIC_FOUND_AND_PERMIT_MAGIC_V1`); restated here because a localhost
/// host utility does not depend on an SBF program crate.
const GENERIC_FOUND_AND_PERMIT_MAGIC_V1: [u8; 8] = *b"DCLTGFP1";
const GENERIC_FOUND_AND_PERMIT_CALLER_BUMP_COUNT_V1: usize = 4;
const GENERIC_FOUND_AND_PERMIT_INSTRUCTION_BYTES_V1: usize =
    8 + GENERIC_FOUND_AND_PERMIT_CALLER_BUMP_COUNT_V1;

/// Exact width of the composed frame's commit-last Core Open window.
const GENERIC_FOUNDING_OPEN_WINDOW_ACCOUNTS_V1: usize = 23;

/// Exact `DCLTGFP1` frame width before the founding's FundingLedgerV2 tail:
/// the composed `DCLTGMF3` frame minus its Core Open window, checkpoint last.
const GENERIC_FOUND_AND_PERMIT_FIXED_ACCOUNTS_V1: usize =
    GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 - GENERIC_FOUNDING_OPEN_WINDOW_ACCOUNTS_V1;
const GENERIC_FOUND_AND_PERMIT_PRICE_GATE_FIXED_ACCOUNTS_V2: usize =
    GENERIC_FOUND_AND_PERMIT_FIXED_ACCOUNTS_V1 + 2;

/// The Open caller PDA is the only key unique to the Open window — every other
/// Open-window key already appears in the Lock, Found, Realize, or Claims
/// windows — so the stage-1 census is the composed census minus exactly one
/// readonly loaded key, and the twelve distinct writable keys are unchanged.
const GENERIC_FOUND_AND_PERMIT_COMPLETE_KEYS_V1: usize =
    GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3 - 1;
const GENERIC_FOUND_AND_PERMIT_PRICE_GATE_COMPLETE_KEYS_V2: usize =
    GENERIC_FOUND_AND_PERMIT_COMPLETE_KEYS_V1 + 2;

/// Stage-2 split founding instruction: the commit-last Core Open alone,
/// consuming the escrowed permit.
///
/// Owned by `programs/dclutch-trading-sbf/src/generic_founding_stages_v1.rs`
/// (`GENERIC_MARKET_OPEN_MAGIC_V1`).
const GENERIC_MARKET_OPEN_MAGIC_V1: [u8; 8] = *b"DCLTGMO1";
const GENERIC_MARKET_OPEN_INSTRUCTION_BYTES_V1: usize = 8 + 1;

/// Two readonly raw requests — the selected Found artifact and the Claims
/// request — then Core's exact 21-account Open window. Small enough for
/// inline v0 with no lookup table and no extended heap.
const GENERIC_MARKET_OPEN_FRAME_ACCOUNTS_V1: usize = 2 + GENERIC_FOUNDING_OPEN_WINDOW_ACCOUNTS_V1;

/// The Market, the permit, and the RentCredit: Core Open's sole mutations.
const GENERIC_MARKET_OPEN_DISTINCT_WRITABLE_V1: usize = 3;

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

/// The addresses a routing table for this frame may carry.
///
/// This used to derive them here, and it PUSHED the invoked program id into the
/// table. A program id can never be resolved through a table -- it has to be
/// known before the message's tables load -- so the runtime kept it inline
/// anyway and the entry bought nothing while its rent was paid forever. The
/// derivation was also a second author for a rule the message compiler already
/// owns, and a third copy of it sits in the relayed vertical, which documents
/// itself as mirroring this function.
///
/// So it asks instead. `canonical_route_lookup_addresses_v1` offers the compiler
/// every address the frame names and keeps the ones the compiler resolved
/// through a table, which cannot drift from what the runtime will do.
///
/// This is message-invariant by construction and the census tests are the
/// proof: the program id was already static in every compiled message, so
/// dropping it from the TABLE moves no static key, no lookup index, and no
/// packet byte. What changes is the table's own width, and the rent that width
/// costs: 32 bytes per entry at the default rent, 222,720 lamports each, held
/// for as long as the table exists.
/// Bytes an empty lookup table occupies before its first address.
///
/// Discriminator, deactivation slot, last-extended slot and index, and the
/// optional authority. Used only to price ONE entry as a difference of two
/// rent-exempt minima, so the exact base cancels; it is named rather than
/// inlined so the subtraction reads as the difference it is.
const LOOKUP_TABLE_META_BYTES_V1: usize = 56;

fn canonical_routing_addresses_v1(
    payer: Pubkey,
    instructions: &[Instruction],
) -> Result<Vec<Pubkey>> {
    canonical_route_lookup_addresses_v1(payer, instructions)
        .map_err(|error| Error::new(format!("routing table addresses: {error:?}")))
}

fn compiled_complete_lock_census_v1(
    payer: Pubkey,
    instruction: &Instruction,
) -> Result<CompleteLockCensusV1> {
    let addresses = canonical_routing_addresses_v1(payer, std::slice::from_ref(instruction))?;
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
    let bare_frame = GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3
        .checked_add(GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3)
        .ok_or_else(|| Error::new("DCLTGMF3 expected frame overflow"))?;
    let gated_frame = GENERIC_MARKET_FOUNDING_PRICE_GATE_FIXED_ACCOUNTS_V4
        .checked_add(GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3)
        .ok_or_else(|| Error::new("gated DCLTGMF3 expected frame overflow"))?;
    let gated = instruction.accounts.len() == gated_frame;
    if instruction.data.len() != GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3
        || instruction.data.get(..8) != Some(GENERIC_MARKET_FOUNDING_MAGIC_V3.as_slice())
        || (instruction.accounts.len() != bare_frame && !gated)
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
    let expected_complete = if gated {
        GENERIC_MARKET_FOUNDING_PRICE_GATE_COMPLETE_KEYS_V4
    } else {
        GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3
    };
    let expected_readonly = if gated { 45 } else { 43 };
    if census.complete_keys != expected_complete
        || census.required_signatures != 1
        || census.static_keys != 3
        || census.loaded_writable != GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3
        || census.loaded_readonly != expected_readonly
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
    /// The failure escrow's Position and admission, derived from the Market's
    /// own `ClaimsCapability` owner at the last coordinate. Both are supplied
    /// on every founding; both are written only when the Market's basis record
    /// refunds on failure.
    escrow_position: Pubkey,
    escrow_admission: Pubkey,
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
    // Decision 0025 item 2. The escrow owner is the ClaimsCapability PDA at
    // `(market, claim_count - 1)`, which is `refunding_failure_index`'s answer
    // for this width, and its Position and admission are the ordinary PDAs
    // under that owner. Nothing here is chosen -- a host that derived a
    // different escrow would be refused by `FailureEscrow` 0x5010.
    let escrow_failure_selector = claim_count
        .checked_sub(1)
        .filter(|_| claim_count >= 2)
        .ok_or_else(|| Error::new("no failure escrow is derivable at this runtime width"))?;
    let escrow_owner = Pubkey::find_program_address(
        &dclutch_claims::protocol_position_v2::ProtocolPositionClaimsCapabilitySeedsV2::new(
            coordinates.market.to_bytes(),
            escrow_failure_selector,
        )
        .map_err(|error| Error::new(format!("Claims escrow owner seeds: {error:?}")))?
        .as_slices(),
        &claims_program,
    )
    .0;
    let escrow_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), escrow_owner.to_bytes())
            .map_err(|error| Error::new(format!("Claims escrow position seeds: {error:?}")))?
            .as_slices(),
        &claims_program,
    )
    .0;
    let escrow_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), escrow_owner.to_bytes())
            .map_err(|error| Error::new(format!("Claims escrow admission seeds: {error:?}")))?
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
        escrow_position,
        escrow_admission,
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
    /// The Market's derived failure escrow (decision 0025 item 2).
    escrow_position: Pubkey,
    escrow_admission: Pubkey,
    /// Whether this founding's basis record refunds on failure, and therefore
    /// whether the two escrow accounts are written and must be pre-funded.
    ///
    /// Read off the record the founding already authenticated, through
    /// `categorical_refunds_on_failure_v3`, which is the sole author of the
    /// rule. The host does not choose it and the program does not take it from
    /// the host: both derive it from the same record.
    seats_failure_escrow: bool,
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
    let registry = pubkey(&plan.registry.program_id)?;
    let claims_program = pubkey(&plan.claims.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let release_set = hex32(&plan.release_set_id)?;
    let principal = coordinates.lock.amount;
    // The bump derivation below reaches for the Market and Realm record
    // addresses on its way to their bumps. Requiring it to land on the ones
    // this founding already holds is what stops a right-looking bump derived
    // from the wrong coordinates.
    require_predicted_bump_coordinates_v1(core, registry, coordinates, records)?;

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
    eprintln!(
        "campaign: founding projected Realize request {} digest {}",
        lower_hex_v1(&realize_raw),
        lower_hex_v1(&realize_digest)
    );

    // The candidate Core state the Found stage writes. Every field of it is
    // fixed by the kernel's `found`: the phase and readiness are constants, the
    // identity is the one this campaign already derived the Market address
    // from, and the rent beneficiary is the founding generation's credit. It is
    // cross-checked against the chain in `authenticate_core_state_encoding_v1`
    // before anything commits to its digest.
    //
    // The bump tail is part of that encoding, not a decoration on it.
    // `programs/dclutch-core-sbf/src/found.rs` fills `StateBumpsV1` from the
    // Market-address search it has already performed and from the Realm record
    // pair's own bumps, and the Found stage hashes THAT candidate into the
    // projected Realize receipt whose digest reaches the permit through
    // `FoundingIntentV5`. A bump byte this side leaves zero moves the permit's
    // Claims request digest and the founding refuses at
    // `ClaimsFoundingSbfErrorV5::Release` three legs later, naming nothing.
    let market_state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Prepaid,
        terminal_winner: 0,
        identity: coordinates.identity,
        outstanding_capabilities: 0,
        principal_cap_sets: coordinates.principal_cap_sets,
        rent_beneficiary: identity_of(coordinates.credit.to_bytes())?,
        terminal_receipt: None,
        bumps: predicted_state_bumps_v1(
            core,
            registry,
            coordinates.identity,
            records,
            core_product_graph_projection_v1(&plan.core, plan.checked_local_mutable_set.as_ref())?,
            // This Market is founded by Core's PROJECTED Found, whose frame
            // carries no linked-basis record. Naming the ordinary walk here is
            // the defect this argument exists to make unspellable.
            CoreProductGraphWalkV1::ProjectedFounding,
        )?,
    };
    let market_state_bytes = market_state
        .encode()
        .map_err(|error| Error::new(format!("candidate Core state: {error:?}")))?;
    let market_state_digest: [u8; 32] = Sha256::digest(market_state_bytes).into();
    eprintln!(
        "campaign: founding candidate Core state {} digest {}",
        lower_hex_v1(&market_state_bytes),
        lower_hex_v1(&market_state_digest)
    );

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
    eprintln!(
        "campaign: founding projected Realize receipt {} digest {}",
        lower_hex_v1(&realize_receipt_bytes),
        lower_hex_v1(&realize_receipt_digest)
    );

    // One semantic owner for the final coordinates. Completed-crash recovery
    // calls the same helper against finalized Open state; it never tries to
    // replay the SourceFunded kernel state that DCLTGMF3 consumed and closed.
    let poststate =
        derive_founding_poststate_expectation_v1(plan, coordinates, founder, claim_count)?;
    let permit = poststate.permit;
    let aggregate = poststate.aggregate;
    let position = poststate.position;
    let admission = poststate.admission;
    let escrow_position = poststate.escrow_position;
    let escrow_admission = poststate.escrow_admission;
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
    // The supervisor's half of the founding-intent byte diff.
    //
    // Claims can only report that its permit's intent hashes to something the
    // request does not carry; it cannot report which coordinate moved, because
    // every coordinate it can compare has already been compared by the time it
    // refuses. So the side that COMPILED the request states its preimage where
    // the run's own evidence keeps it -- this goes to stderr, which the
    // gauntlet captures verbatim as `<run>/campaign.stderr` beside the ledger
    // and the accounts -- and the diff against the permit's bytes names the
    // field. One line per founding derivation, and this is a local-validator
    // harness: nothing here reaches a program.
    eprintln!(
        "campaign: founding intent preimage {} digest {}",
        lower_hex_v1(&intent_bytes),
        lower_hex_v1(&intent_digest)
    );

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
        escrow_position,
        escrow_admission,
        seats_failure_escrow: records.basis_refunds_on_failure,
        aggregate_width,
        position_width,
        market_rent: coordinates.found.market_rent(),
        permit_rent: coordinates.found.permit_rent(),
    })
}

/// Predict the PDA bump tail Core's `found` kernel records in a Market state.
///
/// `programs/dclutch-core-sbf/src/found.rs` fills `StateBumpsV1` from the
/// Market-address search it already performs and from the Realm record pair's
/// bumps, and every one of those seeds is a function of the Market identity —
/// `identity.realm_id` IS the Realm record's content digest, which with
/// `REALM_SCHEMA_RELEASE_ID_V1` is the whole of that pair's derivation. So this
/// projection can be computed for a Market that does not exist yet, which is
/// what the founding needs: it commits to `sha256(CoreState)` two stages before
/// Core writes one.
///
/// A zero bump is `StateBumpsV1`'s unrecorded encoding and cannot be carried, so
/// a search that lands on bump zero is a refusal here rather than a silently
/// unrecorded tail that would disagree with Core's.
///
/// WHICH CORE IS AN ARGUMENT, NOT AN ASSUMPTION. The Product-graph half of the
/// tail arrived in `b312ce3c4`, so a Core deployed before it writes zeros in
/// those four reserved bytes and one deployed after writes eight nibbles. This
/// function predicts for the build the caller names, because the thing being
/// predicted is what the DEPLOYED program does; `crate::core_bump_projection`
/// answers which build that is, and refuses at campaign start rather than here
/// when it cannot. Assuming the answer is what cohort-14c paid 0.139 SOL and a
/// live Found37 Market to disprove.
/// Which of Core's two Product-graph walks founded the Market being predicted.
///
/// `found.rs` has two producers of `References::product_record_bumps` and they
/// reach different numbers of records, because they are handed different
/// account frames:
///
/// * `authenticate_references`, the ordinary `Found`, has the linked-basis
///   record in its frame and walks all four -- Product, ResultDomain,
///   Portfolio, basis -- so all eight nibbles are recorded.
/// * `authenticate_projected_references`, the generic founding's projected
///   `Found`, does NOT: the basis record's content digest arrives with the
///   Trading frame, Core never sees the record, and its walk reaches three.
///   The basis pair stays zero, which is `ProductGraphBumpsV1`'s encoding of
///   "not mined; this reader searches".
///
/// MEASURED 2026-09-03, and this argument exists because of it. This driver
/// predicted the four-record tail for BOTH, so the generic founding committed
/// to a `sha256(CoreState)` whose last packed byte was `0x12` where Core wrote
/// `0x00` -- one byte, in the pair Core cannot reach. It cost the whole
/// campaign: the projected Realize receipt hashes that digest, the receipt's
/// digest is a coordinate of the founding intent, and Claims refused the
/// founding on `0x518D PermitBody`, "intent digest is not the request's
/// founding_intent_digest", 195 transactions in, naming no field. Every one of
/// the twenty-nine joins that route runs passed; the only intent coordinate
/// nothing on that route can compare is the one that moved.
///
/// The reason the driver believed the wrong thing is worth keeping: its one
/// cross-check, `authenticate_core_state_encoding_v1`, proves the prediction
/// against the Found37 Market -- which is founded by the ORDINARY walk. It was
/// a positive control for the wrong walk, and it passed on every run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreProductGraphWalkV1 {
    /// `found::authenticate_references`: four records, eight nibbles.
    OrdinaryFound,
    /// `found::authenticate_projected_references`: three records, and the
    /// linked-basis pair left unrecorded.
    ProjectedFounding,
}

fn predicted_state_bumps_v1(
    core: Pubkey,
    registry: Pubkey,
    identity: MarketIdentity,
    records: &MarketRecords,
    projection: CoreProductGraphProjectionV1,
    walk: CoreProductGraphWalkV1,
) -> Result<StateBumpsV1> {
    let market_bump =
        Pubkey::find_program_address(&MarketCoreStateSeedsV2::new(identity).as_slices(), &core).1;
    let realm_digest = identity.realm_id.to_bytes();
    let raw_bump = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &registry,
    )
    .1;
    let staging_bump = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &registry,
    )
    .1;
    // The Product graph pair-by-pair, in the reader's walk order. Core fills
    // these from `authenticate_founding_product_basis_v3`, whose four record
    // derivations are the RAW/STAGING pair under the Registry at each canonical
    // schema id and the record's own content digest -- and the four digests are
    // exactly what `publish_record` returned for the records this campaign put
    // on chain. A pair this side leaves zero moves the same permit digest the
    // realm pair does.
    let mut product_graph = [0_u8; PRODUCT_GRAPH_BUMP_COUNT];
    for (slot, (schema, digest)) in [
        (PRODUCT_RECORD_SCHEMA_ID_V2, records.product.digest),
        (RESULT_DOMAIN_SCHEMA_ID_V2, records.domain.digest),
        (PORTFOLIO_SCHEMA_ID_V2, records.portfolio.digest),
        (GRADED_BASIS_RECORD_SCHEMA_ID_V3, records.basis.digest),
    ]
    .into_iter()
    .enumerate()
    {
        let seeds: [&[u8]; 2] = [schema.as_slice(), digest.as_slice()];
        let raw =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, seeds[0], seeds[1]], &registry)
                .1;
        let staging = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, seeds[0], seeds[1]],
            &registry,
        )
        .1;
        if let Some(cell) = product_graph.get_mut(slot * 2) {
            *cell = raw;
        }
        if let Some(cell) = product_graph.get_mut(slot * 2 + 1) {
            *cell = staging;
        }
    }
    // The basis pair is slot 3 of the walk, which is bytes 6 and 7 of the bank.
    // The projected founding's Core never derives it, so predicting it is
    // predicting a byte Core does not write. Cleared here rather than skipped
    // above, so the derivation that `require_predicted_bump_coordinates_v1`
    // checks the founding's coordinates against still runs in full.
    if walk == CoreProductGraphWalkV1::ProjectedFounding {
        if let Some(pair) = product_graph.get_mut(6..8) {
            pair.fill(0);
        }
    }
    let bumps = StateBumpsV1 {
        market: StateBumpsV1::record(market_bump),
        realm_raw_record: StateBumpsV1::record(raw_bump),
        realm_staging_record: StateBumpsV1::record(staging_bump),
        // A Core from before `b312ce3c4` never wrote here, and `ABSENT` is the
        // encoding of what it left: four zero bytes, eight unrecorded nibbles.
        // The walk above still runs -- the derivation is what
        // `require_predicted_bump_coordinates_v1` checks the founding's
        // coordinates against -- and only the projection is withheld.
        product_graph: match projection {
            CoreProductGraphProjectionV1::Recorded => ProductGraphBumpsV1::record(product_graph),
            CoreProductGraphProjectionV1::Unrecorded => ProductGraphBumpsV1::ABSENT,
        },
    };
    if bumps.market.is_none()
        || bumps.realm_raw_record.is_none()
        || bumps.realm_staging_record.is_none()
    {
        return Err(Error::new(
            "a Market or Realm record PDA searched down to bump zero, which StateBumpsV1 cannot carry",
        ));
    }
    Ok(bumps)
}

/// Require the bump derivation to land on the founding's own coordinates.
///
/// [`predicted_state_bumps_v1`] reaches for the Market and Realm record
/// addresses on its way to their bumps and then discards them. Comparing them
/// against the addresses this founding already holds is what stops a
/// right-looking bump tail derived from the wrong Market identity: the bumps
/// themselves are single bytes and would collide often enough to be useless as
/// their own evidence.
fn require_predicted_bump_coordinates_v1(
    core: Pubkey,
    registry: Pubkey,
    coordinates: &FoundingCoordinates,
    records: &MarketRecords,
) -> Result<()> {
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(coordinates.identity).as_slices(),
        &core,
    )
    .0;
    if market != coordinates.market {
        return Err(Error::new(
            "recorded-bump derivation reached a Market other than the founding's",
        ));
    }
    if coordinates.identity.realm_id.to_bytes() != records.realm.digest {
        return Err(Error::new(
            "the founding Market identity names a Realm digest the published record does not have",
        ));
    }
    let realm_digest = records.realm.digest;
    let raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &registry,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &registry,
    )
    .0;
    if raw != records.realm.raw || staging != records.realm.staging {
        return Err(Error::new(
            "recorded-bump derivation reached a Realm record pair other than the published one",
        ));
    }
    Ok(())
}

/// Prove the candidate Core state is encoded exactly the way the chain writes.
///
/// The founding commits to `sha256(CoreState)` two stages before that state
/// exists, so an encoding that differed from Core's by one byte would produce a
/// permit the Claims stage refuses and a failure with no visible cause. Two
/// things are required of the Market the chain is already holding:
///
/// * re-encoding its own decoded state reproduces its bytes, and
/// * the PDA bump tail it carries is the one [`predicted_state_bumps_v1`]
///   predicts for it.
///
/// The second is the load-bearing half. Round-tripping alone passes on any
/// state the codec can decode, INCLUDING one whose tail this driver would have
/// left zero — which is exactly the drift that opened when Core started
/// recording those bumps, and which cost a whole campaign to find by hand.
fn authenticate_core_state_encoding_v1(
    rpc: &mut Rpc,
    market: Pubkey,
    core: Pubkey,
    registry: Pubkey,
    records: &MarketRecords,
    projection: CoreProductGraphProjectionV1,
) -> Result<()> {
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
    let predicted = predicted_state_bumps_v1(
        core,
        registry,
        state.identity,
        records,
        projection,
        // This Market was created by Core's ORDINARY Found, which walks all
        // four Product-graph records.
        CoreProductGraphWalkV1::OrdinaryFound,
    )?;
    if predicted != state.bumps {
        return Err(Error::new(format!(
            "the Market the chain holds carries bump tail {:?}, and this driver predicts \
             {predicted:?}; a founding projected from this driver's CoreState would commit to a \
             digest Core never writes",
            state.bumps
        )));
    }
    // AND THE WALK THE FOUNDING ACTUALLY USES, against the same chain bytes.
    //
    // The check above is a positive control for the ORDINARY walk only, and
    // that is exactly how a four-record prediction rode into a three-record
    // founding for as long as the tier has been red: it passed on every run
    // while the tail the founding committed to was wrong in one byte. So the
    // projected walk is stated too, and its expected value is derived from the
    // tail the CHAIN holds -- Core's own bytes with the pair Core's projected
    // frame cannot reach cleared -- rather than from a second call to the same
    // predictor agreeing with the first.
    let projected = predicted_state_bumps_v1(
        core,
        registry,
        state.identity,
        records,
        projection,
        CoreProductGraphWalkV1::ProjectedFounding,
    )?;
    let mut without_linked_basis = state.bumps.product_graph.bumps();
    if let Some(pair) = without_linked_basis.get_mut(6..8) {
        pair.fill(0);
    }
    let expected = StateBumpsV1 {
        product_graph: ProductGraphBumpsV1::record(without_linked_basis),
        ..state.bumps
    };
    if projected != expected {
        return Err(Error::new(format!(
            "the projected founding's bump tail is not the chain's tail without its linked-basis \
             pair: the chain holds {:?}, so a projected founding must commit to {expected:?}, and \
             this driver predicts {projected:?}",
            state.bumps
        )));
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

    let gate_extension = if records.price_gate.is_some() { 2 } else { 0 };
    let mut accounts: Vec<AccountMeta> = Vec::with_capacity(
        GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3
            .checked_add(gate_extension)
            .and_then(|width| width.checked_add(coordinates.funding_ledgers.len()))
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
    if let Some(price_gate) = records.price_gate {
        push(price_gate.raw, false, &mut accounts);
        push(price_gate.staging, false, &mut accounts);
    }

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
    // The failure escrow, appended by the V6 Claims founding frame. Present on
    // every founding and written only by a refunding one.
    push(outer.escrow_position, true, &mut accounts);
    push(outer.escrow_admission, true, &mut accounts);

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
    push(outer.escrow_position, true, &mut accounts);
    push(outer.escrow_admission, true, &mut accounts);

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
        .checked_add(gate_extension)
        .and_then(|width| width.checked_add(coordinates.funding_ledgers.len()))
        .ok_or_else(|| Error::new("founding frame width overflow"))?;
    if accounts.len() != expected {
        return Err(Error::new(format!(
            "assembled founding frame did not match its exact width: assembled {}, expected {} ({} fixed + {} funding ledgers)",
            accounts.len(),
            expected,
            GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 + gate_extension,
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
    if distinct_writable.len() != GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3 {
        return Err(Error::new(format!(
            "the founding frame declared {} writable keys, not the {} the outer requires",
            distinct_writable.len(),
            GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3,
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

/// Build the exact 104 + physical-funding-count account `DCLTGFP1` frame.
///
/// This is a second independent derivation of the composed `DCLTGMF3` frame
/// with its 21-account Core Open window removed and the controller-funding
/// checkpoint kept last, and the corpus asserts the splice equality against
/// `build_generic_market_founding_v3` meta for meta. The union-writability
/// discipline and the twelve distinct writable keys are unchanged, because
/// every writable Open-window key — the Market, the permit, the RentCredit,
/// the projected replay, the Hoard and source vaults, and the three Claims
/// accounts — is already writable in an earlier window; only the readonly
/// Open caller PDA leaves the frame.
// Wired by the split campaign path next; the corpus below executes it today.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn build_generic_found_and_permit_v3(
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
        GENERIC_FOUND_AND_PERMIT_FIXED_ACCOUNTS_V1
            .checked_add(coordinates.funding_ledgers.len())
            .ok_or_else(|| Error::new("DCLTGFP1 frame width overflow"))?,
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
    // this founding's FundingLedgerV2 tail, and the fifteen-account suffix.
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
    if let Some(price_gate) = records.price_gate {
        push(price_gate.raw, false, &mut accounts);
        push(price_gate.staging, false, &mut accounts);
    }

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
    // The failure escrow, appended by the V6 Claims founding frame. Present on
    // every founding and written only by a refunding one.
    push(outer.escrow_position, true, &mut accounts);
    push(outer.escrow_admission, true, &mut accounts);

    // No Open window: the escrowed permit carries the founding to the
    // `DCLTGMO1` stage. The durable CustodyStaged checkpoint is still
    // authenticated before Lock and consumed by THIS stage — it binds the
    // pre-Lock custody prestate, which this transaction is what consumes —
    // so it stays the final physical account, writable for its close-last
    // transition.
    push(
        coordinates.controller_funding_checkpoint,
        true,
        &mut accounts,
    );

    let gate_extension = if records.price_gate.is_some() { 2 } else { 0 };
    let expected = GENERIC_FOUND_AND_PERMIT_FIXED_ACCOUNTS_V1
        .checked_add(gate_extension)
        .and_then(|width| width.checked_add(coordinates.funding_ledgers.len()))
        .ok_or_else(|| Error::new("DCLTGFP1 frame width overflow"))?;
    if accounts.len() != expected {
        return Err(Error::new(format!(
            "assembled DCLTGFP1 frame did not match its exact width: assembled {}, expected {} ({} fixed + {} funding ledgers)",
            accounts.len(),
            expected,
            GENERIC_FOUND_AND_PERMIT_FIXED_ACCOUNTS_V1 + gate_extension,
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
    let mut distinct_writable: Vec<Pubkey> = Vec::new();
    for key in &writable {
        if !distinct_writable.contains(key) {
            distinct_writable.push(*key);
        }
    }
    // The same keys as the composed route: removing the Open window removes no
    // writable key. A different count here means the shared windows drifted,
    // not that this assertion needs adjusting.
    if distinct_writable.len() != GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3 {
        return Err(Error::new(format!(
            "the DCLTGFP1 frame declared {} writable keys, not the {} the outer requires",
            distinct_writable.len(),
            GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3,
        )));
    }
    let mut data = Vec::with_capacity(GENERIC_FOUND_AND_PERMIT_INSTRUCTION_BYTES_V1);
    data.extend_from_slice(&GENERIC_FOUND_AND_PERMIT_MAGIC_V1);
    data.extend_from_slice(&[
        outer.lock_caller_bump,
        outer.found_authority_bump,
        outer.realize_caller_bump,
        outer.claims_caller_bump,
    ]);
    if data.len() != GENERIC_FOUND_AND_PERMIT_INSTRUCTION_BYTES_V1 {
        return Err(Error::new("DCLTGFP1 caller-bump encoding width changed"));
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

/// Build the exact 23-account `DCLTGMO1` frame: two readonly raw requests,
/// then Core's 21-account Open window.
///
/// The window is byte-identical in key order to the composed route's Open
/// section, and the corpus asserts that equality. Writability differs by
/// design: standalone, only the Market, the permit, and the RentCredit are
/// writable — the exact three accounts Core Open mutates — where the composed
/// frame carried union privileges from its earlier windows.
// Wired by the split campaign path next; the corpus below executes it today.
#[allow(dead_code)]
fn build_generic_market_open_v1(
    plan: &SuccessorPlan,
    coordinates: &FoundingCoordinates,
    outer: &FoundingOuterV1,
    found_raw_account: Pubkey,
    claims_raw_account: Pubkey,
) -> Result<PreparedGenericMarketFoundingV3> {
    if found_raw_account == claims_raw_account {
        return Err(Error::new(
            "DCLTGMO1 requires two distinct readonly raw requests",
        ));
    }
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

    let mut accounts: Vec<AccountMeta> = Vec::with_capacity(GENERIC_MARKET_OPEN_FRAME_ACCOUNTS_V1);
    let push = |key: Pubkey, writable: bool, accounts: &mut Vec<AccountMeta>| {
        accounts.push(if writable {
            AccountMeta::new(key, false)
        } else {
            AccountMeta::new_readonly(key, false)
        });
    };

    push(found_raw_account, false, &mut accounts);
    push(claims_raw_account, false, &mut accounts);

    // Core Open, commit-last, 21 accounts: the composed route's exact window.
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
    push(coordinates.projected_replay, false, &mut accounts);
    push(coordinates.hoard_vault, false, &mut accounts);
    push(coordinates.source_vault, false, &mut accounts);
    push(outer.aggregate, false, &mut accounts);
    push(outer.position, false, &mut accounts);
    push(outer.admission, false, &mut accounts);
    push(outer.escrow_position, false, &mut accounts);
    push(outer.escrow_admission, false, &mut accounts);

    if accounts.len() != GENERIC_MARKET_OPEN_FRAME_ACCOUNTS_V1 {
        return Err(Error::new(format!(
            "assembled DCLTGMO1 frame did not match its exact width: assembled {}, expected {}",
            accounts.len(),
            GENERIC_MARKET_OPEN_FRAME_ACCOUNTS_V1,
        )));
    }
    let mut distinct_writable: Vec<Pubkey> = Vec::new();
    for meta in &accounts {
        if meta.is_writable && !distinct_writable.contains(&meta.pubkey) {
            distinct_writable.push(meta.pubkey);
        }
    }
    if distinct_writable.len() != GENERIC_MARKET_OPEN_DISTINCT_WRITABLE_V1 {
        return Err(Error::new(format!(
            "the DCLTGMO1 frame declared {} writable keys, not the three Core Open mutates",
            distinct_writable.len()
        )));
    }
    let data = {
        let mut data = Vec::with_capacity(GENERIC_MARKET_OPEN_INSTRUCTION_BYTES_V1);
        data.extend_from_slice(&GENERIC_MARKET_OPEN_MAGIC_V1);
        data.push(outer.open_authority_bump);
        if data.len() != GENERIC_MARKET_OPEN_INSTRUCTION_BYTES_V1 {
            return Err(Error::new("DCLTGMO1 caller-bump encoding width changed"));
        }
        data
    };
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

/// The `DCLTGFP1` analogue of the composed route's compiled lock census.
// Wired by the split campaign path next; the corpus below executes it today.
#[allow(dead_code)]
fn authenticate_generic_found_and_permit_lock_census_v3(
    payer: Pubkey,
    prepared: &PreparedGenericMarketFoundingV3,
) -> Result<CompleteLockCensusV1> {
    let instruction = &prepared.instruction;
    let bare_frame = GENERIC_FOUND_AND_PERMIT_FIXED_ACCOUNTS_V1
        .checked_add(GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3)
        .ok_or_else(|| Error::new("DCLTGFP1 expected frame overflow"))?;
    let gated_frame = GENERIC_FOUND_AND_PERMIT_PRICE_GATE_FIXED_ACCOUNTS_V2
        .checked_add(GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3)
        .ok_or_else(|| Error::new("gated DCLTGFP1 expected frame overflow"))?;
    let gated = instruction.accounts.len() == gated_frame;
    if instruction.data.len() != GENERIC_FOUND_AND_PERMIT_INSTRUCTION_BYTES_V1
        || instruction.data.get(..8) != Some(GENERIC_FOUND_AND_PERMIT_MAGIC_V1.as_slice())
        || (instruction.accounts.len() != bare_frame && !gated)
        || instruction.accounts.iter().any(|meta| meta.is_signer)
        || instruction.accounts.iter().any(|meta| meta.pubkey == payer)
        || exact_instruction_frame_digest_v1(instruction) != prepared.lock_expectation.frame_digest
    {
        return Err(Error::new(
            "DCLTGFP1 exact frame/key/order/privilege expectation changed before compilation",
        ));
    }
    let census = compiled_complete_lock_census_v1(payer, instruction)?;
    require_devnet_complete_key_limit_v1(census)?;
    let expected_complete = if gated {
        GENERIC_FOUND_AND_PERMIT_PRICE_GATE_COMPLETE_KEYS_V2
    } else {
        GENERIC_FOUND_AND_PERMIT_COMPLETE_KEYS_V1
    };
    let expected_readonly = if gated { 44 } else { 42 };
    if census.complete_keys != expected_complete
        || census.required_signatures != 1
        || census.static_keys != 3
        || census.loaded_writable != GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3
        || census.loaded_readonly != expected_readonly
    {
        return Err(Error::new(format!(
            "DCLTGFP1 compiled lock census refused: {} complete, {} static, {} writable loaded, {} readonly loaded, {} signatures",
            census.complete_keys,
            census.static_keys,
            census.loaded_writable,
            census.loaded_readonly,
            census.required_signatures,
        )));
    }
    Ok(census)
}

/// Frame admission for the inline `DCLTGMO1` stage-2 transaction.
///
/// No lookup-table census: 23 frame keys plus the payer and the ComputeBudget
/// program lock 25 complete keys, under the devnet 64-key limit by simple
/// arithmetic, and the frame carries no loaded accounts at all.
// Wired by the split campaign path next; the corpus below executes it today.
#[allow(dead_code)]
fn authenticate_generic_market_open_frame_v1(
    payer: Pubkey,
    prepared: &PreparedGenericMarketFoundingV3,
) -> Result<()> {
    let instruction = &prepared.instruction;
    let mut distinct: Vec<Pubkey> = Vec::new();
    let mut distinct_writable: Vec<Pubkey> = Vec::new();
    for meta in &instruction.accounts {
        if !distinct.contains(&meta.pubkey) {
            distinct.push(meta.pubkey);
        }
        if meta.is_writable && !distinct_writable.contains(&meta.pubkey) {
            distinct_writable.push(meta.pubkey);
        }
    }
    // Payer, ComputeBudget, and the Trading program id (already a frame key).
    let complete = distinct
        .len()
        .checked_add(2)
        .ok_or_else(|| Error::new("DCLTGMO1 complete-key census overflow"))?;
    if instruction.data.len() != GENERIC_MARKET_OPEN_INSTRUCTION_BYTES_V1
        || instruction.data.get(..8) != Some(GENERIC_MARKET_OPEN_MAGIC_V1.as_slice())
        || instruction.accounts.len() != GENERIC_MARKET_OPEN_FRAME_ACCOUNTS_V1
        || instruction.accounts.iter().any(|meta| meta.is_signer)
        || instruction.accounts.iter().any(|meta| meta.pubkey == payer)
        || exact_instruction_frame_digest_v1(instruction) != prepared.lock_expectation.frame_digest
        || distinct.len() != GENERIC_MARKET_OPEN_FRAME_ACCOUNTS_V1
        || distinct_writable.len() != GENERIC_MARKET_OPEN_DISTINCT_WRITABLE_V1
        || complete > DEVNET_ACCOUNT_LOCK_LIMIT_V1
    {
        return Err(Error::new(
            "DCLTGMO1 exact frame/key/order/privilege expectation refused",
        ));
    }
    Ok(())
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
    recovery_policy: bool,
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
    authenticate_resolved_founding_message_v1(
        operation,
        recovery_policy,
        &compiled.message,
        routing_tables,
    )?;
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
    if base.complete_keys != operation.exact_unique_accounts(recovery_policy)
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
    // The terminal poststate satisfies every earlier completion contract: once
    // the atomic founding consumed the staged readiness into an Open Market,
    // each stage's goal state has been strictly surpassed, and a lazily
    // finalized earlier journal re-running its completion must not refuse the
    // success it was part of producing.
    if matches!(observed, FundingReadinessPlanV1::ConsumedByFounding) {
        return Ok(());
    }
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
    // The frame's own coordinates decide the width: the recovery record's
    // raw/staging pair is in the frame exactly when the market has a recovery
    // policy, so the pin reads the presence from the same struct that builds
    // the accounts rather than from a second opinion.
    let geometry = authenticate_funding_readiness_compiled_geometry_v1(
        payer.pubkey(),
        operation,
        coordinates.recovery_policy.is_some(),
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
        // Bookkeeping only, and this plan has no clock consumer: the absence
        // is recorded on the connection and restated in the run's report.
        observation_block_time: _,
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
        FundingReadinessPlanV1::ConsumedByFounding => {
            completed.push(
                "resolution funding readiness is terminal: the atomic founding consumed the \
                 staged readiness and the Market is Open; nothing adjacent remains to drive"
                    .into(),
            );
            return Ok(());
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
        // Bookkeeping only, and this plan has no clock consumer: the absence
        // is recorded on the connection and restated in the run's report.
        observation_block_time: _,
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
        FundingReadinessPlanV1::ConsumedByFounding => {
            completed.push(
                "resolution funding readiness is terminal after the create stage: the atomic \
                 founding consumed the staged readiness and the Market is Open"
                    .into(),
            );
            return Ok(());
        }
    }

    let FundingReadinessRoutedPlanV1 {
        plan: current,
        routing_tables,
        // Bookkeeping only, and this plan has no clock consumer: the absence
        // is recorded on the connection and restated in the run's report.
        observation_block_time: _,
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
        FundingReadinessPlanV1::ConsumedByFounding => {
            completed.push(
                "resolution funding readiness is terminal after the activate stage: the atomic \
                 founding consumed the staged readiness and the Market is Open"
                    .into(),
            );
            return Ok(());
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

/// Pre-fund the five accounts founding allocates but never funds.
///
/// Nothing in the protocol funds these five. Core allocates the Market and the
/// permit and Claims allocates its three accounts with `allocate` and `assign`
/// only, never a transfer, so each must already hold its rent. The Market and
/// the permit are checked for EXACT equality with their rent minima, and all
/// three Claims balances are folded byte-exactly into the permit's committed
/// Claims request, so this transaction is part of the founding's authenticated
/// prestate and not a convenience. It is identical for the composed and split
/// routes: both create the same five accounts, only in different transactions.
fn prefund_founding_accounts_v1(
    rpc: &mut Rpc,
    outer: &FoundingOuterV1,
    market: Pubkey,
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let aggregate_rent = rpc.minimum_balance(outer.aggregate_width)?;
    let position_rent = rpc.minimum_balance(outer.position_width)?;
    let admission_rent = rpc.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)?;
    // Seven accounts on a refunding Market, five on a categorical one. THE
    // FOUNDER PAYS THE ESCROW'S RENT: the escrow is the Market's own identity
    // and has no funds of its own, so its Position and admission are
    // pre-funded here exactly as the founder's three are. A categorical
    // founding pre-funds neither -- the program leaves both accounts vacant,
    // and funding them would strand two rent-exempt minima in addresses
    // nothing will ever allocate.
    let mut prefunding = vec![
        (market, outer.market_rent),
        (outer.permit, outer.permit_rent),
        (outer.aggregate, aggregate_rent),
        (outer.position, position_rent),
        (outer.admission, admission_rent),
    ];
    if outer.seats_failure_escrow {
        prefunding.push((outer.escrow_position, position_rent));
        prefunding.push((outer.escrow_admission, admission_rent));
    }
    let observed_prefunding = prefunding
        .iter()
        .map(|(address, _)| rpc.account(*address))
        .collect::<Result<Vec<_>>>()?;
    if observed_prefunding.iter().all(Option::is_none) {
        transactions.push(
            rpc.send(
                "pre-fund the founding's program-allocated accounts",
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
            "campaign: exact founding pre-funding already finalized; resumed without a second debit"
        );
    } else {
        return Err(Error::new(
            "founding pre-funding is partial or differs from the exact rent principals; never top up or overwrite it",
        ));
    }
    authenticate_founding_prefunding_v1(rpc, outer, market)?;
    Ok(())
}

/// Publish the founding's Realize, Claims, and substituted-Claims requests.
///
/// The Realize and Claims requests join the founding artifact and terminal Lock
/// the campaign already published; a substituted request is a substituted
/// address, so the outer's frame checks catch it. Each publication is verified
/// against its pre-mutation derivation. Returns the substituted-Claims record
/// address the hostile probe substitutes into the frame. Shared by both routes.
#[allow(clippy::too_many_arguments)]
fn publish_founding_request_records_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    payer: &Keypair,
    outer: &FoundingOuterV1,
    expected_realize_record: PublishedRecord,
    expected_claims_record: PublishedRecord,
    expected_substituted_claims_record: PublishedRecord,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<Pubkey> {
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
    Ok(substituted_claims_record.raw)
}

/// Found the Market atomically on a real validator: `DCLTGMF3`.
///
/// Five stages in one rollback domain against the prestate `DCLTPCB2` left.
/// The Market is created by the Found stage and Opened by the last, so this
/// single transaction is the whole distance between a projected-Custody
/// prestate and a live Market with a Claims aggregate, a founder Position, and
/// a Hoard holding the collateral.
///
/// When the run input selects [`FoundingRouteV1::Split`], founding runs as two
/// transactions instead; this function delegates to
/// [`execute_split_market_founding`] at the top, sharing the entire prestate
/// prologue up to the founding submission itself.
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
    if input.founding_route == FoundingRouteV1::Split {
        return execute_split_market_founding(
            rpc,
            plan,
            input,
            records,
            coordinates,
            product,
            mint,
            found31_market,
            actors,
            found_raw_account,
            lock_raw_account,
            claim_count,
            payer,
            transactions,
            accounts,
            completed,
        );
    }
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let claims_program = pubkey(&plan.claims.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    authenticate_core_state_encoding_v1(
        rpc,
        found31_market,
        core,
        registry,
        records,
        core_product_graph_projection_v1(&plan.core, plan.checked_local_mutable_set.as_ref())?,
    )?;
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

    prefund_founding_accounts_v1(rpc, &outer, coordinates.market, payer, transactions)?;

    let substituted_claims_raw = publish_founding_request_records_v1(
        rpc,
        registry,
        payer,
        &outer,
        expected_realize_record,
        expected_claims_record,
        expected_substituted_claims_record,
        transactions,
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
    let (routing, tables) = publish_routing_table(
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
        .pubkey = substituted_claims_raw;
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
            // The one-shot permit is NOT here: the commit-last Open stage
            // consumes it inside the same transaction and
            // authenticate_open_market_poststate_v1 already requires its
            // ABSENCE. The journal's completion capture reads every account
            // listed here as required-present, so listing the permit refused
            // every founding that succeeded - after the transaction landed.
            let completion_addresses = vec![
                coordinates.market,
                outer.aggregate,
                outer.position,
                outer.admission,
                coordinates.hoard_vault,
                coordinates.projected_replay,
            ];
            // A RESUME reaches this closure with the founding already
            // Complete, and on a resume the three post-Open funding stages may
            // themselves already be Finalized -- cohort-13 is exactly that
            // state. A fresh founding finds none of them, so the resolver is
            // empty here and the Open verifier refuses in its own words as
            // before.
            let later = LaterFoundingStagesV1::authenticated(
                &recorder.binding,
                FoundingSubmissionOperationV1::Dcltgmf3,
                &recorder.ordered(),
            )?;
            let mut completion = |rpc: &mut Rpc| {
                authenticate_open_market_poststate_v1(
                    &mut BoundaryRpcV1::after_boundary(rpc, &later),
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

    // Same clock as the completion closure above: on a fresh founding this
    // resolver is empty and nothing changes, on a resume it is the only reason
    // the post-Open funding stages are not read as Open's own tampering.
    let later_than_open = match submission_recorder.as_deref() {
        Some(recorder) => Some(LaterFoundingStagesV1::authenticated(
            &recorder.binding,
            FoundingSubmissionOperationV1::Dcltgmf3,
            &recorder.ordered(),
        )?),
        None => None,
    };
    authenticate_open_market_poststate_v1(
        &mut match &later_than_open {
            Some(later) => BoundaryRpcV1::after_boundary(rpc, later),
            None => BoundaryRpcV1::at_boundary(rpc),
        },
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

/// Found the Market in two transactions: `DCLTGFP1` then `DCLTGMO1`.
///
/// The prestate (`DCLTPCB2` and everything before it) is identical to the
/// composed route; only the founding submission itself splits. Stage 1 commits
/// the Market in its `Founding` phase, escrows the one-shot Core permit, and
/// realizes the collateral into normal Custody — one rollback domain that
/// closes the controller-funding checkpoint as it commits. Stage 2 is the
/// permissionless commit-last Open: it consumes the permit and opens the
/// Market, and every coordinate of that open is pinned by the escrowed permit,
/// so nobody but the founding's own committed intent can steer it.
///
/// This run uses no durable submission journal: it is a local proof of the
/// route, and the composed route's `None` submission path is the same shape.
/// Durable-journal split submission is a follow-on.
#[allow(clippy::too_many_arguments)]
fn execute_split_market_founding(
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
) -> Result<()> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let claims_program = pubkey(&plan.claims.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    authenticate_core_state_encoding_v1(
        rpc,
        found31_market,
        core,
        registry,
        records,
        core_product_graph_projection_v1(&plan.core, plan.checked_local_mutable_set.as_ref())?,
    )?;
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

    // Admission before mutation: derive the still-vacant request coordinates and
    // assemble both stage frames now, so a lock drift stops before any transfer
    // or publication.
    let expected_realize_record =
        derive_raw_request_record_v1(registry, "projected-custody-realize", &outer.realize_raw)?;
    let expected_claims_record =
        derive_raw_request_record_v1(registry, "claims-founding-v5", &outer.claims_raw)?;
    let expected_substituted_claims_record = derive_raw_request_record_v1(
        registry,
        "claims-founding-v5-substituted-founder",
        &outer.substituted_claims_raw,
    )?;
    let prepared_stage1 = build_generic_found_and_permit_v3(
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
    let stage1_census =
        authenticate_generic_found_and_permit_lock_census_v3(payer.pubkey(), &prepared_stage1)?;
    let prepared_stage2 = build_generic_market_open_v1(
        plan,
        coordinates,
        &outer,
        found_raw_account,
        expected_claims_record.raw,
    )?;
    authenticate_generic_market_open_frame_v1(payer.pubkey(), &prepared_stage2)?;
    eprintln!(
        "campaign: split founding admission — DCLTGFP1 {} complete keys ({} writable loaded), DCLTGMO1 {} accounts inline",
        stage1_census.complete_keys,
        stage1_census.loaded_writable,
        prepared_stage2.instruction.accounts.len(),
    );

    // The stage-1 frame carries the four readonly raw requests first; index 3 is
    // the Claims request the substituted-Claims hostile swaps. The stage-2 frame
    // is two readonly raws then Core's Open window, so the permit — Open-window
    // index 2 — is frame index 4.
    const STAGE1_CLAIMS_RAW_FRAME_INDEX_V1: usize = 3;
    const STAGE2_PERMIT_FRAME_INDEX_V1: usize = 2 + 2;

    // Stage 2 and the stage-2 hostiles are inline v0 with no lookup table, so
    // this finalized observation resolves no table entries; it is required by
    // the v0 send signature but inert here.
    let stage2_slot = rpc.finalized_slot()?;
    let stage2_observation = Observation {
        slot: stage2_slot,
        unix_timestamp: rpc.block_time(stage2_slot)?,
        finality: dclutch_versioned_message_operator::Finality::Finalized,
    };

    prefund_founding_accounts_v1(rpc, &outer, coordinates.market, payer, transactions)?;
    let substituted_claims_raw = publish_founding_request_records_v1(
        rpc,
        registry,
        payer,
        &outer,
        expected_realize_record,
        expected_claims_record,
        expected_substituted_claims_record,
        transactions,
    )?;

    // The prefunded-but-unfounded prestate: every account stage 1 will allocate
    // is still system-owned and empty, the projected-Custody prestate intact.
    let created = [
        ("founding_market", coordinates.market),
        ("founding_permit", outer.permit),
        ("claims_aggregate", outer.aggregate),
        ("founder_position", outer.position),
        ("claims_admission", outer.admission),
    ];
    let prestate_intact = |rpc: &mut Rpc, label: &str| -> Result<()> {
        for (name, key) in created {
            let account = rpc.required_account(key, name)?;
            if account.owner != system_program::ID || !account.data.is_empty() {
                return Err(Error::new(format!("{label} allocated the {name} account")));
            }
        }
        Ok(())
    };
    prestate_intact(rpc, "the split founding prestate")?;

    let (routing1, tables1) = publish_routing_table(
        rpc,
        payer,
        "DCLTGFP1",
        std::slice::from_ref(&prepared_stage1.instruction),
        transactions,
    )?;

    // HOSTILE — stage interleaving: DCLTGMO1 (Open) before DCLTGFP1 (Found).
    // The Market is still the prefunded, system-owned account, so the stage-2
    // outer refuses at its own market-owner check before any CPI, and the
    // prestate is proven untouched.
    match rpc.send_v0_expected_failure(
        "DCLTGMO1 refuses stage-2 Open before its stage-1 Found",
        std::slice::from_ref(&prepared_stage2.instruction),
        payer,
        stage2_observation,
        &[],
    ) {
        Ok(refused) => {
            refused.refusing(0x4003)?;
            prestate_intact(rpc, "the refused stage-interleaving Open")?;
            completed.push(
                "proved DCLTGMO1 refuses (0x4003) a stage-2 Open submitted before its stage-1 Found, prestate untouched".into(),
            );
        }
        Err(error) if error.to_string().contains("dropped") => {
            eprintln!(
                "campaign: EVIDENCE GAP, recorded: the interleaving hostile was undeliverable ({error})"
            );
            completed.push(
                "EVIDENCE GAP: the stage-interleaving hostile was undeliverable on this cluster"
                    .into(),
            );
        }
        Err(error) => return Err(error),
    }

    // HOSTILE — stage 1 refuses a substituted Claims request and rolls the
    // whole Lock/Found/Realize/Claims domain back, exactly as the composed
    // route's own substituted-Claims probe does.
    let mut substituted = prepared_stage1.instruction.clone();
    substituted
        .accounts
        .get_mut(STAGE1_CLAIMS_RAW_FRAME_INDEX_V1)
        .ok_or_else(|| Error::new("the stage-1 frame omitted its Claims request"))?
        .pubkey = substituted_claims_raw;
    let rollback_recipient = crate::seed::fresh_probe_address();
    match rpc.send_v0_on_founding_heap_expected_failure_with_signers(
        "DCLTGFP1 refuses a substituted Claims request and rolls stage 1 back",
        &[
            transfer(&payer.pubkey(), &rollback_recipient, 1),
            substituted,
        ],
        payer,
        &[],
        routing1,
        &tables1,
    ) {
        Ok(rolled_back) => {
            let fee_only = rolled_back.fee_only_balance_change;
            if rpc.account(rollback_recipient)?.is_some() || fee_only != Some(true) {
                return Err(Error::new(format!(
                    "refused stage 1 did not roll its whole transaction back to a fee-only debit: fee_only_balance_change={fee_only:?}",
                )));
            }
            transactions.push(rolled_back);
            prestate_intact(rpc, "the refused substituted-Claims stage 1")?;
            completed.push(
                "proved DCLTGFP1 refuses a substituted Claims request and rolls stage 1 back to a fee-only debit".into(),
            );
        }
        Err(error) if error.to_string().contains("dropped") => {
            eprintln!(
                "campaign: EVIDENCE GAP, recorded: the stage-1 substituted-Claims hostile was undeliverable ({error})"
            );
            completed.push("EVIDENCE GAP: the stage-1 substituted-Claims hostile was undeliverable on this cluster".into());
        }
        Err(error) => return Err(error),
    }

    // Stage 1: the honest Lock/Found/Realize/Claims domain.
    let stage1 = rpc.send_v0_on_founding_heap_with_signers(
        "found and permit stage 1: Lock, Found, Realize, Claims; permit escrowed (DCLTGFP1)",
        std::slice::from_ref(&prepared_stage1.instruction),
        payer,
        &[],
        routing1,
        &tables1,
    )?;
    let stage1_slot = stage1.slot;
    transactions.push(stage1);
    let poststate =
        derive_founding_poststate_expectation_v1(plan, coordinates, actors.founder, claim_count)?;
    authenticate_found_and_permit_poststate_v1(
        rpc,
        coordinates,
        &poststate,
        core,
        claims_program,
        custody,
        token_program,
        mint,
    )?;
    completed.push(
        "executed DCLTGFP1: the Market is FOUNDING with its one-shot Core permit escrowed, the Claims aggregate/Position/admission live, the collateral realized into normal Custody, and the controller-funding checkpoint closed".into(),
    );

    // HOSTILE — wrong permit: a stage-2 Open naming a vacant address where the
    // escrowed permit belongs. The market is Core-owned so the outer's join
    // passes, and Core's own permit authenticator refuses the CPI; the Market
    // stays FOUNDING and the real permit stays escrowed.
    let wrong_permit = crate::seed::fresh_probe_address();
    let mut wrong = prepared_stage2.instruction.clone();
    wrong
        .accounts
        .get_mut(STAGE2_PERMIT_FRAME_INDEX_V1)
        .ok_or_else(|| Error::new("the stage-2 frame omitted its permit"))?
        .pubkey = wrong_permit;
    match rpc.send_v0_expected_failure(
        "DCLTGMO1 refuses a stage-2 Open naming the wrong permit",
        std::slice::from_ref(&wrong),
        payer,
        stage2_observation,
        &[],
    ) {
        Ok(refused) => {
            refused.refusing(0x4004)?;
            authenticate_found_and_permit_poststate_v1(
                rpc,
                coordinates,
                &poststate,
                core,
                claims_program,
                custody,
                token_program,
                mint,
            )?;
            completed.push(
                "proved DCLTGMO1 refuses (0x4004, Core permit authenticator) a wrong-permit Open; the Market stays FOUNDING and the real permit stays escrowed".into(),
            );
        }
        Err(error) if error.to_string().contains("dropped") => {
            eprintln!(
                "campaign: EVIDENCE GAP, recorded: the wrong-permit hostile was undeliverable ({error})"
            );
            completed.push(
                "EVIDENCE GAP: the wrong-permit hostile was undeliverable on this cluster".into(),
            );
        }
        Err(error) => return Err(error),
    }

    // Stage 2: the honest permissionless commit-last Open. Twenty-three
    // accounts fit an inline v0 packet, so it needs no lookup table and no
    // extended heap.
    let stage2 = rpc.send_v0(
        "open the Market last, consuming the escrowed permit (DCLTGMO1)",
        std::slice::from_ref(&prepared_stage2.instruction),
        payer,
        stage2_observation,
        &[],
    )?;
    let open_slot = stage2.slot;
    transactions.push(stage2);
    // The split founding keeps no submission journal, so it has no later
    // stages to resolve through and no reconstruction path reaches it: the
    // caller IS the Open boundary here.
    authenticate_open_market_poststate_v1(
        &mut BoundaryRpcV1::at_boundary(rpc),
        coordinates,
        &poststate,
        core,
        claims_program,
        custody,
        token_program,
        mint,
    )?;

    // HOSTILE — permit replay: resend the exact stage-2 Open. The permit is
    // consumed and the Market is Open, so Core refuses; the poststate is
    // idempotent — the Market stays Open, the permit stays consumed.
    match rpc.send_v0_expected_failure(
        "DCLTGMO1 refuses a replay of a consumed permit",
        std::slice::from_ref(&prepared_stage2.instruction),
        payer,
        stage2_observation,
        &[],
    ) {
        Ok(refused) => {
            refused.refusing(0x4004)?;
            authenticate_open_market_poststate_v1(
                &mut BoundaryRpcV1::at_boundary(rpc),
                coordinates,
                &poststate,
                core,
                claims_program,
                custody,
                token_program,
                mint,
            )?;
            completed.push(
                "proved DCLTGMO1 refuses (0x4004) a replay of a consumed permit; the Market stays Open and the permit stays consumed".into(),
            );
        }
        Err(error) if error.to_string().contains("dropped") => {
            eprintln!(
                "campaign: EVIDENCE GAP, recorded: the permit-replay hostile was undeliverable ({error})"
            );
            completed.push(
                "EVIDENCE GAP: the permit-replay hostile was undeliverable on this cluster".into(),
            );
        }
        Err(error) => return Err(error),
    }

    execute_funding_readiness_suffix_v1(
        rpc,
        plan,
        records,
        coordinates,
        payer,
        transactions,
        accounts,
        completed,
        open_slot.max(stage1_slot),
        &tables1.iter().map(|table| table.key).collect::<Vec<_>>(),
        None,
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
        "executed the two-stage founding: DCLTGFP1 escrowed the permit and DCLTGMO1 consumed it to open the Market last, economically atomic via the permit".into(),
    );
    Ok(())
}

/// Require the exact stage-1 poststate: Market founding, permit escrowed.
///
/// This is [`authenticate_open_market_poststate_v1`] with the Market and permit
/// assertions inverted for the point between the two founding stages: the
/// Market is committed in its `Founding` phase (not `Open`), and the one-shot
/// Core permit is present and escrowed (not consumed). Everything else — the
/// live Claims accounts, the Hoard holding the principal, the closed source
/// compartment, the realized normal Custody replay, and the closed
/// controller-funding checkpoint — is already in the state the composed route
/// reaches, because stage 1 runs those legs identically.
#[allow(clippy::too_many_arguments)]
fn authenticate_found_and_permit_poststate_v1(
    rpc: &mut Rpc,
    coordinates: &FoundingCoordinates,
    expected: &FoundingPoststateExpectationV1,
    core: Pubkey,
    claims_program: Pubkey,
    custody: Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
) -> Result<()> {
    let market = rpc.required_account(coordinates.market, "stage-1 Market")?;
    let state = CoreState::decode(&market.data)
        .map_err(|error| Error::new(format!("stage-1 Market state: {error:?}")))?;
    if market.owner != core
        || market.data.len() != STATE_BYTES
        || market.executable
        || state.phase != Phase::Founding
        || state.readiness != Readiness::Prepaid
        || state.identity != coordinates.identity
        || state.terminal_receipt.is_some()
        || state.rent_beneficiary.to_bytes() != coordinates.credit.to_bytes()
    {
        return Err(Error::new(
            "stage 1 did not commit the Market in the Founding phase its permit is escrowed against",
        ));
    }

    // The one-shot permit is ESCROWED after stage 1: Core-owned, exact width,
    // rent-exempt. This is the opposite of the Open poststate, which requires it
    // consumed.
    let permit = rpc.required_account(expected.permit, "stage-1 escrowed permit")?;
    if permit.owner != core
        || permit.data.len() != SERIES_FOUNDING_PERMIT_BYTES_V1
        || permit.data.iter().all(|byte| *byte == 0)
        || permit.lamports < rpc.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)?
    {
        return Err(Error::new(
            "stage 1 did not escrow the one-shot Core permit stage 2 will consume",
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
                "{label} was not allocated, owned, and written by the stage-1 Claims founding"
            )));
        }
    }

    let hoard = rpc.required_account(coordinates.hoard_vault, "stage-1 Hoard")?;
    let hoard_state = TokenAccount::parse(&hoard.data)
        .map_err(|error| Error::new(format!("Hoard vault: {error:?}")))?;
    if hoard.owner != token_program
        || hoard_state.mint != mint.to_bytes()
        || hoard_state.amount != expected.principal
        || hoard_state.state != AccountState::Initialized
    {
        return Err(Error::new(
            "stage 1 did not leave the Hoard holding exactly the founding principal",
        ));
    }

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
                "{label} was not closed by the stage-1 Lock stage"
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
            "stage 1 did not realize the projected replay into the Market's normal Custody replay",
        ));
    }
    if rpc
        .account(coordinates.controller_funding_checkpoint)?
        .is_some_and(|account| account.lamports != 0 || !account.data.is_empty())
    {
        return Err(Error::new(
            "stage 1 finalized without consuming the controller funding checkpoint",
        ));
    }
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
///
/// Every read here goes through [`BoundaryRpcV1`], because this function's
/// whole subject is one transaction's boundary and the reconstruction path
/// runs it hours after that boundary passed. The classification below is the
/// sweep the cohort-13 recovery lane made of this function on 2026-09-02, and
/// it is what the method names now carry:
///
/// - the founded Market's owner, width, phase, readiness, identity and rent
///   beneficiary; the consumed permit; the Claims aggregate, founder Position
///   and admission; the Hoard's principal; the closed source vault and source
///   replay; the realized normal Custody replay; the consumed controller
///   funding checkpoint -- all PERMANENT. No stage of this founding after Open
///   reopens a closed account, deallocates a Claims record, moves principal, or
///   takes the Market back out of Open; the post-Open stages are Core,
///   Resolution and Trading FUNDING, which move rent.
/// - the Pending controller funding ledgers -- BOUNDARY. Open must not change
///   one while consuming its checkpoint, and that is the whole point of the
///   check; but `core-funding-create-v1`, `resolution-funding-activate-v1` and
///   `core-funding-accept-v1` finalize after Open precisely to move those
///   ledgers off Pending. Read live at recovery time the sentence accused Open
///   of what its own three successors did by design.
#[allow(clippy::too_many_arguments)]
fn authenticate_open_market_poststate_v1(
    rpc: &mut BoundaryRpcV1<'_>,
    coordinates: &FoundingCoordinates,
    expected: &FoundingPoststateExpectationV1,
    core: Pubkey,
    claims_program: Pubkey,
    custody: Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
) -> Result<()> {
    let market = rpc.permanent_required_account(coordinates.market, "founded Market")?;
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
    let permit = rpc.permanent_account(expected.permit)?;
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
        let account = rpc.permanent_required_account(key, label)?;
        if account.owner != owner
            || account.data.len() != width
            || account.data.iter().all(|byte| *byte == 0)
        {
            return Err(Error::new(format!(
                "{label} was not allocated, owned, and written by the Claims founding"
            )));
        }
    }

    let hoard = rpc.permanent_required_account(coordinates.hoard_vault, "founded Hoard")?;
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
        if let Some(account) = rpc.permanent_account(key)?
            && (account.owner != system_program::ID
                || account.lamports != 0
                || !account.data.is_empty())
        {
            return Err(Error::new(format!(
                "{label} was not closed by the founding Lock stage"
            )));
        }
    }
    let replay =
        rpc.permanent_required_account(coordinates.projected_replay, "normal Custody replay")?;
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
        .permanent_account(coordinates.controller_funding_checkpoint)?
        .is_some_and(|account| account.lamports != 0 || !account.data.is_empty())
    {
        return Err(Error::new(
            "the Open acknowledgement finalized without consuming the controller funding checkpoint",
        ));
    }
    for ledger in &coordinates.funding_ledgers {
        rpc.boundary_account(
            ledger.address,
            "Open controller funding ledger",
            |account| {
                account.owner == ledger.controller
                    && account.lamports == ledger.required_lamports
                    && account.data == ledger.bytes
            },
            "Open changed a Pending controller funding ledger while consuming its checkpoint",
        )?;
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
    Pull(&'a dclutch_source::pyth::PythReleaseV1),
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

    fn authenticate_price_update(self, update: &dclutch_source::pyth::FullPriceUpdateV2) -> Result<()> {
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
    /// Raw collateral atoms the founding commits. Was a constant inside the
    /// shared core, which meant a local caller could vary the band and not the
    /// stake -- and a load simulator's markets differ in both.
    pub(crate) initial_collateral_atoms: u64,
    pub(crate) local_participant_fixture_liquidity_atoms: u64,
    /// The author's founding band. `None` refuses at compile by name rather
    /// than defaulting to a belief nobody stated.
    pub(crate) founding_band: Option<crate::model::FoundingBandInputV1>,
    /// The funded ordered ladder this market buys, in rung order.
    ///
    /// `None` is the section-12.7/12.8 no-recovery shape and is what every
    /// caller of this producer has always passed, so a market that buys no
    /// ladder compiles to exactly the bytes it did before this field existed.
    /// `Some` authors an alternative source per rung and the `RecoveryPolicyV2`
    /// that funds them.
    pub(crate) recovery: Option<Vec<PythRecoveryRungV1>>,
}

/// One authored rung of a Pyth market's funded ordered ladder.
///
/// A RUNG SUBSTITUTES A SOURCE AND NOTHING ELSE, and for a Pyth-backed source
/// there is exactly one axis on which two sources of the same feed can differ:
/// the adapter's tolerance for the provider's own stated confidence interval.
/// So that is the only thing a rung authors beside its deadline. A market whose
/// first choice went silent is a market with a reason to demand a
/// better-conditioned reading from its second, and `max_confidence_bps` is
/// capped at the type's 10,000-bp ceiling anyway, so tighter is the only
/// direction available.
#[derive(Clone, Debug)]
pub(crate) struct PythRecoveryRungV1 {
    /// This rung's confidence bound, in basis points.
    pub(crate) max_confidence_bps: u16,
    /// The rung's own committed absolute deadline, in Unix seconds.
    ///
    /// PREPAID AT FOUNDING AND CHOSEN BY THE FOUNDER, which is the whole point
    /// of it living in the policy rather than being inherited from the primary
    /// window's liveness grace: the crank that enters this rung is admissible
    /// one second after the previous leg's deadline, and the capture that
    /// answers it is admissible up to and including this one.
    pub(crate) deadline_unix_seconds: i64,
}

/// What one authored ladder contributes to a market's run spec and manifest.
#[derive(Debug)]
struct AuthoredRecoveryLadderV1 {
    /// Canonical `RecoveryPolicyV2` body, hex, for `recovery_policy_hex`.
    policy_hex: String,
    /// The record pairs the run spec publishes, in rung order.
    records: Vec<crate::model::RecoverySourceRecordsV1>,
    /// `(capability kind, capability config)` for every Resolution compartment
    /// this ladder needs EXCEPT the failure entry: one per rung configured by
    /// that rung's own allocation, then the exhaustion entry configured by the
    /// policy digest.
    entries: Vec<([u8; 32], [u8; 32])>,
}

/// Author one Pyth market's funded ordered ladder from its primary source.
///
/// Everything the rungs share with the primary is READ OFF the primary spec
/// rather than restated: the coordinate domain, the source unit, the provider
/// release, the access profile and the capacity profile. That is what makes the
/// authored ladder satisfy `validate_recovery_source_graph` by construction
/// instead of by coincidence, and it is why this function takes the compiled
/// primary rather than the parameters that produced it.
fn author_pyth_recovery_ladder_v1(
    rungs: &[PythRecoveryRungV1],
    local_label: &[u8; 32],
    feed_id: [u8; 32],
    exponent: i32,
    primary: SourceSpecV1,
    primary_deadline_unix_seconds: i64,
) -> Result<AuthoredRecoveryLadderV1> {
    if rungs.is_empty() {
        return Err(Error::new(
            "a recovery-bearing market must author at least one rung; a policy funding no attempt \
             is the no-recovery market spelled at greater length",
        ));
    }
    let mut attempts = [None; 4];
    let mut records = Vec::with_capacity(rungs.len());
    let mut entries = Vec::with_capacity(rungs.len().saturating_add(1));
    for (index, rung) in rungs.iter().enumerate() {
        let slot = u8::try_from(index).map_err(|_| Error::new("recovery rung index overflow"))?;
        // A rung whose deadline has already passed when the crank that enters
        // it becomes legal is a leg nobody can answer, and a market that sold
        // it sold nothing. The crank onto rung zero is admissible one second
        // after the primary deadline, so that is the floor.
        if rung.deadline_unix_seconds <= primary_deadline_unix_seconds {
            return Err(Error::new(format!(
                "rung {slot} commits to deadline {} and the primary leg is not even over until \
                 {primary_deadline_unix_seconds}: the crank that enters this rung is admissible \
                 only after the primary deadline, so the rung would expire before it opened",
                rung.deadline_unix_seconds
            )));
        }
        let adapter = PythAdapterConfigV1::new(feed_id, exponent, rung.max_confidence_bps)
            .map_err(|error| {
                Error::new(format!("rung {slot} Pyth adapter configuration: {error:?}"))
            })?;
        let adapter_bytes = adapter.to_bytes();
        let spec = SourceSpecV1::new(
            primary.domain_id(),
            primary.unit_id(),
            primary.provider_release_id(),
            primary.access_profile(),
            SourceContentId::new(record_identity(&adapter_bytes))
                .map_err(|error| Error::new(format!("rung {slot} adapter identity: {error:?}")))?,
            primary.capacity_profile_id(),
        );
        let spec_bytes = spec.to_bytes();
        let spec_id = record_identity(&spec_bytes);
        // One allocation identity per rung, because a compartment is found by
        // its CONFIGURATION and two rungs sharing one identity would name one
        // compartment: the first spends it and the second has nothing to be
        // paid from. `RecoveryPolicyV2::validate_shape` refuses that outright,
        // and folding the rung index into the derivation is what stops this
        // producer from ever handing it one.
        let allocation = demo_id("funding-allocation/recovery-rung", &[local_label, &[slot]]);
        attempts[index] = Some(
            RecoveryAttemptV2::new(
                SourceContentId::new(spec_id).map_err(|error| {
                    Error::new(format!("rung {slot} source spec identity: {error:?}"))
                })?,
                primary.provider_release_id(),
                rung.deadline_unix_seconds,
                SourceContentId::new(allocation).map_err(|error| {
                    Error::new(format!("rung {slot} allocation identity: {error:?}"))
                })?,
            )
            .map_err(|error| Error::new(format!("rung {slot} recovery attempt: {error:?}")))?,
        );
        records.push(crate::model::RecoverySourceRecordsV1 {
            source_spec_hex: hex(&spec_bytes),
            pyth_adapter_config_hex: hex(&adapter_bytes),
        });
        entries.push((
            demo_id("capability/recovery-rung", &[local_label, &[slot]]),
            allocation,
        ));
    }
    let count =
        u8::try_from(rungs.len()).map_err(|_| Error::new("recovery rung count overflow"))?;
    let policy = RecoveryPolicyV2::new(primary.capacity_profile_id(), attempts, count)
        .map_err(|error| Error::new(format!("recovery policy: {error:?}")))?;
    let policy_bytes = policy.to_bytes();
    // The exhaustion compartment belongs to no rung, so the POLICY's own digest
    // configures it -- exactly the binding `next_crank_funding_config` reads
    // when the ladder runs out of legs.
    entries.push((
        demo_id("capability/exhaustion-companion", &[local_label]),
        record_identity(&policy_bytes),
    ));
    Ok(AuthoredRecoveryLadderV1 {
        policy_hex: hex(&policy_bytes),
        records,
        entries,
    })
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
/// One rung of a ladder, in the terms a CALLER can state.
///
/// The rung's committed deadline is ABSOLUTE in the record, and no caller of
/// either producer can name an absolute second usefully: the primary leg's own
/// deadline is the captured publication plus the fixture's declared shelf life
/// for the lab, and a live window's close plus its submission-latency budget on
/// devnet. Both are computed inside the producer. So a caller states how long
/// this rung lives AFTER the leg before it, and `authored_relative_ladder_v1`
/// folds the offsets into the absolute deadlines the policy carries. Strictly
/// positive, which is what makes the ladder's deadlines strictly increasing by
/// construction rather than by a check the caller has to pass.
#[derive(Clone, Debug)]
pub(crate) struct RelativeRecoveryRungV1 {
    /// This rung's confidence bound, in basis points.
    pub(crate) max_confidence_bps: u16,
    /// Seconds after the previous leg's deadline that this rung's falls.
    pub(crate) deadline_after_previous_seconds: i64,
}

/// The dimensions of the local demo market a CALLER may choose, with the
/// values this fixture has always emitted as its defaults.
///
/// Everything else about the local market -- the feed, the release row, the
/// captured publication, the semantic identities, the fixture liquidity -- is
/// a fact about the lab and not a parameter, and stays a constant below.
///
/// WHY THIS EXISTS. Until now `demo_market_input_base` hard-coded the band and
/// the collateral, so every market this repository could found on a local
/// validator was the SAME market: four outcomes, one claim unit, 1,000,000,000
/// atoms, a 300-second terminal window. A load simulator that draws twelve
/// markets of five widths then founds twelve identical ones, and the
/// heterogeneity it drew is a plan nothing on a chain ever expresses
/// (`docs/evidence/SIMULATOR_MARKET_LIFE_2026_08_30.md`, "THE SHAPE THE
/// COMPILER CANNOT VARY"). These are the knobs already in `MarketRunInput`;
/// exposing them is a lab fixture growing a parameter list, not new protocol.
///
/// NOT HERE, and deliberately: the claim unit. It is not a `MarketRunInput`
/// field at all -- `compile_linked_basis_v3` hard-wires `payout_scale: 1`
/// alongside the categorical basis kind, so varying it is the same edit as
/// emitting a graded basis and belongs to whoever does that one.
#[derive(Clone, Debug)]
pub(crate) struct LocalMarketShapeV1 {
    /// Denominator under every cut. The band's scale.
    pub(crate) cut_denominator: u64,
    /// Interior cuts, strictly increasing. The market's WIDTH is
    /// `cuts.len() + 2`: the two tails plus the explicit failure outcome.
    pub(crate) cuts: Vec<i128>,
    /// One payout coefficient per outcome, so exactly `cuts.len() + 2` of them.
    pub(crate) coefficients: Vec<u64>,
    /// Raw collateral atoms the founding commits.
    pub(crate) initial_collateral_atoms: u64,
    /// How long the terminal window is, in seconds. The window still ENDS at
    /// the captured fixture publication -- that instant is the one the local
    /// Pyth release can be resolved against and is not a free parameter -- so
    /// this moves the window's start, which is the deadline a market is
    /// measured against.
    pub(crate) terminal_window_width_seconds: i64,
    /// Product generation. Two markets drawn from one seed with the same band
    /// would otherwise collide on every derived identity.
    pub(crate) generation: u64,
    /// The funded ordered ladder this market buys, or `None`.
    ///
    /// `None` is the default and is what every fixture and campaign that takes
    /// this shape has always founded, so the no-recovery market compiles to the
    /// bytes it always did. `Some` is a loopback market that buys named
    /// alternative sources, which is the only shape `advance-recovery` has
    /// anything to crank.
    pub(crate) recovery: Option<Vec<RelativeRecoveryRungV1>>,
    /// The author's founding band, or `None`.
    ///
    /// `Option` rather than a plain field so `Default` stays constructible
    /// WITHOUT inventing a belief. `None` is not "use a sensible band"; it is
    /// an absent declaration, and every path that compiles a partition refuses
    /// by name when it finds one. That is the ruling: volatility is authored,
    /// so a caller that has not authored it has not finished describing its
    /// market.
    pub(crate) founding_band: Option<crate::model::FoundingBandInputV1>,
}

/// Fold a caller's relative rung offsets into the absolute deadlines a
/// `RecoveryPolicyV2` carries, starting from the primary leg's own deadline.
fn authored_relative_ladder_v1(
    rungs: Option<&[RelativeRecoveryRungV1]>,
    primary_deadline_unix_seconds: i64,
) -> Result<Option<Vec<PythRecoveryRungV1>>> {
    let Some(rungs) = rungs else {
        return Ok(None);
    };
    let mut previous = primary_deadline_unix_seconds;
    let mut authored = Vec::with_capacity(rungs.len());
    for (index, rung) in rungs.iter().enumerate() {
        if rung.deadline_after_previous_seconds <= 0 {
            return Err(Error::new(format!(
                "rung {index} lives {} seconds past the leg before it: a rung whose deadline is \
                 not strictly later than its predecessor's is a leg that expires before it opens",
                rung.deadline_after_previous_seconds
            )));
        }
        let deadline = previous
            .checked_add(rung.deadline_after_previous_seconds)
            .ok_or_else(|| Error::new(format!("rung {index} deadline overflowed")))?;
        previous = deadline;
        authored.push(PythRecoveryRungV1 {
            max_confidence_bps: rung.max_confidence_bps,
            deadline_unix_seconds: deadline,
        });
    }
    Ok(Some(authored))
}

impl Default for LocalMarketShapeV1 {
    /// A market that asks a question, and a band that says which question.
    ///
    /// This used to be EXACTLY what `demo_market_input_base` emitted before it
    /// took a shape -- cuts `[12_000, 18_000]` over denominator 100, which is
    /// $120 and $180 against a SOL/USD spot near $150. Read aloud that is
    /// "will SOL be under $120, between, or over $180 within the hour", and at
    /// any plausible hourly volatility the middle cell takes essentially all
    /// of the ex-ante mass. It was a market nobody could lose, and every
    /// fixture and campaign that took the default founded one.
    ///
    /// Ember's steer is that one-bucket dominance is a bug. So the default is
    /// now a centred partition around the same spot, with the band it is
    /// measured against stated rather than assumed. Measured shares:
    /// `[3024, 3950, 3024]` bps -- roughly 30/40/30, dominant cell 1 at 3,950
    /// against a 9,000 ceiling. The width is unchanged at `cuts + 2 = 4`
    /// outcomes, so every coefficient vector that fitted the old default still
    /// fits this one.
    fn default() -> Self {
        Self {
            cut_denominator: 100,
            cuts: vec![14_800, 15_200],
            coefficients: vec![1, 0, 1, 0],
            initial_collateral_atoms: 1_000_000_000,
            terminal_window_width_seconds: TERMINAL_WINDOW_WIDTH_SECONDS,
            generation: 1,
            // The lab's stated belief about its own fixture: 200 bp of a
            // $150 spot over a ten-thousand-slot window, three characteristic
            // displacements each way, no cell above 90%. This is a DECLARATION
            // and not a derivation -- it is what the fixture's author believes,
            // written down where the compiler can hold them to it, which is
            // exactly what makes the refusal legible when cuts stop describing
            // it. A caller that means something else states something else.
            founding_band: Some(crate::model::FoundingBandInputV1::spot_band(
                15_000, 200, 10_000, 3, 9_000,
            )),
            // NO LADDER BY DEFAULT, which is what every fixture and campaign
            // that takes this shape has founded since there was a shape to
            // take. A ladder is prepaid at founding -- one extra compartment
            // per rung, plus a named alternative feed -- so defaulting a market
            // into buying one would be spending on the caller's behalf.
            recovery: None,
        }
    }
}

impl LocalMarketShapeV1 {
    /// The market's outcome width: one region per cut boundary plus the two
    /// open tails, and the explicit failure outcome.
    ///
    /// NO CUTS is legal and is the narrowest market this compiler can emit:
    /// the whole coordinate domain as one region plus the failure outcome, two
    /// cells. It is the only way to reach a two-cell market here, and it was
    /// checked by compiling and founding one rather than reasoned about.
    pub(crate) fn outcome_count(&self) -> usize {
        self.cuts.len() + 2
    }

    /// Refuse a shape at the point it is CHOSEN rather than at a validator.
    ///
    /// `validate_market_input` re-checks the coefficient width and the
    /// denominators over the compiled input, and would catch most of this --
    /// but it speaks about a 40-KB document, and an operator who typed three
    /// cuts and four coefficients deserves to be told that, in those words,
    /// before a single record is compiled.
    pub(crate) fn validate(&self) -> Result<()> {
        if self.cut_denominator == 0 {
            return Err(Error::new("--cut-denominator must be positive"));
        }
        if self.initial_collateral_atoms == 0 {
            return Err(Error::new("--initial-collateral-atoms must be positive"));
        }
        if self.terminal_window_width_seconds <= 0 {
            return Err(Error::new(
                "--terminal-window-width-seconds must be positive: a window forced to one instant \
                 is a market nobody can resolve",
            ));
        }
        if self.coefficients.len() != self.outcome_count() {
            return Err(Error::new(format!(
                "{} cuts describe a {}-outcome market (two tails plus the explicit failure \
                 outcome), so it needs {} coefficients and {} were given",
                self.cuts.len(),
                self.outcome_count(),
                self.outcome_count(),
                self.coefficients.len()
            )));
        }
        if self.cuts.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Error::new(
                "cuts must be STRICTLY increasing: equal or descending cuts describe a region of \
                 zero or negative width, which is an outcome no coordinate can land in",
            ));
        }
        Ok(())
    }
}

pub(crate) fn demo_market_input(
    registry: Pubkey,
    direct: DirectMarketCompilerInputV1<'_>,
) -> Result<MarketRunInput> {
    demo_market_input_shaped(registry, direct, &LocalMarketShapeV1::default())
}

/// `demo_market_input` at a caller-chosen shape.
pub(crate) fn demo_market_input_shaped(
    registry: Pubkey,
    direct: DirectMarketCompilerInputV1<'_>,
    shape: &LocalMarketShapeV1,
) -> Result<MarketRunInput> {
    let mut input = demo_market_input_base_shaped(registry, direct.resolution_release, shape)?;
    attach_direct_market_capability_v1(&mut input, direct)?;
    validate_market_input(&input)?;
    Ok(input)
}

/// The demo market's capability-free graph. See `demo_market_input` for the
/// market itself; a family-neutral caller attaches its own selected closure
/// to this base through the selection seam.
pub(crate) fn demo_market_input_base(
    registry: Pubkey,
    resolution_release: [u8; 32],
) -> Result<MarketRunInput> {
    demo_market_input_base_shaped(registry, resolution_release, &LocalMarketShapeV1::default())
}

/// `demo_market_input_base` at a caller-chosen shape. See
/// [`LocalMarketShapeV1`] for which dimensions are a caller's and which are the
/// lab's.
pub(crate) fn demo_market_input_base_shaped(
    registry: Pubkey,
    resolution_release: [u8; 32],
    shape: &LocalMarketShapeV1,
) -> Result<MarketRunInput> {
    use dclutch_source::pyth::{FullPriceUpdateV2, synthetic_fixture::local_validator_release_v1};

    shape.validate()?;

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
    pyth_market_input_base(
        PythMarketParamsV1 {
            founding_band: shape.founding_band.clone(),
            recovery: authored_relative_ladder_v1(
                shape.recovery.as_deref(),
                update
                    .publish_time()
                    .checked_add(i64::from(FIXTURE_SHELF_LIFE_SECONDS))
                    .ok_or_else(|| Error::new("fixture primary deadline overflowed"))?,
            )?,
            registry,
            release: PythMarketProviderV1::Pull(fixture.release()),
            label: fixture.local_label(),
            product_name: "product/sol-usd-range-protection",
            coordinate_domain_name: "coordinate-domain/usd-cents-per-sol",
            feed_label: b"sol-usd",
            price_update: FIXTURE_PRICE_UPDATE,
            window_start: update
                .publish_time()
                .checked_sub(shape.terminal_window_width_seconds)
                .ok_or_else(|| Error::new("terminal window start underflowed"))?,
            window_end: update.publish_time(),
            max_age_seconds: FIXTURE_SHELF_LIFE_SECONDS,
            // The adapter's ceiling — the widest the type admits. A LAB setting:
            // this Market resolves against a single captured publication whose
            // confidence is whatever it was on the day it was captured, and
            // refusing it on confidence would be refusing the fixture rather than
            // testing the adapter. The devnet flagship states a real bound.
            max_confidence_bps: 10_000,
            cut_denominator: shape.cut_denominator,
            cuts: shape.cuts.clone(),
            coefficients: shape.coefficients.clone(),
            generation: shape.generation,
            initial_collateral_atoms: shape.initial_collateral_atoms,
            local_participant_fixture_liquidity_atoms: LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1,
        },
        resolution_release,
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
    /// The author's founding band. `None` refuses at compile by name: a
    /// devnet market whose author has not stated how uncertain they think
    /// the outcome is has not finished describing itself.
    pub(crate) founding_band: Option<crate::model::FoundingBandInputV1>,
    /// The funded ordered ladder this market buys, in rung order.
    ///
    /// `None` is the no-recovery shape every devnet market founded before this
    /// field existed, and it stays the default: a ladder is PREPAID at founding
    /// -- one extra Resolution compartment per rung -- so defaulting a market
    /// into buying one would be spending on the founder's behalf. `Some` costs
    /// real lamports and buys a real second answerer.
    pub(crate) recovery: Option<Vec<RelativeRecoveryRungV1>>,
}

/// Four measured 313-second cadences, the §12.3 guidance floor.
pub(crate) const DEVNET_MINIMUM_WINDOW_WIDTH_SECONDS: u32 = 1_252;

pub(crate) fn devnet_market_input(
    spec: DevnetPythMarketSpecV1<'_>,
    direct: DirectMarketCompilerInputV1<'_>,
) -> Result<MarketRunInput> {
    let window_end = devnet_window_end_v1(&spec)?;
    let release = dclutch_source::pyth::devnet_release_v1()
        .map_err(|error| Error::new(format!("devnet Pyth release row: {error:?}")))?;
    pyth_market_input(
        PythMarketParamsV1 {
            founding_band: spec.founding_band.clone(),
            // The ladder a devnet author bought, folded from offsets against
            // this market's own primary deadline -- the live window's close
            // plus its submission-latency budget, which is the second after
            // which a crank onto rung zero becomes admissible.
            recovery: authored_relative_ladder_v1(
                spec.recovery.as_deref(),
                window_end
                    .checked_add(i64::from(spec.max_age_seconds))
                    .ok_or_else(|| Error::new("devnet primary deadline overflowed"))?,
            )?,
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
            // The devnet flagships state the collateral the lab always used;
            // widening it is a local-fixture affordance and not a devnet one.
            initial_collateral_atoms: 1_000_000_000,
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
    release: PythSponsoredPushReleaseV1,
) -> Result<MarketRunInput> {
    let mut input = devnet_sponsored_market_input_base(spec, direct.resolution_release, release)?;
    attach_direct_market_capability_v1(&mut input, direct)?;
    validate_market_input(&input)?;
    Ok(input)
}

/// The devnet sponsored market's capability-free graph.
///
/// The same split `demo_market_input_base_shaped` makes for the lab, made for
/// the flagship: everything a devnet sponsored market is, up to and including
/// its Resolution manifest base, with no trade capability attached. Direct's
/// caller above attaches Direct; a family-neutral caller attaches its own
/// closure through the selection seam instead. Without this the only way to
/// reach the devnet Pyth graph was through a Direct compiler, which is why
/// the second family could be founded on a local validator and nowhere else.
///
/// **The release is a PARAMETER and not the constant**, because Pyth redeployed
/// their devnet Receiver and changed their Receiver `Config` after the constant
/// was typed, and a market pins its provider release at founding. Its author is
/// `sponsored_release_observation`, which reads the chain-owned half off a
/// finalized snapshot; the constant remains the declaration every observed
/// field is compared against.
pub(crate) fn devnet_sponsored_market_input_base(
    spec: DevnetPythMarketSpecV1<'_>,
    resolution_release: [u8; 32],
    release: PythSponsoredPushReleaseV1,
) -> Result<MarketRunInput> {
    let window_end = devnet_window_end_v1(&spec)?;
    pyth_market_input_base(
        PythMarketParamsV1 {
            founding_band: spec.founding_band.clone(),
            // The ladder a devnet author bought, folded from offsets against
            // this market's own primary deadline -- the live window's close
            // plus its submission-latency budget, which is the second after
            // which a crank onto rung zero becomes admissible.
            recovery: authored_relative_ladder_v1(
                spec.recovery.as_deref(),
                window_end
                    .checked_add(i64::from(spec.max_age_seconds))
                    .ok_or_else(|| Error::new("devnet primary deadline overflowed"))?,
            )?,
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
            // The devnet flagships state the collateral the lab always used;
            // widening it is a local-fixture affordance and not a devnet one.
            initial_collateral_atoms: 1_000_000_000,
            local_participant_fixture_liquidity_atoms: 0,
        },
        resolution_release,
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
    let mut input = pyth_market_input_base(params, direct.resolution_release)?;
    attach_direct_market_capability_v1(&mut input, direct)?;
    validate_market_input(&input)?;
    Ok(input)
}

/// The capability-free Pyth market graph: everything a market is, up to and
/// including its three-entry Resolution manifest base, with no trade
/// capability attached. `pyth_market_input` attaches Direct; a family-neutral
/// caller attaches its own closure through the selection seam instead.
fn pyth_market_input_base(
    params: PythMarketParamsV1<'_>,
    resolution_release: [u8; 32],
) -> Result<MarketRunInput> {
    use dclutch_market::capability_manifest::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_source::pyth::FullPriceUpdateV2;
    use dclutch_source::{
        CapacityEnvelope, ProviderReleaseV1, PythAdapterConfigV1, RoundingBoundary,
        SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SourceCapacityProfileV1, SourceSpecV1, StatisticKind,
        StatisticSpecV1, WindowKind, WindowSpecV1,
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
    //     (dclutch-source lib.rs) refuses anything that is not
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

    // The factor, written from the adapter configuration this same function
    // built from the observed publication. This market's source unit is
    // `source-unit/pyth-scaled-price` and its result unit is
    // `result-unit/usd-cents`, so it declares a conversion, and the only
    // conversion a Pyth-backed statistic may declare is the feed's own decimal
    // exponent. Until this line the record declared the conversion and left
    // the number out; cohort-14 market B was founded here, and its selector
    // compared raw feed atoms against cuts in dollars.
    let statistic = StatisticSpecV1::new(
        source_content(source_unit)?,
        source_content(result_unit)?,
        adapter_config.expected_exponent(),
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
    // The authored band, or a refusal BY NAME. A Pyth market whose author has
    // not said how uncertain they think the outcome is is not yet described.
    let declared = params.founding_band.as_ref().ok_or_else(|| {
        Error::new(
            "founding_band is required to compile a Pyth market: state anchor, \
             volatility_bps, window_slots, plausible_half_widths and \
             max_cell_share_bps. There is no default -- volatility is an \
             authoring input, and a partition cannot be measured for \
             degeneracy without the belief it is supposed to describe",
        )
    })?;
    let (belief, ceiling) = founding_belief_for(declared, params.cut_denominator, "pyth market")?;
    compile_interesting_product_records_v2(
        params.registry,
        &belief,
        ceiling,
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

    // NO ORDERED RECOVERY WALK, and it is now a CHOICE rather than a wall.
    //
    // It used to be a wall: a material that bought a ladder had no terminal at
    // all, Core welded `CreateFund` shut against it, and the off-chain builder
    // mirrored the weld, so a recovery market's founding refused OFFLINE before
    // any transaction. Decision 0027 built the ladder --
    // `RelayActionV1::AdvanceRecovery` advances the funded attempt and exhausts
    // into the failure commit, walked end to end on real ELFs in
    // `resolution_core_v3_lifecycle.rs` -- and the weld is deleted.
    //
    // This market stays the section-12.7/12.8 no-recovery shape because that is
    // what this driver is for: the funded `Primary -> Exhausted ->
    // FailureCommitted` walk to the Product's own pre-disclosed outcome, on a
    // market that bought no alternatives. Its record set is two accounts
    // narrower (no recovery-policy pair) and its funding entries are selected
    // structurally rather than by allocation identity --
    // `authenticate_no_recovery_entries` in Core and
    // `select_resolution_funding_entries` in the operator. The relayed family
    // already founds and funds this shape in execution (2026-08-27: CreateFund
    // 1,200,587 CU, VerifyFundReady 1,185,248 CU).
    //
    // AND A CALLER MAY NOW BUY ONE. `params.recovery` authors an alternative
    // source per rung and the `RecoveryPolicyV2` that funds them; the manifest
    // below then carries one compartment per rung configured by that rung's own
    // allocation, plus the exhaustion entry configured by the policy digest,
    // which is what turns `authenticate_no_recovery_entries` into the `Some`
    // arm both Core and the Resolution controller take. What crank the ladder
    // is `advance-recovery`'s, and it is a frame builder and a bounded wait
    // rather than a decision because `AdvanceRecovery` reads which rung, which
    // source and when it expires off the market's own state.
    let primary_deadline = params
        .window_end
        .checked_add(i64::from(params.max_age_seconds))
        .ok_or_else(|| Error::new("primary window end + max_age overflows"))?;
    let ladder = match &params.recovery {
        None => None,
        Some(rungs) => Some(author_pyth_recovery_ladder_v1(
            rungs,
            &local_label,
            update.feed_id(),
            update.exponent(),
            source_spec,
            primary_deadline,
        )?),
    };
    let recovery_link = match &ladder {
        None => None,
        Some(authored) => Some(
            SourceContentId::new(record_identity(&decode_hex(&authored.policy_hex)?))
                .map_err(|error| Error::new(format!("recovery policy identity: {error:?}")))?,
        ),
    };
    let material = SourceMaterialV3::explicitly_unbounded(
        SourceContentId::new(product_digest)
            .map_err(|error| Error::new(format!("demo Product digest: {error:?}")))?,
        SourceContentId::new(primary_source)
            .map_err(|error| Error::new(format!("demo primary source: {error:?}")))?,
        SourceContentId::new(window)
            .map_err(|error| Error::new(format!("demo window: {error:?}")))?,
        SourceContentId::new(statistic)
            .map_err(|error| Error::new(format!("demo statistic: {error:?}")))?,
        recovery_link,
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
    let release = CapabilityContentId::new(resolution_release)
        .map_err(|error| Error::new(format!("demo Resolution release: {error:?}")))?;
    // The three Resolution-controller compartments of a no-recovery material.
    // With no policy record there is no allocation identity and no policy
    // digest to pin the first two to, so the selection both Core and the
    // operator perform is STRUCTURAL: exactly one entry configured by this
    // market's own Source material -- the failure compartment the funded
    // deadline walk admits -- and exactly two other Resolution-controller
    // entries, pairwise distinct and neither equal to the material. The two
    // companions stay prepaid until `CloseFund` refunds them.
    let mut entries_input: Vec<([u8; 32], [u8; 32])> = match &ladder {
        // The two structural companions of a market that bought no ladder.
        None => vec![
            (
                demo_id("capability/recovery-companion", &[&local_label]),
                demo_id("companion-config/recovery", &[&local_label]),
            ),
            (
                demo_id("capability/exhaustion-companion", &[&local_label]),
                demo_id("companion-config/exhaustion", &[&local_label]),
            ),
        ],
        // One compartment per rung plus the exhaustion entry, each pinned to
        // the identity the crank that spends it will name.
        Some(authored) => authored.entries.clone(),
    };
    entries_input.push((
        demo_id("capability/source-material", &[&local_label]),
        material_digest,
    ));
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

    let input = MarketRunInput {
        // The band this market was actually measured against, carried into
        // the run spec so the compiled market records the belief it was
        // judged by rather than leaving it in the producer's arguments.
        founding_band: Some(declared.clone()),
        generation: params.generation,
        collateral_display_decimals: 6,
        local_participant_fixture_liquidity_atoms: params.local_participant_fixture_liquidity_atoms,
        initial_collateral_atoms: params.initial_collateral_atoms,
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
        // Empty IS the statement that this material bought no ordered recovery
        // walk: the compiler derives the same `None` link the material above
        // carries, and no recovery-policy record is published or observed. A
        // market that bought a ladder carries the policy and one alternative
        // record pair per rung, and `validate_market_input` refuses any other
        // count.
        recovery_policy_hex: ladder
            .as_ref()
            .map_or_else(String::new, |authored| authored.policy_hex.clone()),
        recovery_source_records: ladder
            .as_ref()
            .map_or_else(Vec::new, |authored| authored.records.clone()),
        capability_manifest_hex: hex(&manifest),
        direct_capability: None,
        selected_capability: None,
        linked_basis_hex: hex(&linked_basis),
        price_gate_hex: String::new(),
        // The infrastructure floor founds atomically; a split tier-1 campaign
        // selects the other route through `SuccessorRunSpec::founding_route`.
        founding_route: FoundingRouteV1::Atomic,
    };
    Ok(input)
}

#[cfg(test)]
mod tests {

    /// Every founding journal has an owner that will put it in the projection.
    ///
    /// The consumer's rule is all-or-nothing:
    /// `authenticate_recovery_to_complete_v1` corroborates each of the six
    /// journal signatures against `execution.transactions` and refuses the whole
    /// report when one is missing. Cohort-13 was refused by that rule with a
    /// sound founding underneath it -- "recovery-to-complete named a DCLTCFQ1
    /// signature its own transaction projection does not carry" -- because the
    /// stages before Open had no projecting owner at all, and a producer gap
    /// reads exactly like a defect in the founding.
    ///
    /// So the partition is asserted rather than remembered. Three owners cover
    /// `ORDER` between them and their union must be the whole of it:
    /// this array for everything before Open,
    /// `finalize_existing_founding_submission_v1` for Open itself, and
    /// `execute_funding_readiness_suffix_v1` for the post-Open suffix. Add a
    /// seventh stage anywhere and exactly one arm of this goes red, naming the
    /// half whose owner is missing -- which is the failure this test exists to
    /// stop being discovered on a deadline against a live market.
    #[test]
    fn every_founding_journal_before_open_has_a_projecting_owner() {
        use super::{
            RECONSTRUCTION_PROJECTED_HISTORY_V1,
            founding_submission_journal::FoundingSubmissionOperationV1 as Operation,
        };

        let projected = RECONSTRUCTION_PROJECTED_HISTORY_V1
            .iter()
            .map(|(operation, _)| *operation)
            .collect::<Vec<_>>();
        let before_open = Operation::ORDER
            .iter()
            .copied()
            .take_while(|operation| *operation != Operation::Dcltgmf3)
            .collect::<Vec<_>>();
        assert!(
            !before_open.is_empty(),
            "the canonical order must have stages before Open, or this test proves nothing"
        );
        assert_eq!(
            projected, before_open,
            "the reconstruction projects {projected:?} from history but the founding order runs {before_open:?} before DCLTGMF3; every one of them is corroborated by the consumer"
        );

        // The other half of the partition, so the union is checked and not just
        // this array: Open, then the suffix, then nothing left over.
        let after_open = Operation::ORDER
            .iter()
            .copied()
            .skip_while(|operation| *operation != Operation::Dcltgmf3)
            .collect::<Vec<_>>();
        assert_eq!(
            after_open,
            vec![
                Operation::Dcltgmf3,
                Operation::CoreFundingCreateV1,
                Operation::ResolutionFundingActivateV1,
                Operation::CoreFundingAcceptV1,
            ],
            "DCLTGMF3 is finalized by finalize_existing_founding_submission_v1 and the three funding stages by execute_funding_readiness_suffix_v1; a change here needs a projecting owner before the consumer will accept a recovered report"
        );
        assert_eq!(projected.len() + after_open.len(), Operation::ORDER.len());

        // A label is what a reader sees beside a signature, so it may not be
        // blank and the two may not be the same string.
        for (operation, label) in RECONSTRUCTION_PROJECTED_HISTORY_V1 {
            assert!(
                label.contains(operation.label()),
                "{label:?} does not name {}",
                operation.label()
            );
        }
    }

    /// The historical fixture partition is DEGENERATE, and now something says so.
    ///
    /// `LocalMarketShapeV1::default()` carries cuts `[12_000, 18_000]` over
    /// denominator 100 -- that is $120 and $180 against a SOL/USD spot near
    /// $150. Read as a question it is "will SOL be under $120, between, or over
    /// $180 in an hour", and at any plausible hourly volatility the middle cell
    /// takes essentially all of the ex-ante mass. It is a market nobody can
    /// lose, and it was foundable for as long as this path called the ungated
    /// compiler.
    ///
    /// The control is the SAME band with a centred partition: if that were
    /// refused too, this test would only prove the gate refuses everything.
    #[test]
    fn the_default_shape_is_refused_and_a_centred_partition_is_not() {
        use dclutch_product_runtime_v2_operator::{
            FoundingBandV1, FoundingBeliefV1, require_interesting_partition_v1,
        };

        // 200 bp of $150 over a ten-thousand-slot window, three displacements
        // each way. A stated belief, not a derived one.
        let band = FoundingBeliefV1::SpotBand {
            band: FoundingBandV1 {
                anchor: 15_000,
                denominator: 100,
                volatility_bps: 200,
                window_slots: 10_000,
            },
            plausible_half_widths: 3,
        };

        // The HISTORICAL partition, kept as a literal so this stays a test
        // about that market rather than about whatever the default is today.
        let historical = vec![12_000, 18_000];
        let degenerate = require_interesting_partition_v1(&historical, &band, 9_000);
        assert!(
            degenerate.is_err(),
            "the historical fixture partition must be refused: {degenerate:?}"
        );

        // And the default must now be a market that is actually foundable.
        let shape = LocalMarketShapeV1::default();
        assert_ne!(
            shape.cuts, historical,
            "the degenerate default must be gone"
        );
        let declared = shape.founding_band.expect("the default states its band");
        let (declared_belief, declared_ceiling) =
            founding_belief_for(&declared, shape.cut_denominator, "default shape")
                .expect("the default shape's belief must parse");
        require_interesting_partition_v1(&shape.cuts, &declared_belief, declared_ceiling)
            .expect("the default shape must compile against its own stated band");

        // Cuts a displacement or so either side of spot: a real question.
        let centred = vec![14_800, 15_200];
        let report = require_interesting_partition_v1(&centred, &band, 9_000)
            .expect("a centred partition around spot must be admitted");
        assert!(
            report.dominant_share_bps <= 9_000,
            "no cell may take more than the ceiling; shares were {:?}",
            report.cell_share_bps
        );
        println!(
            "centred shares bps = {:?}, dominant cell {} at {} bps",
            report.cell_share_bps, report.dominant_cell, report.dominant_share_bps
        );
    }

    use super::*;

    fn cubic_price_gate_v1()
    -> [u8; dclutch_product::payoff::price_gate_v1::PRICE_GATE_REQUEST_BYTES_V1] {
        use dclutch_product::payoff::price_gate_v1::*;

        let mut gate = [0_u8; PRICE_GATE_REQUEST_BYTES_V1];
        gate[PRICE_GATE_MAGIC_OFFSET_V1..PRICE_GATE_MAGIC_OFFSET_V1 + 8]
            .copy_from_slice(&PRICE_GATE_MAGIC_V1);
        gate[PRICE_GATE_VERSION_OFFSET_V1..PRICE_GATE_VERSION_OFFSET_V1 + 2]
            .copy_from_slice(&PRICE_GATE_SCHEMA_VERSION_V1.to_le_bytes());
        gate[PRICE_GATE_PROFILE_OFFSET_V1..PRICE_GATE_PROFILE_OFFSET_V1 + 2]
            .copy_from_slice(&PRICE_GATE_PROFILE_V1.to_le_bytes());
        gate[PRICE_GATE_SCALE_OFFSET_V1..PRICE_GATE_SCALE_OFFSET_V1 + 4]
            .copy_from_slice(&11_u32.to_le_bytes());
        gate[PRICE_GATE_MASS_OFFSET_V1..PRICE_GATE_MASS_OFFSET_V1 + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        gate[PRICE_GATE_DEGREE_OFFSET_V1] = 3;
        gate[PRICE_GATE_WIDTH_OFFSET_V1] = 4;
        gate[PRICE_GATE_ATOM_COUNT_OFFSET_V1] = 1;
        for (claim, payout) in [1_u64, 4, 4, 2].iter().enumerate() {
            let offset = PRICE_GATE_PRICES_OFFSET_V1 + claim * 8;
            gate[offset..offset + 8].copy_from_slice(&payout.to_le_bytes());
        }
        gate[PRICE_GATE_WEIGHTS_OFFSET_V1..PRICE_GATE_WEIGHTS_OFFSET_V1 + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        gate[PRICE_GATE_NUMERATORS_OFFSET_V1..PRICE_GATE_NUMERATORS_OFFSET_V1 + 8]
            .copy_from_slice(&3_i64.to_le_bytes());
        gate[PRICE_GATE_DENOMINATORS_OFFSET_V1..PRICE_GATE_DENOMINATORS_OFFSET_V1 + 4]
            .copy_from_slice(&2_u32.to_le_bytes());
        gate
    }

    #[test]
    fn categorical_market_construction_remains_gate_absent_and_byte_identical() {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("deployment widths"),
        );
        let input = demo_market_input(registry, direct.compiler()).expect("categorical input");
        let compiled = compile_market_bodies(registry, &input, Pubkey::new_unique())
            .expect("categorical market bodies");
        assert_eq!(
            compiled.basis,
            decode_hex(&input.linked_basis_hex).expect("basis")
        );
        assert_eq!(compiled.price_gate, None);
        assert_eq!(compiled.basis_scale, 1);
    }

    /// One local market compiler shared by the ladder tests below.
    fn ladder_fixture_v1(
        rungs: Option<Vec<RelativeRecoveryRungV1>>,
    ) -> (
        Pubkey,
        crate::direct_market::DirectMarketCompilerOwnedV1,
        MarketRunInput,
    ) {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("deployment widths"),
        );
        let shape = LocalMarketShapeV1 {
            recovery: rungs,
            ..LocalMarketShapeV1::default()
        };
        let input = demo_market_input_shaped(registry, direct.compiler(), &shape)
            .expect("local market at the stated shape");
        (registry, direct, input)
    }

    /// THE CONTROL. A market that buys no ladder is the market it always was.
    ///
    /// Two fields grew here and a third check widened, and every one of them
    /// had to leave the no-recovery founding alone: `recovery_source_records`
    /// is `skip_serializing_if`-empty so the serialized run spec carries no new
    /// key at all, the manifest count `1 + rungs.max(1) + 2` is still four when
    /// nothing was bought, and the two structural companions are still the
    /// entries `authenticate_no_recovery_entries` selects. If any of that had
    /// moved, every existing fixture, campaign and spec digest in the tree
    /// would have moved with it.
    #[test]
    fn a_market_that_buys_no_ladder_is_the_market_it_always_was() {
        let (_, _, input) = ladder_fixture_v1(None);
        assert!(
            input.recovery_policy_hex.is_empty(),
            "empty IS the statement that no ordered walk was bought"
        );
        assert!(input.recovery_source_records.is_empty());
        let wire = serde_json::to_string(&input).expect("run spec serializes");
        assert!(
            !wire.contains("recovery_source_records"),
            "a no-recovery run spec must serialize to the bytes it did before the field existed"
        );
        let manifest_bytes = decode_hex(&input.capability_manifest_hex).expect("manifest hex");
        let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest");
        assert_eq!(manifest.entry_count(), 4);
        validate_market_input(&input).expect("the no-recovery market still validates");
        let compiled = compile_market_bodies(
            Pubkey::new_from_array([0x41; 32]),
            &input,
            Pubkey::new_unique(),
        )
        .expect("no-recovery bodies");
        assert!(compiled.recovery.is_empty());
        assert_eq!(
            SourceMaterialV3::decode(&compiled.source)
                .expect("material")
                .recovery_policy(),
            None
        );
    }

    /// A two-source founding names a REAL alternative and funds it.
    ///
    /// The rung's spec is a record this founding publishes, its identity is the
    /// one the attempt names, and the only thing it moves is the adapter
    /// configuration -- which is the only axis a Pyth adapter has. Everything
    /// the market sold stays the market's: unit, coordinate domain, capacity
    /// profile, access profile, provider release. That is
    /// `validate_recovery_source_graph`'s rule, satisfied by construction here
    /// rather than discovered at a capture.
    #[test]
    fn a_founding_that_buys_a_rung_publishes_the_source_that_rung_names() {
        let (registry, _, input) = ladder_fixture_v1(Some(vec![RelativeRecoveryRungV1 {
            max_confidence_bps: 9_000,
            deadline_after_previous_seconds: 20,
        }]));
        let policy_bytes = decode_hex(&input.recovery_policy_hex).expect("policy hex");
        let policy = RecoveryPolicyV2::decode(&policy_bytes).expect("policy");
        assert_eq!(policy.attempt_count(), 1);
        let attempt = policy.attempt(0).expect("the funded rung");

        assert_eq!(input.recovery_source_records.len(), 1);
        let alternative_bytes =
            decode_hex(&input.recovery_source_records[0].source_spec_hex).expect("rung spec hex");
        assert_eq!(
            record_identity(&alternative_bytes),
            attempt.source_spec_id().to_bytes(),
            "the attempt must name a record this founding actually publishes"
        );
        let primary_bytes = decode_hex(&input.source_spec_hex).expect("primary spec hex");
        assert_ne!(alternative_bytes, primary_bytes);
        let alternative = SourceSpecV1::decode(&alternative_bytes).expect("rung spec");
        let primary = SourceSpecV1::decode(&primary_bytes).expect("primary spec");
        assert_ne!(
            alternative.adapter_config_id(),
            primary.adapter_config_id(),
            "the rung is a different SOURCE, and the adapter configuration is what makes it one"
        );
        assert_eq!(alternative.unit_id(), primary.unit_id());
        assert_eq!(alternative.domain_id(), primary.domain_id());
        assert_eq!(alternative.access_profile(), primary.access_profile());
        assert_eq!(
            alternative.provider_release_id(),
            primary.provider_release_id()
        );
        assert_eq!(
            alternative.capacity_profile_id(),
            primary.capacity_profile_id()
        );
        assert_eq!(
            policy.capacity_profile_id(),
            primary.capacity_profile_id(),
            "a ladder running under a profile the market did not publish is one nobody priced"
        );
        let adapter_bytes = decode_hex(&input.recovery_source_records[0].pyth_adapter_config_hex)
            .expect("rung adapter hex");
        assert_eq!(
            record_identity(&adapter_bytes),
            alternative.adapter_config_id().to_bytes()
        );

        // FOUNDING FUNDS EVERY RUNG: one compartment pinned to this rung's own
        // allocation, one to the policy digest, one to the material, and the
        // Direct entry the Resolution subset does not select.
        let manifest_bytes = decode_hex(&input.capability_manifest_hex).expect("manifest hex");
        let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest");
        assert_eq!(manifest.entry_count(), 4);
        let configs: Vec<[u8; 32]> = (0..manifest.entry_count())
            .map(|index| manifest.entry(index).expect("entry").config_id().to_bytes())
            .collect();
        assert!(configs.contains(&attempt.funding_allocation_id().to_bytes()));
        assert!(configs.contains(&record_identity(&policy_bytes)));

        validate_market_input(&input).expect("the two-source market validates");
        let compiled =
            compile_market_bodies(registry, &input, Pubkey::new_unique()).expect("ladder bodies");
        assert_eq!(compiled.recovery, policy_bytes);
        assert_eq!(
            SourceMaterialV3::decode(&compiled.source)
                .expect("material")
                .recovery_policy()
                .map(|id| id.to_bytes()),
            Some(record_identity(&policy_bytes)),
            "the material is the one bit that separates a market with a ladder from one without"
        );
    }

    /// Each way of authoring a ladder wrong refuses, and each refusal says which.
    #[test]
    fn a_ladder_that_would_not_found_refuses_offline_and_by_name() {
        let (_, _, no_ladder) = ladder_fixture_v1(None);
        let (_, _, ladder) = ladder_fixture_v1(Some(vec![RelativeRecoveryRungV1 {
            max_confidence_bps: 9_000,
            deadline_after_previous_seconds: 20,
        }]));

        // Records nothing names. Publishing a source spec no attempt selects is
        // a record no market can ever resolve against.
        let mut orphaned = no_ladder.clone();
        orphaned
            .recovery_source_records
            .clone_from(&ladder.recovery_source_records);
        let refusal = validate_market_input(&orphaned).expect_err("orphaned records must refuse");
        assert!(
            format!("{refusal}").contains("no recovery policy"),
            "got {refusal}"
        );

        // A rung whose spec nobody publishes: the market can be advanced onto
        // it and never answered on it.
        let mut unpublished = ladder.clone();
        unpublished.recovery_source_records.clear();
        let refusal =
            validate_market_input(&unpublished).expect_err("an unpublished rung must refuse");
        assert!(
            format!("{refusal}").contains("funds 1 attempts"),
            "got {refusal}"
        );

        // A rung that IS the primary. The policy names the primary's own spec
        // and the records carry it, so every digest joins -- and the founding
        // still refuses, because a ladder whose alternative is the feed that
        // already went silent buys nothing.
        let primary_bytes = decode_hex(&ladder.source_spec_hex).expect("primary hex");
        let primary = SourceSpecV1::decode(&primary_bytes).expect("primary spec");
        let mirrored = RecoveryPolicyV2::new(
            primary.capacity_profile_id(),
            [
                Some(
                    RecoveryAttemptV2::new(
                        SourceContentId::new(record_identity(&primary_bytes)).expect("primary id"),
                        primary.provider_release_id(),
                        i64::from(u32::MAX),
                        SourceContentId::new([0x5a; 32]).expect("allocation"),
                    )
                    .expect("mirrored attempt"),
                ),
                None,
                None,
                None,
            ],
            1,
        )
        .expect("mirrored policy");
        let mut mirror = ladder.clone();
        mirror.recovery_policy_hex = hex(&mirrored.to_bytes());
        mirror.recovery_source_records = vec![crate::model::RecoverySourceRecordsV1 {
            source_spec_hex: ladder.source_spec_hex.clone(),
            pyth_adapter_config_hex: ladder.pyth_adapter_config_hex.clone(),
        }];
        let refusal = validate_market_input(&mirror).expect_err("a mirrored rung must refuse");
        assert!(
            format!("{refusal}").contains("PRIMARY source"),
            "got {refusal}"
        );

        // A ladder whose compartments the manifest does not carry: the
        // no-recovery manifest under a recovery-bearing policy.
        let mut unfunded = ladder.clone();
        unfunded
            .capability_manifest_hex
            .clone_from(&no_ladder.capability_manifest_hex);
        let refusal = validate_market_input(&unfunded).expect_err("an unfunded rung must refuse");
        assert!(
            format!("{refusal}").contains("funding allocation"),
            "got {refusal}"
        );

        // A rung that expires before it opens. The crank that enters rung zero
        // is admissible only after the primary deadline, so a rung whose
        // deadline is not strictly later is a leg nobody can answer. TWO gates
        // say so and the local one is reached first: the shape's fold refuses a
        // non-positive lifetime before an absolute deadline exists at all.
        let refusal = ladder_fixture_zero_lifetime_v1();
        assert!(
            format!("{refusal}").contains("expires before it opens"),
            "got {refusal}"
        );

        // The second gate, reached directly, because a caller that hands
        // absolute deadlines -- every non-lab caller -- never passes through
        // the fold above.
        let primary_bytes = decode_hex(&ladder.source_spec_hex).expect("primary hex");
        let primary = SourceSpecV1::decode(&primary_bytes).expect("primary spec");
        let refusal = author_pyth_recovery_ladder_v1(
            &[PythRecoveryRungV1 {
                max_confidence_bps: 9_000,
                deadline_unix_seconds: 1_800_000_000,
            }],
            &[0x11; 32],
            [0x22; 32],
            -8,
            primary,
            1_800_000_000,
        )
        .expect_err("a rung due on the primary deadline must refuse");
        assert!(
            format!("{refusal}").contains("expire before it opened"),
            "got {refusal}"
        );
    }

    /// The zero-lifetime rung, authored through the local shape's own fold.
    fn ladder_fixture_zero_lifetime_v1() -> Error {
        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("deployment widths"),
        );
        let shape = LocalMarketShapeV1 {
            recovery: Some(vec![RelativeRecoveryRungV1 {
                max_confidence_bps: 9_000,
                deadline_after_previous_seconds: 0,
            }]),
            ..LocalMarketShapeV1::default()
        };
        demo_market_input_shaped(registry, direct.compiler(), &shape)
            .expect_err("a rung with no lifetime must refuse")
    }

    #[test]
    fn founding_quantity_preserves_scale_one_and_refuses_a_second_rounding_boundary() {
        assert_eq!(
            founding_quantity_v1(1_000_000_000, 1).expect("Q=1"),
            500_000_000
        );
        assert_eq!(founding_quantity_v1(198, 11).expect("Q=11"), 9);
        assert_eq!(
            founding_quantity_v1(200, 11)
                .expect_err("nonintegral cubic reserve")
                .to_string(),
            "founding collateral reserve is not exactly divisible by basis scale"
        );
        assert_eq!(
            founding_quantity_v1(u64::MAX, u64::MAX)
                .expect_err("scale larger than reserve")
                .to_string(),
            "basis scale exceeds the founding collateral reserve"
        );
        assert_eq!(
            founding_quantity_v1(198, 0)
                .expect_err("zero scale")
                .to_string(),
            "basis scale exceeds the founding collateral reserve"
        );
    }

    #[test]
    fn cubic_market_basis_owns_scale_gate_and_product_links() {
        use dclutch_product_runtime_v2_operator::spline_basis_v3::{
            SplineProductCompilationInputV3, compile_spline_product_records_v3,
            spline_basis_output_bytes_v3,
        };

        let registry = Pubkey::new_from_array([0x41; 32]);
        let direct = crate::direct_market::DirectMarketCompilerOwnedV1::for_test(
            registry,
            crate::direct_market::DirectDeploymentWidthsV1::new(1_141_117, 971_053, 934_037)
                .expect("deployment widths"),
        );
        let mut input = demo_market_input(registry, direct.compiler()).expect("market input");
        let gate = cubic_price_gate_v1();
        let knots = [0_i128, 0, 0, 0, 3, 3, 3, 3];
        let failure = [0_u64, 0, 0, 11];
        let cuts = [1_i128, 2];
        let coefficients = [1_u64; 4];
        let semantic_product_id = ProductContentId::new([0x31; 32]).expect("Product identity");
        let spline = SplineProductCompilationInputV3 {
            product_id: semantic_product_id,
            coordinate_domain_id: ProductContentId::new([0x32; 32]).expect("coordinate"),
            result_unit_id: ProductContentId::new([0x33; 32]).expect("unit"),
            claim_basis_id: ProductContentId::new([0x34; 32]).expect("claim basis"),
            representation_release_id: ProductContentId::new([0x35; 32]).expect("representation"),
            mapping_release_id: ProductContentId::new([0x36; 32]).expect("mapping"),
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: 1,
            coefficients: &coefficients,
            evaluator_release_id: ProductContentId::new([0x37; 32]).expect("evaluator"),
            degree: 3,
            interior_multiplicity: false,
            payout_scale: 11,
            knot_denominator: 1,
            knots: &knots,
            failure_payouts: &failure,
            price_gate_certificate: &gate,
        };
        let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
        let mut domain = vec![0_u8; result_domain_record_bytes(2).expect("domain")];
        let mut portfolio = vec![0_u8; portfolio_record_bytes(4).expect("portfolio")];
        let mut basis = vec![0_u8; spline_basis_output_bytes_v3(spline).expect("basis width")];
        let compiled = compile_spline_product_records_v3(
            registry,
            spline,
            &mut product,
            &mut domain,
            &mut portfolio,
            &mut basis,
        )
        .expect("compiler-shaped cubic Product");
        input.liability_basis_id = hex(&compiled.semantic_basis_id.to_bytes());
        input.linked_basis_hex = hex(&basis);
        input.price_gate_hex = hex(&gate);
        let domain_digest: [u8; 32] = Sha256::digest(&domain).into();
        let admitted = authenticate_market_basis_v1(&input, semantic_product_id, domain_digest, 4)
            .expect("admitted cubic basis");
        assert_eq!(admitted.payout_scale, 11);
        assert_eq!(admitted.body, basis);
        assert_eq!(admitted.price_gate.as_deref(), Some(gate.as_slice()));

        let mut missing = input.clone();
        missing.price_gate_hex.clear();
        assert!(
            authenticate_market_basis_v1(&missing, semantic_product_id, domain_digest, 4).is_err()
        );
        let mut forged = input.clone();
        let mut forged_gate = gate;
        forged_gate[dclutch_product::payoff::price_gate_v1::PRICE_GATE_PRICES_OFFSET_V1] ^=
            1;
        forged.price_gate_hex = hex(&forged_gate);
        assert!(
            authenticate_market_basis_v1(&forged, semantic_product_id, domain_digest, 4).is_err()
        );
        let mut scaled = input.clone();
        let mut scaled_basis = basis.clone();
        scaled_basis[160..168].copy_from_slice(&12_u64.to_le_bytes());
        scaled.linked_basis_hex = hex(&scaled_basis);
        assert!(
            authenticate_market_basis_v1(&scaled, semantic_product_id, domain_digest, 4).is_err()
        );
        assert!(authenticate_market_basis_v1(&input, semantic_product_id, [0x99; 32], 4).is_err());
        assert!(
            authenticate_market_basis_v1(
                &input,
                ProductContentId::new([0x98; 32]).expect("substituted Product"),
                domain_digest,
                4,
            )
            .is_err()
        );
    }

    struct FoundInfrastructureFixtureV1 {
        plan: SuccessorPlan,
        profile_address: Pubkey,
        profile_account: RpcAccount,
        coordinates: FoundInfrastructureCoordinatesV1,
        registry_raw_account: RpcAccount,
    }

    fn found_infrastructure_fixture_v1() -> FoundInfrastructureFixtureV1 {
        use crate::model::InfrastructureSuccessionPinV1;
        use dclutch_registry::release_set::{
            ArtifactReleaseIdV1, ExecutionRoleBindingV1,
            PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
            PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1, ProgramIdentityV1,
        };

        let mut plan = split_founding_fixture_v1().plan;
        let registry = pubkey(&plan.registry.program_id).expect("Registry program");
        let rent = pubkey(&plan.rent_credit.program_id).expect("Rent program");
        let core = pubkey(&plan.core.program_id).expect("Core program");
        let predecessor_registry =
            ArtifactReleaseIdV1::new([0xa1; 32]).expect("predecessor Registry artifact");
        let predecessor_rent =
            ArtifactReleaseIdV1::new([0xa2; 32]).expect("predecessor Rent artifact");
        let registry_body = b"successor Registry artifact release fixture".to_vec();
        let successor_registry_digest: [u8; 32] = Sha256::digest(&registry_body).into();
        let successor_registry = ArtifactReleaseIdV1::new(successor_registry_digest)
            .expect("successor Registry artifact");
        let registry_binding = |artifact| {
            ExecutionRoleBindingV1::new(
                ProgramIdentityV1::new(registry.to_bytes()).expect("Registry identity"),
                artifact,
            )
        };
        let rent_binding = ExecutionRoleBindingV1::new(
            ProgramIdentityV1::new(rent.to_bytes()).expect("Rent identity"),
            predecessor_rent,
        );
        let predecessor = ProtocolInfrastructureProfileV1::new(
            registry_binding(predecessor_registry),
            rent_binding,
        )
        .expect("predecessor infrastructure profile");
        let successor = ProtocolInfrastructureProfileV2::new(
            registry_binding(successor_registry),
            rent_binding,
            predecessor_registry,
            predecessor_rent,
        )
        .expect("successor infrastructure profile");
        let predecessor_bytes = predecessor.to_bytes();
        let profile_address =
            Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;
        let profile_account = RpcAccount {
            lamports: 1,
            owner: core,
            executable: false,
            rent_epoch: 0,
            data: successor.to_bytes().to_vec(),
        };
        plan.infrastructure_profile.address =
            Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &core)
                .0
                .to_string();
        plan.infrastructure_profile.schema_id = hex(&PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1);
        plan.infrastructure_profile.body_sha256 = hex(&Sha256::digest(predecessor_bytes));
        plan.infrastructure_profile.body_hex = hex(&predecessor_bytes);
        plan.infrastructure_profile.registry_artifact_release_id =
            hex(predecessor_registry.as_bytes());
        plan.infrastructure_profile.rent_artifact_release_id = hex(predecessor_rent.as_bytes());
        plan.infrastructure_succession = Some(InfrastructureSuccessionPinV1 {
            schema: "dclutch-local-infrastructure-succession-pin-v1".into(),
            registry_upgrade_buffer: Pubkey::new_unique().to_string(),
            registry_candidate_elf_sha256: hex(&[0xb1; 32]),
            predecessor_registry_artifact_release_id: hex(predecessor_registry.as_bytes()),
            predecessor_rent_artifact_release_id: hex(predecessor_rent.as_bytes()),
        });
        let coordinates =
            checked_successor_found_coordinates_v1(&plan, profile_address, &profile_account)
                .expect("successor Found coordinates");
        let registry_raw_account = RpcAccount {
            lamports: 1,
            owner: registry,
            executable: false,
            rent_epoch: 0,
            data: registry_body,
        };
        FoundInfrastructureFixtureV1 {
            plan,
            profile_address,
            profile_account,
            coordinates,
            registry_raw_account,
        }
    }

    /// A cohort born at V2: no succession pin, a genesis V2 at the V2 PDA.
    struct GenesisFoundFixtureV1 {
        plan: SuccessorPlan,
        profile_address: Pubkey,
        profile_account: RpcAccount,
    }

    fn genesis_found_fixture_v1() -> GenesisFoundFixtureV1 {
        use dclutch_registry::release_set::{
            ArtifactReleaseIdV1, ExecutionRoleBindingV1,
            PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
            PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1, ProgramIdentityV1,
        };

        let mut plan = split_founding_fixture_v1().plan;
        let registry = pubkey(&plan.registry.program_id).expect("Registry program");
        let rent = pubkey(&plan.rent_credit.program_id).expect("Rent program");
        let core = pubkey(&plan.core.program_id).expect("Core program");
        let registry_artifact =
            ArtifactReleaseIdV1::new([0xd1; 32]).expect("Registry artifact release");
        let rent_artifact = ArtifactReleaseIdV1::new([0xd2; 32]).expect("Rent artifact release");
        let registry_binding = ExecutionRoleBindingV1::new(
            ProgramIdentityV1::new(registry.to_bytes()).expect("Registry identity"),
            registry_artifact,
        );
        let rent_binding = ExecutionRoleBindingV1::new(
            ProgramIdentityV1::new(rent.to_bytes()).expect("Rent identity"),
            rent_artifact,
        );
        let v1 = ProtocolInfrastructureProfileV1::new(registry_binding, rent_binding)
            .expect("sealed V1 profile");
        let genesis = ProtocolInfrastructureProfileV2::genesis(registry_binding, rent_binding)
            .expect("genesis V2 profile");
        let v1_bytes = v1.to_bytes();
        let genesis_bytes = genesis.to_bytes();
        let v1_address =
            Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &core).0;
        let profile_address =
            Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;

        plan.infrastructure_succession = None;
        plan.infrastructure_profile.address = v1_address.to_string();
        plan.infrastructure_profile.schema_id = hex(&PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1);
        plan.infrastructure_profile.body_sha256 = hex(&Sha256::digest(v1_bytes));
        plan.infrastructure_profile.body_hex = hex(&v1_bytes);
        plan.infrastructure_profile.registry_artifact_release_id =
            hex(registry_artifact.as_bytes());
        plan.infrastructure_profile.rent_artifact_release_id = hex(rent_artifact.as_bytes());
        plan.genesis_infrastructure_profile.address = profile_address.to_string();
        plan.genesis_infrastructure_profile.schema_id =
            hex(&PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V2);
        plan.genesis_infrastructure_profile.body_sha256 = hex(&Sha256::digest(genesis_bytes));
        plan.genesis_infrastructure_profile.body_hex = hex(&genesis_bytes);
        plan.genesis_infrastructure_profile
            .registry_artifact_release_id = hex(registry_artifact.as_bytes());
        plan.genesis_infrastructure_profile.rent_artifact_release_id =
            hex(rent_artifact.as_bytes());

        GenesisFoundFixtureV1 {
            plan,
            profile_address,
            profile_account: RpcAccount {
                lamports: 1,
                owner: core,
                executable: false,
                rent_epoch: 0,
                data: genesis_bytes.to_vec(),
            },
        }
    }

    /// The genesis arm founds, the vacancy refuses, and no arm falls back to V1.
    ///
    /// `Predecessor` -- the arm that fed the 144-byte V1 into the projection
    /// whenever a plan had no succession -- is gone, and this is the test of
    /// that: a plan with no succession and no V2 on chain is now a REFUSAL
    /// naming the vacancy, where it used to be a foundable-looking projection
    /// that failed sixty transactions deep on chain.
    #[test]
    fn found_infrastructure_selection_is_genesis_or_planned_successor_and_never_v1() {
        let genesis = genesis_found_fixture_v1();
        assert_eq!(
            checked_found_infrastructure_selection_v1(&genesis.plan, true)
                .expect("a born-at-V2 cohort founds against its genesis V2"),
            FoundInfrastructureSelectionV1::Genesis
        );
        let vacant = checked_found_infrastructure_selection_v1(&genesis.plan, false)
            .expect_err("a vacant V2 PDA is not a fallback to the V1");
        assert!(
            vacant.to_string().contains("run the initialize stage"),
            "{vacant}"
        );

        // The selected plan points the founding path at the V2 the chain
        // authenticates, and touches nothing else.
        let records_before =
            serde_json::to_value(&genesis.plan.records).expect("plan records JSON");
        let selected = checked_genesis_found_plan_v1(
            &genesis.plan,
            genesis.profile_address,
            &genesis.profile_account,
        )
        .expect("genesis Found selection");
        assert_eq!(
            selected.infrastructure_profile.address,
            genesis.profile_address.to_string()
        );
        assert_eq!(
            selected.infrastructure_profile.schema_id,
            hex(&PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V2)
        );
        assert_eq!(
            selected.infrastructure_profile.body_hex,
            genesis.plan.genesis_infrastructure_profile.body_hex
        );
        assert_eq!(
            serde_json::to_value(&selected.records).expect("selected records JSON"),
            records_before,
            "a genesis founding moves no Registry artifact record"
        );

        // Hostile: a SUCCEEDED V2 at the same address under the same plan.
        // Half a forgery is still a forgery on chain; here the whole account
        // is well-formed and Core-owned and still must not found, because this
        // plan carries no succession to authenticate it against.
        let succeeded = ProtocolInfrastructureProfileV2::new(
            ProtocolInfrastructureProfileV1::decode(
                &decode_hex(&genesis.plan.infrastructure_profile.body_hex).expect("V1 body"),
            )
            .expect("V1 profile")
            .registry(),
            ProtocolInfrastructureProfileV1::decode(
                &decode_hex(&genesis.plan.infrastructure_profile.body_hex).expect("V1 body"),
            )
            .expect("V1 profile")
            .rent(),
            dclutch_registry::release_set::ArtifactReleaseIdV1::new([0xe1; 32])
                .expect("real predecessor Registry"),
            dclutch_registry::release_set::ArtifactReleaseIdV1::new([0xe2; 32])
                .expect("real predecessor Rent"),
        )
        .expect("succeeded profile");
        let mut succeeded_account = genesis.profile_account.clone();
        succeeded_account.data = succeeded.to_bytes().to_vec();
        assert!(!succeeded.born_at_v2());
        assert!(
            checked_genesis_found_plan_v1(
                &genesis.plan,
                genesis.profile_address,
                &succeeded_account
            )
            .is_err(),
            "a succeeded V2 is not a born-at-V2 cohort and must not found without its ceremony"
        );

        // Hostile: a well-formed genesis V2 for DIFFERENT bindings.
        let mut foreign_plan = genesis.plan.clone();
        foreign_plan.genesis_infrastructure_profile.body_hex = hex(&[0x00_u8; 4]);
        assert!(
            checked_genesis_found_plan_v1(
                &foreign_plan,
                genesis.profile_address,
                &genesis.profile_account
            )
            .is_err(),
            "the observed V2 must be byte-identical to the plan's own pin"
        );

        // Positive control: the succession arm is untouched.
        let planned = found_infrastructure_fixture_v1();
        assert_eq!(
            checked_found_infrastructure_selection_v1(&planned.plan, true)
                .expect("a planned succession still selects its successor"),
            FoundInfrastructureSelectionV1::PlannedSuccessor
        );
    }

    #[test]
    fn found_successor_selection_is_atomic_and_rewrites_only_registry() {
        let fixture = found_infrastructure_fixture_v1();
        let rent_before = serde_json::to_value(
            fixture
                .plan
                .records
                .get("rent_artifact_release")
                .expect("Rent record"),
        )
        .expect("Rent record JSON");
        let selected = checked_successor_found_plan_v1(
            &fixture.plan,
            &fixture.profile_account,
            fixture.coordinates,
            fixture.coordinates.registry_raw,
            &fixture.registry_raw_account,
            fixture.coordinates.registry_staging,
            None,
        )
        .expect("atomic successor selection");
        assert_eq!(
            selected.infrastructure_profile.address,
            fixture.profile_address.to_string()
        );
        assert_eq!(
            selected.infrastructure_profile.schema_id,
            hex(&PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V2)
        );
        assert_eq!(
            selected.infrastructure_profile.registry_artifact_release_id,
            hex(&fixture.coordinates.registry_artifact_id)
        );
        assert_eq!(
            selected
                .records
                .get("registry_artifact_release")
                .expect("selected Registry record")
                .raw,
            fixture.coordinates.registry_raw.to_string()
        );
        assert_eq!(
            selected.infrastructure_profile.rent_artifact_release_id,
            fixture.plan.infrastructure_profile.rent_artifact_release_id
        );
        assert_eq!(
            serde_json::to_value(
                selected
                    .records
                    .get("rent_artifact_release")
                    .expect("selected Rent record")
            )
            .expect("selected Rent record JSON"),
            rent_before,
            "Registry succession must not rewrite Rent"
        );
    }

    #[test]
    fn found_successor_profile_refuses_cross_generation_and_account_substitution() {
        use dclutch_registry::release_set::{
            ArtifactReleaseIdV1, ExecutionRoleBindingV1, ProgramIdentityV1,
        };

        let fixture = found_infrastructure_fixture_v1();
        let predecessor = ProtocolInfrastructureProfileV1::decode(
            &decode_hex(&fixture.plan.infrastructure_profile.body_hex).expect("predecessor body"),
        )
        .expect("predecessor profile");
        let successor = ProtocolInfrastructureProfileV2::decode(&fixture.profile_account.data)
            .expect("successor profile");

        let cross_generation = ProtocolInfrastructureProfileV2::new(
            successor.registry(),
            predecessor.rent(),
            ArtifactReleaseIdV1::new([0xc1; 32]).expect("foreign predecessor"),
            predecessor.rent().artifact_release(),
        )
        .expect("cross-generation profile")
        .to_bytes();
        let mut account = fixture.profile_account.clone();
        account.data = cross_generation.to_vec();
        assert!(
            checked_successor_found_coordinates_v1(
                &fixture.plan,
                fixture.profile_address,
                &account
            )
            .is_err()
        );

        let foreign_rent = ExecutionRoleBindingV1::new(
            ProgramIdentityV1::new(predecessor.rent().program().to_bytes()).expect("Rent program"),
            ArtifactReleaseIdV1::new([0xc2; 32]).expect("foreign Rent artifact"),
        );
        account.data = ProtocolInfrastructureProfileV2::new(
            successor.registry(),
            foreign_rent,
            predecessor.registry().artifact_release(),
            predecessor.rent().artifact_release(),
        )
        .expect("cross-generation Rent profile")
        .to_bytes()
        .to_vec();
        assert!(
            checked_successor_found_coordinates_v1(
                &fixture.plan,
                fixture.profile_address,
                &account
            )
            .is_err()
        );

        for data in [
            fixture.profile_account.data[..PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2 - 1].to_vec(),
            {
                let mut extended = fixture.profile_account.data.clone();
                extended.push(0);
                extended
            },
        ] {
            account = fixture.profile_account.clone();
            account.data = data;
            assert!(
                checked_successor_found_coordinates_v1(
                    &fixture.plan,
                    fixture.profile_address,
                    &account
                )
                .is_err(),
                "non-canonical V2 profile width must refuse"
            );
        }

        assert!(
            checked_successor_found_coordinates_v1(
                &fixture.plan,
                Pubkey::new_unique(),
                &fixture.profile_account
            )
            .is_err(),
            "substituted profile PDA must refuse"
        );
        for account in [
            RpcAccount {
                owner: Pubkey::new_unique(),
                ..fixture.profile_account.clone()
            },
            RpcAccount {
                executable: true,
                ..fixture.profile_account.clone()
            },
        ] {
            assert!(
                checked_successor_found_coordinates_v1(
                    &fixture.plan,
                    fixture.profile_address,
                    &account
                )
                .is_err(),
                "substituted profile authority must refuse"
            );
        }
    }

    #[test]
    fn found_successor_record_refuses_raw_and_staging_substitution() {
        let fixture = found_infrastructure_fixture_v1();
        for (raw, staging) in [
            (Pubkey::new_unique(), fixture.coordinates.registry_staging),
            (fixture.coordinates.registry_raw, Pubkey::new_unique()),
        ] {
            assert!(
                checked_successor_found_plan_v1(
                    &fixture.plan,
                    &fixture.profile_account,
                    fixture.coordinates,
                    raw,
                    &fixture.registry_raw_account,
                    staging,
                    None,
                )
                .is_err(),
                "substituted raw/staging coordinate must refuse"
            );
        }

        let mut wrong_body = fixture.registry_raw_account.clone();
        wrong_body.data[0] ^= 1;
        let wrong_accounts = [
            wrong_body,
            RpcAccount {
                owner: Pubkey::new_unique(),
                ..fixture.registry_raw_account.clone()
            },
            RpcAccount {
                executable: true,
                ..fixture.registry_raw_account.clone()
            },
        ];
        for account in &wrong_accounts {
            assert!(
                checked_successor_found_plan_v1(
                    &fixture.plan,
                    &fixture.profile_account,
                    fixture.coordinates,
                    fixture.coordinates.registry_raw,
                    account,
                    fixture.coordinates.registry_staging,
                    None,
                )
                .is_err(),
                "substituted finalized raw record must refuse"
            );
        }
        let unexpected_staging = RpcAccount {
            lamports: 1,
            owner: pubkey(&fixture.plan.registry.program_id).expect("Registry program"),
            executable: false,
            rent_epoch: 0,
            data: vec![1],
        };
        assert!(
            checked_successor_found_plan_v1(
                &fixture.plan,
                &fixture.profile_account,
                fixture.coordinates,
                fixture.coordinates.registry_raw,
                &fixture.registry_raw_account,
                fixture.coordinates.registry_staging,
                Some(&unexpected_staging),
            )
            .is_err(),
            "a live staging cursor must refuse finalized selection"
        );
    }

    #[test]
    fn ordinary_found_snapshot_pins_manifest_and_runtime_rent_coordinates() {
        let manifest = Pubkey::new_unique();
        let mut keys = (0..FOUND_ACCOUNT_COUNT_V3)
            .map(|_| Pubkey::new_unique())
            .collect::<Vec<_>>();
        keys[FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3] = manifest;
        keys[FOUND_RENT_SYSVAR_INDEX_V3] = sysvar::rent::ID;
        authenticate_found_snapshot_coordinates_v3(&keys, manifest, None)
            .expect("canonical Found37 coordinates");

        let mut wrong_rent = keys.clone();
        wrong_rent[FOUND_RENT_SYSVAR_INDEX_V3] = Pubkey::new_unique();
        assert!(authenticate_found_snapshot_coordinates_v3(&wrong_rent, manifest, None).is_err());

        let mut missing_rent = keys;
        missing_rent.remove(FOUND_RENT_SYSVAR_INDEX_V3);
        assert!(authenticate_found_snapshot_coordinates_v3(&missing_rent, manifest, None).is_err());

        let gate = (Pubkey::new_unique(), Pubkey::new_unique());
        let mut gated = vec![Pubkey::new_unique(); FOUND_ACCOUNT_COUNT_V3];
        gated[FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3] = manifest;
        gated[FOUND_RENT_SYSVAR_INDEX_V3] = sysvar::rent::ID;
        gated.extend([gate.0, gate.1]);
        authenticate_found_snapshot_coordinates_v3(&gated, manifest, Some(gate))
            .expect("canonical Found39 coordinates");
        let mut swapped = gated.clone();
        swapped.swap(FOUND_ACCOUNT_COUNT_V3, FOUND_ACCOUNT_COUNT_V3 + 1);
        assert!(
            authenticate_found_snapshot_coordinates_v3(&swapped, manifest, Some(gate)).is_err()
        );
        assert!(authenticate_found_snapshot_coordinates_v3(&gated, manifest, None).is_err());
    }

    fn sponsored_price_update_for_test() -> Vec<u8> {
        let release = dclutch_source::pyth::devnet_sponsored_sol_usd_release_v1()
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
        let update = dclutch_source::pyth::FullPriceUpdateV2::parse(&price).expect("price update");
        devnet_sponsored_market_input(
            DevnetPythMarketSpecV1 {
                // The fixture states no ladder, which is the shape every
                // devnet market founded before ladders were authorable.
                recovery: None,
                founding_band: LocalMarketShapeV1::default().founding_band,
                registry,
                price_update: &price,
                product_name: "product/sol-usd-sponsored-range-protection",
                coordinate_domain_name: "coordinate-domain/usd-cents-per-sol",
                feed_label: b"sol-usd-sponsored",
                window_start: update.publish_time() - 1_800,
                window_width_seconds: 1_800,
                max_age_seconds: 7_200,
                cut_denominator: 100,
                // Centred on the same $150 spot the band declares, for the
                // same reason the shared default moved: $120/$180 is one
                // cell taking the whole question.
                cuts: vec![14_800, 15_200],
                coefficients: vec![1, 0, 1, 0],
                generation: 1,
            },
            direct.compiler(),
            dclutch_source::pyth::devnet_sponsored_sol_usd_release_v1()
                .expect("the declared devnet sponsored release"),
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
        use dclutch_market::capability_manifest::{
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
                selected_founding_controller_masks_v1(
                    CapabilityManifestV1::decode(&bytes).expect("manifest"),
                    [0x30; 32],
                    DIRECT_SUCCESSOR_KIND_ID_V3,
                )
                .expect("partition"),
                (
                    u16::try_from(direct_index).expect("index"),
                    [0b1111 ^ direct_mask, direct_mask],
                ),
            );
        }
    }

    /// The founding census's half of the cycle-3 multi-capability ruling:
    /// even where the codec admits two coexisting trade kinds, the funding
    /// arithmetic welds founding to one — every entry must be funded by
    /// exactly one of the two controllers, the Trading controller funds
    /// exactly the selected entry, and a second non-Resolution entry of
    /// another kind is refused symmetrically whichever kind is selected.
    #[test]
    fn founding_masks_weld_the_manifest_to_one_selected_capability() {
        use dclutch_market::capability_manifest::{
            ActivationPolicy, CAPABILITY_ENTRY_BYTES, CompartmentFundingV1, FundingAmountsV1,
            FundingQuoteV1, MANIFEST_HEADER_BYTES,
        };
        let none = CompartmentFundingV1::not_applicable();
        let quote = FundingQuoteV1::new(
            FundingAmountsV1::new(none, none, none, none, none, none, none).expect("amounts"),
            None,
        )
        .expect("quote");
        let entry = |kind: [u8; 32], release: [u8; 32], salt: u8| {
            CapabilityEntryV1::new(
                CapabilityContentId::new(kind).expect("kind"),
                CapabilityContentId::new(release).expect("release"),
                CapabilityContentId::new([0x40 + salt; 32]).expect("config"),
                CapabilityContentId::new([0x50 + salt; 32]).expect("capacity"),
                CapabilityContentId::new([0x60; 32]).expect("schema"),
                CapabilityContentId::new([0x70; 32]).expect("derivation"),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quote,
            )
            .expect("entry")
        };
        let resolution = [0x30; 32];
        let first_kind = [0x81; 32];
        let second_kind = [0x82; 32];
        let entries = [
            entry([0x11; 32], resolution, 0),
            entry([0x12; 32], resolution, 1),
            entry([0x13; 32], resolution, 2),
            entry(first_kind, [0x31; 32], 3),
            entry(second_kind, [0x32; 32], 4),
        ];
        let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&entries, &mut bytes)
            .expect("five distinct kinds encode");
        let manifest = CapabilityManifestV1::decode(&bytes).expect("manifest");
        for selected in [first_kind, second_kind] {
            selected_founding_controller_masks_v1(manifest, resolution, selected)
                .expect_err("a second trade capability cannot ride any founding");
        }
    }

    #[test]
    fn successor_controller_masks_refuse_missing_or_ambiguous_direct_ownership() {
        for bytes in [
            direct_controller_manifest(None, None),
            direct_controller_manifest(Some(3), Some(1)),
        ] {
            assert!(
                selected_founding_controller_masks_v1(
                    CapabilityManifestV1::decode(&bytes).expect("manifest"),
                    [0x30; 32],
                    DIRECT_SUCCESSOR_KIND_ID_V3,
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
        let expected = dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_ID_V7;
        assert!(
            selected_founding_controller_masks_v1(manifest, expected, DIRECT_SUCCESSOR_KIND_ID_V3)
                .is_ok()
        );
        assert!(
            selected_founding_controller_masks_v1(
                manifest,
                dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_ID_V5,
                DIRECT_SUCCESSOR_KIND_ID_V3,
            )
            .is_err()
        );
        assert!(
            selected_founding_controller_masks_v1(
                manifest,
                [0x5a; 32],
                DIRECT_SUCCESSOR_KIND_ID_V3
            )
            .is_err()
        );
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
        // THE GEOMETRY IS DERIVED, NEVER TYPED. This fixture used to declare
        // 55 distinct keys with a 12-writable prefix as literals and then PAD
        // to the frame width by repeating them, so a frame that grew by two
        // writable accounts grew only in repeated indexes and the census read
        // an unchanged 58/12. Seating the failure escrow is exactly that
        // change, and it made this fixture model a frame nobody builds.
        let loaded = GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3
            .saturating_sub(GENERIC_MARKET_FOUNDING_CENSUS_STATIC_KEYS_V3);
        let distinct = (0..loaded)
            .map(|index| {
                Pubkey::new_from_array([u8::try_from(index).unwrap_or(u8::MAX).saturating_add(1); 32])
            })
            .collect::<Vec<_>>();
        let mut accounts = distinct
            .iter()
            .enumerate()
            .map(|(index, key)| {
                if index < GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3 {
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

    /// One coherent synthetic founding input set: a distinct key per protocol
    /// role, real PDA derivations where the builders verify them, and the
    /// exact two-ledger physical funding profile the censuses pin. Built once
    /// so the composed and split builders are fed byte-identical coordinates.
    struct SplitFoundingFixtureV1 {
        plan: SuccessorPlan,
        coordinates: FoundingCoordinates,
        records: MarketRecords,
        outer: FoundingOuterV1,
        requests: [Pubkey; 4],
        founder: Pubkey,
        mint: Pubkey,
        payer: Pubkey,
    }

    fn split_founding_fixture_v1() -> SplitFoundingFixtureV1 {
        use crate::model::{CoreBootstrapPin, InfrastructureProfilePin, ProgramPin, RecordPair};

        fn pin(program_id: Pubkey, programdata_id: Pubkey) -> ProgramPin {
            ProgramPin {
                program_id: program_id.to_string(),
                programdata_id: programdata_id.to_string(),
                elf_path: String::new(),
                elf_sha256: String::new(),
                checked_candidate_elf_path: String::new(),
                checked_candidate_elf_sha256: String::new(),
                live_elf_sha256: String::new(),
                live_elf_padding_bytes: 0,
                semantic_release_id: String::new(),
                artifact_release_id: String::new(),
                upgrade_authority: None,
                deployment_slot: 0,
                deployment_source: String::new(),
                programdata_sha256: String::new(),
            }
        }
        fn published(seed: u8) -> PublishedRecord {
            PublishedRecord {
                schema: [seed; 32],
                digest: [seed.wrapping_add(1); 32],
                raw: Pubkey::new_unique(),
                staging: Pubkey::new_unique(),
            }
        }
        fn identity(byte: u8) -> Identity {
            Identity::new([byte; 32]).expect("nonzero identity")
        }

        let registry = Pubkey::new_unique();
        let artifact_record = |schema: [u8; 32], content: [u8; 32]| RecordPair {
            raw: Pubkey::find_program_address(
                &[RAW_RECORD_PDA_SEED_V1, &schema, &content],
                &registry,
            )
            .0
            .to_string(),
            staging: Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &content],
                &registry,
            )
            .0
            .to_string(),
            schema_id: hex(&schema),
            content_sha256: hex(&content),
            body_hex: String::new(),
        };
        let mut plan_records = BTreeMap::new();
        plan_records.insert(
            "registry_artifact_release".to_string(),
            artifact_record([0x51; 32], [0x52; 32]),
        );
        plan_records.insert(
            "rent_artifact_release".to_string(),
            artifact_record([0x53; 32], [0x54; 32]),
        );
        let plan = SuccessorPlan {
            schema: String::new(),
            genesis_boundary: Vec::new(),
            bootstrap_order: Vec::new(),
            execution_blocker: String::new(),
            account_dir: String::new(),
            registry: pin(registry, Pubkey::new_unique()),
            core: pin(Pubkey::new_unique(), Pubkey::new_unique()),
            claims: pin(Pubkey::new_unique(), Pubkey::new_unique()),
            trading: pin(Pubkey::new_unique(), Pubkey::new_unique()),
            resolution: pin(Pubkey::new_unique(), Pubkey::new_unique()),
            custody: pin(Pubkey::new_unique(), Pubkey::new_unique()),
            rent_credit: pin(Pubkey::new_unique(), Pubkey::new_unique()),
            activation: Pubkey::new_unique().to_string(),
            release_set_id: String::new(),
            core_bootstrap: CoreBootstrapPin {
                upgrade_authority: String::new(),
                genesis_programdata_sha256: String::new(),
                post_revoke_programdata_sha256: String::new(),
                release_recognition_requires_revoke: false,
            },
            checked_upgrade_set: None,
            checked_local_mutable_set: None,
            infrastructure_succession: None,
            infrastructure_profile: InfrastructureProfilePin {
                address: Pubkey::new_unique().to_string(),
                schema_id: String::new(),
                body_sha256: String::new(),
                body_hex: String::new(),
                registry_artifact_release_id: String::new(),
                rent_artifact_release_id: String::new(),
            },
            genesis_infrastructure_profile: InfrastructureProfilePin {
                address: Pubkey::new_unique().to_string(),
                schema_id: String::new(),
                body_sha256: String::new(),
                body_hex: String::new(),
                registry_artifact_release_id: String::new(),
                rent_artifact_release_id: String::new(),
            },
            records: plan_records,
            record_publication: "genesis".to_string(),
            provider_release_id: String::new(),
            fixture_publish_time: 0,
            genesis_accounts: BTreeMap::new(),
            general_accelerator: None,
        };

        let records = MarketRecords {
            realm: published(0x60),
            product: published(0x62),
            domain: published(0x64),
            portfolio: published(0x66),
            source: published(0x68),
            source_capacity_profile: published(0x6a),
            manipulation_floor: None,
            recovery: None,
            manifest: published(0x6c),
            manifest_body: Vec::new(),
            basis: published(0x6e),
            price_gate: None,
            basis_scale: 1,
            basis_refunds_on_failure: false,
            source_spec: published(0x70),
            window_spec: published(0x72),
            statistic_spec: published(0x74),
            provider_release: published(0x76),
            adapter_config: published(0x78),
            recovery_sources: Vec::new(),
            sponsored_push_release: None,
            direct: BTreeMap::new(),
            principal_cap_sets: 1,
        };

        let found = GenericFoundingRequestV1::new(
            GenericFoundingStageV1::FoundAndPermit,
            2,
            identity(0x01),
            identity(0x02),
            identity(0x03),
            identity(0x04),
            identity(0x05),
            identity(0x06),
            identity(0x07),
            identity(0x08),
            identity(0x09),
            identity(0x0a),
            11,
            12,
            13,
            14,
            15,
            16,
            2,
            1,
        )
        .expect("canonical found request");
        let lock = ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: [0x02; 32],
            generation: 11,
            realm: [0x11; 32],
            product_record: [0x12; 32],
            product: [0x13; 32],
            source: [0x14; 32],
            release_set: [0x01; 32],
            projection_receipt_digest: [0x15; 32],
            parent_capability_root: [0x03; 32],
            context_digest: [0x16; 32],
            caller_program: [0x17; 32],
            payer: [0x18; 32],
            core_program: [0x19; 32],
            rent_program: [0x1a; 32],
            refund_owner: [0x06; 32],
            rent_credit: [0x1b; 32],
            hoard_vault: [0x08; 32],
            funding_source_vault: [0x07; 32],
            funding_source_context: [0x04; 32],
            funding_source_compartment: CompartmentV1::External,
            mint: [0x1c; 32],
            token_program: [0x1d; 32],
            collateral_release: [0x1e; 32],
            expiry_slot: 14,
            expected_revision: 3,
            resulting_revision: 4,
            amount: 156,
            state_rent_lamports: 1,
            vault_rent_lamports: 1,
            funding_source_replay_revision: 1,
            funding_source_state_rent_lamports: 1,
            funding_source_vault_rent_lamports: 1,
        };
        let coordinates = FoundingCoordinates {
            generation: 11,
            identity: MarketIdentity {
                market_id: identity(0x02),
                realm_id: identity(0x11),
                product_record: identity(0x12),
                product_id: identity(0x13),
                resolution_policy: identity(0x14),
                capability_manifest: identity(0x1f),
                selected_release_set: identity(0x01),
                registry_program: Identity::new(registry.to_bytes()).expect("registry identity"),
                generation: 11,
            },
            market: Pubkey::new_unique(),
            credit: Pubkey::new_unique(),
            hoard_vault: Pubkey::new_unique(),
            source_vault: Pubkey::new_unique(),
            source_replay: Pubkey::new_unique(),
            projected_replay: Pubkey::new_unique(),
            custody_authority: Pubkey::new_unique(),
            controller_funding_checkpoint: Pubkey::new_unique(),
            context: [0x04; 32],
            principal_cap_sets: 1,
            capability_entry_count: 2,
            capability_entry_index: 1,
            funding_ledgers: vec![
                FoundingFundingLedgerV2 {
                    address: Pubkey::new_unique(),
                    controller: Pubkey::new_unique(),
                    selected_mask: 1,
                    bytes: Vec::new(),
                    required_lamports: 1,
                },
                FoundingFundingLedgerV2 {
                    address: Pubkey::new_unique(),
                    controller: Pubkey::new_unique(),
                    selected_mask: 2,
                    bytes: Vec::new(),
                    required_lamports: 1,
                },
            ],
            found,
            lock,
        };
        let outer = FoundingOuterV1 {
            found_raw: vec![0x21; 4],
            lock_raw: vec![0x22; 4],
            realize_raw: vec![0x23; 4],
            claims_raw: vec![0x24; 4],
            substituted_claims_raw: vec![0x25; 4],
            lock_caller: Pubkey::new_unique(),
            lock_caller_bump: 251,
            realize_caller: Pubkey::new_unique(),
            realize_caller_bump: 252,
            claims_caller: Pubkey::new_unique(),
            claims_caller_bump: 253,
            found_authority: Pubkey::new_unique(),
            found_authority_bump: 254,
            open_authority: Pubkey::new_unique(),
            open_authority_bump: 255,
            permit: Pubkey::new_unique(),
            aggregate: Pubkey::new_unique(),
            position: Pubkey::new_unique(),
            admission: Pubkey::new_unique(),
            escrow_position: Pubkey::new_unique(),
            escrow_admission: Pubkey::new_unique(),
            seats_failure_escrow: false,
            aggregate_width: 258,
            position_width: 258,
            market_rent: 15,
            permit_rent: 16,
        };
        SplitFoundingFixtureV1 {
            plan,
            coordinates,
            records,
            outer,
            requests: [
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                Pubkey::new_unique(),
            ],
            founder: Pubkey::new_unique(),
            mint: Pubkey::new_unique(),
            payer: Pubkey::new_unique(),
        }
    }

    /// THE PREDICTION FOLLOWS THE DEPLOYED CORE, NOT THIS TREE.
    ///
    /// Cohort-14c founded against a Core built at `8e96ec3f8`, which predates
    /// `b312ce3c4`, and the driver at HEAD predicted eight bumps for it. The
    /// founding refused on the bump tail AFTER 0.139 SOL and AFTER the
    /// canonical Found37 Market existed, naming a disagreement that was
    /// computable before the first transaction:
    ///
    ///     product_graph: ProductGraphBumpsV1([0, 0, 0, 0, 0, 0, 0, 0])
    ///     this driver predicts ProductGraphBumpsV1([254, 255, ...])
    ///
    /// So the same identity, the same records and the same registry must
    /// produce the tail the NAMED Core writes: zeros for cohort-14's deployed
    /// bytes and the walk's eight nibbles for the build this tree installs. The
    /// three bumps that predate `b312ce3c4` are unmoved by either answer, which
    /// is the half that proves the projection is narrowed and not disabled.
    #[test]
    fn the_predicted_bump_tail_is_the_one_the_named_core_writes() {
        let fixture = split_founding_fixture_v1();
        let core = pubkey(&fixture.plan.core.program_id).expect("core id");
        let registry = pubkey(&fixture.plan.registry.program_id).expect("registry id");

        let mut deployed = fixture.plan.core.clone();
        deployed.checked_candidate_elf_sha256 =
            "864394530f37c04e53d10f918c8fab0c265187549895bf5a9207ae91f2a7d02f".into();
        deployed.elf_sha256 = deployed.checked_candidate_elf_sha256.clone();
        deployed.deployment_source = "observed-programdata-account".into();

        let mut installed = fixture.plan.core.clone();
        installed.checked_candidate_elf_sha256 = "9e".repeat(32);
        installed.elf_sha256 = installed.checked_candidate_elf_sha256.clone();
        installed.deployment_source = "genesis-install".into();

        let predict = |pin: &crate::model::ProgramPin| {
            predicted_state_bumps_v1(
                core,
                registry,
                fixture.coordinates.identity,
                &fixture.records,
                core_product_graph_projection_v1(pin, None).expect("a Core this driver can model"),
                CoreProductGraphWalkV1::OrdinaryFound,
            )
            .expect("the fixture's derivations carry")
        };

        let cohort_14 = predict(&deployed);
        let this_tree = predict(&installed);

        assert_eq!(
            cohort_14.product_graph,
            ProductGraphBumpsV1::ABSENT,
            "a Core deployed before b312ce3c4 writes zeros in the reserved nibbles",
        );
        assert_ne!(
            this_tree.product_graph,
            ProductGraphBumpsV1::ABSENT,
            "the Core this tree installs records the walk it performed",
        );
        assert_eq!(
            this_tree.product_graph.bumps().len(),
            PRODUCT_GRAPH_BUMP_COUNT
        );

        // Everything Core recorded BEFORE b312ce3c4 is identical under both
        // projections: the skew is exactly the Product graph and nothing else.
        assert_eq!(cohort_14.market, this_tree.market);
        assert_eq!(cohort_14.realm_raw_record, this_tree.realm_raw_record);
        assert_eq!(
            cohort_14.realm_staging_record,
            this_tree.realm_staging_record
        );
        assert_ne!(cohort_14, this_tree);
    }

    /// The two Core walks differ in exactly one packed byte, and that byte
    /// moves the digest the founding commits to.
    ///
    /// This is the defect that kept tier 1 red, written as a test so it cannot
    /// come back quietly. `found::authenticate_projected_references` walks
    /// three Product-graph records; `found::authenticate_references` walks four.
    /// Predicting the four-record tail for a projected founding moves ONE BYTE
    /// of `CoreState` -- packed byte 3, the linked-basis pair -- and that byte
    /// is hashed into the projected Realize receipt, whose digest is a
    /// coordinate of `FoundingIntentV5`, which Claims compares as one SHA-256.
    ///
    /// So the assertions are: the two walks agree everywhere Core's frames
    /// agree, they differ in exactly the basis pair, and a one-byte difference
    /// in that pair is a different founding intent. The last one is what makes
    /// the first two worth checking rather than trivia about a bump.
    #[test]
    fn the_projected_founding_walk_is_the_ordinary_walk_without_its_basis_pair() {
        let fixture = split_founding_fixture_v1();
        let core = pubkey(&fixture.plan.core.program_id).expect("core id");
        let registry = pubkey(&fixture.plan.registry.program_id).expect("registry id");
        let projection =
            core_product_graph_projection_v1(&fixture.plan.core, None).expect("modelled Core");
        let predict = |walk| {
            predicted_state_bumps_v1(
                core,
                registry,
                fixture.coordinates.identity,
                &fixture.records,
                projection,
                walk,
            )
            .expect("the fixture's derivations carry")
        };
        let ordinary = predict(CoreProductGraphWalkV1::OrdinaryFound);
        let projected = predict(CoreProductGraphWalkV1::ProjectedFounding);

        assert_eq!(ordinary.market, projected.market);
        assert_eq!(ordinary.realm_raw_record, projected.realm_raw_record);
        assert_eq!(
            ordinary.realm_staging_record,
            projected.realm_staging_record
        );

        let ordinary_bumps = ordinary.product_graph.bumps();
        let projected_bumps = projected.product_graph.bumps();
        assert_eq!(
            ordinary_bumps.get(..6),
            projected_bumps.get(..6),
            "the Product, ResultDomain and Portfolio pairs are in both Core frames",
        );
        assert_eq!(
            projected_bumps.get(6..8),
            Some([0_u8, 0].as_slice()),
            "Core's projected Found never sees the linked-basis record, so its pair is unrecorded",
        );
        assert_ne!(
            ordinary_bumps.get(6..8),
            Some([0_u8, 0].as_slice()),
            "the ordinary Found does see it; a fixture where it did not would make this vacuous",
        );

        // ONE BYTE, AND IT IS A DIFFERENT FOUNDING. The two tails are encoded
        // into otherwise identical candidate states and hashed the way
        // `derive_founding_outer_v1` hashes the one it commits to.
        let state_with = |bumps| CoreState {
            phase: Phase::Founding,
            readiness: Readiness::Prepaid,
            terminal_winner: 0,
            identity: fixture.coordinates.identity,
            outstanding_capabilities: 0,
            principal_cap_sets: fixture.coordinates.principal_cap_sets,
            rent_beneficiary: identity_of(fixture.coordinates.credit.to_bytes())
                .expect("rent credit identity"),
            terminal_receipt: None,
            bumps,
        };
        let ordinary_bytes = state_with(ordinary).encode().expect("ordinary state");
        let projected_bytes = state_with(projected).encode().expect("projected state");
        let differing: Vec<usize> = ordinary_bytes
            .iter()
            .zip(projected_bytes.iter())
            .enumerate()
            .filter_map(|(index, (left, right))| (left != right).then_some(index))
            .collect();
        assert_eq!(
            differing.len(),
            1,
            "exactly one CoreState byte separates the two walks, and it was {differing:?}",
        );
        assert_ne!(
            Sha256::digest(ordinary_bytes).to_vec(),
            Sha256::digest(projected_bytes).to_vec(),
            "that byte is what the founding intent commits to three stages early",
        );
    }

    #[test]
    fn split_stage1_frame_is_the_composed_frame_without_its_open_window() {
        let fixture = split_founding_fixture_v1();
        let composed = build_generic_market_founding_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("composed frame");
        let stage1 = build_generic_found_and_permit_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("stage-1 frame");

        // Splice the 21-account Open window out of the composed frame; the
        // stage-1 assembly must reproduce the remainder meta for meta,
        // privilege for privilege — the shared windows were not perturbed.
        let open_start =
            composed.instruction.accounts.len() - 1 - GENERIC_FOUNDING_OPEN_WINDOW_ACCOUNTS_V1;
        let mut spliced = composed.instruction.accounts.clone();
        spliced.drain(open_start..open_start + GENERIC_FOUNDING_OPEN_WINDOW_ACCOUNTS_V1);
        assert_eq!(spliced, stage1.instruction.accounts);
        assert_eq!(
            stage1.instruction.accounts.len(),
            GENERIC_FOUND_AND_PERMIT_FIXED_ACCOUNTS_V1
                + GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3
        );

        // Same program, same bumps in execution order, no Open bump.
        assert_eq!(
            stage1.instruction.program_id,
            composed.instruction.program_id
        );
        let mut expected_data = GENERIC_FOUND_AND_PERMIT_MAGIC_V1.to_vec();
        expected_data.extend_from_slice(&[
            fixture.outer.lock_caller_bump,
            fixture.outer.found_authority_bump,
            fixture.outer.realize_caller_bump,
            fixture.outer.claims_caller_bump,
        ]);
        assert_eq!(stage1.instruction.data, expected_data);

        // The spliced-out window is exactly the composed Open section: its
        // first account is the Open caller PDA, the key the split retires
        // from stage 1.
        assert_eq!(
            composed.instruction.accounts[open_start].pubkey,
            fixture.outer.open_authority
        );
    }

    #[test]
    fn split_stage2_frame_carries_the_composed_open_window_behind_two_raws() {
        let fixture = split_founding_fixture_v1();
        let composed = build_generic_market_founding_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("composed frame");
        let found_raw_account = fixture.requests[0];
        let claims_raw_account = fixture.requests[3];
        let stage2 = build_generic_market_open_v1(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            found_raw_account,
            claims_raw_account,
        )
        .expect("stage-2 frame");

        assert_eq!(
            stage2.instruction.accounts.len(),
            GENERIC_MARKET_OPEN_FRAME_ACCOUNTS_V1
        );
        assert_eq!(stage2.instruction.accounts[0].pubkey, found_raw_account);
        assert_eq!(stage2.instruction.accounts[1].pubkey, claims_raw_account);
        assert!(!stage2.instruction.accounts[0].is_writable);
        assert!(!stage2.instruction.accounts[1].is_writable);

        // Key order of the window is byte-identical to the composed route's
        // Open section; writability is the standalone minimum instead of the
        // composed frame's union privileges.
        let open_start =
            composed.instruction.accounts.len() - 1 - GENERIC_FOUNDING_OPEN_WINDOW_ACCOUNTS_V1;
        let composed_window: Vec<Pubkey> = composed.instruction.accounts
            [open_start..open_start + GENERIC_FOUNDING_OPEN_WINDOW_ACCOUNTS_V1]
            .iter()
            .map(|meta| meta.pubkey)
            .collect();
        let stage2_window: Vec<Pubkey> = stage2.instruction.accounts[2..]
            .iter()
            .map(|meta| meta.pubkey)
            .collect();
        assert_eq!(composed_window, stage2_window);
        for (index, meta) in stage2.instruction.accounts[2..].iter().enumerate() {
            // Window positions 1, 2, 3: the Market, the permit, the RentCredit.
            assert_eq!(
                meta.is_writable,
                matches!(index, 1 | 2 | 3),
                "window index {index}"
            );
        }

        let mut expected_data = GENERIC_MARKET_OPEN_MAGIC_V1.to_vec();
        expected_data.push(fixture.outer.open_authority_bump);
        assert_eq!(stage2.instruction.data, expected_data);

        // Aliased raw requests refuse at assembly.
        assert!(
            build_generic_market_open_v1(
                &fixture.plan,
                &fixture.coordinates,
                &fixture.outer,
                found_raw_account,
                found_raw_account,
            )
            .is_err()
        );
    }

    #[test]
    fn split_stage1_census_is_the_composed_census_minus_the_open_caller() {
        let fixture = split_founding_fixture_v1();
        let composed = build_generic_market_founding_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("composed frame");
        let stage1 = build_generic_found_and_permit_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("stage-1 frame");

        // The synthetic fixture reproduces the exact live census the composed
        // authenticator pins, which is what makes the stage-1 delta meaningful.
        let composed_census =
            authenticate_generic_market_founding_lock_census_v3(fixture.payer, &composed)
                .expect("composed census");
        let stage1_census =
            authenticate_generic_found_and_permit_lock_census_v3(fixture.payer, &stage1)
                .expect("stage-1 census");
        assert_eq!(
            composed_census.complete_keys,
            GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3
        );
        assert_eq!(
            stage1_census.complete_keys,
            GENERIC_FOUND_AND_PERMIT_COMPLETE_KEYS_V1
        );
        // Exactly one readonly loaded key — the Open caller PDA — leaves the
        // census; the twelve distinct writable keys are untouched.
        assert_eq!(
            composed_census.complete_keys - stage1_census.complete_keys,
            1
        );
        assert_eq!(
            composed_census.loaded_writable,
            stage1_census.loaded_writable
        );
        assert_eq!(
            composed_census.loaded_readonly - stage1_census.loaded_readonly,
            1
        );
    }

    #[test]
    fn cubic_gate_pair_extends_both_generic_frames_without_moving_legacy_accounts() {
        let mut fixture = split_founding_fixture_v1();
        let bare_composed = build_generic_market_founding_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("bare composed frame");
        let bare_stage1 = build_generic_found_and_permit_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("bare stage-1 frame");
        let gate = PublishedRecord {
            schema: PRICE_GATE_RECORD_SCHEMA_ID_V1,
            digest: [0x9a; 32],
            raw: Pubkey::new_unique(),
            staging: Pubkey::new_unique(),
        };
        fixture.records.price_gate = Some(gate);
        fixture.records.basis_scale = 11;
        let composed = build_generic_market_founding_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("gated composed frame");
        let stage1 = build_generic_found_and_permit_v3(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            &fixture.records,
            fixture.requests,
            fixture.founder,
            fixture.mint,
        )
        .expect("gated stage-1 frame");
        assert_eq!(
            composed.instruction.accounts.len(),
            GENERIC_MARKET_FOUNDING_PRICE_GATE_FIXED_ACCOUNTS_V4
                + GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3
        );
        assert_eq!(
            stage1.instruction.accounts.len(),
            GENERIC_FOUND_AND_PERMIT_PRICE_GATE_FIXED_ACCOUNTS_V2
                + GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V3
        );
        for (instruction, bare) in [
            (&composed.instruction, &bare_composed.instruction),
            (&stage1.instruction, &bare_stage1.instruction),
        ] {
            let raw = instruction
                .accounts
                .iter()
                .position(|meta| meta.pubkey == gate.raw)
                .expect("gate raw");
            assert_eq!(instruction.accounts[raw + 1].pubkey, gate.staging);
            assert!(!instruction.accounts[raw].is_writable);
            assert!(!instruction.accounts[raw + 1].is_writable);
            let mut stripped = instruction.accounts.clone();
            stripped.drain(raw..raw + 2);
            assert_eq!(stripped, bare.accounts);
        }
        assert_eq!(
            authenticate_generic_market_founding_lock_census_v3(fixture.payer, &composed)
                .expect("gated composed census")
                .complete_keys,
            GENERIC_MARKET_FOUNDING_PRICE_GATE_COMPLETE_KEYS_V4
        );
        assert_eq!(
            authenticate_generic_found_and_permit_lock_census_v3(fixture.payer, &stage1)
                .expect("gated stage-1 census")
                .complete_keys,
            GENERIC_FOUND_AND_PERMIT_PRICE_GATE_COMPLETE_KEYS_V2
        );
    }

    #[test]
    fn split_stage2_frame_admission_holds_and_pins_three_writable_keys() {
        let fixture = split_founding_fixture_v1();
        let stage2 = build_generic_market_open_v1(
            &fixture.plan,
            &fixture.coordinates,
            &fixture.outer,
            fixture.requests[0],
            fixture.requests[3],
        )
        .expect("stage-2 frame");
        authenticate_generic_market_open_frame_v1(fixture.payer, &stage2)
            .expect("stage-2 frame admission");

        let writable: Vec<Pubkey> = stage2
            .instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_writable)
            .map(|meta| meta.pubkey)
            .collect();
        assert_eq!(
            writable,
            vec![
                fixture.coordinates.market,
                fixture.outer.permit,
                fixture.coordinates.credit,
            ]
        );

        // A frame whose digest moved after preparation refuses admission.
        let mut tampered = stage2.clone();
        tampered.instruction.accounts[2].pubkey = Pubkey::new_unique();
        assert!(authenticate_generic_market_open_frame_v1(fixture.payer, &tampered).is_err());
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

        let addresses = canonical_routing_addresses_v1(payer, instructions)
            .expect("the readiness frame names addresses a table may carry");
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
                true,
                &instructions,
                table.observation,
                std::slice::from_ref(&table),
            )
            .expect("routed geometry");
            assert_eq!(
                geometry.complete_keys,
                operation.exact_unique_accounts(true)
            );
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
            true,
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
            assert_eq!(
                base.complete_keys,
                CONTROLLER_FUNDING_CLEANUP_COMPLETE_KEYS_V1
            );
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

    /// A routing table carries no program id, no signer, and no payer -- and the
    /// entries it stopped carrying were dead weight, not a saving traded for a
    /// behaviour change.
    ///
    /// Until 2026-09-02 `canonical_routing_addresses_v1` PUSHED the invoked
    /// program id into every routing table it built, and so did the relayed
    /// vertical's copy of it. A program id can never be resolved through a table
    /// -- it has to be known before the message's tables load -- so the runtime
    /// kept it inline anyway and the entry bought nothing while its rent was
    /// paid for as long as the table existed.
    ///
    /// The second half of this test is what makes the first half safe to
    /// believe. It compiles each frame TWICE, once over the derived table and
    /// once over a table with the program id added back, and requires the two
    /// messages to serialise identically. If the entry had ever been doing
    /// anything, that is where it would show.
    #[test]
    fn a_routing_table_carries_no_program_id_and_the_entry_it_dropped_did_nothing() {
        let frames = [
            ("DCLTPCB2", projected_bootstrap_census_fixture_v2()),
            ("DCLTCFQ1", controller_funding_prepare_census_fixture_v1()),
            ("DCLTPCA1", staged_abort_census_fixture_v1()),
        ];
        for (label, (payer, instruction)) in frames {
            let addresses =
                canonical_routing_addresses_v1(payer, std::slice::from_ref(&instruction))
                    .unwrap_or_else(|error| panic!("{label} routing addresses: {error:?}"));
            assert!(
                !addresses.contains(&instruction.program_id),
                "{label}: the invoked program cannot be resolved through a table"
            );
            assert!(
                !addresses.contains(&payer),
                "{label}: the fee payer is always static key zero"
            );
            for meta in &instruction.accounts {
                if meta.is_signer {
                    assert!(
                        !addresses.contains(&meta.pubkey),
                        "{label}: a signer is authenticated by its header position"
                    );
                }
            }

            let compile = |table_addresses: Vec<Pubkey>| {
                v0::Message::try_compile(
                    &payer,
                    std::slice::from_ref(&instruction),
                    &[AddressLookupTableAccount {
                        key: Pubkey::new_from_array([0xfe; 32]),
                        addresses: table_addresses,
                    }],
                    Hash::new_from_array([0x43; 32]),
                )
                .unwrap_or_else(|error| panic!("{label} census compile: {error}"))
                .serialize()
            };
            let mut widened = addresses.clone();
            widened.push(instruction.program_id);
            assert_eq!(
                compile(addresses),
                compile(widened),
                "{label}: the program-id entry changed the compiled message, so it was not dead \
                 weight and dropping it is not free"
            );
        }
    }

    /// What freezing a routing table costs a market, derived rather than typed.
    ///
    /// Freezing adds no rent. A lookup table is rent-exempt for as long as it
    /// exists either way; what freezing removes is the ability to CLOSE it and
    /// take the rent back. So the price is the whole table's rent-exempt
    /// minimum, and it is a price only to the extent that anyone would have
    /// reclaimed it. Nobody was: `plan_lookup_table_retirement_v1` exists in the
    /// operator and this tree has no caller for it, so before this change the
    /// rent was paid forever on a table that merely retained the power to be
    /// rewritten.
    ///
    /// Priced at the widest routed founding frame, DCLTGMF3's 55 addresses,
    /// which the neighbouring extent test pins.
    #[test]
    fn a_frozen_routing_table_costs_one_rent_exempt_minimum_per_market() {
        let rent = solana_program::rent::Rent::default();
        let table_of = |addresses: usize| {
            rent.minimum_balance(LOOKUP_TABLE_META_BYTES_V1 + addresses.saturating_mul(32))
        };

        // One table at the widest founding width: 0.01353024 SOL.
        assert_eq!(table_of(55), 13_530_240);

        // Eleven, which is what a founding run publishes through this function
        // after the two builders became one: 0.14883264 SOL, forfeited per
        // market rather than per run.
        assert_eq!(table_of(55).saturating_mul(11), 148_832_640);

        // And the shape of it, so a narrower frame prices itself: the entry cost
        // is linear and the base is one account's overhead plus the table meta.
        assert_eq!(
            table_of(56).saturating_sub(table_of(55)),
            222_720,
            "one address is 32 bytes of rent-exempt table"
        );
    }

    /// What one dropped table entry stops costing, derived rather than typed.
    ///
    /// An address is 32 bytes of lookup-table data, and a table is rent-exempt
    /// for as long as it exists, so the entry's cost is the rent-exempt minimum
    /// of 32 more bytes. Every routing table this file publishes carried exactly
    /// one such entry it could never use.
    #[test]
    fn a_dropped_program_id_entry_stops_paying_thirty_two_bytes_of_permanent_rent() {
        let rent = solana_program::rent::Rent::default();
        let one_entry = rent
            .minimum_balance(LOOKUP_TABLE_META_BYTES_V1 + 32)
            .checked_sub(rent.minimum_balance(LOOKUP_TABLE_META_BYTES_V1))
            .expect("one lookup-table entry costs a non-negative rent");
        assert_eq!(one_entry, 222_720);
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
    fn generic_founding_final_compiler_census_pins_the_60_key_shape() {
        let (payer, prepared) = generic_market_founding_census_fixture_v3();
        let census = authenticate_generic_market_founding_lock_census_v3(payer, &prepared)
            .expect("canonical DCLTGMF3 census");
        // Sixty and fourteen since the failure escrow was seated at founding:
        // its Position and its admission are two more writable keys, and the
        // readonly count did not move.
        assert_eq!(census.complete_keys, 60);
        assert_eq!(census.required_signatures, 1);
        assert_eq!(census.static_keys, 3);
        assert_eq!(census.loaded_writable, 14);
        assert_eq!(census.loaded_readonly, 43);
        assert_eq!(
            hex(&census.key_privilege_digest),
            "10e652f8a26ead25d6d08a99590e4eaa465ed2c12e2f73584bd5657ad6eb29cc"
        );
        assert_eq!(
            census,
            authenticate_generic_market_founding_lock_census_v3(payer, &prepared)
                .expect("deterministic census")
        );
    }

    /// DCLTGMF3's extent both ways, on the shape the producer actually sends.
    ///
    /// This is the widest founding frame in the tree and it has been on a frozen
    /// routing table since it was written -- and it is the one founding route
    /// with no measured extent. `CompleteLockCensusV1` pins keys, signatures and
    /// privileges but carries no byte fields, so this frame was packet-bounded
    /// by ARGUMENT while every neighbouring census route was packet-bounded by a
    /// number. The journal refuses a planned packet over 1,232 at three separate
    /// points, which catches a regression on a validator at run time; it does
    /// not tell a reader what the margin is, and a margin nobody can read is a
    /// margin nobody defends.
    ///
    /// Both figures come from the same instruction through
    /// `bounded_instructions`, so they carry the compute-unit limit, the
    /// priority fee and the 256 KiB heap request the real submission carries.
    /// The legacy figure is the control: it is compiled and thrown away, and
    /// without it the routed figure says nothing about whether the route moved
    /// or the instrument did.
    #[test]
    fn generic_founding_final_pins_its_packet_extent_and_its_legacy_control() {
        let (payer, prepared) = generic_market_founding_census_fixture_v3();
        let bounded = bounded_instructions(
            std::slice::from_ref(&prepared.instruction),
            Some(FOUNDING_HEAP_FRAME_BYTES),
        )
        .expect("bounded DCLTGMF3 submission");

        let legacy = solana_sdk::message::legacy::Message::new(&bounded, Some(&payer));
        let legacy_signatures = usize::from(legacy.header.num_required_signatures);
        let legacy_bytes = 1 + legacy_signatures * 64 + legacy.serialize().len();

        let addresses =
            canonical_routing_addresses_v1(payer, &bounded).expect("DCLTGMF3 routing addresses");
        let routed = v0::Message::try_compile(
            &payer,
            &bounded,
            &[AddressLookupTableAccount {
                key: Pubkey::new_from_array([0xfe; 32]),
                addresses,
            }],
            Hash::new_from_array([0x43; 32]),
        )
        .expect("DCLTGMF3 compiles over its routing table");
        let routed_signatures = usize::from(routed.header.num_required_signatures);
        let static_keys = routed.account_keys.len();
        let loaded: usize = routed
            .address_table_lookups
            .iter()
            .map(|lookup| lookup.writable_indexes.len() + lookup.readonly_indexes.len())
            .sum();
        let routed_bytes =
            1 + routed_signatures * 64 + VersionedMessage::V0(routed).serialize().len();

        // 2,198 as a legacy message, 966 over the maximum, so this frame was
        // never submittable inline and the table is not an optimisation. 57 of
        // its 60 coordinates become one-byte indexes; the three that stay are
        // the payer, the invoked program and the ComputeBudget program, none of
        // which a table can move.
        //
        // It was 2,129 over 55 coordinates until the failure escrow was seated
        // at founding: two more keys is 64 bytes of key plus four index bytes
        // plus one compact-u16 step, which is exactly the 69 between them. The
        // table absorbs the keys, so the ROUTED figure moves by five rather
        // than sixty-nine.
        assert_eq!(legacy_bytes, 2_198);
        assert_eq!(routed_bytes, 467);
        assert_eq!(static_keys, 3);
        assert_eq!(loaded, 57);
        assert!(
            legacy_bytes > 1_232,
            "a route that already fit would need no table"
        );
        assert!(routed_bytes <= 1_232, "the table did not close the overrun");
        // The same 60 the neighbouring census pins as `complete_keys`. Two
        // instruments, one number: if they ever disagree, one of them is
        // measuring a frame the producer does not send.
        assert_eq!(
            static_keys + loaded,
            GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3
        );
    }

    #[test]
    fn generic_founding_complete_key_census_enforces_the_64_65_wall() {
        let (payer, prepared) = generic_market_founding_census_fixture_v3();
        // FOUR, WHERE IT WAS SIX. The frame's own sixty keys leave four before
        // the devnet limit, and that headroom is what the failure escrow's two
        // accounts were bought with. The limit did not move; the frame did.
        let headroom = DEVNET_ACCOUNT_LOCK_LIMIT_V1 - GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3;
        assert_eq!(headroom, 4);
        let admitted = compiled_complete_lock_census_v1(
            payer,
            &append_distinct_census_accounts_v1(&prepared.instruction, headroom),
        )
        .expect("64-key census");
        let refused = compiled_complete_lock_census_v1(
            payer,
            &append_distinct_census_accounts_v1(&prepared.instruction, headroom + 1),
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
        // One more distinct key than the frame's own headroom, so the drifted
        // frame lands exactly one past the limit at unchanged width.
        let drift = DEVNET_ACCOUNT_LOCK_LIMIT_V1 - GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3 + 1;
        for offset in 0..drift {
            prepared.instruction.accounts[width - 1 - offset].pubkey = Pubkey::new_from_array(
                [0xa0_u8.saturating_add(u8::try_from(offset).unwrap_or(0)); 32],
            );
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

        // The last two DISTINCT coordinates, derived rather than indexed by
        // hand: the fixture repeats keys past that point, so a literal index
        // silently stops naming a distinct key the moment the frame widens.
        let distinct_keys = GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V3
            - GENERIC_MARKET_FOUNDING_CENSUS_STATIC_KEYS_V3;
        let mut duplicate = prepared.clone();
        let removed = duplicate.instruction.accounts[distinct_keys - 1].pubkey;
        let retained = duplicate.instruction.accounts[distinct_keys - 2].pubkey;
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

        // The FIRST READONLY coordinate, which is the writable prefix's width.
        // It was written as a literal twelve and stopped being readonly the
        // moment the failure escrow's two writable accounts joined the frame,
        // so the privilege drift this asserts became no drift at all.
        let mut privilege = prepared.clone();
        privilege.instruction.accounts[GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3].is_writable =
            true;
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

    /// The rule that lets a COMPLETED founding be resumed at all.
    ///
    /// The topology is cohort-13's, transcribed from its own journals
    /// (`~/jobs/dclutch-cohort13-20260902/market/campaign-open.json`,
    /// 2026-09-02): `Q9zc5g4f…` is the Trading Pending ledger DCLTCFQ1
    /// completes, DCLTPCB2 names it as a prestate and does NOT retain it, and
    /// on chain it is vacant. Before this rule the resume called
    /// `required_account` on it and refused a landed founding by name.
    ///
    /// The controls matter as much as the case: an address no later stage
    /// names must still refuse, and an address a later stage RETAINS must not
    /// be reported consumed.
    #[test]
    fn a_vacant_poststate_is_a_pass_only_when_a_later_stage_consumed_it() {
        use crate::market::founding_submission_journal::{
            FoundingSubmissionJournalV1, FoundingSubmissionOperationV1,
        };

        const PENDING_LEDGER: &str = "Q9zc5g4fqVt84215uXg1XqkZ6kYxRwKkyTRwPiuZsBp";
        const FUNDING_LEDGER: &str = "9VDSCch4JXG3oL3CCFMVX4iypuDeY8fsW5yVAKxEPyCV";
        const NOBODY: &str = "So11111111111111111111111111111111111111112";

        fn journal(
            operation: FoundingSubmissionOperationV1,
            prestate: &[&str],
            completion: &[&str],
        ) -> FoundingSubmissionJournalV1 {
            let mut row = serde_json::from_str::<FoundingSubmissionJournalV1>(
                &serde_json::to_string(&serde_json::json!({
                    "schema": "x", "cluster": "devnet", "genesisHash": "g",
                    "evidencePath": "p", "rpcUrl": "u", "planSha256": "00".repeat(32),
                    "marketSha256": "00".repeat(32), "payer": NOBODY,
                    "operation": "dcltcfq1", "phase": "finalized",
                    "messageBase64": "", "messageSha256": "00".repeat(32),
                    "lastValidBlockHeight": 1, "exactFeeLamports": 5000,
                    "exactUniqueMessageAccounts": 1, "expectedSigners": [],
                    "resolvedAccountsSha256": "00".repeat(32),
                    "prestateAccounts": prestate, "prestateSha256": "00".repeat(32),
                    "completionAccounts": completion,
                    "completionContractSha256": "00".repeat(32),
                    "recoveryPayloadBase64": "", "recoveryPayloadSha256": "00".repeat(32),
                    "expectedWireBytes": 1, "intentSha256": "00".repeat(32),
                    "signedPacketBase64": null, "signedPacketSha256": null,
                    "expectedSignature": null, "finalizedSlot": null,
                    "transactionSha256": null, "feeLamports": null,
                    "computeUnitsConsumed": null, "finalizedPoststates": [],
                    "finalizedPoststatesSha256": null, "stateSha256": "00".repeat(32),
                }))
                .expect("journal fixture JSON"),
            )
            .expect("journal fixture");
            row.operation = operation;
            row
        }

        let dcltpcb2 = journal(
            FoundingSubmissionOperationV1::Dcltpcb2,
            &[PENDING_LEDGER, FUNDING_LEDGER],
            &[FUNDING_LEDGER],
        );
        let later = [&dcltpcb2];

        assert_eq!(
            super::later_founding_stage_naming_v1(PENDING_LEDGER, &later, true),
            Some(FoundingSubmissionOperationV1::Dcltpcb2),
            "DCLTPCB2 reads DCLTCFQ1's Pending ledger and does not retain it: that is consumption"
        );
        assert_eq!(
            super::later_founding_stage_naming_v1(FUNDING_LEDGER, &later, true),
            None,
            "an account a later stage RETAINS was not consumed, and a vacancy there is a refusal"
        );
        assert_eq!(
            super::later_founding_stage_naming_v1(FUNDING_LEDGER, &later, false),
            Some(FoundingSubmissionOperationV1::Dcltpcb2),
            "a retained account is still NAMED, so a changed one has a later owner"
        );
        assert_eq!(
            super::later_founding_stage_naming_v1(NOBODY, &later, false),
            None,
            "an address no later stage names has no explanation, changed or vacant"
        );
        assert_eq!(
            super::later_founding_stage_naming_v1(PENDING_LEDGER, &[], true),
            None,
            "the LAST stage has no successor, so its own poststates must still be live"
        );
    }
}

/// A live, KEY-FREE re-run of the two rules a completed founding's recovery
/// turns on, against a real campaign report and a real cluster.
///
/// Ignored and environment-driven, so an ordinary `cargo test` opens no
/// socket. It exists because the reconstruction's only honest evidence is a
/// real founding's real journals against the chain that holds them: the wall
/// this module fixed was invisible to every offline fixture, twice.
///
/// It reads no keypair. Everything it needs -- the binding, the six journals,
/// the frozen Pending expectation for each controller funding ledger -- comes
/// out of the report the founding itself wrote, and the only chain traffic is
/// `getAccountInfo` under a reads-only write policy.
///
///     DCLUTCH_LIVE_FOUNDING_REPORT=/abs/campaign-open.json \
///     DCLUTCH_LIVE_FOUNDING_RPC=<url> \
///     cargo test --bin dclutch-local-successor-bootstrap -- --ignored --nocapture \
///         a_completed_foundings_later_stages
#[cfg(test)]
mod live_founding_boundary {
    use super::{
        BoundaryRpcV1, LaterFoundingStagesV1, account_evidence,
        authenticate_recorded_founding_poststates_v1,
        founding_submission_journal::{
            FoundingSubmissionBindingV1, FoundingSubmissionJournalV1,
            FoundingSubmissionOperationV1, authenticate_founding_submission_v1,
        },
    };
    use crate::{
        cluster::ClusterOriginV1,
        model::AccountEvidence,
        rpc::{Rpc, WritePolicyV1},
    };
    use solana_sdk::pubkey::Pubkey;

    /// The binding a founding's own journal states, rebuilt from that journal.
    ///
    /// Every field is the journal's; only the recovery-policy shape is not
    /// recorded, and it is resolved by asking which of the two geometries the
    /// journal authenticates under rather than by consulting the Market input.
    fn binding_from_journals_v1(
        journals: &[FoundingSubmissionJournalV1],
    ) -> FoundingSubmissionBindingV1 {
        let head = journals
            .first()
            .expect("a founding states at least one journal");
        let mut refusal = None;
        for market_has_recovery_policy in [true, false] {
            let binding = FoundingSubmissionBindingV1 {
                cluster: head.cluster.clone(),
                genesis_hash: head.genesis_hash.clone(),
                evidence_path: head.evidence_path.clone(),
                rpc_url: head.rpc_url.clone(),
                plan_sha256: head.plan_sha256.clone(),
                market_sha256: head.market_sha256.clone(),
                payer: head.payer.parse::<Pubkey>().expect("journal payer"),
                market_has_recovery_policy,
            };
            // The geometry pin is per-operation, and the three founding legs
            // are shape-invariant -- so only the whole set discriminates.
            match journals
                .iter()
                .try_for_each(|journal| authenticate_founding_submission_v1(&binding, journal))
            {
                Ok(()) => {
                    println!("binding: market_has_recovery_policy = {market_has_recovery_policy}");
                    return binding;
                }
                Err(error) => refusal = Some(error),
            }
        }
        panic!(
            "no geometry authenticates the report's own six journals: {}",
            refusal.map(|error| error.0).unwrap_or_default()
        )
    }

    #[test]
    #[ignore = "reads a live cluster; set DCLUTCH_LIVE_FOUNDING_REPORT and DCLUTCH_LIVE_FOUNDING_RPC"]
    fn a_completed_foundings_later_stages_account_for_every_open_boundary_difference() {
        let report_path =
            std::env::var("DCLUTCH_LIVE_FOUNDING_REPORT").expect("DCLUTCH_LIVE_FOUNDING_REPORT");
        let url = std::env::var("DCLUTCH_LIVE_FOUNDING_RPC").expect("DCLUTCH_LIVE_FOUNDING_RPC");
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&report_path).expect("campaign report"))
                .expect("campaign report JSON");
        let journals: Vec<FoundingSubmissionJournalV1> =
            serde_json::from_value(report["foundingSubmissionJournals"].clone())
                .expect("founding submission journals");
        assert_eq!(
            journals.len(),
            FoundingSubmissionOperationV1::ORDER.len(),
            "a completed founding states all six journals"
        );
        let binding = binding_from_journals_v1(&journals);
        let origin =
            ClusterOriginV1::parse(&url, Some(&binding.genesis_hash)).expect("live cluster origin");
        let mut rpc =
            Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly).expect("live cluster");

        // 1. The rule `00793136` established, over every recorded poststate.
        for journal in &journals {
            let dispositions = authenticate_recorded_founding_poststates_v1(
                &mut rpc, &binding, journal, &journals,
            )
            .unwrap_or_else(|error| {
                panic!("{} poststates: {}", journal.operation.label(), error.0)
            });
            println!(
                "{} : {} recorded poststates",
                journal.operation.label(),
                dispositions.len()
            );
            for row in &dispositions {
                println!("    {} {}", row.address, row.disposition);
            }
        }

        // 2. The rule this commit adds, over the exact accounts the Open
        //    acknowledgement compares: the controller funding ledgers, frozen
        //    at their Pending expectation in the founding's own checkpoint.
        let accounts = report["foundingCheckpoint"]["accounts"]
            .as_object()
            .expect("checkpoint accounts");
        let ledgers = accounts
            .iter()
            .filter(|(label, _)| label.starts_with("founding_funding_ledger_v2_"))
            .map(|(label, value)| {
                (
                    label.clone(),
                    serde_json::from_value::<AccountEvidence>(value.clone())
                        .expect("checkpoint ledger evidence"),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            !ledgers.is_empty(),
            "the instrument found no controller funding ledger in the checkpoint, so a pass \
             below would mean nothing"
        );
        let later = LaterFoundingStagesV1::authenticated(
            &binding,
            FoundingSubmissionOperationV1::Dcltgmf3,
            &journals,
        )
        .expect("later founding stages");
        const REFUSAL_V1: &str =
            "Open changed a Pending controller funding ledger while consuming its checkpoint";
        let mut differing = 0;
        for (label, recorded) in &ledgers {
            let address = recorded.address.parse::<Pubkey>().expect("ledger address");
            let holds =
                |account: &crate::rpc::RpcAccount| &account_evidence(address, account) == recorded;
            // The positive control first: with NO later stages, a ledger the
            // chain has moved must still refuse in the invariant's own words.
            // Without this a clean pass below could equally mean the ledgers
            // never moved and the instrument measured nothing.
            let at_boundary = BoundaryRpcV1::at_boundary(&mut rpc).boundary_account(
                address,
                "Open controller funding ledger",
                holds,
                REFUSAL_V1,
            );
            match &at_boundary {
                Ok(()) => println!("{label} {address}: unchanged since the checkpoint"),
                Err(error) => {
                    differing += 1;
                    println!(
                        "{label} {address}: MOVED since the checkpoint -- {}",
                        error.0
                    );
                }
            }
            BoundaryRpcV1::after_boundary(&mut rpc, &later)
                .boundary_account(address, "Open controller funding ledger", holds, REFUSAL_V1)
                .unwrap_or_else(|error| panic!("{label} {address}: {}", error.0));
            println!("{label} {address}: accounted for at the Open boundary");
        }
        assert!(
            differing > 0,
            "no controller funding ledger differs from its Pending checkpoint bytes, so this run \
             never exercised the rule it is here to check"
        );
    }
}

/// The class of defect this module has now paid for twice, and its detector.
///
/// An authenticator named for a PAST transaction's poststate states an
/// invariant about one boundary. Every live account read it makes silently
/// re-evaluates that invariant at whatever wall time the process runs, and
/// while the caller is the transaction's own driver those two clocks coincide
/// -- which is why nothing caught it until a reconstruction path existed.
/// `capture_founding_poststates_v1` was the first (`00793136`); the Open
/// verifier's Pending funding-ledger loop was the second, one verifier over in
/// the same commit, and it cost cohort-13 a second build-and-run cycle against
/// a live founding under a deadline.
///
/// So the rule is structural rather than remembered: on the reconstruction
/// path, an authenticator whose NAME claims a past poststate or checkpoint may
/// not touch `Rpc`'s account readers directly. It reads through
/// [`BoundaryRpcV1`], whose two method families force each read to say whether
/// it is a permanent fact or a boundary-time expectation.
///
/// Scope is deliberate, and it is the reason the two siblings the cohort-13
/// lane named -- `authenticate_controller_funding_checkpoint_v1` and
/// `authenticate_controller_funding_cleanup_checkpoint_v1` -- are untouched.
/// They compare the same funding ledgers against the same Pending bytes, and
/// they are right to, because every one of their callers
/// (`execute_projected_custody_bootstrap`, `plan_source_abort_recovery_v1`,
/// `execute_source_abort_v1`, `resume_found_market_from_prepared_checkpoint`)
/// runs while that boundary is still NOW. That is a measurement rather than a
/// belief: this detector computes the reconstruction path's own call graph,
/// and the day either sibling joins it, the detector names it.
///
/// Its one honest limitation: edges are counted between TOP-LEVEL functions,
/// so a path that reached an authenticator only through an inherent method
/// would not be seen. Every authenticator in this file is a free function, and
/// the detector's first assertion is that it still reaches its own subject.
#[cfg(test)]
mod historical_boundary_reads {
    use std::collections::BTreeSet;

    /// The entry point of the only path that runs these authenticators long
    /// after the boundary they describe.
    const RECONSTRUCTION_ROOT_V1: &str = "recover_completed_market_from_checkpoint";

    /// `Rpc`'s live account readers, spelled as a method call. The leading dot
    /// is load-bearing: `BoundaryRpcV1::permanent_account` and
    /// `boundary_account` end in the same word and must not match.
    const LIVE_ACCOUNT_READS_V1: [&str; 4] = [
        ".account(",
        ".required_account(",
        ".finalized_accounts(",
        ".finalized_observed_accounts(",
    ];

    struct FunctionV1 {
        name: String,
        body: String,
    }

    /// The name a top-level `fn` line declares.
    ///
    /// Column zero is the discriminator, so the caller passes raw lines: a
    /// method inside an `impl`, and everything inside a test module, is
    /// indented and is not a top-level function -- which is exactly right,
    /// because `BoundaryRpcV1`'s own methods are the ones that must read
    /// `Rpc`.
    fn top_level_fn_name_v1(line: &str) -> Option<String> {
        let mut rest = line;
        while let Some(shorter) = ["pub(crate) ", "pub ", "const ", "async ", "unsafe "]
            .iter()
            .find_map(|prefix| rest.strip_prefix(prefix))
        {
            rest = shorter;
        }
        let name = rest
            .strip_prefix("fn ")?
            .chars()
            .take_while(|value| value.is_alphanumeric() || *value == '_')
            .collect::<String>();
        (!name.is_empty()).then_some(name)
    }

    /// Every top-level function in a rustfmt-formatted source: one starts at
    /// column zero and ends at the next line that is exactly `}`.
    fn top_level_functions_v1(source: &str) -> Vec<FunctionV1> {
        let lines = source.lines().collect::<Vec<_>>();
        let mut functions = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            let Some(name) = top_level_fn_name_v1(lines[index]) else {
                index += 1;
                continue;
            };
            let mut end = index + 1;
            while end < lines.len() && lines[end] != "}" {
                end += 1;
            }
            functions.push(FunctionV1 {
                name,
                body: lines[index..end.min(lines.len())].join("\n"),
            });
            index = end + 1;
        }
        functions
    }

    /// A textual call edge: the callee's name followed by `(`, preceded by
    /// neither `.` (a method of the same name) nor another identifier
    /// character.
    fn calls_v1(body: &str, callee: &str) -> bool {
        let needle = format!("{callee}(");
        body.match_indices(&needle).any(|(index, _)| {
            let before = body[..index].chars().next_back();
            !before.is_some_and(|value| value == '.' || value == '_' || value.is_alphanumeric())
        })
    }

    fn reachable_v1(functions: &[FunctionV1], root: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![root.to_owned()];
        while let Some(name) = stack.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            let Some(function) = functions.iter().find(|value| value.name == name) else {
                continue;
            };
            for candidate in functions {
                if candidate.name != name && calls_v1(&function.body, &candidate.name) {
                    stack.push(candidate.name.clone());
                }
            }
        }
        seen
    }

    /// A name that CLAIMS to authenticate a past transaction's poststate or
    /// checkpoint. `reconstruct_*` is deliberately not here: it rebuilds
    /// state, it does not assert an invariant about a boundary.
    fn claims_a_past_boundary_v1(name: &str) -> bool {
        name.starts_with("authenticate_")
            && (name.ends_with("_poststate_v1") || name.ends_with("_checkpoint_v1"))
    }

    /// What the detector checked, and what it found.
    fn historical_authenticators_v1(source: &str, root: &str) -> (Vec<String>, Vec<String>) {
        let functions = top_level_functions_v1(source);
        let reachable = reachable_v1(&functions, root);
        let mut checked = Vec::new();
        let mut offenders = Vec::new();
        for function in &functions {
            if !reachable.contains(&function.name) || !claims_a_past_boundary_v1(&function.name) {
                continue;
            }
            checked.push(function.name.clone());
            if LIVE_ACCOUNT_READS_V1
                .iter()
                .any(|read| function.body.contains(read))
            {
                offenders.push(function.name.clone());
            }
        }
        (checked, offenders)
    }

    fn market_source_v1() -> String {
        std::fs::read_to_string(crate::model::successor_src_v1().join("market.rs"))
            .expect("market source")
    }

    #[test]
    fn every_reconstruction_path_authenticator_reads_live_accounts_through_the_boundary_reader() {
        let (checked, offenders) =
            historical_authenticators_v1(&market_source_v1(), RECONSTRUCTION_ROOT_V1);
        // An empty selection and a clean selection log identically, and that
        // difference is the whole value of the test: a detector that lost its
        // subject would report success forever.
        assert!(
            checked.contains(&"authenticate_open_market_poststate_v1".to_owned()),
            "the detector lost its own subject: the reconstruction path must reach \
             authenticate_open_market_poststate_v1, and it reached {checked:?}"
        );
        assert!(
            offenders.is_empty(),
            "these authenticators name a PAST boundary and read live accounts directly: {}. \
             A live read re-evaluates their invariant at whatever time the process runs, which \
             is correct only while the caller IS the boundary. Read through BoundaryRpcV1 and \
             say which reads are permanent and which are boundary-time. Checked: {checked:?}",
            offenders.join(", ")
        );
    }

    #[test]
    fn the_detector_is_red_on_an_authenticator_that_reads_a_live_account_directly() {
        // The Open verifier's funding-ledger loop as it stood before this
        // commit, reduced to the shape that matters, with two controls: a
        // reachable authenticator that reads through the boundary reader, and
        // an offending authenticator the root never reaches. Written with line
        // continuations so no line of THIS file starts a fake top-level `fn`
        // that the real scan above would then read as market.rs's own.
        let fixture = "fn recover_completed_market_from_checkpoint(rpc: &mut Rpc) -> Result<()> {\n\
             \x20   authenticate_open_market_poststate_v1(rpc)?;\n\
             \x20   authenticate_recorded_checkpoint_v1(rpc)\n\
             }\n\
             fn authenticate_open_market_poststate_v1(rpc: &mut Rpc) -> Result<()> {\n\
             \x20   let account = rpc.required_account(ledger.address, \"ledger\")?;\n\
             \x20   Ok(())\n\
             }\n\
             fn authenticate_recorded_checkpoint_v1(rpc: &mut BoundaryRpcV1) -> Result<()> {\n\
             \x20   rpc.permanent_required_account(key, \"label\")?;\n\
             \x20   rpc.boundary_account(key, \"label\", holds, \"refusal\")\n\
             }\n\
             fn authenticate_unreached_poststate_v1(rpc: &mut Rpc) -> Result<()> {\n\
             \x20   rpc.account(key)\n\
             }\n";
        let (checked, offenders) = historical_authenticators_v1(fixture, RECONSTRUCTION_ROOT_V1);
        assert_eq!(
            checked,
            vec![
                "authenticate_open_market_poststate_v1".to_owned(),
                "authenticate_recorded_checkpoint_v1".to_owned(),
            ],
            "the detector checks exactly the named authenticators the root reaches"
        );
        assert_eq!(
            offenders,
            vec!["authenticate_open_market_poststate_v1".to_owned()],
            "the detector must name the direct live read, and neither the boundary-reader \
             control nor the unreachable one"
        );
    }
}
