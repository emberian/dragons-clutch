//! One-role, permanent-program-id Upgrade orchestration for Solana devnet.
//!
//! Decision 0012 keeps the seven durable devnet program ids mutable.  That is
//! intentionally different from `campaign.rs`: a campaign publishes and
//! activates facts *after* deployment, while this module is the narrow seam
//! allowed to invoke `solana program deploy` against one existing Loader-v3
//! Program/ProgramData pair.
//!
//! The Solana CLI is behind [`CliRunner`].  Tests therefore exercise every
//! destructive boundary with a fake; they never open a keypair file, contact a
//! cluster, or spawn `solana`.  Production passes keypair paths to the CLI as
//! opaque arguments.  This module itself reads only the candidate ELF, its
//! checked evidence, the deployed-byte dump, and its resumable receipt.

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr as _,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_direct_codec::COMPILED_DIRECT_RELEASE_ID_V1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, DeploymentObservationV1,
    require_slot_pinned_release_v1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProtocolInfrastructureProfileV1,
    SourceSemanticRoleV1, source_semantic_release_preimage_v1,
};
use dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V5;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use solana_program::rent::Rent;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_sdk_ids::{bpf_loader_upgradeable, sysvar};

use crate::{
    Error, Result,
    cluster::{
        ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH, MAINNET_BETA_GENESIS_HASH,
    },
    model::{
        CheckedDeploymentDispositionV1, CheckedInfrastructureCarryForwardPinV1,
        CheckedUpgradeRolePinV1, CheckedUpgradeSetPinV1,
    },
    rpc::{Rpc, RpcAccount, WritePolicyV1},
};

const SCHEMA: &str = "dclutch-devnet-permanent-id-upgrade-receipt-v4";
const PREFLIGHT_SCHEMA: &str = "dclutch-devnet-permanent-id-upgrade-preflight-v1";
const CHECKED_GATE_SCHEMA: &str = "dclutch-checked-upgrade-gate-v1";
const BASELINE_SCHEMA: &str = "dclutch-devnet-upgrade-baseline-v1";
const EXTENSION_SCHEMA: &str = "dclutch-devnet-programdata-extension-receipt-v2";
const SET_JOURNAL_SCHEMA: &str = "dclutch-devnet-deployment-set-journal-v2";
const SET_AUDIT_SCHEMA: &str = "dclutch-devnet-deployment-set-audit-v2";
const CARRY_FORWARD_SNAPSHOT_SCHEMA: &str = "dclutch-carry-forward-rpc-snapshot-v1";
pub(crate) const CHECKED_SET_PREPARE_SCHEMA: &str = "dclutch-checked-deployment-set-release-pin-v2";
pub(crate) const SEMANTIC_DERIVATION_V1: &str =
    "source-semantic-release-v1+compiled-direct-release-v1+resolution-controller-release-v5";
const TARGET_ACK_FLAG: &str = "--i-accept-upgrade";
const EXCLUSIVE_PAYER_ACK_FLAG: &str = "--i-kept-fee-payer-exclusive";
const OPERATION_ACCOUNTING_SCOPE_V1: &str = "exclusive-payer-window-observed-net-v1";
const OPERATION_COST_ATTRIBUTION_V1: &str =
    "final-upgrade-transaction-fee-exact;remaining-cli-net-unattributed";
const EXTENSION_ACK_FLAG: &str = "--i-accept-extension";
/// A provisional operator reserve above the exact rent top-up. The receipt
/// records the actual fee separately; this bound is lifted by measured devnet
/// extension receipts rather than guessed downward during an operation.
const EXTENSION_FEE_RESERVE_LAMPORTS: u64 = 1_000_000;
/// Operational bound, not a chain bound. One page carries the RPC maximum of
/// 1,000 signature rows; sixteen pages cover 16,000 ProgramData touches while
/// still refusing an unbounded history walk. A refusal reports the target slot
/// so the operator can preserve the evidence and lift this bound explicitly.
const SIGNATURE_HISTORY_MAX_PAGES: usize = 16;
const SIGNATURE_HISTORY_PAGE_ROWS: u64 = 1_000;
const ROLES: &[&str] = &[
    "registry",
    "rent",
    "custody",
    "resolution",
    "claims",
    "trading",
    "core",
];
/// Decision-0012's permanent devnet Loader pairs. This is the operational
/// identity owner for the seven-role set auditor; a journal can pin evidence
/// for these accounts, never nominate a replacement set.
const PERMANENT_DEVNET_UPGRADE_TARGETS_V1: &[(&str, &str, &str)] = &[
    (
        "registry",
        "Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj",
        "ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz",
    ),
    (
        "rent",
        "DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3",
        "78MW6W4iPzBVLceAwTL51CtyLcpcFM2iGVMDbzZtUFmy",
    ),
    (
        "custody",
        "34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH",
        "EhB7hHJ7vsCW3nCeqbxbJrn5Jsi6gbqwpVhoLMPZ8ENf",
    ),
    (
        "resolution",
        "2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd",
        "2QFBQJdLBXAnJWTVK8KeeUtWZEFhQqqN2CbkrWjMjY6f",
    ),
    (
        "claims",
        "85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN",
        "4La2511ddSxUcAQfdhKvEeGEasih3TStbQWVFEQKd34j",
    ),
    (
        "trading",
        "5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk",
        "AE1cWbCvXedE23XH3otSxvDQ7xVx7WLNMYDc8y8rqkrn",
    ),
    (
        "core",
        "HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N",
        "AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN",
    ),
];

pub(crate) fn is_permanent_devnet_program_set(programs: &[Pubkey; 7]) -> bool {
    programs
        .iter()
        .zip(PERMANENT_DEVNET_UPGRADE_TARGETS_V1)
        .all(|(observed, (_, expected_program, _))| {
            parse_pubkey(expected_program, "permanent set Program")
                .is_ok_and(|expected| *observed == expected)
        })
}
const SHIPPED_LINKS: &[(&str, &str, bool)] = &[
    ("claims", "dclutch-claims-sbf", true),
    ("core", "dclutch-core-sbf", true),
    ("custody", "dclutch-custody-sbf", true),
    ("dealer-accelerator", "dclutch-dealer-accelerator-sbf", true),
    ("dclutch-dealer-sbf", "dclutch-dealer-sbf", false),
    ("dclutch-direct-aot-sbf", "dclutch-direct-aot-sbf", false),
    (
        "general-accelerator",
        "dclutch-general-accelerator-sbf",
        true,
    ),
    (
        "dclutch-product-runtime-v2-sbf",
        "dclutch-product-runtime-v2-sbf",
        false,
    ),
    ("registry", "dclutch-registry-sbf", true),
    ("rent", "dclutch-rent-sbf", true),
    ("resolution", "dclutch-resolution-proof-sbf", true),
    ("series-shadow", "dclutch-series-shadow-sbf", true),
    ("trading", "dclutch-trading-sbf", true),
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct UpgradeArgsV1 {
    origin: ClusterOriginV1,
    role: String,
    program_id: Pubkey,
    programdata_id: Pubkey,
    expected_upgrade_authority: Pubkey,
    authority_keypair: PathBuf,
    fee_payer: Pubkey,
    fee_payer_keypair: PathBuf,
    elf_path: PathBuf,
    checked_release_gate_path: PathBuf,
    expected_checked_release_gate_sha256: String,
    expected_source_revision: String,
    expected_source_tree_sha256: String,
    baseline_path: PathBuf,
    receipt_path: PathBuf,
    dump_path: PathBuf,
    solana_cli: PathBuf,
    target_acknowledgment: String,
    exclusive_payer_window_acknowledgment: Option<String>,
    execute: bool,
    preflight: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineArgsV1 {
    origin: ClusterOriginV1,
    role: String,
    program_id: Pubkey,
    programdata_id: Pubkey,
    expected_upgrade_authority: Pubkey,
    minimum_context_slot: u64,
    target_live_elf_bytes: u64,
    output_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExtensionArgsV1 {
    origin: ClusterOriginV1,
    role: String,
    program_id: Pubkey,
    programdata_id: Pubkey,
    expected_upgrade_authority: Pubkey,
    authority_keypair: PathBuf,
    fee_payer: Pubkey,
    fee_payer_keypair: PathBuf,
    baseline_path: PathBuf,
    receipt_path: PathBuf,
    solana_cli: PathBuf,
    expected_solana_cli_version: String,
    target_acknowledgment: String,
    execute: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UpgradeSetArgsV1 {
    origin: ClusterOriginV1,
    journal_path: PathBuf,
    solana_cli: PathBuf,
}

/// A path and its exact raw-file digest. The journal deliberately does not
/// copy any fact owned by the referenced evidence document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SetPinnedFileV1 {
    canonical_path: String,
    sha256: String,
}

/// A receipt or dump reference. `sha256: null` means the evidence must not yet
/// exist. Progress is therefore never conferred by a boolean or a copied
/// poststate: the referenced one-role receipt remains the sole semantic owner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SetOptionalFileV1 {
    canonical_path: String,
    sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UpgradeSetRoleV1 {
    role: String,
    disposition: CheckedDeploymentDispositionV1,
    program_id: String,
    programdata_id: String,
    baseline: Option<SetPinnedFileV1>,
    receipt: SetOptionalFileV1,
    dump: SetOptionalFileV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UpgradeSetJournalV1 {
    schema: String,
    checked_release_gate: SetPinnedFileV1,
    source_revision: String,
    source_tree_sha256: String,
    devnet_genesis_hash: String,
    solana_cli_version: String,
    retained_upgrade_authority: String,
    fee_payer: String,
    infrastructure_carry_forward: SetPinnedFileV1,
    roles: Vec<UpgradeSetRoleV1>,
}

/// One account in the externally captured, single-finalized-context
/// infrastructure snapshot. `account: null` is authoritative absence and is
/// admitted only for the two derived staging PDAs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CarryForwardSnapshotAccountV1 {
    role: String,
    address: String,
    account: Option<CarryForwardAccountV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CarryForwardAccountV1 {
    lamports: u64,
    owner: String,
    executable: bool,
    rent_epoch: u64,
    data_encoding: String,
    data_len: usize,
    data_base64: String,
    data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CarryForwardSnapshotV1 {
    schema: String,
    endpoint: String,
    commitment: String,
    rpc_method: String,
    context_slot: u64,
    accounts: Vec<CarryForwardSnapshotAccountV1>,
}

#[derive(Clone, Debug)]
struct AuthenticatedCarryForwardV1 {
    pin: CheckedInfrastructureCarryForwardPinV1,
    registry: CheckedUpgradeRolePinV1,
    rent: CheckedUpgradeRolePinV1,
    addresses: Vec<Pubkey>,
    accounts: Vec<Option<RpcAccountV1>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SetRoleStatusV1 {
    CarriedForward,
    Complete,
    Prepared,
    Submitted,
    AwaitingExtensionAndFreshBaseline,
    ReadyForUpgrade,
    WaitingForEarlierRole,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SetRoleAuditV1 {
    role: String,
    disposition: CheckedDeploymentDispositionV1,
    program_id: String,
    programdata_id: String,
    baseline: Option<SetPinnedFileV1>,
    receipt: SetOptionalFileV1,
    dump: SetOptionalFileV1,
    status: SetRoleStatusV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SetNextRoleV1 {
    ordinal: u8,
    role: String,
    program_id: String,
    programdata_id: String,
    action: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UpgradeSetAuditV1 {
    schema: String,
    journal_sha256: String,
    checked_release_gate: SetPinnedFileV1,
    source_revision: String,
    source_tree_sha256: String,
    devnet_genesis_hash: String,
    solana_cli_version: String,
    retained_upgrade_authority: String,
    fee_payer: String,
    infrastructure_carry_forward: SetPinnedFileV1,
    roles: Vec<SetRoleAuditV1>,
    completed_role_count: u8,
    next_role: Option<SetNextRoleV1>,
    final_set_sha256: Option<String>,
    mutation_permitted: bool,
}

struct SetLocalRoleV1 {
    args: UpgradeArgsV1,
    admission: UpgradeAdmissionV1,
    receipt_phase: Option<ReceiptPhaseV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GateFileV1 {
    canonical_path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CheckedGateLinkV1 {
    label: String,
    package: String,
    build_log: GateFileV1,
    compile_marker: String,
    sbf_diagnostics_count: u64,
    frame_build_log: GateFileV1,
    frame_compile_marker: String,
    frame_report: GateFileV1,
    frame_count: u64,
    frame_bound_bytes: u64,
    frames_at_or_over_bound: u64,
    deepest_frame_bytes: u64,
    elf: Option<GateFileV1>,
    checked_manifest: Option<GateFileV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CheckedUpgradeGateV1 {
    schema: String,
    source_revision: String,
    source_tree_sha256: String,
    solana_cli_version: String,
    build_run_id: String,
    link_count: u64,
    source_tree_manifest: GateFileV1,
    build_links_manifest: GateFileV1,
    build_run_manifest: GateFileV1,
    diagnostics_manifest: GateFileV1,
    links: Vec<CheckedGateLinkV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidatedUpgradeGateV1 {
    gate_sha256: String,
    source_revision: String,
    source_tree_sha256: String,
    solana_cli_version: String,
    raw_elf: Vec<u8>,
    raw_elf_sha256: String,
}

struct UpgradeAdmissionV1 {
    gate: ValidatedUpgradeGateV1,
    baseline: UpgradeBaselineV1,
    candidate_live: Vec<u8>,
    live_elf_sha256: String,
    live_elf_padding_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReceiptPhaseV1 {
    Prepared,
    Submitted,
    Complete,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LoaderObservationV1 {
    program_lamports: u64,
    program_owner: String,
    program_executable: bool,
    program_data_bytes: u64,
    program_account_sha256: String,
    programdata_lamports: u64,
    programdata_owner: String,
    programdata_executable: bool,
    programdata_data_bytes: u64,
    deployment_slot: u64,
    upgrade_authority: String,
    live_elf_bytes: u64,
    live_elf_sha256: String,
    programdata_account_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UpgradeBaselineV1 {
    schema: String,
    canonical_role_order: Vec<String>,
    role_ordinal: u8,
    role: String,
    program_id: String,
    programdata_id: String,
    expected_upgrade_authority: String,
    rpc_origin_redacted: String,
    genesis_hash: String,
    context_slot: u64,
    observation: LoaderObservationV1,
    target_live_elf_bytes: u64,
    extension_additional_bytes: u64,
    current_rent_exempt_minimum_lamports: u64,
    target_rent_exempt_minimum_lamports: u64,
    extension_lamport_top_up: u64,
    baseline_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UpgradeArithmeticV1 {
    transaction_payer_pre_lamports: u64,
    transaction_payer_post_lamports: u64,
    transaction_fee_lamports: u64,
    payer_fee_delta_lamports: u64,
    programdata_before_lamports: u64,
    programdata_after_lamports: u64,
    programdata_delta_lamports: i128,
    operation_wallet_before_lamports: u64,
    operation_wallet_after_lamports: u64,
    operation_observed_net_spend_lamports: u64,
    unattributed_cli_net_cost_lamports: u64,
    accounting_scope: String,
    cost_attribution: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExtensionArithmeticV1 {
    wallet_before_lamports: u64,
    wallet_after_lamports: u64,
    wallet_spend_lamports: u64,
    rent_top_up_lamports: u64,
    observed_fee_and_cli_cost_lamports: u64,
    programdata_before_lamports: u64,
    programdata_after_lamports: u64,
    programdata_delta_lamports: u64,
    programdata_before_bytes: u64,
    programdata_after_bytes: u64,
    extension_additional_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExtensionReceiptV1 {
    schema: String,
    phase: ReceiptPhaseV1,
    operation_id: String,
    role: String,
    program_id: String,
    programdata_id: String,
    retained_upgrade_authority: String,
    fee_payer: String,
    rpc_origin_redacted: String,
    genesis_hash: String,
    baseline_sha256: String,
    baseline_context_slot: u64,
    solana_cli_version: String,
    extension_additional_bytes: u64,
    target_rent_exempt_minimum_lamports: u64,
    expected_rent_top_up_lamports: u64,
    before_context_slot: u64,
    before: LoaderObservationV1,
    wallet_before_lamports: u64,
    transaction_signature: Option<String>,
    solana_cli_output: Option<Value>,
    finalized_transaction: Option<Value>,
    finalized_transaction_sha256: Option<String>,
    after_context_slot: Option<u64>,
    after: Option<LoaderObservationV1>,
    arithmetic: Option<ExtensionArithmeticV1>,
    receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UpgradeReceiptV1 {
    schema: String,
    phase: ReceiptPhaseV1,
    operation_id: String,
    role: String,
    program_id: String,
    programdata_id: String,
    retained_upgrade_authority: String,
    fee_payer: String,
    rpc_origin_redacted: String,
    genesis_hash: String,
    source_revision: String,
    source_tree_sha256: String,
    checked_release_gate_sha256: String,
    baseline_sha256: String,
    baseline_context_slot: u64,
    raw_elf_sha256: String,
    live_elf_sha256: String,
    live_elf_padding_bytes: u64,
    solana_cli_version: String,
    before_context_slot: u64,
    before: LoaderObservationV1,
    wallet_before_lamports: u64,
    exclusive_payer_window_acknowledgment: String,
    transaction_signature: Option<String>,
    solana_cli_output: Option<Value>,
    finalized_transaction: Option<Value>,
    finalized_transaction_sha256: Option<String>,
    after_context_slot: Option<u64>,
    after: Option<LoaderObservationV1>,
    arithmetic: Option<UpgradeArithmeticV1>,
    dump_sha256: Option<String>,
    dump_shape: Option<String>,
    receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UpgradePreflightV1 {
    schema: String,
    role: String,
    program_id: String,
    programdata_id: String,
    retained_upgrade_authority: String,
    fee_payer: String,
    rpc_origin_redacted: String,
    genesis_hash: String,
    solana_cli_version: String,
    source_revision: String,
    source_tree_sha256: String,
    checked_release_gate_sha256: String,
    baseline_sha256: String,
    observation_context_slot: u64,
    observation: LoaderObservationV1,
    wallet_lamports: u64,
    raw_elf_sha256: String,
    live_elf_sha256: String,
    live_elf_padding_bytes: u64,
    receipt_phase: Option<ReceiptPhaseV1>,
    mutation_permitted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RpcAccountV1 {
    lamports: u64,
    owner: Pubkey,
    executable: bool,
    rent_epoch: u64,
    data: Vec<u8>,
}

impl From<RpcAccount> for RpcAccountV1 {
    fn from(account: RpcAccount) -> Self {
        Self {
            lamports: account.lamports,
            owner: account.owner,
            executable: account.executable,
            rent_epoch: account.rent_epoch,
            data: account.data,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SnapshotV1 {
    context_slot: u64,
    loader: LoaderObservationV1,
    wallet_lamports: u64,
    live_elf: Vec<u8>,
}

struct SnapshotQueryV1<'a> {
    origin: &'a ClusterOriginV1,
    program_id: Pubkey,
    programdata_id: Pubkey,
    expected_upgrade_authority: Pubkey,
    payer: Pubkey,
    minimum_context_slot: u64,
}

struct CarryForwardQueryV1<'a> {
    origin: &'a ClusterOriginV1,
    addresses: &'a [Pubkey],
    minimum_context_slot: u64,
}

struct UpgradeTransactionQueryV1<'a> {
    origin: &'a ClusterOriginV1,
    signature: &'a str,
    program_id: Pubkey,
    programdata_id: Pubkey,
    authority: Pubkey,
    payer: Pubkey,
    deployment_slot: u64,
    programdata_before_lamports: u64,
    programdata_after_lamports: u64,
}

#[derive(Clone, Debug)]
struct UpgradeTransactionEvidenceV1 {
    transaction: Value,
    transaction_sha256: String,
    fee_lamports: u64,
    payer_pre_lamports: u64,
    payer_post_lamports: u64,
    programdata_pre_lamports: u64,
    programdata_post_lamports: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CliOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

struct ExtensionTransactionQueryV1<'a> {
    origin: &'a ClusterOriginV1,
    program_id: Pubkey,
    programdata_id: Pubkey,
    authority: Pubkey,
    payer: Pubkey,
    additional_bytes: u64,
    deployment_slot: u64,
    programdata_before_lamports: u64,
    programdata_after_lamports: u64,
    wallet_before_lamports: u64,
    wallet_after_lamports: u64,
}

#[derive(Clone, Debug)]
struct ExtensionTransactionEvidenceV1 {
    signature: String,
    transaction: Value,
    fee_lamports: u64,
    payer_spend_lamports: u64,
    programdata_delta_lamports: u64,
}

trait CliRunner {
    fn run(&mut self, arguments: &[String]) -> Result<CliOutput>;

    fn read_snapshot(&mut self, _query: &SnapshotQueryV1<'_>) -> Result<SnapshotV1> {
        Err(Error::new(
            "CLI runner does not provide one-context finalized account observation",
        ))
    }

    fn read_carry_forward_accounts(
        &mut self,
        _query: &CarryForwardQueryV1<'_>,
    ) -> Result<(u64, Vec<Option<RpcAccountV1>>)> {
        Err(Error::new(
            "CLI runner does not provide one-context carry-forward account observation",
        ))
    }

    fn resolve_upgrade_transaction(
        &mut self,
        _query: &UpgradeTransactionQueryV1<'_>,
    ) -> Result<UpgradeTransactionEvidenceV1> {
        Err(Error::new(
            "CLI runner does not provide finalized Upgrade transaction resolution",
        ))
    }

    fn resolve_extension_transaction(
        &mut self,
        _query: &ExtensionTransactionQueryV1<'_>,
    ) -> Result<ExtensionTransactionEvidenceV1> {
        Err(Error::new(
            "CLI runner does not provide finalized extension transaction resolution",
        ))
    }
}

struct SystemCliRunner {
    executable: PathBuf,
}

impl CliRunner for SystemCliRunner {
    fn run(&mut self, arguments: &[String]) -> Result<CliOutput> {
        let output = Command::new(&self.executable).args(arguments).output()?;
        Ok(CliOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn read_snapshot(&mut self, query: &SnapshotQueryV1<'_>) -> Result<SnapshotV1> {
        read_snapshot_via_rpc(query)
    }

    fn read_carry_forward_accounts(
        &mut self,
        query: &CarryForwardQueryV1<'_>,
    ) -> Result<(u64, Vec<Option<RpcAccountV1>>)> {
        let mut rpc = Rpc::connect_cluster(query.origin, WritePolicyV1::ReadsOnly)?;
        let (slot, accounts) =
            rpc.finalized_accounts(query.addresses, query.minimum_context_slot)?;
        Ok((
            slot,
            accounts
                .into_iter()
                .map(|account| account.map(RpcAccountV1::from))
                .collect(),
        ))
    }

    fn resolve_upgrade_transaction(
        &mut self,
        query: &UpgradeTransactionQueryV1<'_>,
    ) -> Result<UpgradeTransactionEvidenceV1> {
        resolve_upgrade_transaction_via_rpc(query)
    }

    fn resolve_extension_transaction(
        &mut self,
        query: &ExtensionTransactionQueryV1<'_>,
    ) -> Result<ExtensionTransactionEvidenceV1> {
        resolve_extension_transaction_via_rpc(query)
    }
}

/// Parse and execute the versioned command with the real Solana CLI.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let args = parse_args(arguments)?;
    let mut runner = SystemCliRunner {
        executable: args.solana_cli.clone(),
    };
    let mut stdout = std::io::stdout().lock();
    if args.preflight {
        let report = preflight_with_runner(&args, &mut runner)?;
        serde_json::to_writer_pretty(&mut stdout, &report)?;
    } else {
        let receipt = execute_with_runner(&args, &mut runner)?;
        serde_json::to_writer_pretty(&mut stdout, &receipt)?;
    }
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Capture one finalized, key-free Program/ProgramData baseline.
pub(crate) fn run_baseline(arguments: Vec<String>) -> Result<()> {
    let args = parse_baseline_args(arguments)?;
    if args.output_path.exists() {
        return Err(Error::new(format!(
            "baseline output {} already exists; refusing to overwrite evidence",
            args.output_path.display()
        )));
    }
    let mut rpc = Rpc::connect_cluster(&args.origin, WritePolicyV1::ReadsOnly)?;
    let (context_slot, accounts) = rpc.finalized_accounts(
        &[args.program_id, args.programdata_id],
        args.minimum_context_slot,
    )?;
    let mut accounts = accounts.into_iter();
    let program = accounts
        .next()
        .flatten()
        .ok_or_else(|| Error::new(format!("missing Program account {}", args.program_id)))?;
    let programdata = accounts.next().flatten().ok_or_else(|| {
        Error::new(format!(
            "missing ProgramData account {}",
            args.programdata_id
        ))
    })?;
    let program = RpcAccountV1::from(program);
    let programdata = RpcAccountV1::from(programdata);
    let observation = loader_observation(
        args.program_id,
        args.programdata_id,
        args.expected_upgrade_authority,
        &program,
        &programdata,
    )?;
    let current_space = observation.programdata_data_bytes;
    let target_live_elf_bytes = args.target_live_elf_bytes.max(observation.live_elf_bytes);
    let candidate_space = target_live_elf_bytes
        .checked_add(45)
        .ok_or_else(|| Error::new("target ProgramData width overflow"))?;
    let target_space = current_space.max(candidate_space);
    let extension_additional_bytes = target_space
        .checked_sub(current_space)
        .ok_or_else(|| Error::new("extension width underflow"))?;
    let current_rent_exempt_minimum_lamports = rpc.minimum_balance(
        usize::try_from(current_space)
            .map_err(|_| Error::new("current ProgramData width does not fit this host"))?,
    )?;
    let target_rent_exempt_minimum_lamports = rpc.minimum_balance(
        usize::try_from(target_space)
            .map_err(|_| Error::new("target ProgramData width does not fit this host"))?,
    )?;
    let extension_lamport_top_up =
        target_rent_exempt_minimum_lamports.saturating_sub(observation.programdata_lamports);
    let mut baseline = UpgradeBaselineV1 {
        schema: BASELINE_SCHEMA.into(),
        canonical_role_order: ROLES.iter().map(|role| (*role).into()).collect(),
        role_ordinal: role_ordinal(&args.role)?,
        role: args.role,
        program_id: args.program_id.to_string(),
        programdata_id: args.programdata_id.to_string(),
        expected_upgrade_authority: args.expected_upgrade_authority.to_string(),
        rpc_origin_redacted: args.origin.redacted_url(),
        genesis_hash: DEVNET_GENESIS_HASH.into(),
        context_slot,
        observation,
        target_live_elf_bytes,
        extension_additional_bytes,
        current_rent_exempt_minimum_lamports,
        target_rent_exempt_minimum_lamports,
        extension_lamport_top_up,
        baseline_sha256: String::new(),
    };
    baseline.baseline_sha256 = baseline_digest(&baseline)?;
    write_json_atomic_new(&args.output_path, &baseline)?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &baseline)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Extend one existing ProgramData allocation as a separately acknowledged act.
pub(crate) fn run_extension(arguments: Vec<String>) -> Result<()> {
    let args = parse_extension_args(arguments)?;
    let mut runner = SystemCliRunner {
        executable: args.solana_cli.clone(),
    };
    let receipt = execute_extension_with_runner(&args, &mut runner)?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &receipt)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Audit the ordered seven-program Upgrade set without opening a keypair,
/// submitting a transaction, or writing an evidence file.
pub(crate) fn run_set_journal(arguments: Vec<String>) -> Result<()> {
    let args = parse_set_args(arguments)?;
    let mut runner = SystemCliRunner {
        executable: args.solana_cli.clone(),
    };
    let report = audit_set_journal_with_runner(&args, &mut runner)?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap devnet-upgrade-baseline-v1 --rpc-url HTTPS_URL \\
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\
     --role ROLE --program-id PUBKEY --programdata-id PUBKEY \\
     --expected-upgrade-authority PUBKEY --minimum-context-slot U64 \\
     --target-live-elf-bytes U64 --output ABSOLUTE_JSON\n\n\
     You capture one key-free, read-only, finalized Program/ProgramData baseline with one bounded \
     getMultipleAccounts call after the health and exact-genesis admission checks. Its canonical \
     hash commits the complete seven-role order, selected role, genesis, context slot, addresses, \
     owners, executable flags, byte lengths, account digests, deployment slot, live digest, and \
     retained authority. It also records the exact old/target rent minima and top-up for the \
     candidate live width.\n\n\
  dclutch-local-successor-bootstrap devnet-upgrade-extend-v1 --rpc-url HTTPS_URL \\
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\
     --role ROLE --program-id PUBKEY --programdata-id PUBKEY \\
     --expected-upgrade-authority PUBKEY --authority-keypair ABSOLUTE_JSON \\
     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_JSON --baseline ABSOLUTE_JSON \\
     --receipt ABSOLUTE_JSON --solana-cli ABSOLUTE_EXECUTABLE \\
     --expected-solana-cli-version EXACT_STRING \\
     --i-accept-extension ROLE:PUBKEY:+ADDITIONAL_BYTES --execute\n\n\
     You run this separate act only when the checked candidate is larger than ProgramData's \
     current `space - 45`. It refuses a zero extension, a stale baseline, implicit realloc, \
     insufficient payer balance, or replay ambiguity. It records the pre/post space, lamports, \
     slot, authority, signature, and checked wallet arithmetic. After it completes, capture a new \
     baseline: the Upgrade refuses the old one, and new release records must bind the resulting \
     deployment slot.\n\n\
  dclutch-local-successor-bootstrap devnet-upgrade-v1 --rpc-url HTTPS_URL \\
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\
     --role ROLE --program-id PUBKEY --programdata-id PUBKEY \\
     --expected-upgrade-authority PUBKEY --authority-keypair ABSOLUTE_JSON \\
     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_JSON --elf ABSOLUTE_SO \\
     --checked-release-gate ABSOLUTE_JSON \\
     --expected-checked-release-gate-sha256 SHA256 \\
     --expected-source-revision GIT_COMMIT --expected-source-tree-sha256 SHA256 \\
     --baseline ABSOLUTE_JSON \\
     --receipt ABSOLUTE_JSON --dump ABSOLUTE_SO \\
     --solana-cli ABSOLUTE_EXECUTABLE --i-accept-upgrade ROLE:PUBKEY \
     (--preflight | --i-kept-fee-payer-exclusive ROLE:PUBKEY:PAYER --execute)\n\n\
     With --preflight, you run the complete gate, source, baseline, CLI-version, devnet-genesis, \
     and one-context Program/ProgramData/payer admission without opening either keypair path, \
     writing a receipt or dump, or invoking a program command. With --execute, you update exactly \
     one existing decision-0012 devnet program. Execute also requires the exact \
     ROLE:PUBKEY:PAYER exclusive-window acknowledgment. The genesis hash names the cluster, \
     ROLE:PUBKEY names the permanent program you accept updating, and the payer acknowledgment \
     says no unrelated transaction may spend that wallet until the finalized post-observation. \
     The command checks the Loader-v3 Program link, the \
     non-executable ProgramData account, every fact in the canonical baseline, the retained \
     authority and both keypair addresses immediately before \
     it invokes `solana program deploy`. The generated checked-release gate binds the exact source \
     commit/tree, all thirteen fresh compile logs, all thirteen zero frame reports, and every \
     release ELF. The selected ELF must be the gate's canonical regular file. The command refuses \
     handwritten acceptance, path escape, symlinks, missing links, and changed evidence. It pads \
     the checked raw ELF with zeros only to the separately captured baseline width. After the CLI \
     returns, the command \
     re-checks devnet, requires the deployment slot to advance, resolves the exact CLI-returned \
     finalized transaction, proves that transaction's payer delta equals its fee and ProgramData \
     rent is unchanged, and separately bridges the wallet before the opaque CLI invocation to \
     the finalized wallet after it. The receipt calls the difference between that observed net \
     spend and the final Upgrade fee `unattributed_cli_net_cost_lamports`; buffer lifecycle fees \
     and rent refunds are not individually attributed. It dumps the deployed bytes, verifies \
     them, and completes an atomic digest-bound \
     receipt. Every complete resume freshly rechecks CLI identity, devnet, current Loader state, \
     transaction, payload, authority, deployment slot, and dump. There is no force, recycle, \
     close, set-upgrade-authority, mainnet, testnet, unknown cluster, multi-role, or \
     implicit-wallet path.\n\n\
  dclutch-local-successor-bootstrap devnet-deployment-set-journal-v2 \
     --rpc-url HTTPS_URL \
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
     --journal ABSOLUTE_JSON --solana-cli ABSOLUTE_EXECUTABLE\n\n\
     This key-free command audits, but never performs, the canonical mixed deployment set. Its \
     immutable journal pins one checked gate, source commit/tree, exact devnet genesis, CLI \
     identity, retained authority, fee payer, and seven ordered tagged Program/ProgramData rows. \
     Registry and Rent must be CarryForward under one exact finalized nine-account snapshot and \
     exact live dumps; Custody, Resolution, Claims, Trading, and Core must be receipt-backed \
     Upgrades. Every invocation freshly revalidates the complete CarryForward closure and each \
     completed one-role receipt. Gaps, tag swaps, changed earlier roles, and stale extension \
     baselines refuse. Until complete it reports exactly one next Upgrade role; only two fresh \
     carries plus five fresh receipts produce the v2 final digest. It has no loop, signing, write, \
     execute, or keypair mode."
}

fn parse_set_args(arguments: Vec<String>) -> Result<UpgradeSetArgsV1> {
    let mut values = std::collections::BTreeMap::<String, String>::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if !matches!(
            argument.as_str(),
            "--rpc-url" | DEVNET_ACKNOWLEDGMENT_FLAG | "--journal" | "--solana-cli"
        ) {
            return Err(Error::new(format!(
                "unknown devnet-deployment-set-journal-v2 argument: {argument}"
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let take = |label: &str| {
        values
            .get(label)
            .cloned()
            .ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let rpc_url = take("--rpc-url")?;
    let acknowledgment = take(DEVNET_ACKNOWLEDGMENT_FLAG)?;
    let origin = ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?;
    if !matches!(origin, ClusterOriginV1::AcknowledgedDevnet { .. }) {
        return Err(Error::new(
            "devnet-deployment-set-journal-v2 admits exact public devnet only",
        ));
    }
    Ok(UpgradeSetArgsV1 {
        origin,
        journal_path: absolute(&take("--journal")?, "--journal")?,
        solana_cli: absolute(&take("--solana-cli")?, "--solana-cli")?,
    })
}

fn audit_set_journal_with_runner(
    args: &UpgradeSetArgsV1,
    runner: &mut impl CliRunner,
) -> Result<UpgradeSetAuditV1> {
    let (journal, journal_sha256) = load_set_journal(args)?;
    let carry = authenticate_carry_forward(&journal)?;
    let authority = parse_pubkey(
        &journal.retained_upgrade_authority,
        "deployment-set retained authority",
    )?;
    let payer = parse_pubkey(&journal.fee_payer, "deployment-set fee payer")?;
    let gate_path = PathBuf::from(&journal.checked_release_gate.canonical_path);
    let upgrade_rows = &journal.roles[2..];

    if let Some(first_unpinned) = upgrade_rows
        .iter()
        .position(|role| role.receipt.sha256.is_none())
        && let Some(later) = upgrade_rows
            .iter()
            .skip(first_unpinned + 1)
            .find(|role| role.receipt.sha256.is_some() || role.dump.sha256.is_some())
    {
        return Err(Error::new(format!(
            "deployment-set Upgrade gap: role {} pins evidence after unstarted role {}",
            later.role, upgrade_rows[first_unpinned].role
        )));
    }

    let mut local_roles = Vec::with_capacity(upgrade_rows.len());
    for role in upgrade_rows {
        let baseline = role.baseline.as_ref().expect("mixed closure checked");
        let elf_path = set_role_elf_path(&gate_path, &role.role)?;
        let role_args = UpgradeArgsV1 {
            origin: args.origin.clone(),
            role: role.role.clone(),
            program_id: parse_pubkey(&role.program_id, "deployment-set Program")?,
            programdata_id: parse_pubkey(&role.programdata_id, "deployment-set ProgramData")?,
            expected_upgrade_authority: authority,
            authority_keypair: PathBuf::from("/dclutch-set-journal-never-reads-authority"),
            fee_payer: payer,
            fee_payer_keypair: PathBuf::from("/dclutch-set-journal-never-reads-payer"),
            elf_path,
            checked_release_gate_path: gate_path.clone(),
            expected_checked_release_gate_sha256: journal.checked_release_gate.sha256.clone(),
            expected_source_revision: journal.source_revision.clone(),
            expected_source_tree_sha256: journal.source_tree_sha256.clone(),
            baseline_path: PathBuf::from(&baseline.canonical_path),
            receipt_path: PathBuf::from(&role.receipt.canonical_path),
            dump_path: PathBuf::from(&role.dump.canonical_path),
            solana_cli: args.solana_cli.clone(),
            target_acknowledgment: format!("{}:{}", role.role, role.program_id),
            exclusive_payer_window_acknowledgment: Some(format!(
                "{}:{}:{}",
                role.role, role.program_id, journal.fee_payer
            )),
            execute: false,
            preflight: true,
        };
        let admission = admit_upgrade(&role_args)?;
        if admission.gate.solana_cli_version != journal.solana_cli_version {
            return Err(Error::new(format!(
                "deployment-set CLI identity {:?} differs from checked gate {:?}",
                journal.solana_cli_version, admission.gate.solana_cli_version
            )));
        }
        let receipt_phase = match &role.receipt.sha256 {
            Some(_) => Some(
                load_receipt(&role_args.receipt_path)?
                    .ok_or_else(|| Error::new("pinned deployment-set receipt disappeared"))?
                    .phase,
            ),
            None => None,
        };
        match (&receipt_phase, &role.dump.sha256) {
            (Some(ReceiptPhaseV1::Complete), Some(_)) => {}
            (Some(ReceiptPhaseV1::Complete), None) => {
                return Err(Error::new(format!(
                    "completed deployment-set role {} omitted its dump digest",
                    role.role
                )));
            }
            (Some(_), Some(_)) => {
                return Err(Error::new(format!(
                    "incomplete deployment-set role {} claims a dump",
                    role.role
                )));
            }
            (None, Some(_)) => {
                return Err(Error::new(format!(
                    "deployment-set role {} claims a dump without a receipt",
                    role.role
                )));
            }
            _ => {}
        }
        local_roles.push(SetLocalRoleV1 {
            args: role_args,
            admission,
            receipt_phase,
        });
    }

    let first_incomplete = local_roles
        .iter()
        .position(|role| role.receipt_phase != Some(ReceiptPhaseV1::Complete));
    if let Some(first) = first_incomplete {
        for later in local_roles.iter().skip(first + 1) {
            if later.receipt_phase.is_some() {
                return Err(Error::new(format!(
                    "deployment-set Upgrade gap: role {} has a receipt after incomplete role {}",
                    later.args.role, local_roles[first].args.role
                )));
            }
        }
    }

    // Pin CLI identity and exact devnet genesis before the CarryForward network
    // read. There are always five Upgrade rows under the mixed closure.
    let cli_version = invoke(runner, &local_roles[0].args, &["--version".into()])?;
    if cli_version.stdout.trim() != journal.solana_cli_version {
        return Err(Error::new("deployment-set Solana CLI identity drifted"));
    }
    authenticate_devnet(runner, &local_roles[0].args)?;
    // CarryForward is one atomic finalized observation. Revalidate all nine
    // accounts on every invocation before reporting progress for any Upgrade.
    require_fresh_carry_forward(&carry, args, runner)?;
    let completed_upgrades = first_incomplete.unwrap_or(local_roles.len());
    for role in local_roles.iter().take(completed_upgrades) {
        let report = preflight_with_runner(&role.args, runner)?;
        if report.receipt_phase != Some(ReceiptPhaseV1::Complete) {
            return Err(Error::new(
                "deployment-set completed prefix did not yield a complete one-role preflight",
            ));
        }
    }
    if let Some(index) = first_incomplete {
        let role = &local_roles[index];
        if role.receipt_phase.is_none() && role.admission.baseline.extension_additional_bytes != 0 {
            audit_extension_pending_role(role, &journal.solana_cli_version, runner)?;
        } else {
            let report = preflight_with_runner(&role.args, runner)?;
            if report.receipt_phase != role.receipt_phase {
                return Err(Error::new(
                    "deployment-set receipt phase changed during read-only audit",
                ));
            }
        }
    }

    let mut role_reports = Vec::with_capacity(journal.roles.len());
    for role in &journal.roles[..2] {
        role_reports.push(SetRoleAuditV1 {
            role: role.role.clone(),
            disposition: role.disposition,
            program_id: role.program_id.clone(),
            programdata_id: role.programdata_id.clone(),
            baseline: None,
            receipt: role.receipt.clone(),
            dump: role.dump.clone(),
            status: SetRoleStatusV1::CarriedForward,
        });
    }
    for (relative, (role, local)) in upgrade_rows.iter().zip(&local_roles).enumerate() {
        let status = if relative < completed_upgrades {
            SetRoleStatusV1::Complete
        } else if Some(relative) != first_incomplete {
            SetRoleStatusV1::WaitingForEarlierRole
        } else {
            match local.receipt_phase {
                Some(ReceiptPhaseV1::Prepared) => SetRoleStatusV1::Prepared,
                Some(ReceiptPhaseV1::Submitted) => SetRoleStatusV1::Submitted,
                Some(ReceiptPhaseV1::Complete) => {
                    return Err(Error::new("complete next role escaped completed prefix"));
                }
                None if local.admission.baseline.extension_additional_bytes != 0 => {
                    SetRoleStatusV1::AwaitingExtensionAndFreshBaseline
                }
                None => SetRoleStatusV1::ReadyForUpgrade,
            }
        };
        role_reports.push(SetRoleAuditV1 {
            role: role.role.clone(),
            disposition: role.disposition,
            program_id: role.program_id.clone(),
            programdata_id: role.programdata_id.clone(),
            baseline: role.baseline.clone(),
            receipt: role.receipt.clone(),
            dump: role.dump.clone(),
            status,
        });
    }
    let next_role = first_incomplete
        .map(|relative| {
            let absolute = relative + 2;
            let role = &journal.roles[absolute];
            let local = &local_roles[relative];
            let action = match local.receipt_phase {
                Some(ReceiptPhaseV1::Prepared | ReceiptPhaseV1::Submitted) => {
                    "resume_exact_one_role_upgrade"
                }
                Some(ReceiptPhaseV1::Complete) => {
                    return Err(Error::new("complete role cannot be next"));
                }
                None if local.admission.baseline.extension_additional_bytes != 0 => {
                    "extend_then_capture_fresh_baseline"
                }
                None => "run_exact_one_role_upgrade",
            };
            Ok(SetNextRoleV1 {
                ordinal: u8::try_from(absolute).expect("seven roles fit u8"),
                role: role.role.clone(),
                program_id: role.program_id.clone(),
                programdata_id: role.programdata_id.clone(),
                action: action.into(),
            })
        })
        .transpose()?;
    let final_set_sha256 = if first_incomplete.is_none() {
        Some(final_set_digest(&journal)?)
    } else {
        None
    };
    Ok(UpgradeSetAuditV1 {
        schema: SET_AUDIT_SCHEMA.into(),
        journal_sha256,
        checked_release_gate: journal.checked_release_gate,
        source_revision: journal.source_revision,
        source_tree_sha256: journal.source_tree_sha256,
        devnet_genesis_hash: journal.devnet_genesis_hash,
        solana_cli_version: journal.solana_cli_version,
        retained_upgrade_authority: journal.retained_upgrade_authority,
        fee_payer: journal.fee_payer,
        infrastructure_carry_forward: journal.infrastructure_carry_forward,
        roles: role_reports,
        completed_role_count: u8::try_from(completed_upgrades + 2).expect("seven roles fit u8"),
        next_role,
        final_set_sha256,
        mutation_permitted: false,
    })
}

fn audit_extension_pending_role(
    role: &SetLocalRoleV1,
    expected_cli_version: &str,
    runner: &mut impl CliRunner,
) -> Result<()> {
    if role.receipt_phase.is_some()
        || role.admission.baseline.extension_additional_bytes == 0
        || role.args.receipt_path.exists()
        || role.args.dump_path.exists()
    {
        return Err(Error::new(
            "extension-pending set role has contradictory receipt, dump, or width state",
        ));
    }
    let cli_version = invoke(runner, &role.args, &["--version".into()])?;
    if cli_version.stdout.trim() != expected_cli_version {
        return Err(Error::new(format!(
            "Solana CLI version is {:?}; set journal pins {:?}",
            cli_version.stdout.trim(),
            expected_cli_version
        )));
    }
    authenticate_devnet(runner, &role.args)?;
    let observation = read_snapshot(runner, &role.args, role.admission.baseline.context_slot)?;
    require_baseline_prestate(&role.admission.baseline, &observation.loader)?;
    Ok(())
}

fn load_set_journal(args: &UpgradeSetArgsV1) -> Result<(UpgradeSetJournalV1, String)> {
    load_set_journal_path(&args.journal_path)
}

fn load_set_journal_path(journal_path: &Path) -> Result<(UpgradeSetJournalV1, String)> {
    let journal_path = exact_reference_path(
        journal_path
            .to_str()
            .ok_or_else(|| Error::new("--journal path is not UTF-8"))?,
        "set journal",
    )?;
    let journal_bytes = read_regular_reference(&journal_path, "set journal")?;
    let journal_sha256 = digest(&journal_bytes);
    let journal: UpgradeSetJournalV1 = serde_json::from_slice(&journal_bytes).map_err(|error| {
        Error::new(format!(
            "set journal is not canonical {SET_JOURNAL_SCHEMA} JSON: {error}"
        ))
    })?;
    if journal.schema != SET_JOURNAL_SCHEMA
        || journal.devnet_genesis_hash != DEVNET_GENESIS_HASH
        || journal.solana_cli_version.trim().is_empty()
        || journal.solana_cli_version != journal.solana_cli_version.trim()
    {
        return Err(Error::new(
            "set journal schema, exact devnet genesis, or CLI identity is invalid",
        ));
    }
    require_lower_hex(&journal.source_revision, "set source revision", 40, 40)?;
    require_digest(&journal.source_tree_sha256, "set source tree SHA-256")?;
    let _ = parse_pubkey(
        &journal.retained_upgrade_authority,
        "set retained authority",
    )?;
    let _ = parse_pubkey(&journal.fee_payer, "set fee payer")?;
    if journal.roles.len() != PERMANENT_DEVNET_UPGRADE_TARGETS_V1.len() {
        return Err(Error::new(format!(
            "set journal must carry exactly {} ordered roles",
            PERMANENT_DEVNET_UPGRADE_TARGETS_V1.len()
        )));
    }
    let mut identities = BTreeSet::new();
    for (index, (role, (expected_role, expected_program, expected_programdata))) in journal
        .roles
        .iter()
        .zip(PERMANENT_DEVNET_UPGRADE_TARGETS_V1)
        .enumerate()
    {
        let program = parse_pubkey(&role.program_id, "set Program")?;
        let programdata = parse_pubkey(&role.programdata_id, "set ProgramData")?;
        if role.role != *expected_role
            || role_ordinal(&role.role)? != index as u8
            || program != parse_pubkey(expected_program, "permanent set Program")?
            || programdata != parse_pubkey(expected_programdata, "permanent set ProgramData")?
        {
            return Err(Error::new(format!(
                "set journal target at index {index} is not exact permanent devnet \
                {expected_role}:{expected_program}:{expected_programdata}"
            )));
        }
        let expected_disposition = if index < 2 {
            CheckedDeploymentDispositionV1::CarryForward
        } else {
            CheckedDeploymentDispositionV1::Upgrade
        };
        if role.disposition != expected_disposition {
            return Err(Error::new(format!(
                "set journal {expected_role} disposition is not the canonical mixed deployment-set choice"
            )));
        }
        if !identities.insert(program) || !identities.insert(programdata) {
            return Err(Error::new(
                "set journal Program/ProgramData identities are not fourteen unique accounts",
            ));
        }
    }
    // No referenced evidence is opened until all fourteen account identities
    // have matched the permanent table above.
    read_pinned_reference(&journal.checked_release_gate, "set checked-release gate")?;
    read_pinned_reference(
        &journal.infrastructure_carry_forward,
        "infrastructure carry-forward snapshot",
    )?;
    for role in &journal.roles {
        match role.disposition {
            CheckedDeploymentDispositionV1::CarryForward => {
                if role.baseline.is_some()
                    || role.receipt.sha256.is_some()
                    || role.dump.sha256.is_none()
                {
                    return Err(Error::new(format!(
                        "carry-forward role {} must contain one live dump and no Upgrade baseline or receipt evidence",
                        role.role
                    )));
                }
                read_optional_reference(&role.receipt, &format!("{} receipt", role.role))?;
            }
            CheckedDeploymentDispositionV1::Upgrade => {
                let baseline = role.baseline.as_ref().ok_or_else(|| {
                    Error::new(format!("Upgrade role {} omitted its baseline", role.role))
                })?;
                read_pinned_reference(baseline, &format!("{} baseline", role.role))?;
                read_optional_reference(&role.receipt, &format!("{} receipt", role.role))?;
            }
        }
        read_optional_reference(&role.dump, &format!("{} dump", role.role))?;
    }
    Ok((journal, journal_sha256))
}

fn read_pinned_reference(reference: &SetPinnedFileV1, label: &str) -> Result<Vec<u8>> {
    require_digest(&reference.sha256, &format!("{label} SHA-256"))?;
    let path = exact_reference_path(&reference.canonical_path, label)?;
    let bytes = read_regular_reference(&path, label)?;
    let observed = digest(&bytes);
    if observed != reference.sha256 {
        return Err(Error::new(format!(
            "{label} SHA-256 is {observed}, pinned {}",
            reference.sha256
        )));
    }
    Ok(bytes)
}

fn read_optional_reference(reference: &SetOptionalFileV1, label: &str) -> Result<Option<Vec<u8>>> {
    let path = exact_reference_path(&reference.canonical_path, label)?;
    match &reference.sha256 {
        Some(expected) => {
            require_digest(expected, &format!("{label} SHA-256"))?;
            let bytes = read_regular_reference(&path, label)?;
            let observed = digest(&bytes);
            if &observed != expected {
                return Err(Error::new(format!(
                    "{label} SHA-256 is {observed}, pinned {expected}"
                )));
            }
            Ok(Some(bytes))
        }
        None => {
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
                Ok(_) => {
                    return Err(Error::new(format!(
                        "{label} exists but the set journal pins no digest"
                    )));
                }
            }
            let parent = path
                .parent()
                .ok_or_else(|| Error::new(format!("{label} omitted a parent")))?;
            if fs::canonicalize(parent)? != parent {
                return Err(Error::new(format!(
                    "{label} parent is not an exact canonical directory"
                )));
            }
            Ok(None)
        }
    }
}

fn exact_reference_path(value: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.components().any(|component| {
            !matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
    {
        return Err(Error::new(format!(
            "{label} path must be absolute, normalized, and contain no escape"
        )));
    }
    Ok(path)
}

fn read_regular_reference(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        Error::new(format!(
            "{label} {} cannot be inspected: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::new(format!(
            "{label} must be one regular non-symlink file"
        )));
    }
    if fs::canonicalize(path)? != path {
        return Err(Error::new(format!(
            "{label} path is not canonical or traverses a symlink"
        )));
    }
    Ok(fs::read(path)?)
}

fn exact_account_sha256(account: &RpcAccountV1) -> String {
    let mut hasher = Sha256::new();
    hasher.update(account.owner.as_ref());
    hasher.update(account.lamports.to_le_bytes());
    hasher.update([u8::from(account.executable)]);
    hasher.update(account.rent_epoch.to_le_bytes());
    hasher.update(
        u64::try_from(account.data.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(&account.data);
    hex(&hasher.finalize())
}

fn decode_snapshot_account(row: &CarryForwardSnapshotAccountV1) -> Result<Option<RpcAccountV1>> {
    let Some(account) = &row.account else {
        return Ok(None);
    };
    if account.data_encoding != "base64" {
        return Err(Error::new(format!(
            "carry-forward {} account data is not canonical base64",
            row.role
        )));
    }
    let data = BASE64.decode(&account.data_base64).map_err(|_| {
        Error::new(format!(
            "carry-forward {} account data is invalid base64",
            row.role
        ))
    })?;
    if data.len() != account.data_len || digest(&data) != account.data_sha256 {
        return Err(Error::new(format!(
            "carry-forward {} account data length or digest mismatch",
            row.role
        )));
    }
    let decoded = RpcAccountV1 {
        lamports: account.lamports,
        owner: parse_pubkey(&account.owner, "carry-forward account owner")?,
        executable: account.executable,
        rent_epoch: account.rent_epoch,
        data,
    };
    if decoded.lamports < Rent::default().minimum_balance(decoded.data.len()) {
        return Err(Error::new(format!(
            "carry-forward {} account is not rent exempt",
            row.role
        )));
    }
    Ok(Some(decoded))
}

fn require_account_shape(
    role: &str,
    account: &RpcAccountV1,
    owner: Pubkey,
    executable: bool,
    data_len: Option<usize>,
) -> Result<()> {
    if account.owner != owner
        || account.executable != executable
        || data_len.is_some_and(|len| account.data.len() != len)
    {
        return Err(Error::new(format!(
            "carry-forward {role} owner/executable/length shape is invalid"
        )));
    }
    Ok(())
}

fn authenticate_carry_forward(
    journal: &UpgradeSetJournalV1,
) -> Result<AuthenticatedCarryForwardV1> {
    let snapshot_bytes = read_pinned_reference(
        &journal.infrastructure_carry_forward,
        "infrastructure carry-forward snapshot",
    )?;
    let snapshot: CarryForwardSnapshotV1 =
        serde_json::from_slice(&snapshot_bytes).map_err(|error| {
            Error::new(format!(
                "infrastructure carry-forward snapshot is not canonical v1 JSON: {error}"
            ))
        })?;
    if snapshot.schema != CARRY_FORWARD_SNAPSHOT_SCHEMA
        || snapshot.endpoint != "https://api.devnet.solana.com"
        || snapshot.commitment != "finalized"
        || snapshot.rpc_method != "getMultipleAccounts"
        || snapshot.context_slot == 0
    {
        return Err(Error::new(
            "carry-forward snapshot schema, endpoint, finality, RPC method, or context slot is invalid",
        ));
    }
    let expected_labels = [
        "registry_program",
        "registry_programdata",
        "rent_program",
        "rent_programdata",
        "registry_raw",
        "registry_staging",
        "rent_raw",
        "rent_staging",
        "infrastructure_profile",
    ];
    if snapshot.accounts.len() != expected_labels.len()
        || snapshot
            .accounts
            .iter()
            .zip(expected_labels)
            .any(|(row, expected)| row.role != expected)
    {
        return Err(Error::new(
            "carry-forward snapshot does not contain the exact ordered nine-account closure",
        ));
    }
    if snapshot.accounts[5].account.is_some() || snapshot.accounts[7].account.is_some() {
        return Err(Error::new(
            "carry-forward staging must be finalized account absence, not a fabricated empty account",
        ));
    }
    let addresses = snapshot
        .accounts
        .iter()
        .map(|row| parse_pubkey(&row.address, "carry-forward account address"))
        .collect::<Result<Vec<_>>>()?;
    let accounts = snapshot
        .accounts
        .iter()
        .map(decode_snapshot_account)
        .collect::<Result<Vec<_>>>()?;
    let required = |index: usize, label: &str| {
        accounts[index]
            .as_ref()
            .ok_or_else(|| Error::new(format!("carry-forward snapshot omitted {label}")))
    };
    let registry_program = parse_pubkey(PERMANENT_DEVNET_UPGRADE_TARGETS_V1[0].1, "Registry")?;
    let registry_programdata = parse_pubkey(
        PERMANENT_DEVNET_UPGRADE_TARGETS_V1[0].2,
        "Registry ProgramData",
    )?;
    let rent_program = parse_pubkey(PERMANENT_DEVNET_UPGRADE_TARGETS_V1[1].1, "Rent")?;
    let rent_programdata =
        parse_pubkey(PERMANENT_DEVNET_UPGRADE_TARGETS_V1[1].2, "Rent ProgramData")?;
    let core_program = parse_pubkey(PERMANENT_DEVNET_UPGRADE_TARGETS_V1[6].1, "Core")?;
    if addresses[..4]
        != [
            registry_program,
            registry_programdata,
            rent_program,
            rent_programdata,
        ]
    {
        return Err(Error::new(
            "carry-forward snapshot substituted a permanent Program or ProgramData",
        ));
    }
    let profile_address = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &core_program,
    )
    .0;
    if addresses[8] != profile_address {
        return Err(Error::new(
            "carry-forward snapshot substituted the singleton infrastructure profile PDA",
        ));
    }
    require_account_shape(
        "Registry Program",
        required(0, "Registry Program")?,
        bpf_loader_upgradeable::ID,
        true,
        Some(36),
    )?;
    require_account_shape(
        "Registry ProgramData",
        required(1, "Registry ProgramData")?,
        bpf_loader_upgradeable::ID,
        false,
        None,
    )?;
    require_account_shape(
        "Rent Program",
        required(2, "Rent Program")?,
        bpf_loader_upgradeable::ID,
        true,
        Some(36),
    )?;
    require_account_shape(
        "Rent ProgramData",
        required(3, "Rent ProgramData")?,
        bpf_loader_upgradeable::ID,
        false,
        None,
    )?;
    require_account_shape(
        "infrastructure profile",
        required(8, "infrastructure profile")?,
        core_program,
        false,
        Some(144),
    )?;
    let profile_account = required(8, "infrastructure profile")?;
    let profile = ProtocolInfrastructureProfileV1::decode(&profile_account.data)
        .map_err(|error| Error::new(format!("carry-forward profile decode: {error:?}")))?;
    let profile_registry_program = Pubkey::new_from_array(profile.registry().program().to_bytes());
    let profile_rent_program = Pubkey::new_from_array(profile.rent().program().to_bytes());
    if profile_registry_program != registry_program || profile_rent_program != rent_program {
        return Err(Error::new(
            "carry-forward profile substituted the permanent Registry or Rent program",
        ));
    }
    let registry_artifact_id = profile.registry().artifact_release().to_bytes();
    let rent_artifact_id = profile.rent().artifact_release().to_bytes();
    let registry_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &registry_artifact_id,
        ],
        &registry_program,
    )
    .0;
    let registry_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &registry_artifact_id,
        ],
        &registry_program,
    )
    .0;
    let rent_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &rent_artifact_id,
        ],
        &registry_program,
    )
    .0;
    let rent_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &rent_artifact_id,
        ],
        &registry_program,
    )
    .0;
    if addresses[4..8] != [registry_raw, registry_staging, rent_raw, rent_staging] {
        return Err(Error::new(
            "carry-forward snapshot substituted a derived artifact raw or staging PDA",
        ));
    }
    if accounts[5].is_some() || accounts[7].is_some() {
        return Err(Error::new(
            "carry-forward staging must be finalized account absence, not a fabricated empty account",
        ));
    }
    let authority = parse_pubkey(
        &journal.retained_upgrade_authority,
        "carry-forward retained authority",
    )?;
    let mut role_pins = Vec::new();
    for (role, program, programdata, program_index, programdata_index, raw_index, artifact_id) in [
        (
            "registry",
            registry_program,
            registry_programdata,
            0_usize,
            1_usize,
            4_usize,
            registry_artifact_id,
        ),
        (
            "rent",
            rent_program,
            rent_programdata,
            2,
            3,
            6,
            rent_artifact_id,
        ),
    ] {
        let program_account = required(program_index, &format!("{role} Program"))?;
        let programdata_account = required(programdata_index, &format!("{role} ProgramData"))?;
        let raw_account = required(raw_index, &format!("{role} artifact raw"))?;
        require_account_shape(
            &format!("{role} artifact raw"),
            raw_account,
            registry_program,
            false,
            Some(216),
        )?;
        if digest(&raw_account.data) != hex(&artifact_id) {
            return Err(Error::new(format!(
                "carry-forward {role} raw body does not match the profile artifact ID"
            )));
        }
        let release = ArtifactReleaseV1::decode(&raw_account.data)
            .map_err(|error| Error::new(format!("carry-forward {role} artifact: {error:?}")))?;
        require_slot_pinned_release_v1(release).map_err(|error| {
            Error::new(format!(
                "carry-forward {role} artifact admission: {error:?}"
            ))
        })?;
        let program_view = ProgramV3View::parse(&program_account.data)
            .map_err(|error| Error::new(format!("carry-forward {role} Program: {error:?}")))?;
        let programdata_view = ProgramDataV3View::parse(&programdata_account.data)
            .map_err(|error| Error::new(format!("carry-forward {role} ProgramData: {error:?}")))?;
        if program_view.programdata() != programdata.to_bytes()
            || programdata_view.upgrade_authority() != Some(authority.to_bytes())
        {
            return Err(Error::new(format!(
                "carry-forward {role} Loader link or retained authority mismatch"
            )));
        }
        let live_sha: [u8; 32] = Sha256::digest(programdata_view.elf()).into();
        let observation = DeploymentObservationV1::new(
            program.to_bytes(),
            program_account.owner.to_bytes(),
            program_account.executable,
            programdata.to_bytes(),
            programdata_account.owner.to_bytes(),
            programdata_account.executable,
            program_view.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            programdata_view.deployment_slot(),
            live_sha,
            programdata_view.upgrade_authority(),
        )
        .map_err(|error| Error::new(format!("carry-forward {role} observation: {error:?}")))?;
        release
            .authenticate_deployment(observation)
            .map_err(|error| Error::new(format!("carry-forward {role} deployment: {error:?}")))?;
        let journal_role = journal
            .roles
            .iter()
            .find(|candidate| candidate.role == role)
            .expect("journal role closure already checked");
        let dump_bytes = read_optional_reference(&journal_role.dump, &format!("{role} live ELF"))?
            .ok_or_else(|| Error::new(format!("carry-forward {role} omitted live ELF dump")))?;
        if dump_bytes != programdata_view.elf() {
            return Err(Error::new(format!(
                "carry-forward {role} live ELF dump differs from the finalized ProgramData tail"
            )));
        }
        role_pins.push(CheckedUpgradeRolePinV1 {
            role: role.into(),
            disposition: CheckedDeploymentDispositionV1::CarryForward,
            program_id: program.to_string(),
            programdata_id: programdata.to_string(),
            baseline_path: None,
            baseline_sha256: None,
            receipt_path: None,
            receipt_sha256: None,
            dump_path: journal_role.dump.canonical_path.clone(),
            dump_sha256: journal_role
                .dump
                .sha256
                .clone()
                .expect("carry-forward dump closure checked"),
            checked_candidate_elf_path: journal_role.dump.canonical_path.clone(),
            checked_candidate_elf_sha256: journal_role
                .dump
                .sha256
                .clone()
                .expect("carry-forward dump closure checked"),
            live_elf_sha256: hex(&live_sha),
            deployment_slot: programdata_view.deployment_slot(),
            programdata_account_sha256: digest(&programdata_account.data),
            semantic_release_id: hex(release.semantic_release_id().as_bytes()),
            artifact_release_body_hex: Some(hex(&raw_account.data)),
            artifact_release_id: Some(hex(&artifact_id)),
            carried_programdata_base64: Some(BASE64.encode(&programdata_account.data)),
        });
    }
    let snapshot_path = journal.infrastructure_carry_forward.canonical_path.clone();
    let snapshot_sha256 = journal.infrastructure_carry_forward.sha256.clone();
    Ok(AuthenticatedCarryForwardV1 {
        pin: CheckedInfrastructureCarryForwardPinV1 {
            snapshot_path,
            snapshot_sha256,
            context_slot: snapshot.context_slot,
            profile_address: profile_address.to_string(),
            profile_account_sha256: exact_account_sha256(profile_account),
            profile_body_sha256: digest(&profile_account.data),
            profile_body_hex: hex(&profile_account.data),
            registry_raw_address: registry_raw.to_string(),
            registry_staging_address: registry_staging.to_string(),
            registry_programdata_account_sha256: exact_account_sha256(required(
                1,
                "Registry ProgramData",
            )?),
            rent_raw_address: rent_raw.to_string(),
            rent_staging_address: rent_staging.to_string(),
            rent_programdata_account_sha256: exact_account_sha256(required(3, "Rent ProgramData")?),
        },
        registry: role_pins.remove(0),
        rent: role_pins.remove(0),
        addresses,
        accounts,
    })
}

fn require_fresh_carry_forward(
    carry: &AuthenticatedCarryForwardV1,
    args: &UpgradeSetArgsV1,
    runner: &mut impl CliRunner,
) -> Result<()> {
    let (slot, observed) = runner.read_carry_forward_accounts(&CarryForwardQueryV1 {
        origin: &args.origin,
        addresses: &carry.addresses,
        minimum_context_slot: carry.pin.context_slot,
    })?;
    if slot < carry.pin.context_slot || observed != carry.accounts {
        return Err(Error::new(
            "live finalized infrastructure differs from the admitted one-context carry-forward snapshot",
        ));
    }
    Ok(())
}

fn set_role_elf_path(gate_path: &Path, role: &str) -> Result<PathBuf> {
    let bytes = read_regular_reference(gate_path, "set checked-release gate")?;
    let gate: CheckedUpgradeGateV1 = serde_json::from_slice(&bytes).map_err(|error| {
        Error::new(format!(
            "set checked-release gate is not canonical v1 JSON: {error}"
        ))
    })?;
    let matches = gate
        .links
        .iter()
        .filter(|link| link.label == role)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(Error::new(format!(
            "set checked-release gate does not contain one exact {role} link"
        )));
    }
    let elf = matches[0].elf.as_ref().ok_or_else(|| {
        Error::new(format!(
            "set checked-release gate role {role} has no deployable ELF"
        ))
    })?;
    let root = gate_path
        .parent()
        .ok_or_else(|| Error::new("set checked-release gate omitted evidence root"))?;
    Ok(root.join(&elf.canonical_path))
}

fn final_set_digest(journal: &UpgradeSetJournalV1) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/devnet-deployment-set/final/v2\0");
    hash_text(&mut hasher, &journal.schema)?;
    hash_text(&mut hasher, &journal.checked_release_gate.canonical_path)?;
    hash_text(&mut hasher, &journal.checked_release_gate.sha256)?;
    hash_text(&mut hasher, &journal.source_revision)?;
    hash_text(&mut hasher, &journal.source_tree_sha256)?;
    hash_text(&mut hasher, &journal.devnet_genesis_hash)?;
    hash_text(&mut hasher, &journal.solana_cli_version)?;
    hasher.update(
        parse_pubkey(
            &journal.retained_upgrade_authority,
            "set retained authority",
        )?
        .as_ref(),
    );
    hasher.update(parse_pubkey(&journal.fee_payer, "set fee payer")?.as_ref());
    hash_text(
        &mut hasher,
        &journal.infrastructure_carry_forward.canonical_path,
    )?;
    hash_text(&mut hasher, &journal.infrastructure_carry_forward.sha256)?;
    for role in &journal.roles {
        hash_text(&mut hasher, &role.role)?;
        hasher.update([match role.disposition {
            CheckedDeploymentDispositionV1::CarryForward => 0,
            CheckedDeploymentDispositionV1::Upgrade => 1,
        }]);
        hasher.update(parse_pubkey(&role.program_id, "set Program")?.as_ref());
        hasher.update(parse_pubkey(&role.programdata_id, "set ProgramData")?.as_ref());
        match &role.baseline {
            Some(baseline) => {
                hasher.update([1]);
                hash_text(&mut hasher, &baseline.canonical_path)?;
                hash_text(&mut hasher, &baseline.sha256)?;
            }
            None => hasher.update([0]),
        }
        hash_text(&mut hasher, &role.receipt.canonical_path)?;
        match &role.receipt.sha256 {
            Some(receipt) => {
                hasher.update([1]);
                hash_text(&mut hasher, receipt)?;
            }
            None => hasher.update([0]),
        }
        hash_text(&mut hasher, &role.dump.canonical_path)?;
        hash_text(
            &mut hasher,
            role.dump
                .sha256
                .as_deref()
                .ok_or_else(|| Error::new("final deployment set omitted dump digest"))?,
        )?;
    }
    Ok(hex(&hasher.finalize()))
}

fn checked_semantic_release_id(role: &str, source_revision: &str) -> Result<String> {
    let fixed = match role {
        "trading" => Some(COMPILED_DIRECT_RELEASE_ID_V1),
        "resolution" => Some(RESOLUTION_CONTROLLER_RELEASE_ID_V5),
        _ => None,
    };
    if let Some(release_id) = fixed {
        return Ok(hex(&release_id));
    }
    let source_role = match role {
        "registry" => SourceSemanticRoleV1::Registry,
        "core" => SourceSemanticRoleV1::Core,
        "claims" => SourceSemanticRoleV1::Claims,
        "custody" => SourceSemanticRoleV1::Custody,
        "rent" => SourceSemanticRoleV1::RentCredit,
        _ => {
            return Err(Error::new(format!(
                "role {role:?} has no protocol-owned semantic release identity"
            )));
        }
    };
    let preimage = source_semantic_release_preimage_v1(source_role, source_revision.as_bytes())
        .map_err(|_| Error::new("source semantic release revision is not canonical"))?;
    Ok(digest(&preimage))
}

/// Authenticate the canonical mixed deployment set for checked prepare. The
/// two infrastructure roles are admitted only from the exact one-context
/// CarryForward snapshot; the five execution roles remain exact complete
/// one-role Upgrade receipts.
pub(crate) fn authenticate_complete_upgrade_set_for_prepare(
    journal_path: &Path,
) -> Result<CheckedUpgradeSetPinV1> {
    let (journal, journal_sha256) = load_set_journal_path(journal_path)?;
    let journal_canonical = exact_reference_path(
        journal_path
            .to_str()
            .ok_or_else(|| Error::new("--deployment-set-journal path is not UTF-8"))?,
        "deployment-set journal",
    )?;
    let carry = authenticate_carry_forward(&journal)?;
    let gate_path = PathBuf::from(&journal.checked_release_gate.canonical_path);
    let authority = parse_pubkey(
        &journal.retained_upgrade_authority,
        "deployment-set retained authority",
    )?;
    let payer = parse_pubkey(&journal.fee_payer, "deployment-set fee payer")?;
    let offline_origin =
        ClusterOriginV1::parse("https://api.devnet.solana.com", Some(DEVNET_GENESIS_HASH))?;
    let mut roles = vec![carry.registry.clone(), carry.rent.clone()];
    for role in &journal.roles[2..] {
        let baseline = role.baseline.as_ref().expect("mixed closure checked");
        let receipt_bytes =
            read_optional_reference(&role.receipt, &format!("{} receipt", role.role))?
                .ok_or_else(|| Error::new(format!("{} receipt is not complete", role.role)))?;
        let dump_bytes = read_optional_reference(&role.dump, &format!("{} dump", role.role))?
            .ok_or_else(|| Error::new(format!("{} dump is not complete", role.role)))?;
        let receipt: UpgradeReceiptV1 =
            serde_json::from_slice(&receipt_bytes).map_err(|error| {
                Error::new(format!(
                    "{} receipt is not canonical {SCHEMA} JSON: {error}",
                    role.role
                ))
            })?;
        let elf_path = set_role_elf_path(&gate_path, &role.role)?;
        let role_args = UpgradeArgsV1 {
            origin: offline_origin.clone(),
            role: role.role.clone(),
            program_id: parse_pubkey(&role.program_id, "deployment-set Program")?,
            programdata_id: parse_pubkey(&role.programdata_id, "deployment-set ProgramData")?,
            expected_upgrade_authority: authority,
            authority_keypair: PathBuf::from("/offline-upgrade-authority-not-read"),
            fee_payer: payer,
            fee_payer_keypair: PathBuf::from("/offline-fee-payer-not-read"),
            elf_path: elf_path.clone(),
            checked_release_gate_path: gate_path.clone(),
            expected_checked_release_gate_sha256: journal.checked_release_gate.sha256.clone(),
            expected_source_revision: journal.source_revision.clone(),
            expected_source_tree_sha256: journal.source_tree_sha256.clone(),
            baseline_path: PathBuf::from(&baseline.canonical_path),
            receipt_path: PathBuf::from(&role.receipt.canonical_path),
            dump_path: PathBuf::from(&role.dump.canonical_path),
            solana_cli: PathBuf::from("/offline-solana-not-run"),
            target_acknowledgment: format!("{}:{}", role.role, role.program_id),
            exclusive_payer_window_acknowledgment: Some(format!(
                "{}:{}:{}",
                role.role, role.program_id, journal.fee_payer
            )),
            execute: false,
            preflight: false,
        };
        let admission = admit_upgrade(&role_args)?;
        if admission.gate.solana_cli_version != journal.solana_cli_version {
            return Err(Error::new(format!(
                "{} gate CLI identity differs from the deployment-set journal",
                role.role
            )));
        }
        validate_receipt_binding(
            &role_args,
            &admission.gate,
            &admission.baseline,
            &admission.live_elf_sha256,
            admission.live_elf_padding_bytes,
            &receipt,
            false,
        )?;
        if receipt.phase != ReceiptPhaseV1::Complete {
            return Err(Error::new(format!(
                "{} Upgrade receipt is not complete",
                role.role
            )));
        }
        let after = receipt.after.as_ref().expect("validated complete receipt");
        if after.program_lamports != receipt.before.program_lamports
            || after.program_owner != receipt.before.program_owner
            || after.program_executable != receipt.before.program_executable
            || after.program_data_bytes != receipt.before.program_data_bytes
            || after.program_account_sha256 != receipt.before.program_account_sha256
            || after.programdata_owner != receipt.before.programdata_owner
            || after.programdata_executable != receipt.before.programdata_executable
            || after.programdata_data_bytes
                != u64::try_from(admission.candidate_live.len())
                    .map_err(|_| Error::new("checked live payload width does not fit u64"))?
                    .checked_add(45)
                    .ok_or_else(|| Error::new("checked ProgramData width overflow"))?
            || after.live_elf_bytes
                != u64::try_from(admission.candidate_live.len())
                    .map_err(|_| Error::new("checked live payload width does not fit u64"))?
        {
            return Err(Error::new(format!(
                "{} receipt does not preserve exact Program linkage and Loader shape",
                role.role
            )));
        }
        let (dump_sha256, dump_shape) = classify_dump(
            &dump_bytes,
            &admission.gate.raw_elf,
            &admission.candidate_live,
        )?;
        if receipt.dump_sha256.as_deref() != Some(dump_sha256.as_str())
            || receipt.dump_shape.as_deref() != Some(dump_shape.as_str())
            || role.dump.sha256.as_deref() != Some(dump_sha256.as_str())
        {
            return Err(Error::new(format!(
                "{} dump digest/shape differs from its complete receipt",
                role.role
            )));
        }
        roles.push(CheckedUpgradeRolePinV1 {
            role: role.role.clone(),
            disposition: CheckedDeploymentDispositionV1::Upgrade,
            program_id: role.program_id.clone(),
            programdata_id: role.programdata_id.clone(),
            baseline_path: Some(baseline.canonical_path.clone()),
            baseline_sha256: Some(baseline.sha256.clone()),
            receipt_path: Some(role.receipt.canonical_path.clone()),
            receipt_sha256: Some(role.receipt.sha256.clone().expect("checked receipt digest")),
            dump_path: role.dump.canonical_path.clone(),
            dump_sha256,
            checked_candidate_elf_path: fs::canonicalize(&elf_path)?.display().to_string(),
            checked_candidate_elf_sha256: admission.gate.raw_elf_sha256,
            live_elf_sha256: admission.live_elf_sha256,
            deployment_slot: after.deployment_slot,
            programdata_account_sha256: after.programdata_account_sha256.clone(),
            semantic_release_id: checked_semantic_release_id(&role.role, &journal.source_revision)?,
            artifact_release_body_hex: None,
            artifact_release_id: None,
            carried_programdata_base64: None,
        });
    }
    Ok(CheckedUpgradeSetPinV1 {
        schema: CHECKED_SET_PREPARE_SCHEMA.into(),
        journal_path: journal_canonical.display().to_string(),
        journal_sha256,
        final_set_sha256: final_set_digest(&journal)?,
        checked_release_gate_path: journal.checked_release_gate.canonical_path,
        checked_release_gate_sha256: journal.checked_release_gate.sha256,
        source_revision: journal.source_revision,
        source_tree_sha256: journal.source_tree_sha256,
        devnet_genesis_hash: journal.devnet_genesis_hash,
        solana_cli_version: journal.solana_cli_version,
        retained_upgrade_authority: journal.retained_upgrade_authority,
        fee_payer: journal.fee_payer,
        semantic_derivation: SEMANTIC_DERIVATION_V1.into(),
        infrastructure_carry_forward: carry.pin,
        roles,
    })
}

/// Fresh network admission for checked prepare. This is still key-free and
/// read-only: it runs the v2 set auditor first, requires a fresh final digest,
/// then returns the locally rehashed projection of the same journal.
pub(crate) fn authenticate_complete_deployment_set_for_prepare_live(
    journal_path: &Path,
    rpc_url: &str,
    devnet_acknowledgment: &str,
    solana_cli: &Path,
) -> Result<CheckedUpgradeSetPinV1> {
    let origin = ClusterOriginV1::parse(rpc_url, Some(devnet_acknowledgment))?;
    if !matches!(origin, ClusterOriginV1::AcknowledgedDevnet { .. }) {
        return Err(Error::new(
            "checked deployment-set prepare admits exact public devnet only",
        ));
    }
    let args = UpgradeSetArgsV1 {
        origin,
        journal_path: journal_path.to_path_buf(),
        solana_cli: solana_cli.to_path_buf(),
    };
    let mut runner = SystemCliRunner {
        executable: solana_cli.to_path_buf(),
    };
    let audit = audit_set_journal_with_runner(&args, &mut runner)?;
    let audit_digest = audit
        .final_set_sha256
        .ok_or_else(|| Error::new("checked deployment set is not complete"))?;
    let pin = authenticate_complete_upgrade_set_for_prepare(journal_path)?;
    if pin.final_set_sha256 != audit_digest {
        return Err(Error::new(
            "fresh deployment-set audit digest differs from the prepare projection",
        ));
    }
    Ok(pin)
}

/// Rehash a persisted checked pin immediately before plan derivation. No
/// caller-authored projection, including a semantic ID, may survive this
/// equality against the canonical evidence owners.
pub(crate) fn reauthenticate_checked_deployment_set_pin(
    set: &CheckedUpgradeSetPinV1,
) -> Result<()> {
    let fresh = authenticate_complete_upgrade_set_for_prepare(Path::new(&set.journal_path))?;
    if &fresh != set {
        return Err(Error::new(
            "checked deployment-set evidence changed or a caller projection was substituted",
        ));
    }
    Ok(())
}

fn parse_args(arguments: Vec<String>) -> Result<UpgradeArgsV1> {
    let mut values = std::collections::BTreeMap::<String, String>::new();
    let mut execute = false;
    let mut preflight = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if matches!(argument.as_str(), "--execute" | "--preflight") {
            let selected = if argument == "--execute" {
                &mut execute
            } else {
                &mut preflight
            };
            if *selected {
                return Err(Error::new(format!("{argument} may be supplied only once")));
            }
            *selected = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if !matches!(
            argument.as_str(),
            "--rpc-url"
                | DEVNET_ACKNOWLEDGMENT_FLAG
                | "--role"
                | "--program-id"
                | "--programdata-id"
                | "--expected-upgrade-authority"
                | "--authority-keypair"
                | "--fee-payer"
                | "--fee-payer-keypair"
                | "--elf"
                | "--checked-release-gate"
                | "--expected-checked-release-gate-sha256"
                | "--expected-source-revision"
                | "--expected-source-tree-sha256"
                | "--baseline"
                | "--receipt"
                | "--dump"
                | "--solana-cli"
                | TARGET_ACK_FLAG
                | EXCLUSIVE_PAYER_ACK_FLAG
        ) {
            return Err(Error::new(format!(
                "unknown devnet-upgrade-v1 argument: {argument}"
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    if execute == preflight {
        return Err(Error::new(
            "devnet-upgrade-v1 requires exactly one of --preflight (read-only and key-free) or \
             --execute (the separately acknowledged mutation)",
        ));
    }
    let take = |label: &str| {
        values
            .get(label)
            .cloned()
            .ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let rpc_url = take("--rpc-url")?;
    let acknowledgment = take(DEVNET_ACKNOWLEDGMENT_FLAG)?;
    let origin = ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?;
    if !matches!(origin, ClusterOriginV1::AcknowledgedDevnet { .. }) {
        return Err(Error::new(
            "devnet-upgrade-v1 admits exact public devnet only; loopback is not an Upgrade target",
        ));
    }
    let role = take("--role")?;
    require_role(&role)?;
    Ok(UpgradeArgsV1 {
        origin,
        role,
        program_id: parse_pubkey(&take("--program-id")?, "--program-id")?,
        programdata_id: parse_pubkey(&take("--programdata-id")?, "--programdata-id")?,
        expected_upgrade_authority: parse_pubkey(
            &take("--expected-upgrade-authority")?,
            "--expected-upgrade-authority",
        )?,
        authority_keypair: absolute(&take("--authority-keypair")?, "--authority-keypair")?,
        fee_payer: parse_pubkey(&take("--fee-payer")?, "--fee-payer")?,
        fee_payer_keypair: absolute(&take("--fee-payer-keypair")?, "--fee-payer-keypair")?,
        elf_path: absolute(&take("--elf")?, "--elf")?,
        checked_release_gate_path: absolute(
            &take("--checked-release-gate")?,
            "--checked-release-gate",
        )?,
        expected_checked_release_gate_sha256: take("--expected-checked-release-gate-sha256")?,
        expected_source_revision: take("--expected-source-revision")?,
        expected_source_tree_sha256: take("--expected-source-tree-sha256")?,
        baseline_path: absolute(&take("--baseline")?, "--baseline")?,
        receipt_path: absolute(&take("--receipt")?, "--receipt")?,
        dump_path: absolute(&take("--dump")?, "--dump")?,
        solana_cli: absolute(&take("--solana-cli")?, "--solana-cli")?,
        target_acknowledgment: take(TARGET_ACK_FLAG)?,
        exclusive_payer_window_acknowledgment: values.get(EXCLUSIVE_PAYER_ACK_FLAG).cloned(),
        execute,
        preflight,
    })
}

fn parse_baseline_args(arguments: Vec<String>) -> Result<BaselineArgsV1> {
    let mut values = std::collections::BTreeMap::<String, String>::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if !matches!(
            argument.as_str(),
            "--rpc-url"
                | DEVNET_ACKNOWLEDGMENT_FLAG
                | "--role"
                | "--program-id"
                | "--programdata-id"
                | "--expected-upgrade-authority"
                | "--minimum-context-slot"
                | "--target-live-elf-bytes"
                | "--output"
        ) {
            return Err(Error::new(format!(
                "unknown devnet-upgrade-baseline-v1 argument: {argument}"
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let take = |label: &str| {
        values
            .get(label)
            .cloned()
            .ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let rpc_url = take("--rpc-url")?;
    let acknowledgment = take(DEVNET_ACKNOWLEDGMENT_FLAG)?;
    let origin = ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?;
    if !matches!(origin, ClusterOriginV1::AcknowledgedDevnet { .. }) {
        return Err(Error::new(
            "devnet-upgrade-baseline-v1 admits exact public devnet only",
        ));
    }
    let role = take("--role")?;
    require_role(&role)?;
    let minimum_context_slot = take("--minimum-context-slot")?
        .parse::<u64>()
        .map_err(|_| Error::new("--minimum-context-slot must be a u64"))?;
    if minimum_context_slot == 0 {
        return Err(Error::new("--minimum-context-slot must be nonzero"));
    }
    let target_live_elf_bytes = take("--target-live-elf-bytes")?
        .parse::<u64>()
        .map_err(|_| Error::new("--target-live-elf-bytes must be a u64"))?;
    if target_live_elf_bytes == 0 {
        return Err(Error::new("--target-live-elf-bytes must be nonzero"));
    }
    Ok(BaselineArgsV1 {
        origin,
        role,
        program_id: parse_pubkey(&take("--program-id")?, "--program-id")?,
        programdata_id: parse_pubkey(&take("--programdata-id")?, "--programdata-id")?,
        expected_upgrade_authority: parse_pubkey(
            &take("--expected-upgrade-authority")?,
            "--expected-upgrade-authority",
        )?,
        minimum_context_slot,
        target_live_elf_bytes,
        output_path: absolute(&take("--output")?, "--output")?,
    })
}

fn parse_extension_args(arguments: Vec<String>) -> Result<ExtensionArgsV1> {
    let mut values = std::collections::BTreeMap::<String, String>::new();
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
        if !matches!(
            argument.as_str(),
            "--rpc-url"
                | DEVNET_ACKNOWLEDGMENT_FLAG
                | "--role"
                | "--program-id"
                | "--programdata-id"
                | "--expected-upgrade-authority"
                | "--authority-keypair"
                | "--fee-payer"
                | "--fee-payer-keypair"
                | "--baseline"
                | "--receipt"
                | "--solana-cli"
                | "--expected-solana-cli-version"
                | EXTENSION_ACK_FLAG
        ) {
            return Err(Error::new(format!(
                "unknown devnet-upgrade-extend-v1 argument: {argument}"
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    if !execute {
        return Err(Error::new(
            "devnet-upgrade-extend-v1 requires --execute; allocation extension is a separate \
             acknowledged devnet write",
        ));
    }
    let take = |label: &str| {
        values
            .get(label)
            .cloned()
            .ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let rpc_url = take("--rpc-url")?;
    let acknowledgment = take(DEVNET_ACKNOWLEDGMENT_FLAG)?;
    let origin = ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?;
    if !matches!(origin, ClusterOriginV1::AcknowledgedDevnet { .. }) {
        return Err(Error::new(
            "devnet-upgrade-extend-v1 admits exact public devnet only",
        ));
    }
    let role = take("--role")?;
    require_role(&role)?;
    Ok(ExtensionArgsV1 {
        origin,
        role,
        program_id: parse_pubkey(&take("--program-id")?, "--program-id")?,
        programdata_id: parse_pubkey(&take("--programdata-id")?, "--programdata-id")?,
        expected_upgrade_authority: parse_pubkey(
            &take("--expected-upgrade-authority")?,
            "--expected-upgrade-authority",
        )?,
        authority_keypair: absolute(&take("--authority-keypair")?, "--authority-keypair")?,
        fee_payer: parse_pubkey(&take("--fee-payer")?, "--fee-payer")?,
        fee_payer_keypair: absolute(&take("--fee-payer-keypair")?, "--fee-payer-keypair")?,
        baseline_path: absolute(&take("--baseline")?, "--baseline")?,
        receipt_path: absolute(&take("--receipt")?, "--receipt")?,
        solana_cli: absolute(&take("--solana-cli")?, "--solana-cli")?,
        expected_solana_cli_version: take("--expected-solana-cli-version")?,
        target_acknowledgment: take(EXTENSION_ACK_FLAG)?,
        execute,
    })
}

fn extension_shadow_args(args: &ExtensionArgsV1) -> UpgradeArgsV1 {
    UpgradeArgsV1 {
        origin: args.origin.clone(),
        role: args.role.clone(),
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        expected_upgrade_authority: args.expected_upgrade_authority,
        authority_keypair: args.authority_keypair.clone(),
        fee_payer: args.fee_payer,
        fee_payer_keypair: args.fee_payer_keypair.clone(),
        elf_path: PathBuf::from("/extension-has-no-elf"),
        checked_release_gate_path: PathBuf::from("/extension-has-no-checked-release-gate"),
        expected_checked_release_gate_sha256: "0".repeat(64),
        expected_source_revision: "0".repeat(40),
        expected_source_tree_sha256: "0".repeat(64),
        baseline_path: args.baseline_path.clone(),
        receipt_path: args.receipt_path.clone(),
        dump_path: PathBuf::from("/extension-has-no-dump"),
        solana_cli: args.solana_cli.clone(),
        target_acknowledgment: String::new(),
        exclusive_payer_window_acknowledgment: None,
        execute: args.execute,
        preflight: false,
    }
}

fn execute_with_runner(
    args: &UpgradeArgsV1,
    runner: &mut impl CliRunner,
) -> Result<UpgradeReceiptV1> {
    if !args.execute || args.preflight {
        return Err(Error::new("Upgrade runner reached without --execute"));
    }
    let exclusive_payer_ack = format!("{}:{}:{}", args.role, args.program_id, args.fee_payer);
    if args.exclusive_payer_window_acknowledgment.as_deref() != Some(exclusive_payer_ack.as_str()) {
        return Err(Error::new(format!(
            "{EXCLUSIVE_PAYER_ACK_FLAG} must be the exact exclusive window {exclusive_payer_ack}; the observed operation-wide wallet delta cannot be attributed otherwise"
        )));
    }
    let admission = admit_upgrade(args)?;
    let gate = &admission.gate;
    let baseline = &admission.baseline;
    let candidate_live = &admission.candidate_live;
    let live_elf_sha256 = &admission.live_elf_sha256;
    let live_elf_padding_bytes = admission.live_elf_padding_bytes;

    let existing = load_receipt(&args.receipt_path)?;
    if let Some(receipt) = &existing {
        validate_receipt_binding(
            args,
            gate,
            baseline,
            live_elf_sha256,
            live_elf_padding_bytes,
            receipt,
            true,
        )?;
    } else if args.dump_path.exists() {
        return Err(Error::new(format!(
            "dump target {} already exists without this operation's receipt; refusing to \
             overwrite or reinterpret it",
            args.dump_path.display()
        )));
    }

    let cli_version = invoke(runner, args, &["--version".into()])?;
    let observed_cli_version = cli_version.stdout.trim();
    if observed_cli_version != gate.solana_cli_version {
        return Err(Error::new(format!(
            "Solana CLI version is {observed_cli_version:?} but artifact evidence pins {:?}",
            gate.solana_cli_version
        )));
    }
    authenticate_devnet(runner, args)?;
    if existing
        .as_ref()
        .is_some_and(|receipt| receipt.phase == ReceiptPhaseV1::Complete)
    {
        return verify_complete_receipt_live(
            runner,
            args,
            gate,
            candidate_live,
            existing.as_ref().expect("checked complete receipt"),
        );
    }
    authenticate_keypair(
        runner,
        args,
        &args.authority_keypair,
        args.expected_upgrade_authority,
        "upgrade authority",
    )?;
    authenticate_keypair(
        runner,
        args,
        &args.fee_payer_keypair,
        args.fee_payer,
        "fee payer",
    )?;

    match existing {
        None => {
            let before = read_snapshot(runner, args, baseline.context_slot)?;
            require_baseline_prestate(baseline, &before.loader)?;
            require_candidate_fits(&before, candidate_live)?;
            if before.loader.live_elf_sha256 == *live_elf_sha256 {
                return Err(Error::new(
                    "the current live payload already equals the candidate but no receipt binds \
                     an Upgrade; refusing replay ambiguity",
                ));
            }
            let operation_id = operation_id(args, &gate.gate_sha256, &before);
            let mut receipt = UpgradeReceiptV1 {
                schema: SCHEMA.into(),
                phase: ReceiptPhaseV1::Prepared,
                operation_id,
                role: args.role.clone(),
                program_id: args.program_id.to_string(),
                programdata_id: args.programdata_id.to_string(),
                retained_upgrade_authority: args.expected_upgrade_authority.to_string(),
                fee_payer: args.fee_payer.to_string(),
                rpc_origin_redacted: args.origin.redacted_url(),
                genesis_hash: DEVNET_GENESIS_HASH.into(),
                source_revision: gate.source_revision.clone(),
                source_tree_sha256: gate.source_tree_sha256.clone(),
                checked_release_gate_sha256: gate.gate_sha256.clone(),
                baseline_sha256: baseline.baseline_sha256.clone(),
                baseline_context_slot: baseline.context_slot,
                raw_elf_sha256: gate.raw_elf_sha256.clone(),
                live_elf_sha256: live_elf_sha256.clone(),
                live_elf_padding_bytes,
                solana_cli_version: gate.solana_cli_version.clone(),
                before_context_slot: before.context_slot,
                before: before.loader.clone(),
                wallet_before_lamports: before.wallet_lamports,
                exclusive_payer_window_acknowledgment: exclusive_payer_ack,
                transaction_signature: None,
                solana_cli_output: None,
                finalized_transaction: None,
                finalized_transaction_sha256: None,
                after_context_slot: None,
                after: None,
                arithmetic: None,
                dump_sha256: None,
                dump_shape: None,
                receipt_sha256: String::new(),
            };
            write_receipt(&args.receipt_path, &mut receipt)?;
            submit_and_finish(runner, args, gate, candidate_live, &mut receipt)
        }
        Some(mut receipt) if receipt.phase == ReceiptPhaseV1::Prepared => {
            let current = read_snapshot(runner, args, receipt.before_context_slot)?;
            if current.loader != receipt.before
                || current.wallet_lamports != receipt.wallet_before_lamports
            {
                return Err(Error::new(
                    "a prepared Upgrade receipt exists but the Loader or wallet prestate moved. \
                     The prior submission outcome is ambiguous; do not replay the deploy. Recover \
                     and attach the original CLI output, or start a separately reviewed operation.",
                ));
            }
            require_baseline_prestate(baseline, &current.loader)?;
            require_candidate_fits(&current, candidate_live)?;
            submit_and_finish(runner, args, gate, candidate_live, &mut receipt)
        }
        Some(mut receipt) if receipt.phase == ReceiptPhaseV1::Submitted => {
            finish_submitted(runner, args, gate, candidate_live, &mut receipt)
        }
        Some(_) => Err(Error::new("unknown Upgrade receipt phase")),
    }
}

fn admit_upgrade(args: &UpgradeArgsV1) -> Result<UpgradeAdmissionV1> {
    require_role(&args.role)?;
    let expected_target_ack = format!("{}:{}", args.role, args.program_id);
    if args.target_acknowledgment != expected_target_ack {
        return Err(Error::new(format!(
            "{TARGET_ACK_FLAG} must be the exact target {expected_target_ack}; it was {:?}",
            args.target_acknowledgment
        )));
    }
    let gate = validate_checked_release_gate(args)?;
    let baseline_bytes = fs::read(&args.baseline_path)?;
    let baseline: UpgradeBaselineV1 = serde_json::from_slice(&baseline_bytes)?;
    validate_baseline(args, &baseline)?;
    let candidate_live = candidate_live_image(&gate.raw_elf, baseline.target_live_elf_bytes)?;
    let live_elf_sha256 = digest(&candidate_live);
    let live_elf_padding_bytes = u64::try_from(candidate_live.len() - gate.raw_elf.len())
        .map_err(|_| Error::new("live ELF padding does not fit u64"))?;
    if u64::try_from(candidate_live.len()).ok() != Some(baseline.target_live_elf_bytes) {
        return Err(Error::new(format!(
            "checked candidate live width is {}, baseline planned {}",
            candidate_live.len(),
            baseline.target_live_elf_bytes
        )));
    }
    Ok(UpgradeAdmissionV1 {
        gate,
        baseline,
        candidate_live,
        live_elf_sha256,
        live_elf_padding_bytes,
    })
}

fn preflight_with_runner(
    args: &UpgradeArgsV1,
    runner: &mut impl CliRunner,
) -> Result<UpgradePreflightV1> {
    if args.execute || !args.preflight {
        return Err(Error::new("preflight runner reached without --preflight"));
    }
    let admission = admit_upgrade(args)?;
    let existing = load_receipt(&args.receipt_path)?;
    if let Some(receipt) = &existing {
        validate_receipt_binding(
            args,
            &admission.gate,
            &admission.baseline,
            &admission.live_elf_sha256,
            admission.live_elf_padding_bytes,
            receipt,
            true,
        )?;
    } else if args.dump_path.exists() {
        return Err(Error::new(
            "preflight dump target already exists without a bound receipt",
        ));
    }
    let cli_version = invoke(runner, args, &["--version".into()])?;
    if cli_version.stdout.trim() != admission.gate.solana_cli_version {
        return Err(Error::new(format!(
            "Solana CLI version is {:?} but artifact evidence pins {:?}",
            cli_version.stdout.trim(),
            admission.gate.solana_cli_version
        )));
    }
    authenticate_devnet(runner, args)?;
    let minimum_context_slot = existing
        .as_ref()
        .and_then(|receipt| receipt.after_context_slot)
        .unwrap_or(admission.baseline.context_slot);
    let observation = read_snapshot(runner, args, minimum_context_slot)?;
    match &existing {
        Some(receipt) if receipt.phase == ReceiptPhaseV1::Complete => {
            verify_complete_observation(
                args,
                &admission.gate,
                &admission.candidate_live,
                receipt,
                &observation,
            )?;
            verify_recorded_upgrade_transaction(runner, args, receipt, &observation)?;
        }
        Some(receipt) if receipt.phase == ReceiptPhaseV1::Prepared => {
            if observation.loader != receipt.before
                || observation.wallet_lamports != receipt.wallet_before_lamports
            {
                return Err(Error::new(
                    "preflight found drift from the prepared receipt prestate",
                ));
            }
        }
        Some(receipt) if receipt.phase == ReceiptPhaseV1::Submitted => {
            verify_submitted_observation_read_only(
                runner,
                args,
                &admission.candidate_live,
                receipt,
                &observation,
            )?;
        }
        Some(_) => return Err(Error::new("preflight found an unknown receipt phase")),
        None => {
            require_baseline_prestate(&admission.baseline, &observation.loader)?;
            require_candidate_fits(&observation, &admission.candidate_live)?;
            if observation.loader.live_elf_sha256 == admission.live_elf_sha256 {
                return Err(Error::new(
                    "current live payload already equals candidate without a bound receipt",
                ));
            }
        }
    }
    Ok(UpgradePreflightV1 {
        schema: PREFLIGHT_SCHEMA.into(),
        role: args.role.clone(),
        program_id: args.program_id.to_string(),
        programdata_id: args.programdata_id.to_string(),
        retained_upgrade_authority: args.expected_upgrade_authority.to_string(),
        fee_payer: args.fee_payer.to_string(),
        rpc_origin_redacted: args.origin.redacted_url(),
        genesis_hash: DEVNET_GENESIS_HASH.into(),
        solana_cli_version: admission.gate.solana_cli_version,
        source_revision: admission.gate.source_revision,
        source_tree_sha256: admission.gate.source_tree_sha256,
        checked_release_gate_sha256: admission.gate.gate_sha256,
        baseline_sha256: admission.baseline.baseline_sha256,
        observation_context_slot: observation.context_slot,
        observation: observation.loader,
        wallet_lamports: observation.wallet_lamports,
        raw_elf_sha256: admission.gate.raw_elf_sha256,
        live_elf_sha256: admission.live_elf_sha256,
        live_elf_padding_bytes: admission.live_elf_padding_bytes,
        receipt_phase: existing.map(|receipt| receipt.phase),
        mutation_permitted: false,
    })
}

fn verify_submitted_observation_read_only(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    candidate_live: &[u8],
    receipt: &UpgradeReceiptV1,
    observation: &SnapshotV1,
) -> Result<()> {
    if observation.loader.deployment_slot <= receipt.before.deployment_slot {
        return Err(Error::new(
            "preflight found no finalized deployment-slot advance for submitted receipt",
        ));
    }
    if observation.live_elf != candidate_live
        || observation.loader.live_elf_sha256 != receipt.live_elf_sha256
        || observation.loader.upgrade_authority != args.expected_upgrade_authority.to_string()
        || observation.loader.programdata_lamports != receipt.before.programdata_lamports
    {
        return Err(Error::new(
            "preflight found submitted Upgrade payload, authority, or parked rent drift",
        ));
    }
    let signature = receipt
        .transaction_signature
        .as_deref()
        .ok_or_else(|| Error::new("submitted receipt omitted Upgrade signature"))?;
    let _ = runner.resolve_upgrade_transaction(&UpgradeTransactionQueryV1 {
        origin: &args.origin,
        signature,
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        deployment_slot: observation.loader.deployment_slot,
        programdata_before_lamports: receipt.before.programdata_lamports,
        programdata_after_lamports: observation.loader.programdata_lamports,
    })?;
    Ok(())
}

fn execute_extension_with_runner(
    args: &ExtensionArgsV1,
    runner: &mut impl CliRunner,
) -> Result<ExtensionReceiptV1> {
    if !args.execute {
        return Err(Error::new("extension runner reached without --execute"));
    }
    let shadow = extension_shadow_args(args);
    let baseline_bytes = fs::read(&args.baseline_path)?;
    let baseline: UpgradeBaselineV1 = serde_json::from_slice(&baseline_bytes)?;
    validate_baseline(&shadow, &baseline)?;
    if baseline.extension_additional_bytes == 0 {
        return Err(Error::new(
            "baseline says the checked candidate already fits; extension is refused rather than \
             turned into an undocumented no-op",
        ));
    }
    let expected_ack = format!(
        "{}:{}:+{}",
        args.role, args.program_id, baseline.extension_additional_bytes
    );
    if args.target_acknowledgment != expected_ack {
        return Err(Error::new(format!(
            "{EXTENSION_ACK_FLAG} must be the exact allocation act {expected_ack}; it was {:?}",
            args.target_acknowledgment
        )));
    }

    let existing = load_extension_receipt(&args.receipt_path)?;
    if let Some(receipt) = &existing {
        validate_extension_receipt_binding(args, &baseline, receipt)?;
    }

    let cli_version = invoke(runner, &shadow, &["--version".into()])?;
    if cli_version.stdout.trim() != args.expected_solana_cli_version {
        return Err(Error::new(format!(
            "Solana CLI version is {:?}; extension pins {:?}",
            cli_version.stdout.trim(),
            args.expected_solana_cli_version
        )));
    }
    authenticate_devnet(runner, &shadow)?;
    if existing
        .as_ref()
        .is_some_and(|receipt| receipt.phase == ReceiptPhaseV1::Complete)
    {
        return verify_complete_extension_live(
            runner,
            args,
            &shadow,
            existing.as_ref().expect("checked complete extension"),
        );
    }
    authenticate_keypair(
        runner,
        &shadow,
        &args.authority_keypair,
        args.expected_upgrade_authority,
        "upgrade authority",
    )?;
    authenticate_keypair(
        runner,
        &shadow,
        &args.fee_payer_keypair,
        args.fee_payer,
        "fee payer",
    )?;

    match existing {
        None => {
            let before = read_snapshot(runner, &shadow, baseline.context_slot)?;
            require_baseline_prestate(&baseline, &before.loader)?;
            let required = baseline
                .extension_lamport_top_up
                .checked_add(EXTENSION_FEE_RESERVE_LAMPORTS)
                .ok_or_else(|| Error::new("extension funding requirement overflow"))?;
            if before.wallet_lamports < required {
                return Err(Error::new(format!(
                    "fee payer has {} lamports; exact rent top-up {} plus the provisional \
                     {}-lamport extension fee reserve requires at least {required}",
                    before.wallet_lamports,
                    baseline.extension_lamport_top_up,
                    EXTENSION_FEE_RESERVE_LAMPORTS
                )));
            }
            let mut receipt = ExtensionReceiptV1 {
                schema: EXTENSION_SCHEMA.into(),
                phase: ReceiptPhaseV1::Prepared,
                operation_id: extension_operation_id(args, &baseline, &before),
                role: args.role.clone(),
                program_id: args.program_id.to_string(),
                programdata_id: args.programdata_id.to_string(),
                retained_upgrade_authority: args.expected_upgrade_authority.to_string(),
                fee_payer: args.fee_payer.to_string(),
                rpc_origin_redacted: args.origin.redacted_url(),
                genesis_hash: DEVNET_GENESIS_HASH.into(),
                baseline_sha256: baseline.baseline_sha256.clone(),
                baseline_context_slot: baseline.context_slot,
                solana_cli_version: args.expected_solana_cli_version.clone(),
                extension_additional_bytes: baseline.extension_additional_bytes,
                target_rent_exempt_minimum_lamports: baseline.target_rent_exempt_minimum_lamports,
                expected_rent_top_up_lamports: baseline.extension_lamport_top_up,
                before_context_slot: before.context_slot,
                before: before.loader,
                wallet_before_lamports: before.wallet_lamports,
                transaction_signature: None,
                solana_cli_output: None,
                finalized_transaction: None,
                finalized_transaction_sha256: None,
                after_context_slot: None,
                after: None,
                arithmetic: None,
                receipt_sha256: String::new(),
            };
            write_extension_receipt(&args.receipt_path, &mut receipt)?;
            submit_extension_and_finish(runner, args, &shadow, &baseline, &mut receipt)
        }
        Some(mut receipt) if receipt.phase == ReceiptPhaseV1::Prepared => {
            let current = read_snapshot(runner, &shadow, receipt.before_context_slot)?;
            require_baseline_prestate(&baseline, &current.loader)?;
            if current.wallet_lamports != receipt.wallet_before_lamports {
                return Err(Error::new(
                    "prepared extension receipt exists but its payer balance moved; prior \
                     submission outcome is ambiguous and the extension will not replay",
                ));
            }
            submit_extension_and_finish(runner, args, &shadow, &baseline, &mut receipt)
        }
        Some(mut receipt) if receipt.phase == ReceiptPhaseV1::Submitted => {
            finish_submitted_extension(runner, args, &shadow, &baseline, &mut receipt)
        }
        Some(_) => Err(Error::new("unknown extension receipt phase")),
    }
}

fn verify_complete_extension_live(
    runner: &mut impl CliRunner,
    args: &ExtensionArgsV1,
    shadow: &UpgradeArgsV1,
    receipt: &ExtensionReceiptV1,
) -> Result<ExtensionReceiptV1> {
    let after_context_slot = receipt
        .after_context_slot
        .ok_or_else(|| Error::new("complete extension omitted after context slot"))?;
    let current = read_snapshot(runner, shadow, after_context_slot)?;
    if receipt.after.as_ref() != Some(&current.loader)
        || current.loader.upgrade_authority != args.expected_upgrade_authority.to_string()
    {
        return Err(Error::new(
            "complete extension receipt drifted from current Program/ProgramData state, authority, or slot",
        ));
    }
    let transaction = runner.resolve_extension_transaction(&ExtensionTransactionQueryV1 {
        origin: &args.origin,
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        additional_bytes: receipt.extension_additional_bytes,
        deployment_slot: current.loader.deployment_slot,
        programdata_before_lamports: receipt.before.programdata_lamports,
        programdata_after_lamports: current.loader.programdata_lamports,
        wallet_before_lamports: receipt.wallet_before_lamports,
        wallet_after_lamports: receipt
            .arithmetic
            .as_ref()
            .ok_or_else(|| Error::new("complete extension omitted arithmetic"))?
            .wallet_after_lamports,
    })?;
    if receipt.transaction_signature.as_deref() != Some(transaction.signature.as_str())
        || receipt.finalized_transaction.as_ref() != Some(&transaction.transaction)
        || receipt.finalized_transaction_sha256.as_deref()
            != Some(digest(&serde_json::to_vec(&transaction.transaction)?).as_str())
    {
        return Err(Error::new(
            "complete extension receipt drifted from its exact finalized transaction",
        ));
    }
    Ok(receipt.clone())
}

fn submit_extension_and_finish(
    runner: &mut impl CliRunner,
    args: &ExtensionArgsV1,
    shadow: &UpgradeArgsV1,
    baseline: &UpgradeBaselineV1,
    receipt: &mut ExtensionReceiptV1,
) -> Result<ExtensionReceiptV1> {
    let output = invoke(
        runner,
        shadow,
        &[
            "program".into(),
            "extend".into(),
            args.program_id.to_string(),
            baseline.extension_additional_bytes.to_string(),
            "--url".into(),
            args.origin.url().into(),
            "--keypair".into(),
            path_argument(&args.fee_payer_keypair, "--fee-payer-keypair")?,
            "--authority".into(),
            path_argument(&args.authority_keypair, "--authority-keypair")?,
            "--output".into(),
            "json".into(),
        ],
    )?;
    let parsed: Value = serde_json::from_str(output.stdout.trim()).map_err(|error| {
        Error::new(format!(
            "solana program extend did not return one JSON object: {error}"
        ))
    })?;
    let returned_program = parsed
        .get("programId")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("extension output omitted string programId"))?;
    if returned_program != args.program_id.to_string() {
        return Err(Error::new("extension output substituted the Program id"));
    }
    if parsed.get("additionalBytes").and_then(Value::as_u64)
        != Some(baseline.extension_additional_bytes)
    {
        return Err(Error::new(
            "extension output substituted the additional-byte count",
        ));
    }
    receipt.phase = ReceiptPhaseV1::Submitted;
    receipt.solana_cli_output = Some(parsed);
    write_extension_receipt(&args.receipt_path, receipt)?;
    finish_submitted_extension(runner, args, shadow, baseline, receipt)
}

fn finish_submitted_extension(
    runner: &mut impl CliRunner,
    args: &ExtensionArgsV1,
    shadow: &UpgradeArgsV1,
    _baseline: &UpgradeBaselineV1,
    receipt: &mut ExtensionReceiptV1,
) -> Result<ExtensionReceiptV1> {
    authenticate_devnet(runner, shadow)?;
    let after = read_snapshot(runner, shadow, receipt.before_context_slot)?;
    if after.loader.deployment_slot <= receipt.before.deployment_slot {
        return Err(Error::new(format!(
            "extension deployment slot did not advance: before {}, after {}. Agave 4.0.2 \
             ExtendProgram advances this slot; unchanged state is partial or substituted and \
             cannot be replayed.",
            receipt.before.deployment_slot, after.loader.deployment_slot
        )));
    }
    if after.loader.program_lamports != receipt.before.program_lamports
        || after.loader.program_owner != receipt.before.program_owner
        || after.loader.program_executable != receipt.before.program_executable
        || after.loader.program_data_bytes != receipt.before.program_data_bytes
        || after.loader.program_account_sha256 != receipt.before.program_account_sha256
        || after.loader.upgrade_authority != receipt.before.upgrade_authority
    {
        return Err(Error::new(
            "extension changed Program linkage, Program balance, or retained authority",
        ));
    }
    let expected_space = receipt
        .before
        .programdata_data_bytes
        .checked_add(receipt.extension_additional_bytes)
        .ok_or_else(|| Error::new("extension post-space overflow"))?;
    if after.loader.programdata_data_bytes != expected_space {
        return Err(Error::new(format!(
            "partial or substituted extension: ProgramData is {} bytes, expected exact {}",
            after.loader.programdata_data_bytes, expected_space
        )));
    }
    let old_live_bytes = usize::try_from(receipt.before.live_elf_bytes)
        .map_err(|_| Error::new("old live width does not fit this host"))?;
    if after
        .live_elf
        .get(..old_live_bytes)
        .is_none_or(|prefix| digest(prefix) != receipt.before.live_elf_sha256)
    {
        return Err(Error::new(
            "extension changed existing deployed payload bytes before Upgrade",
        ));
    }
    if after
        .live_elf
        .get(old_live_bytes..)
        .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
    {
        return Err(Error::new(
            "extension did not add only zero-initialized Loader capacity",
        ));
    }
    let expected_programdata_lamports = receipt
        .before
        .programdata_lamports
        .checked_add(receipt.expected_rent_top_up_lamports)
        .ok_or_else(|| Error::new("extension ProgramData lamport overflow"))?;
    if after.loader.programdata_lamports != expected_programdata_lamports
        || after.loader.programdata_lamports != receipt.target_rent_exempt_minimum_lamports
    {
        return Err(Error::new(format!(
            "ProgramData lamports after extension are {}; expected exact rent {}",
            after.loader.programdata_lamports, receipt.target_rent_exempt_minimum_lamports
        )));
    }
    let wallet_spend = receipt
        .wallet_before_lamports
        .checked_sub(after.wallet_lamports)
        .ok_or_else(|| Error::new("extension payer wallet increased; arithmetic is ambiguous"))?;
    let observed_fee_and_cli_cost = wallet_spend
        .checked_sub(receipt.expected_rent_top_up_lamports)
        .ok_or_else(|| Error::new("payer spent less than the exact ProgramData rent top-up"))?;
    let programdata_delta = after
        .loader
        .programdata_lamports
        .checked_sub(receipt.before.programdata_lamports)
        .ok_or_else(|| Error::new("ProgramData lamports decreased during extension"))?;
    let transaction = runner.resolve_extension_transaction(&ExtensionTransactionQueryV1 {
        origin: &args.origin,
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        additional_bytes: receipt.extension_additional_bytes,
        deployment_slot: after.loader.deployment_slot,
        programdata_before_lamports: receipt.before.programdata_lamports,
        programdata_after_lamports: after.loader.programdata_lamports,
        wallet_before_lamports: receipt.wallet_before_lamports,
        wallet_after_lamports: after.wallet_lamports,
    })?;
    if transaction.payer_spend_lamports != wallet_spend
        || transaction.programdata_delta_lamports != programdata_delta
        || transaction.fee_lamports != observed_fee_and_cli_cost
    {
        return Err(Error::new(
            "resolved extension transaction arithmetic differs from account observations",
        ));
    }
    receipt.phase = ReceiptPhaseV1::Complete;
    receipt.transaction_signature = Some(transaction.signature);
    receipt.finalized_transaction_sha256 =
        Some(digest(&serde_json::to_vec(&transaction.transaction)?));
    receipt.finalized_transaction = Some(transaction.transaction);
    receipt.after_context_slot = Some(after.context_slot);
    receipt.after = Some(after.loader.clone());
    receipt.arithmetic = Some(ExtensionArithmeticV1 {
        wallet_before_lamports: receipt.wallet_before_lamports,
        wallet_after_lamports: after.wallet_lamports,
        wallet_spend_lamports: wallet_spend,
        rent_top_up_lamports: receipt.expected_rent_top_up_lamports,
        observed_fee_and_cli_cost_lamports: observed_fee_and_cli_cost,
        programdata_before_lamports: receipt.before.programdata_lamports,
        programdata_after_lamports: after.loader.programdata_lamports,
        programdata_delta_lamports: programdata_delta,
        programdata_before_bytes: receipt.before.programdata_data_bytes,
        programdata_after_bytes: after.loader.programdata_data_bytes,
        extension_additional_bytes: receipt.extension_additional_bytes,
    });
    write_extension_receipt(&args.receipt_path, receipt)?;
    Ok(receipt.clone())
}

fn submit_and_finish(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    candidate_live: &[u8],
    receipt: &mut UpgradeReceiptV1,
) -> Result<UpgradeReceiptV1> {
    let output = invoke(
        runner,
        args,
        &[
            "program".into(),
            "deploy".into(),
            path_argument(&args.elf_path, "--elf")?,
            "--url".into(),
            args.origin.url().into(),
            "--keypair".into(),
            path_argument(&args.fee_payer_keypair, "--fee-payer-keypair")?,
            "--upgrade-authority".into(),
            path_argument(&args.authority_keypair, "--authority-keypair")?,
            "--program-id".into(),
            args.program_id.to_string(),
            "--output".into(),
            "json".into(),
        ],
    )?;
    let parsed: Value = serde_json::from_str(output.stdout.trim()).map_err(|error| {
        Error::new(format!(
            "solana program deploy did not return one JSON object: {error}"
        ))
    })?;
    let returned_program = parsed
        .get("programId")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("deploy output omitted string programId"))?;
    if returned_program != args.program_id.to_string() {
        return Err(Error::new(format!(
            "deploy output substituted program {returned_program}; expected {}",
            args.program_id
        )));
    }
    let signature = parsed
        .get("signature")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("deploy output omitted string signature"))?;
    Signature::from_str(signature)
        .map_err(|_| Error::new("deploy output signature is not a Solana signature"))?;
    receipt.phase = ReceiptPhaseV1::Submitted;
    receipt.transaction_signature = Some(signature.into());
    receipt.solana_cli_output = Some(parsed);
    write_receipt(&args.receipt_path, receipt)?;
    finish_submitted(runner, args, gate, candidate_live, receipt)
}

fn finish_submitted(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    candidate_live: &[u8],
    receipt: &mut UpgradeReceiptV1,
) -> Result<UpgradeReceiptV1> {
    authenticate_devnet(runner, args)?;
    let after = read_snapshot(runner, args, receipt.before_context_slot)?;
    if after.loader.deployment_slot <= receipt.before.deployment_slot {
        return Err(Error::new(format!(
            "deployment slot did not advance: before {}, after {}. The CLI returned a signature, \
             so replay is refused; inspect that transaction instead.",
            receipt.before.deployment_slot, after.loader.deployment_slot
        )));
    }
    if after.live_elf != candidate_live || after.loader.live_elf_sha256 != receipt.live_elf_sha256 {
        return Err(Error::new(
            "post-Upgrade ProgramData payload is not the checked raw ELF plus its exact allowed \
             zero padding",
        ));
    }
    if after.loader.upgrade_authority != args.expected_upgrade_authority.to_string() {
        return Err(Error::new("post-Upgrade retained authority drifted"));
    }
    if after.loader.programdata_lamports != receipt.before.programdata_lamports {
        return Err(Error::new(format!(
            "ProgramData lamports moved from {} to {}; this exact-allocation Upgrade must not \
             resize or spend parked program rent",
            receipt.before.programdata_lamports, after.loader.programdata_lamports
        )));
    }
    let signature = receipt
        .transaction_signature
        .as_deref()
        .ok_or_else(|| Error::new("submitted Upgrade receipt omitted transaction signature"))?;
    let transaction = runner.resolve_upgrade_transaction(&UpgradeTransactionQueryV1 {
        origin: &args.origin,
        signature,
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        deployment_slot: after.loader.deployment_slot,
        programdata_before_lamports: receipt.before.programdata_lamports,
        programdata_after_lamports: after.loader.programdata_lamports,
    })?;
    let programdata_delta = i128::from(after.loader.programdata_lamports)
        .checked_sub(i128::from(receipt.before.programdata_lamports))
        .ok_or_else(|| Error::new("ProgramData lamport delta overflow"))?;
    let operation_observed_net_spend_lamports = receipt
        .wallet_before_lamports
        .checked_sub(after.wallet_lamports)
        .ok_or_else(|| Error::new("exclusive payer wallet increased across the deploy window"))?;
    let unattributed_cli_net_cost_lamports = operation_observed_net_spend_lamports
        .checked_sub(transaction.fee_lamports)
        .ok_or_else(|| {
            Error::new(
                "operation-wide observed payer spend is smaller than the final Upgrade transaction fee",
            )
        })?;

    let (dump_sha256, dump_shape) = verify_dump(runner, args, &gate.raw_elf, candidate_live)?;
    receipt.phase = ReceiptPhaseV1::Complete;
    receipt.finalized_transaction = Some(transaction.transaction);
    receipt.finalized_transaction_sha256 = Some(transaction.transaction_sha256);
    receipt.after_context_slot = Some(after.context_slot);
    receipt.after = Some(after.loader.clone());
    receipt.arithmetic = Some(UpgradeArithmeticV1 {
        transaction_payer_pre_lamports: transaction.payer_pre_lamports,
        transaction_payer_post_lamports: transaction.payer_post_lamports,
        transaction_fee_lamports: transaction.fee_lamports,
        payer_fee_delta_lamports: transaction
            .payer_pre_lamports
            .checked_sub(transaction.payer_post_lamports)
            .ok_or_else(|| Error::new("validated Upgrade payer delta underflow"))?,
        programdata_before_lamports: transaction.programdata_pre_lamports,
        programdata_after_lamports: transaction.programdata_post_lamports,
        programdata_delta_lamports: programdata_delta,
        operation_wallet_before_lamports: receipt.wallet_before_lamports,
        operation_wallet_after_lamports: after.wallet_lamports,
        operation_observed_net_spend_lamports,
        unattributed_cli_net_cost_lamports,
        accounting_scope: OPERATION_ACCOUNTING_SCOPE_V1.into(),
        cost_attribution: OPERATION_COST_ATTRIBUTION_V1.into(),
    });
    receipt.dump_sha256 = Some(dump_sha256);
    receipt.dump_shape = Some(dump_shape);
    write_receipt(&args.receipt_path, receipt)?;
    Ok(receipt.clone())
}

fn verify_complete_receipt_live(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    candidate_live: &[u8],
    receipt: &UpgradeReceiptV1,
) -> Result<UpgradeReceiptV1> {
    let minimum_context_slot = receipt
        .after_context_slot
        .ok_or_else(|| Error::new("complete receipt omitted after context slot"))?;
    let observation = read_snapshot(runner, args, minimum_context_slot)?;
    verify_complete_observation(args, gate, candidate_live, receipt, &observation)?;
    verify_recorded_upgrade_transaction(runner, args, receipt, &observation)?;
    Ok(receipt.clone())
}

fn verify_recorded_upgrade_transaction(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    receipt: &UpgradeReceiptV1,
    observation: &SnapshotV1,
) -> Result<()> {
    let signature = receipt
        .transaction_signature
        .as_deref()
        .ok_or_else(|| Error::new("complete receipt omitted Upgrade signature"))?;
    let transaction = runner.resolve_upgrade_transaction(&UpgradeTransactionQueryV1 {
        origin: &args.origin,
        signature,
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        deployment_slot: observation.loader.deployment_slot,
        programdata_before_lamports: receipt.before.programdata_lamports,
        programdata_after_lamports: observation.loader.programdata_lamports,
    })?;
    let arithmetic = receipt
        .arithmetic
        .as_ref()
        .ok_or_else(|| Error::new("complete receipt omitted Upgrade arithmetic"))?;
    if receipt.finalized_transaction.as_ref() != Some(&transaction.transaction)
        || receipt.finalized_transaction_sha256.as_deref()
            != Some(transaction.transaction_sha256.as_str())
        || arithmetic.transaction_payer_pre_lamports != transaction.payer_pre_lamports
        || arithmetic.transaction_payer_post_lamports != transaction.payer_post_lamports
        || arithmetic.transaction_fee_lamports != transaction.fee_lamports
        || arithmetic.payer_fee_delta_lamports != transaction.fee_lamports
        || arithmetic.programdata_before_lamports != transaction.programdata_pre_lamports
        || arithmetic.programdata_after_lamports != transaction.programdata_post_lamports
        || arithmetic.programdata_delta_lamports != 0
    {
        return Err(Error::new(
            "complete receipt drifted from its exact finalized Upgrade transaction or arithmetic",
        ));
    }
    Ok(())
}

fn verify_complete_observation(
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    candidate_live: &[u8],
    receipt: &UpgradeReceiptV1,
    observation: &SnapshotV1,
) -> Result<()> {
    let recorded_after = receipt
        .after
        .as_ref()
        .ok_or_else(|| Error::new("complete receipt omitted poststate"))?;
    if &observation.loader != recorded_after
        || observation.live_elf != candidate_live
        || observation.loader.live_elf_sha256 != receipt.live_elf_sha256
        || observation.loader.upgrade_authority != args.expected_upgrade_authority.to_string()
    {
        return Err(Error::new(
            "complete Upgrade receipt drifted from current Program/ProgramData payload, authority, or deployment slot",
        ));
    }
    let dump = read_dump_regular(&args.dump_path).map_err(|error| {
        Error::new(format!(
            "complete Upgrade dump {} is unavailable: {error}",
            args.dump_path.display()
        ))
    })?;
    let (dump_sha256, dump_shape) = classify_dump(&dump, &gate.raw_elf, candidate_live)?;
    if receipt.dump_sha256.as_deref() != Some(dump_sha256.as_str())
        || receipt.dump_shape.as_deref() != Some(dump_shape.as_str())
    {
        return Err(Error::new(
            "complete Upgrade dump bytes or recorded dump shape drifted",
        ));
    }
    Ok(())
}

fn validate_checked_release_gate(args: &UpgradeArgsV1) -> Result<ValidatedUpgradeGateV1> {
    require_digest(
        &args.expected_checked_release_gate_sha256,
        "expected checked-release gate SHA-256",
    )?;
    require_lower_hex(
        &args.expected_source_revision,
        "expected source revision",
        40,
        40,
    )?;
    require_digest(
        &args.expected_source_tree_sha256,
        "expected source tree SHA-256",
    )?;
    let gate_metadata = fs::symlink_metadata(&args.checked_release_gate_path).map_err(|error| {
        Error::new(format!(
            "checked-release gate {} cannot be inspected: {error}",
            args.checked_release_gate_path.display()
        ))
    })?;
    if gate_metadata.file_type().is_symlink() || !gate_metadata.file_type().is_file() {
        return Err(Error::new(
            "checked-release gate must itself be one regular non-symlink file",
        ));
    }
    if args
        .checked_release_gate_path
        .file_name()
        .and_then(|name| name.to_str())
        != Some("CHECKED_UPGRADE_GATE.json")
    {
        return Err(Error::new(
            "checked-release gate must retain its canonical CHECKED_UPGRADE_GATE.json name",
        ));
    }
    let gate_bytes = fs::read(&args.checked_release_gate_path)?;
    let gate_sha256 = digest(&gate_bytes);
    if gate_sha256 != args.expected_checked_release_gate_sha256 {
        return Err(Error::new(format!(
            "checked-release gate SHA-256 is {gate_sha256}, expected {}",
            args.expected_checked_release_gate_sha256
        )));
    }
    let gate: CheckedUpgradeGateV1 = serde_json::from_slice(&gate_bytes).map_err(|error| {
        Error::new(format!(
            "checked-release gate is not canonical v1 JSON; handwritten acceptance has no \
             authority: {error}"
        ))
    })?;
    if gate.schema != CHECKED_GATE_SCHEMA {
        return Err(Error::new(format!(
            "checked-release gate schema is {:?}; expected {CHECKED_GATE_SCHEMA}; handwritten \
             acceptance has no authority",
            gate.schema
        )));
    }
    if gate.source_revision != args.expected_source_revision
        || gate.source_tree_sha256 != args.expected_source_tree_sha256
    {
        return Err(Error::new(
            "checked-release gate source commit/tree differs from the explicitly expected source",
        ));
    }
    require_lower_hex(&gate.source_revision, "gate source revision", 40, 40)?;
    require_digest(&gate.source_tree_sha256, "gate source tree SHA-256")?;
    require_lower_hex(&gate.build_run_id, "gate build run id", 64, 64)?;
    if gate.solana_cli_version.trim().is_empty() {
        return Err(Error::new(
            "checked-release gate omitted solana_cli_version",
        ));
    }
    if gate.link_count != u64::try_from(SHIPPED_LINKS.len()).expect("thirteen links fit u64")
        || gate.links.len() != SHIPPED_LINKS.len()
    {
        return Err(Error::new(format!(
            "checked-release gate must carry all {} shipped links exactly once; declared {}, \
             carried {}",
            SHIPPED_LINKS.len(),
            gate.link_count,
            gate.links.len()
        )));
    }

    let mut seen = BTreeSet::new();
    for link in &gate.links {
        if !seen.insert(link.label.as_str()) {
            return Err(Error::new(format!(
                "checked-release gate duplicated link role {}",
                link.label
            )));
        }
        if !SHIPPED_LINKS
            .iter()
            .any(|(label, _, _)| *label == link.label)
        {
            return Err(Error::new(format!(
                "checked-release gate carried unknown link role {}",
                link.label
            )));
        }
    }
    for (label, _, _) in SHIPPED_LINKS {
        if !seen.contains(label) {
            return Err(Error::new(format!(
                "checked-release gate omitted shipped link role {label}"
            )));
        }
    }
    for (link, (expected_label, expected_package, produces_artifact)) in
        gate.links.iter().zip(SHIPPED_LINKS)
    {
        if link.label != *expected_label || link.package != *expected_package {
            return Err(Error::new(format!(
                "checked-release gate link order/identity is not canonical at {}; expected \
                 {expected_label}<TAB>{expected_package}",
                link.label
            )));
        }
        if link.elf.is_some() != *produces_artifact
            || link.checked_manifest.is_some() != *produces_artifact
        {
            return Err(Error::new(format!(
                "checked-release gate link {} has the wrong release-artifact shape",
                link.label
            )));
        }
    }

    let gate_path = fs::canonicalize(&args.checked_release_gate_path)?;
    let root = gate_path
        .parent()
        .ok_or_else(|| Error::new("checked-release gate has no evidence root"))?
        .to_path_buf();
    let source_tree =
        verify_gate_file(&root, &gate.source_tree_manifest, "source tree manifest")?.1;
    if gate.source_tree_manifest.sha256 != gate.source_tree_sha256 || source_tree.is_empty() {
        return Err(Error::new(
            "checked-release gate source tree manifest does not bind the declared source tree",
        ));
    }
    let build_links = verify_gate_file(&root, &gate.build_links_manifest, "build-link manifest")?.1;
    let expected_build_links = SHIPPED_LINKS
        .iter()
        .map(|(label, package, _)| format!("{label}\t{package}\n"))
        .collect::<String>();
    if build_links != expected_build_links.as_bytes() {
        return Err(Error::new(
            "checked-release gate build-link manifest is missing, reordered, or names an unknown \
             shipped link",
        ));
    }
    let build_run = verify_gate_file(&root, &gate.build_run_manifest, "build-run manifest")?.1;
    let expected_build_run = format!("dclutch-sbf-build-run-v1={}\n", gate.build_run_id);
    if build_run != expected_build_run.as_bytes() {
        return Err(Error::new(
            "checked-release gate build-run manifest does not bind its run id",
        ));
    }
    let diagnostics =
        verify_gate_file(&root, &gate.diagnostics_manifest, "diagnostics manifest")?.1;
    let expected_diagnostics = SHIPPED_LINKS
        .iter()
        .map(|(label, _, _)| format!("{label}=0\n"))
        .collect::<String>();
    if diagnostics != expected_diagnostics.as_bytes() {
        return Err(Error::new(
            "checked-release gate diagnostics manifest is not the exact zero row for every \
             shipped link",
        ));
    }

    let mut selected = None;
    for link in &gate.links {
        if link.sbf_diagnostics_count != 0
            || link.frame_count == 0
            || link.frame_bound_bytes != 4096
            || link.frames_at_or_over_bound != 0
            || link.deepest_frame_bytes >= link.frame_bound_bytes
        {
            return Err(Error::new(format!(
                "checked-release gate link {} is not a fresh zero-diagnostic, below-bound frame \
                 report",
                link.label
            )));
        }
        let build_log =
            verify_gate_file(&root, &link.build_log, &format!("{} build log", link.label))?.1;
        validate_compile_log(
            &build_log,
            &format!("dclutch-sbf-build-run-v1={}", gate.build_run_id),
            &link.package,
            &link.compile_marker,
            &format!("{} build log", link.label),
        )?;
        let frame_build_log = verify_gate_file(
            &root,
            &link.frame_build_log,
            &format!("{} frame build log", link.label),
        )?
        .1;
        validate_compile_log(
            &frame_build_log,
            &format!("dclutch-sbf-frame-run-v1={}", gate.build_run_id),
            &link.package,
            &link.frame_compile_marker,
            &format!("{} frame build log", link.label),
        )?;
        let frame_report = verify_gate_file(
            &root,
            &link.frame_report,
            &format!("{} frame report", link.label),
        )?
        .1;
        validate_frame_report(link, &frame_report)?;
        if let Some(manifest) = &link.checked_manifest {
            verify_gate_file(&root, manifest, &format!("{} checked manifest", link.label))?;
        }
        if let Some(elf) = &link.elf {
            let (canonical, raw_elf) =
                verify_gate_file(&root, elf, &format!("{} ELF", link.label))?;
            if raw_elf.get(..4) != Some(b"\x7fELF") {
                return Err(Error::new(format!(
                    "checked-release gate {} ELF does not begin with ELF magic",
                    link.label
                )));
            }
            if link.label == args.role {
                let argument_metadata = fs::symlink_metadata(&args.elf_path)?;
                if argument_metadata.file_type().is_symlink()
                    || !argument_metadata.file_type().is_file()
                {
                    return Err(Error::new(
                        "selected candidate ELF must be one regular non-symlink file",
                    ));
                }
                if fs::canonicalize(&args.elf_path)? != canonical {
                    return Err(Error::new(format!(
                        "selected {} ELF path is not the gate's exact canonical role ELF",
                        args.role
                    )));
                }
                selected = Some((raw_elf, elf.sha256.clone()));
            }
        }
    }
    let (raw_elf, raw_elf_sha256) = selected.ok_or_else(|| {
        Error::new(format!(
            "checked-release gate carries no deployable ELF for selected role {}",
            args.role
        ))
    })?;
    Ok(ValidatedUpgradeGateV1 {
        gate_sha256,
        source_revision: gate.source_revision,
        source_tree_sha256: gate.source_tree_sha256,
        solana_cli_version: gate.solana_cli_version,
        raw_elf,
        raw_elf_sha256,
    })
}

fn verify_gate_file(root: &Path, evidence: &GateFileV1, label: &str) -> Result<(PathBuf, Vec<u8>)> {
    require_digest(&evidence.sha256, &format!("{label} SHA-256"))?;
    let relative = Path::new(&evidence.canonical_path);
    if evidence.canonical_path.is_empty()
        || evidence.canonical_path.contains('\\')
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(Error::new(format!(
            "{label} canonical path is absolute, empty, or contains an escape"
        )));
    }
    let joined = root.join(relative);
    let metadata = fs::symlink_metadata(&joined).map_err(|error| {
        Error::new(format!(
            "{label} {} cannot be inspected: {error}",
            joined.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::new(format!(
            "{label} must be one regular non-symlink file"
        )));
    }
    let canonical = fs::canonicalize(&joined)?;
    if !canonical.starts_with(root) || canonical != joined {
        return Err(Error::new(format!(
            "{label} canonical path escapes the gate root or traverses a symlink"
        )));
    }
    let bytes = fs::read(&canonical)?;
    if u64::try_from(bytes.len()).ok() != Some(evidence.bytes) || digest(&bytes) != evidence.sha256
    {
        return Err(Error::new(format!(
            "{label} bytes or SHA-256 changed after checked-release admission"
        )));
    }
    Ok((canonical, bytes))
}

fn validate_compile_log(
    bytes: &[u8],
    expected_header: &str,
    package: &str,
    marker: &str,
    label: &str,
) -> Result<()> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| Error::new(format!("{label} is not UTF-8")))?;
    if !text.ends_with('\n') || text.lines().next() != Some(expected_header) {
        return Err(Error::new(format!(
            "{label} is missing the exact current-run freshness marker"
        )));
    }
    let trimmed = marker.trim_start();
    let expected_prefix = format!("Compiling {package} v");
    if marker.bytes().any(|byte| matches!(byte, b'\n' | b'\r'))
        || !trimmed.starts_with(&expected_prefix)
        || !text.lines().any(|line| line == marker)
    {
        return Err(Error::new(format!(
            "{label} is missing the exact fresh top-package compile marker for {package}"
        )));
    }
    Ok(())
}

fn validate_frame_report(link: &CheckedGateLinkV1, bytes: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::new(format!("{} frame report is not UTF-8", link.label)))?;
    let expected = format!(
        "dclutch-sbf-frame-report-v1\nlabel={}\npackage={}\nframe_count={}\n\
         frame_bound_bytes={}\nframes_at_or_over_bound={}\ndeepest_frame_bytes={}\n",
        link.label,
        link.package,
        link.frame_count,
        link.frame_bound_bytes,
        link.frames_at_or_over_bound,
        link.deepest_frame_bytes
    );
    if !text.starts_with(&expected) {
        return Err(Error::new(format!(
            "{} frame report fields differ from the checked gate",
            link.label
        )));
    }
    let object_digest = text
        .lines()
        .nth(7)
        .and_then(|line| line.strip_prefix("object_sha256="))
        .ok_or_else(|| {
            Error::new(format!(
                "{} frame report omitted object SHA-256",
                link.label
            ))
        })?;
    require_digest(
        object_digest,
        &format!("{} frame object SHA-256", link.label),
    )?;
    if text.lines().nth(8) != Some("measurement_output:") {
        return Err(Error::new(format!(
            "{} frame report omitted measurement output",
            link.label
        )));
    }
    Ok(())
}

fn validate_baseline(args: &UpgradeArgsV1, baseline: &UpgradeBaselineV1) -> Result<()> {
    let expected_order = ROLES
        .iter()
        .map(|role| String::from(*role))
        .collect::<Vec<_>>();
    if baseline.schema != BASELINE_SCHEMA
        || baseline.canonical_role_order != expected_order
        || baseline.role_ordinal != role_ordinal(&args.role)?
        || baseline.role != args.role
        || baseline.program_id != args.program_id.to_string()
        || baseline.programdata_id != args.programdata_id.to_string()
        || baseline.expected_upgrade_authority != args.expected_upgrade_authority.to_string()
        || baseline.genesis_hash != DEVNET_GENESIS_HASH
        || baseline.context_slot == 0
    {
        return Err(Error::new(
            "Upgrade baseline does not bind the canonical role order, exact devnet genesis, \
             selected role, Program, ProgramData, retained authority, and nonzero finalized \
             context slot",
        ));
    }
    let expected_digest = baseline_digest(baseline)?;
    if baseline.baseline_sha256 != expected_digest {
        return Err(Error::new(format!(
            "Upgrade baseline digest is {}, canonical fields hash to {expected_digest}",
            baseline.baseline_sha256
        )));
    }
    require_digest(
        &baseline.observation.program_account_sha256,
        "baseline Program digest",
    )?;
    require_digest(
        &baseline.observation.programdata_account_sha256,
        "baseline ProgramData digest",
    )?;
    require_digest(
        &baseline.observation.live_elf_sha256,
        "baseline live ELF digest",
    )?;
    if baseline.observation.program_owner != bpf_loader_upgradeable::ID.to_string()
        || !baseline.observation.program_executable
        || baseline.observation.programdata_owner != bpf_loader_upgradeable::ID.to_string()
        || baseline.observation.programdata_executable
        || baseline.observation.program_data_bytes != 36
        || baseline.observation.programdata_data_bytes
            != baseline
                .observation
                .live_elf_bytes
                .checked_add(45)
                .ok_or_else(|| Error::new("baseline ProgramData width overflow"))?
        || baseline.observation.upgrade_authority != args.expected_upgrade_authority.to_string()
        || baseline.target_live_elf_bytes == 0
        || baseline.extension_additional_bytes
            != baseline
                .target_live_elf_bytes
                .checked_add(45)
                .ok_or_else(|| Error::new("baseline target width overflow"))?
                .max(baseline.observation.programdata_data_bytes)
                .checked_sub(baseline.observation.programdata_data_bytes)
                .ok_or_else(|| Error::new("baseline extension underflow"))?
        || baseline.current_rent_exempt_minimum_lamports
            != baseline.observation.programdata_lamports
        || baseline.target_rent_exempt_minimum_lamports
            < baseline.current_rent_exempt_minimum_lamports
        || baseline.extension_lamport_top_up
            != baseline
                .target_rent_exempt_minimum_lamports
                .saturating_sub(baseline.observation.programdata_lamports)
    {
        return Err(Error::new(
            "Upgrade baseline Loader owners, executable flags, lengths, or authority are invalid",
        ));
    }
    Ok(())
}

fn require_baseline_prestate(
    baseline: &UpgradeBaselineV1,
    observed: &LoaderObservationV1,
) -> Result<()> {
    if observed != &baseline.observation {
        return Err(Error::new(format!(
            "fresh finalized Program/ProgramData observation does not exactly match baseline {} \
             at context slot {}; refuse stale slot, authority, owner, privilege, length, lamport, \
             or account-digest state before Upgrade",
            baseline.baseline_sha256, baseline.context_slot
        )));
    }
    Ok(())
}

fn baseline_digest(baseline: &UpgradeBaselineV1) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/devnet-upgrade-baseline/v1\0");
    hash_text(&mut hasher, &baseline.schema)?;
    for role in &baseline.canonical_role_order {
        hash_text(&mut hasher, role)?;
    }
    hasher.update([baseline.role_ordinal]);
    hash_text(&mut hasher, &baseline.role)?;
    hasher.update(parse_pubkey(&baseline.program_id, "baseline Program")?.as_ref());
    hasher.update(parse_pubkey(&baseline.programdata_id, "baseline ProgramData")?.as_ref());
    hasher.update(
        parse_pubkey(
            &baseline.expected_upgrade_authority,
            "baseline retained authority",
        )?
        .as_ref(),
    );
    hash_text(&mut hasher, &baseline.genesis_hash)?;
    hasher.update(baseline.context_slot.to_le_bytes());
    let observation = &baseline.observation;
    hasher.update(observation.program_lamports.to_le_bytes());
    hash_text(&mut hasher, &observation.program_owner)?;
    hasher.update([u8::from(observation.program_executable)]);
    hasher.update(observation.program_data_bytes.to_le_bytes());
    hash_text(&mut hasher, &observation.program_account_sha256)?;
    hasher.update(observation.programdata_lamports.to_le_bytes());
    hash_text(&mut hasher, &observation.programdata_owner)?;
    hasher.update([u8::from(observation.programdata_executable)]);
    hasher.update(observation.programdata_data_bytes.to_le_bytes());
    hash_text(&mut hasher, &observation.programdata_account_sha256)?;
    hasher.update(observation.deployment_slot.to_le_bytes());
    hash_text(&mut hasher, &observation.upgrade_authority)?;
    hasher.update(observation.live_elf_bytes.to_le_bytes());
    hash_text(&mut hasher, &observation.live_elf_sha256)?;
    hasher.update(baseline.target_live_elf_bytes.to_le_bytes());
    hasher.update(baseline.extension_additional_bytes.to_le_bytes());
    hasher.update(baseline.current_rent_exempt_minimum_lamports.to_le_bytes());
    hasher.update(baseline.target_rent_exempt_minimum_lamports.to_le_bytes());
    hasher.update(baseline.extension_lamport_top_up.to_le_bytes());
    Ok(hex(&hasher.finalize()))
}

fn hash_text(hasher: &mut Sha256, value: &str) -> Result<()> {
    let length = u64::try_from(value.len()).map_err(|_| Error::new("hash field is too long"))?;
    hasher.update(length.to_le_bytes());
    hasher.update(value.as_bytes());
    Ok(())
}

fn candidate_live_image(raw_elf: &[u8], target_live_bytes: u64) -> Result<Vec<u8>> {
    if raw_elf.get(..4) != Some(b"\x7fELF") {
        return Err(Error::new(
            "checked candidate does not begin with ELF magic",
        ));
    }
    let capacity = usize::try_from(target_live_bytes)
        .map_err(|_| Error::new("baseline live ELF width does not fit this host"))?;
    if raw_elf.len() > capacity {
        return Err(Error::new(format!(
            "checked raw ELF is {} bytes but the baseline live width is only {capacity}; capture \
             an extension baseline first",
            raw_elf.len()
        )));
    }
    let mut live = Vec::with_capacity(capacity);
    live.extend_from_slice(raw_elf);
    live.resize(capacity, 0);
    Ok(live)
}

fn authenticate_devnet(runner: &mut impl CliRunner, args: &UpgradeArgsV1) -> Result<()> {
    let output = invoke(
        runner,
        args,
        &[
            "genesis-hash".into(),
            "--url".into(),
            args.origin.url().into(),
        ],
    )?;
    let observed = output.stdout.trim();
    if observed == MAINNET_BETA_GENESIS_HASH {
        return Err(Error::new(
            "Solana CLI observed mainnet-beta genesis; mainnet is unconditionally refused",
        ));
    }
    if observed != DEVNET_GENESIS_HASH {
        return Err(Error::new(format!(
            "Solana CLI observed genesis {observed:?}; devnet-upgrade-v1 accepts only exact \
             devnet {DEVNET_GENESIS_HASH} (testnet and unknown clusters are refused)"
        )));
    }
    Ok(())
}

fn authenticate_keypair(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    path: &Path,
    expected: Pubkey,
    label: &str,
) -> Result<()> {
    let output = invoke(
        runner,
        args,
        &["address".into(), "-k".into(), path_argument(path, label)?],
    )?;
    let observed = parse_pubkey(output.stdout.trim(), label)?;
    if observed != expected {
        return Err(Error::new(format!(
            "{label} keypair path resolves to {observed}, expected {expected}"
        )));
    }
    Ok(())
}

fn read_snapshot(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    minimum_context_slot: u64,
) -> Result<SnapshotV1> {
    runner.read_snapshot(&SnapshotQueryV1 {
        origin: &args.origin,
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        expected_upgrade_authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        minimum_context_slot,
    })
}

fn read_snapshot_via_rpc(query: &SnapshotQueryV1<'_>) -> Result<SnapshotV1> {
    let mut rpc = Rpc::connect_cluster(query.origin, WritePolicyV1::ReadsOnly)?;
    let (context_slot, accounts) = rpc.finalized_accounts(
        &[query.program_id, query.programdata_id, query.payer],
        query.minimum_context_slot,
    )?;
    let mut accounts = accounts.into_iter();
    let program = RpcAccountV1::from(
        accounts
            .next()
            .flatten()
            .ok_or_else(|| Error::new(format!("missing Program account {}", query.program_id)))?,
    );
    let programdata = RpcAccountV1::from(accounts.next().flatten().ok_or_else(|| {
        Error::new(format!(
            "missing ProgramData account {}",
            query.programdata_id
        ))
    })?);
    let wallet = RpcAccountV1::from(
        accounts
            .next()
            .flatten()
            .ok_or_else(|| Error::new(format!("missing fee payer account {}", query.payer)))?,
    );
    let loader = loader_observation(
        query.program_id,
        query.programdata_id,
        query.expected_upgrade_authority,
        &program,
        &programdata,
    )?;
    let view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| Error::new("ProgramData is not exact Loader-v3 ProgramData state"))?;
    Ok(SnapshotV1 {
        context_slot,
        loader,
        wallet_lamports: wallet.lamports,
        live_elf: view.elf().to_vec(),
    })
}

fn loader_observation(
    program_id: Pubkey,
    programdata_id: Pubkey,
    expected_upgrade_authority: Pubkey,
    program: &RpcAccountV1,
    programdata: &RpcAccountV1,
) -> Result<LoaderObservationV1> {
    if program.owner != bpf_loader_upgradeable::ID || !program.executable {
        return Err(Error::new(
            "Program is not executable and owned by Loader V3",
        ));
    }
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(|_| Error::new("Program account is not exact Loader-v3 Program state"))?;
    let linked_programdata = Pubkey::from(program_view.programdata());
    if linked_programdata != programdata_id {
        return Err(Error::new(format!(
            "Program {program_id} links ProgramData {linked_programdata}, expected {programdata_id}"
        )));
    }

    if programdata.owner != bpf_loader_upgradeable::ID || programdata.executable {
        return Err(Error::new(
            "ProgramData is not non-executable and owned by Loader V3",
        ));
    }
    let view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| Error::new("ProgramData is not exact Loader-v3 ProgramData state"))?;
    let authority = view
        .upgrade_authority()
        .map(Pubkey::from)
        .ok_or_else(|| Error::new("ProgramData upgrade authority is revoked"))?;
    if authority != expected_upgrade_authority {
        return Err(Error::new(format!(
            "ProgramData retained authority is {authority}, expected {expected_upgrade_authority}"
        )));
    }
    Ok(LoaderObservationV1 {
        program_lamports: program.lamports,
        program_owner: program.owner.to_string(),
        program_executable: program.executable,
        program_data_bytes: u64::try_from(program.data.len())
            .map_err(|_| Error::new("Program width does not fit u64"))?,
        program_account_sha256: digest(&program.data),
        programdata_lamports: programdata.lamports,
        programdata_owner: programdata.owner.to_string(),
        programdata_executable: programdata.executable,
        programdata_data_bytes: u64::try_from(programdata.data.len())
            .map_err(|_| Error::new("ProgramData width does not fit u64"))?,
        deployment_slot: view.deployment_slot(),
        upgrade_authority: authority.to_string(),
        live_elf_bytes: u64::try_from(view.elf().len())
            .map_err(|_| Error::new("live ELF width does not fit u64"))?,
        live_elf_sha256: digest(view.elf()),
        programdata_account_sha256: digest(&programdata.data),
    })
}

fn require_candidate_fits(before: &SnapshotV1, candidate_live: &[u8]) -> Result<()> {
    if before.live_elf.len() != candidate_live.len() {
        return Err(Error::new(format!(
            "checked candidate live width is {} but existing ProgramData allocation exposes {}; \
             permanent-id Upgrade v1 never reallocates",
            candidate_live.len(),
            before.live_elf.len()
        )));
    }
    Ok(())
}

fn verify_dump(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    raw_elf: &[u8],
    candidate_live: &[u8],
) -> Result<(String, String)> {
    let dump = if args.dump_path.exists() {
        read_dump_regular(&args.dump_path)?
    } else {
        invoke(
            runner,
            args,
            &[
                "program".into(),
                "dump".into(),
                args.program_id.to_string(),
                path_argument(&args.dump_path, "--dump")?,
                "--url".into(),
                args.origin.url().into(),
            ],
        )?;
        read_dump_regular(&args.dump_path).map_err(|error| {
            Error::new(format!(
                "Solana CLI reported a successful dump but {} could not be read: {error}",
                args.dump_path.display()
            ))
        })?
    };
    classify_dump(&dump, raw_elf, candidate_live)
}

fn read_dump_regular(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::new(format!(
            "deployed-byte dump {} is not one regular non-symlink file",
            path.display()
        )));
    }
    Ok(fs::read(path)?)
}

fn classify_dump(dump: &[u8], raw_elf: &[u8], candidate_live: &[u8]) -> Result<(String, String)> {
    let shape = if dump == raw_elf {
        "raw-elf"
    } else if dump == candidate_live {
        "live-elf-with-zero-padding"
    } else {
        return Err(Error::new(
            "deployed-byte dump is neither the checked raw ELF nor the exact checked live \
             raw-plus-zero-padding image",
        ));
    };
    Ok((digest(dump), shape.into()))
}

fn invoke(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    arguments: &[String],
) -> Result<CliOutput> {
    let output = runner.run(arguments)?;
    if !output.success {
        let stderr = redact(&output.stderr, args);
        let stdout = redact(&output.stdout, args);
        return Err(Error::new(format!(
            "Solana CLI command failed; stdout={stdout:?}; stderr={stderr:?}"
        )));
    }
    Ok(output)
}

fn resolve_upgrade_transaction_via_rpc(
    query: &UpgradeTransactionQueryV1<'_>,
) -> Result<UpgradeTransactionEvidenceV1> {
    let mut rpc = Rpc::connect_cluster(query.origin, WritePolicyV1::ReadsOnly)?;
    let transaction = rpc.call(
        "getTransaction",
        &serde_json::json!([query.signature, {
            "encoding": "jsonParsed",
            "commitment": "finalized",
            "maxSupportedTransactionVersion": 0
        }]),
    )?;
    validate_upgrade_transaction(query, transaction)
}

fn validate_upgrade_transaction(
    query: &UpgradeTransactionQueryV1<'_>,
    transaction: Value,
) -> Result<UpgradeTransactionEvidenceV1> {
    if transaction.get("slot").and_then(Value::as_u64) != Some(query.deployment_slot) {
        return Err(Error::new(
            "finalized Upgrade transaction slot differs from ProgramData deployment slot",
        ));
    }
    let meta = transaction
        .get("meta")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("Upgrade transaction omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(Error::new("finalized Upgrade transaction failed"));
    }
    let message = transaction
        .get("transaction")
        .and_then(|value| value.get("message"))
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("Upgrade transaction omitted parsed message"))?;
    let signatures = transaction
        .get("transaction")
        .and_then(|value| value.get("signatures"))
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("Upgrade transaction omitted signatures"))?;
    if signatures.first().and_then(Value::as_str) != Some(query.signature) {
        return Err(Error::new(
            "CLI-returned Upgrade signature is not the transaction fee-payer signature",
        ));
    }
    let account_keys = message
        .get("accountKeys")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("Upgrade transaction omitted account keys"))?;
    let key_index = |wanted: Pubkey, signer: bool, writable: bool| -> Result<usize> {
        let wanted = wanted.to_string();
        let matches = account_keys
            .iter()
            .enumerate()
            .filter(|(_, key)| {
                key.get("pubkey").and_then(Value::as_str) == Some(wanted.as_str())
                    && (!signer || key.get("signer").and_then(Value::as_bool) == Some(true))
                    && (!writable || key.get("writable").and_then(Value::as_bool) == Some(true))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::new(format!(
                "Upgrade transaction did not contain one exact{}{} account key {wanted}",
                if signer { " signer" } else { "" },
                if writable { " writable" } else { "" }
            )));
        }
        Ok(matches[0])
    };
    let payer_index = key_index(query.payer, true, true)?;
    if payer_index != 0 {
        return Err(Error::new(
            "Upgrade transaction payer is not canonical account key zero",
        ));
    }
    let programdata_index = key_index(query.programdata_id, false, true)?;
    let _program_index = key_index(query.program_id, false, true)?;
    let _authority_index = key_index(query.authority, true, false)?;

    let instructions = message
        .get("instructions")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("Upgrade transaction omitted instructions"))?;
    let matching = instructions
        .iter()
        .filter(|instruction| {
            instruction.get("programId").and_then(Value::as_str)
                == Some(bpf_loader_upgradeable::ID.to_string().as_str())
                && instruction
                    .get("parsed")
                    .and_then(|value| value.get("type"))
                    .and_then(Value::as_str)
                    == Some("upgrade")
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(Error::new(format!(
            "Upgrade transaction contains {} Loader-v3 Upgrade instructions, expected one",
            matching.len()
        )));
    }
    let info = matching[0]
        .get("parsed")
        .and_then(|value| value.get("info"))
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("Loader-v3 Upgrade instruction omitted parsed info"))?;
    if info.get("programDataAccount").and_then(Value::as_str)
        != Some(query.programdata_id.to_string().as_str())
        || info.get("programAccount").and_then(Value::as_str)
            != Some(query.program_id.to_string().as_str())
        || info.get("authority").and_then(Value::as_str)
            != Some(query.authority.to_string().as_str())
        || info.get("rentSysvar").and_then(Value::as_str)
            != Some(sysvar::rent::ID.to_string().as_str())
        || info.get("clockSysvar").and_then(Value::as_str)
            != Some(sysvar::clock::ID.to_string().as_str())
    {
        return Err(Error::new(
            "Loader-v3 Upgrade instruction substituted Program, ProgramData, authority, or sysvars",
        ));
    }
    for label in ["bufferAccount", "spillAccount"] {
        let key = info
            .get(label)
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new(format!("Loader-v3 Upgrade omitted {label}")))?;
        let key = parse_pubkey(key, label)?;
        let _ = key_index(key, false, true)?;
    }

    let balances = |field: &str| -> Result<&Vec<Value>> {
        meta.get(field)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("Upgrade transaction meta omitted {field}")))
    };
    let pre = balances("preBalances")?;
    let post = balances("postBalances")?;
    if pre.len() != account_keys.len() || post.len() != account_keys.len() {
        return Err(Error::new(
            "Upgrade transaction balance vector width differs from account keys",
        ));
    }
    let balance = |values: &Vec<Value>, index: usize, label: &str| -> Result<u64> {
        values[index]
            .as_u64()
            .ok_or_else(|| Error::new(format!("Upgrade {label} balance is not u64")))
    };
    let payer_pre = balance(pre, payer_index, "pre payer")?;
    let payer_post = balance(post, payer_index, "post payer")?;
    let programdata_pre = balance(pre, programdata_index, "pre ProgramData")?;
    let programdata_post = balance(post, programdata_index, "post ProgramData")?;
    if programdata_pre != query.programdata_before_lamports
        || programdata_post != query.programdata_after_lamports
        || programdata_pre != programdata_post
    {
        return Err(Error::new(
            "Upgrade transaction did not preserve and bridge exact ProgramData rent",
        ));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .filter(|fee| *fee != 0)
        .ok_or_else(|| Error::new("Upgrade transaction omitted a nonzero fee"))?;
    let payer_delta = payer_pre
        .checked_sub(payer_post)
        .ok_or_else(|| Error::new("Upgrade transaction payer increased"))?;
    if payer_delta != fee {
        return Err(Error::new(
            "Upgrade transaction payer delta is not exactly its finalized fee",
        ));
    }
    let transaction_sha256 = digest(&serde_json::to_vec(&transaction)?);
    Ok(UpgradeTransactionEvidenceV1 {
        transaction,
        transaction_sha256,
        fee_lamports: fee,
        payer_pre_lamports: payer_pre,
        payer_post_lamports: payer_post,
        programdata_pre_lamports: programdata_pre,
        programdata_post_lamports: programdata_post,
    })
}

fn resolve_extension_transaction_via_rpc(
    query: &ExtensionTransactionQueryV1<'_>,
) -> Result<ExtensionTransactionEvidenceV1> {
    let mut rpc = Rpc::connect_cluster(query.origin, WritePolicyV1::ReadsOnly)?;
    let signature = find_extension_signature(query.deployment_slot, |before| {
        let mut options = serde_json::Map::new();
        options.insert("commitment".into(), Value::String("finalized".into()));
        options.insert(
            "limit".into(),
            Value::Number(SIGNATURE_HISTORY_PAGE_ROWS.into()),
        );
        if let Some(before) = before {
            options.insert("before".into(), Value::String(before.into()));
        }
        let rows = rpc.call(
            "getSignaturesForAddress",
            &Value::Array(vec![
                Value::String(query.programdata_id.to_string()),
                Value::Object(options),
            ]),
        )?;
        rows.as_array()
            .cloned()
            .ok_or_else(|| Error::new("getSignaturesForAddress did not return an array"))
    })?;
    let transaction = rpc.call(
        "getTransaction",
        &serde_json::json!([&signature, {
            "encoding": "jsonParsed",
            "commitment": "finalized",
            "maxSupportedTransactionVersion": 0
        }]),
    )?;
    validate_extension_transaction(query, &signature, transaction)
}

fn find_extension_signature(
    target_slot: u64,
    mut fetch_page: impl FnMut(Option<&str>) -> Result<Vec<Value>>,
) -> Result<String> {
    let mut before = None::<String>;
    let mut previous_slot = None::<u64>;
    let mut seen = BTreeSet::<String>::new();
    let mut candidates = Vec::<String>::new();
    let mut crossed_target = false;
    for _ in 0..SIGNATURE_HISTORY_MAX_PAGES {
        let rows = fetch_page(before.as_deref())?;
        if rows.len() > usize::try_from(SIGNATURE_HISTORY_PAGE_ROWS).expect("page bound") {
            return Err(Error::new(
                "ProgramData signature page exceeded the requested bounded width",
            ));
        }
        if rows.is_empty() {
            crossed_target = true;
            break;
        }
        for row in &rows {
            let slot = row
                .get("slot")
                .and_then(Value::as_u64)
                .ok_or_else(|| Error::new("extension signature row omitted u64 slot"))?;
            if previous_slot.is_some_and(|previous| slot > previous) {
                return Err(Error::new(
                    "ProgramData signature history is not monotonically newest-to-oldest",
                ));
            }
            previous_slot = Some(slot);
            let signature = row
                .get("signature")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::new("extension signature row omitted signature"))?;
            Signature::from_str(signature)
                .map_err(|_| Error::new("extension history returned an invalid signature"))?;
            if !seen.insert(signature.into()) {
                return Err(Error::new(
                    "ProgramData signature history repeated a cursor row",
                ));
            }
            let row_error = row
                .get("err")
                .ok_or_else(|| Error::new("extension signature row omitted err status"))?;
            if slot == target_slot && row_error.is_null() {
                candidates.push(signature.into());
            }
            if slot < target_slot {
                crossed_target = true;
            }
        }
        before = rows
            .last()
            .and_then(|row| row.get("signature"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if crossed_target {
            break;
        }
    }
    if !crossed_target {
        return Err(Error::new(format!(
            "ProgramData signature history did not cross target slot {target_slot} within the \
             provisional {SIGNATURE_HISTORY_MAX_PAGES}-page bound"
        )));
    }
    if candidates.len() != 1 {
        return Err(Error::new(format!(
            "expected one successful ProgramData signature at extension slot {target_slot}, \
             observed {}; signature attribution is ambiguous",
            candidates.len()
        )));
    }
    Ok(candidates.remove(0))
}

fn validate_extension_transaction(
    query: &ExtensionTransactionQueryV1<'_>,
    signature: &str,
    transaction: Value,
) -> Result<ExtensionTransactionEvidenceV1> {
    if transaction.get("slot").and_then(Value::as_u64) != Some(query.deployment_slot) {
        return Err(Error::new(
            "resolved extension transaction slot differs from ProgramData deployment slot",
        ));
    }
    let meta = transaction
        .get("meta")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("extension transaction omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(Error::new("resolved extension transaction failed"));
    }
    let message = transaction
        .get("transaction")
        .and_then(|value| value.get("message"))
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("extension transaction omitted parsed message"))?;
    let signatures = transaction
        .get("transaction")
        .and_then(|value| value.get("signatures"))
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("extension transaction omitted signatures"))?;
    if !signatures
        .iter()
        .any(|value| value.as_str() == Some(signature))
    {
        return Err(Error::new(
            "resolved extension signature is absent from its transaction",
        ));
    }
    let account_keys = message
        .get("accountKeys")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("extension transaction omitted account keys"))?;
    let key_index = |wanted: Pubkey, signer: bool| -> Result<usize> {
        let wanted = wanted.to_string();
        let matches = account_keys
            .iter()
            .enumerate()
            .filter(|(_, key)| {
                key.get("pubkey").and_then(Value::as_str) == Some(wanted.as_str())
                    && (!signer || key.get("signer").and_then(Value::as_bool) == Some(true))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::new(format!(
                "extension transaction did not contain one{} account key {wanted}",
                if signer { " signer" } else { "" }
            )));
        }
        Ok(matches[0])
    };
    let payer_index = key_index(query.payer, true)?;
    let programdata_index = key_index(query.programdata_id, false)?;
    let _authority_index = key_index(query.authority, true)?;

    let instructions = message
        .get("instructions")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("extension transaction omitted instructions"))?;
    let matching = instructions
        .iter()
        .filter(|instruction| {
            let parsed = instruction.get("parsed");
            instruction.get("programId").and_then(Value::as_str)
                == Some(bpf_loader_upgradeable::ID.to_string().as_str())
                && parsed
                    .and_then(|value| value.get("type"))
                    .and_then(Value::as_str)
                    == Some("extendProgramChecked")
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(Error::new(format!(
            "extension transaction contains {} checked Loader-v3 ExtendProgram instructions, \
             expected one",
            matching.len()
        )));
    }
    let info = matching[0]
        .get("parsed")
        .and_then(|value| value.get("info"))
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("checked extension instruction omitted info"))?;
    if info.get("additionalBytes").and_then(Value::as_u64) != Some(query.additional_bytes)
        || info.get("programDataAccount").and_then(Value::as_str)
            != Some(query.programdata_id.to_string().as_str())
        || info.get("programAccount").and_then(Value::as_str)
            != Some(query.program_id.to_string().as_str())
        || info.get("authority").and_then(Value::as_str)
            != Some(query.authority.to_string().as_str())
        || info.get("payerAccount").and_then(Value::as_str)
            != Some(query.payer.to_string().as_str())
    {
        return Err(Error::new(
            "resolved extension instruction substituted its width, Program, ProgramData, \
             authority, or payer",
        ));
    }
    let balances = |field: &str| -> Result<&Vec<Value>> {
        meta.get(field)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("extension transaction meta omitted {field}")))
    };
    let pre = balances("preBalances")?;
    let post = balances("postBalances")?;
    if pre.len() != account_keys.len() || post.len() != account_keys.len() {
        return Err(Error::new(
            "extension transaction balance vector width differs from account keys",
        ));
    }
    let pre_payer = pre[payer_index]
        .as_u64()
        .ok_or_else(|| Error::new("extension pre payer balance is not u64"))?;
    let post_payer = post[payer_index]
        .as_u64()
        .ok_or_else(|| Error::new("extension post payer balance is not u64"))?;
    let pre_programdata = pre[programdata_index]
        .as_u64()
        .ok_or_else(|| Error::new("extension pre ProgramData balance is not u64"))?;
    let post_programdata = post[programdata_index]
        .as_u64()
        .ok_or_else(|| Error::new("extension post ProgramData balance is not u64"))?;
    if pre_payer != query.wallet_before_lamports
        || post_payer != query.wallet_after_lamports
        || pre_programdata != query.programdata_before_lamports
        || post_programdata != query.programdata_after_lamports
    {
        return Err(Error::new(
            "extension transaction balances do not exactly bridge the observed pre/post wallet \
             and ProgramData states",
        ));
    }
    let payer_spend = pre_payer
        .checked_sub(post_payer)
        .ok_or_else(|| Error::new("extension transaction payer increased"))?;
    let programdata_delta = post_programdata
        .checked_sub(pre_programdata)
        .ok_or_else(|| Error::new("extension transaction ProgramData balance decreased"))?;
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("extension transaction omitted fee"))?;
    if payer_spend
        != programdata_delta
            .checked_add(fee)
            .ok_or_else(|| Error::new("extension transaction fee arithmetic overflow"))?
    {
        return Err(Error::new(
            "extension transaction payer spend is not exact ProgramData rent delta plus fee",
        ));
    }
    Ok(ExtensionTransactionEvidenceV1 {
        signature: signature.into(),
        transaction,
        fee_lamports: fee,
        payer_spend_lamports: payer_spend,
        programdata_delta_lamports: programdata_delta,
    })
}

fn redact(text: &str, args: &UpgradeArgsV1) -> String {
    text.replace(args.origin.url(), &args.origin.redacted_url())
        .replace(
            args.authority_keypair.to_string_lossy().as_ref(),
            "<upgrade-authority-keypair>",
        )
        .replace(
            args.fee_payer_keypair.to_string_lossy().as_ref(),
            "<fee-payer-keypair>",
        )
}

fn validate_receipt_binding(
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    baseline: &UpgradeBaselineV1,
    live_elf_sha256: &str,
    live_elf_padding_bytes: u64,
    receipt: &UpgradeReceiptV1,
    require_exact_rpc_origin: bool,
) -> Result<()> {
    require_digest(&receipt.receipt_sha256, "Upgrade receipt SHA-256")?;
    let mut canonical_receipt = receipt.clone();
    canonical_receipt.receipt_sha256.clear();
    if receipt.receipt_sha256 != digest(&serde_json::to_vec(&canonical_receipt)?) {
        return Err(Error::new(
            "Upgrade receipt SHA-256 does not bind its canonical phase fields",
        ));
    }
    if receipt.schema != SCHEMA
        || receipt.role != args.role
        || receipt.program_id != args.program_id.to_string()
        || receipt.programdata_id != args.programdata_id.to_string()
        || receipt.retained_upgrade_authority != args.expected_upgrade_authority.to_string()
        || receipt.fee_payer != args.fee_payer.to_string()
        || (require_exact_rpc_origin && receipt.rpc_origin_redacted != args.origin.redacted_url())
        || receipt.genesis_hash != DEVNET_GENESIS_HASH
        || receipt.source_revision != gate.source_revision
        || receipt.source_tree_sha256 != gate.source_tree_sha256
        || receipt.checked_release_gate_sha256 != gate.gate_sha256
        || receipt.baseline_sha256 != baseline.baseline_sha256
        || receipt.baseline_context_slot != baseline.context_slot
        || receipt.raw_elf_sha256 != gate.raw_elf_sha256
        || receipt.live_elf_sha256 != live_elf_sha256
        || receipt.live_elf_padding_bytes != live_elf_padding_bytes
        || receipt.solana_cli_version != gate.solana_cli_version
        || receipt.exclusive_payer_window_acknowledgment
            != format!("{}:{}:{}", args.role, args.program_id, args.fee_payer)
        || receipt.before_context_slot < baseline.context_slot
        || receipt.before != baseline.observation
    {
        return Err(Error::new(
            "existing Upgrade receipt does not bind this exact role, deployment, authority, \
             payer, cluster, source, evidence, or payload; substitution and ambiguous replay are \
             refused",
        ));
    }
    let expected_operation = operation_id_from_receipt(receipt);
    if receipt.operation_id != expected_operation {
        return Err(Error::new("Upgrade receipt operation_id is not canonical"));
    }
    match receipt.phase {
        ReceiptPhaseV1::Prepared => {
            if receipt.transaction_signature.is_some()
                || receipt.solana_cli_output.is_some()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
                || receipt.dump_sha256.is_some()
                || receipt.dump_shape.is_some()
            {
                return Err(Error::new(
                    "prepared Upgrade receipt carries later-phase fields",
                ));
            }
        }
        ReceiptPhaseV1::Submitted => {
            if receipt.transaction_signature.is_none()
                || receipt.solana_cli_output.is_none()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
                || receipt.dump_sha256.is_some()
                || receipt.dump_shape.is_some()
            {
                return Err(Error::new("submitted Upgrade receipt phase is incomplete"));
            }
            validate_recorded_deploy_output(receipt)?;
        }
        ReceiptPhaseV1::Complete => {
            if receipt.transaction_signature.is_none()
                || receipt.solana_cli_output.is_none()
                || receipt.finalized_transaction.is_none()
                || receipt.finalized_transaction_sha256.is_none()
                || receipt.after_context_slot.is_none()
                || receipt.after.is_none()
                || receipt.arithmetic.is_none()
                || receipt.dump_sha256.is_none()
                || receipt.dump_shape.is_none()
            {
                return Err(Error::new("complete Upgrade receipt phase is incomplete"));
            }
            validate_recorded_deploy_output(receipt)?;
            let transaction = receipt
                .finalized_transaction
                .as_ref()
                .expect("checked complete transaction");
            if receipt.finalized_transaction_sha256.as_deref()
                != Some(digest(&serde_json::to_vec(transaction)?).as_str())
                || receipt
                    .after_context_slot
                    .is_some_and(|slot| slot < receipt.before_context_slot)
            {
                return Err(Error::new(
                    "complete Upgrade receipt transaction hash or context-slot order is invalid",
                ));
            }
            validate_complete_receipt_shape(receipt)?;
        }
    }
    Ok(())
}

fn validate_complete_receipt_shape(receipt: &UpgradeReceiptV1) -> Result<()> {
    let after = receipt.after.as_ref().expect("checked complete poststate");
    let arithmetic = receipt
        .arithmetic
        .as_ref()
        .expect("checked complete arithmetic");
    let payer_delta = arithmetic
        .transaction_payer_pre_lamports
        .checked_sub(arithmetic.transaction_payer_post_lamports)
        .ok_or_else(|| Error::new("complete Upgrade receipt payer increased"))?;
    let operation_spend = arithmetic
        .operation_wallet_before_lamports
        .checked_sub(arithmetic.operation_wallet_after_lamports)
        .ok_or_else(|| Error::new("complete Upgrade operation wallet increased"))?;
    let unattributed = operation_spend
        .checked_sub(arithmetic.transaction_fee_lamports)
        .ok_or_else(|| {
            Error::new("complete Upgrade operation spend is smaller than final transaction fee")
        })?;
    if after.deployment_slot <= receipt.before.deployment_slot
        || after.upgrade_authority != receipt.retained_upgrade_authority
        || after.live_elf_sha256 != receipt.live_elf_sha256
        || after.programdata_lamports != receipt.before.programdata_lamports
        || arithmetic.transaction_fee_lamports == 0
        || arithmetic.payer_fee_delta_lamports != payer_delta
        || payer_delta != arithmetic.transaction_fee_lamports
        || arithmetic.programdata_before_lamports != receipt.before.programdata_lamports
        || arithmetic.programdata_after_lamports != after.programdata_lamports
        || arithmetic.programdata_delta_lamports != 0
        || arithmetic.operation_wallet_before_lamports != receipt.wallet_before_lamports
        || arithmetic.operation_observed_net_spend_lamports != operation_spend
        || arithmetic.unattributed_cli_net_cost_lamports != unattributed
        || arithmetic.accounting_scope != OPERATION_ACCOUNTING_SCOPE_V1
        || arithmetic.cost_attribution != OPERATION_COST_ATTRIBUTION_V1
        || receipt.exclusive_payer_window_acknowledgment
            != format!(
                "{}:{}:{}",
                receipt.role, receipt.program_id, receipt.fee_payer
            )
        || receipt
            .dump_sha256
            .as_deref()
            .is_none_or(|digest| require_digest(digest, "complete dump SHA-256").is_err())
        || !matches!(
            receipt.dump_shape.as_deref(),
            Some("raw-elf" | "live-elf-with-zero-padding")
        )
    {
        return Err(Error::new(
            "complete Upgrade receipt poststate, transaction arithmetic, or dump shape is not canonical",
        ));
    }
    Ok(())
}

fn validate_recorded_deploy_output(receipt: &UpgradeReceiptV1) -> Result<()> {
    let signature = receipt
        .transaction_signature
        .as_deref()
        .ok_or_else(|| Error::new("Upgrade receipt omitted signature"))?;
    Signature::from_str(signature)
        .map_err(|_| Error::new("Upgrade receipt signature is not a Solana signature"))?;
    let output = receipt
        .solana_cli_output
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("Upgrade receipt omitted CLI JSON object"))?;
    if output.get("programId").and_then(Value::as_str) != Some(receipt.program_id.as_str())
        || output.get("signature").and_then(Value::as_str) != Some(signature)
    {
        return Err(Error::new(
            "Upgrade receipt CLI output does not bind its exact Program and signature",
        ));
    }
    Ok(())
}

fn operation_id(args: &UpgradeArgsV1, gate_sha256: &str, before: &SnapshotV1) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/devnet-permanent-id-upgrade/operation/v4\0");
    hasher.update(args.role.as_bytes());
    hasher.update([0]);
    hasher.update(args.program_id.as_ref());
    hasher.update(args.programdata_id.as_ref());
    hasher.update(args.expected_upgrade_authority.as_ref());
    hasher.update(args.fee_payer.as_ref());
    hasher.update(gate_sha256.as_bytes());
    hasher.update(before.context_slot.to_le_bytes());
    hasher.update(before.loader.deployment_slot.to_le_bytes());
    hasher.update(before.loader.programdata_account_sha256.as_bytes());
    hasher.update(before.wallet_lamports.to_le_bytes());
    hash_text(
        &mut hasher,
        args.exclusive_payer_window_acknowledgment
            .as_deref()
            .expect("execute validated exclusive payer acknowledgment"),
    )
    .expect("bounded acknowledgment hashes");
    hex(&hasher.finalize())
}

fn operation_id_from_receipt(receipt: &UpgradeReceiptV1) -> String {
    let role = receipt.role.as_bytes();
    let program = Pubkey::from_str(&receipt.program_id).expect("validated receipt program");
    let programdata =
        Pubkey::from_str(&receipt.programdata_id).expect("validated receipt programdata");
    let authority =
        Pubkey::from_str(&receipt.retained_upgrade_authority).expect("validated receipt authority");
    let payer = Pubkey::from_str(&receipt.fee_payer).expect("validated receipt payer");
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/devnet-permanent-id-upgrade/operation/v4\0");
    hasher.update(role);
    hasher.update([0]);
    hasher.update(program.as_ref());
    hasher.update(programdata.as_ref());
    hasher.update(authority.as_ref());
    hasher.update(payer.as_ref());
    hasher.update(receipt.checked_release_gate_sha256.as_bytes());
    hasher.update(receipt.before_context_slot.to_le_bytes());
    hasher.update(receipt.before.deployment_slot.to_le_bytes());
    hasher.update(receipt.before.programdata_account_sha256.as_bytes());
    hasher.update(receipt.wallet_before_lamports.to_le_bytes());
    hash_text(&mut hasher, &receipt.exclusive_payer_window_acknowledgment)
        .expect("bounded acknowledgment hashes");
    hex(&hasher.finalize())
}

fn extension_operation_id(
    args: &ExtensionArgsV1,
    baseline: &UpgradeBaselineV1,
    before: &SnapshotV1,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/devnet-programdata-extension/operation/v1\0");
    hasher.update(args.role.as_bytes());
    hasher.update([0]);
    hasher.update(args.program_id.as_ref());
    hasher.update(args.programdata_id.as_ref());
    hasher.update(args.expected_upgrade_authority.as_ref());
    hasher.update(args.fee_payer.as_ref());
    hasher.update(baseline.baseline_sha256.as_bytes());
    hasher.update(baseline.extension_additional_bytes.to_le_bytes());
    hasher.update(before.context_slot.to_le_bytes());
    hasher.update(before.loader.programdata_account_sha256.as_bytes());
    hasher.update(before.wallet_lamports.to_le_bytes());
    hex(&hasher.finalize())
}

fn extension_operation_id_from_receipt(receipt: &ExtensionReceiptV1) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/devnet-programdata-extension/operation/v1\0");
    hasher.update(receipt.role.as_bytes());
    hasher.update([0]);
    hasher.update(parse_pubkey(&receipt.program_id, "extension receipt Program")?.as_ref());
    hasher.update(parse_pubkey(&receipt.programdata_id, "extension receipt ProgramData")?.as_ref());
    hasher.update(
        parse_pubkey(
            &receipt.retained_upgrade_authority,
            "extension receipt authority",
        )?
        .as_ref(),
    );
    hasher.update(parse_pubkey(&receipt.fee_payer, "extension receipt payer")?.as_ref());
    hasher.update(receipt.baseline_sha256.as_bytes());
    hasher.update(receipt.extension_additional_bytes.to_le_bytes());
    hasher.update(receipt.before_context_slot.to_le_bytes());
    hasher.update(receipt.before.programdata_account_sha256.as_bytes());
    hasher.update(receipt.wallet_before_lamports.to_le_bytes());
    Ok(hex(&hasher.finalize()))
}

fn validate_extension_receipt_binding(
    args: &ExtensionArgsV1,
    baseline: &UpgradeBaselineV1,
    receipt: &ExtensionReceiptV1,
) -> Result<()> {
    require_digest(&receipt.receipt_sha256, "extension receipt SHA-256")?;
    let mut canonical_receipt = receipt.clone();
    canonical_receipt.receipt_sha256.clear();
    if receipt.receipt_sha256 != digest(&serde_json::to_vec(&canonical_receipt)?) {
        return Err(Error::new(
            "extension receipt SHA-256 does not bind its canonical phase fields",
        ));
    }
    if receipt.schema != EXTENSION_SCHEMA
        || receipt.role != args.role
        || receipt.program_id != args.program_id.to_string()
        || receipt.programdata_id != args.programdata_id.to_string()
        || receipt.retained_upgrade_authority != args.expected_upgrade_authority.to_string()
        || receipt.fee_payer != args.fee_payer.to_string()
        || receipt.rpc_origin_redacted != args.origin.redacted_url()
        || receipt.genesis_hash != DEVNET_GENESIS_HASH
        || receipt.baseline_sha256 != baseline.baseline_sha256
        || receipt.baseline_context_slot != baseline.context_slot
        || receipt.solana_cli_version != args.expected_solana_cli_version
        || receipt.extension_additional_bytes != baseline.extension_additional_bytes
        || receipt.target_rent_exempt_minimum_lamports
            != baseline.target_rent_exempt_minimum_lamports
        || receipt.expected_rent_top_up_lamports != baseline.extension_lamport_top_up
        || receipt.before != baseline.observation
        || receipt.before_context_slot < baseline.context_slot
        || receipt.operation_id != extension_operation_id_from_receipt(receipt)?
    {
        return Err(Error::new(
            "extension receipt does not bind this exact role, Program, ProgramData, authority, \
             payer, devnet baseline, rent quote, or prestate",
        ));
    }
    match receipt.phase {
        ReceiptPhaseV1::Prepared => {
            if receipt.transaction_signature.is_some()
                || receipt.solana_cli_output.is_some()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
            {
                return Err(Error::new(
                    "prepared extension receipt carries later-phase fields",
                ));
            }
        }
        ReceiptPhaseV1::Submitted => {
            if receipt.transaction_signature.is_some()
                || receipt.solana_cli_output.is_none()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
            {
                return Err(Error::new("submitted extension receipt is incomplete"));
            }
        }
        ReceiptPhaseV1::Complete => {
            if receipt.transaction_signature.is_none()
                || receipt.solana_cli_output.is_none()
                || receipt.finalized_transaction.is_none()
                || receipt.finalized_transaction_sha256.is_none()
                || receipt.after_context_slot.is_none()
                || receipt.after.is_none()
                || receipt.arithmetic.is_none()
            {
                return Err(Error::new("complete extension receipt is incomplete"));
            }
            let transaction = receipt
                .finalized_transaction
                .as_ref()
                .expect("checked complete extension transaction");
            if receipt.finalized_transaction_sha256.as_deref()
                != Some(digest(&serde_json::to_vec(transaction)?).as_str())
                || receipt
                    .after_context_slot
                    .is_some_and(|slot| slot < receipt.before_context_slot)
            {
                return Err(Error::new(
                    "complete extension receipt transaction hash or context-slot order is invalid",
                ));
            }
        }
    }
    Ok(())
}

fn load_receipt(path: &Path) -> Result<Option<UpgradeReceiptV1>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|error| {
            Error::new(format!(
                "existing Upgrade receipt {} is invalid: {error}",
                path.display()
            ))
        })?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn load_extension_receipt(path: &Path) -> Result<Option<ExtensionReceiptV1>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|error| {
            Error::new(format!(
                "existing extension receipt {} is invalid: {error}",
                path.display()
            ))
        })?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn write_receipt(path: &Path, receipt: &mut UpgradeReceiptV1) -> Result<()> {
    receipt.receipt_sha256.clear();
    receipt.receipt_sha256 = digest(&serde_json::to_vec(receipt)?);
    write_json_atomic_replace(path, receipt)
}

fn write_extension_receipt(path: &Path, receipt: &mut ExtensionReceiptV1) -> Result<()> {
    receipt.receipt_sha256.clear();
    receipt.receipt_sha256 = digest(&serde_json::to_vec(receipt)?);
    write_json_atomic_replace(path, receipt)
}

fn write_json_atomic_new<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if path.exists() {
        return Err(Error::new(format!(
            "output {} already exists",
            path.display()
        )));
    }
    write_json_atomic_replace(path, value)
}

fn write_json_atomic_replace<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("receipt path omitted a parent"))?;
    if !parent.is_dir() {
        return Err(Error::new(format!(
            "receipt parent {} does not exist",
            parent.display()
        )));
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new("receipt file name is not UTF-8"))?;
    let temporary = parent.join(format!(".{file_name}.{}.pending", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| {
            Error::new(format!(
                "could not create atomic receipt temporary {}: {error}",
                temporary.display()
            ))
        })?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.write_all(b"\n")) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    if let Err(error) = file.sync_all() {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

fn require_role(role: &str) -> Result<()> {
    if !ROLES.contains(&role) {
        return Err(Error::new(format!(
            "unknown Upgrade role {role:?}; one run names exactly one of {}",
            ROLES.join(", ")
        )));
    }
    Ok(())
}

fn role_ordinal(role: &str) -> Result<u8> {
    ROLES
        .iter()
        .position(|known| *known == role)
        .and_then(|index| u8::try_from(index).ok())
        .ok_or_else(|| Error::new(format!("unknown Upgrade role {role:?}")))
}

fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    Pubkey::from_str(value).map_err(|_| Error::new(format!("{label} is not a Solana pubkey")))
}

fn absolute(value: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

fn path_argument(path: &Path, label: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("{label} path is not UTF-8")))
}

fn require_digest(value: &str, label: &str) -> Result<()> {
    require_lower_hex(value, label, 64, 64)
}

fn require_lower_hex(value: &str, label: &str, minimum: usize, maximum: usize) -> Result<()> {
    if !(minimum..=maximum).contains(&value.len())
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(Error::new(format!(
            "{label} must be {minimum}..={maximum} lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::atomic::{AtomicU64, Ordering},
    };

    use serde_json::json;

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "dclutch-devnet-upgrade-v1-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct FakeRunner {
        program: Pubkey,
        programdata: Pubkey,
        linked_programdata: Pubkey,
        authority: Pubkey,
        payer: Pubkey,
        version: String,
        genesis: String,
        before_live: Vec<u8>,
        after_live: Vec<u8>,
        before_slot: u64,
        after_slot: u64,
        before_context_slot: u64,
        after_context_slot: u64,
        before_wallet: u64,
        after_wallet: u64,
        before_programdata_lamports: u64,
        after_programdata_lamports: u64,
        deployed: bool,
        deploy_success: bool,
        deploy_program: Pubkey,
        dump: Vec<u8>,
        upgrade_fee_lamports: u64,
        upgrade_transaction_override: Option<Value>,
        extension_fee_lamports: u64,
        extension_instruction_bytes: Option<u64>,
        calls: Vec<Vec<String>>,
        snapshot_minimum_slots: Vec<u64>,
        forced: VecDeque<CliOutput>,
    }

    impl FakeRunner {
        fn new(fixture: &Fixture) -> Self {
            Self {
                program: fixture.program,
                programdata: fixture.programdata,
                linked_programdata: fixture.programdata,
                authority: fixture.authority,
                payer: fixture.payer,
                version: fixture.solana_cli_version.clone(),
                genesis: DEVNET_GENESIS_HASH.into(),
                before_live: fixture.before_live.clone(),
                after_live: fixture.candidate_live.clone(),
                before_slot: 91,
                after_slot: 92,
                before_context_slot: 489_212_834,
                after_context_slot: 489_212_835,
                before_wallet: 1_000_000,
                after_wallet: 985_000,
                before_programdata_lamports: 77_000,
                after_programdata_lamports: 77_000,
                deployed: false,
                deploy_success: true,
                deploy_program: fixture.program,
                dump: fixture.raw_elf.clone(),
                upgrade_fee_lamports: 15_000,
                upgrade_transaction_override: None,
                extension_fee_lamports: 5_000,
                extension_instruction_bytes: None,
                calls: Vec::new(),
                snapshot_minimum_slots: Vec::new(),
                forced: VecDeque::new(),
            }
        }

        fn account(&self, address: Pubkey) -> RpcAccountV1 {
            let (lamports, owner, executable, data) = if address == self.program {
                let mut program = vec![0_u8; 36];
                program[..4].copy_from_slice(&2_u32.to_le_bytes());
                program[4..36].copy_from_slice(self.linked_programdata.as_ref());
                (1_140, bpf_loader_upgradeable::ID, true, program)
            } else if address == self.programdata {
                let (slot, live, lamports) = if self.deployed {
                    (
                        self.after_slot,
                        &self.after_live,
                        self.after_programdata_lamports,
                    )
                } else {
                    (
                        self.before_slot,
                        &self.before_live,
                        self.before_programdata_lamports,
                    )
                };
                let mut programdata = vec![0_u8; 45];
                programdata[..4].copy_from_slice(&3_u32.to_le_bytes());
                programdata[4..12].copy_from_slice(&slot.to_le_bytes());
                programdata[12] = 1;
                programdata[13..45].copy_from_slice(self.authority.as_ref());
                programdata.extend_from_slice(live);
                (lamports, bpf_loader_upgradeable::ID, false, programdata)
            } else if address == self.payer {
                (
                    if self.deployed {
                        self.after_wallet
                    } else {
                        self.before_wallet
                    },
                    Pubkey::default(),
                    false,
                    Vec::new(),
                )
            } else {
                panic!("unexpected fake account {address}");
            };
            RpcAccountV1 {
                lamports,
                owner,
                executable,
                rent_epoch: u64::MAX,
                data,
            }
        }
    }

    impl CliRunner for FakeRunner {
        fn run(&mut self, arguments: &[String]) -> Result<CliOutput> {
            self.calls.push(arguments.to_vec());
            if let Some(output) = self.forced.pop_front() {
                return Ok(output);
            }
            let success = |stdout: String| CliOutput {
                success: true,
                stdout,
                stderr: String::new(),
            };
            match arguments.first().map(String::as_str) {
                Some("--version") => Ok(success(format!("{}\n", self.version))),
                Some("genesis-hash") => Ok(success(format!("{}\n", self.genesis))),
                Some("address") => {
                    let path = arguments.get(2).expect("keypair path");
                    let address = if path.contains("authority") {
                        self.authority
                    } else {
                        self.payer
                    };
                    Ok(success(format!("{address}\n")))
                }
                Some("program") if arguments.get(1).map(String::as_str) == Some("deploy") => {
                    if !self.deploy_success {
                        return Ok(CliOutput {
                            success: false,
                            stdout: String::new(),
                            stderr: "synthetic deploy interruption".into(),
                        });
                    }
                    self.deployed = true;
                    Ok(success(
                        json!({
                            "programId": self.deploy_program.to_string(),
                            "signature": Signature::from([7_u8; 64]).to_string()
                        })
                        .to_string(),
                    ))
                }
                Some("program") if arguments.get(1).map(String::as_str) == Some("extend") => {
                    if !self.deploy_success {
                        return Ok(CliOutput {
                            success: false,
                            stdout: String::new(),
                            stderr: "synthetic extension interruption".into(),
                        });
                    }
                    self.deployed = true;
                    let additional_bytes = arguments
                        .get(3)
                        .expect("extension width")
                        .parse::<u64>()
                        .expect("u64 extension width");
                    Ok(success(
                        json!({
                            "programId": self.deploy_program.to_string(),
                            "additionalBytes": additional_bytes
                        })
                        .to_string(),
                    ))
                }
                Some("program") if arguments.get(1).map(String::as_str) == Some("dump") => {
                    let path = PathBuf::from(arguments.get(3).expect("dump path"));
                    fs::write(path, &self.dump).expect("write synthetic dump");
                    Ok(success(String::new()))
                }
                command => panic!("unexpected fake Solana CLI command {command:?}: {arguments:?}"),
            }
        }

        fn read_snapshot(&mut self, query: &SnapshotQueryV1<'_>) -> Result<SnapshotV1> {
            assert_eq!(query.program_id, self.program);
            assert_eq!(query.programdata_id, self.programdata);
            assert_eq!(query.expected_upgrade_authority, self.authority);
            assert_eq!(query.payer, self.payer);
            self.snapshot_minimum_slots.push(query.minimum_context_slot);
            let context_slot = if self.deployed {
                self.after_context_slot
            } else {
                self.before_context_slot
            };
            if context_slot < query.minimum_context_slot {
                return Err(Error::new("synthetic snapshot is below minContextSlot"));
            }
            let program = self.account(self.program);
            let programdata = self.account(self.programdata);
            let wallet = self.account(self.payer);
            let loader = loader_observation(
                self.program,
                self.programdata,
                self.authority,
                &program,
                &programdata,
            )?;
            let view = ProgramDataV3View::parse(&programdata.data)
                .map_err(|_| Error::new("synthetic ProgramData parse"))?;
            Ok(SnapshotV1 {
                context_slot,
                loader,
                wallet_lamports: wallet.lamports,
                live_elf: view.elf().to_vec(),
            })
        }

        fn resolve_upgrade_transaction(
            &mut self,
            query: &UpgradeTransactionQueryV1<'_>,
        ) -> Result<UpgradeTransactionEvidenceV1> {
            let buffer = Pubkey::new_from_array([11; 32]);
            let spill = Pubkey::new_from_array([12; 32]);
            let transaction = self.upgrade_transaction_override.clone().unwrap_or_else(|| {
                json!({
                    "slot": query.deployment_slot,
                    "transaction": {
                        "signatures": [query.signature],
                        "message": {
                            "accountKeys": [
                                {"pubkey": query.payer.to_string(), "signer": true, "writable": true},
                                {"pubkey": query.programdata_id.to_string(), "signer": false, "writable": true},
                                {"pubkey": query.program_id.to_string(), "signer": false, "writable": true},
                                {"pubkey": query.authority.to_string(), "signer": true, "writable": false},
                                {"pubkey": buffer.to_string(), "signer": false, "writable": true},
                                {"pubkey": spill.to_string(), "signer": false, "writable": true},
                                {"pubkey": sysvar::rent::ID.to_string(), "signer": false, "writable": false},
                                {"pubkey": sysvar::clock::ID.to_string(), "signer": false, "writable": false}
                            ],
                            "instructions": [{
                                "program": "bpf-upgradeable-loader",
                                "programId": bpf_loader_upgradeable::ID.to_string(),
                                "parsed": {
                                    "type": "upgrade",
                                    "info": {
                                        "programDataAccount": query.programdata_id.to_string(),
                                        "programAccount": query.program_id.to_string(),
                                        "bufferAccount": buffer.to_string(),
                                        "spillAccount": spill.to_string(),
                                        "rentSysvar": sysvar::rent::ID.to_string(),
                                        "clockSysvar": sysvar::clock::ID.to_string(),
                                        "authority": query.authority.to_string()
                                    }
                                }
                            }]
                        }
                    },
                    "meta": {
                        "err": null,
                        "fee": self.upgrade_fee_lamports,
                        "preBalances": [
                            self.before_wallet,
                            query.programdata_before_lamports,
                            1_140,
                            0,
                            10_000,
                            0,
                            1,
                            1
                        ],
                        "postBalances": [
                            self.before_wallet - self.upgrade_fee_lamports,
                            query.programdata_after_lamports,
                            1_140,
                            0,
                            0,
                            10_000,
                            1,
                            1
                        ]
                    }
                })
            });
            validate_upgrade_transaction(query, transaction)
        }

        fn resolve_extension_transaction(
            &mut self,
            query: &ExtensionTransactionQueryV1<'_>,
        ) -> Result<ExtensionTransactionEvidenceV1> {
            let signature = Signature::from([8_u8; 64]).to_string();
            let additional = self
                .extension_instruction_bytes
                .unwrap_or(query.additional_bytes);
            let transaction = json!({
                "slot": query.deployment_slot,
                "transaction": {
                    "signatures": [signature],
                    "message": {
                        "accountKeys": [
                            {"pubkey": query.payer.to_string(), "signer": true, "writable": true},
                            {"pubkey": query.programdata_id.to_string(), "signer": false, "writable": true},
                            {"pubkey": query.program_id.to_string(), "signer": false, "writable": true},
                            {"pubkey": query.authority.to_string(), "signer": true, "writable": false}
                        ],
                        "instructions": [{
                            "program": "bpf-upgradeable-loader",
                            "programId": bpf_loader_upgradeable::ID.to_string(),
                            "parsed": {
                                "type": "extendProgramChecked",
                                "info": {
                                    "additionalBytes": additional,
                                    "programDataAccount": query.programdata_id.to_string(),
                                    "programAccount": query.program_id.to_string(),
                                    "authority": query.authority.to_string(),
                                    "payerAccount": query.payer.to_string()
                                }
                            }
                        }]
                    }
                },
                "meta": {
                    "err": null,
                    "fee": self.extension_fee_lamports,
                    "preBalances": [
                        query.wallet_before_lamports,
                        query.programdata_before_lamports,
                        1_140,
                        0
                    ],
                    "postBalances": [
                        query.wallet_after_lamports,
                        query.programdata_after_lamports,
                        1_140,
                        0
                    ]
                }
            });
            validate_extension_transaction(query, &signature, transaction)
        }
    }

    struct Fixture {
        _directory: TestDirectory,
        args: UpgradeArgsV1,
        gate: CheckedUpgradeGateV1,
        solana_cli_version: String,
        program: Pubkey,
        programdata: Pubkey,
        authority: Pubkey,
        payer: Pubkey,
        raw_elf: Vec<u8>,
        candidate_live: Vec<u8>,
        before_live: Vec<u8>,
    }

    fn gate_file(root: &Path, relative: &str) -> GateFileV1 {
        let bytes = fs::read(root.join(relative)).expect("gate evidence file");
        GateFileV1 {
            canonical_path: relative.into(),
            bytes: u64::try_from(bytes.len()).expect("gate evidence width"),
            sha256: digest(&bytes),
        }
    }

    fn write_gate_evidence(root: &Path, relative: &str, bytes: &[u8]) -> GateFileV1 {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("gate evidence parent"))
            .expect("create gate evidence parent");
        fs::write(&path, bytes).expect("write gate evidence");
        gate_file(root, relative)
    }

    fn checked_gate(root: &Path, raw_elf: &[u8], solana_cli_version: &str) -> CheckedUpgradeGateV1 {
        let run_id = "ab".repeat(32);
        let source_tree =
            write_gate_evidence(root, "source-tree.txt", b"100644 blob fake\tCargo.toml\n");
        let build_links_text = SHIPPED_LINKS
            .iter()
            .map(|(label, package, _)| format!("{label}\t{package}\n"))
            .collect::<String>();
        let build_links = write_gate_evidence(root, "build-links.tsv", build_links_text.as_bytes());
        let build_run_text = format!("dclutch-sbf-build-run-v1={run_id}\n");
        let build_run = write_gate_evidence(root, "build-run.txt", build_run_text.as_bytes());
        let diagnostics_text = SHIPPED_LINKS
            .iter()
            .map(|(label, _, _)| format!("{label}=0\n"))
            .collect::<String>();
        let diagnostics =
            write_gate_evidence(root, "build-diagnostics.txt", diagnostics_text.as_bytes());
        let links = SHIPPED_LINKS
            .iter()
            .map(|(label, package, produces_artifact)| {
                let compile_marker =
                    format!("   Compiling {package} v0.1.0 (/checked/programs/{package})");
                let build_log_text = format!(
                    "dclutch-sbf-build-run-v1={run_id}\n{compile_marker}\n    Finished release\n"
                );
                let build_log = write_gate_evidence(
                    root,
                    &format!("build-{label}.log"),
                    build_log_text.as_bytes(),
                );
                let frame_compile_marker = compile_marker.clone();
                let frame_build_log_text = format!(
                    "dclutch-sbf-frame-run-v1={run_id}\n{frame_compile_marker}\n    Finished release\n"
                );
                let frame_build_log = write_gate_evidence(
                    root,
                    &format!("frame-build-{label}.log"),
                    frame_build_log_text.as_bytes(),
                );
                let frame_report_text = format!(
                    "dclutch-sbf-frame-report-v1\nlabel={label}\npackage={package}\n\
                     frame_count=3\nframe_bound_bytes=4096\nframes_at_or_over_bound=0\n\
                     deepest_frame_bytes=2048\nobject_sha256={}\nmeasurement_output:\n  3 \
                     measured frames, bound 4096; deepest:\n      2048    2048 spare  fake\n",
                    "cd".repeat(32)
                );
                let frame_report = write_gate_evidence(
                    root,
                    &format!("frame/{label}.txt"),
                    frame_report_text.as_bytes(),
                );
                let (elf, checked_manifest) = if *produces_artifact {
                    let bytes = if *label == "trading" {
                        raw_elf.to_vec()
                    } else {
                        format!("\x7fELF{label}").into_bytes()
                    };
                    (
                        Some(write_gate_evidence(
                            root,
                            &format!("elf/{label}.so"),
                            &bytes,
                        )),
                        Some(write_gate_evidence(
                            root,
                            &format!("evidence/{label}/checked.bin"),
                            format!("checked-{label}").as_bytes(),
                        )),
                    )
                } else {
                    (None, None)
                };
                CheckedGateLinkV1 {
                    label: (*label).into(),
                    package: (*package).into(),
                    build_log,
                    compile_marker,
                    sbf_diagnostics_count: 0,
                    frame_build_log,
                    frame_compile_marker,
                    frame_report,
                    frame_count: 3,
                    frame_bound_bytes: 4096,
                    frames_at_or_over_bound: 0,
                    deepest_frame_bytes: 2048,
                    elf,
                    checked_manifest,
                }
            })
            .collect::<Vec<_>>();
        CheckedUpgradeGateV1 {
            schema: CHECKED_GATE_SCHEMA.into(),
            source_revision: "0123456789abcdef0123456789abcdef01234567".into(),
            source_tree_sha256: source_tree.sha256.clone(),
            solana_cli_version: solana_cli_version.into(),
            build_run_id: run_id,
            link_count: u64::try_from(SHIPPED_LINKS.len()).expect("link count"),
            source_tree_manifest: source_tree,
            build_links_manifest: build_links,
            build_run_manifest: build_run,
            diagnostics_manifest: diagnostics,
            links,
        }
    }

    impl Fixture {
        fn new() -> Self {
            let directory = TestDirectory::new();
            let program = Pubkey::new_from_array([1; 32]);
            let programdata = Pubkey::new_from_array([2; 32]);
            let authority = Pubkey::new_from_array([3; 32]);
            let payer = Pubkey::new_from_array([4; 32]);
            let raw_elf = b"\x7fELFnew!".to_vec();
            let mut candidate_live = raw_elf.clone();
            candidate_live.extend_from_slice(&[0; 4]);
            let before_live = b"\x7fELFold!\0\0\0\0".to_vec();
            let solana_cli_version = "solana-cli 4.0.2 (test fixture)".to_owned();
            let gate = checked_gate(&directory.0, &raw_elf, &solana_cli_version);
            let elf_path = directory.0.join("elf/trading.so");
            let gate_path = directory.0.join("CHECKED_UPGRADE_GATE.json");
            fs::write(
                &gate_path,
                serde_json::to_vec_pretty(&gate).expect("gate JSON"),
            )
            .expect("write gate");
            let gate_sha256 = digest(&fs::read(&gate_path).expect("gate bytes"));
            let mut program_bytes = vec![0_u8; 36];
            program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
            program_bytes[4..36].copy_from_slice(programdata.as_ref());
            let mut programdata_bytes = vec![0_u8; 45];
            programdata_bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
            programdata_bytes[4..12].copy_from_slice(&91_u64.to_le_bytes());
            programdata_bytes[12] = 1;
            programdata_bytes[13..45].copy_from_slice(authority.as_ref());
            programdata_bytes.extend_from_slice(&before_live);
            let observation = LoaderObservationV1 {
                program_lamports: 1_140,
                program_owner: bpf_loader_upgradeable::ID.to_string(),
                program_executable: true,
                program_data_bytes: 36,
                program_account_sha256: digest(&program_bytes),
                programdata_lamports: 77_000,
                programdata_owner: bpf_loader_upgradeable::ID.to_string(),
                programdata_executable: false,
                programdata_data_bytes: u64::try_from(programdata_bytes.len())
                    .expect("ProgramData width"),
                deployment_slot: 91,
                upgrade_authority: authority.to_string(),
                live_elf_bytes: u64::try_from(before_live.len()).expect("live width"),
                live_elf_sha256: digest(&before_live),
                programdata_account_sha256: digest(&programdata_bytes),
            };
            let mut baseline = UpgradeBaselineV1 {
                schema: BASELINE_SCHEMA.into(),
                canonical_role_order: ROLES.iter().map(|role| String::from(*role)).collect(),
                role_ordinal: role_ordinal("trading").expect("role"),
                role: "trading".into(),
                program_id: program.to_string(),
                programdata_id: programdata.to_string(),
                expected_upgrade_authority: authority.to_string(),
                rpc_origin_redacted: "https://api.devnet.solana.com/".into(),
                genesis_hash: DEVNET_GENESIS_HASH.into(),
                context_slot: 489_212_834,
                observation,
                target_live_elf_bytes: u64::try_from(candidate_live.len())
                    .expect("candidate live width"),
                extension_additional_bytes: 0,
                current_rent_exempt_minimum_lamports: 77_000,
                target_rent_exempt_minimum_lamports: 77_000,
                extension_lamport_top_up: 0,
                baseline_sha256: String::new(),
            };
            baseline.baseline_sha256 = baseline_digest(&baseline).expect("baseline digest");
            let baseline_path = directory.0.join("baseline.json");
            fs::write(
                &baseline_path,
                serde_json::to_vec_pretty(&baseline).expect("baseline JSON"),
            )
            .expect("write baseline");
            let args = UpgradeArgsV1 {
                origin: ClusterOriginV1::parse(
                    "https://api.devnet.solana.com",
                    Some(DEVNET_GENESIS_HASH),
                )
                .expect("devnet origin"),
                role: "trading".into(),
                program_id: program,
                programdata_id: programdata,
                expected_upgrade_authority: authority,
                // These files deliberately do not exist. The implementation
                // may pass their paths to the fake CLI; it may never read them.
                authority_keypair: directory.0.join("missing-authority-keypair.json"),
                fee_payer: payer,
                fee_payer_keypair: directory.0.join("missing-payer-keypair.json"),
                elf_path,
                checked_release_gate_path: gate_path,
                expected_checked_release_gate_sha256: gate_sha256,
                expected_source_revision: gate.source_revision.clone(),
                expected_source_tree_sha256: gate.source_tree_sha256.clone(),
                baseline_path,
                receipt_path: directory.0.join("receipt.json"),
                dump_path: directory.0.join("dump.so"),
                solana_cli: directory.0.join("missing-solana-cli"),
                target_acknowledgment: format!("trading:{program}"),
                exclusive_payer_window_acknowledgment: Some(format!("trading:{program}:{payer}")),
                execute: true,
                preflight: false,
            };
            Self {
                _directory: directory,
                args,
                gate,
                solana_cli_version,
                program,
                programdata,
                authority,
                payer,
                raw_elf,
                candidate_live,
                before_live,
            }
        }

        fn rewrite_gate(&mut self) {
            fs::write(
                &self.args.checked_release_gate_path,
                serde_json::to_vec_pretty(&self.gate).expect("gate JSON"),
            )
            .expect("rewrite gate");
            self.args.expected_checked_release_gate_sha256 = digest(
                &fs::read(&self.args.checked_release_gate_path).expect("rewritten gate bytes"),
            );
        }

        fn extension_args(&self, additional_bytes: u64) -> ExtensionArgsV1 {
            let mut baseline: UpgradeBaselineV1 =
                serde_json::from_slice(&fs::read(&self.args.baseline_path).expect("read baseline"))
                    .expect("decode baseline");
            baseline.target_live_elf_bytes = baseline
                .observation
                .live_elf_bytes
                .checked_add(additional_bytes)
                .expect("target live width");
            baseline.extension_additional_bytes = additional_bytes;
            baseline.current_rent_exempt_minimum_lamports =
                baseline.observation.programdata_lamports;
            let rent_delta = additional_bytes
                .checked_mul(6_960)
                .expect("measured rent delta");
            baseline.target_rent_exempt_minimum_lamports = baseline
                .observation
                .programdata_lamports
                .checked_add(rent_delta)
                .expect("target rent");
            baseline.extension_lamport_top_up = rent_delta;
            baseline.baseline_sha256 = baseline_digest(&baseline).expect("extension baseline");
            fs::write(
                &self.args.baseline_path,
                serde_json::to_vec_pretty(&baseline).expect("extension baseline JSON"),
            )
            .expect("write extension baseline");
            ExtensionArgsV1 {
                origin: self.args.origin.clone(),
                role: self.args.role.clone(),
                program_id: self.program,
                programdata_id: self.programdata,
                expected_upgrade_authority: self.authority,
                authority_keypair: self.args.authority_keypair.clone(),
                fee_payer: self.payer,
                fee_payer_keypair: self.args.fee_payer_keypair.clone(),
                baseline_path: self.args.baseline_path.clone(),
                receipt_path: self._directory.0.join("extension-receipt.json"),
                solana_cli: self.args.solana_cli.clone(),
                expected_solana_cli_version: self.solana_cli_version.clone(),
                target_acknowledgment: format!("trading:{}:+{additional_bytes}", self.program),
                execute: true,
            }
        }
    }

    fn set_loader_observation(
        program: Pubkey,
        programdata: Pubkey,
        authority: Pubkey,
        live: &[u8],
        deployment_slot: u64,
    ) -> LoaderObservationV1 {
        let mut program_bytes = vec![0_u8; 36];
        program_bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(programdata.as_ref());
        let mut programdata_bytes = vec![0_u8; 45];
        programdata_bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
        programdata_bytes[4..12].copy_from_slice(&deployment_slot.to_le_bytes());
        programdata_bytes[12] = 1;
        programdata_bytes[13..45].copy_from_slice(authority.as_ref());
        programdata_bytes.extend_from_slice(live);
        loader_observation(
            program,
            programdata,
            authority,
            &RpcAccountV1 {
                lamports: 1_140,
                owner: bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: u64::MAX,
                data: program_bytes,
            },
            &RpcAccountV1 {
                lamports: 77_000,
                owner: bpf_loader_upgradeable::ID,
                executable: false,
                rent_epoch: u64::MAX,
                data: programdata_bytes,
            },
        )
        .expect("set Loader observation")
    }

    fn set_upgrade_transaction(
        signature: &str,
        program: Pubkey,
        programdata: Pubkey,
        authority: Pubkey,
        payer: Pubkey,
        slot: u64,
        programdata_lamports: u64,
        fee: u64,
    ) -> Value {
        let buffer = Pubkey::new_from_array([11; 32]);
        let spill = Pubkey::new_from_array([12; 32]);
        json!({
            "slot": slot,
            "transaction": {
                "signatures": [signature],
                "message": {
                    "accountKeys": [
                        {"pubkey": payer.to_string(), "signer": true, "writable": true},
                        {"pubkey": programdata.to_string(), "signer": false, "writable": true},
                        {"pubkey": program.to_string(), "signer": false, "writable": true},
                        {"pubkey": authority.to_string(), "signer": true, "writable": false},
                        {"pubkey": buffer.to_string(), "signer": false, "writable": true},
                        {"pubkey": spill.to_string(), "signer": false, "writable": true},
                        {"pubkey": sysvar::rent::ID.to_string(), "signer": false, "writable": false},
                        {"pubkey": sysvar::clock::ID.to_string(), "signer": false, "writable": false}
                    ],
                    "instructions": [{
                        "program": "bpf-upgradeable-loader",
                        "programId": bpf_loader_upgradeable::ID.to_string(),
                        "parsed": {
                            "type": "upgrade",
                            "info": {
                                "programDataAccount": programdata.to_string(),
                                "programAccount": program.to_string(),
                                "bufferAccount": buffer.to_string(),
                                "spillAccount": spill.to_string(),
                                "rentSysvar": sysvar::rent::ID.to_string(),
                                "clockSysvar": sysvar::clock::ID.to_string(),
                                "authority": authority.to_string()
                            }
                        }
                    }]
                }
            },
            "meta": {
                "err": null,
                "fee": fee,
                "preBalances": [
                    1_000_000, programdata_lamports, 1_140, 0, 10_000, 0, 1, 1
                ],
                "postBalances": [
                    1_000_000 - fee, programdata_lamports, 1_140, 0, 0, 10_000, 1, 1
                ]
            }
        })
    }

    fn mixed_account(owner: Pubkey, executable: bool, data: Vec<u8>) -> RpcAccountV1 {
        RpcAccountV1 {
            lamports: Rent::default().minimum_balance(data.len()),
            owner,
            executable,
            rent_epoch: u64::MAX,
            data,
        }
    }

    fn mixed_loader_accounts(
        programdata: Pubkey,
        authority: Pubkey,
        slot: u64,
        elf: &[u8],
    ) -> (RpcAccountV1, RpcAccountV1) {
        let mut program = vec![0_u8; 36];
        program[..4].copy_from_slice(&2_u32.to_le_bytes());
        program[4..].copy_from_slice(programdata.as_ref());
        let mut data = vec![0_u8; 45];
        data[..4].copy_from_slice(&3_u32.to_le_bytes());
        data[4..12].copy_from_slice(&slot.to_le_bytes());
        data[12] = 1;
        data[13..45].copy_from_slice(authority.as_ref());
        data.extend_from_slice(elf);
        (
            mixed_account(bpf_loader_upgradeable::ID, true, program),
            mixed_account(bpf_loader_upgradeable::ID, false, data),
        )
    }

    fn mixed_snapshot_row(
        role: &str,
        address: Pubkey,
        account: Option<&RpcAccountV1>,
    ) -> CarryForwardSnapshotAccountV1 {
        CarryForwardSnapshotAccountV1 {
            role: role.into(),
            address: address.to_string(),
            account: account.map(|account| CarryForwardAccountV1 {
                lamports: account.lamports,
                owner: account.owner.to_string(),
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                data_encoding: "base64".into(),
                data_len: account.data.len(),
                data_base64: BASE64.encode(&account.data),
                data_sha256: digest(&account.data),
            }),
        }
    }

    struct MixedSetAuditRunner {
        version: String,
        genesis: String,
        states: BTreeMap<Pubkey, SetRunnerStateV2>,
        transactions: BTreeMap<String, Value>,
        carry_context_slot: u64,
        carry_addresses: Vec<Pubkey>,
        carry_accounts: Vec<Option<RpcAccountV1>>,
        calls: Vec<Vec<String>>,
    }

    #[derive(Clone)]
    struct SetRunnerStateV2 {
        programdata: Pubkey,
        authority: Pubkey,
        payer: Pubkey,
        snapshot: SnapshotV1,
    }

    impl CliRunner for MixedSetAuditRunner {
        fn run(&mut self, arguments: &[String]) -> Result<CliOutput> {
            self.calls.push(arguments.to_vec());
            let stdout = match arguments.first().map(String::as_str) {
                Some("--version") => format!("{}\n", self.version),
                Some("genesis-hash") => format!("{}\n", self.genesis),
                command => panic!("mixed set auditor attempted mutation {command:?}"),
            };
            Ok(CliOutput {
                success: true,
                stdout,
                stderr: String::new(),
            })
        }

        fn read_snapshot(&mut self, query: &SnapshotQueryV1<'_>) -> Result<SnapshotV1> {
            let state = self
                .states
                .get(&query.program_id)
                .ok_or_else(|| Error::new("mixed set fake omitted Program"))?;
            if state.programdata != query.programdata_id
                || state.authority != query.expected_upgrade_authority
                || state.payer != query.payer
                || state.snapshot.context_slot < query.minimum_context_slot
            {
                return Err(Error::new("mixed set fake snapshot query mismatch"));
            }
            Ok(state.snapshot.clone())
        }

        fn read_carry_forward_accounts(
            &mut self,
            query: &CarryForwardQueryV1<'_>,
        ) -> Result<(u64, Vec<Option<RpcAccountV1>>)> {
            if query.addresses != self.carry_addresses
                || self.carry_context_slot < query.minimum_context_slot
            {
                return Err(Error::new("mixed set fake carry query mismatch"));
            }
            Ok((self.carry_context_slot, self.carry_accounts.clone()))
        }

        fn resolve_upgrade_transaction(
            &mut self,
            query: &UpgradeTransactionQueryV1<'_>,
        ) -> Result<UpgradeTransactionEvidenceV1> {
            validate_upgrade_transaction(
                query,
                self.transactions
                    .get(query.signature)
                    .cloned()
                    .ok_or_else(|| Error::new("mixed set fake omitted transaction"))?,
            )
        }
    }

    struct MixedSetFixture {
        _fixture: Fixture,
        args: UpgradeSetArgsV1,
        journal: UpgradeSetJournalV1,
        snapshot_path: PathBuf,
        snapshot: CarryForwardSnapshotV1,
        runner: MixedSetAuditRunner,
    }

    impl MixedSetFixture {
        fn new(completed_upgrades: usize) -> Self {
            assert!(completed_upgrades <= 5);
            let fixture = Fixture::new();
            let root = fs::canonicalize(&fixture._directory.0).expect("canonical fixture");
            let set_root = root.join("mixed-set");
            fs::create_dir(&set_root).expect("mixed set root");
            let gate_sha256 =
                digest(&fs::read(&fixture.args.checked_release_gate_path).expect("gate bytes"));
            let authority = fixture.authority;
            let payer = fixture.payer;
            let carry_slot = 700_000;
            let mut roles = Vec::new();
            let mut states = BTreeMap::new();
            let mut transactions = BTreeMap::new();
            let mut carry = Vec::new();
            for (index, (role, program_text, programdata_text)) in
                PERMANENT_DEVNET_UPGRADE_TARGETS_V1.iter().enumerate()
            {
                let byte = u8::try_from(index).expect("role byte");
                let program = parse_pubkey(program_text, "mixed Program").expect("Program");
                let programdata =
                    parse_pubkey(programdata_text, "mixed ProgramData").expect("ProgramData");
                let link = fixture
                    .gate
                    .links
                    .iter()
                    .find(|link| link.label == *role)
                    .expect("gate role");
                let elf = link.elf.as_ref().expect("gate ELF");
                let raw_elf = fs::read(root.join(&elf.canonical_path)).expect("ELF bytes");
                let receipt_path = set_root.join(format!("{role}-receipt.json"));
                let dump_path = set_root.join(format!("{role}-dump.so"));
                let mut receipt_ref = SetOptionalFileV1 {
                    canonical_path: path_argument(&receipt_path, "mixed receipt").expect("path"),
                    sha256: None,
                };
                let mut dump_ref = SetOptionalFileV1 {
                    canonical_path: path_argument(&dump_path, "mixed dump").expect("path"),
                    sha256: None,
                };
                if index < 2 {
                    let slot = 200 + u64::from(byte);
                    let (program_account, programdata_account) =
                        mixed_loader_accounts(programdata, authority, slot, &raw_elf);
                    let release = ArtifactReleaseV1::new(
                        dclutch_release_set_contract::ProgramIdentityV1::new(program.to_bytes())
                            .expect("program identity"),
                        dclutch_release_set_contract::ProgramIdentityV1::new(
                            bpf_loader_upgradeable::ID.to_bytes(),
                        )
                        .expect("loader identity"),
                        programdata.to_bytes(),
                        dclutch_core_contract::ContentId::new([30 + byte; 32]).expect("semantic"),
                        Sha256::digest(&raw_elf).into(),
                        slot,
                        dclutch_registry_contract::ArtifactUpgradePolicyV1::ExactAuthority,
                        Some(authority.to_bytes()),
                    )
                    .expect("carry artifact");
                    let body = release.to_bytes().to_vec();
                    let id: [u8; 32] = Sha256::digest(&body).into();
                    fs::write(&dump_path, &raw_elf).expect("carry dump");
                    dump_ref.sha256 = Some(digest(&raw_elf));
                    carry.push((
                        program,
                        programdata,
                        program_account,
                        programdata_account,
                        raw_elf,
                        body,
                        id,
                    ));
                    roles.push(UpgradeSetRoleV1 {
                        role: (*role).into(),
                        disposition: CheckedDeploymentDispositionV1::CarryForward,
                        program_id: program.to_string(),
                        programdata_id: programdata.to_string(),
                        baseline: None,
                        receipt: receipt_ref,
                        dump: dump_ref,
                    });
                    continue;
                }

                let mut old_elf = vec![0xa0 + byte; raw_elf.len()];
                old_elf[..4].copy_from_slice(b"\x7fOLD");
                let before_slot = 100 + u64::from(byte);
                let after_slot = 200 + u64::from(byte);
                let before =
                    set_loader_observation(program, programdata, authority, &old_elf, before_slot);
                let after =
                    set_loader_observation(program, programdata, authority, &raw_elf, after_slot);
                let context_slot = 600_000 + u64::from(byte) * 10;
                let mut baseline = UpgradeBaselineV1 {
                    schema: BASELINE_SCHEMA.into(),
                    canonical_role_order: ROLES.iter().map(|role| String::from(*role)).collect(),
                    role_ordinal: byte,
                    role: (*role).into(),
                    program_id: program.to_string(),
                    programdata_id: programdata.to_string(),
                    expected_upgrade_authority: authority.to_string(),
                    rpc_origin_redacted: fixture.args.origin.redacted_url(),
                    genesis_hash: DEVNET_GENESIS_HASH.into(),
                    context_slot,
                    observation: before.clone(),
                    target_live_elf_bytes: u64::try_from(raw_elf.len()).expect("ELF width"),
                    extension_additional_bytes: 0,
                    current_rent_exempt_minimum_lamports: before.programdata_lamports,
                    target_rent_exempt_minimum_lamports: before.programdata_lamports,
                    extension_lamport_top_up: 0,
                    baseline_sha256: String::new(),
                };
                baseline.baseline_sha256 = baseline_digest(&baseline).expect("baseline digest");
                let baseline_path = set_root.join(format!("{role}-baseline.json"));
                fs::write(
                    &baseline_path,
                    serde_json::to_vec_pretty(&baseline).expect("baseline JSON"),
                )
                .expect("baseline write");
                let completed = index - 2 < completed_upgrades;
                let snapshot = if completed {
                    let signature = Signature::from([80 + byte; 64]).to_string();
                    let fee = 5_000 + u64::from(byte);
                    let transaction = set_upgrade_transaction(
                        &signature,
                        program,
                        programdata,
                        authority,
                        payer,
                        after_slot,
                        before.programdata_lamports,
                        fee,
                    );
                    let mut receipt = UpgradeReceiptV1 {
                        schema: SCHEMA.into(),
                        phase: ReceiptPhaseV1::Complete,
                        operation_id: String::new(),
                        role: (*role).into(),
                        program_id: program.to_string(),
                        programdata_id: programdata.to_string(),
                        retained_upgrade_authority: authority.to_string(),
                        fee_payer: payer.to_string(),
                        rpc_origin_redacted: fixture.args.origin.redacted_url(),
                        genesis_hash: DEVNET_GENESIS_HASH.into(),
                        source_revision: fixture.gate.source_revision.clone(),
                        source_tree_sha256: fixture.gate.source_tree_sha256.clone(),
                        checked_release_gate_sha256: gate_sha256.clone(),
                        baseline_sha256: baseline.baseline_sha256.clone(),
                        baseline_context_slot: context_slot,
                        raw_elf_sha256: digest(&raw_elf),
                        live_elf_sha256: digest(&raw_elf),
                        live_elf_padding_bytes: 0,
                        solana_cli_version: fixture.solana_cli_version.clone(),
                        before_context_slot: context_slot,
                        before: before.clone(),
                        wallet_before_lamports: 1_000_000,
                        exclusive_payer_window_acknowledgment: format!("{role}:{program}:{payer}"),
                        transaction_signature: Some(signature.clone()),
                        solana_cli_output: Some(json!({
                            "programId": program.to_string(),
                            "signature": signature
                        })),
                        finalized_transaction: Some(transaction.clone()),
                        finalized_transaction_sha256: Some(digest(
                            &serde_json::to_vec(&transaction).expect("transaction JSON"),
                        )),
                        after_context_slot: Some(context_slot + 1),
                        after: Some(after.clone()),
                        arithmetic: Some(UpgradeArithmeticV1 {
                            transaction_payer_pre_lamports: 1_000_000,
                            transaction_payer_post_lamports: 1_000_000 - fee,
                            transaction_fee_lamports: fee,
                            payer_fee_delta_lamports: fee,
                            programdata_before_lamports: before.programdata_lamports,
                            programdata_after_lamports: after.programdata_lamports,
                            programdata_delta_lamports: 0,
                            operation_wallet_before_lamports: 1_000_000,
                            operation_wallet_after_lamports: 1_000_000 - fee,
                            operation_observed_net_spend_lamports: fee,
                            unattributed_cli_net_cost_lamports: 0,
                            accounting_scope: OPERATION_ACCOUNTING_SCOPE_V1.into(),
                            cost_attribution: OPERATION_COST_ATTRIBUTION_V1.into(),
                        }),
                        dump_sha256: Some(digest(&raw_elf)),
                        dump_shape: Some("raw-elf".into()),
                        receipt_sha256: String::new(),
                    };
                    receipt.operation_id = operation_id_from_receipt(&receipt);
                    receipt.receipt_sha256 =
                        digest(&serde_json::to_vec(&receipt).expect("receipt JSON"));
                    fs::write(
                        &receipt_path,
                        serde_json::to_vec_pretty(&receipt).expect("receipt JSON"),
                    )
                    .expect("receipt write");
                    fs::write(&dump_path, &raw_elf).expect("dump write");
                    receipt_ref.sha256 =
                        Some(digest(&fs::read(&receipt_path).expect("receipt bytes")));
                    dump_ref.sha256 = Some(digest(&raw_elf));
                    transactions.insert(
                        receipt.transaction_signature.expect("signature"),
                        transaction,
                    );
                    SnapshotV1 {
                        context_slot: context_slot + 1,
                        loader: after,
                        wallet_lamports: 1_000_000 - fee,
                        live_elf: raw_elf,
                    }
                } else {
                    SnapshotV1 {
                        context_slot,
                        loader: before,
                        wallet_lamports: 1_000_000,
                        live_elf: old_elf,
                    }
                };
                states.insert(
                    program,
                    SetRunnerStateV2 {
                        programdata,
                        authority,
                        payer,
                        snapshot,
                    },
                );
                roles.push(UpgradeSetRoleV1 {
                    role: (*role).into(),
                    disposition: CheckedDeploymentDispositionV1::Upgrade,
                    program_id: program.to_string(),
                    programdata_id: programdata.to_string(),
                    baseline: Some(SetPinnedFileV1 {
                        canonical_path: path_argument(&baseline_path, "mixed baseline")
                            .expect("path"),
                        sha256: digest(&fs::read(&baseline_path).expect("baseline bytes")),
                    }),
                    receipt: receipt_ref,
                    dump: dump_ref,
                });
            }

            let registry = &carry[0];
            let rent = &carry[1];
            let registry_raw = Pubkey::find_program_address(
                &[
                    RAW_RECORD_PDA_SEED_V1,
                    &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                    &registry.6,
                ],
                &registry.0,
            )
            .0;
            let registry_staging = Pubkey::find_program_address(
                &[
                    STAGING_CURSOR_PDA_SEED_V1,
                    &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                    &registry.6,
                ],
                &registry.0,
            )
            .0;
            let rent_raw = Pubkey::find_program_address(
                &[
                    RAW_RECORD_PDA_SEED_V1,
                    &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                    &rent.6,
                ],
                &registry.0,
            )
            .0;
            let rent_staging = Pubkey::find_program_address(
                &[
                    STAGING_CURSOR_PDA_SEED_V1,
                    &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                    &rent.6,
                ],
                &registry.0,
            )
            .0;
            let registry_raw_account = mixed_account(registry.0, false, registry.5.clone());
            let rent_raw_account = mixed_account(registry.0, false, rent.5.clone());
            let registry_binding = dclutch_release_set_contract::ExecutionRoleBindingV1::new(
                dclutch_release_set_contract::ProgramIdentityV1::new(registry.0.to_bytes())
                    .expect("registry identity"),
                dclutch_release_set_contract::ArtifactReleaseIdV1::decode(&registry.6)
                    .expect("registry artifact ID"),
            );
            let rent_binding = dclutch_release_set_contract::ExecutionRoleBindingV1::new(
                dclutch_release_set_contract::ProgramIdentityV1::new(rent.0.to_bytes())
                    .expect("rent identity"),
                dclutch_release_set_contract::ArtifactReleaseIdV1::decode(&rent.6)
                    .expect("rent artifact ID"),
            );
            let profile = ProtocolInfrastructureProfileV1::new(registry_binding, rent_binding)
                .expect("profile")
                .to_bytes()
                .to_vec();
            let core =
                parse_pubkey(PERMANENT_DEVNET_UPGRADE_TARGETS_V1[6].1, "Core").expect("Core");
            let profile_address = Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
                &core,
            )
            .0;
            let profile_account = mixed_account(core, false, profile);
            let carry_addresses = vec![
                registry.0,
                registry.1,
                rent.0,
                rent.1,
                registry_raw,
                registry_staging,
                rent_raw,
                rent_staging,
                profile_address,
            ];
            let carry_accounts = vec![
                Some(registry.2.clone()),
                Some(registry.3.clone()),
                Some(rent.2.clone()),
                Some(rent.3.clone()),
                Some(registry_raw_account.clone()),
                None,
                Some(rent_raw_account.clone()),
                None,
                Some(profile_account.clone()),
            ];
            let snapshot = CarryForwardSnapshotV1 {
                schema: CARRY_FORWARD_SNAPSHOT_SCHEMA.into(),
                endpoint: "https://api.devnet.solana.com".into(),
                commitment: "finalized".into(),
                rpc_method: "getMultipleAccounts".into(),
                context_slot: carry_slot,
                accounts: [
                    ("registry_program", registry.0, Some(&registry.2)),
                    ("registry_programdata", registry.1, Some(&registry.3)),
                    ("rent_program", rent.0, Some(&rent.2)),
                    ("rent_programdata", rent.1, Some(&rent.3)),
                    ("registry_raw", registry_raw, Some(&registry_raw_account)),
                    ("registry_staging", registry_staging, None),
                    ("rent_raw", rent_raw, Some(&rent_raw_account)),
                    ("rent_staging", rent_staging, None),
                    (
                        "infrastructure_profile",
                        profile_address,
                        Some(&profile_account),
                    ),
                ]
                .into_iter()
                .map(|(role, address, account)| mixed_snapshot_row(role, address, account))
                .collect(),
            };
            let snapshot_path = set_root.join("carry-forward-snapshot.json");
            fs::write(
                &snapshot_path,
                serde_json::to_vec_pretty(&snapshot).expect("snapshot JSON"),
            )
            .expect("snapshot write");
            let journal = UpgradeSetJournalV1 {
                schema: SET_JOURNAL_SCHEMA.into(),
                checked_release_gate: SetPinnedFileV1 {
                    canonical_path: path_argument(
                        &root.join("CHECKED_UPGRADE_GATE.json"),
                        "mixed gate",
                    )
                    .expect("path"),
                    sha256: gate_sha256,
                },
                source_revision: fixture.gate.source_revision.clone(),
                source_tree_sha256: fixture.gate.source_tree_sha256.clone(),
                devnet_genesis_hash: DEVNET_GENESIS_HASH.into(),
                solana_cli_version: fixture.solana_cli_version.clone(),
                retained_upgrade_authority: authority.to_string(),
                fee_payer: payer.to_string(),
                infrastructure_carry_forward: SetPinnedFileV1 {
                    canonical_path: path_argument(&snapshot_path, "mixed snapshot").expect("path"),
                    sha256: digest(&fs::read(&snapshot_path).expect("snapshot bytes")),
                },
                roles,
            };
            let journal_path = set_root.join("deployment-set-journal.json");
            fs::write(
                &journal_path,
                serde_json::to_vec_pretty(&journal).expect("journal JSON"),
            )
            .expect("journal write");
            let args = UpgradeSetArgsV1 {
                origin: fixture.args.origin.clone(),
                journal_path,
                solana_cli: root.join("missing-solana-cli"),
            };
            let runner = MixedSetAuditRunner {
                version: fixture.solana_cli_version.clone(),
                genesis: DEVNET_GENESIS_HASH.into(),
                states,
                transactions,
                carry_context_slot: carry_slot,
                carry_addresses,
                carry_accounts,
                calls: Vec::new(),
            };
            Self {
                _fixture: fixture,
                args,
                journal,
                snapshot_path,
                snapshot,
                runner,
            }
        }

        fn rewrite_journal(&self) {
            fs::write(
                &self.args.journal_path,
                serde_json::to_vec_pretty(&self.journal).expect("journal JSON"),
            )
            .expect("journal rewrite");
        }

        fn rewrite_snapshot(&mut self) {
            fs::write(
                &self.snapshot_path,
                serde_json::to_vec_pretty(&self.snapshot).expect("snapshot JSON"),
            )
            .expect("snapshot rewrite");
            self.journal.infrastructure_carry_forward.sha256 =
                digest(&fs::read(&self.snapshot_path).expect("snapshot bytes"));
            self.rewrite_journal();
        }

        fn rewrite_snapshot_data(&mut self, index: usize, bytes: &[u8]) {
            let account = self.snapshot.accounts[index]
                .account
                .as_mut()
                .expect("snapshot account");
            account.data_base64 = BASE64.encode(bytes);
            account.data_len = bytes.len();
            account.data_sha256 = digest(bytes);
            self.rewrite_snapshot();
        }
    }

    #[test]
    fn mixed_deployment_set_reports_only_the_next_execution_upgrade() {
        let mut fixture = MixedSetFixture::new(2);
        let report = audit_set_journal_with_runner(&fixture.args, &mut fixture.runner)
            .expect("mixed deployment-set audit");
        assert_eq!(report.schema, SET_AUDIT_SCHEMA);
        assert_eq!(report.completed_role_count, 4);
        assert_eq!(
            report.roles[..2]
                .iter()
                .map(|role| (&role.disposition, &role.status))
                .collect::<Vec<_>>(),
            vec![
                (
                    &CheckedDeploymentDispositionV1::CarryForward,
                    &SetRoleStatusV1::CarriedForward
                ),
                (
                    &CheckedDeploymentDispositionV1::CarryForward,
                    &SetRoleStatusV1::CarriedForward
                )
            ]
        );
        assert_eq!(
            report.next_role,
            Some(SetNextRoleV1 {
                ordinal: 4,
                role: "claims".into(),
                program_id: fixture.journal.roles[4].program_id.clone(),
                programdata_id: fixture.journal.roles[4].programdata_id.clone(),
                action: "run_exact_one_role_upgrade".into(),
            })
        );
        assert!(report.final_set_sha256.is_none());
        assert!(!report.mutation_permitted);
        assert!(fixture.runner.calls.iter().all(|call| matches!(
            call.first().map(String::as_str),
            Some("--version" | "genesis-hash")
        )));
    }

    #[test]
    fn complete_mixed_set_projects_carry_semantics_only_from_existing_artifacts() {
        let mut fixture = MixedSetFixture::new(5);
        let report = audit_set_journal_with_runner(&fixture.args, &mut fixture.runner)
            .expect("complete mixed audit");
        assert_eq!(report.completed_role_count, 7);
        assert!(report.next_role.is_none());
        assert_eq!(
            report.final_set_sha256,
            Some(final_set_digest(&fixture.journal).expect("final digest"))
        );
        let pin = authenticate_complete_upgrade_set_for_prepare(&fixture.args.journal_path)
            .expect("checked mixed prepare pin");
        assert_eq!(pin.roles.len(), 7);
        assert_eq!(
            pin.roles
                .iter()
                .map(|role| role.disposition)
                .collect::<Vec<_>>(),
            vec![
                CheckedDeploymentDispositionV1::CarryForward,
                CheckedDeploymentDispositionV1::CarryForward,
                CheckedDeploymentDispositionV1::Upgrade,
                CheckedDeploymentDispositionV1::Upgrade,
                CheckedDeploymentDispositionV1::Upgrade,
                CheckedDeploymentDispositionV1::Upgrade,
                CheckedDeploymentDispositionV1::Upgrade,
            ]
        );
        for role in &pin.roles[..2] {
            let release = ArtifactReleaseV1::decode(
                &crate::runtime::decode_hex(
                    role.artifact_release_body_hex
                        .as_deref()
                        .expect("carried artifact body"),
                )
                .expect("artifact hex"),
            )
            .expect("artifact decode");
            assert_eq!(
                role.semantic_release_id,
                hex(release.semantic_release_id().as_bytes())
            );
        }
        assert_eq!(
            pin.roles[5].semantic_release_id,
            hex(&COMPILED_DIRECT_RELEASE_ID_V1)
        );
        assert_eq!(
            pin.roles[3].semantic_release_id,
            hex(&RESOLUTION_CONTROLLER_RELEASE_ID_V5)
        );
        let mut substituted = pin;
        substituted.roles[0].semantic_release_id = "11".repeat(32);
        assert!(
            reauthenticate_checked_deployment_set_pin(&substituted)
                .expect_err("caller-authored carry semantic")
                .to_string()
                .contains("caller projection was substituted")
        );
    }

    #[test]
    fn mixed_set_refuses_every_disposition_and_role_closure_substitution() {
        let mut tag = MixedSetFixture::new(0);
        tag.journal.roles[0].disposition = CheckedDeploymentDispositionV1::Upgrade;
        tag.rewrite_journal();
        assert!(
            load_set_journal_path(&tag.args.journal_path)
                .expect_err("Registry Upgrade tag")
                .to_string()
                .contains("canonical mixed")
        );

        let mut five_carry = MixedSetFixture::new(0);
        five_carry.journal.roles[2].disposition = CheckedDeploymentDispositionV1::CarryForward;
        five_carry.rewrite_journal();
        assert!(
            load_set_journal_path(&five_carry.args.journal_path)
                .expect_err("execution carry tag")
                .to_string()
                .contains("canonical mixed")
        );

        let mut receipt = MixedSetFixture::new(0);
        let path = PathBuf::from(&receipt.journal.roles[0].receipt.canonical_path);
        fs::write(&path, b"not an Upgrade receipt").expect("receipt fixture");
        receipt.journal.roles[0].receipt.sha256 =
            Some(digest(&fs::read(&path).expect("receipt fixture bytes")));
        receipt.rewrite_journal();
        assert!(
            load_set_journal_path(&receipt.args.journal_path)
                .expect_err("carry receipt")
                .to_string()
                .contains("must contain one live dump and no Upgrade")
        );

        let mut reordered = MixedSetFixture::new(0);
        reordered.journal.roles.swap(0, 1);
        reordered.rewrite_journal();
        assert!(
            load_set_journal_path(&reordered.args.journal_path)
                .expect_err("reordered roles")
                .to_string()
                .contains("exact permanent")
        );
        let mut missing = MixedSetFixture::new(0);
        missing.journal.roles.pop();
        missing.rewrite_journal();
        assert!(
            load_set_journal_path(&missing.args.journal_path)
                .expect_err("missing role")
                .to_string()
                .contains("exactly 7")
        );
        let mut extra = MixedSetFixture::new(0);
        extra.journal.roles.push(extra.journal.roles[6].clone());
        extra.rewrite_journal();
        assert!(
            load_set_journal_path(&extra.args.journal_path)
                .expect_err("extra role")
                .to_string()
                .contains("exactly 7")
        );
    }

    #[test]
    fn carry_snapshot_refuses_transport_and_freshness_substitutions() {
        for field in ["endpoint", "commitment", "method"] {
            let mut fixture = MixedSetFixture::new(0);
            match field {
                "endpoint" => fixture.snapshot.endpoint = "https://example.invalid".into(),
                "commitment" => fixture.snapshot.commitment = "confirmed".into(),
                "method" => fixture.snapshot.rpc_method = "getAccountInfo".into(),
                _ => unreachable!(),
            }
            fixture.rewrite_snapshot();
            assert!(
                authenticate_carry_forward(&fixture.journal)
                    .expect_err("hostile snapshot transport")
                    .to_string()
                    .contains("schema, endpoint, finality, RPC method")
            );
        }
        let mut stale = MixedSetFixture::new(0);
        stale.snapshot.context_slot += 1;
        stale.rewrite_snapshot();
        assert!(
            audit_set_journal_with_runner(&stale.args, &mut stale.runner)
                .expect_err("stale context")
                .to_string()
                .contains("carry query mismatch")
        );

        let mut slot = MixedSetFixture::new(0);
        let mut bytes = BASE64
            .decode(
                slot.snapshot.accounts[1]
                    .account
                    .as_ref()
                    .expect("ProgramData")
                    .data_base64
                    .as_bytes(),
            )
            .expect("base64");
        bytes[4..12].copy_from_slice(&999_u64.to_le_bytes());
        slot.rewrite_snapshot_data(1, &bytes);
        assert!(
            authenticate_carry_forward(&slot.journal)
                .expect_err("stale slot")
                .to_string()
                .contains("deployment")
        );

        let mut authority = MixedSetFixture::new(0);
        let mut bytes = BASE64
            .decode(
                authority.snapshot.accounts[1]
                    .account
                    .as_ref()
                    .expect("ProgramData")
                    .data_base64
                    .as_bytes(),
            )
            .expect("base64");
        bytes[13..45].copy_from_slice(Pubkey::new_unique().as_ref());
        authority.rewrite_snapshot_data(1, &bytes);
        assert!(
            authenticate_carry_forward(&authority.journal)
                .expect_err("stale authority")
                .to_string()
                .contains("retained authority")
        );

        let mut live = MixedSetFixture::new(0);
        let mut bytes = BASE64
            .decode(
                live.snapshot.accounts[1]
                    .account
                    .as_ref()
                    .expect("ProgramData")
                    .data_base64
                    .as_bytes(),
            )
            .expect("base64");
        *bytes.last_mut().expect("ELF tail") ^= 1;
        live.rewrite_snapshot_data(1, &bytes);
        let error = authenticate_carry_forward(&live.journal).expect_err("stale live tail");
        assert!(error.to_string().contains("ElfDigestMismatch"), "{error}");
    }

    #[test]
    fn carry_snapshot_refuses_record_profile_and_staging_substitutions() {
        let mut raw_pda = MixedSetFixture::new(0);
        raw_pda.snapshot.accounts[4].address = Pubkey::new_unique().to_string();
        raw_pda.rewrite_snapshot();
        assert!(
            authenticate_carry_forward(&raw_pda.journal)
                .expect_err("raw PDA")
                .to_string()
                .contains("raw or staging PDA")
        );

        let mut profile_pda = MixedSetFixture::new(0);
        profile_pda.snapshot.accounts[8].address = Pubkey::new_unique().to_string();
        profile_pda.rewrite_snapshot();
        assert!(
            authenticate_carry_forward(&profile_pda.journal)
                .expect_err("profile PDA")
                .to_string()
                .contains("singleton infrastructure profile PDA")
        );

        let mut raw_body = MixedSetFixture::new(0);
        let mut bytes = BASE64
            .decode(
                raw_body.snapshot.accounts[4]
                    .account
                    .as_ref()
                    .expect("raw")
                    .data_base64
                    .as_bytes(),
            )
            .expect("base64");
        bytes[112] ^= 1;
        raw_body.rewrite_snapshot_data(4, &bytes);
        assert!(
            authenticate_carry_forward(&raw_body.journal)
                .expect_err("raw body")
                .to_string()
                .contains("profile artifact ID")
        );

        let mut owner = MixedSetFixture::new(0);
        owner.snapshot.accounts[4]
            .account
            .as_mut()
            .expect("raw")
            .owner = Pubkey::new_unique().to_string();
        owner.rewrite_snapshot();
        assert!(
            authenticate_carry_forward(&owner.journal)
                .expect_err("raw owner")
                .to_string()
                .contains("owner/executable/length")
        );

        let mut rent = MixedSetFixture::new(0);
        rent.snapshot.accounts[4]
            .account
            .as_mut()
            .expect("raw")
            .lamports = 0;
        rent.rewrite_snapshot();
        assert!(
            authenticate_carry_forward(&rent.journal)
                .expect_err("raw rent")
                .to_string()
                .contains("not rent exempt")
        );

        let mut staging = MixedSetFixture::new(0);
        staging.snapshot.accounts[5].account = Some(CarryForwardAccountV1 {
            lamports: 0,
            owner: solana_sdk_ids::system_program::ID.to_string(),
            executable: false,
            rent_epoch: u64::MAX,
            data_encoding: "base64".into(),
            data_len: 0,
            data_base64: String::new(),
            data_sha256: digest(&[]),
        });
        staging.rewrite_snapshot();
        let error =
            authenticate_carry_forward(&staging.journal).expect_err("fabricated empty staging");
        assert!(
            error.to_string().contains("finalized account absence"),
            "{error}"
        );
    }

    #[test]
    fn happy_path_updates_one_permanent_id_and_records_exact_arithmetic() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        let receipt =
            execute_with_runner(&fixture.args, &mut runner).expect("checked Upgrade completes");
        assert_eq!(receipt.phase, ReceiptPhaseV1::Complete);
        assert_eq!(receipt.role, "trading");
        assert_eq!(receipt.program_id, fixture.program.to_string());
        assert_eq!(
            receipt.checked_release_gate_sha256,
            fixture.args.expected_checked_release_gate_sha256
        );
        assert_eq!(
            receipt.source_tree_sha256,
            fixture.args.expected_source_tree_sha256
        );
        assert_eq!(
            receipt.transaction_signature,
            Some(Signature::from([7_u8; 64]).to_string())
        );
        assert_eq!(receipt.after.as_ref().expect("after").deployment_slot, 92);
        assert_eq!(
            receipt.arithmetic,
            Some(UpgradeArithmeticV1 {
                transaction_payer_pre_lamports: 1_000_000,
                transaction_payer_post_lamports: 985_000,
                transaction_fee_lamports: 15_000,
                payer_fee_delta_lamports: 15_000,
                programdata_before_lamports: 77_000,
                programdata_after_lamports: 77_000,
                programdata_delta_lamports: 0,
                operation_wallet_before_lamports: 1_000_000,
                operation_wallet_after_lamports: 985_000,
                operation_observed_net_spend_lamports: 15_000,
                unattributed_cli_net_cost_lamports: 0,
                accounting_scope: OPERATION_ACCOUNTING_SCOPE_V1.into(),
                cost_attribution: OPERATION_COST_ATTRIBUTION_V1.into(),
            })
        );
        assert_eq!(receipt.dump_shape.as_deref(), Some("raw-elf"));
        assert!(fixture.args.receipt_path.is_file());
        assert!(runner.calls.iter().any(|call| {
            call.first().map(String::as_str) == Some("program")
                && call.get(1).map(String::as_str) == Some("deploy")
                && call.contains(&fixture.program.to_string())
        }));
        assert_eq!(
            runner.snapshot_minimum_slots,
            vec![489_212_834, 489_212_834]
        );
    }

    #[test]
    fn execute_refuses_without_exact_exclusive_payer_window_acknowledgment() {
        let mut fixture = Fixture::new();
        fixture.args.exclusive_payer_window_acknowledgment = None;
        let mut runner = FakeRunner::new(&fixture);
        let error = execute_with_runner(&fixture.args, &mut runner).expect_err("must refuse");
        assert!(error.to_string().contains(EXCLUSIVE_PAYER_ACK_FLAG));
        assert!(runner.calls.is_empty());
    }

    #[test]
    fn operation_accounting_keeps_opaque_cli_net_cost_unattributed() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.after_wallet = 980_000;
        let receipt = execute_with_runner(&fixture.args, &mut runner).expect("checked Upgrade");
        let arithmetic = receipt.arithmetic.expect("complete arithmetic");
        assert_eq!(arithmetic.transaction_fee_lamports, 15_000);
        assert_eq!(arithmetic.operation_observed_net_spend_lamports, 20_000);
        assert_eq!(arithmetic.unattributed_cli_net_cost_lamports, 5_000);
        assert_eq!(arithmetic.accounting_scope, OPERATION_ACCOUNTING_SCOPE_V1);
        assert_eq!(arithmetic.cost_attribution, OPERATION_COST_ATTRIBUTION_V1);
    }

    #[test]
    fn preflight_is_key_free_read_only_and_runs_full_candidate_admission() {
        let mut fixture = Fixture::new();
        fixture.args.execute = false;
        fixture.args.preflight = true;
        let mut runner = FakeRunner::new(&fixture);
        let report = preflight_with_runner(&fixture.args, &mut runner).expect("preflight");
        assert_eq!(report.schema, PREFLIGHT_SCHEMA);
        assert_eq!(
            report.checked_release_gate_sha256,
            fixture.args.expected_checked_release_gate_sha256
        );
        assert!(!report.mutation_permitted);
        assert_eq!(report.receipt_phase, None);
        assert!(!fixture.args.receipt_path.exists());
        assert!(!fixture.args.dump_path.exists());
        assert_eq!(runner.snapshot_minimum_slots, vec![489_212_834]);
        assert!(runner.calls.iter().all(|call| {
            call.first().map(String::as_str) != Some("address")
                && call.first().map(String::as_str) != Some("program")
        }));
    }

    #[test]
    fn exact_upgrade_transaction_refuses_fee_signature_and_instruction_substitution() {
        let make_transaction = |fixture: &Fixture, runner: &mut FakeRunner| {
            let signature = Signature::from([7_u8; 64]).to_string();
            let query = UpgradeTransactionQueryV1 {
                origin: &fixture.args.origin,
                signature: &signature,
                program_id: fixture.program,
                programdata_id: fixture.programdata,
                authority: fixture.authority,
                payer: fixture.payer,
                deployment_slot: runner.after_slot,
                programdata_before_lamports: runner.before_programdata_lamports,
                programdata_after_lamports: runner.after_programdata_lamports,
            };
            runner
                .resolve_upgrade_transaction(&query)
                .expect("valid synthetic Upgrade transaction")
                .transaction
        };

        let fee = Fixture::new();
        let mut fee_runner = FakeRunner::new(&fee);
        let mut fee_transaction = make_transaction(&fee, &mut fee_runner);
        fee_transaction["meta"]["fee"] = json!(14_999);
        fee_runner.upgrade_transaction_override = Some(fee_transaction);
        let error = execute_with_runner(&fee.args, &mut fee_runner)
            .expect_err("payer delta must equal finalized fee");
        assert!(error.to_string().contains("payer delta is not exactly"));

        let signature = Fixture::new();
        let mut signature_runner = FakeRunner::new(&signature);
        let mut signature_transaction = make_transaction(&signature, &mut signature_runner);
        signature_transaction["transaction"]["signatures"][0] =
            json!(Signature::from([9_u8; 64]).to_string());
        signature_runner.upgrade_transaction_override = Some(signature_transaction);
        let error = execute_with_runner(&signature.args, &mut signature_runner)
            .expect_err("CLI signature substitution must refuse");
        assert!(error.to_string().contains("fee-payer signature"));

        let instruction = Fixture::new();
        let mut instruction_runner = FakeRunner::new(&instruction);
        let mut instruction_transaction = make_transaction(&instruction, &mut instruction_runner);
        instruction_transaction["transaction"]["message"]["instructions"][0]["parsed"]["info"]["programAccount"] =
            json!(Pubkey::new_unique().to_string());
        instruction_runner.upgrade_transaction_override = Some(instruction_transaction);
        let error = execute_with_runner(&instruction.args, &mut instruction_runner)
            .expect_err("instruction Program substitution must refuse");
        assert!(error.to_string().contains("substituted Program"));

        let rent = Fixture::new();
        let mut rent_runner = FakeRunner::new(&rent);
        let mut rent_transaction = make_transaction(&rent, &mut rent_runner);
        rent_transaction["meta"]["postBalances"][1] = json!(77_001);
        rent_runner.upgrade_transaction_override = Some(rent_transaction);
        let error = execute_with_runner(&rent.args, &mut rent_runner)
            .expect_err("transaction ProgramData rent movement must refuse");
        assert!(
            error
                .to_string()
                .contains("preserve and bridge exact ProgramData rent")
        );
    }

    #[test]
    fn canonical_receipt_digest_and_phase_shape_refuse_tampering() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        let accepted = execute_with_runner(&fixture.args, &mut runner).expect("complete Upgrade");
        let mut hostile = accepted.clone();
        hostile.dump_sha256 = Some("f".repeat(64));
        fs::write(
            &fixture.args.receipt_path,
            serde_json::to_vec_pretty(&hostile).expect("hostile receipt"),
        )
        .expect("write hostile receipt");
        let mut replay = FakeRunner::new(&fixture);
        replay.deployed = true;
        let error = execute_with_runner(&fixture.args, &mut replay)
            .expect_err("changed completion field must break canonical digest");
        assert!(error.to_string().contains("receipt SHA-256"));
        assert!(replay.calls.is_empty());

        let prepared = Fixture::new();
        let mut interrupted = FakeRunner::new(&prepared);
        interrupted.deploy_success = false;
        let _ = execute_with_runner(&prepared.args, &mut interrupted)
            .expect_err("leave prepared receipt");
        let mut receipt = load_receipt(&prepared.args.receipt_path)
            .expect("read prepared")
            .expect("prepared");
        receipt.dump_sha256 = Some(digest(b"later-only"));
        write_receipt(&prepared.args.receipt_path, &mut receipt)
            .expect("canonical hostile receipt");
        let mut resume = FakeRunner::new(&prepared);
        let error = execute_with_runner(&prepared.args, &mut resume)
            .expect_err("prepared receipt cannot carry dump evidence");
        assert!(error.to_string().contains("later-phase fields"));
        assert!(resume.calls.is_empty());

        let late = Fixture::new();
        let mut first = FakeRunner::new(&late);
        let accepted = execute_with_runner(&late.args, &mut first).expect("complete Upgrade");
        let mut receipt = accepted;
        receipt
            .arithmetic
            .as_mut()
            .expect("complete arithmetic")
            .transaction_fee_lamports += 1;
        write_receipt(&late.args.receipt_path, &mut receipt).expect("rehash hostile receipt");
        let mut replay = FakeRunner::new(&late);
        replay.deployed = true;
        let error = execute_with_runner(&late.args, &mut replay)
            .expect_err("rehashed self-declared arithmetic cannot replace transaction truth");
        assert!(
            error
                .to_string()
                .contains("operation spend is smaller than final transaction fee"),
            "{error}"
        );
    }

    #[test]
    fn complete_receipt_reports_cli_payload_dump_and_context_drift() {
        let cli = Fixture::new();
        let mut first = FakeRunner::new(&cli);
        let _ = execute_with_runner(&cli.args, &mut first).expect("complete Upgrade");
        let mut replay = FakeRunner::new(&cli);
        replay.deployed = true;
        replay.version.push_str(" drift");
        let error = execute_with_runner(&cli.args, &mut replay).expect_err("CLI drift");
        assert!(error.to_string().contains("artifact evidence pins"));

        let payload = Fixture::new();
        let mut first = FakeRunner::new(&payload);
        let _ = execute_with_runner(&payload.args, &mut first).expect("complete Upgrade");
        let mut replay = FakeRunner::new(&payload);
        replay.deployed = true;
        replay.after_live[4] ^= 1;
        let error = execute_with_runner(&payload.args, &mut replay).expect_err("payload drift");
        assert!(error.to_string().contains("receipt drifted"));

        let dump = Fixture::new();
        let mut first = FakeRunner::new(&dump);
        let _ = execute_with_runner(&dump.args, &mut first).expect("complete Upgrade");
        fs::write(&dump.args.dump_path, b"changed dump").expect("change dump");
        let mut replay = FakeRunner::new(&dump);
        replay.deployed = true;
        let error = execute_with_runner(&dump.args, &mut replay).expect_err("dump drift");
        assert!(error.to_string().contains("deployed-byte dump is neither"));

        let context = Fixture::new();
        let mut first = FakeRunner::new(&context);
        let _ = execute_with_runner(&context.args, &mut first).expect("complete Upgrade");
        let mut replay = FakeRunner::new(&context);
        replay.deployed = true;
        replay.after_context_slot = 489_212_834;
        let error = execute_with_runner(&context.args, &mut replay).expect_err("stale context");
        assert!(error.to_string().contains("below minContextSlot"));
    }

    #[test]
    fn extension_signature_history_paginates_until_below_target_and_refuses_ambiguity() {
        let signature = |byte| Signature::from([byte; 64]).to_string();
        let mut pages = VecDeque::from([
            vec![
                json!({"signature": signature(1), "slot": 100, "err": null}),
                json!({"signature": signature(2), "slot": 99, "err": null}),
            ],
            vec![
                json!({"signature": signature(3), "slot": 92, "err": null}),
                json!({"signature": signature(4), "slot": 91, "err": null}),
            ],
        ]);
        let mut cursors = Vec::new();
        let selected = find_extension_signature(92, |before| {
            cursors.push(before.map(str::to_owned));
            Ok(pages.pop_front().expect("bounded page"))
        })
        .expect("second page target");
        assert_eq!(selected, signature(3));
        assert_eq!(cursors, vec![None, Some(signature(2))]);

        let ambiguous = vec![
            json!({"signature": signature(5), "slot": 92, "err": null}),
            json!({"signature": signature(6), "slot": 92, "err": null}),
            json!({"signature": signature(7), "slot": 91, "err": null}),
        ];
        let error = find_extension_signature(92, |_| Ok(ambiguous.clone()))
            .expect_err("two same-slot candidates must refuse");
        assert!(error.to_string().contains("observed 2"));

        let nonmonotonic = vec![
            json!({"signature": signature(8), "slot": 91, "err": null}),
            json!({"signature": signature(9), "slot": 93, "err": null}),
        ];
        let error = find_extension_signature(92, |_| Ok(nonmonotonic.clone()))
            .expect_err("nonmonotonic provider history must refuse");
        assert!(error.to_string().contains("not monotonically"));

        let mut page = 0_u8;
        let error = find_extension_signature(92, |_| {
            page = page.checked_add(1).expect("bounded test page");
            Ok(vec![json!({
                "signature": signature(page.checked_add(20).expect("signature byte")),
                "slot": 200_u64.checked_sub(u64::from(page)).expect("slot"),
                "err": null
            })])
        })
        .expect_err("history that never crosses target must stop at page bound");
        assert!(error.to_string().contains("16-page bound"));
        assert_eq!(usize::from(page), SIGNATURE_HISTORY_MAX_PAGES);
    }

    #[test]
    fn baseline_hash_commits_role_order_and_fresh_prestate() {
        let ordered = Fixture::new();
        let mut hostile: UpgradeBaselineV1 =
            serde_json::from_slice(&fs::read(&ordered.args.baseline_path).expect("baseline bytes"))
                .expect("baseline");
        hostile.canonical_role_order.swap(0, 1);
        hostile.baseline_sha256 = baseline_digest(&hostile).expect("hostile digest");
        fs::write(
            &ordered.args.baseline_path,
            serde_json::to_vec_pretty(&hostile).expect("hostile baseline JSON"),
        )
        .expect("write hostile baseline");
        let mut ordered_runner = FakeRunner::new(&ordered);
        let error = execute_with_runner(&ordered.args, &mut ordered_runner)
            .expect_err("noncanonical role order must refuse");
        assert!(error.to_string().contains("canonical role order"));
        assert!(ordered_runner.calls.is_empty());

        let stale = Fixture::new();
        let mut stale_baseline: UpgradeBaselineV1 =
            serde_json::from_slice(&fs::read(&stale.args.baseline_path).expect("baseline bytes"))
                .expect("baseline");
        stale_baseline.observation.deployment_slot += 1;
        stale_baseline.baseline_sha256 =
            baseline_digest(&stale_baseline).expect("stale baseline digest");
        fs::write(
            &stale.args.baseline_path,
            serde_json::to_vec_pretty(&stale_baseline).expect("stale baseline JSON"),
        )
        .expect("write stale baseline");
        let mut stale_runner = FakeRunner::new(&stale);
        let error = execute_with_runner(&stale.args, &mut stale_runner)
            .expect_err("fresh observation must refuse stale slot");
        assert!(
            error
                .to_string()
                .contains("does not exactly match baseline")
        );
        assert!(
            stale_runner
                .calls
                .iter()
                .all(|call| call.get(1).map(String::as_str) != Some("deploy"))
        );
    }

    #[test]
    fn completed_receipt_rechecks_live_state_without_another_write_or_key_read() {
        let fixture = Fixture::new();
        let mut first = FakeRunner::new(&fixture);
        let accepted = execute_with_runner(&fixture.args, &mut first).expect("first run");
        let mut second = FakeRunner::new(&fixture);
        second.deployed = true;
        let replay = execute_with_runner(&fixture.args, &mut second).expect("receipt replay");
        assert_eq!(replay, accepted);
        assert!(second.calls.iter().all(|call| {
            call.first().map(String::as_str) != Some("address")
                && call.get(1).map(String::as_str) != Some("deploy")
                && call.get(1).map(String::as_str) != Some("dump")
        }));
        assert_eq!(second.snapshot_minimum_slots, vec![489_212_835]);
    }

    #[test]
    fn exact_target_acknowledgment_refuses_role_or_program_substitution() {
        let mut fixture = Fixture::new();
        fixture.args.target_acknowledgment = format!("core:{}", fixture.program);
        let mut runner = FakeRunner::new(&fixture);
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("substituted target must refuse");
        assert!(error.to_string().contains("exact target trading:"));
        assert!(runner.calls.is_empty());
    }

    #[test]
    fn canonical_generated_gate_validates_all_thirteen_links() {
        let fixture = Fixture::new();
        let gate = validate_checked_release_gate(&fixture.args).expect("canonical gate");
        assert_eq!(gate.raw_elf, fixture.raw_elf);
        assert_eq!(
            gate.gate_sha256,
            fixture.args.expected_checked_release_gate_sha256
        );
        assert_eq!(fixture.gate.links.len(), 13);
    }

    #[test]
    fn baseline_capacity_derives_only_zero_padding_and_never_truncates() {
        let raw = b"\x7fELFgate";
        let live = candidate_live_image(raw, 16).expect("candidate fits");
        assert_eq!(&live[..raw.len()], raw);
        assert!(live[raw.len()..].iter().all(|byte| *byte == 0));
        let error = candidate_live_image(raw, 4).expect_err("candidate cannot truncate");
        assert!(error.to_string().contains("extension baseline first"));
    }

    #[test]
    fn handwritten_true_zero_json_confers_no_authority() {
        let mut fixture = Fixture::new();
        let handwritten = json!({
            "schema": "dclutch-checked-devnet-upgrade-artifact-v1",
            "checked_release_accepted": true,
            "sbf_build_diagnostics_total": 0,
            "frame_diagnostics_total": 0
        });
        fs::write(
            &fixture.args.checked_release_gate_path,
            serde_json::to_vec_pretty(&handwritten).expect("handwritten JSON"),
        )
        .expect("write handwritten JSON");
        fixture.args.expected_checked_release_gate_sha256 =
            digest(&fs::read(&fixture.args.checked_release_gate_path).expect("handwritten bytes"));
        let error = validate_checked_release_gate(&fixture.args)
            .expect_err("handwritten acceptance must refuse");
        assert!(error.to_string().contains("handwritten acceptance"));
    }

    #[test]
    fn missing_duplicate_and_unknown_link_roles_refuse() {
        let mut missing = Fixture::new();
        missing.gate.links.pop();
        missing.gate.link_count -= 1;
        missing.rewrite_gate();
        assert!(
            validate_checked_release_gate(&missing.args)
                .expect_err("missing link")
                .to_string()
                .contains("all 13 shipped links")
        );

        let mut duplicate = Fixture::new();
        duplicate.gate.links[1].label = duplicate.gate.links[0].label.clone();
        duplicate.rewrite_gate();
        assert!(
            validate_checked_release_gate(&duplicate.args)
                .expect_err("duplicate link")
                .to_string()
                .contains("duplicated link role")
        );

        let mut unknown = Fixture::new();
        unknown.gate.links[0].label = "unknown-role".into();
        unknown.rewrite_gate();
        assert!(
            validate_checked_release_gate(&unknown.args)
                .expect_err("unknown link")
                .to_string()
                .contains("unknown link role")
        );
    }

    #[test]
    fn swapped_role_elf_refuses_even_when_gate_digest_is_reacknowledged() {
        let mut fixture = Fixture::new();
        let core = fixture
            .gate
            .links
            .iter()
            .find(|link| link.label == "core")
            .and_then(|link| link.elf.clone())
            .expect("core ELF");
        fixture
            .gate
            .links
            .iter_mut()
            .find(|link| link.label == "trading")
            .expect("trading link")
            .elf = Some(core);
        fixture.rewrite_gate();
        let error = validate_checked_release_gate(&fixture.args).expect_err("swapped role ELF");
        assert!(error.to_string().contains("exact canonical role ELF"));
    }

    #[test]
    fn changed_compile_log_and_frame_report_refuse_by_hash() {
        let compile = Fixture::new();
        fs::write(
            compile._directory.0.join("build-claims.log"),
            b"changed after admission\n",
        )
        .expect("change compile log");
        assert!(
            validate_checked_release_gate(&compile.args)
                .expect_err("changed compile log")
                .to_string()
                .contains("bytes or SHA-256 changed")
        );

        let frame = Fixture::new();
        fs::write(
            frame._directory.0.join("frame/claims.txt"),
            b"changed after admission\n",
        )
        .expect("change frame report");
        assert!(
            validate_checked_release_gate(&frame.args)
                .expect_err("changed frame report")
                .to_string()
                .contains("bytes or SHA-256 changed")
        );
    }

    #[test]
    fn wrong_expected_source_refuses_before_cli() {
        let mut fixture = Fixture::new();
        fixture.args.expected_source_revision = "f".repeat(40);
        let mut runner = FakeRunner::new(&fixture);
        let error = execute_with_runner(&fixture.args, &mut runner).expect_err("wrong source");
        assert!(error.to_string().contains("source commit/tree differs"));
        assert!(runner.calls.is_empty());
    }

    #[test]
    fn gate_path_escape_and_symlink_refuse() {
        let mut escape = Fixture::new();
        escape
            .gate
            .links
            .iter_mut()
            .find(|link| link.label == "trading")
            .and_then(|link| link.elf.as_mut())
            .expect("trading ELF")
            .canonical_path = "../outside.so".into();
        escape.rewrite_gate();
        assert!(
            validate_checked_release_gate(&escape.args)
                .expect_err("path escape")
                .to_string()
                .contains("contains an escape")
        );

        let symlink = Fixture::new();
        let outside = symlink._directory.0.join("outside.so");
        fs::rename(&symlink.args.elf_path, &outside).expect("move admitted ELF");
        std::os::unix::fs::symlink(&outside, &symlink.args.elf_path).expect("symlink ELF");
        assert!(
            validate_checked_release_gate(&symlink.args)
                .expect_err("symlink ELF")
                .to_string()
                .contains("regular non-symlink")
        );
    }

    #[test]
    fn non_devnet_genesis_refuses_before_any_account_or_deploy() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.genesis = "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY".into();
        let error =
            execute_with_runner(&fixture.args, &mut runner).expect_err("testnet must refuse");
        assert!(error.to_string().contains("accepts only exact devnet"));
        assert!(runner.calls.iter().all(|call| {
            call.first().map(String::as_str) != Some("account")
                && call.get(1).map(String::as_str) != Some("deploy")
        }));
    }

    #[test]
    fn mainnet_genesis_has_its_own_unconditional_refusal() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.genesis = MAINNET_BETA_GENESIS_HASH.into();
        let error =
            execute_with_runner(&fixture.args, &mut runner).expect_err("mainnet must refuse");
        assert!(
            error
                .to_string()
                .contains("mainnet is unconditionally refused")
        );
    }

    #[test]
    fn raw_digest_mismatch_refuses_before_the_cli_or_key_paths() {
        let fixture = Fixture::new();
        fs::write(&fixture.args.elf_path, b"\x7fELFtampered").expect("tamper candidate fixture");
        let mut runner = FakeRunner::new(&fixture);
        let error =
            execute_with_runner(&fixture.args, &mut runner).expect_err("tampered ELF must refuse");
        assert!(error.to_string().contains("bytes or SHA-256 changed"));
        assert!(runner.calls.is_empty());
    }

    #[test]
    fn current_candidate_without_receipt_is_stale_and_refused() {
        let fixture = Fixture::new();
        let mut baseline: UpgradeBaselineV1 =
            serde_json::from_slice(&fs::read(&fixture.args.baseline_path).expect("baseline bytes"))
                .expect("baseline");
        let mut account = vec![0_u8; 45];
        account[..4].copy_from_slice(&3_u32.to_le_bytes());
        account[4..12].copy_from_slice(&91_u64.to_le_bytes());
        account[12] = 1;
        account[13..45].copy_from_slice(fixture.authority.as_ref());
        account.extend_from_slice(&fixture.candidate_live);
        baseline.observation.programdata_data_bytes =
            u64::try_from(account.len()).expect("ProgramData width");
        baseline.observation.live_elf_bytes =
            u64::try_from(fixture.candidate_live.len()).expect("live width");
        baseline.observation.live_elf_sha256 = digest(&fixture.candidate_live);
        baseline.observation.programdata_account_sha256 = digest(&account);
        baseline.baseline_sha256 = baseline_digest(&baseline).expect("baseline digest");
        fs::write(
            &fixture.args.baseline_path,
            serde_json::to_vec_pretty(&baseline).expect("baseline JSON"),
        )
        .expect("rewrite baseline");
        let mut runner = FakeRunner::new(&fixture);
        runner.before_live = fixture.candidate_live.clone();
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("already-current candidate without receipt must refuse");
        assert!(error.to_string().contains("already equals the candidate"));
        assert!(
            runner
                .calls
                .iter()
                .all(|call| call.get(1).map(String::as_str) != Some("deploy"))
        );
    }

    #[test]
    fn loader_program_linkage_is_authenticated_before_deploy() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.linked_programdata = Pubkey::new_unique();
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("substituted ProgramData must refuse");
        assert!(error.to_string().contains("links ProgramData"));
        assert!(
            runner
                .calls
                .iter()
                .all(|call| call.get(1).map(String::as_str) != Some("deploy"))
        );
    }

    #[test]
    fn retained_authority_and_keypair_identity_are_both_authenticated() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.authority = Pubkey::new_unique();
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("wrong authority keypair must refuse");
        assert!(error.to_string().contains("keypair path resolves"));
        assert!(
            runner
                .calls
                .iter()
                .all(|call| call.get(1).map(String::as_str) != Some("deploy"))
        );
    }

    #[test]
    fn deploy_output_cannot_substitute_the_program() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.deploy_program = Pubkey::new_unique();
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("substituted deploy output must refuse");
        assert!(
            error
                .to_string()
                .contains("deploy output substituted program")
        );
        let receipt = load_receipt(&fixture.args.receipt_path)
            .expect("receipt read")
            .expect("prepared receipt");
        assert_eq!(receipt.phase, ReceiptPhaseV1::Prepared);
    }

    #[test]
    fn poststate_requires_slot_advancement_and_never_replays() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.after_slot = runner.before_slot;
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("stationary slot must refuse");
        assert!(
            error
                .to_string()
                .contains("deployment slot did not advance")
        );
        let deploy_calls = runner
            .calls
            .iter()
            .filter(|call| call.get(1).map(String::as_str) == Some("deploy"))
            .count();
        assert_eq!(deploy_calls, 1);

        let mut resume = FakeRunner::new(&fixture);
        resume.deployed = true;
        resume.after_slot = resume.before_slot;
        let _ = execute_with_runner(&fixture.args, &mut resume)
            .expect_err("submitted stationary state remains ambiguous");
        assert!(
            resume
                .calls
                .iter()
                .all(|call| call.get(1).map(String::as_str) != Some("deploy"))
        );
    }

    #[test]
    fn poststate_requires_exact_payload_and_parked_rent() {
        let fixture = Fixture::new();
        let mut payload_runner = FakeRunner::new(&fixture);
        payload_runner.after_live[4] ^= 1;
        let payload_error = execute_with_runner(&fixture.args, &mut payload_runner)
            .expect_err("wrong payload must refuse");
        assert!(
            payload_error
                .to_string()
                .contains("post-Upgrade ProgramData payload")
        );

        let other = Fixture::new();
        let mut rent_runner = FakeRunner::new(&other);
        rent_runner.after_programdata_lamports += 1;
        let rent_error = execute_with_runner(&other.args, &mut rent_runner)
            .expect_err("ProgramData rent movement must refuse");
        assert!(
            rent_error
                .to_string()
                .contains("ProgramData lamports moved")
        );
    }

    #[test]
    fn prepared_receipt_stops_if_a_crashed_submission_may_have_moved_chain() {
        let fixture = Fixture::new();
        let mut interrupted = FakeRunner::new(&fixture);
        interrupted.deploy_success = false;
        let first = execute_with_runner(&fixture.args, &mut interrupted)
            .expect_err("synthetic deploy interruption");
        assert!(first.to_string().contains("synthetic deploy interruption"));
        let receipt = load_receipt(&fixture.args.receipt_path)
            .expect("receipt read")
            .expect("prepared receipt");
        assert_eq!(receipt.phase, ReceiptPhaseV1::Prepared);

        let mut ambiguous = FakeRunner::new(&fixture);
        ambiguous.deployed = true;
        let second = execute_with_runner(&fixture.args, &mut ambiguous)
            .expect_err("moved chain behind prepared receipt must stop");
        assert!(
            second
                .to_string()
                .contains("submission outcome is ambiguous")
        );
        assert!(
            ambiguous
                .calls
                .iter()
                .all(|call| call.get(1).map(String::as_str) != Some("deploy"))
        );
    }

    #[test]
    fn dump_must_be_checked_raw_or_checked_live_bytes() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.dump = b"hostile deployed bytes".to_vec();
        let error =
            execute_with_runner(&fixture.args, &mut runner).expect_err("hostile dump must refuse");
        assert!(error.to_string().contains("deployed-byte dump is neither"));
    }

    fn configure_extension_runner(
        fixture: &Fixture,
        runner: &mut FakeRunner,
        additional_bytes: u64,
    ) {
        let rent_delta = additional_bytes.checked_mul(6_960).expect("rent delta");
        runner.before_wallet = 2_000_000;
        runner.after_wallet = runner
            .before_wallet
            .checked_sub(rent_delta)
            .and_then(|balance| balance.checked_sub(5_000))
            .expect("extension payer balance");
        runner.after_programdata_lamports = runner
            .before_programdata_lamports
            .checked_add(rent_delta)
            .expect("extension ProgramData balance");
        runner.after_live = fixture.before_live.clone();
        runner.after_live.extend(std::iter::repeat_n(
            0,
            usize::try_from(additional_bytes).expect("extension host width"),
        ));
        runner.after_slot = 94;
    }

    #[test]
    fn extension_is_separate_slot_advancing_act_with_exact_rent_and_fee_arithmetic() {
        let fixture = Fixture::new();
        let args = fixture.extension_args(128);
        let mut runner = FakeRunner::new(&fixture);
        configure_extension_runner(&fixture, &mut runner, 128);
        let receipt =
            execute_extension_with_runner(&args, &mut runner).expect("checked extension completes");
        assert_eq!(receipt.phase, ReceiptPhaseV1::Complete);
        assert_eq!(receipt.before.deployment_slot, 91);
        assert_eq!(receipt.after.as_ref().expect("after").deployment_slot, 94);
        assert_eq!(
            receipt.transaction_signature,
            Some(Signature::from([8_u8; 64]).to_string())
        );
        assert!(receipt.finalized_transaction.is_some());
        assert_eq!(
            receipt.arithmetic,
            Some(ExtensionArithmeticV1 {
                wallet_before_lamports: 2_000_000,
                wallet_after_lamports: 1_104_120,
                wallet_spend_lamports: 895_880,
                rent_top_up_lamports: 890_880,
                observed_fee_and_cli_cost_lamports: 5_000,
                programdata_before_lamports: 77_000,
                programdata_after_lamports: 967_880,
                programdata_delta_lamports: 890_880,
                programdata_before_bytes: 57,
                programdata_after_bytes: 185,
                extension_additional_bytes: 128,
            })
        );
        assert!(runner.calls.iter().any(|call| {
            call.first().map(String::as_str) == Some("program")
                && call.get(1).map(String::as_str) == Some("extend")
        }));
    }

    #[test]
    fn extension_refuses_fit_candidate_and_underfunded_payer() {
        let fit = Fixture::new();
        let fit_args = fit.extension_args(0);
        let mut fit_runner = FakeRunner::new(&fit);
        let error = execute_extension_with_runner(&fit_args, &mut fit_runner)
            .expect_err("zero extension must refuse");
        assert!(error.to_string().contains("already fits"));
        assert!(fit_runner.calls.is_empty());

        let underfunded = Fixture::new();
        let underfunded_args = underfunded.extension_args(128);
        let mut underfunded_runner = FakeRunner::new(&underfunded);
        configure_extension_runner(&underfunded, &mut underfunded_runner, 128);
        underfunded_runner.before_wallet = 1_890_879;
        let error = execute_extension_with_runner(&underfunded_args, &mut underfunded_runner)
            .expect_err("underfunded extension must refuse");
        assert!(error.to_string().contains("requires at least 1890880"));
        assert!(
            underfunded_runner
                .calls
                .iter()
                .all(|call| call.get(1).map(String::as_str) != Some("extend"))
        );
    }

    #[test]
    fn extension_refuses_unchanged_slot_partial_space_and_transaction_arithmetic() {
        let unchanged = Fixture::new();
        let unchanged_args = unchanged.extension_args(128);
        let mut unchanged_runner = FakeRunner::new(&unchanged);
        configure_extension_runner(&unchanged, &mut unchanged_runner, 128);
        unchanged_runner.after_slot = unchanged_runner.before_slot;
        let error = execute_extension_with_runner(&unchanged_args, &mut unchanged_runner)
            .expect_err("unchanged ExtendProgram slot must refuse");
        assert!(
            error
                .to_string()
                .contains("extension deployment slot did not advance")
        );

        let partial = Fixture::new();
        let partial_args = partial.extension_args(128);
        let mut partial_runner = FakeRunner::new(&partial);
        configure_extension_runner(&partial, &mut partial_runner, 64);
        // The chain received only half the plan, but its lamport delta still
        // claims the full plan. Space catches the partial before attribution.
        partial_runner.after_programdata_lamports = 967_880;
        partial_runner.after_wallet = 1_104_120;
        let error = execute_extension_with_runner(&partial_args, &mut partial_runner)
            .expect_err("partial extension must refuse");
        assert!(
            error
                .to_string()
                .contains("partial or substituted extension")
        );

        let arithmetic = Fixture::new();
        let arithmetic_args = arithmetic.extension_args(128);
        let mut arithmetic_runner = FakeRunner::new(&arithmetic);
        configure_extension_runner(&arithmetic, &mut arithmetic_runner, 128);
        arithmetic_runner.extension_fee_lamports = 6_000;
        let error = execute_extension_with_runner(&arithmetic_args, &mut arithmetic_runner)
            .expect_err("transaction arithmetic mismatch must refuse");
        assert!(
            error
                .to_string()
                .contains("payer spend is not exact ProgramData rent delta plus fee")
        );
    }

    #[test]
    fn submitted_extension_never_replays_and_complete_receipt_requires_transaction() {
        let partial = Fixture::new();
        let partial_args = partial.extension_args(128);
        let mut first = FakeRunner::new(&partial);
        configure_extension_runner(&partial, &mut first, 64);
        first.after_programdata_lamports = 967_880;
        first.after_wallet = 1_104_120;
        let _ = execute_extension_with_runner(&partial_args, &mut first)
            .expect_err("partial first extension");
        let mut resume = FakeRunner::new(&partial);
        configure_extension_runner(&partial, &mut resume, 64);
        resume.deployed = true;
        resume.after_programdata_lamports = 967_880;
        resume.after_wallet = 1_104_120;
        let _ = execute_extension_with_runner(&partial_args, &mut resume)
            .expect_err("submitted partial remains stopped");
        assert!(
            resume
                .calls
                .iter()
                .all(|call| call.get(1).map(String::as_str) != Some("extend"))
        );

        let complete = Fixture::new();
        let complete_args = complete.extension_args(128);
        let mut complete_runner = FakeRunner::new(&complete);
        configure_extension_runner(&complete, &mut complete_runner, 128);
        let accepted = execute_extension_with_runner(&complete_args, &mut complete_runner)
            .expect("complete extension");
        let mut replay_runner = FakeRunner::new(&complete);
        configure_extension_runner(&complete, &mut replay_runner, 128);
        replay_runner.deployed = true;
        let replay = execute_extension_with_runner(&complete_args, &mut replay_runner)
            .expect("complete receipt replay");
        assert_eq!(replay, accepted);
        assert!(replay_runner.calls.iter().all(|call| {
            call.first().map(String::as_str) != Some("address")
                && call.get(1).map(String::as_str) != Some("extend")
        }));

        let mut hostile = serde_json::to_value(&accepted).expect("receipt value");
        hostile
            .as_object_mut()
            .expect("receipt object")
            .remove("finalized_transaction");
        fs::write(
            &complete_args.receipt_path,
            serde_json::to_vec_pretty(&hostile).expect("hostile receipt JSON"),
        )
        .expect("write hostile receipt");
        let mut hostile_runner = FakeRunner::new(&complete);
        let error = execute_extension_with_runner(&complete_args, &mut hostile_runner)
            .expect_err("complete receipt without transaction must refuse");
        assert!(error.to_string().contains("receipt SHA-256"));
        assert!(hostile_runner.calls.is_empty());
    }

    #[test]
    fn command_parser_refuses_loopback_and_requires_one_mode() {
        let no_execute = parse_args(Vec::new()).expect_err("execute is required");
        assert!(
            no_execute
                .to_string()
                .contains("exactly one of --preflight")
        );

        let fixture = Fixture::new();
        let arguments = vec![
            "--rpc-url".into(),
            "http://127.0.0.1:20890/".into(),
            DEVNET_ACKNOWLEDGMENT_FLAG.into(),
            DEVNET_GENESIS_HASH.into(),
            "--execute".into(),
        ];
        let loopback = parse_args(arguments).expect_err("loopback Upgrade must refuse");
        assert!(
            loopback
                .to_string()
                .contains("given for the loopback origin")
        );
        drop(fixture);
    }
}
