//! Durable exterior lifecycle for one ordinary Direct trade.
//!
//! The semantic transaction and lookup geometry remain owned by
//! `dclutch_operator::direct_inline_route_v3`. This module owns only the
//! external-cluster membrane: strict public/private manifests, one finalized
//! observation, write-ahead transaction journals, exact frozen-ALT
//! reconciliation, capability-seal materialization, and finalized transaction
//! evidence. Hot submission remains refused until the operator exposes one
//! public exact `HotExecutionAckV3` and complete poststate owner.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_program_contract::hot_v3::HotExecutionAckV3;
use dclutch_claims_svm::{
    liability_basis_state_v2::LiabilityBasisPositionViewV2,
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_direct_codec::{
    intent_v2::CompactIntentV2,
    ordinary_v3::DirectOrdinaryAuthenticatedContextV3,
    replay_setup_v1::{DirectReplaySetupReceiptV1, DirectReplaySetupRequestV1},
    successor::DirectExecutionConfigV1,
    token_setup_v1::{
        DirectTokenAccountRoleV1, DirectTokenAccountSeedsV1, DirectTokenSetupReceiptV1,
        DirectTokenSetupRequestV1,
    },
};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    direct_inline_route_v3::{
        DirectClaimsRouteV3, DirectCustodyRouteV3, DirectHotFixedRouteV3,
        DirectInlineAuthenticatedRouteV3, DirectInlineCapabilitySealPlanV3,
        DirectInlineCheckedProgramAccountsV3, DirectInlineHotFinalizationPlanV3,
        DirectInlineLookupTableProvisionV3, DirectInlineOrdinaryRouteV3,
        DirectInlinePhysicalRouteV3, DirectInlineRouteAuthenticationV3, FinalizedRecordRouteV3,
        assemble_authenticated_direct_inline_ordinary_route_v3,
        assemble_direct_inline_ordinary_route_v3, build_direct_inline_capability_seal_v3,
        build_direct_inline_lookup_table_provision_v3,
        compile_direct_inline_capability_seal_routed_v0_v3, compile_direct_inline_routed_v0_v3,
        prepare_direct_inline_hot_finalization_v3,
        project_direct_inline_sealed_execution_physical_v3,
        rederive_direct_inline_lookup_table_provision_v3, verify_direct_inline_capability_seal_v3,
    },
    direct_inline_v3::{
        CheckedHotOuterReleaseV3, DirectInlineHotStateV3, SignedDirectIntentV3,
        build_direct_inline_hot_v4, compile_direct_inline_request_v3,
    },
};
use dclutch_release_tool::CheckedExecutionReleaseSetV1;
use dclutch_token_svm::TokenAccount;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_hash::Hash as SolanaHash;
use solana_program::{hash::hash, pubkey::Pubkey, rent::Rent};
use solana_sdk::{
    message::{Message, VersionedMessage},
    signature::{Keypair, Signature, Signer as _},
    transaction::VersionedTransaction,
};
use solana_sdk_ids::{compute_budget, ed25519_program, system_program, sysvar};

use crate::direct_trade_setup::{
    DirectReplaySetupBuildInputV1, DirectReplaySetupCoordinatesV1,
    DirectReplaySetupDerivationInputV1, DirectReplaySetupPlanV1, DirectSetupObservedAccountV1,
    build_direct_replay_setup_v1, derive_direct_replay_setup_accounts_v1,
    project_direct_replay_setup_poststate_v1, verify_direct_replay_setup_poststate_v1,
};
use crate::direct_trade_setup_journal::{
    DIRECT_SETUP_JOURNAL_SCHEMA_V1, DirectSetupAccountPoststateV1, DirectSetupFinalizationV1,
    DirectSetupJournalPhaseV1, DirectSetupJournalPlanV1, DirectSetupJournalV1,
    DirectSetupManifestBindingV1, DirectSetupRecoveryActionV1, DirectSetupReturnDataV1,
    DirectSetupStageV1, advance_direct_setup_dispatching_v1, advance_direct_setup_finalized_v1,
    advance_direct_setup_signed_v1, advance_direct_setup_submitted_v1,
    authenticate_direct_setup_against_plan_v1, authenticate_direct_setup_chain_v1,
    authenticate_direct_setup_journal_v1, direct_setup_recovery_action_v1,
    plan_direct_replay_setup_journal_v1, plan_direct_token_setup_journal_v1,
};
use crate::direct_trade_token_setup::{
    DirectTradeTokenSetupBuildInputV1, DirectTradeTokenSetupCoordinatesV1,
    DirectTradeTokenSetupObservedAccountV1, DirectTradeTokenSetupPlanV1,
    DirectTradeTokenSetupPoststateV1, build_direct_trade_token_setup_v1,
    verify_direct_trade_token_setup_poststate_v1,
};
use crate::{
    Error, Result, campaign,
    cluster::{
        ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH, ExpectedClusterV1,
    },
    model::{MarketRunInput, ProgramPin, SuccessorPlan},
    plan::pubkey,
    rpc::{Rpc, RpcAccount, WritePolicyV1, parse_json_without_duplicate_keys_v1},
    terminal_lifecycle::finalized_snapshot,
    wallet_terminal::FinalizedSnapshotV1,
};

const PUBLIC_MANIFEST_SCHEMA_V1: &str = "dclutch-devnet-direct-trade-public-manifest-v1";
const PRIVATE_SESSION_SCHEMA_V1: &str = "dclutch-devnet-direct-trade-private-session-v1";
const JOURNAL_SCHEMA_V1: &str = "dclutch-devnet-direct-trade-journal-v1";
const OWNED_PUBLIC_MANIFEST_SCHEMA_V1: &str =
    "dclutch-owned-loopback-direct-trade-public-manifest-v1";
const OWNED_PRIVATE_SESSION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-direct-trade-private-session-v1";
const OWNED_JOURNAL_SCHEMA_V1: &str = "dclutch-owned-loopback-direct-trade-journal-v1";
const DEVNET_EVIDENCE_SCHEMA_V1: &str = "dclutch-devnet-direct-trade-finalized-v1";
const OWNED_EVIDENCE_SCHEMA_V1: &str = "dclutch-owned-loopback-direct-trade-finalized-v1";

fn direct_public_schema_v1(cluster: ExpectedClusterV1) -> &'static str {
    match cluster {
        ExpectedClusterV1::Devnet => PUBLIC_MANIFEST_SCHEMA_V1,
        ExpectedClusterV1::OwnedLoopback => OWNED_PUBLIC_MANIFEST_SCHEMA_V1,
    }
}

fn direct_private_schema_v1(cluster: ExpectedClusterV1) -> &'static str {
    match cluster {
        ExpectedClusterV1::Devnet => PRIVATE_SESSION_SCHEMA_V1,
        ExpectedClusterV1::OwnedLoopback => OWNED_PRIVATE_SESSION_SCHEMA_V1,
    }
}

fn direct_journal_schema_v1(cluster: &str) -> Result<&'static str> {
    match cluster {
        "devnet" => Ok(JOURNAL_SCHEMA_V1),
        "owned-loopback" => Ok(OWNED_JOURNAL_SCHEMA_V1),
        _ => Err(refusal("Direct manifest cluster label changed")),
    }
}

fn direct_evidence_schema_v1(cluster: &str) -> Result<&'static str> {
    match cluster {
        "devnet" => Ok(DEVNET_EVIDENCE_SCHEMA_V1),
        "owned-loopback" => Ok(OWNED_EVIDENCE_SCHEMA_V1),
        _ => Err(refusal("Direct manifest cluster label changed")),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SignedIntentManifestV1 {
    maker: String,
    intent_base64: String,
    signature_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RecordPairCoordinatesV1 {
    raw: String,
    staging: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectFixedCoordinatesV1 {
    market: String,
    root: String,
    manifest: RecordPairCoordinatesV1,
    program_set: RecordPairCoordinatesV1,
    descriptor: RecordPairCoordinatesV1,
    config: RecordPairCoordinatesV1,
    account_profile: RecordPairCoordinatesV1,
    request_profile: RecordPairCoordinatesV1,
    transition: RecordPairCoordinatesV1,
    effect: RecordPairCoordinatesV1,
    lifecycle: RecordPairCoordinatesV1,
    strategy: RecordPairCoordinatesV1,
    activation_cache: String,
    core_program: String,
    core_programdata: String,
    trading_program: String,
    trading_programdata: String,
    registry_program: String,
    product: RecordPairCoordinatesV1,
    result_domain: RecordPairCoordinatesV1,
    portfolio: RecordPairCoordinatesV1,
    linked_basis: RecordPairCoordinatesV1,
    capability_seal: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectClaimsCoordinatesV1 {
    caller_authority: String,
    aggregate: String,
    claims_program: String,
    claims_programdata: String,
    seller_position: String,
    buyer_position: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectCustodyCoordinatesV1 {
    caller_authorities: [String; 4],
    realm: RecordPairCoordinatesV1,
    replay: String,
    mint: String,
    buyer_token: String,
    seller_token: String,
    fee_token: String,
    custody_authority: String,
    token_program: String,
    custody_program: String,
    custody_programdata: String,
}

/// Untrusted coordinate hints. The operator authenticates every PDA, owner,
/// release, artifact, Product, Realm, Claims, Custody, and token join before a
/// table or seal plan is admitted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectRouteCoordinatesV1 {
    fixed: DirectFixedCoordinatesV1,
    seller_maker: String,
    payer: String,
    lifecycle_rent_credit: String,
    buyer_maker: String,
    rent_program: String,
    claims: DirectClaimsCoordinatesV1,
    custody: DirectCustodyCoordinatesV1,
}

/// Scalar observations which cannot be known from addresses alone. These are
/// routing hints, never persisted authority: the production operator derives
/// every one again from finalized account bytes and requires exact equality.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectContextHintsV1 {
    generation: u64,
    outcome_count: u32,
    root_phase: u8,
    seller_next_nonce: u64,
    buyer_next_nonce: u64,
    root_open_maker_count: u64,
    seller_created: bool,
    seller_bump_observation: u8,
    seller_bump: u8,
    seller_rent_principal_observation: u64,
    seller_rent_principal: u64,
    buyer_created: bool,
    buyer_bump_observation: u8,
    buyer_bump: u8,
    buyer_rent_principal_observation: u64,
    buyer_rent_principal: u64,
    claims_market_revision: u64,
    seller_position_revision: u64,
    buyer_position_revision: u64,
    custody_revision: u64,
    release_set: String,
    semantic_basis: String,
    seller_rent_beneficiary: String,
    seller_rent_beneficiary_observation: String,
    buyer_rent_beneficiary: String,
    buyer_rent_beneficiary_observation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectTradePublicManifestV1 {
    schema: String,
    cluster: String,
    genesis_hash: String,
    plan_sha256: String,
    market_input_sha256: String,
    market: String,
    payer: String,
    fill: u64,
    execution_price: u64,
    fee_basis_points: u16,
    fee_recipient: String,
    pub(crate) checked_execution_release_set_base64: String,
    seller: SignedIntentManifestV1,
    buyer: SignedIntentManifestV1,
    route: DirectRouteCoordinatesV1,
    context: DirectContextHintsV1,
    replay_setup: DirectReplaySetupManifestV1,
    token_setup: DirectTokenSetupManifestV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectReplaySetupManifestV1 {
    request_base64: String,
    request_sha256: String,
    maker: String,
    maker_root: String,
    custody_replay: String,
    payer: String,
    rent_refund: String,
    expected_initial_revision: u64,
    expected_resulting_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectTokenSetupManifestV1 {
    request_base64: String,
    request_sha256: String,
    seller_token: String,
    seller_owner: String,
    fee_token: String,
    fee_recipient: String,
    payer: String,
    rent_refund: String,
    mint: String,
    token_program: String,
    trading_program: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectTradePrivateSessionV1 {
    schema: String,
    public_manifest: String,
    public_manifest_sha256: String,
    plan: String,
    market_input: String,
    payer_keypair: String,
    journal_dir: String,
    evidence_file: String,
    session_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DirectTradeStageV1 {
    LookupCreate,
    LookupExtend,
    LookupFreeze,
    LookupActivation,
    CapabilitySeal,
    Hot,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DirectTradeJournalPhaseV1 {
    Planned,
    Prepared,
    Dispatching,
    Submitted,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectTradeJournalV1 {
    schema: String,
    public_manifest_sha256: String,
    private_session_sha256: String,
    stage: DirectTradeStageV1,
    action_index: u16,
    phase: DirectTradeJournalPhaseV1,
    observation_slot: u64,
    lookup_creation_slot: u64,
    lookup_table: String,
    lookup_addresses: Vec<String>,
    lookup_addresses_sha256: String,
    message_base64: Option<String>,
    message_sha256: Option<String>,
    last_valid_block_height: Option<u64>,
    exact_fee_lamports: Option<u64>,
    expected_wire_bytes: Option<usize>,
    unique_message_account_count: Option<usize>,
    signed_packet_base64: Option<String>,
    expected_signature: Option<String>,
    expected_return_data_producer: Option<String>,
    expected_return_data_base64: Option<String>,
    expected_prestates: Vec<DirectTradeExpectedPoststateV1>,
    expected_poststates: Vec<DirectTradeExpectedPoststateV1>,
    finalized_slot: Option<u64>,
    transaction_sha256: Option<String>,
    fee_lamports: Option<u64>,
    compute_units_consumed: Option<u64>,
    return_data_producer: Option<String>,
    return_data_base64: Option<String>,
    return_data_was_null: Option<bool>,
    finalized_poststates: Vec<DirectTradeObservedPoststateV1>,
    intent_sha256: String,
    state_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectTradeExpectedPoststateV1 {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data_base64: String,
    pub(crate) data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectTradeObservedPoststateV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_len: usize,
    data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectFinalizedMutationEvidenceV1 {
    pub(crate) kind: String,
    pub(crate) prefix_len: Option<usize>,
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) intent_sha256: String,
    pub(crate) schema: String,
    pub(crate) completion_pointer: String,
    pub(crate) completion_value: String,
    pub(crate) signature: String,
    pub(crate) slot: u64,
    pub(crate) fee_payer: String,
    pub(crate) fee_lamports: u64,
    pub(crate) compute_units_consumed: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectLookupActivationEvidenceV1 {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) intent_sha256: String,
    pub(crate) schema: String,
    pub(crate) completion_pointer: String,
    pub(crate) completion_value: String,
    pub(crate) finalized_slot: u64,
    pub(crate) lookup_table: String,
    pub(crate) lookup_addresses_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectPositionTransitionEvidenceV1 {
    pub(crate) account: String,
    pub(crate) owner: String,
    pub(crate) pre_data_base64: String,
    pub(crate) post_data_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectClaimBalanceEvidenceV1 {
    pub(crate) owner: String,
    pub(crate) position: String,
    pub(crate) recipient_token: String,
    pub(crate) claim_index: u32,
    pub(crate) quantity_atoms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DirectTradeFinalizedEvidenceV1 {
    schema: String,
    status: String,
    cluster: String,
    public_manifest_sha256: String,
    public_manifest_base64: String,
    private_session_sha256: String,
    journal_state_sha256: String,
    hot_journal_base64: String,
    signature: String,
    finalized_slot: u64,
    market: String,
    seller_owner: String,
    seller_position: String,
    buyer_position: String,
    buyer_owner: String,
    buyer_collateral_source: String,
    seller_collateral_destination: String,
    fee_token_account: String,
    fee_basis_points_per_side: u16,
    fee_recipient: String,
    mint: String,
    outcome_index: u32,
    outcome_count: u32,
    fill_atoms: u64,
    execution_price: u64,
    price_scale: u64,
    lookup_table: String,
    lookup_addresses_sha256: String,
    lookup_address_count: usize,
    static_account_count: usize,
    loaded_address_count: usize,
    unique_message_account_count: usize,
    wire_bytes: usize,
    capability_seal: String,
    capability_seal_sha256: String,
    hot_ack_producer: String,
    hot_ack_base64: String,
    hot_ack_sha256: String,
    mutations: Vec<DirectFinalizedMutationEvidenceV1>,
    lookup_activation: DirectLookupActivationEvidenceV1,
    positions: [DirectPositionTransitionEvidenceV1; 2],
    claim_balances: Vec<DirectClaimBalanceEvidenceV1>,
    final_accounts: Vec<DirectTradeExpectedPoststateV1>,
    poststates: Vec<DirectTradeObservedPoststateV1>,
    evidence_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDirectTradeEvidenceV1 {
    pub(crate) market: Pubkey,
    pub(crate) seller_owner: Pubkey,
    pub(crate) seller_position: Pubkey,
    pub(crate) seller_collateral_destination: Pubkey,
    pub(crate) buyer_owner: Pubkey,
    pub(crate) buyer_position: Pubkey,
    pub(crate) buyer_collateral_source: Pubkey,
    pub(crate) fee_recipient: Pubkey,
    pub(crate) fee_token_account: Pubkey,
    pub(crate) mint: Pubkey,
    pub(crate) outcome_index: u32,
    pub(crate) outcome_count: u32,
    pub(crate) mutations: Vec<DirectFinalizedMutationEvidenceV1>,
    pub(crate) positions: [DirectPositionTransitionEvidenceV1; 2],
    pub(crate) claim_balances: Vec<DirectClaimBalanceEvidenceV1>,
    pub(crate) final_accounts: Vec<DirectTradeExpectedPoststateV1>,
    pub(crate) finalized_slot: u64,
    pub(crate) evidence_sha256: String,
}

struct ArgumentsV1 {
    origin: ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
    session: PathBuf,
    execute: bool,
}

pub(crate) struct ValidatedManifestV1 {
    pub(crate) public: DirectTradePublicManifestV1,
    public_sha256: String,
    private: DirectTradePrivateSessionV1,
    private_sha256: String,
    pub(crate) plan: SuccessorPlan,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
}

/// Key-free projection of an exact public Direct manifest after the manifest,
/// its signed tickets, its plan/Market digests, and the permanent checked
/// deployment evidence have all been reauthenticated. The producer persists
/// this projection only in its write-ahead journal; the executor still owns
/// current chain authentication and never accepts it as account authority.
#[derive(Clone, Debug)]
pub(crate) struct AuthenticatedDevnetDirectSessionSourceV1 {
    pub(crate) public_manifest_sha256: String,
    pub(crate) plan_sha256: String,
    pub(crate) market_input_sha256: String,
    pub(crate) checked_execution_release_sha256: String,
    pub(crate) payer: Pubkey,
    pub(crate) market: Pubkey,
    pub(crate) seller: Pubkey,
    pub(crate) buyer: Pubkey,
    pub(crate) claims_market: Pubkey,
    pub(crate) seller_position: Pubkey,
    pub(crate) buyer_position: Pubkey,
    pub(crate) seller_collateral: Pubkey,
    pub(crate) buyer_collateral: Pubkey,
    pub(crate) custody_authority: Pubkey,
    pub(crate) mint: Pubkey,
    pub(crate) token_program: Pubkey,
    pub(crate) seller_replay: Pubkey,
    pub(crate) buyer_replay: Pubkey,
    pub(crate) seller_nonce: u64,
    pub(crate) buyer_nonce: u64,
    pub(crate) seller_ticket_sha256: String,
    pub(crate) buyer_ticket_sha256: String,
    pub(crate) checked_binaries: BTreeMap<String, ProgramPin>,
}

pub(crate) struct DirectTradePlanningV1 {
    pub(crate) route: DirectInlineAuthenticatedRouteV3,
    pub(crate) provision: DirectInlineLookupTableProvisionV3,
    seal: DirectInlineCapabilitySealPlanV3,
    hot: DirectInlineHotFinalizationPlanV3,
    hot_prestates: Vec<DirectTradeExpectedPoststateV1>,
    pub(crate) lookup_table: Option<ObservedAccount>,
}

struct DirectTradeSetupPlanningV1 {
    observation: Observation,
    replay: DirectReplaySetupPlanV1,
    tokens: DirectTradeTokenSetupPlanV1,
    replay_observed: ObservedAccount,
    seller_token_observed: ObservedAccount,
    fee_token_observed: ObservedAccount,
    payer_observed: ObservedAccount,
    rent_refund_observed: ObservedAccount,
    projected_hot: DirectTradePlanningV1,
}

#[derive(Default)]
struct DirectSetupJournalsV1 {
    replay: Option<DirectSetupJournalV1>,
    token: Option<DirectSetupJournalV1>,
}

struct NextActionV1 {
    stage: DirectTradeStageV1,
    action_index: usize,
    stage_name: &'static str,
}

pub(crate) fn run_devnet(arguments: Vec<String>) -> Result<()> {
    run_for_cluster_v1(arguments, ExpectedClusterV1::Devnet)
}

pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    run_for_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

fn run_for_cluster_v1(arguments: Vec<String>, expected_cluster: ExpectedClusterV1) -> Result<()> {
    let arguments = parse_arguments(arguments, expected_cluster)?;
    let validated = load_and_validate_manifests(&arguments.session, expected_cluster)?;
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    authenticate_manifest_cluster_v1(&mut rpc, &arguments, &validated)?;
    let evidence_path = direct_evidence_path_v1(&validated.private)?;
    if evidence_path.exists() {
        let evidence =
            authenticate_persisted_direct_evidence_v1(&mut rpc, &validated, &evidence_path)?;
        return stdout_json_v1(&evidence);
    }
    if let Some(evidence) = recover_finalized_hot_evidence_v1(&mut rpc, &validated, &evidence_path)?
    {
        return stdout_json_v1(&evidence);
    }
    let setup_journals = load_direct_setup_journals_v1(&validated)?;
    let setup = collect_direct_trade_setup_planning_v1(
        &mut rpc,
        &validated,
        setup_journals.replay.as_ref(),
        setup_journals.token.as_ref(),
    )?;
    let setup_complete = setup_journals
        .token
        .as_ref()
        .is_some_and(|journal| journal.phase == DirectSetupJournalPhaseV1::Finalized);
    if !setup_complete {
        if !journal_entries_v1(&validated)?.is_empty() {
            return Err(refusal(
                "Direct lookup/seal/Hot journal exists before replay and token setup finalized",
            ));
        }
        if arguments.execute {
            return execute_direct_setup_action_v1(
                &mut rpc,
                &arguments,
                &validated,
                &setup,
                &setup_journals,
            );
        }
        let next = if setup_journals
            .replay
            .as_ref()
            .is_some_and(|journal| journal.phase == DirectSetupJournalPhaseV1::Finalized)
        {
            "token-setup"
        } else {
            "replay-setup"
        };
        let output = serde_json::json!({
            "schema": "dclutch-direct-trade-setup-preflight-v1",
            "mutationPermitted": false,
            "publicManifestSha256": validated.public_sha256,
            "privateSessionSha256": validated.private_sha256,
            "observationSlot": setup.observation.slot,
            "nextAction": next,
            "projectedHotUniqueMessageAccounts": 61,
            "projectedHotLookupAddressCount": setup.projected_hot.provision.addresses.len(),
            "replayObservedLamports": setup.replay_observed.lamports,
            "sellerTokenObservedLamports": setup.seller_token_observed.lamports,
            "feeTokenObservedLamports": setup.fee_token_observed.lamports,
        });
        return stdout_json_v1(&output);
    }
    let replay_journal = setup_journals
        .replay
        .as_ref()
        .ok_or_else(|| refusal("Direct finalized token setup omitted replay journal"))?;
    let token_journal = setup_journals
        .token
        .as_ref()
        .ok_or_else(|| refusal("Direct finalized token setup journal disappeared"))?;
    authenticate_finalized_direct_setup_history_v1(&mut rpc, &setup, replay_journal)?;
    authenticate_finalized_direct_setup_history_v1(&mut rpc, &setup, token_journal)?;
    let journal_root = journal_root_v1(&validated)?;
    let planning = collect_direct_trade_planning_v1(&mut rpc, &validated, journal_root.as_ref())?;
    let next = next_action_v1(&validated, &planning)?;
    if arguments.execute {
        return execute_direct_action_v1(
            &mut rpc,
            &arguments,
            &validated,
            &planning,
            &next,
            &evidence_path,
        );
    }
    let output = serde_json::json!({
        "schema": "dclutch-devnet-direct-trade-preflight-v1",
        "mutationPermitted": false,
        "publicManifestSha256": validated.public_sha256,
        "privateSessionSha256": validated.private_sha256,
        "observationSlot": planning.route.physical.observation.slot,
        "lookupTable": planning.provision.lookup_table.to_string(),
        "lookupAddressCount": planning.provision.addresses.len(),
        "lookupAddressesSha256": pubkey_list_sha256(&planning.provision.addresses),
        "capabilitySeal": planning.seal.seal.to_string(),
        "capabilitySealExpectedBodySha256": sha256_hex(&planning.seal.expected_body),
        "hotWireInstructionCount": planning.hot.sealed_report.instructions.len(),
        "hotEconomicPreview": {
            "claimTransfer": planning.hot.sealed_report.preview.claim_transfer,
            "grossCollateral": planning.hot.sealed_report.preview.gross_collateral,
            "sellerNetCollateralCredit": planning.hot.sealed_report.preview.seller_net_collateral_credit,
            "buyerCollateralDebit": planning.hot.sealed_report.preview.buyer_collateral_debit,
            "totalFeeTransfer": planning.hot.sealed_report.preview.total_fee_transfer,
        },
        "expectedHotAckSha256": sha256_hex(&planning.hot.finalization.ack_bytes),
        "expectedHotPoststateCount": planning.hot.poststates.len(),
        "nextAction": next.stage_name,
        "hotSubmissionBlocked": false,
    });
    stdout_json_v1(&output)
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-direct-trade-v1 --rpc-url http://127.0.0.1:PORT/ --session ABSOLUTE_PRIVATE_JSON [--execute]\n\
     dclutch-local-successor-bootstrap devnet-direct-trade-v1 --rpc-url https://api.devnet.solana.com --i-mean-devnet DEVNET_GENESIS --session ABSOLUTE_PRIVATE_JSON [--execute]\n\
     \nThe public manifest freezes two already-signed, host-verified Direct intents, explicit fee facts, the checked release manifest, and untrusted named account hints. The private session contains only local paths and the payer keypair path. Preflight opens no key and uses a read-only RPC allowlist. Execute advances exactly one durable ALT, seal, or Hot action and never blind-resubmits an ambiguous packet."
}

fn parse_arguments(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut session = None;
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
            "--session" => &mut session,
            _ => {
                return Err(Error::new(format!(
                    "unknown Direct trade argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let rpc_url = rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?;
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    match expected_cluster {
        ExpectedClusterV1::Devnet if acknowledgment.is_none() => {
            return Err(Error::new(format!(
                "{DEVNET_ACKNOWLEDGMENT_FLAG} is required"
            )));
        }
        ExpectedClusterV1::OwnedLoopback if acknowledgment.is_some() => {
            return Err(refusal(
                "owned-loopback Direct command refuses the devnet acknowledgment flag",
            ));
        }
        _ => {}
    }
    expected_cluster.authenticate(&origin)?;
    let session = absolute_existing_file(
        &PathBuf::from(session.ok_or_else(|| Error::new("--session is required"))?),
        "Direct private session",
    )?;
    Ok(ArgumentsV1 {
        origin,
        expected_cluster,
        session,
        execute,
    })
}

pub(crate) fn load_and_validate_manifests(
    session_path: &Path,
    expected_cluster: ExpectedClusterV1,
) -> Result<ValidatedManifestV1> {
    let private_bytes = fs::read(session_path)?;
    require_unique_json_v1(&private_bytes, "Direct private session")?;
    let private: DirectTradePrivateSessionV1 = serde_json::from_slice(&private_bytes)
        .map_err(|error| Error::new(format!("Direct private session: {error}")))?;
    if private.schema != direct_private_schema_v1(expected_cluster) {
        return Err(refusal("Direct private session schema changed"));
    }
    let private_sha256 = session_state_sha256(&private)?;
    if private.session_sha256 != private_sha256 {
        return Err(refusal("Direct private session digest changed"));
    }
    let public_path = exact_path(&private.public_manifest, "Direct public manifest")?;
    let plan_path = exact_path(&private.plan, "Direct successor plan")?;
    let market_path = exact_path(&private.market_input, "Direct market input")?;
    let journal_dir = exact_directory(&private.journal_dir, "Direct journal directory")?;
    if !journal_dir.as_os_str().is_empty() {
        // The directory is authenticated above. It is intentionally not
        // created by preflight and never carries wallet material.
    }
    let public_bytes = fs::read(&public_path)?;
    require_unique_json_v1(&public_bytes, "Direct public manifest")?;
    let public_sha256 = sha256_hex(&public_bytes);
    if private.public_manifest_sha256 != public_sha256 {
        return Err(refusal("Direct public manifest bytes changed"));
    }
    let public: DirectTradePublicManifestV1 = serde_json::from_slice(&public_bytes)
        .map_err(|error| Error::new(format!("Direct public manifest: {error}")))?;
    if public.schema != direct_public_schema_v1(expected_cluster) {
        return Err(refusal("Direct public manifest schema changed"));
    }
    let plan_bytes = fs::read(&plan_path)?;
    let market_bytes = fs::read(&market_path)?;
    require_unique_json_v1(&plan_bytes, "Direct successor plan")?;
    require_unique_json_v1(&market_bytes, "Direct market input")?;
    if public.plan_sha256 != sha256_hex(&plan_bytes)
        || public.market_input_sha256 != sha256_hex(&market_bytes)
    {
        return Err(refusal(
            "Direct public manifest plan or market-input digest changed",
        ));
    }
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let seller = decode_signed_intent(&public.seller, 0, &public)?;
    let buyer = decode_signed_intent(&public.buyer, 1, &public)?;
    if seller.maker == buyer.maker {
        return Err(refusal("Direct seller and buyer identities are equal"));
    }
    validate_public_facts(&public, seller, buyer)?;
    Ok(ValidatedManifestV1 {
        public,
        public_sha256,
        private,
        private_sha256,
        plan,
        seller,
        buyer,
    })
}

/// Reauthenticate one immutable devnet public source entirely offline. This is
/// intentionally narrower than executor preflight: it proves source bytes and
/// checked binary provenance, while the executor remains responsible for one
/// current finalized RPC snapshot before any transaction can be prepared.
pub(crate) fn authenticate_devnet_direct_session_source_v1(
    public_bytes: &[u8],
    plan_bytes: &[u8],
    market_bytes: &[u8],
) -> Result<AuthenticatedDevnetDirectSessionSourceV1> {
    require_unique_json_v1(public_bytes, "Direct public manifest")?;
    require_unique_json_v1(plan_bytes, "Direct successor plan")?;
    require_unique_json_v1(market_bytes, "Direct Market input")?;
    let public: DirectTradePublicManifestV1 = serde_json::from_slice(public_bytes)
        .map_err(|error| Error::new(format!("Direct public manifest: {error}")))?;
    if public.schema != PUBLIC_MANIFEST_SCHEMA_V1
        || public.cluster != ExpectedClusterV1::Devnet.evidence_label()
        || public.genesis_hash != DEVNET_GENESIS_HASH
    {
        return Err(refusal(
            "Direct session producer source is not one exact public-devnet manifest",
        ));
    }
    let plan_sha256 = sha256_hex(plan_bytes);
    let market_input_sha256 = sha256_hex(market_bytes);
    if public.plan_sha256 != plan_sha256 || public.market_input_sha256 != market_input_sha256 {
        return Err(refusal(
            "Direct public manifest changed its checked plan or Market input bytes",
        ));
    }
    let plan: SuccessorPlan = serde_json::from_slice(plan_bytes)?;
    let market: MarketRunInput = serde_json::from_slice(market_bytes)?;
    crate::market::validate_market_input(&market)?;
    // The manifest's generation is the OPEN Market's PDA-authenticated
    // identity generation: either the input's own value (a compiled input
    // carrying the live Open generation) or the founding lane's Open
    // generation for a founding-shaped input, exactly as the producer admits.
    if public.context.generation != market.generation
        && public.context.generation != crate::market::open_market_generation_v1(&market)?
    {
        return Err(refusal(
            "Direct public manifest changed the authenticated Market generation",
        ));
    }
    let seller = decode_signed_intent(&public.seller, 0, &public)?;
    let buyer = decode_signed_intent(&public.buyer, 1, &public)?;
    if seller.maker == buyer.maker {
        return Err(refusal("Direct seller and buyer identities are equal"));
    }
    validate_public_facts(&public, seller, buyer)?;
    let set = plan.checked_upgrade_set.as_ref().ok_or_else(|| {
        refusal("devnet Direct plan omitted its authenticated permanent checked deployment set")
    })?;
    if plan.checked_local_mutable_set.is_some() || set.devnet_genesis_hash != DEVNET_GENESIS_HASH {
        return Err(refusal(
            "devnet Direct session producer refuses localhost or another genesis provenance",
        ));
    }
    crate::upgrade::reauthenticate_checked_deployment_set_pin(set)?;
    let checked_bytes = decode_canonical_base64_v1(
        &public.checked_execution_release_set_base64,
        "checked Direct execution release",
    )?;
    let checked = CheckedExecutionReleaseSetV1::decode(&checked_bytes)
        .map_err(|error| Error::new(format!("checked Direct execution release: {error:?}")))?;
    if checked
        .execution_release_set_id()
        .map_err(|error| Error::new(format!("checked Direct execution identity: {error:?}")))?
        .as_bytes()
        != &hex32(&plan.release_set_id, "Direct release set")?
    {
        return Err(refusal(
            "checked Direct execution release selected another release set",
        ));
    }
    let role_pins: [(&str, &ProgramPin); 5] = [
        ("core", &plan.core),
        ("claims", &plan.claims),
        ("trading", &plan.trading),
        ("resolution", &plan.resolution),
        ("custody", &plan.custody),
    ];
    let artifacts = checked.artifacts();
    let mut checked_binaries = BTreeMap::new();
    for ((role, pin), artifact) in role_pins.into_iter().zip(artifacts) {
        let program = pubkey(&pin.program_id)?;
        let programdata = pubkey(&pin.programdata_id)?;
        if artifact.program().to_bytes() != program.to_bytes()
            || artifact.programdata() != programdata.to_bytes()
            || artifact.elf_digest() != hex32(&pin.live_elf_sha256, "Direct live ELF digest")?
            || artifact.semantic_release_id().to_bytes()
                != hex32(&pin.semantic_release_id, "Direct semantic release")?
            || artifact.deployment_slot() != pin.deployment_slot
            || sha256_hex(&artifact.to_bytes()) != pin.artifact_release_id
        {
            return Err(refusal(format!(
                "checked Direct {role} artifact differs from the accepted program, binary, semantic release, or deployment slot",
            )));
        }
        if checked_binaries.insert(role.into(), pin.clone()).is_some() {
            return Err(refusal("checked Direct execution role appeared twice"));
        }
    }
    let ticket_sha256 = |value: &SignedIntentManifestV1| -> Result<String> {
        Ok(sha256_hex(&serde_json::to_vec(value)?))
    };
    Ok(AuthenticatedDevnetDirectSessionSourceV1 {
        public_manifest_sha256: sha256_hex(public_bytes),
        plan_sha256,
        market_input_sha256,
        checked_execution_release_sha256: sha256_hex(&checked_bytes),
        payer: parse_key(&public.payer, "Direct payer")?,
        market: parse_key(&public.market, "Direct Market")?,
        seller: seller.maker,
        buyer: buyer.maker,
        claims_market: parse_key(&public.route.claims.aggregate, "Direct Claims aggregate")?,
        seller_position: parse_key(
            &public.route.claims.seller_position,
            "Direct seller Position",
        )?,
        buyer_position: parse_key(&public.route.claims.buyer_position, "Direct buyer Position")?,
        seller_collateral: parse_key(
            &public.route.custody.seller_token,
            "Direct seller collateral",
        )?,
        buyer_collateral: parse_key(&public.route.custody.buyer_token, "Direct buyer collateral")?,
        custody_authority: parse_key(
            &public.route.custody.custody_authority,
            "Direct Custody authority",
        )?,
        mint: parse_key(&public.route.custody.mint, "Direct collateral Mint")?,
        token_program: parse_key(&public.route.custody.token_program, "Direct token program")?,
        seller_replay: parse_key(&public.route.seller_maker, "Direct seller replay")?,
        buyer_replay: parse_key(&public.route.buyer_maker, "Direct buyer replay")?,
        seller_nonce: public.context.seller_next_nonce,
        buyer_nonce: public.context.buyer_next_nonce,
        seller_ticket_sha256: ticket_sha256(&public.seller)?,
        buyer_ticket_sha256: ticket_sha256(&public.buyer)?,
        checked_binaries,
    })
}

fn decode_signed_intent(
    manifest: &SignedIntentManifestV1,
    side: u8,
    public: &DirectTradePublicManifestV1,
) -> Result<SignedDirectIntentV3> {
    let maker = parse_key(&manifest.maker, "Direct maker")?;
    let intent_bytes = BASE64
        .decode(&manifest.intent_base64)
        .map_err(|error| Error::new(format!("Direct intent base64: {error}")))?;
    let intent = CompactIntentV2::decode(&intent_bytes)
        .map_err(|error| Error::new(format!("Direct intent: {error:?}")))?;
    if intent
        .encode()
        .map_err(|error| Error::new(format!("Direct intent reencode: {error:?}")))?
        .as_slice()
        != intent_bytes
        || intent.side != side
        || intent.market != parse_key(&public.market, "Direct Market")?.to_bytes()
        || intent.generation != public.context.generation
        || intent.fee_basis_points != public.fee_basis_points
    {
        return Err(refusal("Direct signed intent semantic facts changed"));
    }
    let signature_bytes = BASE64
        .decode(&manifest.signature_base64)
        .map_err(|error| Error::new(format!("Direct signature base64: {error}")))?;
    let signature_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| Error::new("Direct signature is not exactly 64 bytes"))?;
    let signature = Signature::from(signature_array);
    let signed_preimage = intent
        .signed_preimage()
        .map_err(|error| Error::new(format!("Direct signed preimage: {error:?}")))?;
    if !signature.verify(maker.as_ref(), &signed_preimage) {
        return Err(refusal("Direct detached signature did not verify"));
    }
    Ok(SignedDirectIntentV3 {
        maker,
        signature: signature_array,
        intent,
    })
}

fn validate_public_facts(
    public: &DirectTradePublicManifestV1,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
) -> Result<()> {
    let market = parse_key(&public.market, "Direct Market")?;
    let payer = parse_key(&public.payer, "Direct payer")?;
    let fee_recipient = parse_key(&public.fee_recipient, "Direct fee recipient")?;
    if market == Pubkey::default()
        || payer == Pubkey::default()
        || fee_recipient == Pubkey::default()
        || public.route.fixed.market != public.market
        || public.route.payer != public.payer
        || public.fill == 0
        || public.execution_price == 0
        || public.fee_basis_points != 50
        || public.fill > seller.intent.maximum_fill
        || public.fill > buyer.intent.maximum_fill
        || seller.intent.lifecycle > 1
        || buyer.intent.lifecycle > 1
    {
        return Err(refusal("Direct public request facts are not canonical"));
    }
    compile_direct_inline_request_v3(seller, buyer, public.fill, public.execution_price)
        .map_err(|error| Error::new(format!("Direct request: {error:?}")))?;
    validate_setup_facts_v1(public, seller, buyer)?;
    Ok(())
}

fn validate_setup_facts_v1(
    public: &DirectTradePublicManifestV1,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
) -> Result<()> {
    let replay_bytes = BASE64
        .decode(&public.replay_setup.request_base64)
        .map_err(|error| Error::new(format!("Direct replay setup base64: {error}")))?;
    let replay_request = DirectReplaySetupRequestV1::decode(&replay_bytes)
        .map_err(|error| Error::new(format!("Direct replay setup: {error:?}")))?;
    if replay_request
        .to_bytes()
        .map_err(|error| Error::new(format!("Direct replay setup reencode: {error:?}")))?
        .as_slice()
        != replay_bytes
        || public.replay_setup.request_sha256 != sha256_hex(&replay_bytes)
        || replay_request.market != parse_key(&public.market, "Direct Market")?.to_bytes()
        || replay_request.maker != buyer.maker.to_bytes()
        || replay_request.generation != public.context.generation
        || public.replay_setup.maker != buyer.maker.to_string()
        || public.replay_setup.maker_root != public.route.buyer_maker
        || public.replay_setup.custody_replay != public.route.custody.replay
        || public.replay_setup.payer != public.payer
        || public.replay_setup.rent_refund != public.route.lifecycle_rent_credit
        || public.replay_setup.expected_initial_revision != 0
        || public.replay_setup.expected_resulting_revision != 1
    {
        return Err(refusal("Direct replay setup manifest changed"));
    }
    let token_request_bytes = decode_canonical_base64_v1(
        &public.token_setup.request_base64,
        "Direct token setup request",
    )?;
    let token_request = DirectTokenSetupRequestV1::decode(&token_request_bytes)
        .map_err(|error| Error::new(format!("Direct token setup: {error:?}")))?;
    if token_request
        .to_bytes()
        .map_err(|error| Error::new(format!("Direct token setup reencode: {error:?}")))?
        .as_slice()
        != token_request_bytes
        || public.token_setup.request_sha256 != sha256_hex(&token_request_bytes)
        || token_request.market != parse_key(&public.market, "Direct Market")?.to_bytes()
        || token_request.seller_owner != seller.maker.to_bytes()
        || token_request.generation != public.context.generation
    {
        return Err(refusal("Direct token setup request manifest changed"));
    }
    let payer = parse_key(&public.payer, "Direct setup payer")?;
    let mint = parse_key(&public.route.custody.mint, "Direct collateral Mint")?;
    let token_program = parse_key(&public.route.custody.token_program, "Direct token program")?;
    let trading = parse_key(
        &public.route.fixed.trading_program,
        "Direct Trading program",
    )?;
    let fee_recipient = parse_key(&public.fee_recipient, "Direct fee recipient")?;
    let seller_seeds = DirectTokenAccountSeedsV1::new(
        token_request.market,
        token_request.generation,
        seller.maker.to_bytes(),
        DirectTokenAccountRoleV1::Seller,
    )
    .map_err(|error| Error::new(format!("Direct seller token seeds: {error:?}")))?;
    let fee_seeds = DirectTokenAccountSeedsV1::new(
        token_request.market,
        token_request.generation,
        fee_recipient.to_bytes(),
        DirectTokenAccountRoleV1::Fee,
    )
    .map_err(|error| Error::new(format!("Direct fee token seeds: {error:?}")))?;
    let seller_token = Pubkey::find_program_address(&seller_seeds.as_slices(), &trading).0;
    let fee_token = Pubkey::find_program_address(&fee_seeds.as_slices(), &trading).0;
    if public.token_setup.seller_token != public.route.custody.seller_token
        || public.token_setup.seller_owner != seller.maker.to_string()
        || public.token_setup.fee_token != public.route.custody.fee_token
        || public.token_setup.fee_recipient != fee_recipient.to_string()
        || public.token_setup.payer != payer.to_string()
        || public.token_setup.rent_refund != public.route.lifecycle_rent_credit
        || public.token_setup.mint != mint.to_string()
        || public.token_setup.token_program != token_program.to_string()
        || public.token_setup.trading_program != trading.to_string()
        || parse_key(
            &public.token_setup.seller_token,
            "Direct seller setup token",
        )? != seller_token
        || parse_key(&public.token_setup.fee_token, "Direct fee setup token")? != fee_token
    {
        return Err(refusal("Direct token setup manifest changed"));
    }
    if seller.intent.collateral_account
        != parse_key(&public.route.custody.seller_token, "Direct seller token")?.to_bytes()
        || buyer.intent.collateral_account
            != parse_key(&public.route.custody.buyer_token, "Direct buyer token")?.to_bytes()
        || seller_token == fee_token
    {
        return Err(refusal(
            "Direct token and signed-intent coordinates changed",
        ));
    }
    Ok(())
}

fn collect_direct_trade_setup_planning_v1(
    rpc: &mut Rpc,
    validated: &ValidatedManifestV1,
    replay_journal: Option<&DirectSetupJournalV1>,
    token_journal: Option<&DirectSetupJournalV1>,
) -> Result<DirectTradeSetupPlanningV1> {
    let public = &validated.public;
    let mut keys = route_keys(&public.route)?;
    for key in [system_program::ID, sysvar::rent::ID] {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys.sort_unstable();
    keys.dedup();
    if keys.len() > 100 {
        return Err(refusal(format!(
            "Direct setup projection requires {} unique observation keys; bounded finalized collection admits at most 100",
            keys.len()
        )));
    }
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let market = parse_key(&public.market, "Direct setup Market")?;
    let activation_cache = parse_key(
        &public.route.fixed.activation_cache,
        "Direct setup activation cache",
    )?;
    let registry = parse_key(
        &public.route.fixed.registry_program,
        "Direct setup Registry program",
    )?;
    let trading = parse_key(
        &public.route.fixed.trading_program,
        "Direct setup Trading program",
    )?;
    let trading_programdata = parse_key(
        &public.route.fixed.trading_programdata,
        "Direct setup Trading ProgramData",
    )?;
    let core_program = parse_key(
        &public.route.fixed.core_program,
        "Direct setup Core program",
    )?;
    let claims_program = parse_key(
        &public.route.claims.claims_program,
        "Direct setup Claims program",
    )?;
    let claims_programdata = parse_key(
        &public.route.claims.claims_programdata,
        "Direct setup Claims ProgramData",
    )?;
    let direct_root = parse_key(&public.route.fixed.root, "Direct setup root")?;
    let realm_raw = parse_key(&public.route.custody.realm.raw, "Direct setup Realm")?;
    let realm_staging = parse_key(
        &public.route.custody.realm.staging,
        "Direct setup Realm staging",
    )?;
    let config_raw = parse_key(&public.route.fixed.config.raw, "Direct setup config")?;
    let config_staging = parse_key(
        &public.route.fixed.config.staging,
        "Direct setup config staging",
    )?;
    let claims_aggregate = parse_key(
        &public.route.claims.aggregate,
        "Direct setup Claims aggregate",
    )?;
    let seller_position = parse_key(
        &public.route.claims.seller_position,
        "Direct setup seller Position",
    )?;
    let custody = parse_key(
        &public.route.custody.custody_program,
        "Direct setup Custody program",
    )?;
    let payer = parse_key(&public.payer, "Direct setup payer")?;
    let rent_refund = parse_key(
        &public.route.lifecycle_rent_credit,
        "Direct setup RentCredit",
    )?;
    let buyer_maker_root = parse_key(&public.route.buyer_maker, "Direct buyer maker root")?;
    let replay = parse_key(&public.replay_setup.custody_replay, "Direct setup replay")?;
    let seller_token = parse_key(
        &public.route.custody.seller_token,
        "Direct seller token destination",
    )?;
    let fee_token = parse_key(
        &public.route.custody.fee_token,
        "Direct fee token destination",
    )?;
    let mint = parse_key(&public.route.custody.mint, "Direct setup collateral Mint")?;
    let token_program = parse_key(
        &public.route.custody.token_program,
        "Direct setup Token program",
    )?;
    let request_bytes = BASE64
        .decode(&public.replay_setup.request_base64)
        .map_err(|error| Error::new(format!("Direct replay setup base64: {error}")))?;
    let market_account = snapshot.account(market)?;
    let root_account = snapshot.account(direct_root)?;
    let realm_account = snapshot.account(realm_raw)?;
    let config_account = snapshot.account(config_raw)?;
    let claims_aggregate_account = snapshot.account(claims_aggregate)?;
    let seller_position_account = snapshot.account(seller_position)?;
    let mint_account = snapshot.account(mint)?;
    let replay_observed = snapshot.account(replay)?.clone();
    let seller_token_observed = snapshot.account(seller_token)?.clone();
    let fee_token_observed = snapshot.account(fee_token)?.clone();
    let payer_observed = snapshot.account(payer)?.clone();
    let rent_refund_observed = snapshot.account(rent_refund)?.clone();
    let rent_account = snapshot.account(sysvar::rent::ID)?;
    let rent: Rent = bincode::deserialize(&rent_account.data)
        .map_err(|error| Error::new(format!("Direct setup Rent sysvar: {error}")))?;

    let derived = derive_direct_replay_setup_accounts_v1(DirectReplaySetupDerivationInputV1 {
        request_bytes: &request_bytes,
        market_bytes: &market_account.data,
        realm_bytes: &realm_account.data,
        buyer_maker: validated.buyer.maker,
        buyer_maker_root,
        trading_program: trading,
        custody_program: custody,
        payer,
        rent_refund,
        rent: rent.clone(),
    })?;
    if derived.custody_replay != replay {
        return Err(refusal(
            "Direct replay manifest differed from the semantic owner's derived PDA",
        ));
    }
    let planned_replay_lamports = match replay_journal {
        Some(journal) => {
            let receipt_bytes = journal
                .expected_return_data
                .as_ref()
                .ok_or_else(|| refusal("Direct replay journal omitted its planned receipt"))?
                .body()?;
            DirectReplaySetupReceiptV1::decode(&receipt_bytes)
                .map_err(|error| Error::new(format!("Direct replay planned receipt: {error:?}")))?
                .observed_lamports
        }
        None => replay_observed.lamports,
    };
    let replay_plan = build_direct_replay_setup_v1(DirectReplaySetupBuildInputV1 {
        request_bytes: &request_bytes,
        market_bytes: &market_account.data,
        realm_bytes: &realm_account.data,
        buyer_maker: validated.buyer.maker,
        buyer_maker_root,
        coordinates: DirectReplaySetupCoordinatesV1 {
            caller_authority: derived.caller_authority,
            market,
            activation_cache,
            registry_program: registry,
            trading_program: trading,
            trading_programdata,
            realm_raw,
            realm_staging,
            custody_replay: replay,
            payer,
            system_program: system_program::ID,
            rent_sysvar: sysvar::rent::ID,
            rent_refund,
            custody_program: custody,
        },
        rent: rent.clone(),
        observed_replay_lamports: planned_replay_lamports,
    })?;
    // Every prestate this build reads is consumed by the setup, so on resume a
    // live read would re-derive a different plan than the one already signed:
    // the two token PDAs hold exact rent once it lands, and the payer and rent
    // refund have moved by the transaction's own deltas. The journal is
    // therefore the authority on resume - its receipt for the token balances,
    // its recorded rows, wound back through the receipt's deltas, for the two
    // wallets - and the reconstruction reproduces the original plan exactly.
    let (
        planned_seller_lamports,
        planned_fee_lamports,
        planned_payer_lamports,
        planned_refund_lamports,
    ) = match token_journal {
        Some(journal) => {
            let returned = journal
                .expected_return_data
                .as_ref()
                .ok_or_else(|| refusal("Direct token journal omitted its planned receipt"))?;
            let receipt = DirectTokenSetupReceiptV1::decode(&returned.body()?)
                .map_err(|error| Error::new(format!("Direct token planned receipt: {error:?}")))?;
            let projected_lamports = |address: Pubkey| -> Result<u64> {
                journal
                    .expected_poststates
                    .iter()
                    .find(|state| state.address == address.to_string())
                    .map(|state| state.lamports)
                    .ok_or_else(|| refusal("Direct token journal omitted planned account"))
            };
            let top_up = receipt
                .seller_normalization
                .payer_top_up
                .checked_add(receipt.fee_normalization.payer_top_up)
                .ok_or_else(|| refusal("Direct token planned payer top-up overflowed"))?;
            let refunded = receipt
                .seller_normalization
                .refunded_excess
                .checked_add(receipt.fee_normalization.refunded_excess)
                .ok_or_else(|| refusal("Direct token planned refund overflowed"))?;
            (
                receipt.seller_normalization.observed_lamports,
                receipt.fee_normalization.observed_lamports,
                projected_lamports(payer)?
                    .checked_add(top_up)
                    .ok_or_else(|| refusal("Direct token planned payer balance overflowed"))?,
                projected_lamports(rent_refund)?
                    .checked_sub(refunded)
                    .ok_or_else(|| refusal("Direct token planned refund balance underflowed"))?,
            )
        }
        None => (
            seller_token_observed.lamports,
            fee_token_observed.lamports,
            payer_observed.lamports,
            rent_refund_observed.lamports,
        ),
    };
    let token_plan = build_direct_trade_token_setup_v1(DirectTradeTokenSetupBuildInputV1 {
        market_bytes: &market_account.data,
        root_bytes: &root_account.data,
        realm_bytes: &realm_account.data,
        config_bytes: &config_account.data,
        claims_aggregate_bytes: &claims_aggregate_account.data,
        seller_position_bytes: &seller_position_account.data,
        collateral_mint_bytes: &mint_account.data,
        generation: public.context.generation,
        seller_owner: validated.seller.maker,
        coordinates: DirectTradeTokenSetupCoordinatesV1 {
            market,
            core_program,
            registry_program: registry,
            activation_cache,
            trading_program: trading,
            trading_programdata,
            claims_program,
            claims_programdata,
            direct_root,
            realm_raw,
            realm_staging,
            config_raw,
            config_staging,
            claims_aggregate,
            seller_position,
            collateral_mint: mint,
            seller_token,
            fee_token,
            payer,
            rent_refund,
            rent_sysvar: sysvar::rent::ID,
            system_program: system_program::ID,
            token_program,
        },
        rent,
        observed_seller_lamports: planned_seller_lamports,
        observed_fee_lamports: planned_fee_lamports,
        observed_payer_lamports: planned_payer_lamports,
        observed_refund_lamports: planned_refund_lamports,
    })?;
    let manifest_request = decode_canonical_base64_v1(
        &public.token_setup.request_base64,
        "Direct token setup request",
    )?;
    if token_plan.request_bytes.as_slice() != manifest_request
        || public.token_setup.request_sha256 != sha256_hex(&manifest_request)
    {
        return Err(refusal(
            "Direct token setup manifest differed from the authenticated prestate builder",
        ));
    }

    match replay_journal.map(|journal| journal.phase) {
        Some(DirectSetupJournalPhaseV1::Finalized) => {
            let returned = replay_journal
                .and_then(|journal| journal.expected_return_data.as_ref())
                .ok_or_else(|| refusal("Direct finalized replay journal omitted receipt"))?;
            verify_direct_replay_setup_poststate_v1(
                &replay_plan,
                parse_key(&returned.producer, "Direct replay receipt producer")?,
                &returned.body()?,
                setup_observed_account_v1(&replay_observed),
            )?;
        }
        Some(DirectSetupJournalPhaseV1::Prepared)
        | Some(DirectSetupJournalPhaseV1::Dispatching)
        | Some(DirectSetupJournalPhaseV1::Submitted) => {
            if !((replay_observed.owner == system_program::ID && replay_observed.data.is_empty())
                || (replay_observed.owner == custody
                    && replay_observed.lamports == replay_plan.exact_replay_rent))
            {
                return Err(refusal(
                    "Direct ambiguous replay setup no longer had either exact prestate or poststate envelope",
                ));
            }
        }
        Some(DirectSetupJournalPhaseV1::Planned) | None => {
            if replay_observed.owner != system_program::ID
                || !replay_observed.data.is_empty()
                || replay_observed.lamports != planned_replay_lamports
            {
                return Err(refusal(
                    "Direct planned replay setup prestate changed before key access",
                ));
            }
        }
    }
    match token_journal.map(|journal| journal.phase) {
        Some(DirectSetupJournalPhaseV1::Finalized) => {
            let journal = token_journal
                .ok_or_else(|| refusal("Direct finalized token setup journal disappeared"))?;
            let returned = journal
                .expected_return_data
                .as_ref()
                .ok_or_else(|| refusal("Direct finalized token setup omitted receipt"))?;
            let returned_bytes = returned.body()?;
            verify_direct_trade_token_setup_poststate_v1(
                &token_plan,
                DirectTradeTokenSetupPoststateV1 {
                    return_program: parse_key(&returned.producer, "Direct token receipt producer")?,
                    return_data: &returned_bytes,
                    seller_token: direct_token_setup_observed_v1(&seller_token_observed),
                    fee_token: direct_token_setup_observed_v1(&fee_token_observed),
                    market_bytes: &market_account.data,
                    root_bytes: &root_account.data,
                    realm_bytes: &realm_account.data,
                    config_bytes: &config_account.data,
                    claims_aggregate_bytes: &claims_aggregate_account.data,
                    seller_position_bytes: &seller_position_account.data,
                    collateral_mint_bytes: &mint_account.data,
                },
            )?;
        }
        Some(DirectSetupJournalPhaseV1::Prepared)
        | Some(DirectSetupJournalPhaseV1::Dispatching)
        | Some(DirectSetupJournalPhaseV1::Submitted) => {
            let vacant = |account: &ObservedAccount| {
                account.owner == system_program::ID && account.data.is_empty()
            };
            let live = |account: &ObservedAccount| {
                account.owner == token_program && account.lamports == token_plan.exact_account_rent
            };
            if !((vacant(&seller_token_observed) && vacant(&fee_token_observed))
                || (live(&seller_token_observed) && live(&fee_token_observed)))
            {
                return Err(refusal(
                    "Direct ambiguous token setup no longer had exact paired prestate or poststate envelopes",
                ));
            }
        }
        Some(DirectSetupJournalPhaseV1::Planned) | None => {
            if seller_token_observed.owner != system_program::ID
                || !seller_token_observed.data.is_empty()
                || seller_token_observed.lamports
                    != token_plan.seller_normalization.observed_lamports
                || fee_token_observed.owner != system_program::ID
                || !fee_token_observed.data.is_empty()
                || fee_token_observed.lamports != token_plan.fee_normalization.observed_lamports
            {
                return Err(refusal(
                    "Direct planned token setup System PDA prestates changed",
                ));
            }
        }
    }

    // Before any key access, authenticate the complete eventual Hot route over
    // the exact poststates these two setup plans commit. No setup manifest hint
    // can thereby select a foreign fee recipient, mint, Token program, Realm,
    // or release and merely hope the later Hot path catches it.
    let expected_replay = project_direct_replay_setup_poststate_v1(&replay_plan)?;
    let mut projected = FinalizedSnapshotV1 {
        observation: snapshot.observation,
        accounts: snapshot.accounts.clone(),
    };
    projected.accounts.insert(
        replay,
        ObservedAccount {
            observation: snapshot.observation,
            key: replay,
            owner: custody,
            lamports: replay_plan.exact_replay_rent,
            executable: false,
            data: expected_replay.replay_bytes.to_vec(),
        },
    );
    for (address, bytes) in [
        (seller_token, token_plan.expected_seller_bytes.as_slice()),
        (fee_token, token_plan.expected_fee_bytes.as_slice()),
    ] {
        projected.accounts.insert(
            address,
            ObservedAccount {
                observation: snapshot.observation,
                key: address,
                owner: token_program,
                lamports: token_plan.exact_account_rent,
                executable: false,
                data: bytes.to_vec(),
            },
        );
    }
    let projected_hot =
        collect_direct_trade_planning_from_snapshot_v1(validated, None, &projected)?;
    Ok(DirectTradeSetupPlanningV1 {
        observation: snapshot.observation,
        replay: replay_plan,
        tokens: token_plan,
        replay_observed,
        seller_token_observed,
        fee_token_observed,
        payer_observed,
        rent_refund_observed,
        projected_hot,
    })
}

fn setup_observed_account_v1(account: &ObservedAccount) -> DirectSetupObservedAccountV1<'_> {
    DirectSetupObservedAccountV1 {
        address: account.key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: &account.data,
    }
}

fn direct_token_setup_observed_v1(
    account: &ObservedAccount,
) -> DirectTradeTokenSetupObservedAccountV1<'_> {
    DirectTradeTokenSetupObservedAccountV1 {
        address: account.key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: &account.data,
    }
}

pub(crate) fn collect_direct_trade_planning_v1(
    rpc: &mut Rpc,
    validated: &ValidatedManifestV1,
    journal_root: Option<&DirectTradeJournalV1>,
) -> Result<DirectTradePlanningV1> {
    let mut keys = route_keys(&validated.public.route)?;
    if let Some(root) = journal_root {
        keys.push(parse_key(
            &root.lookup_table,
            "Direct journal lookup table",
        )?);
        keys.sort_unstable();
        keys.dedup();
    }
    if keys.len() > 100 {
        return Err(refusal(format!(
            "Direct named route requires {} unique observation keys; bounded finalized collection admits at most 100",
            keys.len()
        )));
    }
    let snapshot = finalized_snapshot(rpc, &keys)?;
    collect_direct_trade_planning_from_snapshot_v1(validated, journal_root, &snapshot)
}

fn collect_direct_trade_planning_from_snapshot_v1(
    validated: &ValidatedManifestV1,
    journal_root: Option<&DirectTradeJournalV1>,
    snapshot: &FinalizedSnapshotV1,
) -> Result<DirectTradePlanningV1> {
    let named_route = build_named_route(&snapshot, &validated.public.route)?;
    let context = build_context(validated, &named_route, snapshot.observation)?;
    let checked_execution_release_set = BASE64
        .decode(&validated.public.checked_execution_release_set_base64)
        .map_err(|error| Error::new(format!("checked Direct release base64: {error}")))?
        .try_into()
        .map_err(|_| Error::new("checked Direct release manifest has the wrong width"))?;
    let programs = DirectInlineCheckedProgramAccountsV3 {
        core_program: pubkey(&validated.plan.core.program_id)?,
        core_programdata: pubkey(&validated.plan.core.programdata_id)?,
        trading_program: pubkey(&validated.plan.trading.program_id)?,
        trading_programdata: pubkey(&validated.plan.trading.programdata_id)?,
        checked_execution_release_set,
        registry_program: pubkey(&validated.plan.registry.program_id)?,
        claims_program: pubkey(&validated.plan.claims.program_id)?,
        claims_programdata: pubkey(&validated.plan.claims.programdata_id)?,
        custody_program: pubkey(&validated.plan.custody.program_id)?,
        rent_program: pubkey(&validated.plan.rent_credit.program_id)?,
        token_program: named_route.custody.token_program.key,
    };
    let authentication = DirectInlineRouteAuthenticationV3 {
        seller: validated.seller,
        buyer: validated.buyer,
        fill: validated.public.fill,
        execution_price: validated.public.execution_price,
        context,
        programs,
    };
    let route = assemble_authenticated_direct_inline_ordinary_route_v3(
        named_route.clone(),
        validated.public.context.outcome_count,
        authentication,
    )
    .map_err(|error| Error::new(format!("authenticated Direct named route: {error:?}")))?;
    let config = context.config;
    if config.fee_basis_points() != validated.public.fee_basis_points
        || config.fee_recipient()
            != parse_key(&validated.public.fee_recipient, "Direct fee recipient")?.to_bytes()
        || validated.seller.intent.fee_basis_points != config.fee_basis_points()
        || validated.buyer.intent.fee_basis_points != config.fee_basis_points()
    {
        return Err(refusal(
            "Direct fee facts differ from the finalized selected config or signed intents",
        ));
    }
    let provision = match journal_root {
        Some(root) => rederive_direct_inline_lookup_table_provision_v3(
            &route,
            route.payer,
            root.lookup_creation_slot,
        ),
        None => build_direct_inline_lookup_table_provision_v3(
            &route,
            route.payer,
            route.physical.observation.slot,
        ),
    }
    .map_err(|error| Error::new(format!("Direct lookup-table provision: {error:?}")))?;
    let seal = build_direct_inline_capability_seal_v3(
        named_route.clone(),
        validated.public.context.outcome_count,
        authentication,
    )
    .map_err(|error| Error::new(format!("Direct capability seal: {error:?}")))?;
    if seal.lookup_addresses != provision.addresses
        || seal.payer != provision.authority
        || seal.seal
            != parse_key(
                &validated.public.route.fixed.capability_seal,
                "Direct capability seal",
            )?
    {
        return Err(refusal(
            "Direct seal and request-specific lookup-table closures differ",
        ));
    }
    let hot_state = DirectInlineHotStateV3 {
        fixed_accounts: route.physical.fixed_accounts.clone(),
        strategy_accounts: route.physical.strategy_accounts.clone(),
        runtime_accounts: route.physical.runtime_accounts.clone(),
        release_set: context.release_set,
        generation: context.generation,
        clock_slot: context.slot,
        minimum_finalized_slot: route.physical.observation.slot,
        hot_outer: Some(CheckedHotOuterReleaseV3 {
            trading_program: programs.trading_program,
            artifact_release: route.chain.trading_artifact_release,
            checked_manifest_digest: route.checked_manifest_digest,
        }),
    };
    let hot_report = build_direct_inline_hot_v4(
        &hot_state,
        validated.seller,
        validated.buyer,
        validated.public.fill,
        validated.public.execution_price,
    )
    .map_err(|error| Error::new(format!("Direct Hot report: {error:?}")))?;
    if hot_report.trading_artifact_release != route.chain.trading_artifact_release
        || hot_report.product_record != route.chain.product_record
        || hot_report.outcome_count != route.chain.outcome_count
    {
        return Err(refusal(
            "Direct Hot report differs from the authenticated named-route chain",
        ));
    }
    let hot = prepare_direct_inline_hot_finalization_v3(
        named_route.clone(),
        validated.public.context.outcome_count,
        authentication,
        &hot_report,
    )
    .map_err(|error| Error::new(format!("Direct Hot finalization: {error:?}")))?;
    let hot_prestates = direct_hot_prestates_v1(&named_route);
    let lookup_table = journal_root
        .map(|_| snapshot.account(provision.lookup_table).cloned())
        .transpose()?;
    Ok(DirectTradePlanningV1 {
        route,
        provision,
        seal,
        hot,
        hot_prestates,
        lookup_table,
    })
}

fn direct_hot_prestates_v1(
    route: &DirectInlineOrdinaryRouteV3,
) -> Vec<DirectTradeExpectedPoststateV1> {
    [
        &route.fixed.root,
        &route.seller_maker,
        &route.buyer_maker,
        &route.claims.aggregate,
        &route.claims.seller_position,
        &route.claims.buyer_position,
        &route.custody.replay,
        &route.custody.buyer_token,
        &route.custody.seller_token,
        &route.custody.fee_token,
    ]
    .into_iter()
    .map(|account| DirectTradeExpectedPoststateV1 {
        address: account.key.to_string(),
        owner: account.owner.to_string(),
        lamports: account.lamports,
        executable: account.executable,
        data_base64: BASE64.encode(&account.data),
        data_sha256: sha256_hex(&account.data),
    })
    .collect()
}

fn build_context(
    validated: &ValidatedManifestV1,
    route: &DirectInlineOrdinaryRouteV3,
    observation: Observation,
) -> Result<DirectOrdinaryAuthenticatedContextV3> {
    if observation.finality != Finality::Finalized || observation.slot == 0 {
        return Err(refusal("Direct route observation is not finalized"));
    }
    let hints = &validated.public.context;
    let request = compile_direct_inline_request_v3(
        validated.seller,
        validated.buyer,
        validated.public.fill,
        validated.public.execution_price,
    )
    .map_err(|error| Error::new(format!("Direct request context: {error:?}")))?;
    let config_content_id = hash(&route.fixed.config.raw.data).to_bytes();
    let config = DirectExecutionConfigV1::decode_selected(
        config_content_id,
        config_content_id,
        &route.fixed.config.raw.data,
    )
    .map_err(|error| Error::new(format!("Direct finalized config: {error:?}")))?;
    Ok(DirectOrdinaryAuthenticatedContextV3 {
        parent_request_digest: hash(&request).to_bytes(),
        config_content_id,
        config,
        market: route.fixed.market.key.to_bytes(),
        generation: hints.generation,
        outcome_count: hints.outcome_count,
        slot: observation.slot,
        root_phase: hints.root_phase,
        seller_next_nonce: hints.seller_next_nonce,
        buyer_next_nonce: hints.buyer_next_nonce,
        root_open_maker_count: hints.root_open_maker_count,
        seller_created: hints.seller_created,
        seller_bump_observation: hints.seller_bump_observation,
        seller_bump: hints.seller_bump,
        seller_rent_principal_observation: hints.seller_rent_principal_observation,
        seller_rent_principal: hints.seller_rent_principal,
        buyer_created: hints.buyer_created,
        buyer_bump_observation: hints.buyer_bump_observation,
        buyer_bump: hints.buyer_bump,
        buyer_rent_principal_observation: hints.buyer_rent_principal_observation,
        buyer_rent_principal: hints.buyer_rent_principal,
        claims_market_revision: hints.claims_market_revision,
        seller_position_revision: hints.seller_position_revision,
        buyer_position_revision: hints.buyer_position_revision,
        custody_revision: hints.custody_revision,
        release_set: hex32(&hints.release_set, "Direct release set")?,
        product_record_digest: hash(&route.fixed.product.raw.data).to_bytes(),
        semantic_basis: hex32(&hints.semantic_basis, "Direct semantic basis")?,
        linked_basis_record_digest: hash(&route.fixed.linked_basis.raw.data).to_bytes(),
        trading_program: route.fixed.trading_program.key.to_bytes(),
        realm: hash(&route.custody.realm.raw.data).to_bytes(),
        mint: route.custody.mint.key.to_bytes(),
        token_program: route.custody.token_program.key.to_bytes(),
        seller_maker_root: route.seller_maker.key.to_bytes(),
        buyer_maker_root: route.buyer_maker.key.to_bytes(),
        system_program: system_program::ID.to_bytes(),
        custody_authority: route.custody.custody_authority.key.to_bytes(),
        seller_rent_beneficiary: parse_key(
            &hints.seller_rent_beneficiary,
            "seller rent beneficiary",
        )?
        .to_bytes(),
        seller_rent_beneficiary_observation: parse_optional_key(
            &hints.seller_rent_beneficiary_observation,
            "seller rent beneficiary observation",
        )?,
        buyer_rent_beneficiary: parse_key(&hints.buyer_rent_beneficiary, "buyer rent beneficiary")?
            .to_bytes(),
        buyer_rent_beneficiary_observation: parse_optional_key(
            &hints.buyer_rent_beneficiary_observation,
            "buyer rent beneficiary observation",
        )?,
        fee_token_account: route.custody.fee_token.key.to_bytes(),
        seller_token_account: route.custody.seller_token.key.to_bytes(),
        buyer_token_account: route.custody.buyer_token.key.to_bytes(),
        seller_native_signer: validated.seller.maker.to_bytes(),
        buyer_native_signer: validated.buyer.maker.to_bytes(),
    })
}

fn route_keys(coordinates: &DirectRouteCoordinatesV1) -> Result<Vec<Pubkey>> {
    let mut keys = Vec::new();
    push_route_key_v1(&mut keys, &coordinates.fixed.market, "Direct Market")?;
    push_route_key_v1(&mut keys, &coordinates.fixed.root, "Direct root")?;
    for (value, label) in [
        (&coordinates.fixed.manifest, "Direct manifest"),
        (&coordinates.fixed.program_set, "Direct ProgramSet"),
        (&coordinates.fixed.descriptor, "Direct descriptor"),
        (&coordinates.fixed.config, "Direct config"),
        (&coordinates.fixed.account_profile, "Direct AccountProfile"),
        (&coordinates.fixed.request_profile, "Direct RequestProfile"),
        (&coordinates.fixed.transition, "Direct Transition"),
        (&coordinates.fixed.effect, "Direct Effect"),
        (&coordinates.fixed.lifecycle, "Direct lifecycle"),
        (&coordinates.fixed.strategy, "Direct strategy"),
        (&coordinates.fixed.product, "Direct Product"),
        (&coordinates.fixed.result_domain, "Direct ResultDomain"),
        (&coordinates.fixed.portfolio, "Direct Portfolio"),
        (&coordinates.fixed.linked_basis, "Direct linked basis"),
        (&coordinates.custody.realm, "Direct Realm"),
    ] {
        push_route_pair_v1(&mut keys, value, label)?;
    }
    for (value, label) in [
        (
            &coordinates.fixed.activation_cache,
            "Direct activation cache",
        ),
        (&coordinates.fixed.core_program, "Core program"),
        (&coordinates.fixed.core_programdata, "Core ProgramData"),
        (&coordinates.fixed.trading_program, "Trading program"),
        (
            &coordinates.fixed.trading_programdata,
            "Trading ProgramData",
        ),
        (&coordinates.fixed.registry_program, "Registry program"),
        (&coordinates.fixed.capability_seal, "Direct capability seal"),
        (&coordinates.seller_maker, "seller maker replay"),
        (&coordinates.payer, "Direct payer"),
        (
            &coordinates.lifecycle_rent_credit,
            "Direct lifecycle RentCredit",
        ),
        (&coordinates.buyer_maker, "buyer maker replay"),
        (&coordinates.rent_program, "Rent program"),
        (
            &coordinates.claims.caller_authority,
            "Claims caller authority",
        ),
        (&coordinates.claims.aggregate, "Claims aggregate"),
        (&coordinates.claims.claims_program, "Claims program"),
        (&coordinates.claims.claims_programdata, "Claims ProgramData"),
        (
            &coordinates.claims.seller_position,
            "seller Claims Position",
        ),
        (&coordinates.claims.buyer_position, "buyer Claims Position"),
        (&coordinates.custody.replay, "Custody replay"),
        (&coordinates.custody.mint, "Realm collateral mint"),
        (&coordinates.custody.buyer_token, "buyer token"),
        (&coordinates.custody.seller_token, "seller token"),
        (&coordinates.custody.fee_token, "fee token"),
        (&coordinates.custody.custody_authority, "Custody authority"),
        (&coordinates.custody.token_program, "Realm token program"),
        (&coordinates.custody.custody_program, "Custody program"),
        (
            &coordinates.custody.custody_programdata,
            "Custody ProgramData",
        ),
    ] {
        push_route_key_v1(&mut keys, value, label)?;
    }
    for (index, value) in coordinates.custody.caller_authorities.iter().enumerate() {
        push_route_key_v1(
            &mut keys,
            value,
            &format!("Custody caller authority {index}"),
        )?;
    }
    keys.extend([
        sysvar::rent::ID,
        sysvar::instructions::ID,
        system_program::ID,
    ]);
    keys.sort_unstable();
    keys.dedup();
    Ok(keys)
}

fn push_route_key_v1(keys: &mut Vec<Pubkey>, value: &str, label: &str) -> Result<()> {
    keys.push(parse_key(value, label)?);
    Ok(())
}

fn push_route_pair_v1(
    keys: &mut Vec<Pubkey>,
    value: &RecordPairCoordinatesV1,
    label: &str,
) -> Result<()> {
    push_route_key_v1(keys, &value.raw, &format!("{label} raw"))?;
    push_route_key_v1(keys, &value.staging, &format!("{label} staging"))
}

fn build_named_route(
    snapshot: &FinalizedSnapshotV1,
    coordinates: &DirectRouteCoordinatesV1,
) -> Result<DirectInlineOrdinaryRouteV3> {
    let account = |value: &str, label: &str| -> Result<ObservedAccount> {
        observed(snapshot, parse_key(value, label)?)
    };
    let pair = |value: &RecordPairCoordinatesV1, label: &str| -> Result<FinalizedRecordRouteV3> {
        Ok(FinalizedRecordRouteV3 {
            raw: account(&value.raw, &format!("{label} raw"))?,
            staging: account(&value.staging, &format!("{label} staging"))?,
        })
    };
    let fixed = &coordinates.fixed;
    let claims = &coordinates.claims;
    let custody = &coordinates.custody;
    let [
        custody_authority_0,
        custody_authority_1,
        custody_authority_2,
        custody_authority_3,
    ] = &custody.caller_authorities;
    Ok(DirectInlineOrdinaryRouteV3 {
        fixed: DirectHotFixedRouteV3 {
            market: account(&fixed.market, "Direct Market")?,
            root: account(&fixed.root, "Direct root")?,
            manifest: pair(&fixed.manifest, "Direct manifest")?,
            program_set: pair(&fixed.program_set, "Direct ProgramSet")?,
            descriptor: pair(&fixed.descriptor, "Direct descriptor")?,
            config: pair(&fixed.config, "Direct config")?,
            account_profile: pair(&fixed.account_profile, "Direct AccountProfile")?,
            request_profile: pair(&fixed.request_profile, "Direct RequestProfile")?,
            transition: pair(&fixed.transition, "Direct Transition")?,
            effect: pair(&fixed.effect, "Direct Effect")?,
            lifecycle: pair(&fixed.lifecycle, "Direct lifecycle")?,
            strategy: pair(&fixed.strategy, "Direct strategy")?,
            activation_cache: account(&fixed.activation_cache, "Direct activation cache")?,
            core_program: account(&fixed.core_program, "Core program")?,
            core_programdata: account(&fixed.core_programdata, "Core ProgramData")?,
            trading_program: account(&fixed.trading_program, "Trading program")?,
            trading_programdata: account(&fixed.trading_programdata, "Trading ProgramData")?,
            registry_program: account(&fixed.registry_program, "Registry program")?,
            rent_sysvar: observed(snapshot, sysvar::rent::ID)?,
            instructions_sysvar: observed(snapshot, sysvar::instructions::ID)?,
            product: pair(&fixed.product, "Direct Product")?,
            result_domain: pair(&fixed.result_domain, "Direct ResultDomain")?,
            portfolio: pair(&fixed.portfolio, "Direct Portfolio")?,
            linked_basis: pair(&fixed.linked_basis, "Direct linked basis")?,
            capability_seal: account(&fixed.capability_seal, "Direct capability seal")?,
        },
        seller_maker: account(&coordinates.seller_maker, "seller maker replay")?,
        payer: account(&coordinates.payer, "Direct payer")?,
        lifecycle_rent_credit: account(
            &coordinates.lifecycle_rent_credit,
            "Direct lifecycle RentCredit",
        )?,
        buyer_maker: account(&coordinates.buyer_maker, "buyer maker replay")?,
        rent_program: account(&coordinates.rent_program, "Rent program")?,
        system_program: observed(snapshot, system_program::ID)?,
        claims: DirectClaimsRouteV3 {
            caller_authority: account(&claims.caller_authority, "Claims caller authority")?,
            aggregate: account(&claims.aggregate, "Claims aggregate")?,
            claims_program: account(&claims.claims_program, "Claims program")?,
            claims_programdata: account(&claims.claims_programdata, "Claims ProgramData")?,
            seller_position: account(&claims.seller_position, "seller Claims Position")?,
            buyer_position: account(&claims.buyer_position, "buyer Claims Position")?,
        },
        custody: DirectCustodyRouteV3 {
            caller_authorities: [
                account(custody_authority_0, "Custody caller authority 0")?,
                account(custody_authority_1, "Custody caller authority 1")?,
                account(custody_authority_2, "Custody caller authority 2")?,
                account(custody_authority_3, "Custody caller authority 3")?,
            ],
            realm: pair(&custody.realm, "Direct Realm")?,
            replay: account(&custody.replay, "Custody replay")?,
            mint: account(&custody.mint, "Realm collateral mint")?,
            buyer_token: account(&custody.buyer_token, "buyer token")?,
            seller_token: account(&custody.seller_token, "seller token")?,
            fee_token: account(&custody.fee_token, "fee token")?,
            custody_authority: account(&custody.custody_authority, "Custody authority")?,
            token_program: account(&custody.token_program, "Realm token program")?,
            custody_program: account(&custody.custody_program, "Custody program")?,
            custody_programdata: account(&custody.custody_programdata, "Custody ProgramData")?,
        },
    })
}

fn observed(snapshot: &FinalizedSnapshotV1, key: Pubkey) -> Result<ObservedAccount> {
    Ok(snapshot.account(key)?.clone())
}

fn authenticate_manifest_cluster_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    validated: &ValidatedManifestV1,
) -> Result<()> {
    arguments.expected_cluster.authenticate(&arguments.origin)?;
    let genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("Direct getGenesisHash result was not a string"))?
        .to_owned();
    arguments.origin.authenticate_genesis(&genesis)?;
    if validated.public.cluster != arguments.expected_cluster.evidence_label()
        || validated.public.genesis_hash != genesis
        || (arguments.expected_cluster == ExpectedClusterV1::Devnet
            && genesis != DEVNET_GENESIS_HASH)
    {
        return Err(refusal(
            "Direct public manifest cluster or exact genesis identity changed",
        ));
    }
    Ok(())
}

fn direct_evidence_path_v1(session: &DirectTradePrivateSessionV1) -> Result<PathBuf> {
    let path = PathBuf::from(&session.evidence_file);
    if !path.is_absolute() {
        return Err(refusal("Direct evidence path is not absolute"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| refusal("Direct evidence path omitted its parent"))?;
    let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
        Error::new(format!(
            "Direct evidence parent {}: {error}",
            parent.display()
        ))
    })?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(refusal("Direct evidence parent is not one exact directory"));
    }
    if path.exists() {
        absolute_existing_file(&path, "Direct finalized evidence")
    } else {
        Ok(path)
    }
}

fn direct_setup_binding_v1(
    validated: &ValidatedManifestV1,
) -> Result<DirectSetupManifestBindingV1> {
    DirectSetupManifestBindingV1::new(
        validated.public_sha256.clone(),
        validated.private_sha256.clone(),
    )
}

fn expected_direct_cluster_v1(validated: &ValidatedManifestV1) -> Result<ExpectedClusterV1> {
    match validated.public.cluster.as_str() {
        "devnet" => Ok(ExpectedClusterV1::Devnet),
        "owned-loopback" => Ok(ExpectedClusterV1::OwnedLoopback),
        _ => Err(refusal("Direct manifest cluster is not admitted")),
    }
}

fn direct_setup_paths_v1(validated: &ValidatedManifestV1) -> Result<[PathBuf; 2]> {
    let root = exact_directory(&validated.private.journal_dir, "Direct journal directory")?;
    let setup = exact_directory(
        &root.join("setup").display().to_string(),
        "Direct setup journal directory",
    )?;
    Ok([
        setup.join("0000-replay-setup.json"),
        setup.join("0001-token-setup.json"),
    ])
}

fn load_direct_setup_journals_v1(validated: &ValidatedManifestV1) -> Result<DirectSetupJournalsV1> {
    let binding = direct_setup_binding_v1(validated)?;
    let paths = direct_setup_paths_v1(validated)?;
    let setup_dir = paths
        .first()
        .and_then(|path| path.parent())
        .ok_or_else(|| refusal("Direct setup journal path omitted parent"))?;
    let mut names = fs::read_dir(setup_dir)?
        .map(|entry| {
            entry.map_err(Error::from).and_then(|entry| {
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| refusal("Direct setup directory contained non-UTF8 entry"))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    names.sort_unstable();
    for name in &names {
        if name != "0000-replay-setup.json" && name != "0001-token-setup.json" {
            return Err(refusal(format!(
                "Direct setup journal directory contained unexpected entry {name}"
            )));
        }
    }
    let read = |path: &Path, label: &str| -> Result<Option<DirectSetupJournalV1>> {
        if !path.exists() {
            return Ok(None);
        }
        let path = absolute_existing_file(path, label)?;
        let bytes = fs::read(path)?;
        require_unique_json_v1(&bytes, label)?;
        Ok(Some(serde_json::from_slice(&bytes)?))
    };
    let replay = read(&paths[0], "Direct replay setup journal")?;
    let token = read(&paths[1], "Direct token setup journal")?;
    match (&replay, &token) {
        (None, Some(_)) => {
            return Err(refusal(
                "Direct token setup journal existed without replay setup",
            ));
        }
        (Some(replay), Some(token)) => {
            authenticate_direct_setup_chain_v1(&binding, replay, token)?;
        }
        (Some(replay), None) => {
            authenticate_direct_setup_journal_v1(&binding, replay, None)?;
        }
        (None, None) => {}
    }
    let expected_cluster = expected_direct_cluster_v1(validated)?;
    for journal in replay.iter().chain(token.iter()) {
        expected_cluster.authenticate_finalized_fee(
            journal.exact_fee_lamports,
            "Direct setup durable transaction",
        )?;
        if let Some(fee) = journal.fee_lamports {
            expected_cluster
                .authenticate_finalized_fee(fee, "Direct setup finalized transaction")?;
        }
    }
    Ok(DirectSetupJournalsV1 { replay, token })
}

fn write_direct_setup_journal_v1(
    path: &Path,
    binding: &DirectSetupManifestBindingV1,
    journal: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
    create_new: bool,
    previous_state_sha256: Option<&str>,
) -> Result<()> {
    authenticate_direct_setup_journal_v1(binding, journal, predecessor)?;
    if !path.is_absolute() {
        return Err(refusal("Direct setup journal path must be absolute"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| refusal("Direct setup journal omitted parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("Direct setup journal omitted UTF-8 file name"))?;
    let lock = parent.join(format!(".{name}.direct-setup.lock"));
    let lock_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| {
            Error::new(format!(
                "REFUSED: acquire exclusive Direct setup journal lock {}: {error}",
                lock.display()
            ))
        })?;
    lock_file.sync_all()?;
    if create_new {
        if path.exists() {
            let _ = fs::remove_file(&lock);
            return Err(refusal("Direct setup journal already exists"));
        }
    } else {
        let bytes = fs::read(path)?;
        require_unique_json_v1(&bytes, "Direct persisted setup journal")?;
        let persisted: DirectSetupJournalV1 = serde_json::from_slice(&bytes)?;
        if previous_state_sha256 != Some(persisted.state_sha256.as_str()) {
            let _ = fs::remove_file(&lock);
            return Err(refusal(
                "Direct setup journal update was based on stale persisted state",
            ));
        }
    }
    let temporary = parent.join(format!(".{name}.direct-setup-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(journal)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    if create_new {
        fs::hard_link(&temporary, path).map_err(|error| {
            Error::new(format!(
                "publish new Direct setup journal {}: {error}",
                path.display()
            ))
        })?;
        fs::remove_file(&temporary)?;
    } else {
        fs::rename(&temporary, path)?;
    }
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    fs::remove_file(lock)?;
    Ok(())
}

fn direct_setup_message_v1(
    planning: &DirectTradeSetupPlanningV1,
    stage: DirectSetupStageV1,
    blockhash: SolanaHash,
) -> VersionedMessage {
    let instruction = match stage {
        DirectSetupStageV1::ReplaySetup => &planning.replay.instruction,
        DirectSetupStageV1::TokenSetup => &planning.tokens.instruction,
    };
    VersionedMessage::Legacy(Message::new_with_blockhash(
        core::slice::from_ref(instruction),
        Some(&planning.replay.coordinates.payer),
        &blockhash,
    ))
}

fn direct_setup_expected_poststates_v1(
    planning: &DirectTradeSetupPlanningV1,
    stage: DirectSetupStageV1,
) -> Result<(
    Option<DirectSetupReturnDataV1>,
    Vec<DirectSetupAccountPoststateV1>,
)> {
    match stage {
        DirectSetupStageV1::ReplaySetup => {
            let expected = project_direct_replay_setup_poststate_v1(&planning.replay)?;
            Ok((
                Some(DirectSetupReturnDataV1::new(
                    planning.replay.coordinates.trading_program,
                    &expected.receipt_bytes,
                )?),
                vec![DirectSetupAccountPoststateV1::new(
                    planning.replay.coordinates.custody_replay,
                    planning.replay.coordinates.custody_program,
                    planning.replay.exact_replay_rent,
                    false,
                    &expected.replay_bytes,
                )],
            ))
        }
        DirectSetupStageV1::TokenSetup => Ok((
            Some(DirectSetupReturnDataV1::new(
                planning.tokens.coordinates.trading_program,
                &planning.tokens.expected_receipt_bytes,
            )?),
            vec![
                DirectSetupAccountPoststateV1::new(
                    planning.tokens.coordinates.seller_token,
                    planning.tokens.coordinates.token_program,
                    planning.tokens.exact_account_rent,
                    false,
                    &planning.tokens.expected_seller_bytes,
                ),
                DirectSetupAccountPoststateV1::new(
                    planning.tokens.coordinates.fee_token,
                    planning.tokens.coordinates.token_program,
                    planning.tokens.exact_account_rent,
                    false,
                    &planning.tokens.expected_fee_bytes,
                ),
                // The payer and the rent refund are shared accounts, so these
                // two rows carry the instruction's PROJECTED effect on them -
                // fee-exclusive, and true of no later chain read. They are the
                // record of what this transaction moved, and the source the
                // resume path winds back through to rebuild the exact plan;
                // finalization holds them to identity and to the landed
                // transaction's own balance deltas, never to a balance.
                DirectSetupAccountPoststateV1::new(
                    planning.tokens.coordinates.payer,
                    planning.payer_observed.owner,
                    planning.tokens.projected_payer_lamports,
                    planning.payer_observed.executable,
                    &planning.payer_observed.data,
                ),
                DirectSetupAccountPoststateV1::new(
                    planning.tokens.coordinates.rent_refund,
                    planning.rent_refund_observed.owner,
                    planning.tokens.projected_refund_lamports,
                    planning.rent_refund_observed.executable,
                    &planning.rent_refund_observed.data,
                ),
            ],
        )),
    }
}

fn direct_setup_planned_journal_v1(
    rpc: &mut Rpc,
    validated: &ValidatedManifestV1,
    planning: &DirectTradeSetupPlanningV1,
    stage: DirectSetupStageV1,
    existing: Option<&DirectSetupJournalV1>,
    predecessor: Option<&DirectSetupJournalV1>,
) -> Result<DirectSetupJournalV1> {
    let (blockhash, last_valid_block_height, exact_fee_lamports) = match existing {
        Some(journal) => {
            let bytes = BASE64
                .decode(&journal.message_base64)
                .map_err(|error| Error::new(format!("Direct setup message base64: {error}")))?;
            let message: VersionedMessage = bincode::deserialize(&bytes)
                .map_err(|error| Error::new(format!("Direct setup message: {error}")))?;
            let blockhash = match message {
                VersionedMessage::Legacy(message) => message.recent_blockhash,
                VersionedMessage::V0(message) => message.recent_blockhash,
            };
            (
                blockhash,
                journal.last_valid_block_height,
                journal.exact_fee_lamports,
            )
        }
        None => {
            let (blockhash, last_valid_block_height) = latest_direct_blockhash_v1(rpc)?;
            let message = direct_setup_message_v1(planning, stage, blockhash);
            let encoded = BASE64.encode(message.serialize());
            let fee =
                direct_fee_for_message_v1(rpc, &encoded, expected_direct_cluster_v1(validated)?)?;
            (blockhash, last_valid_block_height, fee)
        }
    };
    let (expected_return_data, expected_poststates) =
        direct_setup_expected_poststates_v1(planning, stage)?;
    let plan = DirectSetupJournalPlanV1 {
        message: direct_setup_message_v1(planning, stage, blockhash),
        last_valid_block_height,
        exact_fee_lamports,
        expected_signer: planning.replay.coordinates.payer,
        expected_return_data,
        expected_poststates,
    };
    let binding = direct_setup_binding_v1(validated)?;
    match stage {
        DirectSetupStageV1::ReplaySetup => plan_direct_replay_setup_journal_v1(&binding, plan),
        DirectSetupStageV1::TokenSetup => plan_direct_token_setup_journal_v1(
            &binding,
            predecessor.ok_or_else(|| refusal("Direct token setup omitted replay predecessor"))?,
            plan,
        ),
    }
}

struct DirectSetupHistoryV1<'a> {
    meta: &'a Value,
    slot: u64,
    packet: Vec<u8>,
    fee: u64,
    return_data: Option<DirectSetupReturnDataV1>,
}

fn authenticate_direct_setup_history_v1<'a>(
    planning: &DirectTradeSetupPlanningV1,
    journal: &DirectSetupJournalV1,
    transaction: &'a Value,
) -> Result<DirectSetupHistoryV1<'a>> {
    let meta = transaction
        .get("meta")
        .ok_or_else(|| refusal("finalized Direct setup transaction omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(refusal(format!(
            "Direct setup transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let tuple = transaction
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("finalized Direct setup history omitted transaction tuple"))?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(refusal(
            "finalized Direct setup transaction was not exact base64 history",
        ));
    }
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("finalized Direct setup history omitted packet"))?;
    let packet = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("Direct setup history packet base64: {error}")))?;
    if BASE64.encode(&packet) != encoded
        || journal.signed_packet_base64.as_deref() != Some(encoded)
        || journal.signed_packet_sha256.as_deref() != Some(sha256_hex(&packet).as_str())
    {
        return Err(refusal(
            "finalized Direct setup packet differed byte-for-byte from its journal",
        ));
    }
    let finalized_transaction: VersionedTransaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("Direct setup finalized packet: {error}")))?;
    let static_keys = match &finalized_transaction.message {
        VersionedMessage::Legacy(message) => message.account_keys.as_slice(),
        VersionedMessage::V0(_) => {
            return Err(refusal("Direct setup transaction unexpectedly used v0"));
        }
    };
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("finalized Direct setup fee was not u64"))?;
    if fee != journal.exact_fee_lamports {
        return Err(refusal(
            "finalized Direct setup fee differed from its durable message fee",
        ));
    }
    let balances = |name: &str| -> Result<Vec<u64>> {
        meta.get(name)
            .and_then(Value::as_array)
            .ok_or_else(|| refusal(format!("finalized Direct setup {name} was not an array")))?
            .iter()
            .map(|value| {
                value.as_u64().ok_or_else(|| {
                    refusal(format!("finalized Direct setup {name} entry was not u64"))
                })
            })
            .collect()
    };
    let pre = balances("preBalances")?;
    let post = balances("postBalances")?;
    let pre_total = pre
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)));
    let post_total = post
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)));
    if pre.len() != static_keys.len()
        || post.len() != static_keys.len()
        || pre_total.and_then(|before| post_total.and_then(|after| before.checked_sub(after)))
            != Some(u128::from(fee))
    {
        return Err(refusal(
            "finalized Direct setup balances concealed a lamport delta beyond the exact fee",
        ));
    }
    let at = |key: Pubkey| -> Result<(u64, u64)> {
        let index = static_keys
            .iter()
            .position(|candidate| *candidate == key)
            .ok_or_else(|| refusal(format!("Direct setup history omitted account {key}")))?;
        Ok((pre[index], post[index]))
    };
    let payer = planning.replay.coordinates.payer;
    let (payer_before, payer_after) = at(payer)?;
    match journal.stage {
        DirectSetupStageV1::ReplaySetup => {
            let expected = journal
                .expected_return_data
                .as_ref()
                .ok_or_else(|| refusal("Direct replay journal omitted expected receipt"))?;
            let receipt =
                DirectReplaySetupReceiptV1::decode(&expected.body()?).map_err(|error| {
                    Error::new(format!("Direct replay expected receipt: {error:?}"))
                })?;
            let (refund_before, refund_after) = at(planning.replay.coordinates.rent_refund)?;
            let (replay_before, replay_after) = at(planning.replay.coordinates.custody_replay)?;
            if payer_before.checked_sub(payer_after) != fee.checked_add(receipt.payer_top_up)
                || refund_after.checked_sub(refund_before) != Some(receipt.refunded_excess)
                || replay_before != receipt.observed_lamports
                || replay_after != receipt.exact_rent
            {
                return Err(refusal(
                    "Direct replay setup payer/refund/replay lamport arithmetic changed",
                ));
            }
        }
        DirectSetupStageV1::TokenSetup => {
            let expected = journal
                .expected_return_data
                .as_ref()
                .ok_or_else(|| refusal("Direct token journal omitted expected receipt"))?;
            let receipt = DirectTokenSetupReceiptV1::decode(&expected.body()?)
                .map_err(|error| Error::new(format!("Direct token expected receipt: {error:?}")))?;
            let (seller_before, seller_after) = at(planning.tokens.coordinates.seller_token)?;
            let (fee_before, fee_after) = at(planning.tokens.coordinates.fee_token)?;
            let (refund_before, refund_after) = at(planning.tokens.coordinates.rent_refund)?;
            let total_top_up = receipt
                .seller_normalization
                .payer_top_up
                .checked_add(receipt.fee_normalization.payer_top_up)
                .ok_or_else(|| refusal("Direct token setup top-up overflowed"))?;
            let total_refund = receipt
                .seller_normalization
                .refunded_excess
                .checked_add(receipt.fee_normalization.refunded_excess)
                .ok_or_else(|| refusal("Direct token setup refund overflowed"))?;
            if payer_before.checked_sub(payer_after) != fee.checked_add(total_top_up)
                || refund_after.checked_sub(refund_before) != Some(total_refund)
                || seller_before != receipt.seller_normalization.observed_lamports
                || seller_after != planning.tokens.exact_account_rent
                || fee_before != receipt.fee_normalization.observed_lamports
                || fee_after != planning.tokens.exact_account_rent
            {
                return Err(refusal(
                    "Direct token setup payer/refund/account lamport arithmetic changed",
                ));
            }
        }
    }
    let (return_program, return_body) = parse_direct_return_data_v1(meta)?;
    let return_data = match (return_program, return_body) {
        (None, None) => None,
        (Some(program), Some(body)) => Some(DirectSetupReturnDataV1::new(
            parse_key(&program, "Direct setup return producer")?,
            &BASE64
                .decode(&body)
                .map_err(|error| Error::new(format!("Direct setup return body: {error}")))?,
        )?),
        _ => return Err(refusal("Direct setup return-data shape was partial")),
    };
    if return_data != journal.expected_return_data {
        return Err(refusal(
            "Direct setup finalized return producer or body changed",
        ));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .filter(|slot| *slot != 0)
        .ok_or_else(|| refusal("finalized Direct setup slot was not positive u64"))?;
    Ok(DirectSetupHistoryV1 {
        meta,
        slot,
        packet,
        fee,
        return_data,
    })
}

fn direct_setup_finalization_v1(
    rpc: &mut Rpc,
    planning: &DirectTradeSetupPlanningV1,
    journal: &DirectSetupJournalV1,
    transaction: &Value,
) -> Result<DirectSetupFinalizationV1> {
    let history = authenticate_direct_setup_history_v1(planning, journal, transaction)?;
    let keys = journal
        .expected_poststates
        .iter()
        .map(|state| parse_key(&state.address, "Direct setup expected poststate"))
        .collect::<Result<Vec<_>>>()?;
    let (_, observed) = rpc.finalized_observed_accounts(&keys, history.slot)?;
    if observed.len() != journal.expected_poststates.len() {
        return Err(refusal(
            "Direct setup finalized complete poststates changed",
        ));
    }
    // Accounts this transaction created are held to their exact bytes and
    // lamports; nothing else can touch a freshly created program-owned PDA.
    // The payer and the rent refund cannot be: their journal rows carry this
    // instruction's projected effect, which excludes the runtime fee the payer
    // also pays, and a live read of a shared account is a fact about the chain
    // now rather than about this transaction. They are held to identity here,
    // and to their exact deltas by the landed balance vectors that
    // `authenticate_direct_setup_history_v1` has already checked; the row that
    // goes into the record stays the projection the journal committed to.
    let payer_address = planning.tokens.coordinates.payer.to_string();
    let refund_address = planning.tokens.coordinates.rent_refund.to_string();
    let mut poststates = Vec::with_capacity(observed.len());
    for (account, expected) in observed.iter().zip(journal.expected_poststates.iter()) {
        let live = DirectSetupAccountPoststateV1::new(
            account.key,
            account.owner,
            account.lamports,
            account.executable,
            &account.data,
        );
        if expected.address == payer_address || expected.address == refund_address {
            if live.address != expected.address
                || live.owner != expected.owner
                || live.executable != expected.executable
                || live.data_base64 != expected.data_base64
            {
                return Err(refusal(
                    "Direct setup finalized shared-account identity changed",
                ));
            }
            poststates.push(expected.clone());
        } else {
            if live != *expected {
                return Err(refusal(
                    "Direct setup finalized complete poststates changed",
                ));
            }
            poststates.push(live);
        }
    }
    match journal.stage {
        DirectSetupStageV1::ReplaySetup => {
            let returned = history
                .return_data
                .as_ref()
                .ok_or_else(|| refusal("Direct replay setup omitted finalized receipt"))?;
            let replay = observed
                .first()
                .ok_or_else(|| refusal("Direct replay setup omitted replay poststate"))?;
            verify_direct_replay_setup_poststate_v1(
                &planning.replay,
                parse_key(&returned.producer, "Direct replay receipt producer")?,
                &returned.body()?,
                setup_observed_account_v1(replay),
            )?;
        }
        DirectSetupStageV1::TokenSetup => {
            let returned = history
                .return_data
                .as_ref()
                .ok_or_else(|| refusal("Direct token setup omitted finalized receipt"))?;
            let returned_bytes = returned.body()?;
            let seller = observed
                .first()
                .ok_or_else(|| refusal("Direct token setup omitted seller poststate"))?;
            let fee = observed
                .get(1)
                .ok_or_else(|| refusal("Direct token setup omitted fee poststate"))?;
            let immutable = &planning.tokens.expected_immutable_bytes;
            verify_direct_trade_token_setup_poststate_v1(
                &planning.tokens,
                DirectTradeTokenSetupPoststateV1 {
                    return_program: parse_key(&returned.producer, "Direct token receipt producer")?,
                    return_data: &returned_bytes,
                    seller_token: direct_token_setup_observed_v1(seller),
                    fee_token: direct_token_setup_observed_v1(fee),
                    market_bytes: &immutable[0],
                    root_bytes: &immutable[1],
                    realm_bytes: &immutable[2],
                    config_bytes: &immutable[3],
                    claims_aggregate_bytes: &immutable[4],
                    seller_position_bytes: &immutable[5],
                    collateral_mint_bytes: &immutable[6],
                },
            )?;
        }
    }
    let compute_units_consumed = history
        .meta
        .get("computeUnitsConsumed")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("Direct setup finalized history omitted compute units"))?;
    Ok(DirectSetupFinalizationV1 {
        finalized_slot: history.slot,
        transaction_sha256: sha256_hex(&history.packet),
        fee_lamports: history.fee,
        compute_units_consumed,
        return_data: history.return_data,
        poststates,
    })
}

fn execute_direct_setup_action_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    validated: &ValidatedManifestV1,
    planning: &DirectTradeSetupPlanningV1,
    journals: &DirectSetupJournalsV1,
) -> Result<()> {
    let binding = direct_setup_binding_v1(validated)?;
    let paths = direct_setup_paths_v1(validated)?;
    let (stage, existing, predecessor, path) = match journals.replay.as_ref() {
        None => (DirectSetupStageV1::ReplaySetup, None, None, &paths[0]),
        Some(replay) if replay.phase != DirectSetupJournalPhaseV1::Finalized => (
            DirectSetupStageV1::ReplaySetup,
            Some(replay),
            None,
            &paths[0],
        ),
        Some(replay) => match journals.token.as_ref() {
            None => (
                DirectSetupStageV1::TokenSetup,
                None,
                Some(replay),
                &paths[1],
            ),
            Some(token) if token.phase != DirectSetupJournalPhaseV1::Finalized => (
                DirectSetupStageV1::TokenSetup,
                Some(token),
                Some(replay),
                &paths[1],
            ),
            Some(_) => return Err(refusal("Direct setup is already finalized")),
        },
    };
    let reconstructed =
        direct_setup_planned_journal_v1(rpc, validated, planning, stage, existing, predecessor)?;
    let mut journal = match existing {
        Some(journal) => {
            authenticate_direct_setup_against_plan_v1(
                &binding,
                journal,
                &reconstructed,
                predecessor,
            )?;
            journal.clone()
        }
        None => {
            write_direct_setup_journal_v1(path, &binding, &reconstructed, predecessor, true, None)?;
            reconstructed
        }
    };
    resume_direct_setup_transaction_v1(
        rpc,
        arguments,
        validated,
        planning,
        path,
        predecessor,
        &mut journal,
    )?;
    stdout_json_v1(&journal)
}

fn resume_direct_setup_transaction_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    validated: &ValidatedManifestV1,
    planning: &DirectTradeSetupPlanningV1,
    path: &Path,
    predecessor: Option<&DirectSetupJournalV1>,
    journal: &mut DirectSetupJournalV1,
) -> Result<()> {
    let binding = direct_setup_binding_v1(validated)?;
    match direct_setup_recovery_action_v1(&binding, journal, predecessor)? {
        DirectSetupRecoveryActionV1::Complete => {
            authenticate_finalized_direct_setup_history_v1(rpc, planning, journal)?;
        }
        DirectSetupRecoveryActionV1::PollOnly => {
            let signature = journal
                .expected_signature
                .as_deref()
                .ok_or_else(|| refusal("ambiguous Direct setup journal omitted signature"))?;
            let transaction = finalized_direct_transaction_v1(rpc, signature)?;
            let Some(transaction) = transaction else {
                return Err(refusal(format!(
                    "Direct setup transaction {signature} is not finalized; {:?} recovery is poll-only and never re-signs or resubmits",
                    journal.phase
                )));
            };
            let finalized = direct_setup_finalization_v1(rpc, planning, journal, &transaction)?;
            let previous = journal.state_sha256.clone();
            *journal =
                advance_direct_setup_finalized_v1(&binding, journal, predecessor, finalized)?;
            write_direct_setup_journal_v1(
                path,
                &binding,
                journal,
                predecessor,
                false,
                Some(&previous),
            )?;
        }
        DirectSetupRecoveryActionV1::BeginDispatch => {
            let previous = journal.state_sha256.clone();
            *journal = advance_direct_setup_dispatching_v1(&binding, journal, predecessor)?;
            write_direct_setup_journal_v1(
                path,
                &binding,
                journal,
                predecessor,
                false,
                Some(&previous),
            )?;
            resume_direct_setup_transaction_v1(
                rpc,
                arguments,
                validated,
                planning,
                path,
                predecessor,
                journal,
            )?;
        }
        DirectSetupRecoveryActionV1::DispatchIdenticalPacket => {
            let signature = journal
                .expected_signature
                .as_deref()
                .ok_or_else(|| refusal("Dispatching Direct setup omitted signature"))?
                .parse::<Signature>()
                .map_err(|error| Error::new(format!("Direct setup signature: {error}")))?;
            let packet = decode_canonical_base64_v1(
                journal
                    .signed_packet_base64
                    .as_deref()
                    .ok_or_else(|| refusal("Dispatching Direct setup omitted packet"))?,
                "Dispatching Direct setup packet",
            )?;
            let already_finalized = finalized_direct_transaction_v1(rpc, &signature.to_string())?;
            if already_finalized.is_none() {
                let returned = rpc
                    .call_once(
                        "sendTransaction",
                        &json!([BASE64.encode(&packet), {
                            "encoding":"base64",
                            "skipPreflight":false,
                            "preflightCommitment":"finalized",
                            "maxRetries":0
                        }]),
                    )?
                    .as_str()
                    .ok_or_else(|| Error::new("Direct setup sendTransaction omitted signature"))?
                    .parse::<Signature>()
                    .map_err(|error| {
                        Error::new(format!("Direct setup returned signature: {error}"))
                    })?;
                if returned != signature {
                    return Err(refusal(
                        "Direct setup RPC returned a signature different from the persisted packet",
                    ));
                }
            }
            let previous = journal.state_sha256.clone();
            *journal = advance_direct_setup_submitted_v1(&binding, journal, predecessor)?;
            write_direct_setup_journal_v1(
                path,
                &binding,
                journal,
                predecessor,
                false,
                Some(&previous),
            )?;
            let transaction = match already_finalized {
                Some(transaction) => transaction,
                None => wait_direct_finalized_v1(
                    rpc,
                    &signature.to_string(),
                    arguments.origin.pacing().confirm_timeout,
                )?,
            };
            let finalized = direct_setup_finalization_v1(rpc, planning, journal, &transaction)?;
            let previous = journal.state_sha256.clone();
            *journal =
                advance_direct_setup_finalized_v1(&binding, journal, predecessor, finalized)?;
            write_direct_setup_journal_v1(
                path,
                &binding,
                journal,
                predecessor,
                false,
                Some(&previous),
            )?;
        }
        DirectSetupRecoveryActionV1::SignAndPrepare => {
            let current_height = rpc
                .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
                .as_u64()
                .ok_or_else(|| Error::new("Direct setup getBlockHeight result was not u64"))?;
            if current_height > journal.last_valid_block_height {
                return Err(refusal(
                    "Direct setup planned blockhash expired before key access; preserve the journal and create a new session",
                ));
            }
            let payer_path = absolute_existing_file(
                Path::new(&validated.private.payer_keypair),
                "Direct payer keypair",
            )?;
            let payer = Keypair::new_from_array(campaign::read_keypair_file(
                &payer_path,
                "direct-trade-setup-payer",
            )?);
            if payer.pubkey() != planning.replay.coordinates.payer {
                return Err(refusal(
                    "Direct setup payer keypair did not expand to the authenticated payer",
                ));
            }
            let message_bytes = BASE64
                .decode(&journal.message_base64)
                .map_err(|error| Error::new(format!("Direct setup message base64: {error}")))?;
            let message: VersionedMessage = bincode::deserialize(&message_bytes)
                .map_err(|error| Error::new(format!("Direct setup versioned message: {error}")))?;
            let transaction = VersionedTransaction::try_new(message, &[&payer])
                .map_err(|error| Error::new(format!("sign Direct setup transaction: {error}")))?;
            let packet = bincode::serialize(&transaction)
                .map_err(|error| Error::new(format!("serialize Direct setup packet: {error}")))?;
            let previous = journal.state_sha256.clone();
            *journal = advance_direct_setup_signed_v1(&binding, journal, predecessor, &packet)?;
            write_direct_setup_journal_v1(
                path,
                &binding,
                journal,
                predecessor,
                false,
                Some(&previous),
            )?;
            resume_direct_setup_transaction_v1(
                rpc,
                arguments,
                validated,
                planning,
                path,
                predecessor,
                journal,
            )?;
        }
    }
    Ok(())
}

fn authenticate_finalized_direct_setup_history_v1(
    rpc: &mut Rpc,
    planning: &DirectTradeSetupPlanningV1,
    journal: &DirectSetupJournalV1,
) -> Result<()> {
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| refusal("finalized Direct setup journal omitted signature"))?;
    let transaction = finalized_direct_transaction_v1(rpc, signature)?
        .ok_or_else(|| refusal("persisted Direct setup finalization disappeared"))?;
    let history = authenticate_direct_setup_history_v1(planning, journal, &transaction)?;
    if Some(history.slot) != journal.finalized_slot
        || Some(history.fee) != journal.fee_lamports
        || history.return_data != journal.return_data
        || history
            .meta
            .get("computeUnitsConsumed")
            .and_then(Value::as_u64)
            != journal.compute_units_consumed
    {
        return Err(refusal("persisted Direct setup finalized history changed"));
    }
    Ok(())
}

fn execute_direct_action_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    validated: &ValidatedManifestV1,
    planning: &DirectTradePlanningV1,
    next: &NextActionV1,
    evidence_path: &Path,
) -> Result<()> {
    if next.stage == DirectTradeStageV1::Hot
        && planning.seal.already_materialized
        && planning.lookup_table.is_none()
    {
        return Err(refusal(
            "Direct Hot execution omitted its authenticated frozen lookup table",
        ));
    }
    let entries = journal_entries_v1(validated)?;
    if let Some(entry) = entries.last() {
        let path = absolute_existing_file(&entry.path(), "Direct action journal")?;
        let bytes = fs::read(&path)?;
        require_unique_json_v1(&bytes, "Direct action journal")?;
        let mut journal: DirectTradeJournalV1 = serde_json::from_slice(&bytes)?;
        if journal.phase != DirectTradeJournalPhaseV1::Finalized {
            resume_direct_transaction_v1(
                rpc,
                arguments,
                validated,
                planning,
                &path,
                &mut journal,
                evidence_path,
            )?;
            return stdout_json_v1(&journal);
        }
    }

    let path = direct_journal_path_v1(validated, next)?;
    if next.stage == DirectTradeStageV1::LookupActivation {
        authenticate_frozen_lookup_v1(planning)?;
        let table = planning
            .lookup_table
            .as_ref()
            .ok_or_else(|| refusal("Direct lookup activation observation is absent"))?;
        let mut journal = base_direct_journal_v1(validated, planning, next)?;
        journal.phase = DirectTradeJournalPhaseV1::Finalized;
        journal.finalized_slot = Some(table.observation.slot);
        refresh_direct_journal_digest_v1(&mut journal)?;
        write_direct_journal_v1(&path, &journal, true, None)?;
        return stdout_json_v1(&journal);
    }

    let mut journal = build_direct_transaction_journal_v1(rpc, validated, planning, next)?;
    write_direct_journal_v1(&path, &journal, true, None)?;
    resume_direct_transaction_v1(
        rpc,
        arguments,
        validated,
        planning,
        &path,
        &mut journal,
        evidence_path,
    )?;
    stdout_json_v1(&journal)
}

fn direct_journal_path_v1(validated: &ValidatedManifestV1, next: &NextActionV1) -> Result<PathBuf> {
    let directory = exact_directory(&validated.private.journal_dir, "Direct journal directory")?;
    Ok(directory.join(format!(
        "{:04}-{}.json",
        next.action_index,
        direct_stage_label_v1(next.stage)
    )))
}

fn direct_stage_label_v1(stage: DirectTradeStageV1) -> &'static str {
    match stage {
        DirectTradeStageV1::LookupCreate => "lookup-create",
        DirectTradeStageV1::LookupExtend => "lookup-extend",
        DirectTradeStageV1::LookupFreeze => "lookup-freeze",
        DirectTradeStageV1::LookupActivation => "lookup-activation",
        DirectTradeStageV1::CapabilitySeal => "capability-seal",
        DirectTradeStageV1::Hot => "hot",
    }
}

fn base_direct_journal_v1(
    validated: &ValidatedManifestV1,
    planning: &DirectTradePlanningV1,
    next: &NextActionV1,
) -> Result<DirectTradeJournalV1> {
    Ok(DirectTradeJournalV1 {
        schema: direct_journal_schema_v1(&validated.public.cluster)?.into(),
        public_manifest_sha256: validated.public_sha256.clone(),
        private_session_sha256: validated.private_sha256.clone(),
        stage: next.stage,
        action_index: u16::try_from(next.action_index)
            .map_err(|_| refusal("Direct action index exceeded u16"))?,
        phase: DirectTradeJournalPhaseV1::Planned,
        observation_slot: planning.route.physical.observation.slot,
        lookup_creation_slot: planning.provision.creation_slot,
        lookup_table: planning.provision.lookup_table.to_string(),
        lookup_addresses: planning
            .provision
            .addresses
            .iter()
            .map(ToString::to_string)
            .collect(),
        lookup_addresses_sha256: pubkey_list_sha256(&planning.provision.addresses),
        message_base64: None,
        message_sha256: None,
        last_valid_block_height: None,
        exact_fee_lamports: None,
        expected_wire_bytes: None,
        unique_message_account_count: None,
        signed_packet_base64: None,
        expected_signature: None,
        expected_return_data_producer: None,
        expected_return_data_base64: None,
        expected_prestates: Vec::new(),
        expected_poststates: Vec::new(),
        finalized_slot: None,
        transaction_sha256: None,
        fee_lamports: None,
        compute_units_consumed: None,
        return_data_producer: None,
        return_data_base64: None,
        return_data_was_null: None,
        finalized_poststates: Vec::new(),
        intent_sha256: String::new(),
        state_sha256: String::new(),
    })
}

fn build_direct_transaction_journal_v1(
    rpc: &mut Rpc,
    validated: &ValidatedManifestV1,
    planning: &DirectTradePlanningV1,
    next: &NextActionV1,
) -> Result<DirectTradeJournalV1> {
    let (blockhash, last_valid_block_height) = latest_direct_blockhash_v1(rpc)?;
    let payer = planning.route.payer;
    let (message, required_signatures, wire_bytes, unique_accounts) = match next.stage {
        DirectTradeStageV1::LookupCreate
        | DirectTradeStageV1::LookupExtend
        | DirectTradeStageV1::LookupFreeze => {
            let instruction = match next.stage {
                DirectTradeStageV1::LookupCreate => planning.provision.create.clone(),
                DirectTradeStageV1::LookupExtend => planning
                    .provision
                    .extensions
                    .get(next.action_index.saturating_sub(1))
                    .cloned()
                    .ok_or_else(|| refusal("Direct lookup extension index changed"))?,
                DirectTradeStageV1::LookupFreeze => planning.provision.freeze.clone(),
                _ => return Err(refusal("Direct infrastructure stage changed")),
            };
            let legacy = Message::new_with_blockhash(
                core::slice::from_ref(&instruction),
                Some(&payer),
                &blockhash,
            );
            let unique_accounts = legacy.account_keys.len();
            let message = VersionedMessage::Legacy(legacy);
            let wire = exact_direct_wire_bytes_v1(&message, 1)?;
            (message, 1_u8, wire, unique_accounts)
        }
        DirectTradeStageV1::CapabilitySeal => {
            let table = planning
                .lookup_table
                .as_ref()
                .ok_or_else(|| refusal("Direct seal omitted its frozen lookup table"))?;
            let plan = compile_direct_inline_capability_seal_routed_v0_v3(
                &planning.seal,
                blockhash,
                &planning.provision,
                table,
            )
            .map_err(|error| Error::new(format!("Direct seal v0 message: {error:?}")))?;
            let unique = versioned_unique_accounts_v1(&plan.message, plan.loaded_addresses)?;
            (
                plan.message,
                plan.required_signatures,
                plan.wire_bytes,
                unique,
            )
        }
        DirectTradeStageV1::Hot => {
            let table = planning
                .lookup_table
                .as_ref()
                .ok_or_else(|| refusal("Direct Hot omitted its frozen lookup table"))?;
            let plan = compile_direct_inline_routed_v0_v3(
                &planning.hot.sealed_report,
                &planning.route.sealed_execution_physical,
                payer,
                blockhash,
                &planning.provision,
                table,
            )
            .map_err(|error| Error::new(format!("Direct Hot v0 message: {error:?}")))?;
            if plan.required_signers != vec![payer] {
                return Err(refusal("Direct Hot message signer closure changed"));
            }
            let unique =
                versioned_unique_accounts_v1(&plan.message.message, plan.message.loaded_addresses)?;
            if unique != 61
                || plan.message.loaded_addresses != 57
                || planning.provision.addresses.len() != 57
                || plan.message.wire_bytes != 1_167
            {
                return Err(refusal(format!(
                    "Direct Hot expected static4+loaded57=61 and 1,167 bytes; observed unique={unique}, loaded={}, table={}, wire={}",
                    plan.message.loaded_addresses,
                    planning.provision.addresses.len(),
                    plan.message.wire_bytes,
                )));
            }
            (
                plan.message.message,
                plan.message.required_signatures,
                plan.message.wire_bytes,
                unique,
            )
        }
        DirectTradeStageV1::LookupActivation => {
            return Err(refusal("Direct lookup activation is observational"));
        }
    };
    if required_signatures != 1 || unique_accounts > 64 {
        return Err(refusal(format!(
            "Direct {:?} message has {required_signatures} signatures and {unique_accounts} unique accounts",
            next.stage
        )));
    }
    let message_bytes = bincode::serialize(&message)
        .map_err(|error| Error::new(format!("serialize Direct versioned message: {error}")))?;
    let message_base64 = BASE64.encode(&message_bytes);
    let exact_fee_lamports =
        direct_fee_for_message_v1(rpc, &message_base64, expected_direct_cluster_v1(validated)?)?;
    let mut journal = base_direct_journal_v1(validated, planning, next)?;
    journal.message_base64 = Some(message_base64);
    journal.message_sha256 = Some(sha256_hex(&message_bytes));
    journal.last_valid_block_height = Some(last_valid_block_height);
    journal.exact_fee_lamports = Some(exact_fee_lamports);
    journal.expected_wire_bytes = Some(wire_bytes);
    journal.unique_message_account_count = Some(unique_accounts);
    if next.stage == DirectTradeStageV1::Hot {
        journal.expected_return_data_producer =
            Some(planning.seal.instruction.program_id.to_string());
        journal.expected_return_data_base64 =
            Some(BASE64.encode(planning.hot.finalization.ack_bytes));
        journal.expected_prestates = planning.hot_prestates.clone();
        journal.expected_poststates = expected_hot_poststates_v1(planning);
    } else if next.stage == DirectTradeStageV1::CapabilitySeal {
        journal.expected_poststates = vec![DirectTradeExpectedPoststateV1 {
            address: planning.seal.seal.to_string(),
            owner: planning.seal.instruction.program_id.to_string(),
            lamports: planning.seal.expected_final_lamports,
            executable: false,
            data_base64: BASE64.encode(&planning.seal.expected_body),
            data_sha256: sha256_hex(&planning.seal.expected_body),
        }];
    }
    refresh_direct_journal_digest_v1(&mut journal)?;
    Ok(journal)
}

fn expected_hot_poststates_v1(
    planning: &DirectTradePlanningV1,
) -> Vec<DirectTradeExpectedPoststateV1> {
    planning
        .hot
        .poststates
        .iter()
        .map(|expected| DirectTradeExpectedPoststateV1 {
            address: Pubkey::new_from_array(expected.commitment.address).to_string(),
            owner: Pubkey::new_from_array(expected.commitment.owner).to_string(),
            lamports: expected.commitment.lamports,
            executable: false,
            data_base64: BASE64.encode(&expected.data),
            data_sha256: sha256_hex(&expected.data),
        })
        .collect()
}

fn direct_finalized_mutations_v1(
    validated: &ValidatedManifestV1,
) -> Result<(
    Vec<DirectFinalizedMutationEvidenceV1>,
    DirectLookupActivationEvidenceV1,
)> {
    let setup = load_direct_setup_journals_v1(validated)?;
    let replay = setup
        .replay
        .ok_or_else(|| refusal("Direct terminal evidence omitted replay setup journal"))?;
    let token = setup
        .token
        .ok_or_else(|| refusal("Direct terminal evidence omitted token setup journal"))?;
    let binding = direct_setup_binding_v1(validated)?;
    authenticate_direct_setup_chain_v1(&binding, &replay, &token)?;
    if replay.phase != DirectSetupJournalPhaseV1::Finalized
        || token.phase != DirectSetupJournalPhaseV1::Finalized
    {
        return Err(refusal(
            "Direct terminal evidence setup journals were not finalized",
        ));
    }
    let setup_paths = direct_setup_paths_v1(validated)?;
    let mut mutations = Vec::new();
    for (path, journal, kind) in [
        (&setup_paths[0], &replay, "replay-setup"),
        (&setup_paths[1], &token, "token-setup"),
    ] {
        let bytes = fs::read(path)?;
        require_unique_json_v1(&bytes, "Direct terminal setup journal")?;
        mutations.push(DirectFinalizedMutationEvidenceV1 {
            kind: kind.into(),
            prefix_len: None,
            path: path.display().to_string(),
            sha256: sha256_hex(&bytes),
            intent_sha256: journal.message_sha256.clone(),
            schema: DIRECT_SETUP_JOURNAL_SCHEMA_V1.into(),
            completion_pointer: "/phase".into(),
            completion_value: "finalized".into(),
            signature: journal
                .expected_signature
                .clone()
                .ok_or_else(|| refusal("Direct setup mutation omitted signature"))?,
            slot: journal
                .finalized_slot
                .ok_or_else(|| refusal("Direct setup mutation omitted finalized slot"))?,
            fee_payer: journal.expected_signer.clone(),
            fee_lamports: journal
                .fee_lamports
                .ok_or_else(|| refusal("Direct setup mutation omitted finalized fee"))?,
            compute_units_consumed: journal
                .compute_units_consumed
                .ok_or_else(|| refusal("Direct setup mutation omitted finalized CU"))?,
        });
    }

    let mut activation = None;
    for entry in journal_entries_v1(validated)? {
        let path = entry.path();
        let bytes = fs::read(&path)?;
        require_unique_json_v1(&bytes, "Direct terminal action journal")?;
        let journal: DirectTradeJournalV1 = serde_json::from_slice(&bytes)?;
        if journal.phase != DirectTradeJournalPhaseV1::Finalized
            || journal.schema != direct_journal_schema_v1(&validated.public.cluster)?
            || journal.public_manifest_sha256 != validated.public_sha256
            || journal.private_session_sha256 != validated.private_sha256
            || journal.state_sha256 != journal_state_sha256(&journal)?
        {
            return Err(refusal(
                "Direct terminal action journal was not exact finalized evidence",
            ));
        }
        if journal.stage == DirectTradeStageV1::LookupActivation {
            if activation.is_some()
                || journal.expected_signature.is_some()
                || journal.fee_lamports.is_some()
                || journal.compute_units_consumed.is_some()
            {
                return Err(refusal(
                    "Direct lookup activation was duplicated or fabricated as a mutation",
                ));
            }
            activation = Some(DirectLookupActivationEvidenceV1 {
                path: path.display().to_string(),
                sha256: sha256_hex(&bytes),
                intent_sha256: journal.intent_sha256,
                schema: journal.schema,
                completion_pointer: "/phase".into(),
                completion_value: "finalized".into(),
                finalized_slot: journal
                    .finalized_slot
                    .ok_or_else(|| refusal("Direct lookup activation omitted finalized slot"))?,
                lookup_table: journal.lookup_table,
                lookup_addresses_sha256: journal.lookup_addresses_sha256,
            });
            continue;
        }
        let kind = match journal.stage {
            DirectTradeStageV1::LookupCreate => "lookup-create",
            DirectTradeStageV1::LookupExtend => "lookup-extend",
            DirectTradeStageV1::LookupFreeze => "lookup-freeze",
            DirectTradeStageV1::CapabilitySeal => "capability-seal",
            DirectTradeStageV1::Hot => "hot",
            DirectTradeStageV1::LookupActivation => unreachable!(),
        };
        let prefix_len = if journal.stage == DirectTradeStageV1::LookupExtend {
            Some(
                usize::from(journal.action_index)
                    .checked_mul(20)
                    .ok_or_else(|| refusal("Direct lookup extension prefix overflowed"))?
                    .min(journal.lookup_addresses.len()),
            )
        } else {
            None
        };
        mutations.push(DirectFinalizedMutationEvidenceV1 {
            kind: kind.into(),
            prefix_len,
            path: path.display().to_string(),
            sha256: sha256_hex(&bytes),
            intent_sha256: journal.intent_sha256,
            schema: journal.schema,
            completion_pointer: "/phase".into(),
            completion_value: "finalized".into(),
            signature: journal
                .expected_signature
                .ok_or_else(|| refusal("Direct terminal mutation omitted signature"))?,
            slot: journal
                .finalized_slot
                .ok_or_else(|| refusal("Direct terminal mutation omitted finalized slot"))?,
            fee_payer: validated.public.payer.clone(),
            fee_lamports: journal
                .fee_lamports
                .ok_or_else(|| refusal("Direct terminal mutation omitted finalized fee"))?,
            compute_units_consumed: journal
                .compute_units_consumed
                .ok_or_else(|| refusal("Direct terminal mutation omitted finalized CU"))?,
        });
    }
    let activation = activation
        .ok_or_else(|| refusal("Direct terminal evidence omitted lookup activation observation"))?;
    if mutations
        .first()
        .is_none_or(|row| row.kind != "replay-setup")
        || mutations.get(1).is_none_or(|row| row.kind != "token-setup")
        || mutations.last().is_none_or(|row| row.kind != "hot")
    {
        return Err(refusal("Direct terminal mutation order changed"));
    }
    Ok((mutations, activation))
}

fn direct_position_transition_v1(
    journal: &DirectTradeJournalV1,
    account: &str,
    owner: &str,
) -> Result<DirectPositionTransitionEvidenceV1> {
    let pre = journal
        .expected_prestates
        .iter()
        .find(|state| state.address == account)
        .ok_or_else(|| refusal("Direct Hot journal omitted Position prestate"))?;
    let post = journal
        .expected_poststates
        .iter()
        .find(|state| state.address == account)
        .ok_or_else(|| refusal("Direct Hot journal omitted Position poststate"))?;
    if pre.owner != post.owner || pre.executable || post.executable {
        return Err(refusal("Direct Position transition envelope changed"));
    }
    let pre_bytes = decode_canonical_base64_v1(&pre.data_base64, "Direct Position prestate")?;
    let post_bytes = decode_canonical_base64_v1(&post.data_base64, "Direct Position poststate")?;
    let pre_view = LiabilityBasisPositionViewV2::decode(&pre_bytes)
        .map_err(|error| Error::new(format!("Direct Position prestate: {error:?}")))?;
    let post_view = LiabilityBasisPositionViewV2::decode(&post_bytes)
        .map_err(|error| Error::new(format!("Direct Position poststate: {error:?}")))?;
    if pre_view.owner != parse_key(owner, "Direct Position semantic owner")?.to_bytes()
        || post_view.owner != pre_view.owner
        || post_view.market_account != pre_view.market_account
        || post_view.basis_id != pre_view.basis_id
        || post_view.claim_count != pre_view.claim_count
        || post_view.revision != pre_view.revision.saturating_add(1)
    {
        return Err(refusal("Direct Position transition semantic facts changed"));
    }
    Ok(DirectPositionTransitionEvidenceV1 {
        account: account.into(),
        owner: owner.into(),
        pre_data_base64: pre.data_base64.clone(),
        post_data_base64: post.data_base64.clone(),
    })
}

fn direct_claim_balances_v1(
    positions: &[DirectPositionTransitionEvidenceV1; 2],
    seller_recipient: &str,
    buyer_recipient: &str,
) -> Result<Vec<DirectClaimBalanceEvidenceV1>> {
    let mut balances = Vec::new();
    for (position, recipient) in positions.iter().zip([seller_recipient, buyer_recipient]) {
        let bytes = decode_canonical_base64_v1(
            &position.post_data_base64,
            "Direct terminal Position poststate",
        )?;
        let view = LiabilityBasisPositionViewV2::decode(&bytes)
            .map_err(|error| Error::new(format!("Direct terminal Position: {error:?}")))?;
        for claim_index in 0..view.claim_count {
            let quantity_atoms = view
                .balance(&bytes, claim_index)
                .map_err(|error| Error::new(format!("Direct terminal claim balance: {error:?}")))?;
            if quantity_atoms != 0 {
                balances.push(DirectClaimBalanceEvidenceV1 {
                    owner: position.owner.clone(),
                    position: position.account.clone(),
                    recipient_token: recipient.into(),
                    claim_index,
                    quantity_atoms,
                });
            }
        }
    }
    Ok(balances)
}

fn authenticate_direct_claim_schedule_v1(evidence: &DirectTradeFinalizedEvidenceV1) -> Result<()> {
    let seller_claim_count = usize::try_from(evidence.outcome_count)
        .map_err(|_| refusal("Direct outcome count does not fit this host"))?;
    let expected_claim_count = seller_claim_count
        .checked_add(1)
        .ok_or_else(|| refusal("Direct claim schedule count overflowed"))?;
    if evidence.claim_balances.len() != expected_claim_count {
        return Err(refusal(
            "Direct terminal claim schedule is not the exact K+1 partition",
        ));
    }
    for (claim_index, row) in evidence
        .claim_balances
        .iter()
        .take(seller_claim_count)
        .enumerate()
    {
        if row.owner != evidence.seller_owner
            || row.position != evidence.seller_position
            || row.recipient_token != evidence.seller_collateral_destination
            || usize::try_from(row.claim_index).ok() != Some(claim_index)
            || row.quantity_atoms == 0
        {
            return Err(refusal(
                "Direct terminal seller claim schedule is not every outcome in canonical order",
            ));
        }
    }
    let buyer = evidence
        .claim_balances
        .last()
        .ok_or_else(|| refusal("Direct terminal claim schedule omitted the buyer fill"))?;
    if buyer.owner != evidence.buyer_owner
        || buyer.position != evidence.buyer_position
        || buyer.recipient_token != evidence.buyer_collateral_source
        || buyer.claim_index != evidence.outcome_index
        || buyer.quantity_atoms == 0
    {
        return Err(refusal(
            "Direct terminal buyer claim schedule is not the one filled outcome",
        ));
    }
    Ok(())
}

fn latest_direct_blockhash_v1(rpc: &mut Rpc) -> Result<(SolanaHash, u64)> {
    let result = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = result
        .get("value")
        .ok_or_else(|| Error::new("Direct getLatestBlockhash omitted value"))?;
    let blockhash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("Direct getLatestBlockhash omitted blockhash"))?
        .parse::<SolanaHash>()
        .map_err(|error| Error::new(format!("Direct latest blockhash: {error}")))?;
    let last_valid = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("Direct getLatestBlockhash omitted last-valid height"))?;
    Ok((blockhash, last_valid))
}

fn direct_fee_for_message_v1(
    rpc: &mut Rpc,
    message_base64: &str,
    expected_cluster: ExpectedClusterV1,
) -> Result<u64> {
    let fee = rpc
        .call(
            "getFeeForMessage",
            &json!([message_base64, {"commitment":"finalized"}]),
        )?
        .get("value")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("Direct getFeeForMessage omitted exact fee"))?;
    expected_cluster.authenticate_finalized_fee(fee, "Direct fee quote")?;
    Ok(fee)
}

fn exact_direct_wire_bytes_v1(message: &VersionedMessage, signatures: u8) -> Result<usize> {
    bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default(); usize::from(signatures)],
        message: message.clone(),
    })
    .map(|wire| wire.len())
    .map_err(|error| Error::new(format!("Direct unsigned packet geometry: {error}")))
}

fn versioned_unique_accounts_v1(
    message: &VersionedMessage,
    loaded_addresses: usize,
) -> Result<usize> {
    let static_count = match message {
        VersionedMessage::Legacy(message) => message.account_keys.len(),
        VersionedMessage::V0(message) => message.account_keys.len(),
    };
    static_count
        .checked_add(loaded_addresses)
        .ok_or_else(|| refusal("Direct unique account count overflow"))
}

fn refresh_direct_journal_digest_v1(journal: &mut DirectTradeJournalV1) -> Result<()> {
    journal.intent_sha256.clear();
    journal.state_sha256.clear();
    journal.intent_sha256 = journal_intent_sha256_v1(journal)?;
    journal.state_sha256 = journal_state_sha256(journal)?;
    Ok(())
}

fn journal_intent_sha256_v1(journal: &DirectTradeJournalV1) -> Result<String> {
    let mut canonical = journal.clone();
    canonical.phase = DirectTradeJournalPhaseV1::Planned;
    canonical.signed_packet_base64 = None;
    canonical.expected_signature = None;
    canonical.finalized_slot = None;
    canonical.transaction_sha256 = None;
    canonical.fee_lamports = None;
    canonical.compute_units_consumed = None;
    canonical.return_data_producer = None;
    canonical.return_data_base64 = None;
    canonical.return_data_was_null = None;
    canonical.finalized_poststates.clear();
    canonical.intent_sha256.clear();
    canonical.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn stdout_json_v1(value: &impl Serialize) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&serde_json::to_vec_pretty(value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn write_direct_journal_v1(
    path: &Path,
    journal: &DirectTradeJournalV1,
    create_new: bool,
    previous_state_sha256: Option<&str>,
) -> Result<()> {
    if !path.is_absolute() {
        return Err(refusal("Direct journal path must be absolute"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| refusal("Direct journal omitted parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("Direct journal omitted UTF-8 file name"))?;
    let lock = parent.join(format!(".{name}.direct-trade.lock"));
    let lock_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| {
            Error::new(format!(
                "REFUSED: acquire exclusive Direct journal lock {}: {error}",
                lock.display()
            ))
        })?;
    lock_file.sync_all()?;
    let persisted = if create_new {
        if path.exists() {
            let _ = fs::remove_file(&lock);
            return Err(refusal("Direct journal already exists"));
        }
        None
    } else {
        let bytes = fs::read(path)?;
        require_unique_json_v1(&bytes, "Direct persisted journal")?;
        let persisted: DirectTradeJournalV1 = serde_json::from_slice(&bytes)?;
        if previous_state_sha256 != Some(persisted.state_sha256.as_str()) {
            let _ = fs::remove_file(&lock);
            return Err(refusal(
                "Direct journal update was based on a stale persisted state",
            ));
        }
        Some(persisted)
    };
    let _ = persisted;
    if journal.intent_sha256 != journal_intent_sha256_v1(journal)?
        || journal.state_sha256 != journal_state_sha256(journal)?
    {
        let _ = fs::remove_file(&lock);
        return Err(refusal(
            "Direct journal intent or state digest changed before persistence",
        ));
    }
    let temporary = parent.join(format!(".{name}.direct-trade-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(journal)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    if create_new {
        fs::hard_link(&temporary, path).map_err(|error| {
            Error::new(format!(
                "publish new Direct journal {}: {error}",
                path.display()
            ))
        })?;
        fs::remove_file(&temporary)?;
    } else {
        fs::rename(&temporary, path)?;
    }
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    fs::remove_file(lock)?;
    Ok(())
}

fn resume_direct_transaction_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    validated: &ValidatedManifestV1,
    planning: &DirectTradePlanningV1,
    path: &Path,
    journal: &mut DirectTradeJournalV1,
    evidence_path: &Path,
) -> Result<()> {
    validate_journal(
        validated,
        planning,
        usize::from(journal.action_index),
        journal,
        None,
    )?;
    match journal.phase {
        DirectTradeJournalPhaseV1::Finalized => {
            authenticate_finalized_direct_history_v1(rpc, journal)?;
        }
        DirectTradeJournalPhaseV1::Submitted => {
            let signature = journal
                .expected_signature
                .clone()
                .ok_or_else(|| refusal("ambiguous Direct journal omitted its signature"))?;
            let Some(transaction) = finalized_direct_transaction_v1(rpc, &signature)? else {
                return Err(refusal(format!(
                    "Direct transaction {signature} is not finalized; {:?} recovery is poll-only and never re-signs or resubmits",
                    journal.phase
                )));
            };
            finalize_direct_transaction_v1(rpc, validated, planning, journal, &transaction)?;
            let previous = journal.state_sha256.clone();
            refresh_direct_journal_digest_v1(journal)?;
            write_direct_journal_v1(path, journal, false, Some(&previous))?;
        }
        DirectTradeJournalPhaseV1::Prepared => {
            let previous = journal.state_sha256.clone();
            journal.phase = DirectTradeJournalPhaseV1::Dispatching;
            refresh_direct_journal_digest_v1(journal)?;
            write_direct_journal_v1(path, journal, false, Some(&previous))?;
            resume_direct_transaction_v1(
                rpc,
                arguments,
                validated,
                planning,
                path,
                journal,
                evidence_path,
            )?;
        }
        DirectTradeJournalPhaseV1::Dispatching => {
            authenticate_planned_direct_message_v1(planning, journal)?;
            let signature = journal
                .expected_signature
                .as_deref()
                .ok_or_else(|| refusal("Dispatching Direct journal omitted signature"))?
                .parse::<Signature>()
                .map_err(|error| Error::new(format!("Direct signature: {error}")))?;
            let packet = decode_canonical_base64_v1(
                journal
                    .signed_packet_base64
                    .as_deref()
                    .ok_or_else(|| refusal("Dispatching Direct journal omitted packet"))?,
                "Dispatching Direct packet",
            )?;
            let already_finalized = finalized_direct_transaction_v1(rpc, &signature.to_string())?;
            if already_finalized.is_none() {
                let returned = rpc
                    .call_once(
                        "sendTransaction",
                        &json!([BASE64.encode(&packet), {
                            "encoding":"base64",
                            "skipPreflight":false,
                            "preflightCommitment":"finalized",
                            "maxRetries":0
                        }]),
                    )?
                    .as_str()
                    .ok_or_else(|| Error::new("Direct sendTransaction omitted signature"))?
                    .parse::<Signature>()
                    .map_err(|error| Error::new(format!("Direct returned signature: {error}")))?;
                if returned != signature {
                    return Err(refusal(
                        "Direct RPC returned a signature different from the persisted packet",
                    ));
                }
            }
            let previous = journal.state_sha256.clone();
            journal.phase = DirectTradeJournalPhaseV1::Submitted;
            refresh_direct_journal_digest_v1(journal)?;
            write_direct_journal_v1(path, journal, false, Some(&previous))?;
            let transaction = match already_finalized {
                Some(transaction) => transaction,
                None => wait_direct_finalized_v1(
                    rpc,
                    &signature.to_string(),
                    arguments.origin.pacing().confirm_timeout,
                )?,
            };
            finalize_direct_transaction_v1(rpc, validated, planning, journal, &transaction)?;
            let previous = journal.state_sha256.clone();
            refresh_direct_journal_digest_v1(journal)?;
            write_direct_journal_v1(path, journal, false, Some(&previous))?;
        }
        DirectTradeJournalPhaseV1::Planned => {
            authenticate_planned_direct_message_v1(planning, journal)?;
            let current_height = rpc
                .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
                .as_u64()
                .ok_or_else(|| Error::new("Direct getBlockHeight result was not u64"))?;
            if current_height
                > journal
                    .last_valid_block_height
                    .ok_or_else(|| refusal("Direct planned journal omitted validity height"))?
            {
                return Err(refusal(
                    "Direct planned blockhash expired before key access; preserve the journal and create a new session",
                ));
            }
            let payer_path = absolute_existing_file(
                Path::new(&validated.private.payer_keypair),
                "Direct payer keypair",
            )?;
            let payer = Keypair::new_from_array(campaign::read_keypair_file(
                &payer_path,
                "direct-trade-payer",
            )?);
            if payer.pubkey() != planning.route.payer {
                return Err(refusal(
                    "Direct payer keypair did not expand to the authenticated payer",
                ));
            }
            let message_bytes = BASE64
                .decode(
                    journal
                        .message_base64
                        .as_deref()
                        .ok_or_else(|| refusal("Direct planned journal omitted its message"))?,
                )
                .map_err(|error| Error::new(format!("Direct message base64: {error}")))?;
            let message: VersionedMessage = bincode::deserialize(&message_bytes)
                .map_err(|error| Error::new(format!("Direct versioned message: {error}")))?;
            if bincode::serialize(&message)
                .map_err(|error| Error::new(format!("Direct message reencode: {error}")))?
                != message_bytes
            {
                return Err(refusal("Direct versioned message was noncanonical"));
            }
            let transaction = VersionedTransaction::try_new(message, &[&payer])
                .map_err(|error| Error::new(format!("sign Direct transaction: {error}")))?;
            let packet = bincode::serialize(&transaction)
                .map_err(|error| Error::new(format!("serialize Direct packet: {error}")))?;
            if Some(packet.len()) != journal.expected_wire_bytes || packet.len() > 1_232 {
                return Err(refusal(
                    "Direct signed packet width differed from its durable message geometry",
                ));
            }
            let signature = transaction
                .signatures
                .first()
                .copied()
                .ok_or_else(|| refusal("Direct signed packet omitted payer signature"))?;
            let previous = journal.state_sha256.clone();
            journal.signed_packet_base64 = Some(BASE64.encode(&packet));
            journal.expected_signature = Some(signature.to_string());
            journal.phase = DirectTradeJournalPhaseV1::Prepared;
            refresh_direct_journal_digest_v1(journal)?;
            write_direct_journal_v1(path, journal, false, Some(&previous))?;
            resume_direct_transaction_v1(
                rpc,
                arguments,
                validated,
                planning,
                path,
                journal,
                evidence_path,
            )?;
        }
    }
    if journal.stage == DirectTradeStageV1::Hot
        && journal.phase == DirectTradeJournalPhaseV1::Finalized
    {
        publish_direct_evidence_v1(validated, journal, evidence_path)?;
    }
    Ok(())
}

fn authenticate_planned_direct_message_v1(
    planning: &DirectTradePlanningV1,
    journal: &DirectTradeJournalV1,
) -> Result<()> {
    let message_bytes = BASE64
        .decode(
            journal
                .message_base64
                .as_deref()
                .ok_or_else(|| refusal("Direct journal omitted message"))?,
        )
        .map_err(|error| Error::new(format!("Direct message base64: {error}")))?;
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("Direct durable message: {error}")))?;
    let blockhash = match &message {
        VersionedMessage::Legacy(message) => message.recent_blockhash,
        VersionedMessage::V0(message) => message.recent_blockhash,
    };
    let expected = expected_direct_message_v1(planning, journal, blockhash)?;
    if expected != message
        || journal.message_sha256.as_deref() != Some(sha256_hex(&message_bytes).as_str())
        || journal.expected_wire_bytes != Some(exact_direct_wire_bytes_v1(&message, 1)?)
    {
        return Err(refusal(
            "Direct durable message differed from the freshly rerun semantic owner",
        ));
    }
    Ok(())
}

fn expected_direct_message_v1(
    planning: &DirectTradePlanningV1,
    journal: &DirectTradeJournalV1,
    blockhash: SolanaHash,
) -> Result<VersionedMessage> {
    match journal.stage {
        DirectTradeStageV1::LookupCreate
        | DirectTradeStageV1::LookupExtend
        | DirectTradeStageV1::LookupFreeze => {
            let instruction = match journal.stage {
                DirectTradeStageV1::LookupCreate => planning.provision.create.clone(),
                DirectTradeStageV1::LookupExtend => planning
                    .provision
                    .extensions
                    .get(usize::from(journal.action_index).saturating_sub(1))
                    .cloned()
                    .ok_or_else(|| refusal("Direct lookup extension index changed"))?,
                DirectTradeStageV1::LookupFreeze => planning.provision.freeze.clone(),
                _ => return Err(refusal("Direct infrastructure stage changed")),
            };
            Ok(VersionedMessage::Legacy(Message::new_with_blockhash(
                core::slice::from_ref(&instruction),
                Some(&planning.route.payer),
                &blockhash,
            )))
        }
        DirectTradeStageV1::CapabilitySeal => compile_direct_inline_capability_seal_routed_v0_v3(
            &planning.seal,
            blockhash,
            &planning.provision,
            planning
                .lookup_table
                .as_ref()
                .ok_or_else(|| refusal("Direct seal omitted lookup observation"))?,
        )
        .map(|plan| plan.message)
        .map_err(|error| Error::new(format!("Direct seal message rederive: {error:?}"))),
        DirectTradeStageV1::Hot => compile_direct_inline_routed_v0_v3(
            &planning.hot.sealed_report,
            &planning.route.sealed_execution_physical,
            planning.route.payer,
            blockhash,
            &planning.provision,
            planning
                .lookup_table
                .as_ref()
                .ok_or_else(|| refusal("Direct Hot omitted lookup observation"))?,
        )
        .map(|plan| plan.message.message)
        .map_err(|error| Error::new(format!("Direct Hot message rederive: {error:?}"))),
        DirectTradeStageV1::LookupActivation => {
            Err(refusal("Direct activation has no transaction message"))
        }
    }
}

fn finalized_direct_transaction_v1(rpc: &mut Rpc, signature: &str) -> Result<Option<Value>> {
    let statuses = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let status = statuses
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
            "finalized Direct signature omitted transaction history",
        ));
    }
    Ok(Some(transaction))
}

fn wait_direct_finalized_v1(rpc: &mut Rpc, signature: &str, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(transaction) = finalized_direct_transaction_v1(rpc, signature)? {
            return Ok(transaction);
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(refusal(format!(
        "Direct transaction {signature} did not reach finalized history before the bounded deadline; the durable signature is retained and no replay is attempted"
    )))
}

struct DirectHistoryV1<'a> {
    meta: &'a Value,
    slot: u64,
    packet: Vec<u8>,
    fee: u64,
    return_data_producer: Option<String>,
    return_data_base64: Option<String>,
}

fn authenticate_direct_history_v1<'a>(
    journal: &DirectTradeJournalV1,
    transaction: &'a Value,
) -> Result<DirectHistoryV1<'a>> {
    let meta = transaction
        .get("meta")
        .ok_or_else(|| refusal("finalized Direct transaction omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(refusal(format!(
            "Direct transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let tuple = transaction
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("finalized Direct history omitted transaction tuple"))?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(refusal(
            "finalized Direct transaction was not exact base64 history",
        ));
    }
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("finalized Direct history omitted packet"))?;
    let packet = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("Direct history packet base64: {error}")))?;
    if BASE64.encode(&packet) != encoded
        || journal.signed_packet_base64.as_deref() != Some(encoded)
        || journal
            .transaction_sha256
            .as_ref()
            .is_some_and(|digest| digest != &sha256_hex(&packet))
    {
        return Err(refusal(
            "finalized Direct packet differed byte-for-byte from its journal",
        ));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("finalized Direct fee was not u64"))?;
    if Some(fee) != journal.exact_fee_lamports {
        return Err(refusal(
            "finalized Direct fee differed from its exact durable message fee",
        ));
    }
    let balances = |name: &str| -> Result<Vec<u64>> {
        meta.get(name)
            .and_then(Value::as_array)
            .ok_or_else(|| refusal(format!("finalized Direct {name} was not an array")))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| refusal(format!("finalized Direct {name} entry was not u64")))
            })
            .collect()
    };
    let pre = balances("preBalances")?;
    let post = balances("postBalances")?;
    let pre_total = pre
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)));
    let post_total = post
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)));
    if pre.len() != post.len()
        || pre_total.and_then(|pre| post_total.and_then(|post| pre.checked_sub(post)))
            != Some(u128::from(fee))
    {
        return Err(refusal(
            "finalized Direct balance vectors concealed a lamport delta beyond the exact fee",
        ));
    }
    let (return_data_producer, return_data_base64) = parse_direct_return_data_v1(meta)?;
    if let Some(expected) = &journal.expected_return_data_base64 {
        if return_data_producer != journal.expected_return_data_producer
            || return_data_base64.as_ref() != Some(expected)
        {
            return Err(refusal(
                "finalized Direct Hot return producer or canonical ACK bytes changed",
            ));
        }
    } else if return_data_producer.is_some() || return_data_base64.is_some() {
        return Err(refusal(
            "Direct infrastructure/seal transaction carried unexpected return data",
        ));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("finalized Direct slot was not u64"))?;
    Ok(DirectHistoryV1 {
        meta,
        slot,
        packet,
        fee,
        return_data_producer,
        return_data_base64,
    })
}

fn parse_direct_return_data_v1(meta: &Value) -> Result<(Option<String>, Option<String>)> {
    // A transaction that set no return data is reported either as an explicit
    // null or with the key left out entirely, depending on the RPC - devnet
    // omits it. The two encode one fact, so both read as "no return data", and
    // requiring the key would refuse every transaction whose programs simply
    // never called set_return_data. Nothing is conceded by accepting the
    // absence: a caller who wanted to hide return data could send the null
    // just as easily, and each stage separately pins whether it expects any.
    let Some(return_data) = meta.get("returnData").filter(|value| !value.is_null()) else {
        return Ok((None, None));
    };
    let producer = return_data
        .get("programId")
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("Direct returnData omitted producer"))?
        .to_owned();
    let tuple = return_data
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("Direct returnData omitted body tuple"))?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(refusal(
            "Direct returnData tuple was not exactly [body, base64]",
        ));
    }
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("Direct returnData omitted base64 body"))?;
    let body = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("Direct returnData base64: {error}")))?;
    if BASE64.encode(&body) != encoded {
        return Err(refusal("Direct returnData base64 was noncanonical"));
    }
    Ok((Some(producer), Some(encoded.to_owned())))
}

fn finalize_direct_transaction_v1(
    rpc: &mut Rpc,
    _validated: &ValidatedManifestV1,
    planning: &DirectTradePlanningV1,
    journal: &mut DirectTradeJournalV1,
    transaction: &Value,
) -> Result<()> {
    let history = authenticate_direct_history_v1(journal, transaction)?;
    let finalized_poststates =
        verify_direct_stage_poststate_v1(rpc, planning, journal, history.slot)?;
    journal.phase = DirectTradeJournalPhaseV1::Finalized;
    journal.finalized_slot = Some(history.slot);
    journal.transaction_sha256 = Some(sha256_hex(&history.packet));
    journal.fee_lamports = Some(history.fee);
    journal.compute_units_consumed = history
        .meta
        .get("computeUnitsConsumed")
        .and_then(Value::as_u64);
    journal.return_data_was_null = Some(history.return_data_producer.is_none());
    journal.return_data_producer = history.return_data_producer;
    journal.return_data_base64 = history.return_data_base64;
    journal.finalized_poststates = finalized_poststates;
    Ok(())
}

fn authenticate_finalized_direct_history_v1(
    rpc: &mut Rpc,
    journal: &DirectTradeJournalV1,
) -> Result<()> {
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| refusal("finalized Direct journal omitted signature"))?;
    let transaction = finalized_direct_transaction_v1(rpc, signature)?
        .ok_or_else(|| refusal("persisted Direct finalization disappeared"))?;
    let history = authenticate_direct_history_v1(journal, &transaction)?;
    if Some(history.slot) != journal.finalized_slot
        || Some(history.fee) != journal.fee_lamports
        || history.return_data_producer != journal.return_data_producer
        || history.return_data_base64 != journal.return_data_base64
    {
        return Err(refusal(
            "persisted Direct finalization slot, fee, or return data changed",
        ));
    }
    Ok(())
}

fn verify_direct_stage_poststate_v1(
    rpc: &mut Rpc,
    planning: &DirectTradePlanningV1,
    journal: &DirectTradeJournalV1,
    finalized_slot: u64,
) -> Result<Vec<DirectTradeObservedPoststateV1>> {
    match journal.stage {
        DirectTradeStageV1::LookupCreate
        | DirectTradeStageV1::LookupExtend
        | DirectTradeStageV1::LookupFreeze => {
            let account = finalized_observed_accounts_v1(
                rpc,
                &[planning.provision.lookup_table],
                finalized_slot,
            )?
            .into_iter()
            .next()
            .ok_or_else(|| refusal("Direct lookup poststate disappeared"))?;
            if account.owner != lookup_table_program::id() || account.executable {
                return Err(refusal("Direct lookup poststate owner/executable changed"));
            }
            let table = AddressLookupTable::deserialize(&account.data)
                .map_err(|_| refusal("Direct lookup poststate bytes refused"))?;
            let expected_len = match journal.stage {
                DirectTradeStageV1::LookupCreate => 0,
                DirectTradeStageV1::LookupExtend => usize::from(journal.action_index)
                    .saturating_mul(20)
                    .min(planning.provision.addresses.len()),
                DirectTradeStageV1::LookupFreeze => planning.provision.addresses.len(),
                _ => return Err(refusal("Direct lookup poststate stage changed")),
            };
            let expected_authority = if journal.stage == DirectTradeStageV1::LookupFreeze {
                None
            } else {
                Some(planning.provision.authority)
            };
            if table.meta.authority != expected_authority
                || table.meta.deactivation_slot != u64::MAX
                || table.addresses.as_ref() != &planning.provision.addresses[..expected_len]
            {
                return Err(refusal(
                    "Direct lookup authority, ordered prefix, or lifecycle poststate changed",
                ));
            }
            Ok(vec![observed_poststate_v1(&account)])
        }
        DirectTradeStageV1::CapabilitySeal => {
            let account =
                finalized_observed_accounts_v1(rpc, &[planning.seal.seal], finalized_slot)?
                    .into_iter()
                    .next()
                    .ok_or_else(|| refusal("Direct capability seal poststate disappeared"))?;
            verify_direct_inline_capability_seal_v3(&planning.seal, &account)
                .map_err(|error| Error::new(format!("Direct finalized seal: {error:?}")))?;
            verify_expected_direct_poststates_v1(
                core::slice::from_ref(&account),
                &journal.expected_poststates,
            )
        }
        DirectTradeStageV1::Hot => {
            let keys = journal
                .expected_poststates
                .iter()
                .map(|expected| parse_key(&expected.address, "Direct expected poststate"))
                .collect::<Result<Vec<_>>>()?;
            let accounts = finalized_observed_accounts_v1(rpc, &keys, finalized_slot)?;
            verify_expected_direct_poststates_v1(&accounts, &journal.expected_poststates)
        }
        DirectTradeStageV1::LookupActivation => {
            Err(refusal("Direct activation has no transaction poststate"))
        }
    }
}

fn finalized_observed_accounts_v1(
    rpc: &mut Rpc,
    keys: &[Pubkey],
    minimum_slot: u64,
) -> Result<Vec<ObservedAccount>> {
    let (slot, values) = rpc.finalized_accounts(keys, minimum_slot)?;
    if slot < minimum_slot || values.len() != keys.len() {
        return Err(refusal(
            "Direct finalized poststate snapshot changed width or slot",
        ));
    }
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    keys.iter()
        .copied()
        .zip(values)
        .map(|(key, value)| {
            let value =
                value.ok_or_else(|| refusal(format!("Direct poststate {key} is absent")))?;
            Ok(observed_from_rpc_v1(observation, key, value))
        })
        .collect()
}

fn observed_from_rpc_v1(
    observation: Observation,
    key: Pubkey,
    value: RpcAccount,
) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner: value.owner,
        lamports: value.lamports,
        executable: value.executable,
        data: value.data,
    }
}

fn verify_expected_direct_poststates_v1(
    accounts: &[ObservedAccount],
    expected: &[DirectTradeExpectedPoststateV1],
) -> Result<Vec<DirectTradeObservedPoststateV1>> {
    if accounts.len() != expected.len() {
        return Err(refusal("Direct finalized poststate width changed"));
    }
    let mut output = Vec::with_capacity(accounts.len());
    for (account, expected) in accounts.iter().zip(expected) {
        let expected_data = BASE64
            .decode(&expected.data_base64)
            .map_err(|error| Error::new(format!("Direct expected poststate base64: {error}")))?;
        if account.key.to_string() != expected.address
            || account.owner.to_string() != expected.owner
            || account.lamports != expected.lamports
            || account.executable != expected.executable
            || account.data != expected_data
            || sha256_hex(&account.data) != expected.data_sha256
        {
            return Err(refusal(format!(
                "Direct finalized account {} differed from its semantic-owner poststate",
                expected.address
            )));
        }
        output.push(observed_poststate_v1(account));
    }
    Ok(output)
}

fn observed_poststate_v1(account: &ObservedAccount) -> DirectTradeObservedPoststateV1 {
    DirectTradeObservedPoststateV1 {
        address: account.key.to_string(),
        owner: account.owner.to_string(),
        lamports: account.lamports,
        executable: account.executable,
        data_len: account.data.len(),
        data_sha256: sha256_hex(&account.data),
    }
}

fn publish_direct_evidence_v1(
    validated: &ValidatedManifestV1,
    journal: &DirectTradeJournalV1,
    path: &Path,
) -> Result<()> {
    if validated.public.buyer.maker != validated.buyer.maker.to_string()
        || !journal
            .finalized_poststates
            .iter()
            .any(|state| state.address == validated.public.route.claims.buyer_position)
    {
        return Err(refusal(
            "Direct evidence buyer owner or buyer Position poststate was not the signed buyer",
        ));
    }
    let signature = journal
        .expected_signature
        .clone()
        .ok_or_else(|| refusal("Direct finalized Hot journal omitted signature"))?;
    let hot_ack_base64 = journal
        .return_data_base64
        .clone()
        .ok_or_else(|| refusal("Direct finalized Hot journal omitted ACK"))?;
    let hot_ack = BASE64
        .decode(&hot_ack_base64)
        .map_err(|error| Error::new(format!("Direct evidence ACK base64: {error}")))?;
    let (static_account_count, loaded_address_count) = direct_hot_message_geometry_v1(journal)?;
    let wire_bytes = journal
        .expected_wire_bytes
        .ok_or_else(|| refusal("Direct finalized Hot omitted wire width"))?;
    if static_account_count != 4
        || loaded_address_count != 57
        || wire_bytes != 1_167
        || journal.unique_message_account_count != Some(61)
    {
        return Err(refusal(
            "Direct evidence did not preserve static4+loaded57=61 and 1,167-byte geometry",
        ));
    }
    let capability_seal_sha256 = finalized_capability_seal_digest_v1(validated)?;
    let (mutations, lookup_activation) = direct_finalized_mutations_v1(validated)?;
    let positions = [
        direct_position_transition_v1(
            journal,
            &validated.public.route.claims.seller_position,
            &validated.public.seller.maker,
        )?,
        direct_position_transition_v1(
            journal,
            &validated.public.route.claims.buyer_position,
            &validated.public.buyer.maker,
        )?,
    ];
    let claim_balances = direct_claim_balances_v1(
        &positions,
        &validated.public.route.custody.seller_token,
        &validated.public.route.custody.buyer_token,
    )?;
    let mut evidence = DirectTradeFinalizedEvidenceV1 {
        schema: direct_evidence_schema_v1(&validated.public.cluster)?.into(),
        status: "finalized".into(),
        cluster: validated.public.cluster.clone(),
        public_manifest_sha256: validated.public_sha256.clone(),
        public_manifest_base64: BASE64.encode(fs::read(&validated.private.public_manifest)?),
        private_session_sha256: validated.private_sha256.clone(),
        journal_state_sha256: journal.state_sha256.clone(),
        hot_journal_base64: BASE64.encode(serde_json::to_vec(journal)?),
        signature,
        finalized_slot: journal
            .finalized_slot
            .ok_or_else(|| refusal("Direct finalized Hot journal omitted slot"))?,
        market: validated.public.market.clone(),
        seller_owner: validated.public.seller.maker.clone(),
        seller_position: validated.public.route.claims.seller_position.clone(),
        buyer_position: validated.public.route.claims.buyer_position.clone(),
        buyer_owner: validated.public.buyer.maker.clone(),
        buyer_collateral_source: validated.public.route.custody.buyer_token.clone(),
        seller_collateral_destination: validated.public.route.custody.seller_token.clone(),
        fee_token_account: validated.public.route.custody.fee_token.clone(),
        fee_basis_points_per_side: validated.public.fee_basis_points,
        fee_recipient: validated.public.fee_recipient.clone(),
        mint: validated.public.route.custody.mint.clone(),
        outcome_index: u32::from(validated.seller.intent.outcome),
        outcome_count: validated.public.context.outcome_count,
        fill_atoms: validated.public.fill,
        execution_price: validated.public.execution_price,
        price_scale: 1_000_000,
        lookup_table: journal.lookup_table.clone(),
        lookup_addresses_sha256: journal.lookup_addresses_sha256.clone(),
        lookup_address_count: journal.lookup_addresses.len(),
        static_account_count,
        loaded_address_count,
        unique_message_account_count: journal
            .unique_message_account_count
            .ok_or_else(|| refusal("Direct finalized Hot omitted account-lock census"))?,
        wire_bytes,
        capability_seal: validated.public.route.fixed.capability_seal.clone(),
        capability_seal_sha256,
        hot_ack_producer: journal
            .return_data_producer
            .clone()
            .ok_or_else(|| refusal("Direct finalized Hot omitted ACK producer"))?,
        hot_ack_base64,
        hot_ack_sha256: sha256_hex(&hot_ack),
        mutations,
        lookup_activation,
        positions,
        claim_balances,
        final_accounts: journal.expected_poststates.clone(),
        poststates: journal.finalized_poststates.clone(),
        evidence_sha256: String::new(),
    };
    authenticate_direct_claim_schedule_v1(&evidence)?;
    evidence.evidence_sha256 = direct_evidence_digest_v1(&evidence)?;
    write_create_only_json_v1(path, &evidence)
}

fn finalized_capability_seal_digest_v1(validated: &ValidatedManifestV1) -> Result<String> {
    let entries = journal_entries_v1(validated)?;
    for entry in entries.into_iter().rev() {
        let bytes = fs::read(entry.path())?;
        require_unique_json_v1(&bytes, "Direct seal journal")?;
        let journal: DirectTradeJournalV1 = serde_json::from_slice(&bytes)?;
        if journal.stage == DirectTradeStageV1::CapabilitySeal
            && journal.phase == DirectTradeJournalPhaseV1::Finalized
        {
            let expected = journal
                .expected_poststates
                .first()
                .ok_or_else(|| refusal("Direct seal journal omitted expected poststate"))?;
            if expected.address != validated.public.route.fixed.capability_seal
                || journal.finalized_poststates.first().is_none_or(|observed| {
                    observed.address != expected.address
                        || observed.data_sha256 != expected.data_sha256
                })
            {
                return Err(refusal("Direct seal journal and finalized seal differed"));
            }
            return Ok(expected.data_sha256.clone());
        }
    }
    Err(refusal(
        "Direct finalized Hot evidence omitted its seal journal",
    ))
}

fn recover_finalized_hot_evidence_v1(
    rpc: &mut Rpc,
    validated: &ValidatedManifestV1,
    path: &Path,
) -> Result<Option<DirectTradeFinalizedEvidenceV1>> {
    let entries = journal_entries_v1(validated)?;
    let Some(last) = entries.last() else {
        return Ok(None);
    };
    let bytes = fs::read(last.path())?;
    require_unique_json_v1(&bytes, "Direct terminal journal")?;
    let journal: DirectTradeJournalV1 = serde_json::from_slice(&bytes)?;
    if journal.stage != DirectTradeStageV1::Hot
        || journal.phase != DirectTradeJournalPhaseV1::Finalized
    {
        return Ok(None);
    }
    if journal.schema != direct_journal_schema_v1(&validated.public.cluster)?
        || journal.public_manifest_sha256 != validated.public_sha256
        || journal.private_session_sha256 != validated.private_sha256
        || journal.intent_sha256 != journal_intent_sha256_v1(&journal)?
        || journal.state_sha256 != journal_state_sha256(&journal)?
    {
        return Err(refusal("Direct terminal journal identity changed"));
    }
    authenticate_finalized_direct_history_v1(rpc, &journal)?;
    publish_direct_evidence_v1(validated, &journal, path)?;
    let evidence = authenticate_persisted_direct_evidence_v1(rpc, validated, path)?;
    Ok(Some(evidence))
}

fn direct_hot_message_geometry_v1(journal: &DirectTradeJournalV1) -> Result<(usize, usize)> {
    let bytes = BASE64
        .decode(
            journal
                .message_base64
                .as_deref()
                .ok_or_else(|| refusal("Direct Hot evidence omitted message"))?,
        )
        .map_err(|error| Error::new(format!("Direct Hot evidence message base64: {error}")))?;
    let message: VersionedMessage = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("Direct Hot evidence message: {error}")))?;
    let VersionedMessage::V0(message) = message else {
        return Err(refusal("Direct Hot evidence message was not v0"));
    };
    let loaded = message
        .address_table_lookups
        .iter()
        .try_fold(0_usize, |count, lookup| {
            count
                .checked_add(lookup.writable_indexes.len())
                .and_then(|count| count.checked_add(lookup.readonly_indexes.len()))
        })
        .ok_or_else(|| refusal("Direct Hot loaded-address count overflow"))?;
    Ok((message.account_keys.len(), loaded))
}

fn direct_evidence_digest_v1(evidence: &DirectTradeFinalizedEvidenceV1) -> Result<String> {
    let mut projected = evidence.clone();
    projected.evidence_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&projected)?))
}

fn write_create_only_json_v1(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| refusal("Direct evidence omitted parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("Direct evidence omitted UTF-8 name"))?;
    let temporary = parent.join(format!(
        ".{name}.direct-evidence-{}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    fs::hard_link(&temporary, path)
        .map_err(|error| Error::new(format!("publish Direct evidence: {error}")))?;
    fs::remove_file(temporary)?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn authenticate_persisted_direct_evidence_v1(
    rpc: &mut Rpc,
    validated: &ValidatedManifestV1,
    path: &Path,
) -> Result<DirectTradeFinalizedEvidenceV1> {
    let bytes = fs::read(path)?;
    require_unique_json_v1(&bytes, "Direct finalized evidence")?;
    let evidence: DirectTradeFinalizedEvidenceV1 = serde_json::from_slice(&bytes)?;
    if evidence.schema != direct_evidence_schema_v1(&validated.public.cluster)?
        || evidence.status != "finalized"
        || evidence.public_manifest_sha256 != validated.public_sha256
        || evidence.private_session_sha256 != validated.private_sha256
        || evidence.evidence_sha256 != direct_evidence_digest_v1(&evidence)?
        || evidence.fee_basis_points_per_side != 50
        || evidence.static_account_count != 4
        || evidence.loaded_address_count != 57
        || evidence.unique_message_account_count != 61
        || evidence.lookup_address_count != 57
        || evidence.wire_bytes != 1_167
        || evidence.poststates.len() != 10
        || evidence.final_accounts.len() != 10
        || evidence.positions[0].account != validated.public.route.claims.seller_position
        || evidence.positions[1].account != validated.public.route.claims.buyer_position
        || evidence.seller_owner != validated.public.seller.maker
        || evidence.buyer_owner != validated.public.buyer.maker
        || evidence.mint != validated.public.route.custody.mint
        || evidence.outcome_index != u32::from(validated.seller.intent.outcome)
        || evidence.outcome_count != validated.public.context.outcome_count
        || evidence.fill_atoms != validated.public.fill
        || evidence.execution_price != validated.public.execution_price
        || evidence.price_scale != 1_000_000
    {
        return Err(refusal(
            "Direct finalized evidence identity or digest changed",
        ));
    }
    authenticate_direct_claim_schedule_v1(&evidence)?;
    let entries = journal_entries_v1(validated)?;
    let last = entries
        .last()
        .ok_or_else(|| refusal("Direct evidence has no Hot journal"))?;
    let journal_bytes = fs::read(last.path())?;
    require_unique_json_v1(&journal_bytes, "Direct Hot journal")?;
    let journal: DirectTradeJournalV1 = serde_json::from_slice(&journal_bytes)?;
    if journal.stage != DirectTradeStageV1::Hot
        || journal.phase != DirectTradeJournalPhaseV1::Finalized
        || journal.state_sha256 != evidence.journal_state_sha256
        || journal.expected_signature.as_deref() != Some(evidence.signature.as_str())
        || journal.return_data_base64.as_deref() != Some(evidence.hot_ack_base64.as_str())
        || journal.expected_poststates != evidence.final_accounts
        || journal.finalized_poststates != evidence.poststates
    {
        return Err(refusal("Direct evidence and Hot journal differed"));
    }
    let (mutations, lookup_activation) = direct_finalized_mutations_v1(validated)?;
    if evidence.mutations != mutations || evidence.lookup_activation != lookup_activation {
        return Err(refusal(
            "Direct evidence mutation journals or lookup activation changed",
        ));
    }
    authenticate_finalized_direct_history_v1(rpc, &journal)?;
    Ok(evidence)
}

/// Independently authenticate one owned-loopback Direct result from the sole
/// evidence file handed to a downstream lifecycle. The producer's private
/// session and journal directory are deliberately not inputs: the evidence
/// embeds the exact public signed-intent manifest and finalized Hot journal,
/// and this consumer reopens chain history and all ten live poststates.
pub(crate) fn authenticate_owned_loopback_finalized_evidence_v1(
    rpc: &mut Rpc,
    path: &Path,
    expected_market: Pubkey,
    expected_plan_sha256: &str,
    expected_market_input_sha256: &str,
) -> Result<AuthenticatedDirectTradeEvidenceV1> {
    let path = absolute_existing_file(path, "owned-loopback Direct finalized evidence")?;
    let evidence_bytes = fs::read(path)?;
    require_unique_json_v1(&evidence_bytes, "owned-loopback Direct finalized evidence")?;
    let evidence: DirectTradeFinalizedEvidenceV1 = serde_json::from_slice(&evidence_bytes)?;
    authenticate_embedded_direct_evidence_identity_v1(&evidence, ExpectedClusterV1::OwnedLoopback)?;

    let public_bytes = decode_canonical_base64_v1(
        &evidence.public_manifest_base64,
        "embedded Direct public manifest",
    )?;
    require_unique_json_v1(&public_bytes, "embedded Direct public manifest")?;
    let public: DirectTradePublicManifestV1 = serde_json::from_slice(&public_bytes)?;
    if sha256_hex(&public_bytes) != evidence.public_manifest_sha256
        || public.schema != direct_public_schema_v1(ExpectedClusterV1::OwnedLoopback)
        || public.cluster != ExpectedClusterV1::OwnedLoopback.evidence_label()
        || public.market != expected_market.to_string()
        || public.plan_sha256 != expected_plan_sha256
        || public.market_input_sha256 != expected_market_input_sha256
    {
        return Err(refusal(
            "embedded Direct manifest did not match the expected owned-loopback Market and founding inputs",
        ));
    }
    hex32(&public.plan_sha256, "Direct plan digest")?;
    hex32(&public.market_input_sha256, "Direct market-input digest")?;
    let observed_genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| refusal("owned-loopback Direct genesis was not a string"))?
        .to_owned();
    if public.genesis_hash != observed_genesis {
        return Err(refusal(
            "embedded Direct manifest named another owned-loopback genesis",
        ));
    }
    let seller = decode_signed_intent(&public.seller, 0, &public)?;
    let buyer = decode_signed_intent(&public.buyer, 1, &public)?;
    validate_public_facts(&public, seller, buyer)?;
    if seller.maker == buyer.maker
        || evidence.market != public.market
        || evidence.seller_owner != seller.maker.to_string()
        || evidence.seller_position != public.route.claims.seller_position
        || evidence.buyer_position != public.route.claims.buyer_position
        || evidence.buyer_owner != buyer.maker.to_string()
        || evidence.buyer_collateral_source != public.route.custody.buyer_token
        || public.route.custody.buyer_token
            != Pubkey::new_from_array(buyer.intent.collateral_account).to_string()
        || evidence.seller_collateral_destination != public.route.custody.seller_token
        || evidence.fee_token_account != public.route.custody.fee_token
        || evidence.fee_recipient != public.fee_recipient
        || evidence.mint != public.route.custody.mint
        || evidence.outcome_index != u32::from(seller.intent.outcome)
        || evidence.outcome_count != public.context.outcome_count
        || evidence.fill_atoms != public.fill
        || evidence.execution_price != public.execution_price
        || evidence.capability_seal != public.route.fixed.capability_seal
    {
        return Err(refusal(
            "embedded Direct manifest did not own the evidence participant or collateral coordinates",
        ));
    }

    let journal_bytes =
        decode_canonical_base64_v1(&evidence.hot_journal_base64, "embedded Direct Hot journal")?;
    require_unique_json_v1(&journal_bytes, "embedded Direct Hot journal")?;
    let journal: DirectTradeJournalV1 = serde_json::from_slice(&journal_bytes)?;
    authenticate_embedded_hot_journal_v1(&public, &evidence, &journal)?;
    authenticate_embedded_direct_mutations_v1(rpc, &public, &evidence, &journal)?;

    let route_keys = route_keys(&public.route)?;
    let snapshot = finalized_snapshot(rpc, &route_keys)?;
    let named_route = build_named_route(&snapshot, &public.route)?;
    let physical =
        assemble_direct_inline_ordinary_route_v3(named_route, public.context.outcome_count)
            .map_err(|error| Error::new(format!("embedded Direct named route: {error:?}")))?;
    let physical = project_direct_inline_sealed_execution_physical_v3(&physical)
        .map_err(|error| Error::new(format!("embedded Direct sealed route: {error:?}")))?;

    let lookup_key = parse_key(&journal.lookup_table, "embedded Direct lookup table")?;
    let lookup_account = finalized_observed_accounts_v1(
        rpc,
        core::slice::from_ref(&lookup_key),
        evidence.finalized_slot,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| refusal("embedded Direct lookup table disappeared"))?;
    let table = authenticate_embedded_lookup_table_v1(&journal, &lookup_account)?;
    authenticate_embedded_hot_message_v1(&public, seller, buyer, &journal, &physical, &table)?;
    authenticate_finalized_direct_history_v1(rpc, &journal)?;

    let expected_keys = journal
        .expected_poststates
        .iter()
        .map(|expected| parse_key(&expected.address, "embedded Direct poststate"))
        .collect::<Result<Vec<_>>>()?;
    let accounts = finalized_observed_accounts_v1(rpc, &expected_keys, evidence.finalized_slot)?;
    let observed = verify_expected_direct_poststates_v1(&accounts, &journal.expected_poststates)?;
    if observed != journal.finalized_poststates || observed != evidence.poststates {
        return Err(refusal(
            "embedded Direct evidence did not match all ten live finalized poststates",
        ));
    }
    authenticate_embedded_buyer_poststates_v1(&public, buyer, &accounts)?;
    let seal = snapshot.account(parse_key(
        &public.route.fixed.capability_seal,
        "embedded Direct capability seal",
    )?)?;
    if sha256_hex(&seal.data) != evidence.capability_seal_sha256
        || seal.owner
            != parse_key(
                &public.route.fixed.trading_program,
                "embedded Direct Trading program",
            )?
    {
        return Err(refusal(
            "embedded Direct capability seal bytes or owner changed",
        ));
    }
    Ok(AuthenticatedDirectTradeEvidenceV1 {
        market: expected_market,
        seller_owner: seller.maker,
        seller_position: parse_key(
            &public.route.claims.seller_position,
            "Direct seller Position",
        )?,
        seller_collateral_destination: parse_key(
            &public.route.custody.seller_token,
            "Direct seller collateral destination",
        )?,
        buyer_owner: buyer.maker,
        buyer_position: parse_key(&public.route.claims.buyer_position, "Direct buyer Position")?,
        buyer_collateral_source: parse_key(
            &public.route.custody.buyer_token,
            "Direct buyer collateral source",
        )?,
        fee_recipient: parse_key(&public.fee_recipient, "Direct fee recipient")?,
        fee_token_account: parse_key(&public.route.custody.fee_token, "Direct fee token account")?,
        mint: parse_key(&public.route.custody.mint, "Direct collateral Mint")?,
        outcome_index: evidence.outcome_index,
        outcome_count: evidence.outcome_count,
        mutations: evidence.mutations.clone(),
        positions: evidence.positions.clone(),
        claim_balances: evidence.claim_balances.clone(),
        final_accounts: evidence.final_accounts.clone(),
        finalized_slot: evidence.finalized_slot,
        evidence_sha256: evidence.evidence_sha256,
    })
}

fn authenticate_embedded_direct_evidence_identity_v1(
    evidence: &DirectTradeFinalizedEvidenceV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    if evidence.schema != direct_evidence_schema_v1(expected_cluster.evidence_label())?
        || evidence.status != "finalized"
        || evidence.cluster != expected_cluster.evidence_label()
        || evidence.evidence_sha256 != direct_evidence_digest_v1(evidence)?
        || evidence.fee_basis_points_per_side != 50
        || evidence.static_account_count != 4
        || evidence.loaded_address_count != 57
        || evidence.unique_message_account_count != 61
        || evidence.lookup_address_count != 57
        || evidence.wire_bytes != 1_167
        || evidence.poststates.len() != 10
        || evidence.final_accounts.len() != 10
        || evidence.price_scale != 1_000_000
        || evidence.fill_atoms == 0
        || evidence.outcome_count == 0
        || evidence.outcome_index >= evidence.outcome_count
        || evidence
            .mutations
            .first()
            .is_none_or(|row| row.kind != "replay-setup")
        || evidence
            .mutations
            .get(1)
            .is_none_or(|row| row.kind != "token-setup")
        || evidence
            .mutations
            .last()
            .is_none_or(|row| row.kind != "hot")
        || evidence.positions[0].account != evidence.seller_position
        || evidence.positions[0].owner != evidence.seller_owner
        || evidence.positions[1].account != evidence.buyer_position
        || evidence.positions[1].owner != evidence.buyer_owner
    {
        return Err(refusal(
            "embedded Direct finalized evidence identity, geometry, or digest changed",
        ));
    }
    authenticate_direct_claim_schedule_v1(evidence)?;
    for row in &evidence.mutations {
        expected_cluster
            .authenticate_finalized_fee(row.fee_lamports, "Direct mutation evidence")?;
    }
    Ok(())
}

fn decode_canonical_base64_v1(value: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label} base64: {error}")))?;
    if BASE64.encode(&bytes) != value {
        return Err(refusal(format!("{label} base64 was noncanonical")));
    }
    Ok(bytes)
}

fn authenticate_embedded_hot_journal_v1(
    public: &DirectTradePublicManifestV1,
    evidence: &DirectTradeFinalizedEvidenceV1,
    journal: &DirectTradeJournalV1,
) -> Result<()> {
    if journal.schema != direct_journal_schema_v1(&public.cluster)?
        || journal.public_manifest_sha256 != evidence.public_manifest_sha256
        || journal.private_session_sha256 != evidence.private_session_sha256
        || journal.stage != DirectTradeStageV1::Hot
        || journal.phase != DirectTradeJournalPhaseV1::Finalized
        || journal.intent_sha256 != journal_intent_sha256_v1(journal)?
        || journal.state_sha256 != journal_state_sha256(journal)?
        || journal.state_sha256 != evidence.journal_state_sha256
        || journal.finalized_slot != Some(evidence.finalized_slot)
        || journal.expected_signature.as_deref() != Some(evidence.signature.as_str())
        || journal.return_data_producer.as_deref() != Some(evidence.hot_ack_producer.as_str())
        || journal.return_data_base64.as_deref() != Some(evidence.hot_ack_base64.as_str())
        || journal.return_data_was_null != Some(false)
        || journal.expected_poststates != evidence.final_accounts
        || journal.finalized_poststates != evidence.poststates
        || journal.lookup_table != evidence.lookup_table
        || journal.lookup_addresses_sha256 != evidence.lookup_addresses_sha256
        || journal.lookup_addresses.len() != 57
        || journal.unique_message_account_count != Some(61)
        || journal.expected_wire_bytes != Some(1_167)
        || journal.expected_prestates.len() != 10
        || journal.expected_poststates.len() != 10
        || journal.finalized_poststates.len() != 10
    {
        return Err(refusal(
            "embedded Direct Hot journal identity or finalized shape changed",
        ));
    }
    let trading = parse_key(
        &public.route.fixed.trading_program,
        "Direct Trading program",
    )?;
    if journal.expected_return_data_producer.as_deref() != Some(trading.to_string().as_str())
        || journal.return_data_producer != journal.expected_return_data_producer
        || journal.expected_return_data_base64 != journal.return_data_base64
    {
        return Err(refusal(
            "embedded Direct Hot journal did not require the exact Trading ACK producer and bytes",
        ));
    }
    let expected_addresses = [
        public.route.fixed.root.as_str(),
        public.route.seller_maker.as_str(),
        public.route.buyer_maker.as_str(),
        public.route.claims.aggregate.as_str(),
        public.route.claims.seller_position.as_str(),
        public.route.claims.buyer_position.as_str(),
        public.route.custody.replay.as_str(),
        public.route.custody.buyer_token.as_str(),
        public.route.custody.seller_token.as_str(),
        public.route.custody.fee_token.as_str(),
    ];
    if journal
        .expected_prestates
        .iter()
        .map(|prestate| prestate.address.as_str())
        .ne(expected_addresses)
        || journal
            .expected_poststates
            .iter()
            .map(|poststate| poststate.address.as_str())
            .ne(expected_addresses)
        || journal
            .finalized_poststates
            .iter()
            .map(|poststate| poststate.address.as_str())
            .ne(expected_addresses)
    {
        return Err(refusal(
            "embedded Direct Hot journal changed the canonical ten-role poststate order",
        ));
    }
    let positions = [
        direct_position_transition_v1(
            journal,
            &public.route.claims.seller_position,
            &public.seller.maker,
        )?,
        direct_position_transition_v1(
            journal,
            &public.route.claims.buyer_position,
            &public.buyer.maker,
        )?,
    ];
    let claim_balances = direct_claim_balances_v1(
        &positions,
        &public.route.custody.seller_token,
        &public.route.custody.buyer_token,
    )?;
    if evidence.positions != positions || evidence.claim_balances != claim_balances {
        return Err(refusal(
            "embedded Direct Position transitions or claim schedule changed",
        ));
    }
    for (expected, observed) in journal
        .expected_poststates
        .iter()
        .zip(&journal.finalized_poststates)
    {
        let data = decode_canonical_base64_v1(
            &expected.data_base64,
            "embedded Direct expected poststate",
        )?;
        if sha256_hex(&data) != expected.data_sha256
            || observed.address != expected.address
            || observed.owner != expected.owner
            || observed.lamports != expected.lamports
            || observed.executable != expected.executable
            || observed.data_len != data.len()
            || observed.data_sha256 != expected.data_sha256
        {
            return Err(refusal(
                "embedded Direct Hot journal changed a complete expected poststate",
            ));
        }
    }
    let lookup_keys = journal
        .lookup_addresses
        .iter()
        .map(|value| parse_key(value, "embedded Direct lookup address"))
        .collect::<Result<Vec<_>>>()?;
    if lookup_keys.iter().any(|key| *key == Pubkey::default())
        || lookup_keys
            .iter()
            .enumerate()
            .any(|(index, key)| lookup_keys.iter().take(index).any(|earlier| earlier == key))
        || pubkey_list_sha256(&lookup_keys) != journal.lookup_addresses_sha256
    {
        return Err(refusal(
            "embedded Direct Hot journal lookup closure was not exact, ordered, and distinct",
        ));
    }
    let message = decode_canonical_base64_v1(
        journal
            .message_base64
            .as_deref()
            .ok_or_else(|| refusal("embedded Direct Hot journal omitted message"))?,
        "embedded Direct Hot message",
    )?;
    if journal.message_sha256.as_deref() != Some(sha256_hex(&message).as_str())
        || journal.last_valid_block_height.is_none()
        || journal.exact_fee_lamports.is_none()
        || journal.fee_lamports != journal.exact_fee_lamports
        || journal.transaction_sha256.is_none()
    {
        return Err(refusal(
            "embedded Direct Hot journal omitted its exact message, fee, or transaction digest",
        ));
    }
    authenticate_signed_journal_v1(journal, &message)?;
    let packet = decode_canonical_base64_v1(
        journal
            .signed_packet_base64
            .as_deref()
            .ok_or_else(|| refusal("embedded Direct Hot journal omitted signed packet"))?,
        "embedded Direct Hot packet",
    )?;
    if packet.len() != 1_167
        || journal.transaction_sha256.as_deref() != Some(sha256_hex(&packet).as_str())
        || direct_hot_message_geometry_v1(journal)? != (4, 57)
    {
        return Err(refusal(
            "embedded Direct Hot packet changed its exact 4+57=61 / 1,167-byte geometry",
        ));
    }
    Ok(())
}

fn authenticate_embedded_direct_mutations_v1(
    rpc: &mut Rpc,
    public: &DirectTradePublicManifestV1,
    evidence: &DirectTradeFinalizedEvidenceV1,
    hot: &DirectTradeJournalV1,
) -> Result<()> {
    let extension_count = hot
        .lookup_addresses
        .len()
        .checked_add(19)
        .ok_or_else(|| refusal("embedded Direct extension count overflowed"))?
        / 20;
    let expected_mutation_count = extension_count
        .checked_add(6)
        .ok_or_else(|| refusal("embedded Direct mutation count overflowed"))?;
    if evidence.mutations.len() != expected_mutation_count {
        return Err(refusal(
            "embedded Direct evidence did not own the exact setup/ALT/seal/Hot mutation count",
        ));
    }
    let mut previous_slot = 0_u64;
    for (index, row) in evidence.mutations.iter().enumerate() {
        if evidence
            .mutations
            .iter()
            .take(index)
            .any(|earlier| earlier.path == row.path || earlier.signature == row.signature)
            || row.path == evidence.lookup_activation.path
            || row.slot < previous_slot
        {
            return Err(refusal(
                "embedded Direct mutation reused a path/signature or reversed finalized slot order",
            ));
        }
        previous_slot = row.slot;
    }

    let binding = DirectSetupManifestBindingV1::new(
        evidence.public_manifest_sha256.clone(),
        evidence.private_session_sha256.clone(),
    )?;
    let replay = load_embedded_setup_mutation_v1(&evidence.mutations[0], "replay-setup")?;
    let token = load_embedded_setup_mutation_v1(&evidence.mutations[1], "token-setup")?;
    authenticate_direct_setup_chain_v1(&binding, &replay, &token)?;
    authenticate_embedded_setup_mutation_v1(
        rpc,
        public,
        &evidence.mutations[0],
        &replay,
        DirectSetupStageV1::ReplaySetup,
    )?;
    authenticate_embedded_setup_mutation_v1(
        rpc,
        public,
        &evidence.mutations[1],
        &token,
        DirectSetupStageV1::TokenSetup,
    )?;

    let action_rows = &evidence.mutations[2..];
    let mut creation_slot = None;
    for (ordinal, row) in action_rows.iter().enumerate() {
        let (expected_kind, expected_stage, expected_action_index, expected_prefix) =
            if ordinal == 0 {
                ("lookup-create", DirectTradeStageV1::LookupCreate, 0, None)
            } else if ordinal <= extension_count {
                (
                    "lookup-extend",
                    DirectTradeStageV1::LookupExtend,
                    ordinal,
                    Some(ordinal.saturating_mul(20).min(hot.lookup_addresses.len())),
                )
            } else if ordinal == extension_count + 1 {
                (
                    "lookup-freeze",
                    DirectTradeStageV1::LookupFreeze,
                    extension_count + 1,
                    None,
                )
            } else if ordinal == extension_count + 2 {
                (
                    "capability-seal",
                    DirectTradeStageV1::CapabilitySeal,
                    extension_count + 3,
                    None,
                )
            } else if ordinal == extension_count + 3 {
                ("hot", DirectTradeStageV1::Hot, extension_count + 4, None)
            } else {
                return Err(refusal("embedded Direct mutation order was not total"));
            };
        let journal = load_embedded_action_mutation_v1(row)?;
        if row.kind != expected_kind
            || row.prefix_len != expected_prefix
            || journal.stage != expected_stage
            || usize::from(journal.action_index) != expected_action_index
            || journal.phase != DirectTradeJournalPhaseV1::Finalized
            || journal.schema != direct_journal_schema_v1(&public.cluster)?
            || journal.public_manifest_sha256 != evidence.public_manifest_sha256
            || journal.private_session_sha256 != evidence.private_session_sha256
            || journal.lookup_table != hot.lookup_table
            || journal.lookup_addresses != hot.lookup_addresses
            || journal.lookup_addresses_sha256 != hot.lookup_addresses_sha256
            || journal.intent_sha256 != journal_intent_sha256_v1(&journal)?
            || journal.state_sha256 != journal_state_sha256(&journal)?
            || row.intent_sha256 != journal.intent_sha256
            || row.schema != journal.schema
            || row.completion_pointer != "/phase"
            || row.completion_value != "finalized"
            || journal.expected_signature.as_deref() != Some(row.signature.as_str())
            || journal.finalized_slot != Some(row.slot)
            || row.fee_payer != public.payer
            || journal.fee_lamports != Some(row.fee_lamports)
            || journal.compute_units_consumed != Some(row.compute_units_consumed)
            || journal.fee_lamports != journal.exact_fee_lamports
            || creation_slot.is_some_and(|slot| slot != journal.lookup_creation_slot)
        {
            return Err(refusal(
                "embedded Direct mutation journal identity, order, intent, fee, or completion changed",
            ));
        }
        creation_slot.get_or_insert(journal.lookup_creation_slot);
        let message = decode_canonical_base64_v1(
            journal
                .message_base64
                .as_deref()
                .ok_or_else(|| refusal("embedded Direct mutation omitted message"))?,
            "embedded Direct mutation message",
        )?;
        if journal.message_sha256.as_deref() != Some(sha256_hex(&message).as_str()) {
            return Err(refusal("embedded Direct mutation message digest changed"));
        }
        authenticate_signed_journal_v1(&journal, &message)?;
        authenticate_finalized_direct_history_v1(rpc, &journal)?;
        if expected_stage == DirectTradeStageV1::Hot && &journal != hot {
            return Err(refusal(
                "embedded Direct Hot mutation differed from the terminal Hot journal",
            ));
        }
    }

    let activation = load_embedded_activation_observation_v1(&evidence.lookup_activation)?;
    let expected_activation_index = extension_count
        .checked_add(2)
        .ok_or_else(|| refusal("embedded Direct activation index overflowed"))?;
    let freeze_slot = evidence
        .mutations
        .get(extension_count + 2)
        .ok_or_else(|| refusal("embedded Direct lookup freeze mutation disappeared"))?
        .slot;
    let seal_slot = evidence
        .mutations
        .get(extension_count + 3)
        .ok_or_else(|| refusal("embedded Direct capability-seal mutation disappeared"))?
        .slot;
    authenticate_lookup_activation_slot_order_v1(
        activation
            .finalized_slot
            .ok_or_else(|| refusal("embedded Direct lookup activation omitted finalized slot"))?,
        activation.lookup_creation_slot,
        freeze_slot,
        seal_slot,
    )?;
    if activation.stage != DirectTradeStageV1::LookupActivation
        || activation.phase != DirectTradeJournalPhaseV1::Finalized
        || usize::from(activation.action_index) != expected_activation_index
        || activation.schema != direct_journal_schema_v1(&public.cluster)?
        || activation.public_manifest_sha256 != evidence.public_manifest_sha256
        || activation.private_session_sha256 != evidence.private_session_sha256
        || activation.lookup_table != hot.lookup_table
        || activation.lookup_addresses != hot.lookup_addresses
        || activation.lookup_addresses_sha256 != hot.lookup_addresses_sha256
        || activation.lookup_creation_slot != creation_slot.unwrap_or_default()
        || activation.intent_sha256 != journal_intent_sha256_v1(&activation)?
        || activation.state_sha256 != journal_state_sha256(&activation)?
        || activation.intent_sha256 != evidence.lookup_activation.intent_sha256
        || activation.finalized_slot != Some(evidence.lookup_activation.finalized_slot)
        || activation.message_base64.is_some()
        || activation.message_sha256.is_some()
        || activation.last_valid_block_height.is_some()
        || activation.exact_fee_lamports.is_some()
        || activation.expected_wire_bytes.is_some()
        || activation.unique_message_account_count.is_some()
        || activation.signed_packet_base64.is_some()
        || activation.expected_signature.is_some()
        || activation.expected_return_data_producer.is_some()
        || activation.expected_return_data_base64.is_some()
        || !activation.expected_prestates.is_empty()
        || !activation.expected_poststates.is_empty()
        || activation.transaction_sha256.is_some()
        || activation.fee_lamports.is_some()
        || activation.compute_units_consumed.is_some()
        || activation.return_data_producer.is_some()
        || activation.return_data_base64.is_some()
        || activation.return_data_was_null.is_some()
        || !activation.finalized_poststates.is_empty()
    {
        return Err(refusal(
            "embedded Direct lookup activation was not the exact signatureless later observation",
        ));
    }
    Ok(())
}

fn authenticate_lookup_activation_slot_order_v1(
    activation_slot: u64,
    creation_slot: u64,
    freeze_slot: u64,
    seal_slot: u64,
) -> Result<()> {
    if activation_slot <= creation_slot
        || activation_slot < freeze_slot
        || activation_slot > seal_slot
    {
        return Err(refusal(
            "embedded Direct lookup activation was outside the freeze-to-seal interval",
        ));
    }
    Ok(())
}

fn load_embedded_setup_mutation_v1(
    row: &DirectFinalizedMutationEvidenceV1,
    expected_kind: &str,
) -> Result<DirectSetupJournalV1> {
    let path = absolute_existing_file(Path::new(&row.path), "embedded Direct setup journal")?;
    let bytes = fs::read(path)?;
    require_unique_json_v1(&bytes, "embedded Direct setup journal")?;
    if row.kind != expected_kind
        || row.prefix_len.is_some()
        || row.sha256 != sha256_hex(&bytes)
        || row.schema != DIRECT_SETUP_JOURNAL_SCHEMA_V1
        || row.completion_pointer != "/phase"
        || row.completion_value != "finalized"
    {
        return Err(refusal("embedded Direct setup mutation descriptor changed"));
    }
    serde_json::from_slice(&bytes).map_err(Into::into)
}

fn load_embedded_action_mutation_v1(
    row: &DirectFinalizedMutationEvidenceV1,
) -> Result<DirectTradeJournalV1> {
    let path = absolute_existing_file(Path::new(&row.path), "embedded Direct mutation journal")?;
    let bytes = fs::read(path)?;
    require_unique_json_v1(&bytes, "embedded Direct mutation journal")?;
    if row.sha256 != sha256_hex(&bytes) {
        return Err(refusal("embedded Direct mutation file digest changed"));
    }
    serde_json::from_slice(&bytes).map_err(Into::into)
}

fn load_embedded_activation_observation_v1(
    row: &DirectLookupActivationEvidenceV1,
) -> Result<DirectTradeJournalV1> {
    let path = absolute_existing_file(
        Path::new(&row.path),
        "embedded Direct lookup activation journal",
    )?;
    let bytes = fs::read(path)?;
    require_unique_json_v1(&bytes, "embedded Direct lookup activation journal")?;
    if row.sha256 != sha256_hex(&bytes)
        || row.completion_pointer != "/phase"
        || row.completion_value != "finalized"
    {
        return Err(refusal(
            "embedded Direct lookup activation descriptor changed",
        ));
    }
    let journal: DirectTradeJournalV1 = serde_json::from_slice(&bytes)?;
    if row.schema != journal.schema
        || row.lookup_table != journal.lookup_table
        || row.lookup_addresses_sha256 != journal.lookup_addresses_sha256
    {
        return Err(refusal(
            "embedded Direct lookup activation coordinates changed",
        ));
    }
    Ok(journal)
}

fn authenticate_embedded_setup_mutation_v1(
    rpc: &mut Rpc,
    public: &DirectTradePublicManifestV1,
    row: &DirectFinalizedMutationEvidenceV1,
    journal: &DirectSetupJournalV1,
    expected_stage: DirectSetupStageV1,
) -> Result<()> {
    if journal.stage != expected_stage
        || journal.phase != DirectSetupJournalPhaseV1::Finalized
        || row.intent_sha256 != journal.message_sha256
        || journal.expected_signature.as_deref() != Some(row.signature.as_str())
        || journal.finalized_slot != Some(row.slot)
        || journal.expected_signer != row.fee_payer
        || row.fee_payer != public.payer
        || journal.fee_lamports != Some(row.fee_lamports)
        || journal.compute_units_consumed != Some(row.compute_units_consumed)
        || journal.fee_lamports != Some(journal.exact_fee_lamports)
        || journal.return_data != journal.expected_return_data
        || journal.finalized_poststates != journal.expected_poststates
    {
        return Err(refusal(
            "embedded Direct setup mutation signature, fee, CU, receipt, or poststate changed",
        ));
    }
    let message_bytes =
        decode_canonical_base64_v1(&journal.message_base64, "embedded Direct setup message")?;
    if journal.message_sha256 != sha256_hex(&message_bytes) {
        return Err(refusal("embedded Direct setup message digest changed"));
    }
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("embedded Direct setup message: {error}")))?;
    let VersionedMessage::Legacy(message) = message else {
        return Err(refusal("embedded Direct setup message was not legacy"));
    };
    let instruction = message
        .instructions
        .first()
        .filter(|_| message.instructions.len() == 1)
        .ok_or_else(|| refusal("embedded Direct setup message did not own one instruction"))?;
    let payer = message
        .account_keys
        .first()
        .ok_or_else(|| refusal("embedded Direct setup message omitted payer"))?;
    let program = message
        .account_keys
        .get(usize::from(instruction.program_id_index))
        .ok_or_else(|| refusal("embedded Direct setup program index was out of range"))?;
    let (request_base64, trading_program) = match expected_stage {
        DirectSetupStageV1::ReplaySetup => (
            public.replay_setup.request_base64.as_str(),
            public.route.fixed.trading_program.as_str(),
        ),
        DirectSetupStageV1::TokenSetup => (
            public.token_setup.request_base64.as_str(),
            public.token_setup.trading_program.as_str(),
        ),
    };
    let request = decode_canonical_base64_v1(request_base64, "embedded Direct setup request")?;
    if *payer != parse_key(&public.payer, "embedded Direct payer")?
        || *program != parse_key(trading_program, "embedded Direct Trading program")?
        || instruction.data != request
    {
        return Err(refusal(
            "embedded Direct setup message changed its payer, Trading program, or exact request",
        ));
    }
    authenticate_embedded_finalized_setup_history_v1(rpc, public, journal)
}

fn authenticate_embedded_finalized_setup_history_v1(
    rpc: &mut Rpc,
    public: &DirectTradePublicManifestV1,
    journal: &DirectSetupJournalV1,
) -> Result<()> {
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| refusal("embedded Direct setup omitted signature"))?;
    let transaction = finalized_direct_transaction_v1(rpc, signature)?
        .ok_or_else(|| refusal("embedded Direct setup finalization disappeared"))?;
    let finalized_slot = transaction.get("slot").and_then(Value::as_u64);
    let meta = transaction
        .get("meta")
        .ok_or_else(|| refusal("embedded Direct setup history omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(refusal("embedded Direct setup transaction failed"));
    }
    let tuple = transaction
        .get("transaction")
        .and_then(Value::as_array)
        .filter(|tuple| tuple.len() == 2 && tuple.get(1).and_then(Value::as_str) == Some("base64"))
        .ok_or_else(|| refusal("embedded Direct setup history was not exact base64"))?;
    let encoded = tuple
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("embedded Direct setup history omitted packet"))?;
    let packet = decode_canonical_base64_v1(encoded, "embedded Direct setup history packet")?;
    if journal.signed_packet_base64.as_deref() != Some(encoded)
        || journal.signed_packet_sha256.as_deref() != Some(sha256_hex(&packet).as_str())
        || journal.transaction_sha256.as_deref() != Some(sha256_hex(&packet).as_str())
    {
        return Err(refusal(
            "embedded Direct setup history differed byte-for-byte from its journal",
        ));
    }
    let wire_transaction: VersionedTransaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("embedded Direct setup packet: {error}")))?;
    let VersionedMessage::Legacy(message) = &wire_transaction.message else {
        return Err(refusal("embedded Direct setup packet unexpectedly used v0"));
    };
    let balances = |name: &str| -> Result<Vec<u64>> {
        meta.get(name)
            .and_then(Value::as_array)
            .ok_or_else(|| refusal(format!("embedded Direct setup {name} was not an array")))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| refusal(format!("embedded Direct setup {name} was not u64")))
            })
            .collect()
    };
    let pre = balances("preBalances")?;
    let post = balances("postBalances")?;
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("embedded Direct setup fee was not u64"))?;
    let pre_total = pre
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)));
    let post_total = post
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)));
    if pre.len() != message.account_keys.len()
        || post.len() != message.account_keys.len()
        || fee != journal.exact_fee_lamports
        || Some(fee) != journal.fee_lamports
        || pre_total.and_then(|before| post_total.and_then(|after| before.checked_sub(after)))
            != Some(u128::from(fee))
        || finalized_slot != journal.finalized_slot
        || meta.get("computeUnitsConsumed").and_then(Value::as_u64)
            != journal.compute_units_consumed
    {
        return Err(refusal(
            "embedded Direct setup slot, fee, CU, or conserved lamport history changed",
        ));
    }
    let at = |key: Pubkey| -> Result<(u64, u64)> {
        let index = message
            .account_keys
            .iter()
            .position(|candidate| *candidate == key)
            .ok_or_else(|| refusal(format!("embedded Direct setup history omitted {key}")))?;
        Ok((pre[index], post[index]))
    };
    let expected_return = journal
        .expected_return_data
        .as_ref()
        .ok_or_else(|| refusal("embedded Direct setup omitted expected receipt"))?;
    let (producer, body) = parse_direct_return_data_v1(meta)?;
    if producer.as_deref() != Some(expected_return.producer.as_str())
        || body.as_deref() != Some(expected_return.body_base64.as_str())
    {
        return Err(refusal(
            "embedded Direct setup finalized receipt producer or body changed",
        ));
    }
    let payer = parse_key(&public.payer, "embedded Direct setup payer")?;
    let (payer_before, payer_after) = at(payer)?;
    match journal.stage {
        DirectSetupStageV1::ReplaySetup => {
            let receipt =
                DirectReplaySetupReceiptV1::decode(&expected_return.body()?).map_err(|error| {
                    Error::new(format!("embedded Direct replay receipt: {error:?}"))
                })?;
            let replay = Pubkey::new_from_array(receipt.custody_replay);
            let refund = Pubkey::new_from_array(receipt.rent_refund);
            let (replay_before, replay_after) = at(replay)?;
            let (refund_before, refund_after) = at(refund)?;
            let poststate = journal
                .expected_poststates
                .first()
                .filter(|_| {
                    journal.expected_poststates.len() == 1
                        && journal.finalized_poststates.len() == 1
                })
                .ok_or_else(|| refusal("embedded Direct replay poststate width changed"))?;
            if receipt.request_digest
                != hex32(
                    &public.replay_setup.request_sha256,
                    "Direct replay request digest",
                )?
                || receipt.market != parse_key(&public.market, "Direct replay Market")?.to_bytes()
                || receipt.maker
                    != parse_key(&public.replay_setup.maker, "Direct replay maker")?.to_bytes()
                || receipt.maker_root
                    != parse_key(&public.replay_setup.maker_root, "Direct replay maker root")?
                        .to_bytes()
                || replay
                    != parse_key(&public.replay_setup.custody_replay, "Direct replay account")?
                || refund != parse_key(&public.replay_setup.rent_refund, "Direct replay refund")?
                || receipt.payer != payer.to_bytes()
                || payer_before.checked_sub(payer_after) != fee.checked_add(receipt.payer_top_up)
                || refund_after.checked_sub(refund_before) != Some(receipt.refunded_excess)
                || replay_before != receipt.observed_lamports
                || replay_after != receipt.exact_rent
                || poststate.address != replay.to_string()
                || poststate.lamports != receipt.exact_rent
                || hex32(&poststate.data_sha256, "Direct replay poststate digest")?
                    != receipt.custody_replay_digest
            {
                return Err(refusal(
                    "embedded Direct replay setup receipt or lamport normalization changed",
                ));
            }
        }
        DirectSetupStageV1::TokenSetup => {
            let receipt = DirectTokenSetupReceiptV1::decode(&expected_return.body()?)
                .map_err(|error| Error::new(format!("embedded Direct token receipt: {error:?}")))?;
            let seller = Pubkey::new_from_array(receipt.seller_token);
            let fee_token = Pubkey::new_from_array(receipt.fee_token);
            let refund = Pubkey::new_from_array(receipt.rent_refund);
            let (seller_before, seller_after) = at(seller)?;
            let (fee_before, fee_after) = at(fee_token)?;
            let (refund_before, refund_after) = at(refund)?;
            let total_top_up = receipt
                .seller_normalization
                .payer_top_up
                .checked_add(receipt.fee_normalization.payer_top_up)
                .ok_or_else(|| refusal("embedded Direct token top-up overflowed"))?;
            let total_refund = receipt
                .seller_normalization
                .refunded_excess
                .checked_add(receipt.fee_normalization.refunded_excess)
                .ok_or_else(|| refusal("embedded Direct token refund overflowed"))?;
            let expected = &journal.expected_poststates;
            if receipt.request_digest
                != hex32(
                    &public.token_setup.request_sha256,
                    "Direct token request digest",
                )?
                || receipt.market != parse_key(&public.market, "Direct token Market")?.to_bytes()
                || receipt.release_set
                    != hex32(&public.context.release_set, "Direct token release set")?
                || receipt.seller_position
                    != parse_key(
                        &public.route.claims.seller_position,
                        "Direct token seller Position",
                    )?
                    .to_bytes()
                || receipt.collateral_mint
                    != parse_key(&public.token_setup.mint, "Direct token Mint")?.to_bytes()
                || receipt.token_program
                    != parse_key(&public.token_setup.token_program, "Direct token program")?
                        .to_bytes()
                || receipt.seller_owner
                    != parse_key(&public.token_setup.seller_owner, "Direct token seller")?
                        .to_bytes()
                || receipt.fee_recipient
                    != parse_key(&public.token_setup.fee_recipient, "Direct fee recipient")?
                        .to_bytes()
                || seller != parse_key(&public.token_setup.seller_token, "Direct seller token")?
                || fee_token != parse_key(&public.token_setup.fee_token, "Direct fee token")?
                || refund != parse_key(&public.token_setup.rent_refund, "Direct token refund")?
                || receipt.payer != payer.to_bytes()
                || payer_before.checked_sub(payer_after) != fee.checked_add(total_top_up)
                || refund_after.checked_sub(refund_before) != Some(total_refund)
                || seller_before != receipt.seller_normalization.observed_lamports
                || seller_after != receipt.seller_normalization.exact_rent
                || fee_before != receipt.fee_normalization.observed_lamports
                || fee_after != receipt.fee_normalization.exact_rent
                || expected.len() != 4
                || journal.finalized_poststates.len() != 4
                || expected[0].address != seller.to_string()
                || hex32(
                    &expected[0].data_sha256,
                    "Direct seller token poststate digest",
                )? != receipt.seller_poststate_digest
                || expected[1].address != fee_token.to_string()
                || hex32(
                    &expected[1].data_sha256,
                    "Direct fee token poststate digest",
                )? != receipt.fee_poststate_digest
            {
                return Err(refusal(
                    "embedded Direct token setup receipt or lamport normalization changed",
                ));
            }
        }
    }
    Ok(())
}

fn authenticate_embedded_lookup_table_v1<'a>(
    journal: &DirectTradeJournalV1,
    account: &'a ObservedAccount,
) -> Result<AddressLookupTable<'a>> {
    if account.key != parse_key(&journal.lookup_table, "embedded Direct lookup table")?
        || account.owner != lookup_table_program::id()
        || account.executable
    {
        return Err(refusal(
            "embedded Direct lookup table key, owner, or executable bit changed",
        ));
    }
    let table = AddressLookupTable::deserialize(&account.data)
        .map_err(|_| refusal("embedded Direct lookup table bytes refused"))?;
    let expected = journal
        .lookup_addresses
        .iter()
        .map(|value| parse_key(value, "embedded Direct lookup address"))
        .collect::<Result<Vec<_>>>()?;
    if table.meta.authority.is_some()
        || table.meta.deactivation_slot != u64::MAX
        || table.addresses.as_ref() != expected
        || table.meta.last_extended_slot >= account.observation.slot
    {
        return Err(refusal(
            "embedded Direct lookup table was not exact, frozen, active, and activated before observation",
        ));
    }
    Ok(table)
}

fn authenticate_embedded_hot_message_v1(
    public: &DirectTradePublicManifestV1,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    journal: &DirectTradeJournalV1,
    physical: &DirectInlinePhysicalRouteV3,
    table: &AddressLookupTable<'_>,
) -> Result<()> {
    let message_bytes = decode_canonical_base64_v1(
        journal
            .message_base64
            .as_deref()
            .ok_or_else(|| refusal("embedded Direct Hot message was absent"))?,
        "embedded Direct Hot message",
    )?;
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("embedded Direct Hot message: {error}")))?;
    if bincode::serialize(&message)
        .map_err(|error| Error::new(format!("embedded Direct Hot message reencode: {error}")))?
        != message_bytes
    {
        return Err(refusal("embedded Direct Hot message was noncanonical"));
    }
    let VersionedMessage::V0(message) = message else {
        return Err(refusal("embedded Direct Hot message was not v0"));
    };
    let lookup = message
        .address_table_lookups
        .first()
        .filter(|_| message.address_table_lookups.len() == 1)
        .ok_or_else(|| refusal("embedded Direct Hot message did not use one exact lookup table"))?;
    if lookup.account_key != parse_key(&journal.lookup_table, "embedded Direct lookup table")? {
        return Err(refusal(
            "embedded Direct Hot message selected another lookup table",
        ));
    }
    let mut resolved = message.account_keys.clone();
    for indexes in [&lookup.writable_indexes, &lookup.readonly_indexes] {
        for index in indexes {
            resolved.push(
                table
                    .addresses
                    .get(usize::from(*index))
                    .copied()
                    .ok_or_else(|| refusal("embedded Direct Hot lookup index was out of range"))?,
            );
        }
    }
    if resolved.len() != 61 || message.account_keys.len() != 4 || message.instructions.len() != 4 {
        return Err(refusal(
            "embedded Direct Hot resolved key or instruction width changed",
        ));
    }
    let program_at = |index: u8| -> Result<Pubkey> {
        resolved
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| refusal("embedded Direct program index was out of range"))
    };
    let compute = message
        .instructions
        .first()
        .ok_or_else(|| refusal("embedded Direct ComputeBudget instruction was absent"))?;
    let heap = message
        .instructions
        .get(1)
        .ok_or_else(|| refusal("embedded Direct RequestHeapFrame instruction was absent"))?;
    let native = message
        .instructions
        .get(2)
        .ok_or_else(|| refusal("embedded Direct Ed25519 instruction was absent"))?;
    let hot = message
        .instructions
        .get(3)
        .ok_or_else(|| refusal("embedded Direct Trading instruction was absent"))?;
    let trading = parse_key(
        &public.route.fixed.trading_program,
        "Direct Trading program",
    )?;
    if program_at(compute.program_id_index)? != compute_budget::ID
        || program_at(heap.program_id_index)? != compute_budget::ID
        || program_at(native.program_id_index)? != ed25519_program::ID
        || program_at(hot.program_id_index)? != trading
    {
        return Err(refusal(
            "embedded Direct Hot top-level program order changed",
        ));
    }
    let expected_accounts = physical
        .fixed_accounts
        .iter()
        .chain(physical.runtime_accounts.iter().skip(5))
        .map(|meta| meta.account.key);
    let actual_accounts = hot.accounts.iter().map(|index| {
        resolved
            .get(usize::from(*index))
            .copied()
            .ok_or_else(|| refusal("embedded Direct Hot account index was out of range"))
    });
    for (expected, actual) in expected_accounts.zip(actual_accounts) {
        if expected != actual? {
            return Err(refusal(
                "embedded Direct Hot message changed a named semantic route coordinate",
            ));
        }
    }
    if hot.accounts.len()
        != physical
            .fixed_accounts
            .len()
            .checked_add(physical.runtime_accounts.len().saturating_sub(5))
            .ok_or_else(|| refusal("embedded Direct Hot account width overflow"))?
    {
        return Err(refusal("embedded Direct Hot account frame width changed"));
    }
    let family_request =
        compile_direct_inline_request_v3(seller, buyer, public.fill, public.execution_price)
            .map_err(|error| Error::new(format!("embedded Direct request: {error:?}")))?;
    if !hot.data.ends_with(&family_request) {
        return Err(refusal(
            "embedded Direct Hot instruction did not carry the two authenticated signed intents",
        ));
    }
    let ack_bytes = decode_canonical_base64_v1(
        journal
            .return_data_base64
            .as_deref()
            .ok_or_else(|| refusal("embedded Direct Hot ACK was absent"))?,
        "embedded Direct Hot ACK",
    )?;
    let ack = HotExecutionAckV3::decode(&ack_bytes)
        .map_err(|error| Error::new(format!("embedded Direct Hot ACK: {error:?}")))?;
    let root_digest = hex32(
        journal
            .expected_poststates
            .first()
            .ok_or_else(|| refusal("embedded Direct root poststate was absent"))?
            .data_sha256
            .as_str(),
        "embedded Direct root poststate digest",
    )?;
    if ack.to_bytes().as_slice() != ack_bytes
        || ack.market != parse_key(&public.market, "Direct Market")?.to_bytes()
        || ack.generation != public.context.generation
        || ack.root != parse_key(&public.route.fixed.root, "Direct root")?.to_bytes()
        || ack.release_set != hex32(&public.context.release_set, "Direct release set")?
        || ack.request_digest != hash(&family_request).to_bytes()
        || ack.root_poststate_digest != root_digest
    {
        return Err(refusal(
            "embedded Direct Hot ACK did not authenticate the exact request, Market, release, root, and root poststate",
        ));
    }
    Ok(())
}

fn authenticate_embedded_buyer_poststates_v1(
    public: &DirectTradePublicManifestV1,
    buyer: SignedDirectIntentV3,
    accounts: &[ObservedAccount],
) -> Result<()> {
    let claims = parse_key(&public.route.claims.claims_program, "Direct Claims program")?;
    let aggregate = parse_key(&public.route.claims.aggregate, "Direct Claims aggregate")?;
    let position = accounts
        .get(5)
        .ok_or_else(|| refusal("Direct buyer Position poststate was absent"))?;
    let position_view = LiabilityBasisPositionViewV2::decode(&position.data)
        .map_err(|error| Error::new(format!("Direct buyer Position: {error:?}")))?;
    let position_seeds = ProtocolPositionSeedsV2::new(aggregate.to_bytes(), buyer.maker.to_bytes())
        .map_err(|error| Error::new(format!("Direct buyer Position seeds: {error:?}")))?;
    let expected_position = Pubkey::find_program_address(&position_seeds.as_slices(), &claims).0;
    if position.key != expected_position
        || position.key != parse_key(&public.route.claims.buyer_position, "Direct buyer Position")?
        || position.owner != claims
        || position.executable
        || position_view.market_account != aggregate.to_bytes()
        || position_view.owner != buyer.maker.to_bytes()
        || position_view.claim_count != public.context.outcome_count
    {
        return Err(refusal(
            "Direct buyer Position was not the canonical Claims state owned by the signed buyer",
        ));
    }
    let buyer_token = accounts
        .get(7)
        .ok_or_else(|| refusal("Direct buyer collateral poststate was absent"))?;
    let token_program = parse_key(&public.route.custody.token_program, "Direct token program")?;
    let token = TokenAccount::parse(&buyer_token.data)
        .map_err(|error| Error::new(format!("Direct buyer token account: {error:?}")))?;
    if buyer_token.key != Pubkey::new_from_array(buyer.intent.collateral_account)
        || buyer_token.key != parse_key(&public.route.custody.buyer_token, "Direct buyer token")?
        || buyer_token.owner != token_program
        || buyer_token.executable
        || token.owner != buyer.maker.to_bytes()
        || token.mint != parse_key(&public.route.custody.mint, "Direct collateral mint")?.to_bytes()
    {
        return Err(refusal(
            "Direct buyer collateral source was not the signed buyer's exact Realm token account",
        ));
    }
    Ok(())
}

fn next_action_v1(
    validated: &ValidatedManifestV1,
    planning: &DirectTradePlanningV1,
) -> Result<NextActionV1> {
    let journals = journal_entries_v1(validated)?;
    if journals.is_empty() {
        if planning.seal.already_materialized {
            return Err(refusal(
                "Direct seal exists but the request-specific ALT journal is absent",
            ));
        }
        return Ok(NextActionV1 {
            stage: DirectTradeStageV1::LookupCreate,
            action_index: 0,
            stage_name: "lookup-create",
        });
    }
    let mut previous = None;
    for (index, entry) in journals.iter().enumerate() {
        let path = entry.path();
        let canonical = absolute_existing_file(&path, "Direct action journal")?;
        let bytes = fs::read(&canonical)?;
        require_unique_json_v1(&bytes, "Direct action journal")?;
        let journal: DirectTradeJournalV1 = serde_json::from_slice(&bytes)
            .map_err(|error| Error::new(format!("Direct action journal: {error}")))?;
        validate_journal(validated, planning, index, &journal, previous.as_ref())?;
        previous = Some(journal);
    }
    let last = previous.ok_or_else(|| refusal("Direct journal scan lost its last entry"))?;
    if last.phase != DirectTradeJournalPhaseV1::Finalized {
        return Ok(NextActionV1 {
            stage: last.stage,
            action_index: usize::from(last.action_index),
            stage_name: match last.phase {
                DirectTradeJournalPhaseV1::Planned => "resume-planned-without-key-access",
                DirectTradeJournalPhaseV1::Prepared => "arm-prepared-exact-packet",
                DirectTradeJournalPhaseV1::Dispatching => "dispatch-identical-persisted-packet",
                DirectTradeJournalPhaseV1::Submitted => "reconcile-submitted-never-blind-resubmit",
                DirectTradeJournalPhaseV1::Finalized => {
                    return Err(refusal("Direct journal phase branch was inconsistent"));
                }
            },
        });
    }
    if matches!(
        last.stage,
        DirectTradeStageV1::LookupFreeze
            | DirectTradeStageV1::LookupActivation
            | DirectTradeStageV1::CapabilitySeal
    ) {
        authenticate_frozen_lookup_v1(planning)?;
    }
    if planning.seal.already_materialized && last.stage != DirectTradeStageV1::CapabilitySeal {
        return Err(refusal(
            "Direct capability seal exists without its exact finalized journal stage",
        ));
    }
    if last.stage == DirectTradeStageV1::CapabilitySeal && !planning.seal.already_materialized {
        return Err(refusal(
            "Direct finalized seal journal has no exact materialized chain poststate",
        ));
    }
    Ok(NextActionV1 {
        stage: match last.stage {
            DirectTradeStageV1::LookupCreate | DirectTradeStageV1::LookupExtend
                if usize::from(last.action_index) < planning.provision.extensions.len() =>
            {
                DirectTradeStageV1::LookupExtend
            }
            DirectTradeStageV1::LookupCreate | DirectTradeStageV1::LookupExtend => {
                DirectTradeStageV1::LookupFreeze
            }
            DirectTradeStageV1::LookupFreeze => DirectTradeStageV1::LookupActivation,
            DirectTradeStageV1::LookupActivation => DirectTradeStageV1::CapabilitySeal,
            DirectTradeStageV1::CapabilitySeal => DirectTradeStageV1::Hot,
            DirectTradeStageV1::Hot => DirectTradeStageV1::Hot,
        },
        action_index: usize::from(last.action_index)
            .checked_add(1)
            .ok_or_else(|| refusal("Direct action index overflow"))?,
        stage_name: match last.stage {
            DirectTradeStageV1::LookupCreate => {
                if planning.provision.extensions.is_empty() {
                    "lookup-freeze"
                } else {
                    "lookup-extend"
                }
            }
            DirectTradeStageV1::LookupExtend => {
                if usize::from(last.action_index) < planning.provision.extensions.len() {
                    "lookup-extend"
                } else {
                    "lookup-freeze"
                }
            }
            DirectTradeStageV1::LookupFreeze => "lookup-activation",
            DirectTradeStageV1::LookupActivation => "capability-seal",
            DirectTradeStageV1::CapabilitySeal => "hot",
            DirectTradeStageV1::Hot => "complete",
        },
    })
}

pub(crate) fn authenticate_frozen_lookup_v1(planning: &DirectTradePlanningV1) -> Result<()> {
    let table = planning
        .lookup_table
        .as_ref()
        .ok_or_else(|| refusal("Direct frozen lookup-table observation is absent"))?;
    compile_direct_inline_capability_seal_routed_v0_v3(
        &planning.seal,
        SolanaHash::new_from_array([0x46; 32]),
        &planning.provision,
        table,
    )
    .map_err(|error| {
        refusal(format!(
            "Direct lookup table is not the exact frozen, later-finalized request union: {error:?}"
        ))
    })?;
    Ok(())
}

fn validate_journal(
    validated: &ValidatedManifestV1,
    planning: &DirectTradePlanningV1,
    index: usize,
    journal: &DirectTradeJournalV1,
    previous: Option<&DirectTradeJournalV1>,
) -> Result<()> {
    if journal.schema != direct_journal_schema_v1(&validated.public.cluster)?
        || journal.public_manifest_sha256 != validated.public_sha256
        || journal.private_session_sha256 != validated.private_sha256
        || usize::from(journal.action_index) != index
        || journal.lookup_table != planning.provision.lookup_table.to_string()
        || journal.lookup_addresses
            != planning
                .provision
                .addresses
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        || journal.lookup_addresses_sha256 != pubkey_list_sha256(&planning.provision.addresses)
        || journal.lookup_creation_slot != planning.provision.creation_slot
        || journal.observation_slot == 0
        || journal.intent_sha256 != journal_intent_sha256_v1(journal)?
        || journal.state_sha256 != journal_state_sha256(journal)?
    {
        return Err(refusal(
            "Direct action journal identity or state digest changed",
        ));
    }
    let expected_stage = expected_stage_v1(index, planning.provision.extensions.len())
        .ok_or_else(|| refusal("Direct action journal has an out-of-range action index"))?;
    if journal.stage != expected_stage {
        return Err(refusal("Direct action journal stage order changed"));
    }
    if let Some(previous) = previous
        && (previous.phase != DirectTradeJournalPhaseV1::Finalized
            || journal.action_index != previous.action_index.saturating_add(1)
            || journal.lookup_creation_slot != previous.lookup_creation_slot)
    {
        return Err(refusal(
            "Direct action journal skipped an unfinished or differently rooted predecessor",
        ));
    }
    let observational = journal.stage == DirectTradeStageV1::LookupActivation;
    if observational {
        if journal.phase != DirectTradeJournalPhaseV1::Finalized
            || journal.message_base64.is_some()
            || journal.message_sha256.is_some()
            || journal.last_valid_block_height.is_some()
            || journal.exact_fee_lamports.is_some()
            || journal.expected_wire_bytes.is_some()
            || journal.unique_message_account_count.is_some()
            || journal.signed_packet_base64.is_some()
            || journal.expected_signature.is_some()
            || journal.expected_return_data_producer.is_some()
            || journal.expected_return_data_base64.is_some()
            || !journal.expected_prestates.is_empty()
            || !journal.expected_poststates.is_empty()
            || journal.finalized_slot.is_none()
            || journal.transaction_sha256.is_some()
            || journal.fee_lamports.is_some()
            || journal.compute_units_consumed.is_some()
            || journal.return_data_producer.is_some()
            || journal.return_data_base64.is_some()
            || journal.return_data_was_null.is_some()
            || !journal.finalized_poststates.is_empty()
            || journal
                .finalized_slot
                .is_some_and(|slot| slot <= journal.lookup_creation_slot)
        {
            return Err(refusal(
                "Direct lookup activation journal is not an exact later finalized observation",
            ));
        }
        return Ok(());
    }
    let message = journal
        .message_base64
        .as_ref()
        .ok_or_else(|| refusal("Direct transaction journal omitted its exact message"))?;
    let message_bytes = BASE64
        .decode(message)
        .map_err(|error| Error::new(format!("Direct journal message base64: {error}")))?;
    if journal.message_sha256.as_deref() != Some(sha256_hex(&message_bytes).as_str()) {
        return Err(refusal("Direct transaction journal message digest changed"));
    }
    if journal.last_valid_block_height.is_none()
        || journal.exact_fee_lamports.is_none()
        || journal.expected_wire_bytes.is_none()
        || journal
            .unique_message_account_count
            .is_none_or(|count| count == 0 || count > 64)
        || (journal.stage == DirectTradeStageV1::Hot
            && (journal.expected_return_data_producer.is_none()
                || journal.expected_return_data_base64.is_none()
                || journal.expected_prestates.len() != 10
                || journal.expected_poststates.len() != 10))
        || (journal.stage != DirectTradeStageV1::Hot
            && (journal.expected_return_data_producer.is_some()
                || journal.expected_return_data_base64.is_some()
                || !journal.expected_prestates.is_empty()))
    {
        return Err(refusal(
            "Direct transaction journal durable intent is incomplete",
        ));
    }
    expected_direct_cluster_v1(validated)?.authenticate_finalized_fee(
        journal
            .exact_fee_lamports
            .ok_or_else(|| refusal("Direct transaction journal omitted its exact fee"))?,
        "Direct durable transaction",
    )?;
    match journal.phase {
        DirectTradeJournalPhaseV1::Planned => {
            if journal.signed_packet_base64.is_some()
                || journal.expected_signature.is_some()
                || journal.finalized_slot.is_some()
                || journal.transaction_sha256.is_some()
                || journal.fee_lamports.is_some()
                || journal.compute_units_consumed.is_some()
                || journal.return_data_producer.is_some()
                || journal.return_data_base64.is_some()
                || journal.return_data_was_null.is_some()
                || !journal.finalized_poststates.is_empty()
            {
                return Err(refusal(
                    "planned Direct journal carries later-only evidence",
                ));
            }
        }
        DirectTradeJournalPhaseV1::Prepared
        | DirectTradeJournalPhaseV1::Dispatching
        | DirectTradeJournalPhaseV1::Submitted => {
            if journal.signed_packet_base64.is_none()
                || journal.expected_signature.is_none()
                || journal.finalized_slot.is_some()
                || journal.transaction_sha256.is_some()
                || journal.fee_lamports.is_some()
                || journal.compute_units_consumed.is_some()
                || journal.return_data_producer.is_some()
                || journal.return_data_base64.is_some()
                || journal.return_data_was_null.is_some()
                || !journal.finalized_poststates.is_empty()
            {
                return Err(refusal(
                    "signed Direct journal has an invalid evidence shape",
                ));
            }
            authenticate_signed_journal_v1(journal, &message_bytes)?;
        }
        DirectTradeJournalPhaseV1::Finalized => {
            if journal.signed_packet_base64.is_none()
                || journal.expected_signature.is_none()
                || journal.finalized_slot.is_none()
                || journal.transaction_sha256.is_none()
                || journal.fee_lamports != journal.exact_fee_lamports
                || journal.return_data_was_null
                    != Some(journal.expected_return_data_base64.is_none())
                || (journal.stage == DirectTradeStageV1::Hot
                    && journal.finalized_poststates.len() != 10)
            {
                return Err(refusal(
                    "finalized Direct journal omitted exact transaction evidence",
                ));
            }
            authenticate_signed_journal_v1(journal, &message_bytes)?;
        }
    }
    Ok(())
}

fn authenticate_signed_journal_v1(
    journal: &DirectTradeJournalV1,
    message_bytes: &[u8],
) -> Result<()> {
    let packet = BASE64
        .decode(
            journal
                .signed_packet_base64
                .as_deref()
                .ok_or_else(|| refusal("signed Direct journal omitted its exact packet"))?,
        )
        .map_err(|error| Error::new(format!("Direct signed packet base64: {error}")))?;
    let transaction: VersionedTransaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("Direct signed packet: {error}")))?;
    if bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("Direct signed packet reencode: {error}")))?
        != packet
        || bincode::serialize(&transaction.message)
            .map_err(|error| Error::new(format!("Direct signed message reencode: {error}")))?
            != message_bytes
    {
        return Err(refusal(
            "Direct signed packet is noncanonical or differs from its durable message",
        ));
    }
    let (payer, required_signatures) = match &transaction.message {
        VersionedMessage::Legacy(message) => (
            message.account_keys.first(),
            message.header.num_required_signatures,
        ),
        VersionedMessage::V0(message) => (
            message.account_keys.first(),
            message.header.num_required_signatures,
        ),
    };
    let payer = payer.ok_or_else(|| refusal("Direct signed packet omitted its payer"))?;
    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| refusal("Direct signed packet omitted its payer signature"))?;
    let expected_signature = signature.to_string();
    let packet_sha256 = sha256_hex(&packet);
    if required_signatures != 1
        || transaction.signatures.len() != 1
        || !signature.verify(payer.as_ref(), &transaction.message.serialize())
        || journal.expected_signature.as_deref() != Some(expected_signature.as_str())
        || journal
            .transaction_sha256
            .as_ref()
            .is_some_and(|digest| digest != &packet_sha256)
    {
        return Err(refusal(
            "Direct signed packet signer, signature, or exact packet digest changed",
        ));
    }
    Ok(())
}

fn expected_stage_v1(index: usize, extension_count: usize) -> Option<DirectTradeStageV1> {
    if index == 0 {
        return Some(DirectTradeStageV1::LookupCreate);
    }
    if index <= extension_count {
        return Some(DirectTradeStageV1::LookupExtend);
    }
    if index == extension_count.checked_add(1)? {
        return Some(DirectTradeStageV1::LookupFreeze);
    }
    if index == extension_count.checked_add(2)? {
        return Some(DirectTradeStageV1::LookupActivation);
    }
    if index == extension_count.checked_add(3)? {
        return Some(DirectTradeStageV1::CapabilitySeal);
    }
    if index == extension_count.checked_add(4)? {
        return Some(DirectTradeStageV1::Hot);
    }
    None
}

fn journal_entries_v1(validated: &ValidatedManifestV1) -> Result<Vec<std::fs::DirEntry>> {
    let journal_dir = exact_directory(&validated.private.journal_dir, "Direct journal directory")?;
    let mut entries = fs::read_dir(journal_dir)?.collect::<core::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    Ok(entries
        .into_iter()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".json"))
        })
        .collect())
}

/// Load only the immutable creation root needed to rederive a request-specific
/// table on a later finalized observation. Full journal authentication occurs
/// again after the named route and exact table address have been rederived.
pub(crate) fn journal_root_v1(validated: &ValidatedManifestV1) -> Result<Option<DirectTradeJournalV1>> {
    let entries = journal_entries_v1(validated)?;
    let Some(first) = entries.first() else {
        return Ok(None);
    };
    let path = absolute_existing_file(&first.path(), "Direct root action journal")?;
    let bytes = fs::read(path)?;
    require_unique_json_v1(&bytes, "Direct root action journal")?;
    let journal: DirectTradeJournalV1 = serde_json::from_slice(&bytes)
        .map_err(|error| Error::new(format!("Direct root action journal: {error}")))?;
    let address_digest = pubkey_strings_sha256(&journal.lookup_addresses)?;
    if journal.schema != direct_journal_schema_v1(&validated.public.cluster)?
        || journal.public_manifest_sha256 != validated.public_sha256
        || journal.private_session_sha256 != validated.private_sha256
        || journal.stage != DirectTradeStageV1::LookupCreate
        || journal.action_index != 0
        || journal.lookup_creation_slot == 0
        || journal.lookup_table.parse::<Pubkey>().is_err()
        || journal.lookup_addresses.is_empty()
        || journal.lookup_addresses_sha256 != address_digest
        || journal.state_sha256 != journal_state_sha256(&journal)?
    {
        return Err(refusal(
            "Direct root journal does not authenticate one immutable lookup-table creation",
        ));
    }
    Ok(Some(journal))
}

fn refusal(reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: {}", reason.as_ref()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn pubkey_list_sha256(keys: &[Pubkey]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"dclutch-direct-request-lookup-addresses-v1");
    digest.update(u64::try_from(keys.len()).unwrap_or(u64::MAX).to_le_bytes());
    for key in keys {
        digest.update(key.as_ref());
    }
    let mut output = String::with_capacity(64);
    for byte in digest.finalize() {
        use core::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn pubkey_strings_sha256(keys: &[String]) -> Result<String> {
    let parsed = keys
        .iter()
        .map(|key| parse_key(key, "Direct journal lookup address"))
        .collect::<Result<Vec<_>>>()?;
    Ok(pubkey_list_sha256(&parsed))
}

fn session_state_sha256(session: &DirectTradePrivateSessionV1) -> Result<String> {
    let mut canonical = session.clone();
    canonical.session_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn journal_state_sha256(journal: &DirectTradeJournalV1) -> Result<String> {
    let mut canonical = journal.clone();
    canonical.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn absolute_existing_file(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(refusal(format!("{label} path is not absolute")));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(refusal(format!("{label} is not one exact regular file")));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err(refusal(format!("{label} path is not canonical")));
    }
    Ok(canonical)
}

fn exact_path(value: &str, label: &str) -> Result<PathBuf> {
    absolute_existing_file(Path::new(value), label)
}

fn exact_directory(value: &str, label: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(refusal(format!("{label} path is not absolute")));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(refusal(format!("{label} is not one exact directory")));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err(refusal(format!("{label} path is not canonical")));
    }
    Ok(canonical)
}

fn parse_key(value: &str, label: &str) -> Result<Pubkey> {
    Pubkey::from_str(value).map_err(|error| Error::new(format!("{label}: {error}")))
}

fn parse_optional_key(value: &str, label: &str) -> Result<[u8; 32]> {
    Ok(parse_key(value, label)?.to_bytes())
}

fn hex32(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(refusal(format!("{label} is not canonical lowercase hex32")));
    }
    let mut output = [0_u8; 32];
    for (target, pair) in output.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let [high, low] = pair else {
            return Err(refusal(format!("{label} hex width")));
        };
        let high = hex_nibble(*high).ok_or_else(|| refusal(format!("{label} hex")))?;
        let low = hex_nibble(*low).ok_or_else(|| refusal(format!("{label} hex")))?;
        *target = high << 4 | low;
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn require_unique_json_v1(bytes: &[u8], label: &str) -> Result<()> {
    parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("{label} {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use serde_json::json;
    use solana_program::pubkey::Pubkey;

    use super::{
        DirectClaimBalanceEvidenceV1, DirectFinalizedMutationEvidenceV1,
        DirectLookupActivationEvidenceV1, DirectPositionTransitionEvidenceV1,
        DirectTradeExpectedPoststateV1, DirectTradeFinalizedEvidenceV1, DirectTradeJournalPhaseV1,
        DirectTradeJournalV1, DirectTradeObservedPoststateV1, DirectTradeStageV1,
        JOURNAL_SCHEMA_V1, OWNED_EVIDENCE_SCHEMA_V1, OWNED_JOURNAL_SCHEMA_V1,
        OWNED_PRIVATE_SESSION_SCHEMA_V1, OWNED_PUBLIC_MANIFEST_SCHEMA_V1,
        authenticate_direct_claim_schedule_v1, authenticate_direct_history_v1,
        authenticate_embedded_direct_evidence_identity_v1,
        authenticate_lookup_activation_slot_order_v1, direct_evidence_digest_v1,
        direct_evidence_schema_v1, direct_journal_schema_v1, direct_private_schema_v1,
        direct_public_schema_v1, expected_stage_v1, hex32, journal_intent_sha256_v1,
        journal_state_sha256, refresh_direct_journal_digest_v1, require_unique_json_v1, usage,
        parse_direct_return_data_v1, verify_expected_direct_poststates_v1, write_direct_journal_v1,
    };
    use crate::cluster::ExpectedClusterV1;
    use dclutch_operator::{Finality, Observation, ObservedAccount};

    /// An omitted `returnData` and an explicit null carry the same fact, and
    /// devnet sends the omission. Everything past that stays exact.
    #[test]
    fn absent_and_null_return_data_read_alike_and_malformed_bodies_refuse() {
        let none = (None, None);
        assert_eq!(
            parse_direct_return_data_v1(&json!({"fee": 5000})).expect("absent"),
            none
        );
        assert_eq!(
            parse_direct_return_data_v1(&json!({"returnData": null})).expect("null"),
            none
        );
        let body = BASE64.encode([1_u8, 2, 3]);
        assert_eq!(
            parse_direct_return_data_v1(
                &json!({"returnData": {"programId": "prog", "data": [body, "base64"]}})
            )
            .expect("present"),
            (Some("prog".to_owned()), Some(body))
        );
        for malformed in [
            json!({"returnData": {"data": [BASE64.encode([1_u8]), "base64"]}}),
            json!({"returnData": {"programId": "prog"}}),
            json!({"returnData": {"programId": "prog", "data": [BASE64.encode([1_u8])]}}),
            json!({"returnData": {"programId": "prog", "data": [BASE64.encode([1_u8]), "hex"]}}),
            json!({"returnData": {"programId": "prog", "data": ["!not base64!", "base64"]}}),
        ] {
            assert!(parse_direct_return_data_v1(&malformed).is_err());
        }
    }

    #[test]
    fn usage_exposes_exact_owned_loopback_direct_executor() {
        assert!(usage().lines().any(|line| {
            line.trim()
                == "dclutch-local-successor-bootstrap local-private-validator-direct-trade-v1 --rpc-url http://127.0.0.1:PORT/ --session ABSOLUTE_PRIVATE_JSON [--execute]"
        }));
    }

    fn journal() -> DirectTradeJournalV1 {
        DirectTradeJournalV1 {
            schema: JOURNAL_SCHEMA_V1.into(),
            public_manifest_sha256: "11".repeat(32),
            private_session_sha256: "22".repeat(32),
            stage: DirectTradeStageV1::LookupCreate,
            action_index: 0,
            phase: DirectTradeJournalPhaseV1::Planned,
            observation_slot: 9,
            lookup_creation_slot: 9,
            lookup_table: "11111111111111111111111111111111".into(),
            lookup_addresses: vec!["11111111111111111111111111111111".into()],
            lookup_addresses_sha256: "33".repeat(32),
            message_base64: Some("AA==".into()),
            message_sha256: Some("44".repeat(32)),
            last_valid_block_height: Some(99),
            exact_fee_lamports: Some(5_000),
            expected_wire_bytes: Some(200),
            unique_message_account_count: Some(4),
            signed_packet_base64: None,
            expected_signature: None,
            expected_return_data_producer: None,
            expected_return_data_base64: None,
            expected_prestates: Vec::new(),
            expected_poststates: Vec::new(),
            finalized_slot: None,
            transaction_sha256: None,
            fee_lamports: None,
            compute_units_consumed: None,
            return_data_producer: None,
            return_data_base64: None,
            return_data_was_null: None,
            finalized_poststates: Vec::new(),
            intent_sha256: String::new(),
            state_sha256: String::new(),
        }
    }

    fn terminal_evidence() -> DirectTradeFinalizedEvidenceV1 {
        let seller_owner = Pubkey::new_unique().to_string();
        let seller_position = Pubkey::new_unique().to_string();
        let buyer_owner = Pubkey::new_unique().to_string();
        let buyer_position = Pubkey::new_unique().to_string();
        let buyer_recipient = Pubkey::new_unique().to_string();
        let seller_recipient = Pubkey::new_unique().to_string();
        let expected = DirectTradeExpectedPoststateV1 {
            address: Pubkey::new_unique().to_string(),
            owner: Pubkey::new_unique().to_string(),
            lamports: 1,
            executable: false,
            data_base64: BASE64.encode([1]),
            data_sha256: super::sha256_hex(&[1]),
        };
        let observed = DirectTradeObservedPoststateV1 {
            address: expected.address.clone(),
            owner: expected.owner.clone(),
            lamports: expected.lamports,
            executable: expected.executable,
            data_len: 1,
            data_sha256: expected.data_sha256.clone(),
        };
        let mutation = |kind: &str| DirectFinalizedMutationEvidenceV1 {
            kind: kind.into(),
            prefix_len: None,
            path: format!("/tmp/{kind}.json"),
            sha256: "33".repeat(32),
            intent_sha256: "44".repeat(32),
            schema: OWNED_JOURNAL_SCHEMA_V1.into(),
            completion_pointer: "/phase".into(),
            completion_value: "finalized".into(),
            signature: format!("{kind}-signature"),
            slot: 7,
            fee_payer: Pubkey::new_unique().to_string(),
            fee_lamports: 5_000,
            compute_units_consumed: 1,
        };
        let mut evidence = DirectTradeFinalizedEvidenceV1 {
            schema: OWNED_EVIDENCE_SCHEMA_V1.into(),
            status: "finalized".into(),
            cluster: "owned-loopback".into(),
            public_manifest_sha256: "11".repeat(32),
            public_manifest_base64: BASE64.encode(b"{}"),
            private_session_sha256: "22".repeat(32),
            journal_state_sha256: "33".repeat(32),
            hot_journal_base64: BASE64.encode(b"{}"),
            signature: "hot-signature".into(),
            finalized_slot: 7,
            market: Pubkey::new_unique().to_string(),
            seller_owner: seller_owner.clone(),
            seller_position: seller_position.clone(),
            buyer_position: buyer_position.clone(),
            buyer_owner: buyer_owner.clone(),
            buyer_collateral_source: buyer_recipient.clone(),
            seller_collateral_destination: seller_recipient.clone(),
            fee_token_account: Pubkey::new_unique().to_string(),
            fee_basis_points_per_side: 50,
            fee_recipient: Pubkey::new_unique().to_string(),
            mint: Pubkey::new_unique().to_string(),
            outcome_index: 0,
            outcome_count: 4,
            fill_atoms: 1,
            execution_price: 500_000,
            price_scale: 1_000_000,
            lookup_table: Pubkey::new_unique().to_string(),
            lookup_addresses_sha256: "55".repeat(32),
            lookup_address_count: 57,
            static_account_count: 4,
            loaded_address_count: 57,
            unique_message_account_count: 61,
            wire_bytes: 1_167,
            capability_seal: Pubkey::new_unique().to_string(),
            capability_seal_sha256: "66".repeat(32),
            hot_ack_producer: Pubkey::new_unique().to_string(),
            hot_ack_base64: BASE64.encode([2]),
            hot_ack_sha256: super::sha256_hex(&[2]),
            mutations: vec![
                mutation("replay-setup"),
                mutation("token-setup"),
                mutation("hot"),
            ],
            lookup_activation: DirectLookupActivationEvidenceV1 {
                path: "/tmp/activation.json".into(),
                sha256: "77".repeat(32),
                intent_sha256: "88".repeat(32),
                schema: OWNED_JOURNAL_SCHEMA_V1.into(),
                completion_pointer: "/phase".into(),
                completion_value: "finalized".into(),
                finalized_slot: 6,
                lookup_table: Pubkey::new_unique().to_string(),
                lookup_addresses_sha256: "55".repeat(32),
            },
            positions: [
                DirectPositionTransitionEvidenceV1 {
                    account: seller_position.clone(),
                    owner: seller_owner.clone(),
                    pre_data_base64: BASE64.encode([3]),
                    post_data_base64: BASE64.encode([4]),
                },
                DirectPositionTransitionEvidenceV1 {
                    account: buyer_position.clone(),
                    owner: buyer_owner.clone(),
                    pre_data_base64: BASE64.encode([5]),
                    post_data_base64: BASE64.encode([6]),
                },
            ],
            claim_balances: vec![
                DirectClaimBalanceEvidenceV1 {
                    owner: seller_owner.clone(),
                    position: seller_position.clone(),
                    recipient_token: seller_recipient.clone(),
                    claim_index: 0,
                    quantity_atoms: 9,
                },
                DirectClaimBalanceEvidenceV1 {
                    owner: seller_owner.clone(),
                    position: seller_position.clone(),
                    recipient_token: seller_recipient.clone(),
                    claim_index: 1,
                    quantity_atoms: 10,
                },
                DirectClaimBalanceEvidenceV1 {
                    owner: seller_owner.clone(),
                    position: seller_position.clone(),
                    recipient_token: seller_recipient.clone(),
                    claim_index: 2,
                    quantity_atoms: 10,
                },
                DirectClaimBalanceEvidenceV1 {
                    owner: seller_owner,
                    position: seller_position,
                    recipient_token: seller_recipient,
                    claim_index: 3,
                    quantity_atoms: 10,
                },
                DirectClaimBalanceEvidenceV1 {
                    owner: buyer_owner,
                    position: buyer_position,
                    recipient_token: buyer_recipient,
                    claim_index: 0,
                    quantity_atoms: 1,
                },
            ],
            final_accounts: vec![expected; 10],
            poststates: vec![observed; 10],
            evidence_sha256: String::new(),
        };
        evidence.evidence_sha256 = direct_evidence_digest_v1(&evidence).expect("evidence digest");
        evidence
    }

    #[test]
    fn action_order_is_total_over_one_exact_extension_count() {
        assert_eq!(
            expected_stage_v1(0, 2),
            Some(DirectTradeStageV1::LookupCreate)
        );
        assert_eq!(
            expected_stage_v1(1, 2),
            Some(DirectTradeStageV1::LookupExtend)
        );
        assert_eq!(
            expected_stage_v1(2, 2),
            Some(DirectTradeStageV1::LookupExtend)
        );
        assert_eq!(
            expected_stage_v1(3, 2),
            Some(DirectTradeStageV1::LookupFreeze)
        );
        assert_eq!(
            expected_stage_v1(4, 2),
            Some(DirectTradeStageV1::LookupActivation)
        );
        assert_eq!(
            expected_stage_v1(5, 2),
            Some(DirectTradeStageV1::CapabilitySeal)
        );
        assert_eq!(expected_stage_v1(6, 2), Some(DirectTradeStageV1::Hot));
        assert_eq!(expected_stage_v1(7, 2), None);
    }

    #[test]
    fn lookup_activation_is_strictly_after_creation_and_between_freeze_and_seal() {
        assert!(authenticate_lookup_activation_slot_order_v1(11, 10, 11, 12).is_ok());
        assert!(authenticate_lookup_activation_slot_order_v1(12, 10, 11, 12).is_ok());
        assert!(authenticate_lookup_activation_slot_order_v1(10, 10, 10, 12).is_err());
        assert!(authenticate_lookup_activation_slot_order_v1(11, 10, 12, 13).is_err());
        assert!(authenticate_lookup_activation_slot_order_v1(13, 10, 11, 12).is_err());
    }

    #[test]
    fn terminal_identity_refuses_position_order_and_mutation_role_substitution() {
        let exact = terminal_evidence();
        assert!(
            authenticate_embedded_direct_evidence_identity_v1(
                &exact,
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_ok()
        );

        let mut local_zero = exact.clone();
        for mutation in &mut local_zero.mutations {
            mutation.fee_lamports = 0;
        }
        local_zero.evidence_sha256 =
            direct_evidence_digest_v1(&local_zero).expect("zero-fee digest");
        assert!(
            authenticate_embedded_direct_evidence_identity_v1(
                &local_zero,
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_ok()
        );

        let mut public_zero = local_zero;
        public_zero.schema = direct_evidence_schema_v1("devnet")
            .expect("devnet evidence schema")
            .into();
        public_zero.cluster = "devnet".into();
        public_zero.evidence_sha256 =
            direct_evidence_digest_v1(&public_zero).expect("public zero-fee digest");
        assert!(
            authenticate_embedded_direct_evidence_identity_v1(
                &public_zero,
                ExpectedClusterV1::Devnet,
            )
            .is_err()
        );

        let mut reordered = exact.clone();
        reordered.positions.swap(0, 1);
        reordered.evidence_sha256 = direct_evidence_digest_v1(&reordered).expect("digest");
        assert!(
            authenticate_embedded_direct_evidence_identity_v1(
                &reordered,
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_err()
        );

        let mut substituted = exact.clone();
        substituted.mutations[0].kind = "token-setup".into();
        substituted.evidence_sha256 = direct_evidence_digest_v1(&substituted).expect("digest");
        assert!(
            authenticate_embedded_direct_evidence_identity_v1(
                &substituted,
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_err()
        );

        let mut public_cluster = exact;
        public_cluster.cluster = "devnet".into();
        public_cluster.evidence_sha256 =
            direct_evidence_digest_v1(&public_cluster).expect("digest");
        assert!(
            authenticate_embedded_direct_evidence_identity_v1(
                &public_cluster,
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_claim_schedule_requires_canonical_k_plus_one_and_losing_burns() {
        let exact = terminal_evidence();
        authenticate_direct_claim_schedule_v1(&exact).expect("exact K+1 claim schedule");
        assert_eq!(exact.claim_balances.len(), 5);
        assert!(
            exact.claim_balances[..4]
                .iter()
                .filter(|row| row.claim_index != exact.outcome_index)
                .all(|row| row.quantity_atoms != 0),
            "every losing seller claim remains scheduled for its zero-collateral burn"
        );

        let mut missing_losing = exact.clone();
        missing_losing.claim_balances.remove(2);
        assert!(authenticate_direct_claim_schedule_v1(&missing_losing).is_err());

        let mut zero_losing = exact.clone();
        zero_losing.claim_balances[2].quantity_atoms = 0;
        assert!(authenticate_direct_claim_schedule_v1(&zero_losing).is_err());

        let mut reordered = exact.clone();
        reordered.claim_balances.swap(1, 2);
        assert!(authenticate_direct_claim_schedule_v1(&reordered).is_err());

        let mut extra_buyer = exact;
        let mut row = extra_buyer.claim_balances[4].clone();
        row.claim_index = 1;
        extra_buyer.claim_balances.push(row);
        assert!(authenticate_direct_claim_schedule_v1(&extra_buyer).is_err());
    }

    #[test]
    fn journal_digest_excludes_only_its_own_digest_field() -> crate::Result<()> {
        let mut first = journal();
        let digest = journal_state_sha256(&first)?;
        first.state_sha256 = digest.clone();
        assert_eq!(journal_state_sha256(&first)?, digest);
        first.lookup_creation_slot = first.lookup_creation_slot.saturating_add(1);
        assert_ne!(journal_state_sha256(&first)?, digest);
        Ok(())
    }

    #[test]
    fn journal_intent_is_stable_across_dispatch_and_changes_with_the_message() -> crate::Result<()>
    {
        let mut journal = journal();
        refresh_direct_journal_digest_v1(&mut journal)?;
        let intent = journal.intent_sha256.clone();
        let planned_state = journal.state_sha256.clone();

        journal.phase = DirectTradeJournalPhaseV1::Prepared;
        journal.signed_packet_base64 = Some("AA==".into());
        journal.expected_signature = Some("fixture".into());
        refresh_direct_journal_digest_v1(&mut journal)?;
        assert_eq!(journal.intent_sha256, intent);
        assert_ne!(journal.state_sha256, planned_state);

        for phase in [
            DirectTradeJournalPhaseV1::Dispatching,
            DirectTradeJournalPhaseV1::Submitted,
        ] {
            journal.phase = phase;
            refresh_direct_journal_digest_v1(&mut journal)?;
            assert_eq!(journal.intent_sha256, intent);
            assert_eq!(journal.intent_sha256, journal_intent_sha256_v1(&journal)?);
        }

        journal.phase = DirectTradeJournalPhaseV1::Finalized;
        journal.finalized_slot = Some(77);
        journal.transaction_sha256 = Some("55".repeat(32));
        journal.fee_lamports = journal.exact_fee_lamports;
        journal.compute_units_consumed = Some(123);
        journal.return_data_was_null = Some(true);
        refresh_direct_journal_digest_v1(&mut journal)?;
        assert_eq!(journal.intent_sha256, intent);

        journal.message_sha256 = Some("66".repeat(32));
        refresh_direct_journal_digest_v1(&mut journal)?;
        assert_ne!(journal.intent_sha256, intent);
        Ok(())
    }

    #[test]
    fn duplicate_json_and_noncanonical_hex_are_refused() -> crate::Result<()> {
        assert!(require_unique_json_v1(br#"{"a":{"b":1,"b":2}}"#, "fixture").is_err());
        assert!(require_unique_json_v1(br#"{"a":{"b":1}}"#, "fixture").is_ok());
        assert_eq!(hex32(&"ab".repeat(32), "fixture")?, [0xab; 32]);
        assert!(hex32(&"AB".repeat(32), "fixture").is_err());
        Ok(())
    }

    #[test]
    fn devnet_and_owned_loopback_have_disjoint_durable_schemas() -> crate::Result<()> {
        assert_eq!(
            direct_public_schema_v1(ExpectedClusterV1::OwnedLoopback),
            OWNED_PUBLIC_MANIFEST_SCHEMA_V1
        );
        assert_eq!(
            direct_private_schema_v1(ExpectedClusterV1::OwnedLoopback),
            OWNED_PRIVATE_SESSION_SCHEMA_V1
        );
        assert_eq!(
            direct_journal_schema_v1("owned-loopback")?,
            OWNED_JOURNAL_SCHEMA_V1
        );
        assert_eq!(
            direct_evidence_schema_v1("owned-loopback")?,
            OWNED_EVIDENCE_SCHEMA_V1
        );
        assert_ne!(
            direct_public_schema_v1(ExpectedClusterV1::Devnet),
            OWNED_PUBLIC_MANIFEST_SCHEMA_V1
        );
        assert!(direct_journal_schema_v1("mainnet-beta").is_err());
        Ok(())
    }

    #[test]
    fn finalized_history_requires_exact_ack_producer_body_fee_and_balance_delta() {
        let packet = vec![1_u8, 2, 3];
        let ack = vec![4_u8; 280];
        let producer = Pubkey::new_unique().to_string();
        let mut durable = journal();
        durable.exact_fee_lamports = Some(5);
        durable.signed_packet_base64 = Some(BASE64.encode(&packet));
        durable.expected_return_data_producer = Some(producer.clone());
        durable.expected_return_data_base64 = Some(BASE64.encode(&ack));
        let history = json!({
            "slot": 11,
            "transaction": [BASE64.encode(&packet), "base64"],
            "meta": {
                "err": null,
                "fee": 5,
                "preBalances": [100],
                "postBalances": [95],
                "returnData": {
                    "programId": producer,
                    "data": [BASE64.encode(&ack), "base64"]
                }
            }
        });
        assert!(authenticate_direct_history_v1(&durable, &history).is_ok());

        let mut substituted = history.clone();
        substituted["meta"]["returnData"]["data"][0] = json!(BASE64.encode([5_u8; 280]));
        assert!(authenticate_direct_history_v1(&durable, &substituted).is_err());
        let mut substituted = history.clone();
        substituted["meta"]["returnData"]["programId"] = json!(Pubkey::new_unique().to_string());
        assert!(authenticate_direct_history_v1(&durable, &substituted).is_err());
        let mut substituted = history.clone();
        substituted["meta"]["postBalances"] = json!([94]);
        assert!(authenticate_direct_history_v1(&durable, &substituted).is_err());
        let mut substituted = history;
        substituted["meta"]
            .as_object_mut()
            .expect("meta object")
            .remove("returnData");
        assert!(authenticate_direct_history_v1(&durable, &substituted).is_err());
    }

    #[test]
    fn all_expected_poststate_bytes_owner_lamports_and_order_are_exact() {
        let observation = Observation {
            slot: 9,
            unix_timestamp: 10,
            finality: Finality::Finalized,
        };
        let key = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let account = ObservedAccount {
            observation,
            key,
            owner,
            lamports: 77,
            executable: false,
            data: vec![1, 2, 3],
        };
        let expected = DirectTradeExpectedPoststateV1 {
            address: key.to_string(),
            owner: owner.to_string(),
            lamports: 77,
            executable: false,
            data_base64: BASE64.encode([1, 2, 3]),
            data_sha256: super::sha256_hex(&[1, 2, 3]),
        };
        assert_eq!(
            verify_expected_direct_poststates_v1(
                core::slice::from_ref(&account),
                core::slice::from_ref(&expected),
            )
            .expect("exact poststate")
            .len(),
            1
        );
        for case in 0..3 {
            let mut hostile = account.clone();
            match case {
                0 => hostile.owner = Pubkey::new_unique(),
                1 => hostile.lamports += 1,
                _ => hostile.data[0] ^= 1,
            }
            assert!(
                verify_expected_direct_poststates_v1(
                    core::slice::from_ref(&hostile),
                    core::slice::from_ref(&expected),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn journal_writer_refuses_duplicate_and_stale_updates() -> crate::Result<()> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "dclutch-direct-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        let path = root.join("0000-lookup-create.json");
        let mut first = journal();
        refresh_direct_journal_digest_v1(&mut first)?;
        write_direct_journal_v1(&path, &first, true, None)?;
        assert!(write_direct_journal_v1(&path, &first, true, None).is_err());

        let previous = first.state_sha256.clone();
        let mut second = first.clone();
        second.phase = DirectTradeJournalPhaseV1::Prepared;
        second.signed_packet_base64 = Some("AA==".into());
        second.expected_signature = Some("fixture".into());
        refresh_direct_journal_digest_v1(&mut second)?;
        write_direct_journal_v1(&path, &second, false, Some(&previous))?;

        let mut stale = first;
        stale.observation_slot += 1;
        refresh_direct_journal_digest_v1(&mut stale)?;
        assert!(write_direct_journal_v1(&path, &stale, false, Some(&previous)).is_err());
        let persisted: DirectTradeJournalV1 = serde_json::from_slice(&std::fs::read(&path)?)?;
        assert_eq!(persisted, second);
        std::fs::remove_file(path)?;
        std::fs::remove_dir(root)?;
        Ok(())
    }
}
