//! Ordered, crash-safe routing for the exterior terminal sequence.
//!
//! This module deliberately does not own any protocol encoding.  Stage
//! producers pass it observations that their existing semantic owners have
//! already authenticated.  It admits exactly one next mutation and refuses
//! every skipped, substituted, or partially committed lifecycle shape.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
};
use dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2;
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyVaultSeedsV1,
};
use dclutch_direct_codec::{
    native_close_bundle_v1::{
        direct_native_close_account_profile_schema_v1, direct_native_close_effect_schema_v1,
    },
    ordinary_bundle_v4::DirectInlineOrdinaryHotBundleV4,
    retirement_v1::{DirectBeginRetiringRequestV1, direct_begin_retiring_context_v1},
    successor::DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
};
use dclutch_market_core_codec::{
    Action, Admission, Binding, CoreState, Identity, Phase, Readiness, ReleaseReceipt, ReleaseSet,
    Request, Role, begin_retiring,
};
use dclutch_market_retirement_v1_operator::{
    MarketRetirementSnapshotV1, build_market_retirement_v1,
};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    direct_begin_retiring_v1::{
        DirectBeginRetiringCoordinateInputV1, DirectBeginRetiringMetaClassV1,
        DirectBeginRetiringPlanV1, DirectBeginRetiringSnapshotV1,
        derive_direct_begin_retiring_meta_closure_v1, plan_direct_begin_retiring_v1,
    },
    resolution_core_v3::{ResolutionCloseFundSnapshotV3, build_resolution_close_fund_v3},
    terminal_retirement_v1::{
        DirectNativeCloseCoordinateInputV1, DirectNativeCloseSnapshotV1,
        RetirementReplayHandoffCoordinateInputV1, RetirementReplayHandoffSnapshotV1,
        TerminalDeploymentCoordinatesV1, TerminalMetaClassV1, TerminalRecordCoordinatesV1,
        build_direct_native_close_v1, build_retirement_replay_handoff_v1,
        preflight_direct_native_close_caller_v1, preflight_retirement_replay_handoff_caller_v1,
        project_direct_native_close_coordinate_closure_v1,
        project_retirement_replay_handoff_coordinate_closure_v1,
    },
};
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_registry_svm::continuation_v1::{
    RegistryContinuationAdmissionSeedsV1, RegistryContinuationRequestV1,
};
use dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::{
    LifecycleAccountIdV2, LifecycleRentCoreCloseAuthoritySeedsV2, LifecycleRentCreditV2,
};
use dclutch_resolution_codec::{
    SOURCE_CLOSURE_RECEIPT_BYTES_V3, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3, SourceClosureReceiptV3,
};
use dclutch_source_contract::{
    RECOVERY_POLICY_SCHEMA_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceResolutionPhaseV1, SourceResolutionStateV2,
};
use dclutch_versioned_message_operator::{
    EXTEND_ADDRESSES_PER_TRANSACTION_V1, LookupTableCreationPlanV1, PACKET_DATA_BYTES,
    VersionedMessagePlanV0, build_lookup_table_creation_v1, build_lookup_table_freeze,
    compile_v0_message_with_optional_tables,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    program as lookup_table_program,
    state::{AddressLookupTable, LOOKUP_TABLE_META_SIZE, LookupTableMeta},
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk::{
    hash::Hash,
    message::VersionedMessage,
    signature::{Keypair, Signature, Signer},
    transaction::VersionedTransaction,
};

use crate::{
    Error, Result, campaign,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH},
    model::{MarketRunInput, SuccessorPlan},
    plan::{hex32, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1},
    runtime::decode_hex,
    terminal_lifecycle::{
        CampaignTerminalEvidenceV1, authenticate_campaign_market_v1, authenticate_plan_source,
        authenticate_zero_claims, decode_routed_market, finalized_snapshot,
        parse_campaign_terminal_evidence_v1, require_direct_retirement_evidence, required_account,
        routed_record,
    },
    wallet_terminal::authenticate_role,
};
use solana_sdk_ids::{system_program, sysvar};

const ALT_ADDRESS_BYTES: usize = 32;
const ALT_GEOMETRY_BLOCKHASH: [u8; 32] = [0x5a; 32];
const TERMINAL_JOURNAL_SCHEMA_V1: &str = "dclutch-devnet-terminal-sequence-journal-v1";
const TERMINAL_SESSION_SCHEMA_V1: &str = "dclutch-devnet-terminal-sequence-session-v1";
const TERMINAL_FINALITY_WAIT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalSequenceSessionV1 {
    schema: String,
    devnet_genesis_hash: String,
    rpc_url: String,
    plan_sha256: String,
    market_input_sha256: String,
    evidence_sha256: String,
    market: String,
    payer: String,
    source_receipt: String,
    receipt_initial_lamports: u64,
    receipt_rent_lamports: u64,
    supplied_lookup_table: bool,
    lookup_table: String,
    lookup_recent_slot: u64,
    lookup_addresses: Vec<String>,
    lookup_addresses_sha256: String,
    session_sha256: String,
}

struct TerminalSequenceArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    market_input: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    payer: Pubkey,
    payer_keypair: PathBuf,
    session: PathBuf,
    journal_dir: PathBuf,
    supplied_lookup_table: Option<Pubkey>,
    execute: bool,
}

/// The six protocol mutations in their sole admissible order.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) enum TerminalStageV1 {
    CoreBeginRetiring,
    DirectBeginRetiring,
    ResolutionCloseFund,
    DirectCloseCapability,
    RetirementReplayHandoff,
    AggregateRetirement,
}

impl TerminalStageV1 {
    pub(crate) const ORDERED: [Self; 6] = [
        Self::CoreBeginRetiring,
        Self::DirectBeginRetiring,
        Self::ResolutionCloseFund,
        Self::DirectCloseCapability,
        Self::RetirementReplayHandoff,
        Self::AggregateRetirement,
    ];

    pub(crate) const fn ordinal(self) -> u8 {
        match self {
            Self::CoreBeginRetiring => 0,
            Self::DirectBeginRetiring => 1,
            Self::ResolutionCloseFund => 2,
            Self::DirectCloseCapability => 3,
            Self::RetirementReplayHandoff => 4,
            Self::AggregateRetirement => 5,
        }
    }
}

/// Durable phase of one exact stage intent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) enum StageJournalPhaseV1 {
    /// The complete unsigned message and prestate are durable.
    Planned,
    /// A signed packet and its locally derived signature are durable.
    SignedNotSubmitted,
    /// Submission returned that exact signature; replay is forbidden.
    Submitted,
    /// The exact transaction and poststate were observed at finalized.
    Finalized,
}

/// Core Market state relevant to terminal routing.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreTerminalStateV1 {
    Terminal,
    Retiring,
    Closed,
}

/// Direct root state relevant to terminal routing.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectTerminalStateV1 {
    Open,
    Retiring,
    Closed,
}

/// Resolution closure state.  A rent prepayment is an operational prerequisite
/// of CloseFund, not a seventh protocol stage.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolutionTerminalStateV1 {
    /// Source and funding ledger are live; the closure receipt coordinate is
    /// vacant and below its exact rent minimum.
    NeedsReceiptPrepayment,
    /// Source and funding ledger are live; the closure receipt coordinate has
    /// exactly the admitted prepaid balance.
    ReadyToClose,
    /// Source and selected ledger are closed and the exact closure receipt is
    /// finalized.
    Closed,
}

/// Custody replay ownership relevant to the handoff and aggregate close.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetirementReplayStateV1 {
    Trading,
    Core,
    Closed,
}

/// Result of routing one fully authenticated finalized graph.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TerminalRouteV1 {
    /// A System transfer must first prepay the exact Resolution receipt rent.
    PrepayResolutionReceipt,
    /// Execute one of the six protocol mutations.
    Execute(TerminalStageV1),
    /// Every terminal resource has closed in the aggregate transaction.
    Complete,
}

/// Next infrastructure mutation while producing the one immutable terminal
/// lookup table.  It is intentionally outside [`TerminalStageV1`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalLookupTableRouteV1 {
    Create(Instruction),
    Extend {
        prefix_len: usize,
        instruction: Instruction,
    },
    Freeze(Instruction),
    Complete,
}

/// Non-finalized account-coordinate closure for one protocol stage. It owns
/// no account bytes, lamports, observation, or claim that the stage can run.
/// A fresh finalized semantic report must later match this frame exactly.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) enum TerminalAddressClassV1 {
    /// Identity invariant across every legal fresh plan and resume. Only this
    /// class is admitted into the dedicated immutable lookup table.
    LookupStable,
    /// Externally signing account that must remain in the static key set.
    InlineSigner,
    /// Executable program identity that must remain in the static key set.
    InlineProgram,
    /// Request-, balance-, fee-, rent-, or predecessor-bound coordinate that
    /// may change across a legal crash/replan and must remain inline.
    InlineRequestBound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalMetaClosureV1 {
    pub(crate) stage: TerminalStageV1,
    pub(crate) program_id: Pubkey,
    pub(crate) program_class: TerminalAddressClassV1,
    pub(crate) accounts: Vec<AccountMeta>,
    pub(crate) classes: Vec<TerminalAddressClassV1>,
}

impl TerminalMetaClosureV1 {
    pub(crate) fn authenticate_instruction(&self, instruction: &Instruction) -> Result<()> {
        if self.program_class != TerminalAddressClassV1::InlineProgram
            || self.classes.len() != self.accounts.len()
            || instruction.program_id != self.program_id
            || instruction.accounts != self.accounts
        {
            return Err(refusal(
                "fresh semantic stage frame differed from its immutable ALT coordinate closure",
            ));
        }
        Ok(())
    }

    pub(crate) fn authenticate_fresh_closure(&self, fresh: &Self) -> Result<()> {
        if self != fresh {
            return Err(refusal(
                "fresh semantic stage keys, metas, or address classes differed from the persisted coordinate closure",
            ));
        }
        Ok(())
    }
}

const fn direct_begin_class(class: DirectBeginRetiringMetaClassV1) -> TerminalAddressClassV1 {
    match class {
        DirectBeginRetiringMetaClassV1::LookupStable => TerminalAddressClassV1::LookupStable,
        DirectBeginRetiringMetaClassV1::InlineSigner => TerminalAddressClassV1::InlineSigner,
        DirectBeginRetiringMetaClassV1::InlineProgram => TerminalAddressClassV1::InlineProgram,
        DirectBeginRetiringMetaClassV1::InlineRequestBound => {
            TerminalAddressClassV1::InlineRequestBound
        }
    }
}

const fn terminal_owner_class(class: TerminalMetaClassV1) -> TerminalAddressClassV1 {
    match class {
        TerminalMetaClassV1::LookupStable => TerminalAddressClassV1::LookupStable,
        TerminalMetaClassV1::InlineSigner => TerminalAddressClassV1::InlineSigner,
        TerminalMetaClassV1::InlineProgram => TerminalAddressClassV1::InlineProgram,
        TerminalMetaClassV1::InlineRequestBound => TerminalAddressClassV1::InlineRequestBound,
    }
}

/// Wrap Direct's semantic coordinate owner; do not reconstruct its 20 metas.
pub(crate) fn direct_begin_retiring_meta_closure_v1(
    input: DirectBeginRetiringCoordinateInputV1,
) -> Result<TerminalMetaClosureV1> {
    let closure = derive_direct_begin_retiring_meta_closure_v1(input)
        .map_err(|error| Error::new(format!("Direct BeginRetiring coordinates: {error:?}")))?;
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::DirectBeginRetiring,
        program_id: closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: closure.accounts.to_vec(),
        classes: closure
            .classes
            .into_iter()
            .map(direct_begin_class)
            .collect(),
    })
}

/// Wrap Core/Trading's production F=2 native-close coordinate owner; do not
/// reconstruct its 38 metas or seven admitted aliases.
pub(crate) fn direct_native_close_meta_closure_v1(
    input: &DirectNativeCloseCoordinateInputV1,
) -> Result<TerminalMetaClosureV1> {
    let closure = project_direct_native_close_coordinate_closure_v1(input)
        .map_err(|error| Error::new(format!("Direct native-close coordinates: {error:?}")))?;
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::DirectCloseCapability,
        program_id: closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: closure.accounts,
        classes: closure
            .classes
            .into_iter()
            .map(terminal_owner_class)
            .collect(),
    })
}

/// Wrap Core/Custody's request-bound replay-handoff coordinate owner; do not
/// reconstruct its 23 metas or infer the payer/caller placement classes.
pub(crate) fn retirement_replay_handoff_meta_closure_v1(
    input: &RetirementReplayHandoffCoordinateInputV1,
) -> Result<TerminalMetaClosureV1> {
    let closure = project_retirement_replay_handoff_coordinate_closure_v1(input)
        .map_err(|error| Error::new(format!("retirement replay coordinates: {error:?}")))?;
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::RetirementReplayHandoff,
        program_id: closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: closure.accounts,
        classes: closure
            .classes
            .into_iter()
            .map(terminal_owner_class)
            .collect(),
    })
}

/// Exact five-role coordinate closure for Core `BeginRetiring`.
pub(crate) fn core_begin_retiring_meta_closure_v1(
    market: Pubkey,
    activation_cache: Pubkey,
    registry_program: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
) -> TerminalMetaClosureV1 {
    TerminalMetaClosureV1 {
        stage: TerminalStageV1::CoreBeginRetiring,
        program_id: core_program,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: vec![
            AccountMeta::new(market, false),
            AccountMeta::new_readonly(activation_cache, false),
            AccountMeta::new_readonly(registry_program, false),
            AccountMeta::new_readonly(core_program, false),
            AccountMeta::new_readonly(core_programdata, false),
        ],
        classes: vec![
            TerminalAddressClassV1::LookupStable,
            TerminalAddressClassV1::LookupStable,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::LookupStable,
        ],
    }
}

/// Immutable identities and the role-request commitment needed to derive
/// Resolution CloseFund's request-bound Core caller PDA.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolutionCloseMetaCoordinatesV1 {
    pub(crate) release_set: [u8; 32],
    pub(crate) role_request_digest: [u8; 32],
    pub(crate) market: Pubkey,
    pub(crate) activation_cache: Pubkey,
    pub(crate) registry_program: Pubkey,
    pub(crate) core_program: Pubkey,
    pub(crate) core_programdata: Pubkey,
    pub(crate) resolution_program: Pubkey,
    pub(crate) resolution_programdata: Pubkey,
    pub(crate) source_material: Pubkey,
    pub(crate) source_material_staging: Pubkey,
    pub(crate) capability_manifest: Pubkey,
    pub(crate) capability_manifest_staging: Pubkey,
    pub(crate) source_state: Pubkey,
    pub(crate) funding_ledger: Pubkey,
    pub(crate) certificate: Pubkey,
    pub(crate) closure_receipt: Pubkey,
    pub(crate) beneficiary: Pubkey,
    pub(crate) clock_sysvar: Pubkey,
    pub(crate) rent_sysvar: Pubkey,
    pub(crate) system_program: Pubkey,
    pub(crate) recovery_policy: Option<(Pubkey, Pubkey)>,
}

/// Exact Resolution CloseFund coordinate closure. The caller authority is
/// derived here from the immutable request commitment, never supplied as a
/// hand-curated address.
pub(crate) fn resolution_close_meta_closure_v1(
    coordinates: &ResolutionCloseMetaCoordinatesV1,
) -> Result<TerminalMetaClosureV1> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        coordinates.release_set,
        coordinates.market.to_bytes(),
        ExecutionRoleV1::Core,
        coordinates.source_state.to_bytes(),
        coordinates.role_request_digest,
    )
    .map_err(|error| Error::new(format!("Resolution close caller seeds: {error:?}")))?;
    let authority = Pubkey::find_program_address(&seeds.as_slices(), &coordinates.core_program).0;
    let mut accounts = vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new(coordinates.market, false),
        AccountMeta::new_readonly(coordinates.activation_cache, false),
        AccountMeta::new_readonly(coordinates.registry_program, false),
        AccountMeta::new_readonly(coordinates.core_program, false),
        AccountMeta::new_readonly(coordinates.core_programdata, false),
        AccountMeta::new_readonly(coordinates.resolution_program, false),
        AccountMeta::new_readonly(coordinates.resolution_programdata, false),
        AccountMeta::new_readonly(coordinates.source_material, false),
        AccountMeta::new_readonly(coordinates.source_material_staging, false),
        AccountMeta::new_readonly(coordinates.capability_manifest, false),
        AccountMeta::new_readonly(coordinates.capability_manifest_staging, false),
        AccountMeta::new(coordinates.source_state, false),
        AccountMeta::new(coordinates.funding_ledger, false),
        AccountMeta::new_readonly(coordinates.certificate, false),
        AccountMeta::new(coordinates.closure_receipt, false),
        AccountMeta::new(coordinates.beneficiary, false),
        AccountMeta::new_readonly(coordinates.clock_sysvar, false),
        AccountMeta::new_readonly(coordinates.rent_sysvar, false),
        AccountMeta::new_readonly(coordinates.system_program, false),
    ];
    if let Some((raw, staging)) = coordinates.recovery_policy {
        accounts.push(AccountMeta::new_readonly(raw, false));
        accounts.push(AccountMeta::new_readonly(staging, false));
    }
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        program_id: coordinates.core_program,
        program_class: TerminalAddressClassV1::InlineProgram,
        classes: resolution_close_meta_classes_v1(coordinates.recovery_policy.is_some()),
        accounts,
    })
}

fn resolution_close_meta_classes_v1(has_recovery_policy: bool) -> Vec<TerminalAddressClassV1> {
    let mut classes = vec![
        TerminalAddressClassV1::InlineRequestBound,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::InlineProgram,
        TerminalAddressClassV1::InlineProgram,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::InlineProgram,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::InlineProgram,
    ];
    if has_recovery_policy {
        classes.extend([
            TerminalAddressClassV1::LookupStable,
            TerminalAddressClassV1::LookupStable,
        ]);
    }
    classes
}

/// Immutable account identities and typed request commitments for the final
/// Registry-wrapped aggregate-retirement frame. No projected account bytes or
/// balances are accepted here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AggregateRetirementMetaCoordinatesV1 {
    pub(crate) release_set: [u8; 32],
    pub(crate) parent_request_digest: [u8; 32],
    pub(crate) claims_request_body: Vec<u8>,
    pub(crate) custody_context: [u8; 32],
    pub(crate) close_vault_request_body: Vec<u8>,
    pub(crate) close_replay_request_body: Vec<u8>,
    pub(crate) rent_post_resource_digest: [u8; 32],
    pub(crate) continuation: RegistryContinuationRequestV1,
    pub(crate) market: Pubkey,
    pub(crate) rent_credit: Pubkey,
    pub(crate) activation_cache: Pubkey,
    pub(crate) registry_program: Pubkey,
    pub(crate) core_program: Pubkey,
    pub(crate) core_programdata: Pubkey,
    pub(crate) claims_program: Pubkey,
    pub(crate) claims_programdata: Pubkey,
    pub(crate) resolution_program: Pubkey,
    pub(crate) resolution_programdata: Pubkey,
    pub(crate) custody_program: Pubkey,
    pub(crate) custody_programdata: Pubkey,
    pub(crate) rent_program: Pubkey,
    pub(crate) source_receipt: Pubkey,
    pub(crate) claims_aggregate: Pubkey,
    pub(crate) custody_replay: Pubkey,
    pub(crate) hoard_vault: Pubkey,
    pub(crate) custody_authority: Pubkey,
    pub(crate) collateral_mint: Pubkey,
    pub(crate) collateral_token_program: Pubkey,
    pub(crate) realm_raw: Pubkey,
    pub(crate) realm_staging: Pubkey,
    pub(crate) infrastructure_profile: Pubkey,
    pub(crate) registry_artifact_raw: Pubkey,
    pub(crate) registry_artifact_staging: Pubkey,
    pub(crate) registry_programdata: Pubkey,
    pub(crate) rent_artifact_raw: Pubkey,
    pub(crate) rent_artifact_staging: Pubkey,
    pub(crate) rent_programdata: Pubkey,
    pub(crate) rent_sysvar: Pubkey,
    pub(crate) refund_wallet: Pubkey,
}

/// Exact 46-meta aggregate-retirement coordinate closure. The four request-
/// bound Core PDAs and invocation-scoped Registry admission are derived from
/// typed commitments rather than accepted as caller-selected table entries.
pub(crate) fn aggregate_retirement_meta_closure_v1(
    coordinates: &AggregateRetirementMetaCoordinatesV1,
) -> Result<TerminalMetaClosureV1> {
    let claims_authority = aggregate_caller_authority(
        coordinates.release_set,
        coordinates.market,
        coordinates.parent_request_digest,
        &coordinates.claims_request_body,
        coordinates.core_program,
    )?;
    let close_vault_authority = aggregate_caller_authority(
        coordinates.release_set,
        coordinates.market,
        coordinates.custody_context,
        &coordinates.close_vault_request_body,
        coordinates.core_program,
    )?;
    let close_replay_authority = aggregate_caller_authority(
        coordinates.release_set,
        coordinates.market,
        coordinates.custody_context,
        &coordinates.close_replay_request_body,
        coordinates.core_program,
    )?;
    let rent_seeds = LifecycleRentCoreCloseAuthoritySeedsV2::new(
        LifecycleAccountIdV2::new(coordinates.rent_credit.to_bytes())
            .map_err(|error| Error::new(format!("RentCredit identity: {error:?}")))?,
        coordinates.rent_post_resource_digest,
    )
    .map_err(|error| Error::new(format!("rent close authority seeds: {error:?}")))?;
    let credit = rent_seeds.credit().to_bytes();
    let post = rent_seeds.post_resource_digest();
    let rent_close_authority = Pubkey::find_program_address(
        &[rent_seeds.domain(), &credit, &post],
        &coordinates.core_program,
    )
    .0;
    let role_batch = coordinates
        .continuation
        .role_batch_request()
        .map_err(|error| Error::new(format!("retirement role batch: {error:?}")))?;
    let role_batch_digest = ContentId::new(hash(&role_batch.to_bytes()).to_bytes())
        .map_err(|error| Error::new(format!("retirement role-batch digest: {error:?}")))?;
    let admission_seeds = RegistryContinuationAdmissionSeedsV1::new(
        coordinates.continuation,
        coordinates.activation_cache.to_bytes(),
        role_batch_digest,
    )
    .map_err(|error| Error::new(format!("retirement continuation seeds: {error:?}")))?;
    let release = admission_seeds.release_set();
    let cache = admission_seeds.activation_cache();
    let role_batch = admission_seeds.batch_request_digest();
    let role_mask = admission_seeds.role_mask();
    let role = admission_seeds.continuation_role();
    let continuation_digest = admission_seeds.continuation_digest();
    let admission = Pubkey::find_program_address(
        &[
            admission_seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            role_batch.as_slice(),
            role_mask.as_slice(),
            role.as_slice(),
            continuation_digest.as_slice(),
        ],
        &coordinates.registry_program,
    )
    .0;
    let core = vec![
        AccountMeta::new(coordinates.market, false),
        AccountMeta::new(coordinates.rent_credit, false),
        AccountMeta::new_readonly(coordinates.activation_cache, false),
        AccountMeta::new_readonly(coordinates.registry_program, false),
        AccountMeta::new_readonly(coordinates.core_program, false),
        AccountMeta::new_readonly(coordinates.core_programdata, false),
        AccountMeta::new_readonly(coordinates.claims_program, false),
        AccountMeta::new_readonly(coordinates.claims_programdata, false),
        AccountMeta::new_readonly(coordinates.resolution_program, false),
        AccountMeta::new_readonly(coordinates.resolution_programdata, false),
        AccountMeta::new_readonly(coordinates.custody_program, false),
        AccountMeta::new_readonly(coordinates.custody_programdata, false),
        AccountMeta::new_readonly(coordinates.rent_program, false),
        AccountMeta::new_readonly(coordinates.source_receipt, false),
        AccountMeta::new(coordinates.claims_aggregate, false),
        AccountMeta::new(coordinates.custody_replay, false),
        AccountMeta::new(coordinates.hoard_vault, false),
        AccountMeta::new_readonly(coordinates.custody_authority, false),
        AccountMeta::new_readonly(coordinates.collateral_mint, false),
        AccountMeta::new_readonly(coordinates.collateral_token_program, false),
        AccountMeta::new_readonly(coordinates.realm_raw, false),
        AccountMeta::new_readonly(coordinates.realm_staging, false),
        AccountMeta::new_readonly(claims_authority, false),
        AccountMeta::new_readonly(close_vault_authority, false),
        AccountMeta::new_readonly(close_replay_authority, false),
        AccountMeta::new_readonly(coordinates.infrastructure_profile, false),
        AccountMeta::new_readonly(coordinates.registry_artifact_raw, false),
        AccountMeta::new_readonly(coordinates.registry_artifact_staging, false),
        AccountMeta::new_readonly(coordinates.registry_programdata, false),
        AccountMeta::new_readonly(coordinates.rent_artifact_raw, false),
        AccountMeta::new_readonly(coordinates.rent_artifact_staging, false),
        AccountMeta::new_readonly(coordinates.rent_programdata, false),
        AccountMeta::new_readonly(coordinates.rent_sysvar, false),
        AccountMeta::new(coordinates.refund_wallet, false),
        AccountMeta::new_readonly(rent_close_authority, false),
    ];
    if core.iter().any(|meta| meta.pubkey == admission) {
        return Err(refusal(
            "aggregate Registry admission aliased a Core retirement role",
        ));
    }
    let mut accounts = vec![
        AccountMeta::new_readonly(coordinates.activation_cache, false),
        AccountMeta::new_readonly(coordinates.core_program, false),
        AccountMeta::new_readonly(coordinates.core_programdata, false),
        AccountMeta::new_readonly(coordinates.claims_program, false),
        AccountMeta::new_readonly(coordinates.claims_programdata, false),
        AccountMeta::new_readonly(coordinates.resolution_program, false),
        AccountMeta::new_readonly(coordinates.resolution_programdata, false),
        AccountMeta::new_readonly(coordinates.custody_program, false),
        AccountMeta::new_readonly(coordinates.custody_programdata, false),
        AccountMeta::new_readonly(admission, false),
    ];
    accounts.extend(core);
    accounts.push(AccountMeta::new_readonly(admission, false));
    if accounts.len() != 46 {
        return Err(refusal(
            "aggregate retirement coordinate closure has another width than 46",
        ));
    }
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::AggregateRetirement,
        program_id: coordinates.registry_program,
        program_class: TerminalAddressClassV1::InlineProgram,
        classes: aggregate_retirement_meta_classes_v1(),
        accounts,
    })
}

fn aggregate_retirement_meta_classes_v1() -> Vec<TerminalAddressClassV1> {
    let mut classes = vec![TerminalAddressClassV1::LookupStable; 46];
    for index in [1_usize, 3, 5, 7, 13, 14, 16, 18, 20, 22, 29] {
        classes[index] = TerminalAddressClassV1::InlineProgram;
    }
    for index in [9_usize, 32, 33, 34, 44, 45] {
        classes[index] = TerminalAddressClassV1::InlineRequestBound;
    }
    classes
}

fn aggregate_caller_authority(
    release_set: [u8; 32],
    market: Pubkey,
    context: [u8; 32],
    request_body: &[u8],
    core_program: Pubkey,
) -> Result<Pubkey> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        hash(request_body).to_bytes(),
    )
    .map_err(|error| Error::new(format!("aggregate caller seeds: {error:?}")))?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &core_program).0)
}

/// Exact return data that a finalized transaction must carry.  Absence means
/// the semantic owner does not define transaction return data for that stage;
/// it never weakens the exact account-poststate checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExpectedReturnDataV1 {
    pub(crate) producer: Pubkey,
    pub(crate) body: Vec<u8>,
}

/// One account poststate owned by a semantic stage builder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExpectedAccountPoststateV1 {
    pub(crate) key: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data: Vec<u8>,
    pub(crate) data_digest: [u8; 32],
}

impl ExpectedAccountPoststateV1 {
    fn exact(key: Pubkey, owner: Pubkey, lamports: u64, executable: bool, data: Vec<u8>) -> Self {
        let data_digest = hash(&data).to_bytes();
        Self {
            key,
            owner,
            lamports,
            executable,
            data,
            data_digest,
        }
    }
}

/// Caller-facing projection of one existing semantic operator report.
/// Protocol encodings, frames, request PDAs, and expected account bytes stay
/// owned by those reports.  This projection adds only exterior journal facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalSemanticMutationV1 {
    pub(crate) stage: TerminalStageV1,
    pub(crate) observation: Observation,
    pub(crate) instruction: Instruction,
    pub(crate) expected_return_data: Option<ExpectedReturnDataV1>,
    pub(crate) expected_accounts: Vec<ExpectedAccountPoststateV1>,
    /// Exact protocol-only lamport deltas before the transaction fee. The
    /// exterior caller maps these identities onto its payer/refund wallets;
    /// it never assumes those roles are distinct.
    pub(crate) protocol_lamport_deltas: BTreeMap<Pubkey, i128>,
}

/// Durable identity of one mutation. ALT operations are infrastructure
/// preflight and never consume a protocol-stage ordinal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", tag = "kind")]
pub(crate) enum DurableTerminalMutationV1 {
    LookupCreate,
    LookupExtend { prefix_len: usize },
    LookupFreeze,
    ResolutionReceiptPrepay,
    Protocol { stage: TerminalStageV1 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableAccountStateV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_len: usize,
    data_sha256: String,
    account_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableInstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
    class: TerminalAddressClassV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableExpectedAccountV1 {
    address: String,
    owner: String,
    lamports_after_protocol: u64,
    lamports_after_fee: u64,
    executable: bool,
    data_base64: String,
    data_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lookup_table: Option<DurableLookupTablePoststateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableLookupTablePoststateV1 {
    authority: Option<String>,
    addresses: Vec<String>,
    deactivation_slot: u64,
    last_extended_slot: DurableLookupLastExtendedSlotV1,
    last_extended_slot_start_index: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "camelCase",
    tag = "kind",
    content = "slot"
)]
enum DurableLookupLastExtendedSlotV1 {
    Exact(u64),
    FinalizedTransaction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableReturnDataV1 {
    producer: String,
    body_base64: String,
    body_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableTerminalIntentV1 {
    mutation: DurableTerminalMutationV1,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    payer: String,
    program_id: String,
    program_class: TerminalAddressClassV1,
    accounts: Vec<DurableInstructionAccountV1>,
    instruction_data_base64: String,
    instruction_data_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lookup_table: Option<String>,
    lookup_table_addresses: Vec<String>,
    lookup_table_addresses_sha256: String,
    loaded_writable: Vec<String>,
    loaded_readonly: Vec<String>,
    resolved_account_keys: Vec<String>,
    pre_balances: Vec<u64>,
    post_balances: Vec<u64>,
    recent_blockhash: String,
    last_valid_block_height: u64,
    transaction_fee_lamports: u64,
    wire_bytes: usize,
    message_base64: String,
    message_sha256: String,
    prestate: BTreeMap<String, DurableAccountStateV1>,
    expected_accounts: BTreeMap<String, DurableExpectedAccountV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_return_data: Option<DurableReturnDataV1>,
    protocol_lamport_deltas: BTreeMap<String, i128>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableFinalizedEvidenceV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    packet_sha256: String,
    poststate: BTreeMap<String, DurableAccountStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableTerminalJournalV1 {
    schema: String,
    cluster: String,
    rpc_url: String,
    authorized_mutation: bool,
    state_sha256: String,
    phase: StageJournalPhaseV1,
    intent_sha256: String,
    intent: DurableTerminalIntentV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalized: Option<DurableFinalizedEvidenceV1>,
}

/// In-memory proof that the exact Planned intent was reconstructed by the
/// stage's semantic owner from a fresh finalized snapshot. It is deliberately
/// not serializable and cannot be produced by editing or rehashing a journal.
#[derive(Debug)]
pub(crate) struct AuthenticatedPlannedTerminalIntentV1 {
    intent_sha256: String,
}

impl AuthenticatedPlannedTerminalIntentV1 {
    fn authenticate(&self, journal: &DurableTerminalJournalV1) -> Result<()> {
        if journal.phase != StageJournalPhaseV1::Planned
            || self.intent_sha256 != journal.intent_sha256
        {
            return Err(refusal(
                "planned terminal semantic-owner authorization did not bind this exact durable intent",
            ));
        }
        Ok(())
    }
}

/// Construct the exact durable protocol-stage intent. Callers must atomically
/// persist the returned journal before invoking [`resume_terminal_journal_v1`]
/// with execute authorization. This function performs reads only.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_protocol_stage_journal_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    payer: Pubkey,
    mutation: &TerminalSemanticMutationV1,
    fresh_closure: &TerminalMetaClosureV1,
    frozen_table: &ObservedAccount,
    frozen_addresses: &[Pubkey],
    rent: &Rent,
    prestate: &[ObservedAccount],
    authorized_mutation: bool,
) -> Result<DurableTerminalJournalV1> {
    if origin.label() != "devnet" || mutation.observation.finality != Finality::Finalized {
        return Err(refusal(
            "terminal execution only admits one exact finalized devnet observation",
        ));
    }
    if checked_terminal_delta_sum_v1(mutation.protocol_lamport_deltas.values().copied())? != 0 {
        return Err(refusal(
            "protocol-stage lamport deltas did not conserve exactly before transaction fee",
        ));
    }
    fresh_closure.authenticate_instruction(&mutation.instruction)?;
    if mutation.stage != fresh_closure.stage
        || frozen_table.observation != mutation.observation
        || prestate
            .iter()
            .any(|account| account.observation != mutation.observation)
    {
        return Err(refusal(
            "terminal intent mixed stages or finalized observations",
        ));
    }
    let mut prestate_by_key = BTreeMap::new();
    for account in prestate {
        if prestate_by_key.insert(account.key, account).is_some() {
            return Err(refusal("terminal prestate contained a duplicate account"));
        }
    }
    for key in std::iter::once(payer)
        .chain(mutation.instruction.accounts.iter().map(|meta| meta.pubkey))
        .chain(std::iter::once(frozen_table.key))
    {
        if !prestate_by_key.contains_key(&key) {
            return Err(refusal(
                "terminal prestate omitted a payer, instruction, or lookup-table account",
            ));
        }
    }
    authenticate_supplied_terminal_lookup_table_v1(frozen_addresses, frozen_table, rent).map_err(
        |_| refusal("terminal stage lookup table was not the exact activated frozen stable union"),
    )?;
    let (recent_blockhash, last_valid_block_height) = terminal_latest_blockhash(rpc)?;
    let compiled = compile_v0_message_with_optional_tables(
        payer,
        std::slice::from_ref(&mutation.instruction),
        recent_blockhash,
        mutation.observation,
        std::slice::from_ref(frozen_table),
    )
    .map_err(|error| Error::new(format!("terminal v0 message: {error:?}")))?;
    authenticate_terminal_v0_placement_v1(
        payer,
        fresh_closure,
        frozen_table.key,
        frozen_addresses,
        &compiled,
    )?;
    let (loaded_writable, loaded_readonly, resolved_account_keys) =
        resolved_terminal_v0_keys_v1(&compiled, frozen_table.key, frozen_addresses)?;
    let pre_balances = resolved_account_keys
        .iter()
        .map(|key| {
            prestate_by_key
                .get(key)
                .map(|account| account.lamports)
                .ok_or_else(|| refusal("terminal prestate omitted one resolved v0 account"))
        })
        .collect::<Result<Vec<_>>>()?;
    let message_bytes = compiled.message.serialize();
    let message_base64 = BASE64.encode(&message_bytes);
    let transaction_fee_lamports = terminal_fee_for_message(rpc, &message_base64)?;
    let payer_pre = prestate_by_key
        .get(&payer)
        .ok_or_else(|| refusal("terminal prestate omitted fee payer"))?;
    let payer_protocol_delta = mutation
        .protocol_lamport_deltas
        .get(&payer)
        .copied()
        .unwrap_or(0);
    let payer_after_protocol =
        checked_apply_lamport_delta(payer_pre.lamports, payer_protocol_delta)?;
    let payer_after_fee = payer_after_protocol
        .checked_sub(transaction_fee_lamports)
        .ok_or_else(|| refusal("terminal payer cannot cover exact protocol delta plus fee"))?;

    let mut expected_accounts = BTreeMap::new();
    for account in &mutation.expected_accounts {
        if hash(&account.data).to_bytes() != account.data_digest {
            return Err(refusal(
                "semantic expected account digest disagreed with its exact bytes",
            ));
        }
        let pre = prestate_by_key
            .get(&account.key)
            .ok_or_else(|| refusal("semantic expected account omitted from exact prestate"))?;
        let observed_delta = i128::from(account.lamports) - i128::from(pre.lamports);
        if mutation
            .protocol_lamport_deltas
            .get(&account.key)
            .copied()
            .unwrap_or(0)
            != observed_delta
        {
            return Err(refusal(
                "semantic protocol lamport delta disagreed with exact pre/post balance",
            ));
        }
        let lamports_after_fee = if account.key == payer {
            account
                .lamports
                .checked_sub(transaction_fee_lamports)
                .ok_or_else(|| refusal("terminal expected payer fee debit underflowed"))?
        } else {
            account.lamports
        };
        if expected_accounts
            .insert(
                account.key.to_string(),
                DurableExpectedAccountV1 {
                    address: account.key.to_string(),
                    owner: account.owner.to_string(),
                    lamports_after_protocol: account.lamports,
                    lamports_after_fee,
                    executable: account.executable,
                    data_base64: BASE64.encode(&account.data),
                    data_sha256: sha256_hex(&account.data),
                    lookup_table: None,
                },
            )
            .is_some()
        {
            return Err(refusal("semantic expected accounts contained a duplicate"));
        }
    }
    expected_accounts
        .entry(payer.to_string())
        .or_insert_with(|| DurableExpectedAccountV1 {
            address: payer.to_string(),
            owner: payer_pre.owner.to_string(),
            lamports_after_protocol: payer_after_protocol,
            lamports_after_fee: payer_after_fee,
            executable: payer_pre.executable,
            data_base64: BASE64.encode(&payer_pre.data),
            data_sha256: sha256_hex(&payer_pre.data),
            lookup_table: None,
        });
    if expected_accounts
        .get(&payer.to_string())
        .is_none_or(|expected| expected.lamports_after_fee != payer_after_fee)
    {
        return Err(refusal(
            "semantic payer poststate disagreed with protocol delta and exact fee",
        ));
    }
    let writable = mutation
        .instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .chain(std::iter::once(payer))
        .collect::<BTreeSet<_>>();
    if writable
        .iter()
        .any(|key| !expected_accounts.contains_key(&key.to_string()))
        || mutation
            .protocol_lamport_deltas
            .keys()
            .any(|key| !writable.contains(key))
    {
        return Err(refusal(
            "terminal semantic report omitted a writable poststate or named a readonly lamport delta",
        ));
    }
    let post_balances = resolved_account_keys
        .iter()
        .zip(pre_balances.iter().copied())
        .map(|(key, pre)| {
            let protocol = mutation
                .protocol_lamport_deltas
                .get(key)
                .copied()
                .unwrap_or(0);
            let after_protocol = checked_apply_lamport_delta(pre, protocol)?;
            if *key == payer {
                after_protocol
                    .checked_sub(transaction_fee_lamports)
                    .ok_or_else(|| refusal("terminal post-balance fee debit underflowed"))
            } else {
                Ok(after_protocol)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let intent = DurableTerminalIntentV1 {
        mutation: DurableTerminalMutationV1::Protocol {
            stage: mutation.stage,
        },
        observation_slot: mutation.observation.slot,
        observation_unix_timestamp: mutation.observation.unix_timestamp,
        payer: payer.to_string(),
        program_id: fresh_closure.program_id.to_string(),
        program_class: fresh_closure.program_class,
        accounts: fresh_closure
            .accounts
            .iter()
            .zip(fresh_closure.classes.iter().copied())
            .map(|(meta, class)| DurableInstructionAccountV1 {
                address: meta.pubkey.to_string(),
                signer: meta.is_signer,
                writable: meta.is_writable,
                class,
            })
            .collect(),
        instruction_data_base64: BASE64.encode(&mutation.instruction.data),
        instruction_data_sha256: sha256_hex(&mutation.instruction.data),
        lookup_table: Some(frozen_table.key.to_string()),
        lookup_table_addresses: frozen_addresses.iter().map(ToString::to_string).collect(),
        lookup_table_addresses_sha256: pubkey_vector_sha256(frozen_addresses),
        loaded_writable: loaded_writable.iter().map(ToString::to_string).collect(),
        loaded_readonly: loaded_readonly.iter().map(ToString::to_string).collect(),
        resolved_account_keys: resolved_account_keys
            .iter()
            .map(ToString::to_string)
            .collect(),
        pre_balances,
        post_balances,
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        transaction_fee_lamports,
        wire_bytes: compiled.wire_bytes,
        message_base64,
        message_sha256: sha256_hex(&message_bytes),
        prestate: prestate_by_key
            .into_iter()
            .map(|(key, account)| (key.to_string(), durable_observed_state(account)))
            .collect(),
        expected_accounts,
        expected_return_data: mutation.expected_return_data.as_ref().map(|expected| {
            DurableReturnDataV1 {
                producer: expected.producer.to_string(),
                body_base64: BASE64.encode(&expected.body),
                body_sha256: sha256_hex(&expected.body),
            }
        }),
        protocol_lamport_deltas: mutation
            .protocol_lamport_deltas
            .iter()
            .map(|(key, delta)| (key.to_string(), *delta))
            .collect(),
    };
    let intent_sha256 = sha256_hex(&serde_json::to_vec(&intent)?);
    let mut journal = DurableTerminalJournalV1 {
        schema: TERMINAL_JOURNAL_SCHEMA_V1.into(),
        cluster: "devnet".into(),
        rpc_url: origin.redacted_url(),
        authorized_mutation,
        state_sha256: String::new(),
        phase: StageJournalPhaseV1::Planned,
        intent_sha256,
        intent,
        signed_packet_base64: None,
        expected_signature: None,
        finalized: None,
    };
    refresh_terminal_journal_digest_v1(&mut journal)?;
    Ok(journal)
}

/// Build the Resolution receipt-rent top-up through the identical durable
/// message/signature/finalizer machinery. It is explicitly an operational
/// prerequisite, not a seventh protocol stage.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_resolution_receipt_prepay_journal_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    payer: &ObservedAccount,
    receipt: &ObservedAccount,
    exact_receipt_rent: u64,
    frozen_table: &ObservedAccount,
    frozen_addresses: &[Pubkey],
    rent: &Rent,
    prestate: &[ObservedAccount],
    authorized_mutation: bool,
) -> Result<DurableTerminalJournalV1> {
    if payer.observation != receipt.observation
        || payer.observation != frozen_table.observation
        || receipt.owner != solana_sdk_ids::system_program::ID
        || receipt.executable
        || !receipt.data.is_empty()
        || receipt.lamports >= exact_receipt_rent
    {
        return Err(refusal(
            "Resolution receipt prepay requires one finalized vacant below-rent destination",
        ));
    }
    let top_up = exact_receipt_rent
        .checked_sub(receipt.lamports)
        .ok_or_else(|| refusal("Resolution receipt top-up underflowed"))?;
    let payer_after = payer
        .lamports
        .checked_sub(top_up)
        .ok_or_else(|| refusal("Resolution receipt payer cannot cover exact rent top-up"))?;
    let instruction =
        solana_system_interface::instruction::transfer(&payer.key, &receipt.key, top_up);
    let closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        program_id: solana_sdk_ids::system_program::ID,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: instruction.accounts.clone(),
        classes: vec![
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::LookupStable,
        ],
    };
    let mutation = TerminalSemanticMutationV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        observation: payer.observation,
        instruction,
        expected_return_data: None,
        expected_accounts: vec![
            ExpectedAccountPoststateV1::exact(
                payer.key,
                payer.owner,
                payer_after,
                payer.executable,
                payer.data.clone(),
            ),
            ExpectedAccountPoststateV1::exact(
                receipt.key,
                receipt.owner,
                exact_receipt_rent,
                receipt.executable,
                receipt.data.clone(),
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (payer.key, -i128::from(top_up)),
            (receipt.key, i128::from(top_up)),
        ]),
    };
    let mut journal = build_protocol_stage_journal_v1(
        rpc,
        origin,
        payer.key,
        &mutation,
        &closure,
        frozen_table,
        frozen_addresses,
        rent,
        prestate,
        authorized_mutation,
    )?;
    journal.intent.mutation = DurableTerminalMutationV1::ResolutionReceiptPrepay;
    journal.intent_sha256 = sha256_hex(&serde_json::to_vec(&journal.intent)?);
    refresh_terminal_journal_digest_v1(&mut journal)?;
    Ok(journal)
}

pub(crate) fn authenticate_resolution_receipt_prepay_planned_journal_v1(
    journal: &DurableTerminalJournalV1,
    payer: &ObservedAccount,
    receipt: &ObservedAccount,
    exact_receipt_rent: u64,
    exact_execution_prestate: &[ObservedAccount],
) -> Result<AuthenticatedPlannedTerminalIntentV1> {
    if payer.observation != receipt.observation
        || receipt.owner != solana_sdk_ids::system_program::ID
        || receipt.executable
        || !receipt.data.is_empty()
        || receipt.lamports >= exact_receipt_rent
    {
        return Err(refusal(
            "Resolution receipt semantic-owner authorization requires one finalized vacant below-rent destination",
        ));
    }
    let top_up = exact_receipt_rent
        .checked_sub(receipt.lamports)
        .ok_or_else(|| refusal("Resolution receipt authorization top-up underflowed"))?;
    let payer_after = payer
        .lamports
        .checked_sub(top_up)
        .ok_or_else(|| refusal("Resolution receipt authorization payer underflowed"))?;
    let instruction =
        solana_system_interface::instruction::transfer(&payer.key, &receipt.key, top_up);
    let closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        program_id: solana_sdk_ids::system_program::ID,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: instruction.accounts.clone(),
        classes: vec![
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::LookupStable,
        ],
    };
    let mutation = TerminalSemanticMutationV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        observation: payer.observation,
        instruction,
        expected_return_data: None,
        expected_accounts: vec![
            ExpectedAccountPoststateV1::exact(
                payer.key,
                payer.owner,
                payer_after,
                payer.executable,
                payer.data.clone(),
            ),
            ExpectedAccountPoststateV1::exact(
                receipt.key,
                receipt.owner,
                exact_receipt_rent,
                receipt.executable,
                receipt.data.clone(),
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (payer.key, -i128::from(top_up)),
            (receipt.key, i128::from(top_up)),
        ]),
    };
    authenticate_planned_protocol_owner_v1(
        journal,
        DurableTerminalMutationV1::ResolutionReceiptPrepay,
        &mutation,
        &closure,
        exact_execution_prestate,
    )
}

fn checked_apply_lamport_delta(pre: u64, delta: i128) -> Result<u64> {
    if delta >= 0 {
        pre.checked_add(
            u64::try_from(delta).map_err(|_| refusal("positive lamport delta exceeded u64"))?,
        )
        .ok_or_else(|| refusal("positive lamport delta overflowed"))
    } else {
        pre.checked_sub(
            u64::try_from(-delta).map_err(|_| refusal("negative lamport delta exceeded u64"))?,
        )
        .ok_or_else(|| refusal("negative lamport delta underflowed"))
    }
}

fn checked_terminal_delta_sum_v1(deltas: impl IntoIterator<Item = i128>) -> Result<i128> {
    deltas.into_iter().try_fold(0_i128, |sum, delta| {
        sum.checked_add(delta)
            .ok_or_else(|| refusal("terminal lamport delta sum overflowed i128"))
    })
}

fn authenticate_planned_protocol_owner_v1(
    journal: &DurableTerminalJournalV1,
    durable_mutation: DurableTerminalMutationV1,
    mutation: &TerminalSemanticMutationV1,
    closure: &TerminalMetaClosureV1,
    exact_prestate: &[ObservedAccount],
) -> Result<AuthenticatedPlannedTerminalIntentV1> {
    authenticate_terminal_journal_v1(journal)?;
    if journal.phase != StageJournalPhaseV1::Planned
        || journal.intent.mutation != durable_mutation
        || mutation.observation.slot < journal.intent.observation_slot
        || mutation.observation.finality != Finality::Finalized
    {
        return Err(refusal(
            "planned protocol journal did not bind the semantic owner's stage and finalized observation",
        ));
    }
    closure.authenticate_instruction(&mutation.instruction)?;
    let instruction_accounts = closure
        .accounts
        .iter()
        .zip(closure.classes.iter().copied())
        .map(|(meta, class)| DurableInstructionAccountV1 {
            address: meta.pubkey.to_string(),
            signer: meta.is_signer,
            writable: meta.is_writable,
            class,
        })
        .collect::<Vec<_>>();
    if journal.intent.program_id != closure.program_id.to_string()
        || journal.intent.program_class != closure.program_class
        || journal.intent.accounts != instruction_accounts
        || journal.intent.instruction_data_base64 != BASE64.encode(&mutation.instruction.data)
        || journal.intent.instruction_data_sha256 != sha256_hex(&mutation.instruction.data)
    {
        return Err(refusal(
            "planned protocol message differed from the freshly rerun semantic owner's exact instruction or closure",
        ));
    }
    let expected_return = mutation
        .expected_return_data
        .as_ref()
        .map(|value| DurableReturnDataV1 {
            producer: value.producer.to_string(),
            body_base64: BASE64.encode(&value.body),
            body_sha256: sha256_hex(&value.body),
        });
    if journal.intent.expected_return_data != expected_return {
        return Err(refusal(
            "planned protocol return-data contract differed from the semantic owner",
        ));
    }
    let mut observed_prestate = BTreeMap::new();
    for account in exact_prestate {
        if account.observation != mutation.observation
            || observed_prestate
                .insert(account.key.to_string(), durable_observed_state(account))
                .is_some()
        {
            return Err(refusal(
                "semantic-owner authorization mixed observations or duplicate prestate accounts",
            ));
        }
    }
    if observed_prestate != journal.intent.prestate {
        return Err(refusal(
            "planned protocol prestate differed from the exact finalized semantic-owner snapshot",
        ));
    }
    let payer = Pubkey::from_str(&journal.intent.payer)
        .map_err(|error| Error::new(format!("terminal semantic payer: {error}")))?;
    let pre_by_key = exact_prestate
        .iter()
        .map(|account| (account.key, account))
        .collect::<BTreeMap<_, _>>();
    let mut expected_accounts = BTreeMap::new();
    for expected in &mutation.expected_accounts {
        if hash(&expected.data).to_bytes() != expected.data_digest {
            return Err(refusal(
                "semantic owner returned expected bytes with another digest",
            ));
        }
        let lamports_after_fee = if expected.key == payer {
            expected
                .lamports
                .checked_sub(journal.intent.transaction_fee_lamports)
                .ok_or_else(|| refusal("semantic owner payer poststate fee underflowed"))?
        } else {
            expected.lamports
        };
        let durable = DurableExpectedAccountV1 {
            address: expected.key.to_string(),
            owner: expected.owner.to_string(),
            lamports_after_protocol: expected.lamports,
            lamports_after_fee,
            executable: expected.executable,
            data_base64: BASE64.encode(&expected.data),
            data_sha256: sha256_hex(&expected.data),
            lookup_table: None,
        };
        if expected_accounts
            .insert(expected.key.to_string(), durable)
            .is_some()
        {
            return Err(refusal(
                "semantic owner returned duplicate expected poststate accounts",
            ));
        }
    }
    if let std::collections::btree_map::Entry::Vacant(entry) =
        expected_accounts.entry(payer.to_string())
    {
        let payer_pre = pre_by_key
            .get(&payer)
            .ok_or_else(|| refusal("semantic-owner snapshot omitted fee payer"))?;
        let payer_delta = mutation
            .protocol_lamport_deltas
            .get(&payer)
            .copied()
            .unwrap_or(0);
        let after_protocol = checked_apply_lamport_delta(payer_pre.lamports, payer_delta)?;
        let after_fee = after_protocol
            .checked_sub(journal.intent.transaction_fee_lamports)
            .ok_or_else(|| refusal("semantic owner payer could not cover exact fee"))?;
        entry.insert(DurableExpectedAccountV1 {
            address: payer.to_string(),
            owner: payer_pre.owner.to_string(),
            lamports_after_protocol: after_protocol,
            lamports_after_fee: after_fee,
            executable: payer_pre.executable,
            data_base64: BASE64.encode(&payer_pre.data),
            data_sha256: sha256_hex(&payer_pre.data),
            lookup_table: None,
        });
    }
    let deltas = mutation
        .protocol_lamport_deltas
        .iter()
        .map(|(key, delta)| (key.to_string(), *delta))
        .collect::<BTreeMap<_, _>>();
    if expected_accounts != journal.intent.expected_accounts
        || deltas != journal.intent.protocol_lamport_deltas
    {
        return Err(refusal(
            "planned protocol poststate or lamport arithmetic differed from the freshly rerun semantic owner",
        ));
    }
    Ok(AuthenticatedPlannedTerminalIntentV1 {
        intent_sha256: journal.intent_sha256.clone(),
    })
}

/// Atomically persist a new or updated terminal journal and fsync its parent
/// directory. The caller invokes this before any key file can be opened.
pub(crate) fn write_terminal_journal_v1(
    path: &Path,
    journal: &mut DurableTerminalJournalV1,
    new: bool,
) -> Result<()> {
    if !path.is_absolute() {
        return Err(refusal("terminal journal path must be absolute"));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("terminal journal needs a UTF-8 file name"))?;
    let lock = path.with_file_name(format!(".{name}.terminal-sequence.lock"));
    let lock_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| {
            Error::new(format!(
                "REFUSED: acquire exclusive terminal journal lock {}: {error}; a concurrent or interrupted writer must be reconciled first",
                lock.display()
            ))
        })?;
    lock_file.sync_all()?;
    let persisted = if new {
        if path.exists() {
            let _ = fs::remove_file(&lock);
            return Err(refusal(
                "terminal journal already exists; resume it or choose another absolute path",
            ));
        }
        None
    } else {
        let persisted = read_terminal_journal_v1(path)?;
        authenticate_terminal_journal_v1(&persisted)?;
        if journal.state_sha256 != persisted.state_sha256 {
            let _ = fs::remove_file(&lock);
            return Err(refusal(
                "terminal journal update was based on a stale persisted state",
            ));
        }
        Some(persisted)
    };
    let _ = persisted;
    refresh_terminal_journal_digest_v1(journal)?;
    authenticate_terminal_journal_v1(journal)?;
    let temporary = path.with_file_name(format!(
        ".{name}.terminal-sequence-{}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| Error::new(format!("create {}: {error}", temporary.display())))?;
    file.write_all(&serde_json::to_vec_pretty(journal)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    if new {
        fs::hard_link(&temporary, path).map_err(|error| {
            Error::new(format!(
                "atomically publish new terminal journal {} without clobber: {error}",
                path.display()
            ))
        })?;
        fs::remove_file(&temporary)?;
    } else {
        fs::rename(&temporary, path).map_err(|error| {
            Error::new(format!(
                "atomically replace {} from {}: {error}",
                path.display(),
                temporary.display()
            ))
        })?;
    }
    if let Some(parent) = path.parent() {
        OpenOptions::new()
            .read(true)
            .open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| Error::new(format!("fsync terminal journal directory: {error}")))?;
    }
    fs::remove_file(&lock)?;
    Ok(())
}

pub(crate) fn read_terminal_journal_v1(path: &Path) -> Result<DurableTerminalJournalV1> {
    let source = fs::read(path)?;
    let _: UniqueJsonV1 = serde_json::from_slice(&source).map_err(|error| {
        Error::new(format!(
            "terminal journal JSON contains a duplicate key or malformed value: {error}"
        ))
    })?;
    let journal: DurableTerminalJournalV1 = serde_json::from_slice(&source)?;
    authenticate_terminal_journal_v1(&journal)?;
    Ok(journal)
}

struct UniqueJsonV1;

impl<'de> Deserialize<'de> for UniqueJsonV1 {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitorV1)
    }
}

struct UniqueJsonVisitorV1;

impl<'de> Visitor<'de> for UniqueJsonVisitorV1 {
    type Value = UniqueJsonV1;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON with duplicate-free object keys")
    }

    fn visit_map<A>(self, mut map: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate object key {key}"
                )));
            }
            let _: UniqueJsonV1 = map.next_value()?;
        }
        Ok(UniqueJsonV1)
    }

    fn visit_seq<A>(self, mut sequence: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<UniqueJsonV1>()?.is_some() {}
        Ok(UniqueJsonV1)
    }

    fn visit_bool<E>(self, _value: bool) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_i64<E>(self, _value: i64) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_u64<E>(self, _value: u64) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_f64<E>(self, _value: f64) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_str<E>(self, _value: &str) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_string<E>(self, _value: String) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_none<E>(self) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_unit<E>(self) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }
}

/// Resume only the exact durable message/signature. A SignedNotSubmitted or
/// Submitted journal is never re-signed and never auto-resubmitted: after a
/// crash the send boundary is ambiguous, so this reconciles/polls only the
/// locally derived signature and preserves the journal on every refusal.
pub(crate) fn resume_terminal_journal_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    journal_path: &Path,
    payer_keypair_path: &Path,
    execute: bool,
    journal: &mut DurableTerminalJournalV1,
    planned_authorization: Option<&AuthenticatedPlannedTerminalIntentV1>,
) -> Result<()> {
    authenticate_terminal_journal_v1(journal)?;
    if journal.rpc_url != origin.redacted_url() {
        return Err(refusal(
            "terminal journal RPC origin differed from the currently admitted redacted origin",
        ));
    }
    authenticate_terminal_devnet_v1(rpc, origin)?;
    if execute && !journal.authorized_mutation {
        journal.authorized_mutation = true;
        write_terminal_journal_v1(journal_path, journal, false)?;
    }
    match journal.phase {
        StageJournalPhaseV1::Finalized => verify_persisted_terminal_finalization_v1(rpc, journal),
        StageJournalPhaseV1::SignedNotSubmitted | StageJournalPhaseV1::Submitted => {
            reconcile_terminal_signature_v1(rpc, journal_path, journal)
        }
        StageJournalPhaseV1::Planned if !execute => Ok(()),
        StageJournalPhaseV1::Planned => {
            planned_authorization
                .ok_or_else(|| {
                    refusal(
                        "planned terminal signing requires fresh stage-specific semantic-owner authorization",
                    )
                })?
                .authenticate(journal)?;
            require_terminal_prestate_unchanged_v1(rpc, journal)?;
            let height = rpc
                .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
                .as_u64()
                .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
            if height > journal.intent.last_valid_block_height {
                return Err(refusal(
                    "terminal journal blockhash expired before key load; preserve it and construct a fresh plan",
                ));
            }
            authenticate_terminal_devnet_v1(rpc, origin)?;
            let payer = Keypair::new_from_array(campaign::read_keypair_file(
                payer_keypair_path,
                "terminal-fee-payer",
            )?);
            let expected_payer = Pubkey::from_str(&journal.intent.payer)
                .map_err(|error| Error::new(format!("terminal journal payer: {error}")))?;
            if payer.pubkey() != expected_payer {
                return Err(refusal(
                    "terminal fee-payer keypair does not expand to the durable payer",
                ));
            }
            let message_bytes = BASE64
                .decode(&journal.intent.message_base64)
                .map_err(|error| Error::new(format!("terminal message base64: {error}")))?;
            if sha256_hex(&message_bytes) != journal.intent.message_sha256 {
                return Err(refusal("terminal durable message digest changed"));
            }
            let message: VersionedMessage = bincode::deserialize(&message_bytes)
                .map_err(|error| Error::new(format!("terminal versioned message: {error}")))?;
            let transaction = VersionedTransaction::try_new(message, &[&payer])
                .map_err(|error| Error::new(format!("sign terminal transaction: {error}")))?;
            let signature =
                transaction.signatures.first().copied().ok_or_else(|| {
                    refusal("terminal signed transaction omitted payer signature")
                })?;
            let wire = bincode::serialize(&transaction)
                .map_err(|error| Error::new(format!("serialize terminal transaction: {error}")))?;
            if wire.len() != journal.intent.wire_bytes {
                return Err(refusal(
                    "terminal signed packet width differed from durable v0 geometry",
                ));
            }
            journal.signed_packet_base64 = Some(BASE64.encode(&wire));
            journal.expected_signature = Some(signature.to_string());
            journal.phase = StageJournalPhaseV1::SignedNotSubmitted;
            write_terminal_journal_v1(journal_path, journal, false)?;

            authenticate_terminal_devnet_v1(rpc, origin)?;
            require_terminal_prestate_unchanged_v1(rpc, journal)?;
            let returned = rpc
                .call(
                    "sendTransaction",
                    &json!([BASE64.encode(&wire), {
                        "encoding":"base64",
                        "skipPreflight":false,
                        "preflightCommitment":"finalized",
                        "maxRetries":0
                    }]),
                )?
                .as_str()
                .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
                .parse::<Signature>()
                .map_err(|error| Error::new(format!("terminal returned signature: {error}")))?;
            if returned != signature {
                return Err(refusal(
                    "RPC returned a signature different from the locally persisted packet",
                ));
            }
            journal.phase = StageJournalPhaseV1::Submitted;
            write_terminal_journal_v1(journal_path, journal, false)?;
            wait_terminal_signature_v1(rpc, &signature.to_string())?;
            finalize_terminal_signature_v1(rpc, journal, &signature.to_string())?;
            write_terminal_journal_v1(journal_path, journal, false)
        }
    }
}

fn authenticate_terminal_journal_v1(journal: &DurableTerminalJournalV1) -> Result<()> {
    if journal.schema != TERMINAL_JOURNAL_SCHEMA_V1
        || journal.cluster != "devnet"
        || sha256_hex(&serde_json::to_vec(&journal.intent)?) != journal.intent_sha256
        || terminal_journal_state_digest_v1(journal)? != journal.state_sha256
    {
        return Err(refusal(
            "terminal journal schema, cluster, intent, origin, authorization, or state digest changed",
        ));
    }
    authenticate_terminal_intent_arithmetic_v1(&journal.intent)?;
    match journal.phase {
        StageJournalPhaseV1::Planned
            if journal.signed_packet_base64.is_none()
                && journal.expected_signature.is_none()
                && journal.finalized.is_none() => {}
        StageJournalPhaseV1::SignedNotSubmitted | StageJournalPhaseV1::Submitted
            if journal.signed_packet_base64.is_some()
                && journal.expected_signature.is_some()
                && journal.finalized.is_none() => {}
        StageJournalPhaseV1::Finalized
            if journal.signed_packet_base64.is_some()
                && journal.expected_signature.is_some()
                && journal.finalized.is_some() => {}
        _ => {
            return Err(refusal(
                "terminal journal phase/evidence shape is noncanonical",
            ));
        }
    }
    if journal.phase != StageJournalPhaseV1::Planned {
        if !journal.authorized_mutation {
            return Err(refusal(
                "signed terminal journal omitted durable mutation authorization",
            ));
        }
        authenticate_terminal_signed_packet_v1(journal)?;
    }
    if let Some(finalized) = &journal.finalized {
        let packet = BASE64
            .decode(
                journal
                    .signed_packet_base64
                    .as_deref()
                    .ok_or_else(|| refusal("finalized journal omitted packet"))?,
            )
            .map_err(|error| Error::new(format!("finalized journal packet base64: {error}")))?;
        if finalized.signature != journal.expected_signature.as_deref().unwrap_or_default()
            || finalized.packet_sha256 != sha256_hex(&packet)
            || finalized.fee_lamports != journal.intent.transaction_fee_lamports
        {
            return Err(refusal(
                "finalized terminal evidence signature, packet, or fee binding changed",
            ));
        }
    }
    Ok(())
}

fn authenticate_terminal_intent_arithmetic_v1(intent: &DurableTerminalIntentV1) -> Result<()> {
    authenticate_terminal_message_decompilation_v1(intent)?;
    if checked_terminal_delta_sum_v1(intent.protocol_lamport_deltas.values().copied())? != 0 {
        return Err(refusal(
            "terminal durable protocol/prepay lamport vector was not exactly conserving before fee",
        ));
    }
    let instruction_data = BASE64
        .decode(&intent.instruction_data_base64)
        .map_err(|error| Error::new(format!("terminal instruction base64: {error}")))?;
    if sha256_hex(&instruction_data) != intent.instruction_data_sha256
        || intent.program_class != TerminalAddressClassV1::InlineProgram
    {
        return Err(refusal(
            "terminal instruction bytes or program address class changed",
        ));
    }
    let lookup_addresses = intent
        .lookup_table_addresses
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal lookup address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if lookup_addresses
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != lookup_addresses.len()
        || pubkey_vector_sha256(&lookup_addresses) != intent.lookup_table_addresses_sha256
    {
        return Err(refusal(
            "terminal frozen lookup addresses contained a duplicate or digest mismatch",
        ));
    }
    let resolved = intent
        .resolved_account_keys
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal resolved address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if resolved.iter().copied().collect::<BTreeSet<_>>().len() != resolved.len()
        || intent.pre_balances.len() != resolved.len()
        || intent.post_balances.len() != resolved.len()
        || intent.resolved_account_keys.first() != Some(&intent.payer)
    {
        return Err(refusal(
            "terminal resolved key or balance vector was duplicate, mis-sized, or omitted payer first",
        ));
    }
    let writable = intent
        .accounts
        .iter()
        .filter(|account| account.writable)
        .map(|account| account.address.as_str())
        .chain(std::iter::once(intent.payer.as_str()))
        .collect::<BTreeSet<_>>();
    if writable
        .iter()
        .any(|key| !intent.expected_accounts.contains_key(*key))
        || intent
            .protocol_lamport_deltas
            .keys()
            .any(|key| !writable.contains(key.as_str()))
    {
        return Err(refusal(
            "terminal durable intent omitted a writable poststate or attached a delta to readonly state",
        ));
    }
    for (key, expected) in &intent.expected_accounts {
        let exact_data = match &expected.lookup_table {
            None => {
                let data = BASE64.decode(&expected.data_base64).map_err(|error| {
                    Error::new(format!("terminal expected account base64: {error}"))
                })?;
                if sha256_hex(&data) != expected.data_sha256 {
                    return Err(refusal("terminal expected account byte digest changed"));
                }
                true
            }
            Some(table) => {
                let addresses = table
                    .addresses
                    .iter()
                    .map(|key| {
                        Pubkey::from_str(key).map_err(|error| {
                            Error::new(format!("terminal expected ALT address: {error}"))
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                if !expected.data_base64.is_empty()
                    || !expected.data_sha256.is_empty()
                    || expected.owner != lookup_table_program::id().to_string()
                    || expected.executable
                    || addresses.iter().copied().collect::<BTreeSet<_>>().len() != addresses.len()
                    || table
                        .authority
                        .as_deref()
                        .map(Pubkey::from_str)
                        .transpose()
                        .is_err()
                {
                    return Err(refusal(
                        "terminal expected ALT rule was malformed or carried parallel byte truth",
                    ));
                }
                false
            }
        };
        if key != &expected.address
            || (key != &intent.payer
                && expected.lamports_after_protocol != expected.lamports_after_fee)
            || (key == &intent.payer
                && expected
                    .lamports_after_protocol
                    .checked_sub(intent.transaction_fee_lamports)
                    != Some(expected.lamports_after_fee))
        {
            return Err(refusal(
                "terminal expected account key, bytes, or fee arithmetic changed",
            ));
        }
        let _ = exact_data;
    }
    for (index, key) in intent.resolved_account_keys.iter().enumerate() {
        let prestate = intent
            .prestate
            .get(key)
            .ok_or_else(|| refusal("terminal resolved key omitted exact prestate"))?;
        if prestate.address != *key || prestate.lamports != intent.pre_balances[index] {
            return Err(refusal(
                "terminal resolved key pre-balance differed from persisted account state",
            ));
        }
        let delta = intent
            .protocol_lamport_deltas
            .get(key)
            .copied()
            .unwrap_or(0);
        let after_protocol = checked_apply_lamport_delta(intent.pre_balances[index], delta)?;
        let expected = if key == &intent.payer {
            after_protocol
                .checked_sub(intent.transaction_fee_lamports)
                .ok_or_else(|| refusal("terminal payer fee arithmetic underflowed"))?
        } else {
            after_protocol
        };
        if intent.post_balances[index] != expected
            || intent
                .expected_accounts
                .get(key)
                .is_some_and(|account| account.lamports_after_fee != intent.post_balances[index])
        {
            return Err(refusal(
                "terminal durable post-balance disagreed with protocol delta or expected account",
            ));
        }
    }
    let pre_total = intent
        .pre_balances
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)))
        .ok_or_else(|| refusal("terminal pre-balance total overflowed"))?;
    let post_total = intent
        .post_balances
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)))
        .ok_or_else(|| refusal("terminal post-balance total overflowed"))?;
    if pre_total.checked_sub(post_total) != Some(u128::from(intent.transaction_fee_lamports)) {
        return Err(refusal(
            "terminal complete balance vector concealed a lamport delta beyond the exact fee",
        ));
    }
    Ok(())
}

fn authenticate_terminal_message_decompilation_v1(intent: &DurableTerminalIntentV1) -> Result<()> {
    let message_bytes = BASE64
        .decode(&intent.message_base64)
        .map_err(|error| Error::new(format!("terminal durable message base64: {error}")))?;
    if sha256_hex(&message_bytes) != intent.message_sha256 {
        return Err(refusal("terminal durable message digest changed"));
    }
    let versioned: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("terminal durable versioned message: {error}")))?;
    if versioned.serialize() != message_bytes {
        return Err(refusal(
            "terminal durable message encoding was noncanonical",
        ));
    }
    let VersionedMessage::V0(message) = &versioned else {
        return Err(refusal("terminal durable message was not v0"));
    };
    let payer = Pubkey::from_str(&intent.payer)
        .map_err(|error| Error::new(format!("terminal durable payer: {error}")))?;
    let recent_blockhash = intent
        .recent_blockhash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("terminal durable blockhash: {error}")))?;
    if message.recent_blockhash != recent_blockhash
        || message.header.num_required_signatures != 1
        || message.header.num_readonly_signed_accounts != 0
        || message.account_keys.first() != Some(&payer)
        || message.instructions.len() != 1
    {
        return Err(refusal(
            "terminal message blockhash, payer, signer header, or instruction width differed from intent",
        ));
    }
    let instruction = &message.instructions[0];
    let resolved = intent
        .resolved_account_keys
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal resolved message key: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let program_id = Pubkey::from_str(&intent.program_id)
        .map_err(|error| Error::new(format!("terminal durable program: {error}")))?;
    if resolved.get(..message.account_keys.len()) != Some(message.account_keys.as_slice())
        || resolved.get(usize::from(instruction.program_id_index)) != Some(&program_id)
        || !message.account_keys.contains(&program_id)
        || intent.program_class != TerminalAddressClassV1::InlineProgram
    {
        return Err(refusal(
            "terminal compiled program index was not the exact inline program identity",
        ));
    }
    let data = BASE64
        .decode(&intent.instruction_data_base64)
        .map_err(|error| Error::new(format!("terminal durable instruction base64: {error}")))?;
    if instruction.data != data || instruction.accounts.len() != intent.accounts.len() {
        return Err(refusal(
            "terminal compiled instruction bytes or account width differed from intent",
        ));
    }
    let static_len = message.account_keys.len();
    let required = usize::from(message.header.num_required_signatures);
    let writable_signed_end = required
        .checked_sub(usize::from(message.header.num_readonly_signed_accounts))
        .ok_or_else(|| refusal("terminal signed account header underflowed"))?;
    let writable_unsigned_end = static_len
        .checked_sub(usize::from(message.header.num_readonly_unsigned_accounts))
        .ok_or_else(|| refusal("terminal unsigned account header underflowed"))?;
    for (compiled_index, expected) in instruction.accounts.iter().zip(&intent.accounts) {
        let index = usize::from(*compiled_index);
        let key = resolved
            .get(index)
            .ok_or_else(|| refusal("terminal compiled account index exceeded resolved keys"))?;
        let expected_key = Pubkey::from_str(&expected.address)
            .map_err(|error| Error::new(format!("terminal intended account: {error}")))?;
        let signer = index < required;
        let writable = if index < required {
            index < writable_signed_end
        } else if index < static_len {
            index < writable_unsigned_end
        } else {
            index < static_len + intent.loaded_writable.len()
        };
        let loaded = index >= static_len;
        let class_placement_ok = match expected.class {
            TerminalAddressClassV1::LookupStable => loaded,
            TerminalAddressClassV1::InlineSigner
            | TerminalAddressClassV1::InlineProgram
            | TerminalAddressClassV1::InlineRequestBound => !loaded,
        };
        if *key != expected_key
            || signer != expected.signer
            || writable != expected.writable
            || !class_placement_ok
        {
            return Err(refusal(
                "terminal compiled account key, order, privilege, or address-class placement differed from intent",
            ));
        }
    }
    match &intent.lookup_table {
        Some(table) => {
            let table = Pubkey::from_str(table)
                .map_err(|error| Error::new(format!("terminal durable lookup table: {error}")))?;
            if message.address_table_lookups.len() != 1
                || message.address_table_lookups[0].account_key != table
            {
                return Err(refusal(
                    "terminal compiled lookup identity differed from durable intent",
                ));
            }
            let addresses = intent
                .lookup_table_addresses
                .iter()
                .map(|key| {
                    Pubkey::from_str(key).map_err(|error| {
                        Error::new(format!("terminal durable lookup address: {error}"))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let lookup = &message.address_table_lookups[0];
            let projected_writable = lookup
                .writable_indexes
                .iter()
                .map(|index| addresses.get(usize::from(*index)).map(ToString::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| refusal("terminal writable lookup index exceeded durable table"))?;
            let projected_readonly = lookup
                .readonly_indexes
                .iter()
                .map(|index| addresses.get(usize::from(*index)).map(ToString::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| refusal("terminal readonly lookup index exceeded durable table"))?;
            if projected_writable != intent.loaded_writable
                || projected_readonly != intent.loaded_readonly
            {
                return Err(refusal(
                    "terminal lookup indices did not reproduce durable loaded address order",
                ));
            }
        }
        None => {
            if !message.address_table_lookups.is_empty()
                || !intent.lookup_table_addresses.is_empty()
                || !intent.loaded_writable.is_empty()
                || !intent.loaded_readonly.is_empty()
            {
                return Err(refusal(
                    "inline terminal infrastructure message concealed lookup state",
                ));
            }
        }
    }
    let expected_wire = 1_usize
        .checked_add(64)
        .and_then(|value| value.checked_add(message_bytes.len()))
        .ok_or_else(|| refusal("terminal durable wire geometry overflowed"))?;
    if intent.wire_bytes != expected_wire || intent.wire_bytes > PACKET_DATA_BYTES {
        return Err(refusal(
            "terminal durable wire geometry differed from exact one-signature message",
        ));
    }
    Ok(())
}

fn authenticate_terminal_signed_packet_v1(journal: &DurableTerminalJournalV1) -> Result<()> {
    let wire = BASE64
        .decode(
            journal
                .signed_packet_base64
                .as_deref()
                .ok_or_else(|| refusal("signed terminal journal omitted packet"))?,
        )
        .map_err(|error| Error::new(format!("signed terminal packet base64: {error}")))?;
    if wire.len() != journal.intent.wire_bytes {
        return Err(refusal(
            "signed terminal packet width differed from durable intent",
        ));
    }
    let transaction: VersionedTransaction = bincode::deserialize(&wire)
        .map_err(|error| Error::new(format!("signed terminal packet: {error}")))?;
    if bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("reencode signed terminal packet: {error}")))?
        != wire
    {
        return Err(refusal("signed terminal packet encoding was noncanonical"));
    }
    let message_bytes = BASE64
        .decode(&journal.intent.message_base64)
        .map_err(|error| Error::new(format!("durable terminal message base64: {error}")))?;
    if sha256_hex(&message_bytes) != journal.intent.message_sha256
        || transaction.message.serialize() != message_bytes
    {
        return Err(refusal(
            "signed terminal packet message differed from the durable exact message",
        ));
    }
    let payer = Pubkey::from_str(&journal.intent.payer)
        .map_err(|error| Error::new(format!("durable terminal payer: {error}")))?;
    let VersionedMessage::V0(message) = &transaction.message else {
        return Err(refusal("signed terminal packet was not v0"));
    };
    if message.header.num_required_signatures != 1
        || message.account_keys.first() != Some(&payer)
        || transaction.signatures.len() != 1
    {
        return Err(refusal(
            "signed terminal packet signer set was not exactly the durable payer",
        ));
    }
    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| refusal("signed terminal packet omitted signature"))?;
    if !signature.verify(payer.as_ref(), &message_bytes)
        || journal.expected_signature.as_deref() != Some(&signature.to_string())
    {
        return Err(refusal(
            "terminal signature was invalid or differed from the durable expected signature",
        ));
    }
    Ok(())
}

fn refresh_terminal_journal_digest_v1(journal: &mut DurableTerminalJournalV1) -> Result<()> {
    journal.state_sha256 = terminal_journal_state_digest_v1(journal)?;
    Ok(())
}

fn terminal_journal_state_digest_v1(journal: &DurableTerminalJournalV1) -> Result<String> {
    let mut canonical = journal.clone();
    canonical.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn reconcile_terminal_signature_v1(
    rpc: &mut Rpc,
    path: &Path,
    journal: &mut DurableTerminalJournalV1,
) -> Result<()> {
    let signature = journal
        .expected_signature
        .clone()
        .ok_or_else(|| refusal("terminal signed phase omitted its expected signature"))?;
    if finalized_terminal_transaction_v1(rpc, &signature)?.is_none() {
        return Err(Error::new(format!(
            "REFUSED: terminal transaction {signature} is not finalized. The durable {:?} journal is ambiguous and will not re-sign or resubmit; reconcile only this exact signature",
            journal.phase
        )));
    }
    finalize_terminal_signature_v1(rpc, journal, &signature)?;
    write_terminal_journal_v1(path, journal, false)
}

fn wait_terminal_signature_v1(rpc: &mut Rpc, signature: &str) -> Result<()> {
    let deadline = Instant::now() + TERMINAL_FINALITY_WAIT;
    while Instant::now() < deadline {
        if finalized_terminal_transaction_v1(rpc, signature)?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(Error::new(format!(
        "terminal transaction {signature} did not reach finalized history within 300 seconds; durable signature retained and no replay attempted"
    )))
}

fn finalized_terminal_transaction_v1(rpc: &mut Rpc, signature: &str) -> Result<Option<Value>> {
    let status = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let status = status
        .get("value")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .filter(|value| !value.is_null());
    let Some(status) = status else {
        return Ok(None);
    };
    if status.get("confirmationStatus").and_then(Value::as_str) != Some("finalized") {
        return Ok(None);
    }
    let transaction = rpc.call(
        "getTransaction",
        &json!([signature, {
            "encoding":"base64",
            "commitment":"finalized",
            "maxSupportedTransactionVersion":0
        }]),
    )?;
    if transaction.is_null() {
        return Err(refusal(
            "finalized terminal signature omitted finalized transaction history",
        ));
    }
    Ok(Some(transaction))
}

fn finalize_terminal_signature_v1(
    rpc: &mut Rpc,
    journal: &mut DurableTerminalJournalV1,
    signature: &str,
) -> Result<()> {
    let history = authenticate_finalized_terminal_history_v1(rpc, journal, signature)?;
    let poststate =
        terminal_expected_poststate_v1(rpc, &journal.intent.expected_accounts, history.slot)?;
    journal.finalized = Some(DurableFinalizedEvidenceV1 {
        signature: signature.into(),
        slot: history.slot,
        fee_lamports: history.fee_lamports,
        compute_units_consumed: history.compute_units_consumed,
        packet_sha256: history.packet_sha256,
        poststate,
    });
    journal.phase = StageJournalPhaseV1::Finalized;
    Ok(())
}

struct AuthenticatedFinalizedHistoryV1 {
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    packet_sha256: String,
}

/// Authenticate only immutable transaction history. Live account reads are
/// intentionally excluded: later terminal stages legitimately mutate or close
/// predecessor accounts, so archived finalized journals must not rot.
fn authenticate_finalized_terminal_history_v1(
    rpc: &mut Rpc,
    journal: &DurableTerminalJournalV1,
    signature: &str,
) -> Result<AuthenticatedFinalizedHistoryV1> {
    let transaction = finalized_terminal_transaction_v1(rpc, signature)?.ok_or_else(|| {
        refusal("terminal signature has not reached finalized transaction history")
    })?;
    let meta = transaction
        .get("meta")
        .ok_or_else(|| refusal("finalized terminal transaction omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(Error::new(format!(
            "finalized terminal transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let transaction_wire = transaction
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("finalized terminal history omitted transaction tuple"))?;
    if transaction_wire.len() != 2
        || transaction_wire.get(1).and_then(Value::as_str) != Some("base64")
    {
        return Err(refusal(
            "finalized terminal transaction tuple was not exactly [body, base64]",
        ));
    }
    let encoded_packet = transaction_wire
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("finalized terminal history omitted base64 packet"))?;
    let packet = BASE64
        .decode(encoded_packet)
        .map_err(|error| Error::new(format!("finalized terminal packet base64: {error}")))?;
    let durable_packet = BASE64
        .decode(
            journal
                .signed_packet_base64
                .as_deref()
                .ok_or_else(|| refusal("terminal journal omitted signed packet"))?,
        )
        .map_err(|error| Error::new(format!("durable terminal packet base64: {error}")))?;
    if packet != durable_packet {
        return Err(refusal(
            "finalized transaction packet differed byte-for-byte from the durable signed packet",
        ));
    }
    let signed: VersionedTransaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("finalized terminal packet: {error}")))?;
    let packet_signature = signed
        .signatures
        .first()
        .ok_or_else(|| refusal("finalized terminal packet omitted signature"))?;
    if packet_signature.to_string() != signature
        || journal.expected_signature.as_deref() != Some(signature)
    {
        return Err(refusal(
            "finalized packet signature differed from the durable expected signature",
        ));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("finalized terminal transaction omitted fee"))?;
    if fee != journal.intent.transaction_fee_lamports {
        return Err(refusal(
            "finalized terminal fee differed from the exact planned fee",
        ));
    }
    authenticate_finalized_terminal_balance_vector_v1(meta, &signed, &journal.intent, fee)?;
    authenticate_terminal_return_data_v1(meta, journal.intent.expected_return_data.as_ref())?;
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("finalized terminal transaction omitted slot"))?;
    Ok(AuthenticatedFinalizedHistoryV1 {
        slot,
        fee_lamports: fee,
        compute_units_consumed: meta.get("computeUnitsConsumed").and_then(Value::as_u64),
        packet_sha256: sha256_hex(&packet),
    })
}

fn authenticate_terminal_return_data_v1(
    meta: &Value,
    expected: Option<&DurableReturnDataV1>,
) -> Result<()> {
    let observed = meta.get("returnData").filter(|value| !value.is_null());
    match (expected, observed) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(refusal(
            "finalized terminal transaction carried unexpected return data",
        )),
        (Some(_), None) => Err(refusal(
            "finalized terminal transaction omitted required return data",
        )),
        (Some(expected), Some(observed)) => {
            let producer = observed
                .get("programId")
                .and_then(Value::as_str)
                .ok_or_else(|| refusal("terminal return data omitted producer"))?;
            let encoded_body = observed
                .get("data")
                .and_then(Value::as_array)
                .ok_or_else(|| refusal("terminal return data omitted body tuple"))?;
            if encoded_body.len() != 2
                || encoded_body.get(1).and_then(Value::as_str) != Some("base64")
            {
                return Err(refusal(
                    "terminal return data tuple was not exactly [body, base64]",
                ));
            }
            let encoded = encoded_body
                .first()
                .and_then(Value::as_str)
                .ok_or_else(|| refusal("terminal return data omitted base64 body"))?;
            let body = BASE64
                .decode(encoded)
                .map_err(|error| Error::new(format!("terminal return data base64: {error}")))?;
            if producer != expected.producer
                || BASE64.encode(&body) != expected.body_base64
                || sha256_hex(&body) != expected.body_sha256
            {
                return Err(refusal(
                    "finalized terminal return producer or body differed from semantic prediction",
                ));
            }
            Ok(())
        }
    }
}

fn authenticate_finalized_terminal_balance_vector_v1(
    meta: &Value,
    transaction: &VersionedTransaction,
    intent: &DurableTerminalIntentV1,
    fee: u64,
) -> Result<()> {
    let loaded = meta
        .get("loadedAddresses")
        .and_then(Value::as_object)
        .ok_or_else(|| refusal("finalized terminal meta omitted loadedAddresses"))?;
    if loaded.len() != 2 {
        return Err(refusal(
            "finalized terminal loadedAddresses carried unknown fields",
        ));
    }
    let strings = |value: Option<&Value>, label: &str| -> Result<Vec<String>> {
        value
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("finalized terminal {label} was not an array")))?
            .iter()
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    Error::new(format!("finalized terminal {label} key was not a string"))
                })
            })
            .collect()
    };
    let loaded_writable = strings(loaded.get("writable"), "loaded writable")?;
    let loaded_readonly = strings(loaded.get("readonly"), "loaded readonly")?;
    if loaded_writable != intent.loaded_writable || loaded_readonly != intent.loaded_readonly {
        return Err(refusal(
            "finalized terminal loaded address order differed from durable v0 resolution",
        ));
    }
    let VersionedMessage::V0(message) = &transaction.message else {
        return Err(refusal("finalized terminal message was not v0"));
    };
    let resolved = message
        .account_keys
        .iter()
        .map(ToString::to_string)
        .chain(loaded_writable.iter().cloned())
        .chain(loaded_readonly.iter().cloned())
        .collect::<Vec<_>>();
    if resolved != intent.resolved_account_keys {
        return Err(refusal(
            "finalized terminal account-index vector differed from durable resolution",
        ));
    }
    let balances = |label: &str| -> Result<Vec<u64>> {
        meta.get(label)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("finalized terminal {label} was not an array")))?
            .iter()
            .map(|value| {
                value.as_u64().ok_or_else(|| {
                    Error::new(format!("finalized terminal {label} entry was not u64"))
                })
            })
            .collect()
    };
    let pre = balances("preBalances")?;
    let post = balances("postBalances")?;
    if pre != intent.pre_balances
        || post != intent.post_balances
        || pre.len() != resolved.len()
        || post.len() != resolved.len()
    {
        return Err(refusal(
            "finalized terminal pre/post balance vector differed from exact durable arithmetic",
        ));
    }
    let payer_index = resolved
        .iter()
        .position(|key| key == &intent.payer)
        .ok_or_else(|| refusal("terminal balance vector omitted payer"))?;
    let payer_protocol_delta = intent
        .protocol_lamport_deltas
        .get(&intent.payer)
        .copied()
        .unwrap_or(0);
    let payer_after_protocol = checked_apply_lamport_delta(pre[payer_index], payer_protocol_delta)?;
    if payer_after_protocol.checked_sub(fee) != Some(post[payer_index]) {
        return Err(refusal(
            "terminal payer pre/post balance did not equal protocol delta plus exact fee",
        ));
    }
    Ok(())
}

fn terminal_expected_poststate_v1(
    rpc: &mut Rpc,
    expected: &BTreeMap<String, DurableExpectedAccountV1>,
    minimum_slot: u64,
) -> Result<BTreeMap<String, DurableAccountStateV1>> {
    let keys = expected
        .values()
        .map(|account| {
            Pubkey::from_str(&account.address)
                .map_err(|error| Error::new(format!("terminal expected address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let (slot, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
    if slot < minimum_slot {
        return Err(refusal(
            "terminal poststate RPC context slot was below the finalized transaction slot",
        ));
    }
    let mut states = BTreeMap::new();
    for ((key, value), expected) in keys.into_iter().zip(values.iter()).zip(expected.values()) {
        let state = durable_rpc_state(key, value.as_ref());
        if state.owner != expected.owner
            || state.lamports != expected.lamports_after_fee
            || state.executable != expected.executable
        {
            return Err(refusal(
                "finalized terminal owner/lamports/executable differed from exact poststate",
            ));
        }
        match &expected.lookup_table {
            None => {
                let data = BASE64.decode(&expected.data_base64).map_err(|error| {
                    Error::new(format!("terminal expected data base64: {error}"))
                })?;
                if state.data_len != data.len()
                    || state.data_sha256 != expected.data_sha256
                    || state.data_sha256 != sha256_hex(&data)
                {
                    return Err(refusal(
                        "finalized terminal bytes differed from exact poststate",
                    ));
                }
            }
            Some(rule) => {
                let account = value.as_ref().ok_or_else(|| {
                    refusal("finalized terminal ALT poststate was unexpectedly vacant")
                })?;
                authenticate_lookup_poststate_rule_v1(account, rule, minimum_slot)?;
            }
        }
        states.insert(key.to_string(), state);
    }
    Ok(states)
}

fn authenticate_lookup_poststate_rule_v1(
    account: &RpcAccount,
    rule: &DurableLookupTablePoststateV1,
    transaction_slot: u64,
) -> Result<()> {
    let decoded = AddressLookupTable::deserialize(&account.data)
        .map_err(|_| refusal("finalized terminal ALT bytes did not decode canonically"))?;
    let authority = rule
        .authority
        .as_deref()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal ALT authority: {error}")))
        })
        .transpose()?;
    let addresses = rule
        .addresses
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal ALT address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let expected_last_extended_slot = match rule.last_extended_slot {
        DurableLookupLastExtendedSlotV1::Exact(slot) => slot,
        DurableLookupLastExtendedSlotV1::FinalizedTransaction => transaction_slot,
    };
    if decoded.meta.authority != authority
        || decoded.meta.deactivation_slot != rule.deactivation_slot
        || decoded.meta.last_extended_slot != expected_last_extended_slot
        || decoded.meta.last_extended_slot_start_index != rule.last_extended_slot_start_index
        || decoded.addresses.as_ref() != addresses.as_slice()
        || account.data.len() != lookup_table_data_len(addresses.len())?
    {
        return Err(refusal(
            "finalized terminal ALT authority, slots, extension boundary, addresses, or width differed from exact poststate",
        ));
    }
    Ok(())
}

fn canonical_lookup_poststate_bytes_v1(
    rule: &DurableLookupTablePoststateV1,
    transaction_slot: u64,
) -> Result<Vec<u8>> {
    let authority = rule
        .authority
        .as_deref()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal ALT authority: {error}")))
        })
        .transpose()?;
    let addresses = rule
        .addresses
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal ALT address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let last_extended_slot = match rule.last_extended_slot {
        DurableLookupLastExtendedSlotV1::Exact(slot) => slot,
        DurableLookupLastExtendedSlotV1::FinalizedTransaction => transaction_slot,
    };
    let mut data = vec![0_u8; lookup_table_data_len(addresses.len())?];
    AddressLookupTable::overwrite_meta_data(
        &mut data,
        LookupTableMeta {
            deactivation_slot: rule.deactivation_slot,
            last_extended_slot,
            last_extended_slot_start_index: rule.last_extended_slot_start_index,
            authority,
            ..LookupTableMeta::default()
        },
    )
    .map_err(|_| refusal("terminal ALT exact poststate metadata did not serialize"))?;
    let mut offset = LOOKUP_TABLE_META_SIZE;
    for address in addresses {
        let end = offset
            .checked_add(ALT_ADDRESS_BYTES)
            .ok_or_else(|| refusal("terminal ALT exact poststate offset overflowed"))?;
        data.get_mut(offset..end)
            .ok_or_else(|| refusal("terminal ALT exact poststate width underflowed"))?
            .copy_from_slice(address.as_ref());
        offset = end;
    }
    Ok(data)
}

fn authenticate_persisted_terminal_poststate_v1(
    intent: &DurableTerminalIntentV1,
    finalized: &DurableFinalizedEvidenceV1,
) -> Result<()> {
    if finalized.poststate.len() != intent.expected_accounts.len() {
        return Err(refusal(
            "persisted finalized poststate width differed from exact durable intent",
        ));
    }
    for (key, expected) in &intent.expected_accounts {
        let address = Pubkey::from_str(key)
            .map_err(|error| Error::new(format!("terminal persisted address: {error}")))?;
        let owner = Pubkey::from_str(&expected.owner)
            .map_err(|error| Error::new(format!("terminal persisted owner: {error}")))?;
        let data = match &expected.lookup_table {
            None => BASE64.decode(&expected.data_base64).map_err(|error| {
                Error::new(format!("terminal expected persisted data base64: {error}"))
            })?,
            Some(rule) => canonical_lookup_poststate_bytes_v1(rule, finalized.slot)?,
        };
        let exact = durable_state(
            address,
            owner,
            expected.lamports_after_fee,
            expected.executable,
            &data,
        );
        if finalized.poststate.get(key) != Some(&exact) {
            return Err(refusal(
                "persisted finalized poststate differed from exact intent bytes, owner, lamports, or executable flag",
            ));
        }
    }
    Ok(())
}

fn verify_persisted_terminal_finalization_v1(
    rpc: &mut Rpc,
    journal: &DurableTerminalJournalV1,
) -> Result<()> {
    let finalized = journal
        .finalized
        .as_ref()
        .ok_or_else(|| refusal("finalized terminal journal omitted evidence"))?;
    let history = authenticate_finalized_terminal_history_v1(rpc, journal, &finalized.signature)?;
    if history.slot != finalized.slot
        || history.fee_lamports != finalized.fee_lamports
        || history.compute_units_consumed != finalized.compute_units_consumed
        || history.packet_sha256 != finalized.packet_sha256
    {
        return Err(refusal(
            "reverified terminal packet, return data, balances, slot, fee, or compute evidence differed from persisted finalized evidence",
        ));
    }
    authenticate_persisted_terminal_poststate_v1(&journal.intent, finalized)
}

fn require_terminal_prestate_unchanged_v1(
    rpc: &mut Rpc,
    journal: &DurableTerminalJournalV1,
) -> Result<()> {
    let keys = journal
        .intent
        .prestate
        .values()
        .map(|account| {
            Pubkey::from_str(&account.address)
                .map_err(|error| Error::new(format!("terminal prestate address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let (slot, values) = rpc.finalized_accounts(&keys, journal.intent.observation_slot)?;
    if slot < journal.intent.observation_slot {
        return Err(refusal(
            "terminal prestate RPC context slot was below durable observation",
        ));
    }
    for ((key, value), expected) in keys
        .into_iter()
        .zip(values.iter())
        .zip(journal.intent.prestate.values())
    {
        if durable_rpc_state(key, value.as_ref()) != *expected {
            return Err(refusal(
                "terminal finalized prestate changed after durable planning",
            ));
        }
    }
    Ok(())
}

fn terminal_latest_blockhash(rpc: &mut Rpc) -> Result<(Hash, u64)> {
    let value = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = value
        .get("value")
        .ok_or_else(|| Error::new("getLatestBlockhash omitted value"))?;
    let blockhash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("terminal blockhash: {error}")))?;
    let last_valid = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted lastValidBlockHeight"))?;
    Ok((blockhash, last_valid))
}

fn terminal_fee_for_message(rpc: &mut Rpc, message_base64: &str) -> Result<u64> {
    rpc.call(
        "getFeeForMessage",
        &json!([message_base64, {"commitment":"finalized"}]),
    )?
    .get("value")
    .and_then(Value::as_u64)
    .ok_or_else(|| Error::new("getFeeForMessage omitted exact terminal fee"))
}

fn authenticate_terminal_devnet_v1(rpc: &mut Rpc, origin: &ClusterOriginV1) -> Result<()> {
    let genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("getGenesisHash result was not a string"))?
        .to_owned();
    if genesis != DEVNET_GENESIS_HASH {
        return Err(refusal(
            "terminal executor observed another genesis than exact Solana devnet",
        ));
    }
    origin.authenticate_genesis(&genesis)
}

fn durable_observed_state(account: &ObservedAccount) -> DurableAccountStateV1 {
    durable_state(
        account.key,
        account.owner,
        account.lamports,
        account.executable,
        &account.data,
    )
}

fn durable_rpc_state(key: Pubkey, account: Option<&RpcAccount>) -> DurableAccountStateV1 {
    match account {
        Some(account) => durable_state(
            key,
            account.owner,
            account.lamports,
            account.executable,
            &account.data,
        ),
        None => durable_state(key, solana_sdk_ids::system_program::ID, 0, false, &[]),
    }
}

fn durable_state(
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    data: &[u8],
) -> DurableAccountStateV1 {
    let mut exact = Sha256::new();
    exact.update(owner.as_ref());
    exact.update(lamports.to_le_bytes());
    exact.update([u8::from(executable)]);
    exact.update(u64::try_from(data.len()).unwrap_or(u64::MAX).to_le_bytes());
    exact.update(data);
    DurableAccountStateV1 {
        address: key.to_string(),
        owner: owner.to_string(),
        lamports,
        executable,
        data_len: data.len(),
        data_sha256: sha256_hex(data),
        account_sha256: hex_bytes(&exact.finalize()),
    }
}

fn pubkey_vector_sha256(addresses: &[Pubkey]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/terminal-alt-stable-addresses/v1");
    hasher.update(
        u64::try_from(addresses.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for address in addresses {
        hasher.update(address.as_ref());
    }
    hex_bytes(&hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_bytes(&Sha256::digest(bytes))
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Stage-two planning is the one protocol stage whose semantic owner admits
/// both Submit and chain-detected Complete.  Complete carries no invented
/// receipt and advances only after the caller authenticates the exact observed
/// Retiring root returned by the operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DirectBeginRetiringCallerPlanV1 {
    Submit(TerminalSemanticMutationV1),
    Complete {
        observation: Observation,
        root: ExpectedAccountPoststateV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChainDerivedTerminalMutationV1 {
    pub(crate) mutation: TerminalSemanticMutationV1,
    pub(crate) closure: TerminalMetaClosureV1,
    pub(crate) prestate: Vec<ObservedAccount>,
}

fn authenticate_chain_derived_planned_journal_v1(
    journal: &DurableTerminalJournalV1,
    chain: &ChainDerivedTerminalMutationV1,
) -> Result<AuthenticatedPlannedTerminalIntentV1> {
    authenticate_planned_protocol_owner_v1(
        journal,
        DurableTerminalMutationV1::Protocol {
            stage: chain.mutation.stage,
        },
        &chain.mutation,
        &chain.closure,
        &chain.prestate,
    )
}

pub(crate) fn plan_core_begin_retiring_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market_key: Pubkey,
    additional_keys: &[Pubkey],
) -> Result<ChainDerivedTerminalMutationV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let claims_programdata = pubkey(&plan.claims.programdata_id)?;
    let activation = pubkey(&plan.activation)?;
    let aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market_key.as_ref()],
        &claims,
    )
    .0;
    if aggregate != evidence_pubkey(evidence, "claims_aggregate")? {
        return Err(refusal(
            "Core BeginRetiring Claims aggregate evidence was not the canonical Market PDA",
        ));
    }
    let mut keys = vec![
        market_key,
        aggregate,
        registry,
        activation,
        core,
        core_programdata,
        claims,
        claims_programdata,
    ];
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let market_account = account(market_key, "Core Market")?;
    let market = decode_routed_market(&market_account, core, plan)?;
    if market.phase != Phase::Terminal || market.readiness != Readiness::Consumed {
        return Err(refusal(
            "Core BeginRetiring requires the exact Terminal/Consumed Market",
        ));
    }
    authenticate_role(
        &account(registry, "Registry program")?,
        &account(activation, "activation cache")?,
        market.identity.selected_release_set.to_bytes(),
        ExecutionRoleV1::Core,
        &account(core, "Core program")?,
        &account(core_programdata, "Core ProgramData")?,
    )?;
    authenticate_role(
        &account(registry, "Registry program")?,
        &account(activation, "activation cache")?,
        market.identity.selected_release_set.to_bytes(),
        ExecutionRoleV1::Claims,
        &account(claims, "Claims program")?,
        &account(claims_programdata, "Claims ProgramData")?,
    )?;
    authenticate_zero_claims(
        &account(aggregate, "Claims aggregate")?,
        aggregate,
        claims,
        market,
        hex32(&evidence.founding_custody_context)?,
    )?;
    let request = Request::administrative(
        Action::BeginRetiring,
        market.identity.generation,
        market.identity.market_id,
    );
    let instruction = Instruction {
        program_id: core,
        accounts: core_begin_retiring_meta_closure_v1(
            market_key,
            activation,
            registry,
            core,
            core_programdata,
        )
        .accounts,
        data: request
            .encode()
            .map_err(|error| Error::new(format!("Core BeginRetiring request: {error:?}")))?
            .to_vec(),
    };
    let closure = core_begin_retiring_meta_closure_v1(
        market_key,
        activation,
        registry,
        core,
        core_programdata,
    );
    closure.authenticate_instruction(&instruction)?;
    let mut expected_market = market;
    begin_retiring(
        request,
        &mut expected_market,
        core_admission_from_plan_v1(plan, market)?,
    )
    .map_err(|error| Error::new(format!("Core BeginRetiring semantic owner: {error:?}")))?;
    let expected_market_data = expected_market
        .encode()
        .map_err(|error| Error::new(format!("Core Retiring Market bytes: {error:?}")))?
        .to_vec();
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok(ChainDerivedTerminalMutationV1 {
        mutation: TerminalSemanticMutationV1 {
            stage: TerminalStageV1::CoreBeginRetiring,
            observation: snapshot.observation,
            instruction,
            expected_return_data: None,
            expected_accounts: vec![ExpectedAccountPoststateV1::exact(
                market_key,
                core,
                market_account.lamports,
                false,
                expected_market_data,
            )],
            protocol_lamport_deltas: BTreeMap::from([(market_key, 0)]),
        },
        closure,
        prestate,
    })
}

fn core_admission_from_plan_v1(plan: &SuccessorPlan, market: CoreState) -> Result<Admission> {
    let binding = |pin: &crate::model::ProgramPin| -> Result<Binding> {
        Ok(Binding {
            program: Identity::new(pubkey(&pin.program_id)?.to_bytes())
                .map_err(|_| refusal("Core release Program identity was zero"))?,
            artifact_release: Identity::new(hex32(&pin.artifact_release_id)?)
                .map_err(|_| refusal("Core release artifact identity was zero"))?,
            semantic_release: Identity::new(hex32(&pin.semantic_release_id)?)
                .map_err(|_| refusal("Core release semantic identity was zero"))?,
        })
    };
    let selected = ReleaseSet {
        release_set_id: market.identity.selected_release_set,
        bindings: [
            binding(&plan.core)?,
            binding(&plan.claims)?,
            binding(&plan.trading)?,
            binding(&plan.resolution)?,
            binding(&plan.custody)?,
        ],
    };
    let observed = selected.bindings[Role::Core as usize];
    Ok(Admission {
        market_registry_program: market.identity.registry_program,
        market_release_set_id: market.identity.selected_release_set,
        selected,
        receipt: ReleaseReceipt {
            registry_program: market.identity.registry_program,
            release_set_id: market.identity.selected_release_set,
            role: Role::Core,
            observed,
            activation_cache_authenticated: true,
            current_deployment_reauthenticated: true,
        },
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChainDirectBeginRetiringV1 {
    Submit(ChainDerivedTerminalMutationV1),
    Complete {
        observation: Observation,
        root: ExpectedAccountPoststateV1,
    },
}

/// Discover and authenticate the complete stage-two graph directly from the
/// plan/evidence identities. No caller PDA or artifact table is supplied: the
/// DCLTDBR1 semantic owner derives all 20 coordinates, and its fresh finalized
/// report must reproduce the initial closure before a journal can be built.
pub(crate) fn plan_direct_begin_retiring_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    additional_keys: &[Pubkey],
) -> Result<ChainDirectBeginRetiringV1> {
    let direct = market_input
        .direct_capability
        .as_ref()
        .ok_or_else(|| refusal("successor plan omitted its Direct capability payload"))?;
    let root = evidence_pubkey(evidence, "direct_capability_root")?;
    let release_set = hex32(&plan.release_set_id)?;
    let manifest = evidence_digest(evidence, "capability_manifest_record")?;
    let program_set = evidence_digest(evidence, "direct_program_set_record")?;
    let config = evidence_digest(evidence, "direct_execution_config_record")?;
    let context = direct_begin_retiring_context_v1(
        release_set,
        market.to_bytes(),
        root.to_bytes(),
        manifest,
        program_set,
        config,
        market_input.generation,
        evidence.direct_selected_manifest_entry_index,
    );
    let placeholder_request = DirectBeginRetiringRequestV1 {
        release_set,
        market: market.to_bytes(),
        context,
        root: root.to_bytes(),
        manifest,
        program_set,
        config,
        // Neither digest derives an account coordinate. Nonzero placeholders
        // let the semantic owner project the stable frame before Core stage
        // one changes Market bytes; the fresh stage-two report still commits
        // and authenticates the real finalized digests.
        expected_market_digest: [1; 32],
        expected_root_digest: [2; 32],
        generation: market_input.generation,
        entry_index: evidence.direct_selected_manifest_entry_index,
    };
    let closure = direct_begin_retiring_meta_closure_v1(DirectBeginRetiringCoordinateInputV1 {
        request: placeholder_request,
        descriptor: evidence_digest(evidence, "direct_begin_retiring_descriptor_record")?,
        account_profile: evidence_digest(evidence, "direct_begin_retiring_account_profile_record")?,
        effect: evidence_digest(evidence, "direct_begin_retiring_effect_record")?,
        registry_program: pubkey(&plan.registry.program_id)?,
        core_program: pubkey(&plan.core.program_id)?,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        trading_program: pubkey(&plan.trading.program_id)?,
        trading_programdata: pubkey(&plan.trading.programdata_id)?,
    })?;
    let mut keys = closure
        .accounts
        .iter()
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |index: usize, label: &str| -> Result<ObservedAccount> {
        let key = closure
            .accounts
            .get(index)
            .ok_or_else(|| refusal("DCLTDBR1 closure omitted a canonical role"))?
            .pubkey;
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let operator = DirectBeginRetiringSnapshotV1 {
        genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
        ordinary_release_witness: direct_ordinary_witness_v1(direct)?,
        root: account(0, "Direct root")?,
        market: account(1, "Core Market")?,
        capability_manifest: account(2, "capability manifest")?,
        program_set: account(3, "Direct ProgramSet")?,
        program_set_staging: account(4, "Direct ProgramSet staging")?,
        descriptor: account(5, "Direct BeginRetiring descriptor")?,
        descriptor_staging: account(6, "Direct BeginRetiring descriptor staging")?,
        config: account(7, "Direct config")?,
        config_staging: account(8, "Direct config staging")?,
        account_profile: account(9, "Direct BeginRetiring profile")?,
        account_profile_staging: account(10, "Direct BeginRetiring profile staging")?,
        effect: account(11, "Direct BeginRetiring effect")?,
        effect_staging: account(12, "Direct BeginRetiring effect staging")?,
        activation_cache: account(13, "Registry activation cache")?,
        core_program: account(14, "Core program")?,
        core_programdata: account(15, "Core ProgramData")?,
        trading_program: account(16, "Trading program")?,
        trading_programdata: account(17, "Trading ProgramData")?,
        registry_program: account(18, "Registry program")?,
        rent_sysvar: account(19, "Rent sysvar")?,
    };
    match plan_direct_begin_retiring_caller_v1(&operator, &closure)? {
        DirectBeginRetiringCallerPlanV1::Submit(mutation) => {
            let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
            prestate.sort_unstable_by_key(|account| account.key);
            Ok(ChainDirectBeginRetiringV1::Submit(
                ChainDerivedTerminalMutationV1 {
                    mutation,
                    closure,
                    prestate,
                },
            ))
        }
        DirectBeginRetiringCallerPlanV1::Complete { observation, root } => {
            Ok(ChainDirectBeginRetiringV1::Complete { observation, root })
        }
    }
}

fn direct_ordinary_witness_v1(
    direct: &crate::model::DirectMarketCapabilityV1,
) -> Result<DirectInlineOrdinaryHotBundleV4> {
    Ok(DirectInlineOrdinaryHotBundleV4 {
        account_profile: decoded_exact_array_v1(
            &direct.ordinary_account_profile_hex,
            "Direct ordinary AccountProfile",
        )?,
        lifecycle_policy: decoded_exact_array_v1(
            &direct.ordinary_lifecycle_policy_hex,
            "Direct ordinary LifecyclePolicy",
        )?,
        request_profile: decoded_exact_array_v1(
            &direct.ordinary_request_profile_hex,
            "Direct ordinary RequestProfile",
        )?,
        transition: decoded_exact_array_v1(
            &direct.ordinary_transition_hex,
            "Direct ordinary Transition",
        )?,
        strategy: decoded_exact_array_v1(
            &direct.ordinary_strategy_hex,
            "Direct ordinary Strategy",
        )?,
        effect: decoded_exact_array_v1(&direct.ordinary_effect_hex, "Direct ordinary Effect")?,
        descriptor: decoded_exact_array_v1(
            &direct.ordinary_descriptor_hex,
            "Direct ordinary descriptor",
        )?,
    })
}

fn decoded_exact_array_v1<const N: usize>(value: &str, label: &str) -> Result<[u8; N]> {
    decode_hex(value)?.try_into().map_err(|bytes: Vec<u8>| {
        Error::new(format!("{label} is {} bytes, expected {N}", bytes.len()))
    })
}

fn evidence_pubkey(evidence: &CampaignTerminalEvidenceV1, label: &str) -> Result<Pubkey> {
    pubkey(&required_account(evidence, label)?.address)
}

fn evidence_digest(evidence: &CampaignTerminalEvidenceV1, label: &str) -> Result<[u8; 32]> {
    hex32(&required_account(evidence, label)?.data_sha256)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChainResolutionCloseV1 {
    NeedsReceiptPrepay {
        observation: Observation,
        payer: ObservedAccount,
        receipt: ObservedAccount,
        exact_receipt_rent: u64,
        prestate: Vec<ObservedAccount>,
    },
    Submit {
        stage: ChainDerivedTerminalMutationV1,
        snapshot: Box<ResolutionCloseFundSnapshotV3>,
    },
}

/// Build Resolution CloseFund (or its exact receipt-rent prerequisite) from
/// one complete finalized snapshot. The role request and request-bound caller
/// remain owned by Resolution's existing semantic operator.
pub(crate) fn plan_resolution_close_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market_key: Pubkey,
    payer: Pubkey,
    additional_keys: &[Pubkey],
) -> Result<ChainResolutionCloseV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let preliminary = finalized_snapshot(rpc, &[market_key])?;
    let preliminary_market = decode_routed_market(preliminary.account(market_key)?, core, plan)?;
    if preliminary_market.phase != Phase::Retiring {
        return Err(refusal(
            "Resolution CloseFund requires the exact Retiring Core Market",
        ));
    }
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market_key.as_ref(),
            &preliminary_market.identity.generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let source_material = routed_record(
        evidence,
        "source_material_record",
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let capability_manifest = routed_record(
        evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let explicit_recovery = evidence.accounts.contains_key("recovery_policy_record");
    let recovery_policy = if explicit_recovery {
        routed_record(
            evidence,
            "recovery_policy_record",
            registry,
            RECOVERY_POLICY_SCHEMA_ID_V2,
        )?
    } else {
        routed_record(
            evidence,
            "source_material_record",
            registry,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        )?
    };
    let source_route = finalized_snapshot(rpc, &[source_state])?;
    let routed_source = source_route.account(source_state)?;
    let source = SourceResolutionStateV2::decode(&routed_source.data)
        .map_err(|error| Error::new(format!("Resolution Source state: {error:?}")))?;
    if !matches!(
        source.phase(),
        SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
    ) {
        return Err(refusal(
            "Resolution Source is not resolved/failure-committed for CloseFund",
        ));
    }
    let terminal = source
        .terminal_projection()
        .map_err(|error| Error::new(format!("Resolution terminal projection: {error:?}")))?;
    let closure_sequence = terminal
        .terminal_sequence()
        .checked_add(1)
        .ok_or_else(|| refusal("Resolution closure sequence overflowed"))?;
    let closure_receipt = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &closure_sequence.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let certificate = preliminary_market
        .terminal_receipt
        .ok_or_else(|| refusal("Retiring Market omitted terminal receipt"))?;
    let certificate = Pubkey::new_from_array(certificate.to_bytes());
    let beneficiary = Pubkey::new_from_array(preliminary_market.rent_beneficiary.to_bytes());
    let funding_ledger = evidence_pubkey(evidence, "founding_funding_ledger_v2_0")?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let resolution_programdata = pubkey(&plan.resolution.programdata_id)?;
    let activation = pubkey(&plan.activation)?;
    let mut keys = vec![
        payer,
        market_key,
        activation,
        registry,
        core,
        core_programdata,
        resolution,
        resolution_programdata,
        source_material.raw,
        source_material.staging,
        capability_manifest.raw,
        capability_manifest.staging,
        source_state,
        funding_ledger,
        certificate,
        closure_receipt,
        beneficiary,
        sysvar::clock::ID,
        sysvar::rent::ID,
        system_program::ID,
        recovery_policy.raw,
        recovery_policy.staging,
    ];
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let market_account = account(market_key, "Resolution close Market")?;
    let market = decode_routed_market(&market_account, core, plan)?;
    if market != preliminary_market || market.phase != Phase::Retiring {
        return Err(refusal(
            "Resolution close Market changed between coordinate discovery and full snapshot",
        ));
    }
    let close_snapshot = ResolutionCloseFundSnapshotV3 {
        market: market_account,
        activation_cache: account(activation, "Resolution close activation cache")?,
        registry_program: account(registry, "Resolution close Registry")?,
        core_program: account(core, "Resolution close Core")?,
        core_programdata: account(core_programdata, "Resolution close Core ProgramData")?,
        resolution_program: account(resolution, "Resolution program")?,
        resolution_programdata: account(resolution_programdata, "Resolution ProgramData")?,
        source_material: account(source_material.raw, "SourceMaterial")?,
        source_material_staging: account(source_material.staging, "SourceMaterial staging")?,
        capability_manifest: account(capability_manifest.raw, "capability manifest")?,
        capability_manifest_staging: account(
            capability_manifest.staging,
            "capability manifest staging",
        )?,
        source_state: account(source_state, "Resolution Source")?,
        funding_ledger: account(funding_ledger, "Resolution FundingLedger")?,
        certificate: account(certificate, "Resolution certificate")?,
        closure_destination: account(closure_receipt, "Resolution closure receipt")?,
        beneficiary: account(beneficiary, "Resolution beneficiary")?,
        clock_sysvar: account(sysvar::clock::ID, "Clock sysvar")?,
        rent_sysvar: account(sysvar::rent::ID, "Rent sysvar")?,
        system_program: account(system_program::ID, "System Program")?,
        recovery_policy: account(recovery_policy.raw, "RecoveryPolicy")?,
        recovery_policy_staging: account(recovery_policy.staging, "RecoveryPolicy staging")?,
    };
    let rent: Rent = bincode::deserialize(&close_snapshot.rent_sysvar.data)
        .map_err(|error| Error::new(format!("Rent sysvar: {error}")))?;
    let exact_receipt_rent = rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    if close_snapshot.closure_destination.owner != system_program::ID
        || close_snapshot.closure_destination.executable
        || !close_snapshot.closure_destination.data.is_empty()
        || close_snapshot.closure_destination.lamports > exact_receipt_rent
    {
        return Err(refusal(
            "Resolution closure receipt was not the exact vacant at-most-rent destination",
        ));
    }
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    if close_snapshot.closure_destination.lamports < exact_receipt_rent {
        return Ok(ChainResolutionCloseV1::NeedsReceiptPrepay {
            observation: snapshot.observation,
            payer: account(payer, "Resolution prepay payer")?,
            receipt: close_snapshot.closure_destination,
            exact_receipt_rent,
            prestate,
        });
    }
    let report = build_resolution_close_fund_v3(&close_snapshot)
        .map_err(|error| Error::new(format!("Resolution CloseFund: {error:?}")))?;
    let closure = resolution_close_meta_closure_v1(&ResolutionCloseMetaCoordinatesV1 {
        release_set: hex32(&plan.release_set_id)?,
        role_request_digest: report.role_request_digest,
        market: market_key,
        activation_cache: activation,
        registry_program: registry,
        core_program: core,
        core_programdata,
        resolution_program: resolution,
        resolution_programdata,
        source_material: source_material.raw,
        source_material_staging: source_material.staging,
        capability_manifest: capability_manifest.raw,
        capability_manifest_staging: capability_manifest.staging,
        source_state,
        funding_ledger,
        certificate,
        closure_receipt,
        beneficiary,
        clock_sysvar: sysvar::clock::ID,
        rent_sysvar: sysvar::rent::ID,
        system_program: system_program::ID,
        recovery_policy: explicit_recovery
            .then_some((recovery_policy.raw, recovery_policy.staging)),
    })?;
    let mutation = plan_resolution_close_caller_v1(&close_snapshot, &closure)?;
    Ok(ChainResolutionCloseV1::Submit {
        stage: ChainDerivedTerminalMutationV1 {
            mutation,
            closure,
            prestate,
        },
        snapshot: Box::new(close_snapshot),
    })
}

pub(crate) fn plan_direct_begin_retiring_caller_v1(
    snapshot: &DirectBeginRetiringSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<DirectBeginRetiringCallerPlanV1> {
    match plan_direct_begin_retiring_v1(snapshot)
        .map_err(|error| Error::new(format!("Direct BeginRetiring: {error:?}")))?
    {
        DirectBeginRetiringPlanV1::Submit(report) => {
            let fresh_closure = TerminalMetaClosureV1 {
                stage: TerminalStageV1::DirectBeginRetiring,
                program_id: report.meta_closure.program_id,
                program_class: TerminalAddressClassV1::InlineProgram,
                accounts: report.meta_closure.accounts.to_vec(),
                classes: report
                    .meta_closure
                    .classes
                    .into_iter()
                    .map(direct_begin_class)
                    .collect(),
            };
            fresh_closure.authenticate_instruction(&report.instruction)?;
            persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
            let root = ExpectedAccountPoststateV1::exact(
                snapshot.root.key,
                snapshot.root.owner,
                snapshot.root.lamports,
                snapshot.root.executable,
                report.expected_post_root_data.clone(),
            );
            if root.data_digest != report.expected_post_root_digest {
                return Err(refusal(
                    "Direct BeginRetiring report post-root digest disagreed with its bytes",
                ));
            }
            Ok(DirectBeginRetiringCallerPlanV1::Submit(
                TerminalSemanticMutationV1 {
                    stage: TerminalStageV1::DirectBeginRetiring,
                    observation: report.observation,
                    instruction: report.instruction.clone(),
                    expected_return_data: Some(ExpectedReturnDataV1 {
                        producer: report.expected_receipt_producer,
                        body: report.expected_receipt_body.to_vec(),
                    }),
                    expected_accounts: vec![root],
                    protocol_lamport_deltas: BTreeMap::new(),
                },
            ))
        }
        DirectBeginRetiringPlanV1::Complete(report) => {
            let root = ExpectedAccountPoststateV1::exact(
                report.root,
                snapshot.root.owner,
                snapshot.root.lamports,
                snapshot.root.executable,
                report.observed_post_root_data.clone(),
            );
            if root.data_digest != report.observed_post_root_digest {
                return Err(refusal(
                    "Direct BeginRetiring complete digest disagreed with observed root bytes",
                ));
            }
            Ok(DirectBeginRetiringCallerPlanV1::Complete {
                observation: report.observation,
                root,
            })
        }
    }
}

/// Consume Resolution's existing CloseFund semantic owner and project every
/// writable account exactly. The closure receipt bytes are encoded by their
/// protocol contract from the report's authenticated typed facts.
pub(crate) fn plan_resolution_close_caller_v1(
    snapshot: &ResolutionCloseFundSnapshotV3,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<TerminalSemanticMutationV1> {
    let report = build_resolution_close_fund_v3(snapshot)
        .map_err(|error| Error::new(format!("Resolution CloseFund: {error:?}")))?;
    let facts = report.expected_retirement_facts;
    let receipt = SourceClosureReceiptV3 {
        market: facts.market,
        source_state: facts.source_state,
        source_material: facts.source_material,
        capability_manifest: facts.capability_manifest,
        terminal_certificate: facts.terminal_certificate,
        receipt_account: facts.resolution_closure_receipt,
        beneficiary: facts.beneficiary,
        source_state_digest: facts.source_state_digest,
        terminal_certificate_digest: facts.terminal_certificate_digest,
        funding_set_digest: facts.funding_set_digest,
        generation: facts.generation,
        terminal_sequence: facts.terminal_sequence,
        selector: facts.selector,
        source_refund_lamports: facts.source_refund_lamports,
        ledger_remaining_native_principal: facts.ledger_remaining_native_principal,
        ledger_rent_lamports: facts.ledger_rent_lamports,
        ledger_lamport_surplus: facts.ledger_lamport_surplus,
        refund_lamports: facts.refund_lamports,
        closed_at: facts.closed_at,
    }
    .to_bytes()
    .map_err(|error| Error::new(format!("Resolution closure receipt: {error:?}")))?
    .to_vec();
    if report.closure_receipt != snapshot.closure_destination.key
        || report.expected_refund_lamports != facts.refund_lamports
        || snapshot
            .beneficiary
            .lamports
            .checked_add(report.expected_refund_lamports)
            .is_none()
    {
        return Err(refusal(
            "Resolution close receipt coordinate or refund arithmetic disagreed",
        ));
    }
    let expected_beneficiary_lamports = snapshot
        .beneficiary
        .lamports
        .checked_add(report.expected_refund_lamports)
        .ok_or_else(|| refusal("Resolution close beneficiary overflowed"))?;
    let fresh_closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        program_id: report.instruction.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        classes: match report.instruction.accounts.len() {
            20 => resolution_close_meta_classes_v1(false),
            22 => resolution_close_meta_classes_v1(true),
            _ => {
                return Err(refusal(
                    "Resolution close fresh frame has another width than 20 or 22",
                ));
            }
        },
        accounts: report.instruction.accounts.clone(),
    };
    persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
    fresh_closure.authenticate_instruction(&report.instruction)?;
    Ok(TerminalSemanticMutationV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        observation: report.observation,
        instruction: report.instruction,
        expected_return_data: None,
        expected_accounts: vec![
            ExpectedAccountPoststateV1::exact(
                snapshot.market.key,
                snapshot.market.owner,
                snapshot.market.lamports,
                snapshot.market.executable,
                snapshot.market.data.clone(),
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.source_state.key,
                solana_sdk_ids::system_program::ID,
                0,
                false,
                Vec::new(),
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.funding_ledger.key,
                solana_sdk_ids::system_program::ID,
                0,
                false,
                Vec::new(),
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.closure_destination.key,
                snapshot.resolution_program.key,
                snapshot.closure_destination.lamports,
                false,
                receipt,
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.beneficiary.key,
                snapshot.beneficiary.owner,
                expected_beneficiary_lamports,
                snapshot.beneficiary.executable,
                snapshot.beneficiary.data.clone(),
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (snapshot.market.key, 0),
            (
                snapshot.source_state.key,
                -i128::from(snapshot.source_state.lamports),
            ),
            (
                snapshot.funding_ledger.key,
                -i128::from(snapshot.funding_ledger.lamports),
            ),
            (snapshot.closure_destination.key, 0),
            (
                snapshot.beneficiary.key,
                i128::from(report.expected_refund_lamports),
            ),
        ]),
    })
}

/// Consume the production F=2 Direct close semantic owner. The selected
/// Trading ledger and root close, the Resolution dependency ledger is preserved
/// exactly, and only the two refunded accounts credit RentCredit.
pub(crate) fn plan_direct_native_close_caller_v1(
    snapshot: &DirectNativeCloseSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<TerminalSemanticMutationV1> {
    let report = build_direct_native_close_v1(snapshot)
        .map_err(|error| Error::new(format!("Direct native CloseCapability: {error:?}")))?;
    let fresh_closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::DirectCloseCapability,
        program_id: report.meta_closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: report.meta_closure.accounts.clone(),
        classes: report
            .meta_closure
            .classes
            .iter()
            .copied()
            .map(terminal_owner_class)
            .collect(),
    };
    persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
    fresh_closure.authenticate_instruction(&report.instruction)?;
    let market = ExpectedAccountPoststateV1::exact(
        snapshot.market.key,
        report.expected_market_owner,
        report.expected_market_lamports,
        snapshot.market.executable,
        report.expected_market_data.clone(),
    );
    if market.data_digest != report.expected_market_digest
        || report.expected_outstanding_capabilities != 0
        || report.rent_credit_delta_lamports
            != report
                .root_refund_lamports
                .checked_add(report.funding_refund_lamports)
                .ok_or_else(|| refusal("Direct close refund arithmetic overflowed"))?
        || report.expected_rent_credit_lamports
            != report
                .rent_credit_pre_lamports
                .checked_add(report.rent_credit_delta_lamports)
                .ok_or_else(|| refusal("Direct close RentCredit arithmetic overflowed"))?
        || report.preserved_dependency_ledgers.len() != 1
    {
        return Err(refusal(
            "production Direct close byte, F=2 dependency, capability, or refund arithmetic disagreed",
        ));
    }
    let preserved = report
        .preserved_dependency_ledgers
        .into_iter()
        .map(|ledger| {
            let expected = ExpectedAccountPoststateV1::exact(
                ledger.key,
                ledger.owner,
                ledger.lamports,
                ledger.executable,
                ledger.data,
            );
            if expected.data_digest != ledger.data_digest {
                return Err(refusal(
                    "Direct close preserved dependency digest disagreed with its bytes",
                ));
            }
            Ok(expected)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut expected_accounts = vec![
        market,
        ExpectedAccountPoststateV1::exact(
            report.closed_root,
            report.expected_root_owner,
            report.expected_root_lamports,
            false,
            report.expected_root_data,
        ),
        ExpectedAccountPoststateV1::exact(
            report.closed_funding_ledger,
            report.expected_funding_owner,
            report.expected_funding_lamports,
            false,
            report.expected_funding_data,
        ),
        ExpectedAccountPoststateV1::exact(
            report.rent_credit,
            report.expected_rent_credit_owner,
            report.expected_rent_credit_lamports,
            snapshot.rent_credit.executable,
            report.expected_rent_credit_data,
        ),
    ];
    expected_accounts.extend(preserved);
    Ok(TerminalSemanticMutationV1 {
        stage: TerminalStageV1::DirectCloseCapability,
        observation: report.observation,
        instruction: report.instruction,
        expected_return_data: None,
        expected_accounts,
        protocol_lamport_deltas: BTreeMap::from([
            (snapshot.market.key, 0),
            (report.closed_root, -i128::from(report.root_refund_lamports)),
            (
                report.closed_funding_ledger,
                -i128::from(report.funding_refund_lamports),
            ),
            (
                report.rent_credit,
                i128::from(report.rent_credit_delta_lamports),
            ),
        ]),
    })
}

/// Discover the request-bound Core caller from a fully authenticated preflight,
/// then reacquire every role plus that exact PDA in one newer finalized
/// snapshot. No fabricated vacant account reaches the submission builder.
pub(crate) fn plan_direct_native_close_two_pass_from_chain_v1(
    rpc: &mut Rpc,
    discovery: &DirectNativeCloseSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
    additional_keys: &[Pubkey],
) -> Result<ChainDerivedTerminalMutationV1> {
    let preflight = preflight_direct_native_close_caller_v1(discovery)
        .map_err(|error| Error::new(format!("Direct close caller preflight: {error:?}")))?;
    let mut keys = direct_native_close_snapshot_keys_v1(discovery);
    keys.push(preflight.caller_authority);
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let full = DirectNativeCloseSnapshotV1 {
        market: account(discovery.market.key, "Direct close Market")?,
        realm: account(discovery.realm.key, "Direct close Realm")?,
        realm_staging: account(discovery.realm_staging.key, "Direct close Realm staging")?,
        manifest: account(discovery.manifest.key, "Direct close manifest")?,
        manifest_staging: account(
            discovery.manifest_staging.key,
            "Direct close manifest staging",
        )?,
        funding_ledgers: discovery
            .funding_ledgers
            .iter()
            .map(|ledger| account(ledger.key, "Direct close funding ledger"))
            .collect::<Result<Vec<_>>>()?,
        root: account(discovery.root.key, "Direct close root")?,
        activation_cache: account(
            discovery.activation_cache.key,
            "Direct close activation cache",
        )?,
        core_program: account(discovery.core_program.key, "Direct close Core program")?,
        core_programdata: account(
            discovery.core_programdata.key,
            "Direct close Core ProgramData",
        )?,
        trading_program: account(
            discovery.trading_program.key,
            "Direct close Trading program",
        )?,
        trading_programdata: account(
            discovery.trading_programdata.key,
            "Direct close Trading ProgramData",
        )?,
        resolution_program: account(
            discovery.resolution_program.key,
            "Direct close Resolution program",
        )?,
        resolution_programdata: account(
            discovery.resolution_programdata.key,
            "Direct close Resolution ProgramData",
        )?,
        registry_program: account(
            discovery.registry_program.key,
            "Direct close Registry program",
        )?,
        rent_sysvar: account(discovery.rent_sysvar.key, "Direct close Rent sysvar")?,
        caller_authority: Some(account(
            preflight.caller_authority,
            "Direct close request-bound caller",
        )?),
        program_set: account(discovery.program_set.key, "Direct close ProgramSet")?,
        program_set_staging: account(
            discovery.program_set_staging.key,
            "Direct close ProgramSet staging",
        )?,
        config: account(discovery.config.key, "Direct close config")?,
        config_staging: account(discovery.config_staging.key, "Direct close config staging")?,
        close_profile: account(discovery.close_profile.key, "Direct close AccountProfile")?,
        close_profile_staging: account(
            discovery.close_profile_staging.key,
            "Direct close AccountProfile staging",
        )?,
        close_effect: account(discovery.close_effect.key, "Direct close Effect")?,
        close_effect_staging: account(
            discovery.close_effect_staging.key,
            "Direct close Effect staging",
        )?,
        system_program: account(discovery.system_program.key, "System Program")?,
        close_descriptor: account(discovery.close_descriptor.key, "Direct close descriptor")?,
        close_descriptor_staging: account(
            discovery.close_descriptor_staging.key,
            "Direct close descriptor staging",
        )?,
        rent_program: account(discovery.rent_program.key, "Rent program")?,
        rent_credit: account(discovery.rent_credit.key, "Direct close RentCredit")?,
    };
    let mutation = plan_direct_native_close_caller_v1(&full, persisted_closure)?;
    let report = build_direct_native_close_v1(&full)
        .map_err(|error| Error::new(format!("Direct close full caller replay: {error:?}")))?;
    if report.caller_authority != preflight.caller_authority
        || report.role_request_digest != preflight.request_digest
        || report.observation != snapshot.observation
    {
        return Err(refusal(
            "Direct close full snapshot changed the preflight request-bound caller identity",
        ));
    }
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok(ChainDerivedTerminalMutationV1 {
        mutation,
        closure: persisted_closure.clone(),
        prestate,
    })
}

pub(crate) fn direct_native_close_discovery_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
) -> Result<DirectNativeCloseSnapshotV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let realm = routed_record(
        evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let manifest = routed_record(
        evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let program_set = routed_record(
        evidence,
        "direct_program_set_record",
        registry,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    )?;
    let config = routed_record(
        evidence,
        "direct_execution_config_record",
        registry,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
    )?;
    let close_profile = routed_record(
        evidence,
        "direct_native_close_account_profile_record",
        registry,
        direct_native_close_account_profile_schema_v1(),
    )?;
    let close_effect = routed_record(
        evidence,
        "direct_native_close_effect_record",
        registry,
        direct_native_close_effect_schema_v1(),
    )?;
    let close_descriptor = routed_record(
        evidence,
        "direct_native_close_descriptor_record",
        registry,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
    )?;
    let coordinates = [
        market,
        realm.raw,
        realm.staging,
        manifest.raw,
        manifest.staging,
        evidence_pubkey(evidence, "founding_funding_ledger_v2_0")?,
        evidence_pubkey(evidence, "direct_trading_funding_ledger")?,
        evidence_pubkey(evidence, "direct_capability_root")?,
        pubkey(&plan.activation)?,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        pubkey(&plan.trading.program_id)?,
        pubkey(&plan.trading.programdata_id)?,
        pubkey(&plan.resolution.program_id)?,
        pubkey(&plan.resolution.programdata_id)?,
        registry,
        sysvar::rent::ID,
        program_set.raw,
        program_set.staging,
        config.raw,
        config.staging,
        close_profile.raw,
        close_profile.staging,
        close_effect.raw,
        close_effect.staging,
        system_program::ID,
        close_descriptor.raw,
        close_descriptor.staging,
        pubkey(&plan.rent_credit.program_id)?,
        evidence_pubkey(evidence, "founding_lifecycle_rent_credit")?,
    ];
    let snapshot = finalized_snapshot(rpc, &coordinates)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    Ok(DirectNativeCloseSnapshotV1 {
        market: account(market, "Direct close Market")?,
        realm: account(realm.raw, "Direct close Realm")?,
        realm_staging: account(realm.staging, "Direct close Realm staging")?,
        manifest: account(manifest.raw, "Direct close manifest")?,
        manifest_staging: account(manifest.staging, "Direct close manifest staging")?,
        funding_ledgers: vec![
            account(coordinates[5], "Resolution dependency FundingLedger")?,
            account(coordinates[6], "Trading selected FundingLedger")?,
        ],
        root: account(coordinates[7], "Direct root")?,
        activation_cache: account(coordinates[8], "activation cache")?,
        core_program: account(coordinates[9], "Core program")?,
        core_programdata: account(coordinates[10], "Core ProgramData")?,
        trading_program: account(coordinates[11], "Trading program")?,
        trading_programdata: account(coordinates[12], "Trading ProgramData")?,
        resolution_program: account(coordinates[13], "Resolution program")?,
        resolution_programdata: account(coordinates[14], "Resolution ProgramData")?,
        registry_program: account(registry, "Registry program")?,
        rent_sysvar: account(sysvar::rent::ID, "Rent sysvar")?,
        caller_authority: None,
        program_set: account(program_set.raw, "Direct ProgramSet")?,
        program_set_staging: account(program_set.staging, "Direct ProgramSet staging")?,
        config: account(config.raw, "Direct config")?,
        config_staging: account(config.staging, "Direct config staging")?,
        close_profile: account(close_profile.raw, "Direct close profile")?,
        close_profile_staging: account(close_profile.staging, "Direct close profile staging")?,
        close_effect: account(close_effect.raw, "Direct close effect")?,
        close_effect_staging: account(close_effect.staging, "Direct close effect staging")?,
        system_program: account(system_program::ID, "System Program")?,
        close_descriptor: account(close_descriptor.raw, "Direct close descriptor")?,
        close_descriptor_staging: account(
            close_descriptor.staging,
            "Direct close descriptor staging",
        )?,
        rent_program: account(coordinates[28], "Rent program")?,
        rent_credit: account(coordinates[29], "RentCredit")?,
    })
}

fn direct_native_close_snapshot_keys_v1(snapshot: &DirectNativeCloseSnapshotV1) -> Vec<Pubkey> {
    let mut keys = vec![
        snapshot.market.key,
        snapshot.realm.key,
        snapshot.realm_staging.key,
        snapshot.manifest.key,
        snapshot.manifest_staging.key,
        snapshot.root.key,
        snapshot.activation_cache.key,
        snapshot.core_program.key,
        snapshot.core_programdata.key,
        snapshot.trading_program.key,
        snapshot.trading_programdata.key,
        snapshot.resolution_program.key,
        snapshot.resolution_programdata.key,
        snapshot.registry_program.key,
        snapshot.rent_sysvar.key,
        snapshot.program_set.key,
        snapshot.program_set_staging.key,
        snapshot.config.key,
        snapshot.config_staging.key,
        snapshot.close_profile.key,
        snapshot.close_profile_staging.key,
        snapshot.close_effect.key,
        snapshot.close_effect_staging.key,
        snapshot.system_program.key,
        snapshot.close_descriptor.key,
        snapshot.close_descriptor_staging.key,
        snapshot.rent_program.key,
        snapshot.rent_credit.key,
    ];
    keys.extend(snapshot.funding_ledgers.iter().map(|ledger| ledger.key));
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Consume the Core/Custody handoff semantic owner without reconstructing its
/// 23-role frame, 208-byte request, or 512-byte receipt.
pub(crate) fn plan_retirement_replay_handoff_caller_v1(
    snapshot: &RetirementReplayHandoffSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<TerminalSemanticMutationV1> {
    let report = build_retirement_replay_handoff_v1(snapshot)
        .map_err(|error| Error::new(format!("retirement replay handoff: {error:?}")))?;
    let fresh_closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::RetirementReplayHandoff,
        program_id: report.meta_closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: report.meta_closure.accounts.clone(),
        classes: report
            .meta_closure
            .classes
            .iter()
            .copied()
            .map(terminal_owner_class)
            .collect(),
    };
    persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
    fresh_closure.authenticate_instruction(&report.instruction)?;
    let replay_rent = report.expected_core_replay_lamports;
    let expected_payer = snapshot
        .payer
        .lamports
        .checked_sub(replay_rent)
        .ok_or_else(|| refusal("replay handoff payer rent debit underflowed"))?;
    let expected_credit = snapshot
        .rent_credit
        .lamports
        .checked_add(report.trading_replay_refund_lamports)
        .ok_or_else(|| refusal("replay handoff RentCredit refund overflowed"))?;
    if expected_payer != report.expected_payer_lamports
        || expected_credit != report.expected_rent_credit_lamports
        || hash(&report.expected_core_replay_data).to_bytes() != report.expected_core_replay_digest
    {
        return Err(refusal(
            "replay handoff byte, payer-rent, or RentCredit arithmetic disagreed",
        ));
    }
    Ok(TerminalSemanticMutationV1 {
        stage: TerminalStageV1::RetirementReplayHandoff,
        observation: report.observation,
        instruction: report.instruction,
        expected_return_data: Some(ExpectedReturnDataV1 {
            producer: snapshot.custody_program.key,
            body: report.expected_receipt_body.to_vec(),
        }),
        expected_accounts: vec![
            ExpectedAccountPoststateV1::exact(
                report.core_replay,
                report.expected_core_replay_owner,
                report.expected_core_replay_lamports,
                report.expected_core_replay_executable,
                report.expected_core_replay_data,
            ),
            ExpectedAccountPoststateV1::exact(
                report.trading_replay,
                report.expected_trading_replay_owner,
                report.expected_trading_replay_lamports,
                false,
                report.expected_trading_replay_data,
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.payer.key,
                report.expected_payer_owner,
                report.expected_payer_lamports,
                snapshot.payer.executable,
                report.expected_payer_data,
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.rent_credit.key,
                report.expected_rent_credit_owner,
                report.expected_rent_credit_lamports,
                snapshot.rent_credit.executable,
                report.expected_rent_credit_data,
            ),
            ExpectedAccountPoststateV1::exact(
                report.hoard,
                report.expected_hoard_owner,
                report.expected_hoard_lamports,
                snapshot.hoard.executable,
                report.expected_hoard_data,
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (snapshot.payer.key, -i128::from(replay_rent)),
            (report.core_replay, i128::from(replay_rent)),
            (
                report.trading_replay,
                -i128::from(report.trading_replay_refund_lamports),
            ),
            (
                snapshot.rent_credit.key,
                i128::from(report.trading_replay_refund_lamports),
            ),
            (report.hoard, 0),
        ]),
    })
}

pub(crate) fn plan_retirement_replay_handoff_two_pass_from_chain_v1(
    rpc: &mut Rpc,
    discovery: &RetirementReplayHandoffSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
    additional_keys: &[Pubkey],
) -> Result<ChainDerivedTerminalMutationV1> {
    let preflight = preflight_retirement_replay_handoff_caller_v1(discovery)
        .map_err(|error| Error::new(format!("replay-handoff caller preflight: {error:?}")))?;
    let mut keys = retirement_replay_handoff_snapshot_keys_v1(discovery);
    keys.push(preflight.caller_authority);
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let full = RetirementReplayHandoffSnapshotV1 {
        payer: account(discovery.payer.key, "handoff payer")?,
        market: account(discovery.market.key, "handoff Market")?,
        activation_cache: account(discovery.activation_cache.key, "handoff activation cache")?,
        registry_program: account(discovery.registry_program.key, "handoff Registry program")?,
        core_program: account(discovery.core_program.key, "handoff Core program")?,
        core_programdata: account(discovery.core_programdata.key, "handoff Core ProgramData")?,
        trading_program: account(discovery.trading_program.key, "handoff Trading program")?,
        trading_programdata: account(
            discovery.trading_programdata.key,
            "handoff Trading ProgramData",
        )?,
        custody_program: account(discovery.custody_program.key, "handoff Custody program")?,
        custody_programdata: account(
            discovery.custody_programdata.key,
            "handoff Custody ProgramData",
        )?,
        caller_authority: Some(account(
            preflight.caller_authority,
            "handoff request-bound caller",
        )?),
        claims_aggregate: account(discovery.claims_aggregate.key, "handoff Claims aggregate")?,
        realm: account(discovery.realm.key, "handoff Realm")?,
        realm_staging: account(discovery.realm_staging.key, "handoff Realm staging")?,
        rent_sysvar: account(discovery.rent_sysvar.key, "handoff Rent sysvar")?,
        rent_credit: account(discovery.rent_credit.key, "handoff RentCredit")?,
        trading_replay: account(discovery.trading_replay.key, "handoff Trading replay")?,
        core_replay: account(discovery.core_replay.key, "handoff Core replay")?,
        hoard: account(discovery.hoard.key, "handoff Hoard")?,
        system_program: account(discovery.system_program.key, "handoff System Program")?,
        mint: account(discovery.mint.key, "handoff collateral Mint")?,
        token_program: account(discovery.token_program.key, "handoff token program")?,
        custody_authority: account(discovery.custody_authority.key, "handoff Custody authority")?,
    };
    let mutation = plan_retirement_replay_handoff_caller_v1(&full, persisted_closure)?;
    let report = build_retirement_replay_handoff_v1(&full)
        .map_err(|error| Error::new(format!("replay-handoff full caller replay: {error:?}")))?;
    if report.caller_authority != preflight.caller_authority
        || report.request_digest != preflight.request_digest
        || report.observation != snapshot.observation
    {
        return Err(refusal(
            "replay-handoff full snapshot changed the preflight request-bound caller identity",
        ));
    }
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok(ChainDerivedTerminalMutationV1 {
        mutation,
        closure: persisted_closure.clone(),
        prestate,
    })
}

pub(crate) fn retirement_replay_handoff_discovery_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    payer: Pubkey,
) -> Result<RetirementReplayHandoffSnapshotV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let release = hex32(&plan.release_set_id)?;
    let context = hex32(&evidence.founding_custody_context)?;
    let realm = routed_record(
        evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let claims_aggregate = evidence_pubkey(evidence, "claims_aggregate")?;
    let trading_replay = evidence_pubkey(evidence, "founding_normal_custody_replay")?;
    let core_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(market.to_bytes(), release, ExecutionRoleV1::Core, context)
            .as_slices(),
        &custody,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release,
            context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody,
    )
    .0;
    if hoard != evidence_pubkey(evidence, "founding_hoard_vault_open")? {
        return Err(refusal(
            "replay-handoff Hoard evidence was not canonical for the authenticated custody context",
        ));
    }
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release).as_slices(),
        &custody,
    )
    .0;
    let mint = evidence_pubkey(evidence, "collateral_mint")?;
    let token_program = pubkey(&required_account(evidence, "collateral_mint")?.owner)?;
    let keys = [
        payer,
        market,
        pubkey(&plan.activation)?,
        registry,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        pubkey(&plan.trading.program_id)?,
        pubkey(&plan.trading.programdata_id)?,
        custody,
        pubkey(&plan.custody.programdata_id)?,
        claims_aggregate,
        realm.raw,
        realm.staging,
        sysvar::rent::ID,
        evidence_pubkey(evidence, "founding_lifecycle_rent_credit")?,
        trading_replay,
        core_replay,
        hoard,
        system_program::ID,
        mint,
        token_program,
        authority,
    ];
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    Ok(RetirementReplayHandoffSnapshotV1 {
        payer: account(payer, "handoff payer")?,
        market: account(market, "handoff Market")?,
        activation_cache: account(keys[2], "handoff activation cache")?,
        registry_program: account(registry, "handoff Registry program")?,
        core_program: account(keys[4], "handoff Core program")?,
        core_programdata: account(keys[5], "handoff Core ProgramData")?,
        trading_program: account(keys[6], "handoff Trading program")?,
        trading_programdata: account(keys[7], "handoff Trading ProgramData")?,
        custody_program: account(custody, "handoff Custody program")?,
        custody_programdata: account(keys[9], "handoff Custody ProgramData")?,
        caller_authority: None,
        claims_aggregate: account(claims_aggregate, "handoff Claims aggregate")?,
        realm: account(realm.raw, "handoff Realm")?,
        realm_staging: account(realm.staging, "handoff Realm staging")?,
        rent_sysvar: account(sysvar::rent::ID, "handoff Rent sysvar")?,
        rent_credit: account(keys[14], "handoff RentCredit")?,
        trading_replay: account(trading_replay, "handoff Trading replay")?,
        core_replay: account(core_replay, "handoff Core replay")?,
        hoard: account(hoard, "handoff Hoard")?,
        system_program: account(system_program::ID, "handoff System Program")?,
        mint: account(mint, "handoff collateral Mint")?,
        token_program: account(token_program, "handoff token program")?,
        custody_authority: account(authority, "handoff Custody authority")?,
    })
}

fn retirement_replay_handoff_snapshot_keys_v1(
    snapshot: &RetirementReplayHandoffSnapshotV1,
) -> Vec<Pubkey> {
    let mut keys = vec![
        snapshot.payer.key,
        snapshot.market.key,
        snapshot.activation_cache.key,
        snapshot.registry_program.key,
        snapshot.core_program.key,
        snapshot.core_programdata.key,
        snapshot.trading_program.key,
        snapshot.trading_programdata.key,
        snapshot.custody_program.key,
        snapshot.custody_programdata.key,
        snapshot.claims_aggregate.key,
        snapshot.realm.key,
        snapshot.realm_staging.key,
        snapshot.rent_sysvar.key,
        snapshot.rent_credit.key,
        snapshot.trading_replay.key,
        snapshot.core_replay.key,
        snapshot.hoard.key,
        snapshot.system_program.key,
        snapshot.mint.key,
        snapshot.token_program.key,
        snapshot.custody_authority.key,
    ];
    if let Some(caller) = &snapshot.caller_authority {
        keys.push(caller.key);
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Consume the existing aggregate-retirement semantic owner and project the
/// complete physical closure/refund poststate. The persisted initial closure
/// must still equal the fresh Registry-wrapped 46-meta report exactly.
pub(crate) fn plan_aggregate_retirement_caller_v1(
    snapshot: &MarketRetirementSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<TerminalSemanticMutationV1> {
    let report = build_market_retirement_v1(snapshot)
        .map_err(|error| Error::new(format!("aggregate Market retirement: {error:?}")))?;
    let fresh_closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::AggregateRetirement,
        program_id: report.instruction.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: report.instruction.accounts.clone(),
        classes: aggregate_retirement_meta_classes_v1(),
    };
    persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
    fresh_closure.authenticate_instruction(&report.instruction)?;
    let expected_refund_wallet_lamports = snapshot
        .refund_wallet
        .lamports
        .checked_add(report.expected_refund_delta)
        .ok_or_else(|| refusal("aggregate retirement refund wallet overflowed"))?;
    let closed = |account: &ObservedAccount| {
        ExpectedAccountPoststateV1::exact(
            account.key,
            solana_sdk_ids::system_program::ID,
            0,
            false,
            Vec::new(),
        )
    };
    Ok(TerminalSemanticMutationV1 {
        stage: TerminalStageV1::AggregateRetirement,
        observation: report.observation,
        instruction: report.instruction,
        expected_return_data: None,
        expected_accounts: vec![
            closed(&snapshot.market),
            closed(&snapshot.rent_credit),
            closed(&snapshot.claims_aggregate),
            closed(&snapshot.custody_replay),
            closed(&snapshot.hoard_vault),
            ExpectedAccountPoststateV1::exact(
                snapshot.refund_wallet.key,
                snapshot.refund_wallet.owner,
                expected_refund_wallet_lamports,
                snapshot.refund_wallet.executable,
                snapshot.refund_wallet.data.clone(),
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (snapshot.market.key, -i128::from(snapshot.market.lamports)),
            (
                snapshot.rent_credit.key,
                -i128::from(snapshot.rent_credit.lamports),
            ),
            (
                snapshot.claims_aggregate.key,
                -i128::from(snapshot.claims_aggregate.lamports),
            ),
            (
                snapshot.custody_replay.key,
                -i128::from(snapshot.custody_replay.lamports),
            ),
            (
                snapshot.hoard_vault.key,
                -i128::from(snapshot.hoard_vault.lamports),
            ),
            (
                snapshot.refund_wallet.key,
                i128::from(report.expected_refund_delta),
            ),
        ]),
    })
}

pub(crate) fn aggregate_retirement_snapshot_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    source_receipt: Pubkey,
    additional_keys: &[Pubkey],
) -> Result<(MarketRetirementSnapshotV1, Vec<ObservedAccount>)> {
    let registry = pubkey(&plan.registry.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let release = hex32(&plan.release_set_id)?;
    let context = hex32(&evidence.founding_custody_context)?;
    let rent_credit = evidence_pubkey(evidence, "founding_lifecycle_rent_credit")?;
    let preliminary = finalized_snapshot(rpc, &[rent_credit])?;
    let credit_account = preliminary.account(rent_credit)?.clone();
    let credit = LifecycleRentCreditV2::decode(&credit_account.data)
        .map_err(|error| Error::new(format!("aggregate RentCredit: {error:?}")))?;
    let refund_wallet = Pubkey::new_from_array(credit.refund_wallet().to_bytes());
    let realm = routed_record(
        evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let registry_artifact = plan_record_pair_v1(plan, "registry_artifact_release")?;
    let rent_artifact = plan_record_pair_v1(plan, "rent_artifact_release")?;
    let core_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(market.to_bytes(), release, ExecutionRoleV1::Core, context)
            .as_slices(),
        &custody,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release,
            context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release).as_slices(),
        &custody,
    )
    .0;
    let mint = evidence_pubkey(evidence, "collateral_mint")?;
    let token_program = pubkey(&required_account(evidence, "collateral_mint")?.owner)?;
    let mut keys = vec![
        market,
        rent_credit,
        pubkey(&plan.activation)?,
        registry,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        pubkey(&plan.claims.program_id)?,
        pubkey(&plan.claims.programdata_id)?,
        pubkey(&plan.resolution.program_id)?,
        pubkey(&plan.resolution.programdata_id)?,
        custody,
        pubkey(&plan.custody.programdata_id)?,
        pubkey(&plan.rent_credit.program_id)?,
        source_receipt,
        evidence_pubkey(evidence, "claims_aggregate")?,
        core_replay,
        hoard,
        authority,
        mint,
        token_program,
        realm.raw,
        realm.staging,
        pubkey(&plan.infrastructure_profile.address)?,
        registry_artifact.0,
        registry_artifact.1,
        pubkey(&plan.registry.programdata_id)?,
        rent_artifact.0,
        rent_artifact.1,
        pubkey(&plan.rent_credit.programdata_id)?,
        sysvar::rent::ID,
        refund_wallet,
    ];
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |index: usize, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(keys[index])
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let retirement = MarketRetirementSnapshotV1 {
        market: account(0, "aggregate Market")?,
        rent_credit: account(1, "aggregate RentCredit")?,
        activation_cache: account(2, "aggregate activation cache")?,
        registry_program: account(3, "aggregate Registry program")?,
        core_program: account(4, "aggregate Core program")?,
        core_programdata: account(5, "aggregate Core ProgramData")?,
        claims_program: account(6, "aggregate Claims program")?,
        claims_programdata: account(7, "aggregate Claims ProgramData")?,
        resolution_program: account(8, "aggregate Resolution program")?,
        resolution_programdata: account(9, "aggregate Resolution ProgramData")?,
        custody_program: account(10, "aggregate Custody program")?,
        custody_programdata: account(11, "aggregate Custody ProgramData")?,
        rent_program: account(12, "aggregate Rent program")?,
        source_receipt: account(13, "aggregate Source receipt")?,
        claims_aggregate: account(14, "aggregate Claims aggregate")?,
        custody_replay: account(15, "aggregate Core Custody replay")?,
        hoard_vault: account(16, "aggregate Hoard vault")?,
        custody_authority: account(17, "aggregate Custody authority")?,
        collateral_mint: account(18, "aggregate collateral Mint")?,
        collateral_token_program: account(19, "aggregate token program")?,
        realm_raw: account(20, "aggregate Realm")?,
        realm_staging: account(21, "aggregate Realm staging")?,
        infrastructure_profile: account(22, "aggregate infrastructure profile")?,
        registry_artifact_raw: account(23, "aggregate Registry ArtifactRelease")?,
        registry_artifact_staging: account(24, "aggregate Registry ArtifactRelease staging")?,
        registry_programdata: account(25, "aggregate Registry ProgramData")?,
        rent_artifact_raw: account(26, "aggregate Rent ArtifactRelease")?,
        rent_artifact_staging: account(27, "aggregate Rent ArtifactRelease staging")?,
        rent_programdata: account(28, "aggregate Rent ProgramData")?,
        rent_sysvar: account(29, "aggregate Rent sysvar")?,
        refund_wallet: account(30, "aggregate refund wallet")?,
    };
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok((retirement, prestate))
}

fn plan_record_pair_v1(plan: &SuccessorPlan, label: &str) -> Result<(Pubkey, Pubkey)> {
    let pair = plan
        .records
        .get(label)
        .ok_or_else(|| Error::new(format!("successor plan omitted {label}")))?;
    Ok((pubkey(&pair.raw)?, pubkey(&pair.staging)?))
}

/// Project the six coordinate closures used only to derive the immutable ALT
/// stable-key union. Request-bound coordinates use nonzero planning
/// commitments and are deliberately excluded from the table; every eventual
/// stage still supplies and authenticates its fresh exact request-bound frame.
pub(crate) fn project_terminal_lookup_closures_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    payer: Pubkey,
    pinned_source_receipt: Option<Pubkey>,
) -> Result<(Vec<TerminalMetaClosureV1>, Pubkey)> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let activation = pubkey(&plan.activation)?;
    let release_set = hex32(&plan.release_set_id)?;
    let context = hex32(&evidence.founding_custody_context)?;
    let root = evidence_pubkey(evidence, "direct_capability_root")?;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &market_input.generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let rent_credit = evidence_pubkey(evidence, "founding_lifecycle_rent_credit")?;
    let initial = finalized_snapshot(rpc, &[market, source_state, rent_credit])?;
    let state = decode_routed_market(initial.account(market)?, core, plan)?;
    if state.identity.generation != market_input.generation
        || !matches!(state.phase, Phase::Terminal | Phase::Retiring)
    {
        return Err(refusal(
            "terminal ALT projection requires the exact terminal/retiring founding Market generation",
        ));
    }
    let source_receipt = match pinned_source_receipt {
        Some(receipt) => receipt,
        None => {
            let source = SourceResolutionStateV2::decode(&initial.account(source_state)?.data)
                .map_err(|error| Error::new(format!("terminal ALT Source state: {error:?}")))?;
            let terminal = source.terminal_projection().map_err(|error| {
                Error::new(format!("terminal ALT Source projection: {error:?}"))
            })?;
            let closure_sequence = terminal
                .terminal_sequence()
                .checked_add(1)
                .ok_or_else(|| refusal("terminal ALT Resolution closure sequence overflowed"))?;
            Pubkey::find_program_address(
                &[
                    SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
                    source_state.as_ref(),
                    &closure_sequence.to_le_bytes(),
                ],
                &resolution,
            )
            .0
        }
    };
    let certificate = Pubkey::new_from_array(
        state
            .terminal_receipt
            .ok_or_else(|| refusal("terminal ALT Market omitted terminal receipt"))?
            .to_bytes(),
    );
    let beneficiary = Pubkey::new_from_array(state.rent_beneficiary.to_bytes());
    let source_material = routed_record(
        evidence,
        "source_material_record",
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let manifest = routed_record(
        evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let recovery = evidence
        .accounts
        .contains_key("recovery_policy_record")
        .then(|| {
            routed_record(
                evidence,
                "recovery_policy_record",
                registry,
                RECOVERY_POLICY_SCHEMA_ID_V2,
            )
        })
        .transpose()?;
    if market_input.direct_capability.is_none() {
        return Err(refusal(
            "terminal ALT projection omitted Direct capability payload",
        ));
    }
    let direct_manifest = evidence_digest(evidence, "capability_manifest_record")?;
    let program_set_digest = evidence_digest(evidence, "direct_program_set_record")?;
    let config_digest = evidence_digest(evidence, "direct_execution_config_record")?;
    let begin_context = direct_begin_retiring_context_v1(
        release_set,
        market.to_bytes(),
        root.to_bytes(),
        direct_manifest,
        program_set_digest,
        config_digest,
        market_input.generation,
        evidence.direct_selected_manifest_entry_index,
    );
    let begin = direct_begin_retiring_meta_closure_v1(DirectBeginRetiringCoordinateInputV1 {
        request: DirectBeginRetiringRequestV1 {
            release_set,
            market: market.to_bytes(),
            context: begin_context,
            root: root.to_bytes(),
            manifest: direct_manifest,
            program_set: program_set_digest,
            config: config_digest,
            expected_market_digest: [1; 32],
            expected_root_digest: [2; 32],
            generation: market_input.generation,
            entry_index: evidence.direct_selected_manifest_entry_index,
        },
        descriptor: evidence_digest(evidence, "direct_begin_retiring_descriptor_record")?,
        account_profile: evidence_digest(evidence, "direct_begin_retiring_account_profile_record")?,
        effect: evidence_digest(evidence, "direct_begin_retiring_effect_record")?,
        registry_program: registry,
        core_program: core,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        trading_program: trading,
        trading_programdata: pubkey(&plan.trading.programdata_id)?,
    })?;
    let close_profile = routed_record(
        evidence,
        "direct_native_close_account_profile_record",
        registry,
        direct_native_close_account_profile_schema_v1(),
    )?;
    let close_effect = routed_record(
        evidence,
        "direct_native_close_effect_record",
        registry,
        direct_native_close_effect_schema_v1(),
    )?;
    let close_descriptor = routed_record(
        evidence,
        "direct_native_close_descriptor_record",
        registry,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
    )?;
    let program_set = routed_record(
        evidence,
        "direct_program_set_record",
        registry,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    )?;
    let config = routed_record(
        evidence,
        "direct_execution_config_record",
        registry,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
    )?;
    let realm = routed_record(
        evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let record = |pair: &crate::wallet_terminal::RecordPairV1| TerminalRecordCoordinatesV1 {
        raw: pair.raw,
        staging: pair.staging,
    };
    let deployment = |program: &crate::model::ProgramPin| -> Result<_> {
        Ok(TerminalDeploymentCoordinatesV1 {
            program: pubkey(&program.program_id)?,
            programdata: pubkey(&program.programdata_id)?,
        })
    };
    let resolution_funding = evidence_pubkey(evidence, "founding_funding_ledger_v2_0")?;
    let trading_funding = evidence_pubkey(evidence, "direct_trading_funding_ledger")?;
    let close = direct_native_close_meta_closure_v1(&DirectNativeCloseCoordinateInputV1 {
        release_set,
        role_request_digest: [4; 32],
        market,
        realm: record(&realm),
        manifest: record(&manifest),
        resolution_funding,
        trading_funding,
        root,
        activation_cache: activation,
        core: deployment(&plan.core)?,
        trading: deployment(&plan.trading)?,
        resolution: deployment(&plan.resolution)?,
        registry_program: registry,
        rent_sysvar: sysvar::rent::ID,
        program_set: record(&program_set),
        config: record(&config),
        close_profile: record(&close_profile),
        close_effect: record(&close_effect),
        system_program: system_program::ID,
        close_descriptor: record(&close_descriptor),
        rent_program,
        rent_credit,
    })?;
    let trading_replay = evidence_pubkey(evidence, "founding_normal_custody_replay")?;
    let core_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market.to_bytes(),
            release_set,
            ExecutionRoleV1::Core,
            context,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release_set,
            context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release_set).as_slices(),
        &custody,
    )
    .0;
    let collateral_mint = evidence_pubkey(evidence, "collateral_mint")?;
    let token_program = pubkey(&required_account(evidence, "collateral_mint")?.owner)?;
    let handoff =
        retirement_replay_handoff_meta_closure_v1(&RetirementReplayHandoffCoordinateInputV1 {
            release_set,
            context,
            request_digest: [5; 32],
            payer,
            market,
            activation_cache: activation,
            registry_program: registry,
            core: deployment(&plan.core)?,
            trading: deployment(&plan.trading)?,
            custody: deployment(&plan.custody)?,
            claims_aggregate: evidence_pubkey(evidence, "claims_aggregate")?,
            realm: record(&realm),
            rent_sysvar: sysvar::rent::ID,
            rent_credit,
            trading_replay,
            core_replay,
            hoard,
            system_program: system_program::ID,
            mint: collateral_mint,
            token_program,
            custody_authority,
        })?;
    let credit = LifecycleRentCreditV2::decode(&initial.account(rent_credit)?.data)
        .map_err(|error| Error::new(format!("terminal ALT RentCredit: {error:?}")))?;
    let refund_wallet = Pubkey::new_from_array(credit.refund_wallet().to_bytes());
    let registry_artifact = plan_record_pair_v1(plan, "registry_artifact_release")?;
    let rent_artifact = plan_record_pair_v1(plan, "rent_artifact_release")?;
    let continuation = RegistryContinuationRequestV1::new(
        ContentId::new(release_set)
            .map_err(|error| Error::new(format!("terminal ALT release identity: {error:?}")))?,
        ContentId::new([6; 32])
            .map_err(|error| Error::new(format!("terminal ALT cache digest: {error:?}")))?,
        ContentId::new([7; 32])
            .map_err(|error| Error::new(format!("terminal ALT instruction digest: {error:?}")))?,
        1,
        ExecutionRoleV1::Core,
        &[
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ],
    )
    .map_err(|error| Error::new(format!("terminal ALT continuation: {error:?}")))?;
    let aggregate = aggregate_retirement_meta_closure_v1(&AggregateRetirementMetaCoordinatesV1 {
        release_set,
        parent_request_digest: [8; 32],
        claims_request_body: vec![1],
        custody_context: context,
        close_vault_request_body: vec![2],
        close_replay_request_body: vec![3],
        rent_post_resource_digest: [9; 32],
        continuation,
        market,
        rent_credit,
        activation_cache: activation,
        registry_program: registry,
        core_program: core,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        claims_program: claims,
        claims_programdata: pubkey(&plan.claims.programdata_id)?,
        resolution_program: resolution,
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
        custody_program: custody,
        custody_programdata: pubkey(&plan.custody.programdata_id)?,
        rent_program,
        source_receipt,
        claims_aggregate: evidence_pubkey(evidence, "claims_aggregate")?,
        custody_replay: core_replay,
        hoard_vault: hoard,
        custody_authority,
        collateral_mint,
        collateral_token_program: token_program,
        realm_raw: realm.raw,
        realm_staging: realm.staging,
        infrastructure_profile: pubkey(&plan.infrastructure_profile.address)?,
        registry_artifact_raw: registry_artifact.0,
        registry_artifact_staging: registry_artifact.1,
        registry_programdata: pubkey(&plan.registry.programdata_id)?,
        rent_artifact_raw: rent_artifact.0,
        rent_artifact_staging: rent_artifact.1,
        rent_programdata: pubkey(&plan.rent_credit.programdata_id)?,
        rent_sysvar: sysvar::rent::ID,
        refund_wallet,
    })?;
    let resolution_close = resolution_close_meta_closure_v1(&ResolutionCloseMetaCoordinatesV1 {
        release_set,
        role_request_digest: [3; 32],
        market,
        activation_cache: activation,
        registry_program: registry,
        core_program: core,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        resolution_program: resolution,
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
        source_material: source_material.raw,
        source_material_staging: source_material.staging,
        capability_manifest: manifest.raw,
        capability_manifest_staging: manifest.staging,
        source_state,
        funding_ledger: resolution_funding,
        certificate,
        closure_receipt: source_receipt,
        beneficiary,
        clock_sysvar: sysvar::clock::ID,
        rent_sysvar: sysvar::rent::ID,
        system_program: system_program::ID,
        recovery_policy: recovery.map(|pair| (pair.raw, pair.staging)),
    })?;
    Ok((
        vec![
            core_begin_retiring_meta_closure_v1(
                market,
                activation,
                registry,
                core,
                pubkey(&plan.core.programdata_id)?,
            ),
            begin,
            resolution_close,
            close,
            handoff,
            aggregate,
        ],
        source_receipt,
    ))
}

fn direct_native_close_closure_from_discovery_v1(
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    snapshot: &DirectNativeCloseSnapshotV1,
    request_digest: [u8; 32],
) -> Result<TerminalMetaClosureV1> {
    let records = |raw: Pubkey, staging: Pubkey| TerminalRecordCoordinatesV1 { raw, staging };
    let deployments = |program: Pubkey, programdata: Pubkey| TerminalDeploymentCoordinatesV1 {
        program,
        programdata,
    };
    if snapshot.funding_ledgers.len() != 2 {
        return Err(refusal(
            "Direct close discovery did not contain exact F=2 funding ledgers",
        ));
    }
    direct_native_close_meta_closure_v1(&DirectNativeCloseCoordinateInputV1 {
        release_set: hex32(&plan.release_set_id)?,
        role_request_digest: request_digest,
        market: snapshot.market.key,
        realm: records(snapshot.realm.key, snapshot.realm_staging.key),
        manifest: records(snapshot.manifest.key, snapshot.manifest_staging.key),
        resolution_funding: snapshot.funding_ledgers[0].key,
        trading_funding: snapshot.funding_ledgers[1].key,
        root: snapshot.root.key,
        activation_cache: snapshot.activation_cache.key,
        core: deployments(snapshot.core_program.key, snapshot.core_programdata.key),
        trading: deployments(
            snapshot.trading_program.key,
            snapshot.trading_programdata.key,
        ),
        resolution: deployments(
            snapshot.resolution_program.key,
            snapshot.resolution_programdata.key,
        ),
        registry_program: snapshot.registry_program.key,
        rent_sysvar: snapshot.rent_sysvar.key,
        program_set: records(snapshot.program_set.key, snapshot.program_set_staging.key),
        config: records(snapshot.config.key, snapshot.config_staging.key),
        close_profile: records(
            snapshot.close_profile.key,
            snapshot.close_profile_staging.key,
        ),
        close_effect: records(snapshot.close_effect.key, snapshot.close_effect_staging.key),
        system_program: snapshot.system_program.key,
        close_descriptor: records(
            snapshot.close_descriptor.key,
            snapshot.close_descriptor_staging.key,
        ),
        rent_program: snapshot.rent_program.key,
        rent_credit: snapshot.rent_credit.key,
    })
    .map_err(|error| Error::new(format!("Direct close coordinate projection: {error:?}")))
    .and_then(|closure| {
        if evidence_pubkey(evidence, "direct_capability_root")? != snapshot.root.key {
            return Err(refusal(
                "Direct close discovery root differed from persisted campaign evidence",
            ));
        }
        Ok(closure)
    })
}

fn retirement_handoff_closure_from_discovery_v1(
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    snapshot: &RetirementReplayHandoffSnapshotV1,
    request_digest: [u8; 32],
) -> Result<TerminalMetaClosureV1> {
    retirement_replay_handoff_meta_closure_v1(&RetirementReplayHandoffCoordinateInputV1 {
        release_set: hex32(&plan.release_set_id)?,
        context: hex32(&evidence.founding_custody_context)?,
        request_digest,
        payer: snapshot.payer.key,
        market: snapshot.market.key,
        activation_cache: snapshot.activation_cache.key,
        registry_program: snapshot.registry_program.key,
        core: TerminalDeploymentCoordinatesV1 {
            program: snapshot.core_program.key,
            programdata: snapshot.core_programdata.key,
        },
        trading: TerminalDeploymentCoordinatesV1 {
            program: snapshot.trading_program.key,
            programdata: snapshot.trading_programdata.key,
        },
        custody: TerminalDeploymentCoordinatesV1 {
            program: snapshot.custody_program.key,
            programdata: snapshot.custody_programdata.key,
        },
        claims_aggregate: snapshot.claims_aggregate.key,
        realm: TerminalRecordCoordinatesV1 {
            raw: snapshot.realm.key,
            staging: snapshot.realm_staging.key,
        },
        rent_sysvar: snapshot.rent_sysvar.key,
        rent_credit: snapshot.rent_credit.key,
        trading_replay: snapshot.trading_replay.key,
        core_replay: snapshot.core_replay.key,
        hoard: snapshot.hoard.key,
        system_program: snapshot.system_program.key,
        mint: snapshot.mint.key,
        token_program: snapshot.token_program.key,
        custody_authority: snapshot.custody_authority.key,
    })
    .map_err(|error| Error::new(format!("replay-handoff coordinate projection: {error:?}")))
}

#[allow(clippy::too_many_arguments)]
fn fresh_protocol_stage_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    payer: Pubkey,
    table: Pubkey,
    source_receipt: Pubkey,
    stage: TerminalStageV1,
) -> Result<ChainDerivedTerminalMutationV1> {
    let extra = [payer, table];
    match stage {
        TerminalStageV1::CoreBeginRetiring => {
            plan_core_begin_retiring_from_chain_v1(rpc, plan, evidence, market, &extra)
        }
        TerminalStageV1::DirectBeginRetiring => {
            match plan_direct_begin_retiring_from_chain_v1(
                rpc,
                plan,
                market_input,
                evidence,
                market,
                &extra,
            )? {
                ChainDirectBeginRetiringV1::Submit(stage) => Ok(stage),
                ChainDirectBeginRetiringV1::Complete { .. } => Err(refusal(
                    "Direct BeginRetiring is complete on chain but has no exact finalized journal to reconcile",
                )),
            }
        }
        TerminalStageV1::ResolutionCloseFund => {
            match plan_resolution_close_from_chain_v1(rpc, plan, evidence, market, payer, &[table])?
            {
                ChainResolutionCloseV1::Submit { stage, .. } => Ok(stage),
                ChainResolutionCloseV1::NeedsReceiptPrepay { .. } => Err(refusal(
                    "Resolution CloseFund remains blocked on its durable receipt prepayment",
                )),
            }
        }
        TerminalStageV1::DirectCloseCapability => {
            let discovery =
                direct_native_close_discovery_from_chain_v1(rpc, plan, evidence, market)?;
            let preflight = preflight_direct_native_close_caller_v1(&discovery)
                .map_err(|error| Error::new(format!("Direct close caller preflight: {error:?}")))?;
            let closure = direct_native_close_closure_from_discovery_v1(
                plan,
                evidence,
                &discovery,
                preflight.request_digest,
            )?;
            plan_direct_native_close_two_pass_from_chain_v1(rpc, &discovery, &closure, &extra)
        }
        TerminalStageV1::RetirementReplayHandoff => {
            let discovery = retirement_replay_handoff_discovery_from_chain_v1(
                rpc, plan, evidence, market, payer,
            )?;
            let preflight =
                preflight_retirement_replay_handoff_caller_v1(&discovery).map_err(|error| {
                    Error::new(format!("replay-handoff caller preflight: {error:?}"))
                })?;
            let closure = retirement_handoff_closure_from_discovery_v1(
                plan,
                evidence,
                &discovery,
                preflight.request_digest,
            )?;
            plan_retirement_replay_handoff_two_pass_from_chain_v1(
                rpc,
                &discovery,
                &closure,
                &[table],
            )
        }
        TerminalStageV1::AggregateRetirement => {
            let (snapshot, prestate) = aggregate_retirement_snapshot_from_chain_v1(
                rpc,
                plan,
                evidence,
                market,
                source_receipt,
                &extra,
            )?;
            let report = build_market_retirement_v1(&snapshot)
                .map_err(|error| Error::new(format!("aggregate retirement: {error:?}")))?;
            let closure = TerminalMetaClosureV1 {
                stage,
                program_id: report.instruction.program_id,
                program_class: TerminalAddressClassV1::InlineProgram,
                accounts: report.instruction.accounts.clone(),
                classes: aggregate_retirement_meta_classes_v1(),
            };
            let mutation = plan_aggregate_retirement_caller_v1(&snapshot, &closure)?;
            Ok(ChainDerivedTerminalMutationV1 {
                mutation,
                closure,
                prestate,
            })
        }
    }
}

/// Canonical immutable ALT plan shared by every large terminal stage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalLookupTablePlanV1 {
    pub(crate) lookup_table: Pubkey,
    pub(crate) payer: Pubkey,
    pub(crate) authority: Pubkey,
    pub(crate) recent_slot: u64,
    pub(crate) addresses: Vec<Pubkey>,
    pub(crate) create: Instruction,
    pub(crate) extensions: Vec<Instruction>,
    pub(crate) freeze: Instruction,
    pub(crate) final_data_len: usize,
    pub(crate) final_rent_lamports: u64,
    pub(crate) maximum_preflight_wire_bytes: usize,
}

/// Build a deterministic, packet-safe create/extend/freeze preflight without
/// reading a key.  The existing versioned-message operator remains the sole
/// owner of address ordering, extension chunking, and message geometry.
pub(crate) fn plan_terminal_lookup_table_v1(
    payer: Pubkey,
    recent_slot: u64,
    union: &[Pubkey],
    rent: &Rent,
) -> Result<TerminalLookupTablePlanV1> {
    let plan = build_lookup_table_creation_v1(payer, payer, recent_slot, union)
        .map_err(|error| Error::new(format!("terminal ALT creation plan: {error:?}")))?;
    if plan.addresses.contains(&payer) {
        return Err(Error::new(
            "terminal ALT union must omit the inline transaction fee payer",
        ));
    }
    let freeze = build_lookup_table_freeze(plan.lookup_table, payer);
    let final_data_len = LOOKUP_TABLE_META_SIZE
        .checked_add(
            ALT_ADDRESS_BYTES
                .checked_mul(plan.addresses.len())
                .ok_or_else(|| Error::new("terminal ALT address width overflow"))?,
        )
        .ok_or_else(|| Error::new("terminal ALT data width overflow"))?;
    let final_rent_lamports = rent.minimum_balance(final_data_len);
    let observation = Observation {
        slot: recent_slot
            .checked_add(1)
            .ok_or_else(|| Error::new("terminal ALT geometry slot overflow"))?,
        unix_timestamp: 1,
        finality: Finality::Finalized,
    };
    let maximum_preflight_wire_bytes = std::iter::once(&plan.create)
        .chain(plan.extensions.iter())
        .chain(std::iter::once(&freeze))
        .map(|instruction| {
            compile_v0_message_with_optional_tables(
                payer,
                std::slice::from_ref(instruction),
                Hash::new_from_array(ALT_GEOMETRY_BLOCKHASH),
                observation,
                &[],
            )
            .map(|message| message.wire_bytes)
            .map_err(|error| Error::new(format!("terminal ALT packet geometry: {error:?}")))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
        .ok_or_else(|| Error::new("terminal ALT preflight has no instructions"))?;
    if maximum_preflight_wire_bytes > PACKET_DATA_BYTES {
        return Err(Error::new("terminal ALT preflight exceeds packet limit"));
    }
    Ok(terminal_lookup_plan(
        plan,
        payer,
        recent_slot,
        freeze,
        final_data_len,
        final_rent_lamports,
        maximum_preflight_wire_bytes,
    ))
}

fn terminal_lookup_plan(
    plan: LookupTableCreationPlanV1,
    payer: Pubkey,
    recent_slot: u64,
    freeze: Instruction,
    final_data_len: usize,
    final_rent_lamports: u64,
    maximum_preflight_wire_bytes: usize,
) -> TerminalLookupTablePlanV1 {
    TerminalLookupTablePlanV1 {
        lookup_table: plan.lookup_table,
        payer,
        authority: payer,
        recent_slot,
        addresses: plan.addresses,
        create: plan.create,
        extensions: plan.extensions,
        freeze,
        final_data_len,
        final_rent_lamports,
        maximum_preflight_wire_bytes,
    }
}

/// Require a caller-supplied table to be the exact, frozen canonical terminal
/// union.  A mutable superset is not immutable routing evidence.
pub(crate) fn authenticate_supplied_terminal_lookup_table_v1(
    expected_addresses: &[Pubkey],
    table: &ObservedAccount,
    rent: &Rent,
) -> Result<()> {
    let canonical = canonical_union_addresses(expected_addresses)?;
    if table.owner != lookup_table_program::id()
        || table.executable
        || table.observation.finality != Finality::Finalized
    {
        return Err(Error::new(
            "supplied terminal ALT owner/executable/finality refused",
        ));
    }
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| Error::new("supplied terminal ALT bytes refused"))?;
    let final_data_len = lookup_table_data_len(canonical.len())?;
    if decoded.meta.authority.is_some()
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= table.observation.slot
        || decoded.addresses.as_ref() != canonical.as_slice()
        || table.data.len() != final_data_len
        || table.lamports != rent.minimum_balance(final_data_len)
    {
        return Err(Error::new(
            "supplied terminal ALT is not the exact activated frozen union",
        ));
    }
    Ok(())
}

/// Resume create/append/freeze from exact finalized state.  Existing bytes may
/// equal only a complete extension prefix; divergence, overfill, a foreign
/// authority, surplus lamports, or premature freeze all refuse.
pub(crate) fn route_terminal_lookup_table_v1(
    plan: &TerminalLookupTablePlanV1,
    table: Option<&ObservedAccount>,
    rent: &Rent,
) -> Result<TerminalLookupTableRouteV1> {
    let Some(table) = table.filter(|account| account.lamports != 0) else {
        return Ok(TerminalLookupTableRouteV1::Create(plan.create.clone()));
    };
    if table.key != plan.lookup_table
        || table.owner != lookup_table_program::id()
        || table.executable
        || table.observation.finality != Finality::Finalized
    {
        return Err(Error::new(
            "terminal ALT address/owner/executable/finality refused",
        ));
    }
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| Error::new("terminal ALT bytes refused"))?;
    if decoded.meta.deactivation_slot != u64::MAX {
        return Err(Error::new("terminal ALT is deactivating"));
    }
    let addresses = decoded.addresses.as_ref();
    let expected_data_len = lookup_table_data_len(addresses.len())?;
    if table.data.len() != expected_data_len
        || table.lamports != rent.minimum_balance(expected_data_len)
        || addresses.len() > plan.addresses.len()
        || addresses != &plan.addresses[..addresses.len()]
    {
        return Err(Error::new(
            "terminal ALT data width, rent, or canonical prefix refused",
        ));
    }
    let complete = addresses.len() == plan.addresses.len();
    if !addresses.is_empty() {
        let expected_start = ((addresses.len() - 1) / EXTEND_ADDRESSES_PER_TRANSACTION_V1)
            * EXTEND_ADDRESSES_PER_TRANSACTION_V1;
        if usize::from(decoded.meta.last_extended_slot_start_index) != expected_start
            || decoded.meta.last_extended_slot >= table.observation.slot
        {
            return Err(Error::new(
                "terminal ALT extension boundary is not canonical or active",
            ));
        }
    }
    match decoded.meta.authority {
        None if complete => {
            if table.data.len() != plan.final_data_len || table.lamports != plan.final_rent_lamports
            {
                return Err(Error::new(
                    "frozen terminal ALT has wrong final width or rent",
                ));
            }
            Ok(TerminalLookupTableRouteV1::Complete)
        }
        None => Err(Error::new(
            "terminal ALT froze before the union was complete",
        )),
        Some(authority) if authority != plan.authority => {
            Err(Error::new("terminal ALT authority was substituted"))
        }
        Some(_) if complete => Ok(TerminalLookupTableRouteV1::Freeze(plan.freeze.clone())),
        Some(_) => {
            let starts = extension_prefix_lengths(plan);
            let extension_index = starts
                .iter()
                .position(|prefix| *prefix == addresses.len())
                .ok_or_else(|| {
                    Error::new("terminal ALT prefix ends inside a canonical extension page")
                })?;
            let instruction = plan
                .extensions
                .get(extension_index)
                .ok_or_else(|| Error::new("terminal ALT extension prefix overflow"))?
                .clone();
            Ok(TerminalLookupTableRouteV1::Extend {
                prefix_len: addresses.len(),
                instruction,
            })
        }
    }
}

/// Build one ALT create/extend/freeze action through the same durable message,
/// signature, balance-vector, submission, and finalized verifier as protocol
/// stages. Extend poststate commits an exact slot-relative rule because the ALT
/// program writes the eventual transaction slot, which does not exist yet at
/// plan time.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_lookup_infrastructure_journal_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    plan: &TerminalLookupTablePlanV1,
    route: &TerminalLookupTableRouteV1,
    payer: &ObservedAccount,
    table: &ObservedAccount,
    rent: &Rent,
    prestate: &[ObservedAccount],
    authorized_mutation: bool,
) -> Result<DurableTerminalJournalV1> {
    if origin.label() != "devnet"
        || payer.key != plan.payer
        || plan.authority != plan.payer
        || table.key != plan.lookup_table
        || payer.observation != table.observation
        || payer.observation.finality != Finality::Finalized
    {
        return Err(refusal(
            "terminal ALT journal mixed cluster, payer, table, authority, or observation",
        ));
    }
    let (mutation, instruction, authority, addresses, last_slot, start_index) = match route {
        TerminalLookupTableRouteV1::Create(instruction) => {
            if table.owner != solana_sdk_ids::system_program::ID
                || table.lamports != 0
                || table.executable
                || !table.data.is_empty()
                || instruction != &plan.create
            {
                return Err(refusal(
                    "terminal ALT create prestate or instruction differed from exact vacant plan",
                ));
            }
            (
                DurableTerminalMutationV1::LookupCreate,
                instruction.clone(),
                Some(plan.authority),
                Vec::new(),
                DurableLookupLastExtendedSlotV1::Exact(0),
                0,
            )
        }
        TerminalLookupTableRouteV1::Extend {
            prefix_len,
            instruction,
        } => {
            let current = AddressLookupTable::deserialize(&table.data)
                .map_err(|_| refusal("terminal ALT extend prestate did not decode"))?;
            let prefixes = extension_prefix_lengths(plan);
            let extension_index = prefixes
                .iter()
                .position(|prefix| prefix == prefix_len)
                .ok_or_else(|| refusal("terminal ALT extend prefix was not canonical"))?;
            if current.meta.authority != Some(plan.authority)
                || current.meta.deactivation_slot != u64::MAX
                || current.addresses.as_ref() != &plan.addresses[..*prefix_len]
                || plan.extensions.get(extension_index) != Some(instruction)
            {
                return Err(refusal(
                    "terminal ALT extend prestate, prefix, authority, or instruction diverged",
                ));
            }
            let next_len = prefix_len
                .checked_add(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
                .map(|value| value.min(plan.addresses.len()))
                .ok_or_else(|| refusal("terminal ALT extension prefix overflowed"))?;
            (
                DurableTerminalMutationV1::LookupExtend {
                    prefix_len: *prefix_len,
                },
                instruction.clone(),
                Some(plan.authority),
                plan.addresses[..next_len].to_vec(),
                DurableLookupLastExtendedSlotV1::FinalizedTransaction,
                u8::try_from(*prefix_len)
                    .map_err(|_| refusal("terminal ALT extension prefix exceeded u8"))?,
            )
        }
        TerminalLookupTableRouteV1::Freeze(instruction) => {
            let current = AddressLookupTable::deserialize(&table.data)
                .map_err(|_| refusal("terminal ALT freeze prestate did not decode"))?;
            if current.meta.authority != Some(plan.authority)
                || current.meta.deactivation_slot != u64::MAX
                || current.addresses.as_ref() != plan.addresses.as_slice()
                || instruction != &plan.freeze
            {
                return Err(refusal(
                    "terminal ALT freeze prestate, addresses, authority, or instruction diverged",
                ));
            }
            (
                DurableTerminalMutationV1::LookupFreeze,
                instruction.clone(),
                None,
                plan.addresses.clone(),
                DurableLookupLastExtendedSlotV1::Exact(current.meta.last_extended_slot),
                current.meta.last_extended_slot_start_index,
            )
        }
        TerminalLookupTableRouteV1::Complete => {
            return Err(refusal(
                "complete frozen ALT has no infrastructure mutation to journal",
            ));
        }
    };
    let mut prestate_by_key = BTreeMap::new();
    for account in prestate {
        if account.observation != payer.observation
            || prestate_by_key.insert(account.key, account).is_some()
        {
            return Err(refusal(
                "terminal ALT prestate mixed observations or duplicate accounts",
            ));
        }
    }
    for key in std::iter::once(payer.key)
        .chain(instruction.accounts.iter().map(|meta| meta.pubkey))
        .chain(std::iter::once(instruction.program_id))
    {
        if !prestate_by_key.contains_key(&key) {
            return Err(refusal(
                "terminal ALT prestate omitted payer, program, or instruction account",
            ));
        }
    }
    let expected_table_lamports = rent.minimum_balance(lookup_table_data_len(addresses.len())?);
    let table_top_up = expected_table_lamports
        .checked_sub(table.lamports)
        .ok_or_else(|| refusal("terminal ALT action would require a rent refund"))?;
    let payer_after_protocol = payer
        .lamports
        .checked_sub(table_top_up)
        .ok_or_else(|| refusal("terminal ALT payer cannot cover exact rent delta"))?;
    let (recent_blockhash, last_valid_block_height) = terminal_latest_blockhash(rpc)?;
    let compiled = compile_v0_message_with_optional_tables(
        payer.key,
        std::slice::from_ref(&instruction),
        recent_blockhash,
        payer.observation,
        &[],
    )
    .map_err(|error| Error::new(format!("terminal ALT durable message: {error:?}")))?;
    let VersionedMessage::V0(message) = &compiled.message else {
        return Err(refusal("terminal ALT action did not compile as v0"));
    };
    if compiled.loaded_addresses != 0 || !compiled.lookup_tables.is_empty() {
        return Err(refusal(
            "terminal ALT infrastructure mutation unexpectedly used a lookup table",
        ));
    }
    let resolved_account_keys = message.account_keys.clone();
    let pre_balances = resolved_account_keys
        .iter()
        .map(|key| {
            prestate_by_key
                .get(key)
                .map(|account| account.lamports)
                .ok_or_else(|| refusal("terminal ALT resolved key omitted prestate"))
        })
        .collect::<Result<Vec<_>>>()?;
    let message_bytes = compiled.message.serialize();
    let message_base64 = BASE64.encode(&message_bytes);
    let transaction_fee_lamports = terminal_fee_for_message(rpc, &message_base64)?;
    let payer_after_fee = payer_after_protocol
        .checked_sub(transaction_fee_lamports)
        .ok_or_else(|| refusal("terminal ALT payer cannot cover exact rent plus fee"))?;
    let deltas = BTreeMap::from([
        (payer.key, -i128::from(table_top_up)),
        (table.key, i128::from(table_top_up)),
    ]);
    if checked_terminal_delta_sum_v1(deltas.values().copied())? != 0 {
        return Err(refusal("terminal ALT rent transfer did not conserve"));
    }
    let post_balances = resolved_account_keys
        .iter()
        .zip(pre_balances.iter().copied())
        .map(|(key, pre)| {
            let after = checked_apply_lamport_delta(pre, deltas.get(key).copied().unwrap_or(0))?;
            if *key == payer.key {
                after
                    .checked_sub(transaction_fee_lamports)
                    .ok_or_else(|| refusal("terminal ALT fee debit underflowed"))
            } else {
                Ok(after)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let expected_accounts = BTreeMap::from([
        (
            payer.key.to_string(),
            DurableExpectedAccountV1 {
                address: payer.key.to_string(),
                owner: payer.owner.to_string(),
                lamports_after_protocol: payer_after_protocol,
                lamports_after_fee: payer_after_fee,
                executable: payer.executable,
                data_base64: BASE64.encode(&payer.data),
                data_sha256: sha256_hex(&payer.data),
                lookup_table: None,
            },
        ),
        (
            table.key.to_string(),
            DurableExpectedAccountV1 {
                address: table.key.to_string(),
                owner: lookup_table_program::id().to_string(),
                lamports_after_protocol: expected_table_lamports,
                lamports_after_fee: expected_table_lamports,
                executable: false,
                data_base64: String::new(),
                data_sha256: String::new(),
                lookup_table: Some(DurableLookupTablePoststateV1 {
                    authority: authority.map(|key| key.to_string()),
                    addresses: addresses.iter().map(ToString::to_string).collect(),
                    deactivation_slot: u64::MAX,
                    last_extended_slot: last_slot,
                    last_extended_slot_start_index: start_index,
                }),
            },
        ),
    ]);
    let classes = instruction
        .accounts
        .iter()
        .map(|meta| {
            if meta.is_signer {
                TerminalAddressClassV1::InlineSigner
            } else if meta.pubkey == lookup_table_program::id()
                || meta.pubkey == solana_sdk_ids::system_program::ID
            {
                TerminalAddressClassV1::InlineProgram
            } else {
                TerminalAddressClassV1::InlineRequestBound
            }
        })
        .collect::<Vec<_>>();
    let intent = DurableTerminalIntentV1 {
        mutation,
        observation_slot: payer.observation.slot,
        observation_unix_timestamp: payer.observation.unix_timestamp,
        payer: payer.key.to_string(),
        program_id: instruction.program_id.to_string(),
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: instruction
            .accounts
            .iter()
            .zip(classes)
            .map(|(meta, class)| DurableInstructionAccountV1 {
                address: meta.pubkey.to_string(),
                signer: meta.is_signer,
                writable: meta.is_writable,
                class,
            })
            .collect(),
        instruction_data_base64: BASE64.encode(&instruction.data),
        instruction_data_sha256: sha256_hex(&instruction.data),
        lookup_table: None,
        lookup_table_addresses: Vec::new(),
        lookup_table_addresses_sha256: pubkey_vector_sha256(&[]),
        loaded_writable: Vec::new(),
        loaded_readonly: Vec::new(),
        resolved_account_keys: resolved_account_keys
            .iter()
            .map(ToString::to_string)
            .collect(),
        pre_balances,
        post_balances,
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        transaction_fee_lamports,
        wire_bytes: compiled.wire_bytes,
        message_base64,
        message_sha256: sha256_hex(&message_bytes),
        prestate: prestate_by_key
            .into_iter()
            .map(|(key, account)| (key.to_string(), durable_observed_state(account)))
            .collect(),
        expected_accounts,
        expected_return_data: None,
        protocol_lamport_deltas: deltas
            .into_iter()
            .map(|(key, delta)| (key.to_string(), delta))
            .collect(),
    };
    let intent_sha256 = sha256_hex(&serde_json::to_vec(&intent)?);
    let mut journal = DurableTerminalJournalV1 {
        schema: TERMINAL_JOURNAL_SCHEMA_V1.into(),
        cluster: "devnet".into(),
        rpc_url: origin.redacted_url(),
        authorized_mutation,
        state_sha256: String::new(),
        phase: StageJournalPhaseV1::Planned,
        intent_sha256,
        intent,
        signed_packet_base64: None,
        expected_signature: None,
        finalized: None,
    };
    refresh_terminal_journal_digest_v1(&mut journal)?;
    authenticate_terminal_journal_v1(&journal)?;
    Ok(journal)
}

pub(crate) fn authenticate_lookup_infrastructure_planned_journal_v1(
    journal: &DurableTerminalJournalV1,
    plan: &TerminalLookupTablePlanV1,
    route: &TerminalLookupTableRouteV1,
    payer: &ObservedAccount,
    table: &ObservedAccount,
    rent: &Rent,
    exact_execution_prestate: &[ObservedAccount],
) -> Result<AuthenticatedPlannedTerminalIntentV1> {
    authenticate_terminal_journal_v1(journal)?;
    if journal.phase != StageJournalPhaseV1::Planned
        || payer.key != plan.payer
        || table.key != plan.lookup_table
        || payer.observation != table.observation
    {
        return Err(refusal(
            "terminal ALT semantic-owner authorization mixed journal, payer, table, or observation",
        ));
    }
    let rerouted = route_terminal_lookup_table_v1(plan, Some(table), rent)?;
    if &rerouted != route {
        return Err(refusal(
            "terminal ALT durable route differed from the fresh canonical chain route",
        ));
    }
    let (mutation, instruction, authority, addresses, last_slot, start_index) = match route {
        TerminalLookupTableRouteV1::Create(instruction) => (
            DurableTerminalMutationV1::LookupCreate,
            instruction,
            Some(plan.authority),
            Vec::new(),
            DurableLookupLastExtendedSlotV1::Exact(0),
            0,
        ),
        TerminalLookupTableRouteV1::Extend {
            prefix_len,
            instruction,
        } => {
            let next_len = prefix_len
                .checked_add(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
                .map(|value| value.min(plan.addresses.len()))
                .ok_or_else(|| refusal("terminal ALT authorization prefix overflowed"))?;
            (
                DurableTerminalMutationV1::LookupExtend {
                    prefix_len: *prefix_len,
                },
                instruction,
                Some(plan.authority),
                plan.addresses[..next_len].to_vec(),
                DurableLookupLastExtendedSlotV1::FinalizedTransaction,
                u8::try_from(*prefix_len)
                    .map_err(|_| refusal("terminal ALT authorization prefix exceeded u8"))?,
            )
        }
        TerminalLookupTableRouteV1::Freeze(instruction) => {
            let current = AddressLookupTable::deserialize(&table.data)
                .map_err(|_| refusal("terminal ALT authorization freeze state did not decode"))?;
            (
                DurableTerminalMutationV1::LookupFreeze,
                instruction,
                None,
                plan.addresses.clone(),
                DurableLookupLastExtendedSlotV1::Exact(current.meta.last_extended_slot),
                current.meta.last_extended_slot_start_index,
            )
        }
        TerminalLookupTableRouteV1::Complete => {
            return Err(refusal(
                "complete terminal ALT has no Planned mutation to authorize",
            ));
        }
    };
    let classes = instruction
        .accounts
        .iter()
        .map(|meta| {
            if meta.is_signer {
                TerminalAddressClassV1::InlineSigner
            } else if meta.pubkey == lookup_table_program::id()
                || meta.pubkey == solana_sdk_ids::system_program::ID
            {
                TerminalAddressClassV1::InlineProgram
            } else {
                TerminalAddressClassV1::InlineRequestBound
            }
        })
        .collect::<Vec<_>>();
    let accounts = instruction
        .accounts
        .iter()
        .zip(classes)
        .map(|(meta, class)| DurableInstructionAccountV1 {
            address: meta.pubkey.to_string(),
            signer: meta.is_signer,
            writable: meta.is_writable,
            class,
        })
        .collect::<Vec<_>>();
    if journal.intent.mutation != mutation
        || payer.observation.slot < journal.intent.observation_slot
        || journal.intent.payer != payer.key.to_string()
        || journal.intent.program_id != instruction.program_id.to_string()
        || journal.intent.program_class != TerminalAddressClassV1::InlineProgram
        || journal.intent.accounts != accounts
        || journal.intent.instruction_data_base64 != BASE64.encode(&instruction.data)
        || journal.intent.instruction_data_sha256 != sha256_hex(&instruction.data)
        || journal.intent.lookup_table.is_some()
    {
        return Err(refusal(
            "terminal ALT durable message differed from the canonical infrastructure semantic owner",
        ));
    }
    let observed_prestate = exact_execution_prestate
        .iter()
        .map(|account| (account.key.to_string(), durable_observed_state(account)))
        .collect::<BTreeMap<_, _>>();
    if observed_prestate.len() != exact_execution_prestate.len()
        || exact_execution_prestate
            .iter()
            .any(|account| account.observation != payer.observation)
        || observed_prestate != journal.intent.prestate
    {
        return Err(refusal(
            "terminal ALT durable prestate differed from the fresh finalized canonical route",
        ));
    }
    let table_lamports = rent.minimum_balance(lookup_table_data_len(addresses.len())?);
    let top_up = table_lamports
        .checked_sub(table.lamports)
        .ok_or_else(|| refusal("terminal ALT authorization required a rent refund"))?;
    let payer_after_protocol = payer
        .lamports
        .checked_sub(top_up)
        .ok_or_else(|| refusal("terminal ALT authorization payer underflowed"))?;
    let payer_after_fee = payer_after_protocol
        .checked_sub(journal.intent.transaction_fee_lamports)
        .ok_or_else(|| refusal("terminal ALT authorization fee underflowed"))?;
    let expected_accounts = BTreeMap::from([
        (
            payer.key.to_string(),
            DurableExpectedAccountV1 {
                address: payer.key.to_string(),
                owner: payer.owner.to_string(),
                lamports_after_protocol: payer_after_protocol,
                lamports_after_fee: payer_after_fee,
                executable: payer.executable,
                data_base64: BASE64.encode(&payer.data),
                data_sha256: sha256_hex(&payer.data),
                lookup_table: None,
            },
        ),
        (
            table.key.to_string(),
            DurableExpectedAccountV1 {
                address: table.key.to_string(),
                owner: lookup_table_program::id().to_string(),
                lamports_after_protocol: table_lamports,
                lamports_after_fee: table_lamports,
                executable: false,
                data_base64: String::new(),
                data_sha256: String::new(),
                lookup_table: Some(DurableLookupTablePoststateV1 {
                    authority: authority.map(|key| key.to_string()),
                    addresses: addresses.iter().map(ToString::to_string).collect(),
                    deactivation_slot: u64::MAX,
                    last_extended_slot: last_slot,
                    last_extended_slot_start_index: start_index,
                }),
            },
        ),
    ]);
    let deltas = BTreeMap::from([
        (payer.key.to_string(), -i128::from(top_up)),
        (table.key.to_string(), i128::from(top_up)),
    ]);
    if journal.intent.expected_accounts != expected_accounts
        || journal.intent.protocol_lamport_deltas != deltas
        || journal.intent.expected_return_data.is_some()
    {
        return Err(refusal(
            "terminal ALT poststate, rent, fee, or lamport vector differed from the canonical route",
        ));
    }
    Ok(AuthenticatedPlannedTerminalIntentV1 {
        intent_sha256: journal.intent_sha256.clone(),
    })
}

fn extension_prefix_lengths(plan: &TerminalLookupTablePlanV1) -> Vec<usize> {
    let mut prefixes = Vec::with_capacity(plan.extensions.len());
    let mut count = 0_usize;
    for _instruction in &plan.extensions {
        prefixes.push(count);
        let remaining_addresses = plan.addresses.len().saturating_sub(count);
        let page = remaining_addresses.min(EXTEND_ADDRESSES_PER_TRANSACTION_V1);
        count = count.saturating_add(page);
    }
    prefixes
}

fn lookup_table_data_len(address_count: usize) -> Result<usize> {
    LOOKUP_TABLE_META_SIZE
        .checked_add(
            ALT_ADDRESS_BYTES
                .checked_mul(address_count)
                .ok_or_else(|| Error::new("terminal ALT address width overflow"))?,
        )
        .ok_or_else(|| Error::new("terminal ALT data width overflow"))
}

fn canonical_union_addresses(addresses: &[Pubkey]) -> Result<Vec<Pubkey>> {
    let mut canonical = addresses.to_vec();
    canonical.sort_unstable_by_key(Pubkey::to_bytes);
    if canonical.is_empty() {
        return Err(Error::new("terminal ALT union is empty"));
    }
    if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::new("terminal ALT union contains a duplicate"));
    }
    Ok(canonical)
}

/// Build the only admitted terminal ALT union from all six typed coordinate
/// closures. Only semantic-owner-assigned `LookupStable` identities enter the
/// table. Signers, program identities, and request/balance-bound coordinates
/// remain inline even when their public keys happen to look durable. This runs
/// before any protocol stage and therefore consumes no projected account state.
pub(crate) fn terminal_lookup_union_from_closures_v1(
    payer: Pubkey,
    closures: &[TerminalMetaClosureV1],
) -> Result<Vec<Pubkey>> {
    if closures.len() != TerminalStageV1::ORDERED.len()
        || closures
            .iter()
            .zip(TerminalStageV1::ORDERED)
            .any(|(closure, expected)| closure.stage != expected)
    {
        return Err(refusal(
            "ALT coordinate closure set is not the exact ordered six-stage sequence",
        ));
    }
    let mut assigned = BTreeMap::<Pubkey, TerminalAddressClassV1>::new();
    let mut stable = BTreeSet::new();
    for closure in closures {
        if closure.program_id == Pubkey::default()
            || closure.program_class != TerminalAddressClassV1::InlineProgram
            || closure.accounts.is_empty()
            || closure.classes.len() != closure.accounts.len()
        {
            return Err(refusal(
                "ALT coordinate closure has a vacant program, missing class, or non-inline program identity",
            ));
        }
        assign_terminal_address_class(
            &mut assigned,
            closure.program_id,
            TerminalAddressClassV1::InlineProgram,
        )?;
        for (account, class) in closure.accounts.iter().zip(closure.classes.iter().copied()) {
            if account.pubkey == Pubkey::default() {
                return Err(refusal(
                    "ALT coordinate closure contains a vacant account identity",
                ));
            }
            match class {
                TerminalAddressClassV1::LookupStable => {
                    if account.is_signer || account.pubkey == payer {
                        return Err(refusal(
                            "ALT LookupStable role was a signer or the inline fee payer",
                        ));
                    }
                    stable.insert(account.pubkey);
                }
                TerminalAddressClassV1::InlineSigner => {
                    if !account.is_signer || account.pubkey != payer {
                        return Err(refusal(
                            "terminal InlineSigner role was not the exact fee payer signer",
                        ));
                    }
                }
                TerminalAddressClassV1::InlineProgram => {
                    if account.is_signer || account.is_writable {
                        return Err(refusal(
                            "terminal InlineProgram role carried signer or writable privilege",
                        ));
                    }
                }
                TerminalAddressClassV1::InlineRequestBound => {
                    if account.is_signer {
                        return Err(refusal(
                            "terminal request-bound invocation coordinate cannot sign",
                        ));
                    }
                }
            }
            assign_terminal_address_class(&mut assigned, account.pubkey, class)?;
        }
    }
    if stable.is_empty() {
        return Err(Error::new("terminal ALT stable union is empty"));
    }
    Ok(stable.into_iter().collect())
}

fn assign_terminal_address_class(
    assigned: &mut BTreeMap<Pubkey, TerminalAddressClassV1>,
    key: Pubkey,
    class: TerminalAddressClassV1,
) -> Result<()> {
    if assigned
        .insert(key, class)
        .is_some_and(|existing| existing != class)
    {
        return Err(refusal(
            "one terminal identity was assigned conflicting ALT placement classes",
        ));
    }
    Ok(())
}

/// Prove the v0 compiler respected every semantic-owner address class. Stable
/// identities must be loaded from the one frozen dedicated table; payer,
/// programs, signers, and request-bound identities must all remain static.
pub(crate) fn authenticate_terminal_v0_placement_v1(
    payer: Pubkey,
    closure: &TerminalMetaClosureV1,
    lookup_table: Pubkey,
    frozen_addresses: &[Pubkey],
    compiled: &VersionedMessagePlanV0,
) -> Result<()> {
    let VersionedMessage::V0(message) = &compiled.message else {
        return Err(refusal("terminal stage did not compile as a v0 message"));
    };
    if compiled.lookup_tables != [lookup_table]
        || message.address_table_lookups.len() != 1
        || message.address_table_lookups[0].account_key != lookup_table
    {
        return Err(refusal(
            "terminal stage did not use exactly the dedicated frozen lookup table",
        ));
    }
    let mut expected_stable = BTreeMap::<Pubkey, bool>::new();
    let mut expected_inline = BTreeSet::<Pubkey>::from([payer, closure.program_id]);
    for (meta, class) in closure.accounts.iter().zip(closure.classes.iter().copied()) {
        match class {
            TerminalAddressClassV1::LookupStable => {
                expected_stable
                    .entry(meta.pubkey)
                    .and_modify(|writable| *writable |= meta.is_writable)
                    .or_insert(meta.is_writable);
            }
            TerminalAddressClassV1::InlineSigner
            | TerminalAddressClassV1::InlineProgram
            | TerminalAddressClassV1::InlineRequestBound => {
                expected_inline.insert(meta.pubkey);
            }
        }
    }
    if expected_stable
        .keys()
        .any(|stable| expected_inline.contains(stable))
    {
        return Err(refusal(
            "terminal stage assigned one identity both stable and inline",
        ));
    }
    let lookup = &message.address_table_lookups[0];
    let loaded_writable = lookup
        .writable_indexes
        .iter()
        .map(|index| {
            frozen_addresses
                .get(usize::from(*index))
                .copied()
                .ok_or_else(|| refusal("terminal v0 writable lookup index exceeded frozen ALT"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let loaded_readonly = lookup
        .readonly_indexes
        .iter()
        .map(|index| {
            frozen_addresses
                .get(usize::from(*index))
                .copied()
                .ok_or_else(|| refusal("terminal v0 readonly lookup index exceeded frozen ALT"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if !loaded_writable.is_disjoint(&loaded_readonly)
        || loaded_writable.len() + loaded_readonly.len() != compiled.loaded_addresses
    {
        return Err(refusal(
            "terminal v0 lookup indices were duplicated or its loaded count disagreed",
        ));
    }
    let expected_writable = expected_stable
        .iter()
        .filter_map(|(key, writable)| writable.then_some(*key))
        .collect::<BTreeSet<_>>();
    let expected_readonly = expected_stable
        .iter()
        .filter_map(|(key, writable)| (!writable).then_some(*key))
        .collect::<BTreeSet<_>>();
    if loaded_writable != expected_writable || loaded_readonly != expected_readonly {
        return Err(refusal(
            "terminal v0 loaded keys or privileges differed from LookupStable roles",
        ));
    }
    let static_keys = message
        .account_keys
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if expected_stable
        .keys()
        .any(|stable| static_keys.contains(stable))
        || expected_inline
            .iter()
            .any(|inline| !static_keys.contains(inline))
    {
        return Err(refusal(
            "terminal v0 static/lookup placement differed from owner-assigned address classes",
        ));
    }
    Ok(())
}

fn resolved_terminal_v0_keys_v1(
    compiled: &VersionedMessagePlanV0,
    lookup_table: Pubkey,
    frozen_addresses: &[Pubkey],
) -> Result<(Vec<Pubkey>, Vec<Pubkey>, Vec<Pubkey>)> {
    let VersionedMessage::V0(message) = &compiled.message else {
        return Err(refusal("terminal message was not v0"));
    };
    if message.address_table_lookups.len() != 1
        || message.address_table_lookups[0].account_key != lookup_table
    {
        return Err(refusal(
            "terminal message did not carry its one dedicated lookup",
        ));
    }
    let lookup = &message.address_table_lookups[0];
    let resolve = |index: &u8| -> Result<Pubkey> {
        frozen_addresses
            .get(usize::from(*index))
            .copied()
            .ok_or_else(|| refusal("terminal lookup index exceeded frozen address vector"))
    };
    let writable = lookup
        .writable_indexes
        .iter()
        .map(resolve)
        .collect::<Result<Vec<_>>>()?;
    let readonly = lookup
        .readonly_indexes
        .iter()
        .map(resolve)
        .collect::<Result<Vec<_>>>()?;
    let mut resolved = message.account_keys.clone();
    resolved.extend(writable.iter().copied());
    resolved.extend(readonly.iter().copied());
    if resolved.len() != message.account_keys.len() + compiled.loaded_addresses
        || resolved.iter().copied().collect::<BTreeSet<_>>().len() != resolved.len()
    {
        return Err(refusal(
            "terminal resolved v0 key vector contained a duplicate or wrong width",
        ));
    }
    Ok((writable, readonly, resolved))
}

#[cfg(test)]
impl TerminalRouteV1 {
    const fn ordinal(self) -> Option<u8> {
        match self {
            Self::PrepayResolutionReceipt => Some(TerminalStageV1::ResolutionCloseFund.ordinal()),
            Self::Execute(stage) => Some(stage.ordinal()),
            Self::Complete => None,
        }
    }
}

/// Protocol facts after their respective semantic owners authenticated one
/// same-finalized observation.  Booleans here are conclusions, never caller
/// assertions or persisted routing hints.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedTerminalProgressV1 {
    pub(crate) core: CoreTerminalStateV1,
    pub(crate) direct: DirectTerminalStateV1,
    pub(crate) resolution: ResolutionTerminalStateV1,
    pub(crate) replay: RetirementReplayStateV1,
    pub(crate) outstanding_capabilities: u64,
    pub(crate) all_claim_supplies_zero: bool,
    pub(crate) claims_aggregate_live: bool,
    pub(crate) rent_credit_live: bool,
    pub(crate) hoard_vault_live: bool,
}

/// Select exactly one next mutation from one authenticated finalized graph.
#[cfg(test)]
pub(crate) fn route_terminal_progress_v1(
    progress: AuthenticatedTerminalProgressV1,
) -> Result<TerminalRouteV1> {
    if progress.claims_aggregate_live && !progress.all_claim_supplies_zero {
        return Err(refusal(
            "Claims supply became nonzero during terminal retirement",
        ));
    }
    match progress.core {
        CoreTerminalStateV1::Terminal => route_terminal_market(progress),
        CoreTerminalStateV1::Retiring => route_retiring_market(progress),
        CoreTerminalStateV1::Closed => route_closed_market(progress),
    }
}

#[cfg(test)]
fn route_terminal_market(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if progress.direct != DirectTerminalStateV1::Open
        || progress.resolution == ResolutionTerminalStateV1::Closed
        || progress.replay != RetirementReplayStateV1::Trading
        || progress.outstanding_capabilities != 1
        || !progress.claims_aggregate_live
        || !progress.rent_credit_live
        || !progress.hoard_vault_live
    {
        return Err(refusal(
            "a downstream terminal resource advanced before Core BeginRetiring",
        ));
    }
    Ok(TerminalRouteV1::Execute(TerminalStageV1::CoreBeginRetiring))
}

#[cfg(test)]
fn route_retiring_market(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if !progress.claims_aggregate_live || !progress.rent_credit_live || !progress.hoard_vault_live {
        return Err(refusal(
            "aggregate resources partially closed while the Core Market remains Retiring",
        ));
    }
    match progress.direct {
        DirectTerminalStateV1::Open => {
            if progress.resolution == ResolutionTerminalStateV1::Closed
                || progress.replay != RetirementReplayStateV1::Trading
                || progress.outstanding_capabilities != 1
            {
                return Err(refusal(
                    "a downstream stage advanced before Trading BeginDirectRetiring",
                ));
            }
            Ok(TerminalRouteV1::Execute(
                TerminalStageV1::DirectBeginRetiring,
            ))
        }
        DirectTerminalStateV1::Retiring => route_retiring_direct(progress),
        DirectTerminalStateV1::Closed => route_closed_direct(progress),
    }
}

#[cfg(test)]
fn route_retiring_direct(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if progress.replay != RetirementReplayStateV1::Trading || progress.outstanding_capabilities != 1
    {
        return Err(refusal(
            "Direct close or replay handoff partially committed before Resolution CloseFund",
        ));
    }
    match progress.resolution {
        ResolutionTerminalStateV1::NeedsReceiptPrepayment => {
            Ok(TerminalRouteV1::PrepayResolutionReceipt)
        }
        ResolutionTerminalStateV1::ReadyToClose => Ok(TerminalRouteV1::Execute(
            TerminalStageV1::ResolutionCloseFund,
        )),
        ResolutionTerminalStateV1::Closed => Ok(TerminalRouteV1::Execute(
            TerminalStageV1::DirectCloseCapability,
        )),
    }
}

#[cfg(test)]
fn route_closed_direct(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if progress.resolution != ResolutionTerminalStateV1::Closed
        || progress.outstanding_capabilities != 0
    {
        return Err(refusal(
            "Direct root closure lacks the exact Resolution closure or Core capability decrement",
        ));
    }
    match progress.replay {
        RetirementReplayStateV1::Trading => Ok(TerminalRouteV1::Execute(
            TerminalStageV1::RetirementReplayHandoff,
        )),
        RetirementReplayStateV1::Core => Ok(TerminalRouteV1::Execute(
            TerminalStageV1::AggregateRetirement,
        )),
        RetirementReplayStateV1::Closed => Err(refusal(
            "Custody replay closed outside the atomic aggregate-retirement poststate",
        )),
    }
}

#[cfg(test)]
fn route_closed_market(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if progress.direct != DirectTerminalStateV1::Closed
        || progress.resolution != ResolutionTerminalStateV1::Closed
        || progress.replay != RetirementReplayStateV1::Closed
        || progress.outstanding_capabilities != 0
        || progress.claims_aggregate_live
        || progress.rent_credit_live
        || progress.hoard_vault_live
    {
        return Err(refusal(
            "closed Core Market has a substituted or partial aggregate-retirement poststate",
        ));
    }
    Ok(TerminalRouteV1::Complete)
}

fn refusal(reason: &str) -> Error {
    Error::new(format!("REFUSED terminal sequence: {reason}"))
}

/// Authenticate that one durable stage journal cannot be replayed or silently
/// retargeted after a later chain stage has finalized.
#[cfg(test)]
pub(crate) fn authenticate_journal_route_v1(
    journal_mutation: TerminalRouteV1,
    journal_phase: StageJournalPhaseV1,
    current: TerminalRouteV1,
) -> Result<()> {
    if journal_mutation == TerminalRouteV1::Complete {
        return Err(refusal("a durable journal cannot plan Complete"));
    }
    match (journal_mutation, current) {
        (journal, observed) if journal == observed => Ok(()),
        (
            TerminalRouteV1::PrepayResolutionReceipt,
            TerminalRouteV1::Execute(TerminalStageV1::ResolutionCloseFund),
        ) => Err(refusal(
            "receipt prepayment appeared but its exact journal is not yet reconciled",
        )),
        (journal, observed)
            if journal
                .ordinal()
                .zip(observed.ordinal())
                .is_some_and(|(left, right)| right == left.saturating_add(1)) =>
        {
            // The caller must still resolve and authenticate the journal's
            // exact finalized signature/receipt before opening the next stage.
            let _ = journal_phase;
            Err(refusal(
                "chain poststate advanced but the prior journal is not yet reconciled",
            ))
        }
        (
            TerminalRouteV1::Execute(TerminalStageV1::AggregateRetirement),
            TerminalRouteV1::Complete,
        ) => Err(refusal(
            "aggregate poststate exists but its exact journal is not yet reconciled",
        )),
        _ => Err(refusal(
            "durable journal stage does not equal the one next stage selected by finalized state",
        )),
    }
}

pub(crate) fn run_terminal_sequence(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_terminal_sequence_arguments_v1(arguments)?;
    if !arguments.journal_dir.is_dir() {
        return Err(refusal(
            "--journal-dir must be an existing absolute directory",
        ));
    }
    let plan_source = fs::read(&arguments.plan)?;
    let market_source = fs::read(&arguments.market_input)?;
    let evidence_source = fs::read(&arguments.evidence)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let market_input: MarketRunInput = serde_json::from_slice(&market_source)?;
    let evidence = parse_campaign_terminal_evidence_v1(&evidence_source)?;
    authenticate_plan_source(&plan_source, &evidence.plan_sha256)?;
    require_direct_retirement_evidence(&evidence)?;
    authenticate_campaign_market_v1(&evidence, arguments.market)?;
    let mut rpc = Rpc::connect_cluster(
        &arguments.origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    authenticate_terminal_devnet_v1(&mut rpc, &arguments.origin)?;
    let input_digests = (
        sha256_hex(&plan_source),
        sha256_hex(&market_source),
        sha256_hex(&evidence_source),
    );
    let session = load_or_create_terminal_session_v1(
        &mut rpc,
        &arguments,
        &plan,
        &market_input,
        &evidence,
        &input_digests,
    )?;
    let source_receipt = Pubkey::from_str(&session.source_receipt)
        .map_err(|error| Error::new(format!("terminal session Source receipt: {error}")))?;
    let lookup_table = Pubkey::from_str(&session.lookup_table)
        .map_err(|error| Error::new(format!("terminal session lookup table: {error}")))?;
    let lookup_addresses = authenticate_terminal_session_semantics_v1(
        &mut rpc,
        &arguments,
        &session,
        &plan,
        &market_input,
        &evidence,
    )?;
    let rent_snapshot = finalized_snapshot(&mut rpc, &[sysvar::rent::ID])?;
    let rent: Rent = bincode::deserialize(&rent_snapshot.account(sysvar::rent::ID)?.data)
        .map_err(|error| Error::new(format!("terminal Rent sysvar: {error}")))?;
    if !operate_terminal_lookup_preflight_v1(
        &mut rpc,
        &arguments,
        &session,
        lookup_table,
        &lookup_addresses,
        &rent,
    )? {
        return Ok(());
    }
    if !operate_terminal_protocol_journals_v1(
        &mut rpc,
        &arguments,
        &session,
        &plan,
        &market_input,
        &evidence,
        lookup_table,
        &lookup_addresses,
        &rent,
        source_receipt,
    )? {
        return Ok(());
    }
    terminal_stdout_v1(json!({
        "status": "complete",
        "market": arguments.market.to_string(),
        "lookupTable": lookup_table.to_string(),
        "journalDirectory": arguments.journal_dir.display().to_string(),
        "message": "Every exact terminal journal reverified at finalized and the aggregate Market account is closed."
    }))
}

fn load_or_create_terminal_session_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    input_digests: &(String, String, String),
) -> Result<TerminalSequenceSessionV1> {
    if arguments.session.exists() {
        let session = read_terminal_session_v1(&arguments.session)?;
        authenticate_terminal_session_inputs_v1(&session, arguments, input_digests)?;
        return Ok(session);
    }
    let (closures, source_receipt) = project_terminal_lookup_closures_from_chain_v1(
        rpc,
        plan,
        market_input,
        evidence,
        arguments.market,
        arguments.payer,
        None,
    )?;
    let lookup_addresses = terminal_lookup_union_from_closures_v1(arguments.payer, &closures)?;
    let snapshot = finalized_snapshot(rpc, &[source_receipt, sysvar::rent::ID])?;
    let rent: Rent = bincode::deserialize(&snapshot.account(sysvar::rent::ID)?.data)
        .map_err(|error| Error::new(format!("terminal session Rent sysvar: {error}")))?;
    let receipt = snapshot.account(source_receipt)?;
    let receipt_rent_lamports = rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    if receipt.owner != system_program::ID
        || receipt.executable
        || !receipt.data.is_empty()
        || receipt.lamports > receipt_rent_lamports
    {
        return Err(refusal(
            "new terminal session requires the canonical vacant at-most-rent Resolution receipt",
        ));
    }
    let (supplied_lookup_table, lookup_table, lookup_recent_slot) =
        match arguments.supplied_lookup_table {
            Some(table) => {
                let supplied = finalized_snapshot(rpc, &[table])?;
                authenticate_supplied_terminal_lookup_table_v1(
                    &lookup_addresses,
                    supplied.account(table)?,
                    &rent,
                )?;
                (true, table, 0)
            }
            None => {
                let recent_slot = rpc.finalized_slot()?;
                let plan = plan_terminal_lookup_table_v1(
                    arguments.payer,
                    recent_slot,
                    &lookup_addresses,
                    &rent,
                )?;
                (false, plan.lookup_table, recent_slot)
            }
        };
    let mut session = TerminalSequenceSessionV1 {
        schema: TERMINAL_SESSION_SCHEMA_V1.into(),
        devnet_genesis_hash: DEVNET_GENESIS_HASH.into(),
        rpc_url: arguments.origin.redacted_url(),
        plan_sha256: input_digests.0.clone(),
        market_input_sha256: input_digests.1.clone(),
        evidence_sha256: input_digests.2.clone(),
        market: arguments.market.to_string(),
        payer: arguments.payer.to_string(),
        source_receipt: source_receipt.to_string(),
        receipt_initial_lamports: receipt.lamports,
        receipt_rent_lamports,
        supplied_lookup_table,
        lookup_table: lookup_table.to_string(),
        lookup_recent_slot,
        lookup_addresses: lookup_addresses.iter().map(ToString::to_string).collect(),
        lookup_addresses_sha256: pubkey_vector_sha256(&lookup_addresses),
        session_sha256: String::new(),
    };
    refresh_terminal_session_digest_v1(&mut session)?;
    write_new_terminal_session_v1(&arguments.session, &session)?;
    Ok(session)
}

fn authenticate_terminal_session_inputs_v1(
    session: &TerminalSequenceSessionV1,
    arguments: &TerminalSequenceArgumentsV1,
    input_digests: &(String, String, String),
) -> Result<()> {
    authenticate_terminal_session_v1(session)?;
    if session.rpc_url != arguments.origin.redacted_url()
        || session.plan_sha256 != input_digests.0
        || session.market_input_sha256 != input_digests.1
        || session.evidence_sha256 != input_digests.2
        || session.market != arguments.market.to_string()
        || session.payer != arguments.payer.to_string()
        || session.supplied_lookup_table != arguments.supplied_lookup_table.is_some()
        || arguments
            .supplied_lookup_table
            .is_some_and(|table| session.lookup_table != table.to_string())
    {
        return Err(refusal(
            "terminal session input digests, origin, Market, payer, or supplied ALT changed",
        ));
    }
    Ok(())
}

fn terminal_protocol_closure_from_journal_v1(
    journal: &DurableTerminalJournalV1,
    stage: TerminalStageV1,
    session: &TerminalSequenceSessionV1,
) -> Result<TerminalMetaClosureV1> {
    if journal.intent.mutation != (DurableTerminalMutationV1::Protocol { stage })
        || journal.intent.lookup_table.as_deref() != Some(session.lookup_table.as_str())
        || journal.intent.lookup_table_addresses != session.lookup_addresses
        || journal.intent.lookup_table_addresses_sha256 != session.lookup_addresses_sha256
    {
        return Err(refusal(
            "terminal protocol journal did not bind its exact stage and immutable session ALT",
        ));
    }
    let program_id = Pubkey::from_str(&journal.intent.program_id)
        .map_err(|error| Error::new(format!("terminal journal program: {error}")))?;
    let accounts = journal
        .intent
        .accounts
        .iter()
        .map(|account| {
            let pubkey = Pubkey::from_str(&account.address)
                .map_err(|error| Error::new(format!("terminal journal account: {error}")))?;
            Ok(AccountMeta {
                pubkey,
                is_signer: account.signer,
                is_writable: account.writable,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let closure = TerminalMetaClosureV1 {
        stage,
        program_id,
        program_class: journal.intent.program_class,
        accounts,
        classes: journal
            .intent
            .accounts
            .iter()
            .map(|account| account.class)
            .collect(),
    };
    let instruction = Instruction {
        program_id,
        accounts: closure.accounts.clone(),
        data: BASE64
            .decode(&journal.intent.instruction_data_base64)
            .map_err(|error| Error::new(format!("terminal journal instruction: {error}")))?,
    };
    closure.authenticate_instruction(&instruction)?;
    Ok(closure)
}

fn authenticate_source_receipt_journal_v1(
    journal: &DurableTerminalJournalV1,
    session: &TerminalSequenceSessionV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    market: Pubkey,
) -> Result<Pubkey> {
    if journal.intent.mutation
        != (DurableTerminalMutationV1::Protocol {
            stage: TerminalStageV1::ResolutionCloseFund,
        })
    {
        return Err(refusal(
            "Resolution receipt evidence came from another terminal mutation",
        ));
    }
    let receipt = Pubkey::from_str(&session.source_receipt)
        .map_err(|error| Error::new(format!("terminal session Source receipt: {error}")))?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let expected = journal
        .intent
        .expected_accounts
        .get(&receipt.to_string())
        .ok_or_else(|| {
            refusal("Resolution CloseFund journal omitted its exact receipt poststate")
        })?;
    let data = BASE64
        .decode(&expected.data_base64)
        .map_err(|error| Error::new(format!("Resolution receipt journal base64: {error}")))?;
    let decoded = SourceClosureReceiptV3::decode(&data)
        .map_err(|error| Error::new(format!("Resolution receipt journal: {error:?}")))?;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &market_input.generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    if expected.address != receipt.to_string()
        || expected.owner != resolution.to_string()
        || expected.executable
        || expected.lookup_table.is_some()
        || expected.lamports_after_protocol != session.receipt_rent_lamports
        || expected.lamports_after_fee != session.receipt_rent_lamports
        || expected.data_sha256 != sha256_hex(&data)
        || data.len() != SOURCE_CLOSURE_RECEIPT_BYTES_V3
        || Pubkey::new_from_array(decoded.receipt_account) != receipt
        || Pubkey::new_from_array(decoded.market) != market
        || Pubkey::new_from_array(decoded.source_state) != source_state
        || decoded.generation != market_input.generation
    {
        return Err(refusal(
            "Resolution CloseFund journal carried a substituted receipt identity, bytes, owner, rent, or generation",
        ));
    }
    Ok(receipt)
}

fn authenticate_terminal_receipt_funding_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    stage_three: Option<&DurableTerminalJournalV1>,
) -> Result<()> {
    let receipt = Pubkey::from_str(&session.source_receipt)
        .map_err(|error| Error::new(format!("terminal session Source receipt: {error}")))?;
    let rent_snapshot = finalized_snapshot(rpc, &[sysvar::rent::ID])?;
    let rent: Rent = bincode::deserialize(&rent_snapshot.account(sysvar::rent::ID)?.data)
        .map_err(|error| Error::new(format!("terminal session Rent sysvar: {error}")))?;
    if rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3) != session.receipt_rent_lamports {
        return Err(refusal(
            "terminal session receipt rent no longer rederived from the canonical Rent sysvar",
        ));
    }
    let prepay_path = arguments
        .journal_dir
        .join("12-resolution-receipt-prepay.json");
    if prepay_path.exists() {
        if session.receipt_initial_lamports >= session.receipt_rent_lamports {
            return Err(refusal(
                "receipt-prepay journal appeared for a session that began exactly funded",
            ));
        }
        let journal = read_terminal_journal_v1(&prepay_path)?;
        if journal.intent.mutation != DurableTerminalMutationV1::ResolutionReceiptPrepay {
            return Err(refusal("receipt-prepay path carried another mutation"));
        }
        let key = receipt.to_string();
        let before = journal
            .intent
            .prestate
            .get(&key)
            .ok_or_else(|| refusal("receipt-prepay journal omitted receipt prestate"))?;
        let after = journal
            .intent
            .expected_accounts
            .get(&key)
            .ok_or_else(|| refusal("receipt-prepay journal omitted receipt poststate"))?;
        if before.owner != system_program::ID.to_string()
            || before.executable
            || before.data_len != 0
            || before.data_sha256 != sha256_hex(&[])
            || before.lamports != session.receipt_initial_lamports
            || after.owner != system_program::ID.to_string()
            || after.executable
            || !after.data_base64.is_empty()
            || after.data_sha256 != sha256_hex(&[])
            || after.lamports_after_protocol != session.receipt_rent_lamports
            || after.lamports_after_fee != session.receipt_rent_lamports
        {
            return Err(refusal(
                "receipt-prepay journal did not reproduce the session's exact vacant receipt and rent arithmetic",
            ));
        }
        if journal.phase == StageJournalPhaseV1::Finalized {
            verify_persisted_terminal_finalization_v1(rpc, &journal)?;
        }
        return Ok(());
    }
    if session.receipt_initial_lamports < session.receipt_rent_lamports
        && stage_three.is_some_and(|journal| journal.phase != StageJournalPhaseV1::Planned)
    {
        return Err(refusal(
            "Resolution CloseFund was signed without the required durable receipt-prepay journal",
        ));
    }
    if let Some(journal) =
        stage_three.filter(|journal| journal.phase != StageJournalPhaseV1::Planned)
    {
        let before = journal
            .intent
            .prestate
            .get(&receipt.to_string())
            .ok_or_else(|| refusal("Resolution CloseFund journal omitted receipt prestate"))?;
        if before.owner != system_program::ID.to_string()
            || before.executable
            || before.data_len != 0
            || before.data_sha256 != sha256_hex(&[])
            || before.lamports != session.receipt_initial_lamports
            || session.receipt_initial_lamports != session.receipt_rent_lamports
        {
            return Err(refusal(
                "Resolution CloseFund journal did not reproduce the exactly funded initial receipt",
            ));
        }
        return Ok(());
    }
    let receipt_snapshot = finalized_snapshot(rpc, &[receipt])?;
    let account = receipt_snapshot.account(receipt)?;
    if account.owner != system_program::ID
        || account.executable
        || !account.data.is_empty()
        || account.lamports != session.receipt_initial_lamports
    {
        return Err(refusal(
            "vacant Resolution receipt changed without its durable prepay or CloseFund journal",
        ));
    }
    Ok(())
}

fn authenticate_terminal_session_semantics_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
) -> Result<Vec<Pubkey>> {
    let stage_three_path = arguments
        .journal_dir
        .join(stage_journal_name_v1(TerminalStageV1::ResolutionCloseFund));
    let stage_three = stage_three_path
        .exists()
        .then(|| read_terminal_journal_v1(&stage_three_path))
        .transpose()?;
    authenticate_terminal_receipt_funding_v1(rpc, arguments, session, stage_three.as_ref())?;
    let pinned_receipt = stage_three
        .as_ref()
        .filter(|journal| journal.phase != StageJournalPhaseV1::Planned)
        .map(|journal| {
            authenticate_source_receipt_journal_v1(
                journal,
                session,
                plan,
                market_input,
                arguments.market,
            )
        })
        .transpose()?;

    let mut finalized_closures = Vec::with_capacity(TerminalStageV1::ORDERED.len());
    let mut all_finalized = true;
    for stage in TerminalStageV1::ORDERED {
        let path = arguments.journal_dir.join(stage_journal_name_v1(stage));
        if !path.exists() {
            all_finalized = false;
            break;
        }
        let journal = read_terminal_journal_v1(&path)?;
        if journal.phase != StageJournalPhaseV1::Finalized {
            all_finalized = false;
            break;
        }
        verify_persisted_terminal_finalization_v1(rpc, &journal)?;
        finalized_closures.push(terminal_protocol_closure_from_journal_v1(
            &journal, stage, session,
        )?);
    }
    let (closures, source_receipt) = if all_finalized {
        let receipt = pinned_receipt.ok_or_else(|| {
            refusal("finalized terminal sequence omitted finalized Resolution receipt evidence")
        })?;
        (finalized_closures, receipt)
    } else {
        project_terminal_lookup_closures_from_chain_v1(
            rpc,
            plan,
            market_input,
            evidence,
            arguments.market,
            arguments.payer,
            pinned_receipt,
        )?
    };
    let union = terminal_lookup_union_from_closures_v1(arguments.payer, &closures)?;
    authenticate_terminal_session_union_v1(session, source_receipt, &union)?;
    Ok(union)
}

fn authenticate_terminal_session_union_v1(
    session: &TerminalSequenceSessionV1,
    source_receipt: Pubkey,
    union: &[Pubkey],
) -> Result<()> {
    let expected_receipt = Pubkey::from_str(&session.source_receipt)
        .map_err(|error| Error::new(format!("terminal session Source receipt: {error}")))?;
    let durable = session
        .lookup_addresses
        .iter()
        .map(|value| {
            Pubkey::from_str(value)
                .map_err(|error| Error::new(format!("terminal session ALT address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if source_receipt != expected_receipt
        || union != durable.as_slice()
        || pubkey_vector_sha256(union) != session.lookup_addresses_sha256
    {
        return Err(refusal(
            "terminal session receipt or ALT union did not rederive from protocol semantic owners",
        ));
    }
    Ok(())
}

fn refresh_terminal_session_digest_v1(session: &mut TerminalSequenceSessionV1) -> Result<()> {
    session.session_sha256.clear();
    session.session_sha256 = sha256_hex(&serde_json::to_vec(session)?);
    Ok(())
}

fn authenticate_terminal_session_v1(session: &TerminalSequenceSessionV1) -> Result<()> {
    let mut material = session.clone();
    let digest = material.session_sha256.clone();
    material.session_sha256.clear();
    let addresses = session
        .lookup_addresses
        .iter()
        .map(|value| {
            Pubkey::from_str(value)
                .map_err(|error| Error::new(format!("terminal session ALT address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if session.schema != TERMINAL_SESSION_SCHEMA_V1
        || session.devnet_genesis_hash != DEVNET_GENESIS_HASH
        || session.rpc_url.is_empty()
        || digest != sha256_hex(&serde_json::to_vec(&material)?)
        || addresses != canonical_union_addresses(&addresses)?
        || session.lookup_addresses_sha256 != pubkey_vector_sha256(&addresses)
        || Pubkey::from_str(&session.lookup_table).is_err()
        || Pubkey::from_str(&session.source_receipt).is_err()
        || session.receipt_initial_lamports > session.receipt_rent_lamports
        || (!session.supplied_lookup_table && session.lookup_recent_slot == 0)
    {
        return Err(refusal(
            "terminal session schema, digest, exact devnet, ALT union, receipt, or lookup identity changed",
        ));
    }
    Ok(())
}

fn read_terminal_session_v1(path: &Path) -> Result<TerminalSequenceSessionV1> {
    if !path.is_absolute() {
        return Err(refusal("terminal session path must be absolute"));
    }
    let source = fs::read(path)?;
    let _: UniqueJsonV1 = serde_json::from_slice(&source).map_err(|error| {
        Error::new(format!(
            "terminal session JSON contains a duplicate key or malformed value: {error}"
        ))
    })?;
    let session: TerminalSequenceSessionV1 = serde_json::from_slice(&source)?;
    authenticate_terminal_session_v1(&session)?;
    Ok(session)
}

fn write_new_terminal_session_v1(path: &Path, session: &TerminalSequenceSessionV1) -> Result<()> {
    if !path.is_absolute() {
        return Err(refusal("--session must be absolute"));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("terminal session needs a UTF-8 file name"))?;
    let lock = path.with_file_name(format!(".{name}.terminal-session.lock"));
    let lock_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| {
            Error::new(format!(
                "REFUSED: acquire exclusive terminal session lock {}: {error}",
                lock.display()
            ))
        })?;
    lock_file.sync_all()?;
    if path.exists() {
        let _ = fs::remove_file(&lock);
        return Err(refusal(
            "terminal session appeared concurrently; rerun and authenticate it",
        ));
    }
    let temporary = path.with_file_name(format!(
        ".{name}.terminal-session-{}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(session)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    fs::hard_link(&temporary, path).map_err(|error| {
        Error::new(format!(
            "atomically publish terminal session {} without clobber: {error}",
            path.display()
        ))
    })?;
    fs::remove_file(&temporary)?;
    if let Some(parent) = path.parent() {
        OpenOptions::new()
            .read(true)
            .open(parent)
            .and_then(|directory| directory.sync_all())?;
    }
    fs::remove_file(&lock)?;
    Ok(())
}

fn operate_terminal_lookup_preflight_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    lookup_table: Pubkey,
    lookup_addresses: &[Pubkey],
    rent: &Rent,
) -> Result<bool> {
    if session.supplied_lookup_table {
        let snapshot = finalized_snapshot(rpc, &[lookup_table])?;
        authenticate_supplied_terminal_lookup_table_v1(
            lookup_addresses,
            snapshot.account(lookup_table)?,
            rent,
        )?;
        return Ok(true);
    }
    let plan = plan_terminal_lookup_table_v1(
        arguments.payer,
        session.lookup_recent_slot,
        lookup_addresses,
        rent,
    )?;
    if plan.lookup_table != lookup_table || plan.addresses != lookup_addresses {
        return Err(refusal(
            "terminal session ALT plan no longer reproduced its exact table or union",
        ));
    }
    let expected = lookup_journal_sequence_v1(&plan, &arguments.journal_dir);
    let mut gap = false;
    for (path, mutation) in &expected {
        if !path.exists() {
            gap = true;
            continue;
        }
        if gap {
            return Err(refusal(
                "terminal ALT journals contained a later action after a missing prefix",
            ));
        }
        let mut journal = read_terminal_journal_v1(path)?;
        if journal.intent.mutation != *mutation {
            return Err(refusal(
                "terminal ALT journal path carried another infrastructure mutation",
            ));
        }
        if journal.phase == StageJournalPhaseV1::Finalized {
            resume_terminal_journal_v1(
                rpc,
                &arguments.origin,
                path,
                &arguments.payer_keypair,
                false,
                &mut journal,
                None,
            )?;
            continue;
        }
        let authorization = if journal.phase == StageJournalPhaseV1::Planned {
            let route = current_lookup_route_v1(rpc, &plan, rent)?;
            if lookup_route_mutation_v1(&route) != Some(mutation.clone()) {
                return Err(refusal(
                    "current ALT route differed from the unresolved Planned journal",
                ));
            }
            let (payer, table, observed_rent, prestate) =
                lookup_execution_snapshot_v1(rpc, &plan, &route)?;
            Some(authenticate_lookup_infrastructure_planned_journal_v1(
                &journal,
                &plan,
                &route,
                &payer,
                &table,
                &observed_rent,
                &prestate,
            )?)
        } else {
            None
        };
        resume_terminal_journal_v1(
            rpc,
            &arguments.origin,
            path,
            &arguments.payer_keypair,
            arguments.execute,
            &mut journal,
            authorization.as_ref(),
        )?;
        terminal_stdout_v1(json!({
            "status": "lookup-action",
            "journal": path.display().to_string(),
            "phase": journal.phase,
            "lookupTable": lookup_table.to_string()
        }))?;
        return Ok(false);
    }
    let route = current_lookup_route_v1(rpc, &plan, rent)?;
    if route == TerminalLookupTableRouteV1::Complete {
        let snapshot = finalized_snapshot(rpc, &[lookup_table])?;
        authenticate_supplied_terminal_lookup_table_v1(
            lookup_addresses,
            snapshot.account(lookup_table)?,
            rent,
        )?;
        return Ok(true);
    }
    let mutation = lookup_route_mutation_v1(&route)
        .ok_or_else(|| refusal("terminal ALT route omitted its infrastructure mutation"))?;
    let path = expected
        .iter()
        .find_map(|(path, expected)| (*expected == mutation).then_some(path.clone()))
        .ok_or_else(|| refusal("terminal ALT route was outside its canonical journal sequence"))?;
    if path.exists() {
        return Err(refusal(
            "terminal ALT next journal appeared after the authenticated prefix scan",
        ));
    }
    let (payer, table, observed_rent, prestate) = lookup_execution_snapshot_v1(rpc, &plan, &route)?;
    let mut journal = build_lookup_infrastructure_journal_v1(
        rpc,
        &arguments.origin,
        &plan,
        &route,
        &payer,
        &table,
        &observed_rent,
        &prestate,
        arguments.execute,
    )?;
    write_terminal_journal_v1(&path, &mut journal, true)?;
    let authorization = authenticate_lookup_infrastructure_planned_journal_v1(
        &journal,
        &plan,
        &route,
        &payer,
        &table,
        &observed_rent,
        &prestate,
    )?;
    resume_terminal_journal_v1(
        rpc,
        &arguments.origin,
        &path,
        &arguments.payer_keypair,
        arguments.execute,
        &mut journal,
        Some(&authorization),
    )?;
    terminal_stdout_v1(json!({
        "status": "lookup-action",
        "journal": path.display().to_string(),
        "phase": journal.phase,
        "lookupTable": lookup_table.to_string(),
        "addresses": lookup_addresses.len(),
        "maximumWireBytes": plan.maximum_preflight_wire_bytes,
        "finalRentLamports": plan.final_rent_lamports
    }))?;
    Ok(false)
}

fn lookup_journal_sequence_v1(
    plan: &TerminalLookupTablePlanV1,
    directory: &Path,
) -> Vec<(PathBuf, DurableTerminalMutationV1)> {
    let mut sequence = vec![(
        directory.join("00-alt-create.json"),
        DurableTerminalMutationV1::LookupCreate,
    )];
    sequence.extend(
        extension_prefix_lengths(plan)
            .into_iter()
            .map(|prefix_len| {
                (
                    directory.join(format!("01-alt-extend-{prefix_len:03}.json")),
                    DurableTerminalMutationV1::LookupExtend { prefix_len },
                )
            }),
    );
    sequence.push((
        directory.join("02-alt-freeze.json"),
        DurableTerminalMutationV1::LookupFreeze,
    ));
    sequence
}

fn lookup_route_mutation_v1(
    route: &TerminalLookupTableRouteV1,
) -> Option<DurableTerminalMutationV1> {
    match route {
        TerminalLookupTableRouteV1::Create(_) => Some(DurableTerminalMutationV1::LookupCreate),
        TerminalLookupTableRouteV1::Extend { prefix_len, .. } => {
            Some(DurableTerminalMutationV1::LookupExtend {
                prefix_len: *prefix_len,
            })
        }
        TerminalLookupTableRouteV1::Freeze(_) => Some(DurableTerminalMutationV1::LookupFreeze),
        TerminalLookupTableRouteV1::Complete => None,
    }
}

fn current_lookup_route_v1(
    rpc: &mut Rpc,
    plan: &TerminalLookupTablePlanV1,
    rent: &Rent,
) -> Result<TerminalLookupTableRouteV1> {
    let snapshot = finalized_snapshot(rpc, &[plan.lookup_table])?;
    route_terminal_lookup_table_v1(plan, Some(snapshot.account(plan.lookup_table)?), rent)
}

fn lookup_execution_snapshot_v1(
    rpc: &mut Rpc,
    plan: &TerminalLookupTablePlanV1,
    expected_route: &TerminalLookupTableRouteV1,
) -> Result<(ObservedAccount, ObservedAccount, Rent, Vec<ObservedAccount>)> {
    let instruction = match expected_route {
        TerminalLookupTableRouteV1::Create(instruction)
        | TerminalLookupTableRouteV1::Freeze(instruction) => instruction,
        TerminalLookupTableRouteV1::Extend { instruction, .. } => instruction,
        TerminalLookupTableRouteV1::Complete => {
            return Err(refusal("complete terminal ALT has no execution snapshot"));
        }
    };
    let mut keys = vec![
        plan.payer,
        plan.lookup_table,
        instruction.program_id,
        sysvar::rent::ID,
    ];
    keys.extend(instruction.accounts.iter().map(|meta| meta.pubkey));
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let payer = snapshot.account(plan.payer)?.clone();
    let table = snapshot.account(plan.lookup_table)?.clone();
    let rent: Rent = bincode::deserialize(&snapshot.account(sysvar::rent::ID)?.data)
        .map_err(|error| Error::new(format!("terminal ALT Rent sysvar: {error}")))?;
    let rerouted = route_terminal_lookup_table_v1(plan, Some(&table), &rent)?;
    if &rerouted != expected_route {
        return Err(refusal(
            "terminal ALT route changed while acquiring its complete execution snapshot",
        ));
    }
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok((payer, table, rent, prestate))
}

#[allow(clippy::too_many_arguments)]
fn operate_terminal_protocol_journals_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    lookup_table: Pubkey,
    lookup_addresses: &[Pubkey],
    rent: &Rent,
    source_receipt: Pubkey,
) -> Result<bool> {
    let prepay_required = session.receipt_initial_lamports < session.receipt_rent_lamports;
    let prepay_path = arguments
        .journal_dir
        .join("12-resolution-receipt-prepay.json");
    authenticate_terminal_journal_prefix_v1(&arguments.journal_dir, prepay_required)?;
    if !prepay_required && prepay_path.exists() {
        return Err(refusal(
            "terminal session began with an exactly funded receipt but a prepay journal appeared",
        ));
    }
    for stage in TerminalStageV1::ORDERED {
        if stage == TerminalStageV1::ResolutionCloseFund
            && prepay_required
            && !operate_resolution_prepay_journal_v1(
                rpc,
                arguments,
                plan,
                evidence,
                lookup_table,
                lookup_addresses,
                rent,
                &prepay_path,
            )?
        {
            return Ok(false);
        }
        let path = arguments.journal_dir.join(stage_journal_name_v1(stage));
        if path.exists() {
            let mut journal = read_terminal_journal_v1(&path)?;
            if journal.intent.mutation != (DurableTerminalMutationV1::Protocol { stage }) {
                return Err(refusal(
                    "terminal protocol journal path carried another stage mutation",
                ));
            }
            if journal.phase == StageJournalPhaseV1::Finalized {
                resume_terminal_journal_v1(
                    rpc,
                    &arguments.origin,
                    &path,
                    &arguments.payer_keypair,
                    false,
                    &mut journal,
                    None,
                )?;
                continue;
            }
            let authorization = if journal.phase == StageJournalPhaseV1::Planned {
                let fresh = fresh_protocol_stage_from_chain_v1(
                    rpc,
                    plan,
                    market_input,
                    evidence,
                    arguments.market,
                    arguments.payer,
                    lookup_table,
                    source_receipt,
                    stage,
                )?;
                Some(authenticate_chain_derived_planned_journal_v1(
                    &journal, &fresh,
                )?)
            } else {
                None
            };
            resume_terminal_journal_v1(
                rpc,
                &arguments.origin,
                &path,
                &arguments.payer_keypair,
                arguments.execute,
                &mut journal,
                authorization.as_ref(),
            )?;
            terminal_stdout_v1(json!({
                "status": "protocol-stage",
                "stage": stage,
                "journal": path.display().to_string(),
                "phase": journal.phase
            }))?;
            return Ok(false);
        }
        let later = TerminalStageV1::ORDERED
            .iter()
            .copied()
            .filter(|candidate| candidate.ordinal() > stage.ordinal())
            .map(|candidate| arguments.journal_dir.join(stage_journal_name_v1(candidate)))
            .find(|candidate| candidate.exists());
        if later.is_some() {
            return Err(refusal(
                "terminal protocol journals contained a later stage after a missing prefix",
            ));
        }
        let fresh = fresh_protocol_stage_from_chain_v1(
            rpc,
            plan,
            market_input,
            evidence,
            arguments.market,
            arguments.payer,
            lookup_table,
            source_receipt,
            stage,
        )?;
        let table = fresh
            .prestate
            .iter()
            .find(|account| account.key == lookup_table)
            .ok_or_else(|| refusal("fresh terminal stage omitted frozen ALT prestate"))?;
        authenticate_supplied_terminal_lookup_table_v1(lookup_addresses, table, rent)?;
        let mut journal = build_protocol_stage_journal_v1(
            rpc,
            &arguments.origin,
            arguments.payer,
            &fresh.mutation,
            &fresh.closure,
            table,
            lookup_addresses,
            rent,
            &fresh.prestate,
            arguments.execute,
        )?;
        write_terminal_journal_v1(&path, &mut journal, true)?;
        let authorization = authenticate_chain_derived_planned_journal_v1(&journal, &fresh)?;
        resume_terminal_journal_v1(
            rpc,
            &arguments.origin,
            &path,
            &arguments.payer_keypair,
            arguments.execute,
            &mut journal,
            Some(&authorization),
        )?;
        terminal_stdout_v1(json!({
            "status": "protocol-stage",
            "stage": stage,
            "journal": path.display().to_string(),
            "phase": journal.phase,
            "feeLamports": journal.intent.transaction_fee_lamports,
            "wireBytes": journal.intent.wire_bytes
        }))?;
        return Ok(false);
    }
    let market = finalized_snapshot(rpc, &[arguments.market])?;
    let account = market.account(arguments.market)?;
    if account.owner != system_program::ID
        || account.lamports != 0
        || account.executable
        || !account.data.is_empty()
    {
        return Err(refusal(
            "all six journals finalized but the aggregate Market account was not exactly closed",
        ));
    }
    Ok(true)
}

fn authenticate_terminal_journal_prefix_v1(
    journal_dir: &Path,
    prepay_required: bool,
) -> Result<()> {
    let mut ordered = Vec::with_capacity(7);
    for stage in TerminalStageV1::ORDERED {
        if stage == TerminalStageV1::ResolutionCloseFund && prepay_required {
            ordered.push(journal_dir.join("12-resolution-receipt-prepay.json"));
        }
        ordered.push(journal_dir.join(stage_journal_name_v1(stage)));
    }
    let mut missing = false;
    for path in ordered {
        if path.exists() {
            if missing {
                return Err(refusal(
                    "terminal journals contained a later action after a missing durable prefix",
                ));
            }
        } else {
            missing = true;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn operate_resolution_prepay_journal_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    lookup_table: Pubkey,
    lookup_addresses: &[Pubkey],
    rent: &Rent,
    path: &Path,
) -> Result<bool> {
    if path.exists() {
        let mut journal = read_terminal_journal_v1(path)?;
        if journal.intent.mutation != DurableTerminalMutationV1::ResolutionReceiptPrepay {
            return Err(refusal(
                "Resolution receipt-prepay journal carried another mutation",
            ));
        }
        if journal.phase == StageJournalPhaseV1::Finalized {
            resume_terminal_journal_v1(
                rpc,
                &arguments.origin,
                path,
                &arguments.payer_keypair,
                false,
                &mut journal,
                None,
            )?;
            return Ok(true);
        }
        let authorization = if journal.phase == StageJournalPhaseV1::Planned {
            let prepay = plan_resolution_close_from_chain_v1(
                rpc,
                plan,
                evidence,
                arguments.market,
                arguments.payer,
                &[lookup_table],
            )?;
            let ChainResolutionCloseV1::NeedsReceiptPrepay {
                payer,
                receipt,
                exact_receipt_rent,
                prestate,
                ..
            } = prepay
            else {
                return Err(refusal(
                    "receipt became funded but its unresolved prepay journal was not reconciled",
                ));
            };
            Some(authenticate_resolution_receipt_prepay_planned_journal_v1(
                &journal,
                &payer,
                &receipt,
                exact_receipt_rent,
                &prestate,
            )?)
        } else {
            None
        };
        resume_terminal_journal_v1(
            rpc,
            &arguments.origin,
            path,
            &arguments.payer_keypair,
            arguments.execute,
            &mut journal,
            authorization.as_ref(),
        )?;
        terminal_stdout_v1(json!({
            "status": "receipt-prepay",
            "journal": path.display().to_string(),
            "phase": journal.phase
        }))?;
        return Ok(false);
    }
    let prepay = plan_resolution_close_from_chain_v1(
        rpc,
        plan,
        evidence,
        arguments.market,
        arguments.payer,
        &[lookup_table],
    )?;
    let ChainResolutionCloseV1::NeedsReceiptPrepay {
        payer,
        receipt,
        exact_receipt_rent,
        prestate,
        ..
    } = prepay
    else {
        return Err(refusal(
            "receipt is funded but the session-required durable prepay journal is missing",
        ));
    };
    let table = prestate
        .iter()
        .find(|account| account.key == lookup_table)
        .ok_or_else(|| refusal("Resolution prepay snapshot omitted frozen ALT"))?;
    authenticate_supplied_terminal_lookup_table_v1(lookup_addresses, table, rent)?;
    let mut journal = build_resolution_receipt_prepay_journal_v1(
        rpc,
        &arguments.origin,
        &payer,
        &receipt,
        exact_receipt_rent,
        table,
        lookup_addresses,
        rent,
        &prestate,
        arguments.execute,
    )?;
    write_terminal_journal_v1(path, &mut journal, true)?;
    let authorization = authenticate_resolution_receipt_prepay_planned_journal_v1(
        &journal,
        &payer,
        &receipt,
        exact_receipt_rent,
        &prestate,
    )?;
    resume_terminal_journal_v1(
        rpc,
        &arguments.origin,
        path,
        &arguments.payer_keypair,
        arguments.execute,
        &mut journal,
        Some(&authorization),
    )?;
    terminal_stdout_v1(json!({
        "status": "receipt-prepay",
        "journal": path.display().to_string(),
        "phase": journal.phase,
        "topUpLamports": exact_receipt_rent.saturating_sub(receipt.lamports),
        "receiptRentLamports": exact_receipt_rent
    }))?;
    Ok(false)
}

fn stage_journal_name_v1(stage: TerminalStageV1) -> &'static str {
    match stage {
        TerminalStageV1::CoreBeginRetiring => "10-core-begin-retiring.json",
        TerminalStageV1::DirectBeginRetiring => "11-direct-begin-retiring.json",
        TerminalStageV1::ResolutionCloseFund => "13-resolution-close-fund.json",
        TerminalStageV1::DirectCloseCapability => "14-direct-close-capability.json",
        TerminalStageV1::RetirementReplayHandoff => "15-retirement-replay-handoff.json",
        TerminalStageV1::AggregateRetirement => "16-aggregate-retirement.json",
    }
}

fn parse_terminal_sequence_arguments_v1(
    arguments: Vec<String>,
) -> Result<TerminalSequenceArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut market_input = None;
    let mut evidence = None;
    let mut market = None;
    let mut payer = None;
    let mut payer_keypair = None;
    let mut session = None;
    let mut journal_dir = None;
    let mut supplied_lookup_table = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(Error::new("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--plan" => &mut plan,
            "--market-input" => &mut market_input,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--fee-payer" => &mut payer,
            "--fee-payer-keypair" => &mut payer_keypair,
            "--session" => &mut session,
            "--journal-dir" => &mut journal_dir,
            "--lookup-table" => &mut supplied_lookup_table,
            _ => {
                return Err(Error::new(format!(
                    "unknown devnet-terminal-sequence-v1 argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let required = |value: Option<String>, label: &str| {
        value.ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let absolute = |value: Option<String>, label: &str| -> Result<PathBuf> {
        let path = PathBuf::from(required(value, label)?);
        if !path.is_absolute() {
            return Err(Error::new(format!("{label} must be absolute")));
        }
        Ok(path)
    };
    Ok(TerminalSequenceArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: absolute(plan, "--plan")?,
        market_input: absolute(market_input, "--market-input")?,
        evidence: absolute(evidence, "--evidence")?,
        market: Pubkey::from_str(&required(market, "--market")?)
            .map_err(|error| Error::new(format!("--market: {error}")))?,
        payer: Pubkey::from_str(&required(payer, "--fee-payer")?)
            .map_err(|error| Error::new(format!("--fee-payer: {error}")))?,
        payer_keypair: absolute(payer_keypair, "--fee-payer-keypair")?,
        session: absolute(session, "--session")?,
        journal_dir: absolute(journal_dir, "--journal-dir")?,
        supplied_lookup_table: supplied_lookup_table
            .map(|value| {
                Pubkey::from_str(&value)
                    .map_err(|error| Error::new(format!("--lookup-table: {error}")))
            })
            .transpose()?,
        execute,
    })
}

fn terminal_stdout_v1(value: Value) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap devnet-terminal-sequence-v1 --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --plan ABSOLUTE_JSON \\
     --market-input ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY \\
     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_KEYPAIR \\
     --session ABSOLUTE_JSON --journal-dir ABSOLUTE_DIRECTORY \\
     [--lookup-table PUBKEY] [--execute]\n\nWithout --execute this command performs bounded \
     finalized devnet reads and persists exactly one unsigned durable next action before any key \
     can be opened. With --execute it reauthenticates that Planned intent through its stage's \
     semantic owner, reads only the named fee-payer key, persists the signed packet and local \
     signature before first send, and accepts only its exact finalized transaction, balances, \
     return data, and account poststate. Rerun to advance. If --lookup-table is absent, the same \
     journal machinery creates, extends, activates, and freezes a dedicated exact-union ALT before \
     protocol stage one. Mainnet-beta is refused unconditionally."
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::Cow,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use solana_address_lookup_table_interface::state::LookupTableMeta;

    use super::*;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn test_observation() -> Observation {
        Observation {
            slot: 101,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn test_account(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
    ) -> ObservedAccount {
        ObservedAccount {
            observation: test_observation(),
            key,
            owner,
            lamports,
            executable,
            data: Vec::new(),
        }
    }

    fn synthetic_system_transfer_journal() -> (
        DurableTerminalJournalV1,
        TerminalSemanticMutationV1,
        TerminalMetaClosureV1,
        Vec<ObservedAccount>,
    ) {
        let payer = key(1);
        let recipient = key(2);
        let instruction = solana_system_interface::instruction::transfer(&payer, &recipient, 10);
        let closure = TerminalMetaClosureV1 {
            stage: TerminalStageV1::CoreBeginRetiring,
            program_id: instruction.program_id,
            program_class: TerminalAddressClassV1::InlineProgram,
            accounts: instruction.accounts.clone(),
            classes: vec![
                TerminalAddressClassV1::InlineSigner,
                TerminalAddressClassV1::InlineRequestBound,
            ],
        };
        let message = compile_v0_message_with_optional_tables(
            payer,
            std::slice::from_ref(&instruction),
            Hash::new_from_array([7; 32]),
            test_observation(),
            &[],
        )
        .expect("synthetic v0 message");
        let VersionedMessage::V0(compiled) = &message.message else {
            panic!("synthetic v0")
        };
        let resolved = compiled.account_keys.clone();
        let prestate = resolved
            .iter()
            .map(|address| {
                if *address == payer {
                    test_account(*address, solana_sdk_ids::system_program::ID, 100, false)
                } else if *address == recipient {
                    test_account(*address, solana_sdk_ids::system_program::ID, 0, false)
                } else {
                    test_account(*address, solana_sdk_ids::native_loader::ID, 1, true)
                }
            })
            .collect::<Vec<_>>();
        let pre_balances = prestate
            .iter()
            .map(|account| account.lamports)
            .collect::<Vec<_>>();
        let post_balances = resolved
            .iter()
            .zip(pre_balances.iter().copied())
            .map(|(address, balance)| {
                if *address == payer {
                    balance - 15
                } else if *address == recipient {
                    balance + 10
                } else {
                    balance
                }
            })
            .collect::<Vec<_>>();
        let expected_accounts = BTreeMap::from([
            (
                payer.to_string(),
                DurableExpectedAccountV1 {
                    address: payer.to_string(),
                    owner: solana_sdk_ids::system_program::ID.to_string(),
                    lamports_after_protocol: 90,
                    lamports_after_fee: 85,
                    executable: false,
                    data_base64: String::new(),
                    data_sha256: sha256_hex(&[]),
                    lookup_table: None,
                },
            ),
            (
                recipient.to_string(),
                DurableExpectedAccountV1 {
                    address: recipient.to_string(),
                    owner: solana_sdk_ids::system_program::ID.to_string(),
                    lamports_after_protocol: 10,
                    lamports_after_fee: 10,
                    executable: false,
                    data_base64: String::new(),
                    data_sha256: sha256_hex(&[]),
                    lookup_table: None,
                },
            ),
        ]);
        let message_bytes = message.message.serialize();
        let intent = DurableTerminalIntentV1 {
            mutation: DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::CoreBeginRetiring,
            },
            observation_slot: test_observation().slot,
            observation_unix_timestamp: test_observation().unix_timestamp,
            payer: payer.to_string(),
            program_id: instruction.program_id.to_string(),
            program_class: TerminalAddressClassV1::InlineProgram,
            accounts: closure
                .accounts
                .iter()
                .zip(closure.classes.iter().copied())
                .map(|(meta, class)| DurableInstructionAccountV1 {
                    address: meta.pubkey.to_string(),
                    signer: meta.is_signer,
                    writable: meta.is_writable,
                    class,
                })
                .collect(),
            instruction_data_base64: BASE64.encode(&instruction.data),
            instruction_data_sha256: sha256_hex(&instruction.data),
            lookup_table: None,
            lookup_table_addresses: Vec::new(),
            lookup_table_addresses_sha256: pubkey_vector_sha256(&[]),
            loaded_writable: Vec::new(),
            loaded_readonly: Vec::new(),
            resolved_account_keys: resolved.iter().map(ToString::to_string).collect(),
            pre_balances,
            post_balances,
            recent_blockhash: Hash::new_from_array([7; 32]).to_string(),
            last_valid_block_height: 1_000,
            transaction_fee_lamports: 5,
            wire_bytes: message.wire_bytes,
            message_base64: BASE64.encode(&message_bytes),
            message_sha256: sha256_hex(&message_bytes),
            prestate: prestate
                .iter()
                .map(|account| (account.key.to_string(), durable_observed_state(account)))
                .collect(),
            expected_accounts,
            expected_return_data: None,
            protocol_lamport_deltas: BTreeMap::from([
                (payer.to_string(), -10),
                (recipient.to_string(), 10),
            ]),
        };
        let mutation = TerminalSemanticMutationV1 {
            stage: TerminalStageV1::CoreBeginRetiring,
            observation: test_observation(),
            instruction,
            expected_return_data: None,
            expected_accounts: vec![
                ExpectedAccountPoststateV1::exact(
                    payer,
                    solana_sdk_ids::system_program::ID,
                    90,
                    false,
                    Vec::new(),
                ),
                ExpectedAccountPoststateV1::exact(
                    recipient,
                    solana_sdk_ids::system_program::ID,
                    10,
                    false,
                    Vec::new(),
                ),
            ],
            protocol_lamport_deltas: BTreeMap::from([(payer, -10), (recipient, 10)]),
        };
        let mut journal = DurableTerminalJournalV1 {
            schema: TERMINAL_JOURNAL_SCHEMA_V1.into(),
            cluster: "devnet".into(),
            rpc_url: "https://example.invalid".into(),
            authorized_mutation: false,
            state_sha256: String::new(),
            phase: StageJournalPhaseV1::Planned,
            intent_sha256: sha256_hex(&serde_json::to_vec(&intent).expect("intent")),
            intent,
            signed_packet_base64: None,
            expected_signature: None,
            finalized: None,
        };
        refresh_terminal_journal_digest_v1(&mut journal).expect("journal digest");
        (journal, mutation, closure, prestate)
    }

    fn unique_test_path(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "dclutch-terminal-{label}-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn test_session(addresses: &[Pubkey]) -> TerminalSequenceSessionV1 {
        let mut session = TerminalSequenceSessionV1 {
            schema: TERMINAL_SESSION_SCHEMA_V1.into(),
            devnet_genesis_hash: DEVNET_GENESIS_HASH.into(),
            rpc_url: "https://example.invalid/".into(),
            plan_sha256: "11".repeat(32),
            market_input_sha256: "22".repeat(32),
            evidence_sha256: "33".repeat(32),
            market: key(3).to_string(),
            payer: key(1).to_string(),
            source_receipt: key(4).to_string(),
            receipt_initial_lamports: 7,
            receipt_rent_lamports: 9,
            supplied_lookup_table: false,
            lookup_table: key(5).to_string(),
            lookup_recent_slot: 99,
            lookup_addresses: addresses.iter().map(ToString::to_string).collect(),
            lookup_addresses_sha256: pubkey_vector_sha256(addresses),
            session_sha256: String::new(),
        };
        refresh_terminal_session_digest_v1(&mut session).expect("session digest");
        session
    }

    fn terminal_cli_arguments() -> Vec<String> {
        vec![
            "--rpc-url".into(),
            "http://127.0.0.1:20890/".into(),
            "--plan".into(),
            "/tmp/plan.json".into(),
            "--market-input".into(),
            "/tmp/market.json".into(),
            "--evidence".into(),
            "/tmp/evidence.json".into(),
            "--market".into(),
            key(3).to_string(),
            "--fee-payer".into(),
            key(1).to_string(),
            "--fee-payer-keypair".into(),
            "/tmp/payer.json".into(),
            "--session".into(),
            "/tmp/session.json".into(),
            "--journal-dir".into(),
            "/tmp/journals".into(),
        ]
    }

    fn observed_table(
        plan: &TerminalLookupTablePlanV1,
        rent: &Rent,
        addresses: Vec<Pubkey>,
        authority: Option<Pubkey>,
        last_extended_slot_start_index: u8,
        lamport_surplus: u64,
    ) -> ObservedAccount {
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority,
                last_extended_slot: 100,
                last_extended_slot_start_index,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        let data = table.serialize_for_tests().expect("table bytes");
        ObservedAccount {
            observation: Observation {
                slot: 101,
                unix_timestamp: 1_800_000_000,
                finality: Finality::Finalized,
            },
            key: plan.lookup_table,
            owner: lookup_table_program::id(),
            lamports: rent
                .minimum_balance(data.len())
                .checked_add(lamport_surplus)
                .expect("lamports"),
            executable: false,
            data,
        }
    }

    fn initial() -> AuthenticatedTerminalProgressV1 {
        AuthenticatedTerminalProgressV1 {
            core: CoreTerminalStateV1::Terminal,
            direct: DirectTerminalStateV1::Open,
            resolution: ResolutionTerminalStateV1::NeedsReceiptPrepayment,
            replay: RetirementReplayStateV1::Trading,
            outstanding_capabilities: 1,
            all_claim_supplies_zero: true,
            claims_aggregate_live: true,
            rent_credit_live: true,
            hoard_vault_live: true,
        }
    }

    #[test]
    fn exact_six_stage_route_and_operational_prepay_are_ordered() {
        let mut value = initial();
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::Execute(TerminalStageV1::CoreBeginRetiring)
        );
        value.core = CoreTerminalStateV1::Retiring;
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::Execute(TerminalStageV1::DirectBeginRetiring)
        );
        value.direct = DirectTerminalStateV1::Retiring;
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::PrepayResolutionReceipt
        );
        value.resolution = ResolutionTerminalStateV1::ReadyToClose;
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::Execute(TerminalStageV1::ResolutionCloseFund)
        );
        value.resolution = ResolutionTerminalStateV1::Closed;
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::Execute(TerminalStageV1::DirectCloseCapability)
        );
        value.direct = DirectTerminalStateV1::Closed;
        value.outstanding_capabilities = 0;
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::Execute(TerminalStageV1::RetirementReplayHandoff)
        );
        value.replay = RetirementReplayStateV1::Core;
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::Execute(TerminalStageV1::AggregateRetirement)
        );
        value.core = CoreTerminalStateV1::Closed;
        value.replay = RetirementReplayStateV1::Closed;
        value.claims_aggregate_live = false;
        value.rent_credit_live = false;
        value.hoard_vault_live = false;
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::Complete
        );
    }

    #[test]
    fn every_skipped_or_partial_shape_refuses() {
        let mut cases = Vec::new();
        let mut skipped_core = initial();
        skipped_core.direct = DirectTerminalStateV1::Retiring;
        cases.push(skipped_core);
        let mut skipped_direct = initial();
        skipped_direct.core = CoreTerminalStateV1::Retiring;
        skipped_direct.resolution = ResolutionTerminalStateV1::Closed;
        cases.push(skipped_direct);
        let mut close_without_decrement = initial();
        close_without_decrement.core = CoreTerminalStateV1::Retiring;
        close_without_decrement.direct = DirectTerminalStateV1::Closed;
        close_without_decrement.resolution = ResolutionTerminalStateV1::Closed;
        cases.push(close_without_decrement);
        let mut early_handoff = initial();
        early_handoff.core = CoreTerminalStateV1::Retiring;
        early_handoff.direct = DirectTerminalStateV1::Retiring;
        early_handoff.replay = RetirementReplayStateV1::Core;
        cases.push(early_handoff);
        let mut partial_aggregate = initial();
        partial_aggregate.core = CoreTerminalStateV1::Closed;
        partial_aggregate.direct = DirectTerminalStateV1::Closed;
        partial_aggregate.resolution = ResolutionTerminalStateV1::Closed;
        partial_aggregate.outstanding_capabilities = 0;
        partial_aggregate.replay = RetirementReplayStateV1::Closed;
        partial_aggregate.claims_aggregate_live = false;
        partial_aggregate.rent_credit_live = true;
        partial_aggregate.hoard_vault_live = false;
        cases.push(partial_aggregate);
        let mut reminted_claim = initial();
        reminted_claim.all_claim_supplies_zero = false;
        cases.push(reminted_claim);

        for case in cases {
            let error = route_terminal_progress_v1(case).unwrap_err();
            assert!(error.to_string().starts_with("REFUSED terminal sequence:"));
        }
    }

    #[test]
    fn unresolved_journal_never_silently_advances_or_replays() {
        assert!(
            authenticate_journal_route_v1(
                TerminalRouteV1::Execute(TerminalStageV1::DirectBeginRetiring),
                StageJournalPhaseV1::Planned,
                TerminalRouteV1::Execute(TerminalStageV1::DirectBeginRetiring),
            )
            .is_ok()
        );
        for phase in [
            StageJournalPhaseV1::SignedNotSubmitted,
            StageJournalPhaseV1::Submitted,
            StageJournalPhaseV1::Finalized,
        ] {
            assert!(
                authenticate_journal_route_v1(
                    TerminalRouteV1::Execute(TerminalStageV1::DirectBeginRetiring),
                    phase,
                    TerminalRouteV1::Execute(TerminalStageV1::ResolutionCloseFund),
                )
                .is_err()
            );
        }
        assert!(
            authenticate_journal_route_v1(
                TerminalRouteV1::Execute(TerminalStageV1::AggregateRetirement),
                StageJournalPhaseV1::Submitted,
                TerminalRouteV1::Complete,
            )
            .is_err()
        );
        assert!(
            authenticate_journal_route_v1(
                TerminalRouteV1::PrepayResolutionReceipt,
                StageJournalPhaseV1::Planned,
                TerminalRouteV1::Execute(TerminalStageV1::ResolutionCloseFund),
            )
            .is_err()
        );
    }

    #[test]
    fn stage_order_is_stable_and_exhaustive() {
        for (index, stage) in TerminalStageV1::ORDERED.into_iter().enumerate() {
            assert_eq!(usize::from(stage.ordinal()), index);
        }
    }

    #[test]
    fn terminal_alt_resumes_only_at_exact_pages_then_freezes() {
        let payer = key(1);
        let addresses = (10_u8..55).map(key).collect::<Vec<_>>();
        let rent = Rent::default();
        let plan = plan_terminal_lookup_table_v1(payer, 99, &addresses, &rent).expect("plan");
        assert_eq!(plan.addresses.len(), 45);
        assert_eq!(plan.extensions.len(), 3);
        assert!(plan.maximum_preflight_wire_bytes <= PACKET_DATA_BYTES);
        assert_eq!(
            route_terminal_lookup_table_v1(&plan, None, &rent).expect("vacant"),
            TerminalLookupTableRouteV1::Create(plan.create.clone())
        );

        for (prefix, start, extension_index) in [(0, 0, 0), (20, 0, 1), (40, 20, 2)] {
            let table = observed_table(
                &plan,
                &rent,
                plan.addresses[..prefix].to_vec(),
                Some(payer),
                start,
                0,
            );
            assert_eq!(
                route_terminal_lookup_table_v1(&plan, Some(&table), &rent).expect("prefix"),
                TerminalLookupTableRouteV1::Extend {
                    prefix_len: prefix,
                    instruction: plan.extensions[extension_index].clone(),
                }
            );
        }

        let mutable = observed_table(&plan, &rent, plan.addresses.clone(), Some(payer), 40, 0);
        assert_eq!(
            route_terminal_lookup_table_v1(&plan, Some(&mutable), &rent).expect("full mutable"),
            TerminalLookupTableRouteV1::Freeze(plan.freeze.clone())
        );
        let frozen = observed_table(&plan, &rent, plan.addresses.clone(), None, 40, 0);
        assert_eq!(
            route_terminal_lookup_table_v1(&plan, Some(&frozen), &rent).expect("frozen"),
            TerminalLookupTableRouteV1::Complete
        );
        authenticate_supplied_terminal_lookup_table_v1(&plan.addresses, &frozen, &rent)
            .expect("supplied exact frozen table");
    }

    #[test]
    fn terminal_alt_refuses_divergence_partial_freeze_surplus_and_wrong_boundary() {
        let payer = key(1);
        let addresses = (10_u8..55).map(key).collect::<Vec<_>>();
        let rent = Rent::default();
        let plan = plan_terminal_lookup_table_v1(payer, 99, &addresses, &rent).expect("plan");

        let mut divergent = plan.addresses[..20].to_vec();
        divergent[19] = key(99);
        let cases = [
            observed_table(&plan, &rent, divergent, Some(payer), 0, 0),
            observed_table(&plan, &rent, plan.addresses[..19].to_vec(), None, 0, 0),
            observed_table(
                &plan,
                &rent,
                plan.addresses[..20].to_vec(),
                Some(payer),
                0,
                1,
            ),
            observed_table(
                &plan,
                &rent,
                plan.addresses[..20].to_vec(),
                Some(payer),
                1,
                0,
            ),
            observed_table(
                &plan,
                &rent,
                plan.addresses[..19].to_vec(),
                Some(payer),
                0,
                0,
            ),
        ];
        for table in cases {
            assert!(route_terminal_lookup_table_v1(&plan, Some(&table), &rent).is_err());
        }

        let mutable = observed_table(&plan, &rent, plan.addresses.clone(), Some(payer), 40, 0);
        assert!(
            authenticate_supplied_terminal_lookup_table_v1(&plan.addresses, &mutable, &rent)
                .is_err()
        );
    }

    #[test]
    fn terminal_alt_union_uses_only_semantic_owner_lookup_stable_classes() {
        let payer = key(1);
        let mut closures = TerminalStageV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, stage)| meta_closure(stage, u8::try_from(index + 10).unwrap()))
            .collect::<Vec<_>>();
        closures[4].accounts.extend([
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new_readonly(key(80), false),
            AccountMeta::new_readonly(key(81), false),
        ]);
        closures[4].classes.extend([
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::InlineRequestBound,
        ]);
        let union = terminal_lookup_union_from_closures_v1(payer, &closures).expect("union");
        assert!(!union.contains(&payer));
        assert!(!union.contains(&key(80)));
        assert!(!union.contains(&key(81)));
        assert!(union.contains(&key(90)));
    }

    fn meta_closure(stage: TerminalStageV1, byte: u8) -> TerminalMetaClosureV1 {
        TerminalMetaClosureV1 {
            stage,
            program_id: key(byte.saturating_add(100)),
            program_class: TerminalAddressClassV1::InlineProgram,
            accounts: vec![
                AccountMeta::new(key(byte), false),
                AccountMeta::new_readonly(key(90), false),
            ],
            classes: vec![
                TerminalAddressClassV1::LookupStable,
                TerminalAddressClassV1::LookupStable,
            ],
        }
    }

    #[test]
    fn terminal_alt_union_requires_all_six_typed_closures_in_order() {
        let payer = key(1);
        let closures = TerminalStageV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, stage)| meta_closure(stage, u8::try_from(index + 10).unwrap()))
            .collect::<Vec<_>>();
        let union = terminal_lookup_union_from_closures_v1(payer, &closures).expect("six closures");
        assert_eq!(union.len(), 7, "six distinct roles plus one shared role");
        assert!(union.contains(&key(90)));

        assert!(terminal_lookup_union_from_closures_v1(payer, &closures[..5]).is_err());
        let mut reordered = closures.clone();
        reordered.swap(1, 2);
        assert!(terminal_lookup_union_from_closures_v1(payer, &reordered).is_err());
        let mut vacant = closures;
        vacant[0].accounts[0].pubkey = Pubkey::default();
        assert!(terminal_lookup_union_from_closures_v1(payer, &vacant).is_err());
    }

    #[test]
    fn fresh_stage_instruction_must_equal_its_coordinate_closure() {
        let closure =
            core_begin_retiring_meta_closure_v1(key(10), key(11), key(12), key(13), key(14));
        let exact = Instruction {
            program_id: closure.program_id,
            accounts: closure.accounts.clone(),
            data: vec![7; 96],
        };
        closure
            .authenticate_instruction(&exact)
            .expect("exact semantic frame");
        let mut substituted = exact.clone();
        substituted.accounts[2].pubkey = key(99);
        assert!(closure.authenticate_instruction(&substituted).is_err());
        let mut privilege = exact.clone();
        privilege.accounts[2].is_writable = true;
        assert!(closure.authenticate_instruction(&privilege).is_err());
        let mut shifted = exact;
        shifted.accounts.swap(1, 2);
        assert!(closure.authenticate_instruction(&shifted).is_err());

        let mut reclassified = closure.clone();
        reclassified.classes[0] = TerminalAddressClassV1::InlineRequestBound;
        assert!(closure.authenticate_fresh_closure(&reclassified).is_err());
    }

    #[test]
    fn replay_handoff_request_replan_changes_inline_pda_but_not_frozen_alt() {
        let payer = key(1);
        let mut before = TerminalStageV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, stage)| meta_closure(stage, u8::try_from(index + 10).unwrap()))
            .collect::<Vec<_>>();
        before[4].accounts.extend([
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new_readonly(key(80), false),
            AccountMeta::new_readonly(key(81), false),
        ]);
        before[4].classes.extend([
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::InlineRequestBound,
        ]);
        let before_union =
            terminal_lookup_union_from_closures_v1(payer, &before).expect("initial stable union");

        let mut replanned = before.clone();
        replanned[4].accounts[4].pubkey = key(82);
        let replan_union = terminal_lookup_union_from_closures_v1(payer, &replanned)
            .expect("replanned stable union");
        assert_eq!(before_union, replan_union);
        assert!(before[4].authenticate_fresh_closure(&replanned[4]).is_err());

        let mut misclassified = replanned.clone();
        misclassified[4].classes[4] = TerminalAddressClassV1::LookupStable;
        let wrong_union = terminal_lookup_union_from_closures_v1(payer, &misclassified)
            .expect("misclassification is visible in projected union");
        assert_ne!(before_union, wrong_union);

        let mut conflicting = before;
        conflicting[0]
            .accounts
            .push(AccountMeta::new_readonly(payer, false));
        conflicting[0]
            .classes
            .push(TerminalAddressClassV1::LookupStable);
        assert!(terminal_lookup_union_from_closures_v1(payer, &conflicting).is_err());
    }

    #[test]
    fn terminal_v0_places_stable_keys_in_frozen_alt_and_every_other_class_inline() {
        let payer = key(1);
        let rent = Rent::default();
        let mut closures = TerminalStageV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, stage)| meta_closure(stage, u8::try_from(index + 10).unwrap()))
            .collect::<Vec<_>>();
        closures[4].accounts.extend([
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new_readonly(key(80), false),
            AccountMeta::new_readonly(key(81), false),
        ]);
        closures[4].classes.extend([
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::InlineRequestBound,
        ]);
        let union = terminal_lookup_union_from_closures_v1(payer, &closures).expect("stable union");
        let plan = plan_terminal_lookup_table_v1(payer, 99, &union, &rent).expect("ALT plan");
        let table = observed_table(&plan, &rent, union, None, 0, 0);
        let instruction = Instruction {
            program_id: closures[4].program_id,
            accounts: closures[4].accounts.clone(),
            data: vec![7; 208],
        };
        let compiled = compile_v0_message_with_optional_tables(
            payer,
            &[instruction],
            Hash::new_from_array([9; 32]),
            table.observation,
            std::slice::from_ref(&table),
        )
        .expect("v0 compile");
        authenticate_terminal_v0_placement_v1(
            payer,
            &closures[4],
            table.key,
            &plan.addresses,
            &compiled,
        )
        .expect("exact placement");

        let mut altered = closures[4].clone();
        altered.classes[4] = TerminalAddressClassV1::LookupStable;
        assert!(
            authenticate_terminal_v0_placement_v1(
                payer,
                &altered,
                table.key,
                &plan.addresses,
                &compiled,
            )
            .is_err()
        );
    }

    #[test]
    fn resolution_close_closure_derives_request_bound_authority_and_exact_frame() {
        let coordinates = ResolutionCloseMetaCoordinatesV1 {
            release_set: [3; 32],
            role_request_digest: [4; 32],
            market: key(10),
            activation_cache: key(11),
            registry_program: key(12),
            core_program: key(13),
            core_programdata: key(14),
            resolution_program: key(15),
            resolution_programdata: key(16),
            source_material: key(17),
            source_material_staging: key(18),
            capability_manifest: key(19),
            capability_manifest_staging: key(20),
            source_state: key(21),
            funding_ledger: key(22),
            certificate: key(23),
            closure_receipt: key(24),
            beneficiary: key(25),
            clock_sysvar: key(26),
            rent_sysvar: key(27),
            system_program: key(28),
            recovery_policy: Some((key(29), key(30))),
        };
        let closure = resolution_close_meta_closure_v1(&coordinates).expect("closure");
        assert_eq!(closure.accounts.len(), 22);
        assert_eq!(closure.accounts[1].pubkey, coordinates.market);
        assert_eq!(closure.accounts[15].pubkey, coordinates.closure_receipt);
        assert_eq!(closure.accounts[20].pubkey, key(29));
        assert_eq!(closure.accounts[21].pubkey, key(30));
        assert_eq!(
            closure
                .accounts
                .iter()
                .enumerate()
                .filter_map(|(index, account)| account.is_writable.then_some(index))
                .collect::<Vec<_>>(),
            vec![1, 12, 13, 15, 16]
        );

        let mut another = coordinates;
        another.role_request_digest[0] ^= 1;
        let another = resolution_close_meta_closure_v1(&another).expect("other request");
        assert_ne!(closure.accounts[0].pubkey, another.accounts[0].pubkey);
    }

    #[test]
    fn self_consistent_journal_cannot_authorize_another_stage_owner() {
        let (journal, mutation, closure, prestate) = synthetic_system_transfer_journal();
        authenticate_terminal_journal_v1(&journal).expect("self-consistent durable envelope");
        authenticate_planned_protocol_owner_v1(
            &journal,
            DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::CoreBeginRetiring,
            },
            &mutation,
            &closure,
            &prestate,
        )
        .expect("exact rerun owner output");

        let mut substituted = mutation;
        substituted.instruction =
            solana_system_interface::instruction::transfer(&key(1), &key(2), 9);
        assert!(
            authenticate_planned_protocol_owner_v1(
                &journal,
                DurableTerminalMutationV1::Protocol {
                    stage: TerminalStageV1::CoreBeginRetiring,
                },
                &substituted,
                &closure,
                &prestate,
            )
            .is_err()
        );
    }

    #[test]
    fn durable_message_requires_canonical_payer_header_and_static_prefix() {
        let (mut journal, _, _, _) = synthetic_system_transfer_journal();
        let bytes = BASE64
            .decode(&journal.intent.message_base64)
            .expect("message");
        let mut message: VersionedMessage = bincode::deserialize(&bytes).expect("v0");
        let VersionedMessage::V0(value) = &mut message else {
            panic!("v0")
        };
        value.header.num_readonly_signed_accounts = 1;
        let bytes = message.serialize();
        journal.intent.message_base64 = BASE64.encode(&bytes);
        journal.intent.message_sha256 = sha256_hex(&bytes);
        journal.intent_sha256 = sha256_hex(&serde_json::to_vec(&journal.intent).expect("intent"));
        refresh_terminal_journal_digest_v1(&mut journal).expect("state digest");
        assert!(authenticate_terminal_journal_v1(&journal).is_err());
    }

    #[test]
    fn planned_owner_rerun_accepts_a_later_identical_snapshot_only() {
        let (journal, mut mutation, closure, mut prestate) = synthetic_system_transfer_journal();
        mutation.observation.slot += 1;
        for account in &mut prestate {
            account.observation = mutation.observation;
        }
        authenticate_planned_protocol_owner_v1(
            &journal,
            DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::CoreBeginRetiring,
            },
            &mutation,
            &closure,
            &prestate,
        )
        .expect("later exact finalized snapshot");

        prestate[0].lamports += 1;
        assert!(
            authenticate_planned_protocol_owner_v1(
                &journal,
                DurableTerminalMutationV1::Protocol {
                    stage: TerminalStageV1::CoreBeginRetiring,
                },
                &mutation,
                &closure,
                &prestate,
            )
            .is_err()
        );
    }

    #[test]
    fn recomputed_session_checksum_cannot_substitute_semantic_alt_union() {
        let exact = vec![key(10), key(11), key(12)];
        let mut session = test_session(&exact);
        authenticate_terminal_session_v1(&session).expect("canonical envelope");
        authenticate_terminal_session_union_v1(&session, key(4), &exact).expect("semantic union");

        let substituted = vec![key(10), key(11), key(13)];
        session.lookup_addresses = substituted.iter().map(ToString::to_string).collect();
        session.lookup_addresses_sha256 = pubkey_vector_sha256(&substituted);
        refresh_terminal_session_digest_v1(&mut session).expect("edited checksum");
        authenticate_terminal_session_v1(&session)
            .expect("checksum detects corruption, not semantic authority");
        assert!(authenticate_terminal_session_union_v1(&session, key(4), &exact).is_err());

        session.source_receipt = key(99).to_string();
        refresh_terminal_session_digest_v1(&mut session).expect("edited receipt checksum");
        assert!(authenticate_terminal_session_union_v1(&session, key(4), &exact).is_err());
    }

    #[test]
    fn terminal_session_publication_is_no_clobber_and_decode_is_hostile() {
        let path = unique_test_path("session");
        let session = test_session(&[key(10), key(11)]);
        assert!(read_terminal_session_v1(Path::new("relative-session.json")).is_err());
        write_new_terminal_session_v1(&path, &session).expect("publish session");
        assert_eq!(
            read_terminal_session_v1(&path).expect("read session"),
            session
        );
        assert!(write_new_terminal_session_v1(&path, &session).is_err());
        assert_eq!(read_terminal_session_v1(&path).expect("preserved"), session);
        let _ = fs::remove_file(&path);

        let exact = serde_json::to_string(&session).expect("session JSON");
        let duplicate = exact.replacen("\"schema\":", "\"schema\":\"substituted\",\"schema\":", 1);
        fs::write(&path, duplicate).expect("duplicate session");
        assert!(read_terminal_session_v1(&path).is_err());
        let unknown = exact.replacen("{", "{\"unownedTerminalTruth\":true,", 1);
        fs::write(&path, unknown).expect("unknown session");
        assert!(read_terminal_session_v1(&path).is_err());
        fs::write(&path, format!("{exact}\ntrue")).expect("trailing session value");
        assert!(read_terminal_session_v1(&path).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn terminal_cli_is_read_only_by_default_and_refuses_ambiguous_shape() {
        let arguments = terminal_cli_arguments();
        let parsed = parse_terminal_sequence_arguments_v1(arguments.clone()).expect("read plan");
        assert!(!parsed.execute);

        let mut execute = arguments.clone();
        execute.push("--execute".into());
        assert!(
            parse_terminal_sequence_arguments_v1(execute)
                .expect("execute")
                .execute
        );
        let mut duplicate = arguments.clone();
        duplicate.extend(["--execute".into(), "--execute".into()]);
        assert!(parse_terminal_sequence_arguments_v1(duplicate).is_err());
        let mut unknown = arguments.clone();
        unknown.extend(["--unknown".into(), "value".into()]);
        assert!(parse_terminal_sequence_arguments_v1(unknown).is_err());
        let mut relative = arguments;
        let plan_index = relative.iter().position(|value| value == "--plan").unwrap() + 1;
        relative[plan_index] = "relative.json".into();
        assert!(parse_terminal_sequence_arguments_v1(relative).is_err());
    }

    #[test]
    fn terminal_protocol_and_prepay_journals_require_one_exact_prefix() {
        let directory = unique_test_path("prefix-dir");
        fs::create_dir(&directory).expect("journal directory");
        let second = directory.join(stage_journal_name_v1(TerminalStageV1::DirectBeginRetiring));
        fs::write(&second, b"later").expect("later stage");
        assert!(authenticate_terminal_journal_prefix_v1(&directory, false).is_err());
        fs::remove_file(&second).expect("remove later stage");

        for stage in [
            TerminalStageV1::CoreBeginRetiring,
            TerminalStageV1::DirectBeginRetiring,
        ] {
            fs::write(directory.join(stage_journal_name_v1(stage)), b"prefix")
                .expect("prefix stage");
        }
        fs::write(
            directory.join(stage_journal_name_v1(TerminalStageV1::ResolutionCloseFund)),
            b"close without prepay",
        )
        .expect("later close");
        assert!(authenticate_terminal_journal_prefix_v1(&directory, true).is_err());
        fs::remove_dir_all(directory).expect("remove journal directory");
    }

    #[test]
    fn journal_publication_is_no_clobber_and_updates_refuse_stale_writers() {
        let path = unique_test_path("writer");
        let lock = path.with_file_name(format!(
            ".{}.terminal-sequence.lock",
            path.file_name().unwrap().to_str().unwrap()
        ));
        let (mut journal, _, _, _) = synthetic_system_transfer_journal();
        write_terminal_journal_v1(&path, &mut journal, true).expect("publish");
        let original = fs::read(&path).expect("original");
        let mut duplicate = journal.clone();
        assert!(write_terminal_journal_v1(&path, &mut duplicate, true).is_err());
        assert_eq!(fs::read(&path).expect("preserved"), original);
        assert!(!lock.exists());

        let mut first = read_terminal_journal_v1(&path).expect("first writer");
        let mut stale = first.clone();
        first.authorized_mutation = true;
        write_terminal_journal_v1(&path, &mut first, false).expect("first update");
        stale.rpc_url.push_str("/stale");
        assert!(write_terminal_journal_v1(&path, &mut stale, false).is_err());
        assert_eq!(
            read_terminal_journal_v1(&path)
                .expect("winner")
                .state_sha256,
            first.state_sha256
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn writer_failure_after_lock_acquisition_leaves_fail_closed_lock() {
        let path = unique_test_path("fail-closed");
        let name = path.file_name().unwrap().to_str().unwrap();
        let lock = path.with_file_name(format!(".{name}.terminal-sequence.lock"));
        let temporary = path.with_file_name(format!(
            ".{name}.terminal-sequence-{}.tmp",
            std::process::id()
        ));
        fs::write(&temporary, b"occupied").expect("occupy exact temporary");
        let (mut journal, _, _, _) = synthetic_system_transfer_journal();
        assert!(write_terminal_journal_v1(&path, &mut journal, true).is_err());
        assert!(lock.exists(), "ambiguous interrupted writer remains locked");
        assert!(!path.exists());
        let _ = fs::remove_file(temporary);
        let _ = fs::remove_file(lock);
    }
}
