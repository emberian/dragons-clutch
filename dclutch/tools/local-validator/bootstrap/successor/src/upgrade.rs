//! One-role, permanent-program-id Upgrade orchestration for Solana devnet.
//!
//! Decision 0012 keeps the seven durable devnet program ids mutable.  That is
//! intentionally different from `campaign.rs`: a campaign publishes and
//! activates facts *after* deployment, while this module is the narrow seam
//! allowed to update one existing Loader-v3 Program/ProgramData pair.
//!
//! The Solana CLI is behind [`CliRunner`].  Tests therefore exercise every
//! destructive boundary with a fake; they never open a keypair file, contact a
//! cluster, or spawn `solana`. Production uses the CLI only for a separately
//! journaled, persistent Buffer upload. It constructs the final Upgrade or
//! Extend message itself, fsyncs that message before reading signing keys,
//! fsyncs the verified signed packet before one `sendTransaction` attempt, and
//! resumes by signature polling without resending an unexpired packet.

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::process::CommandExt as _,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str::FromStr as _,
    thread,
    time::Duration,
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
use dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V7;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use solana_loader_v3_interface::instruction::{
    UpgradeableLoaderInstruction, extend_program_checked, upgrade,
};
use solana_program::rent::Rent;
use solana_sdk::{
    hash::Hash,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer as _, read_keypair_file},
    transaction::Transaction,
};
use solana_sdk_ids::{bpf_loader_upgradeable, sysvar};
use solana_system_interface::instruction::SystemInstruction;

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

const SCHEMA: &str = "dclutch-devnet-permanent-id-upgrade-receipt-v6";
const PREFLIGHT_SCHEMA: &str = "dclutch-devnet-permanent-id-upgrade-preflight-v1";
const CHECKED_GATE_SCHEMA: &str = "dclutch-checked-upgrade-gate-v1";
const BASELINE_SCHEMA: &str = "dclutch-devnet-upgrade-baseline-v1";
const EXTENSION_SCHEMA: &str = "dclutch-devnet-programdata-extension-receipt-v3";
const SET_JOURNAL_SCHEMA: &str = "dclutch-devnet-deployment-set-journal-v2";
const SET_AUDIT_SCHEMA: &str = "dclutch-devnet-deployment-set-audit-v2";
const CARRY_FORWARD_SNAPSHOT_SCHEMA: &str = "dclutch-carry-forward-rpc-snapshot-v1";
pub(crate) const CHECKED_SET_PREPARE_SCHEMA: &str = "dclutch-checked-deployment-set-release-pin-v2";
pub(crate) const SEMANTIC_DERIVATION_V1: &str =
    "source-semantic-release-v1+compiled-direct-release-v1+resolution-controller-release-v6";
const TARGET_ACK_FLAG: &str = "--i-accept-upgrade";
const EXCLUSIVE_PAYER_ACK_FLAG: &str = "--i-kept-fee-payer-exclusive";
const OPERATION_ACCOUNTING_SCOPE_V1: &str = "exclusive-payer-window-observed-net-v1";
const OPERATION_COST_ATTRIBUTION_V1: &str =
    "buffer-upload-transactions-and-fees-exact;final-upgrade-transaction-fee-exact";
const EXTENSION_ACK_FLAG: &str = "--i-accept-extension";
const DEPLOYMENT_SET_JOURNAL_FLAG: &str = "--deployment-set-journal";
const BUFFER_PUBKEY_FLAG: &str = "--buffer-pubkey";
const BUFFER_KEYPAIR_FLAG: &str = "--buffer-keypair";
const BUFFER_METADATA_BYTES: usize = 37;
const BUFFER_WRITER_LEASE_SCHEMA: &str = "dclutch-buffer-writer-lease-v2";
const BUFFER_WRITER_PERMIT_SCHEMA: &str = "dclutch-buffer-writer-permit-v1";
/// A conservative finalized-height horizon for the one-attempt
/// `write-buffer` subprocess. Agave's recent blockhash lifetime is shorter
/// than this bound. A retry is still refused unless the exact leased process
/// identity is gone and bounded Buffer history accounts for the old attempt.
const BUFFER_WRITE_EXPIRY_BLOCKS: u64 = 512;
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
pub(crate) const CHECKED_ROLE_ORDER_V1: [&str; 7] = [
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
pub(crate) const PERMANENT_DEVNET_UPGRADE_TARGETS_V1: &[(&str, &str, &str)] = &[
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
    buffer_pubkey: Pubkey,
    buffer_keypair: PathBuf,
    deployment_set_journal_path: PathBuf,
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
    deployment_set_journal_path: PathBuf,
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
    checked_build_manifest_path: PathBuf,
    checked_build_manifest_sha256: String,
    checked_build_manifest: Vec<u8>,
}

/// Key-free projection of one checked-release role for a localhost mutable
/// substrate.  The local launcher consumes the same gate validator as Upgrade;
/// it does not grow a second, weaker interpretation of the thirteen-link gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedLocalGateRoleV1 {
    pub(crate) gate_sha256: String,
    pub(crate) source_revision: String,
    pub(crate) source_tree_sha256: String,
    pub(crate) solana_cli_version: String,
    pub(crate) raw_elf_sha256: String,
    pub(crate) checked_build_manifest_path: PathBuf,
    pub(crate) checked_build_manifest_sha256: String,
    pub(crate) checked_build_manifest: Vec<u8>,
}

struct CheckedReleaseGateSelectionV1<'a> {
    checked_release_gate_path: &'a Path,
    expected_checked_release_gate_sha256: &'a str,
    expected_source_revision: &'a str,
    expected_source_tree_sha256: &'a str,
    role: &'a str,
    elf_path: &'a Path,
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
    BufferWriteArmed,
    BufferReady,
    MessagePrepared,
    SignedNotSubmitted,
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
struct ExpiredLoaderPacketV1 {
    unsigned_message_base64: String,
    unsigned_message_sha256: String,
    recent_blockhash: String,
    last_valid_block_height: u64,
    signed_packet_base64: String,
    signed_packet_sha256: String,
    transaction_signature: String,
    expiry_observed_finalized_block_height: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BufferWriterLeaseV1 {
    schema: String,
    operation_id: String,
    attempt_ordinal: u64,
    pid: u32,
    process_group_id: u32,
    process_start_token: String,
    process_nonce: String,
    command_sha256: String,
    lease_path: String,
    permit_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BufferWriteAttemptV1 {
    lease: BufferWriterLeaseV1,
    armed_finalized_block_height: u64,
    expiry_finalized_block_height: u64,
    exit_observed_finalized_block_height: Option<u64>,
    exit_disposition: Option<String>,
}

struct BufferWriterQueryV1<'a> {
    operation_id: &'a str,
    attempt_ordinal: u64,
    command_arguments: &'a [String],
    command_sha256: &'a str,
    lease_path: &'a Path,
    permit_path: &'a Path,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BufferWriterStatusV1 {
    AliveStopped,
    AliveRunning,
    Exited,
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
    deployment_set_journal_sha256: String,
    baseline_sha256: String,
    baseline_context_slot: u64,
    solana_cli_version: String,
    extension_additional_bytes: u64,
    target_rent_exempt_minimum_lamports: u64,
    expected_rent_top_up_lamports: u64,
    before_context_slot: u64,
    before: LoaderObservationV1,
    wallet_before_lamports: u64,
    expired_packets: Vec<ExpiredLoaderPacketV1>,
    unsigned_message_base64: Option<String>,
    unsigned_message_sha256: Option<String>,
    recent_blockhash: Option<String>,
    last_valid_block_height: Option<u64>,
    signed_packet_base64: Option<String>,
    signed_packet_sha256: Option<String>,
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
    deployment_set_journal_sha256: String,
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
    buffer_pubkey: String,
    buffer_keypair_path: String,
    buffer_data_bytes: u64,
    buffer_rent_exempt_lamports: u64,
    buffer_write_armed_block_height: Option<u64>,
    buffer_write_expiry_block_height: Option<u64>,
    buffer_write_cli_output: Option<Value>,
    buffer_ready_context_slot: Option<u64>,
    buffer_ready_account_sha256: Option<String>,
    buffer_ready_lamports: Option<u64>,
    buffer_upload_fee_lamports: Option<u64>,
    buffer_upload_transactions: Option<Vec<Value>>,
    buffer_upload_transactions_sha256: Option<String>,
    buffer_upload_wallet_after_lamports: Option<u64>,
    buffer_write_attempts: Vec<BufferWriteAttemptV1>,
    expired_packets: Vec<ExpiredLoaderPacketV1>,
    unsigned_message_base64: Option<String>,
    unsigned_message_sha256: Option<String>,
    recent_blockhash: Option<String>,
    last_valid_block_height: Option<u64>,
    signed_packet_base64: Option<String>,
    signed_packet_sha256: Option<String>,
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
struct ReceiptTransitionLockV1 {
    schema: String,
    owner_pid: u32,
    owner_start_token: String,
    expected_receipt_sha256: Option<String>,
    target_receipt_sha256: String,
    pending_file_name: String,
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
    buffer: Pubkey,
    buffer_lamports: u64,
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
    signature: &'a str,
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum LoaderActionV1 {
    Extend { additional_bytes: u32 },
    Upgrade { buffer: Pubkey },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum JournaledSignatureStatusV1 {
    NotFound,
    Pending,
    FinalizedSuccess,
    FinalizedFailure(String),
}

struct LoaderActionQueryV1<'a> {
    origin: &'a ClusterOriginV1,
    program_id: Pubkey,
    programdata_id: Pubkey,
    authority: Pubkey,
    payer: Pubkey,
    authority_keypair: &'a Path,
    payer_keypair: &'a Path,
    action: LoaderActionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UnsignedLoaderActionV1 {
    message_base64: String,
    message_sha256: String,
    recent_blockhash: String,
    last_valid_block_height: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedLoaderActionV1 {
    packet_base64: String,
    packet_sha256: String,
    signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BufferObservationV1 {
    context_slot: u64,
    lamports: u64,
    data_bytes: u64,
    account_sha256: String,
    authority: String,
    payload_sha256: String,
}

#[derive(Clone, Copy)]
struct BufferUploadQueryV1<'a> {
    origin: &'a ClusterOriginV1,
    buffer: Pubkey,
    authority: Pubkey,
    payer: Pubkey,
    raw_elf: &'a [u8],
    minimum_slot: u64,
    expected_rent_lamports: u64,
    wallet_before_lamports: u64,
    wallet_after_lamports: u64,
}

#[derive(Clone, Debug)]
struct BufferUploadEvidenceV1 {
    transactions: Vec<Value>,
    transactions_sha256: String,
    fee_lamports: u64,
}

trait CliRunner {
    fn run(&mut self, arguments: &[String]) -> Result<CliOutput>;

    fn enforces_fresh_deployment_set_boundary(&self) -> bool {
        false
    }

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

    fn prepare_loader_action(
        &mut self,
        _query: &LoaderActionQueryV1<'_>,
    ) -> Result<UnsignedLoaderActionV1> {
        Err(Error::new(
            "CLI runner does not provide canonical Loader transaction planning",
        ))
    }

    fn sign_loader_action(
        &mut self,
        _query: &LoaderActionQueryV1<'_>,
        _unsigned: &UnsignedLoaderActionV1,
    ) -> Result<SignedLoaderActionV1> {
        Err(Error::new(
            "CLI runner does not provide canonical Loader packet signing",
        ))
    }

    fn send_loader_action(
        &mut self,
        _origin: &ClusterOriginV1,
        _signed: &SignedLoaderActionV1,
    ) -> Result<String> {
        Err(Error::new(
            "CLI runner does not provide one-shot Loader packet submission",
        ))
    }

    fn finalized_block_height(&mut self, _origin: &ClusterOriginV1) -> Result<u64> {
        Err(Error::new(
            "CLI runner does not provide finalized block height",
        ))
    }

    fn journaled_signature_status(
        &mut self,
        _origin: &ClusterOriginV1,
        _signature: &str,
    ) -> Result<JournaledSignatureStatusV1> {
        Err(Error::new(
            "CLI runner does not provide exact journaled signature status",
        ))
    }

    fn read_buffer(
        &mut self,
        _origin: &ClusterOriginV1,
        _buffer: Pubkey,
        _authority: Pubkey,
        _minimum_context_slot: u64,
    ) -> Result<Option<BufferObservationV1>> {
        Err(Error::new(
            "CLI runner does not provide finalized Buffer observation",
        ))
    }

    fn minimum_balance_for_rent_exemption(
        &mut self,
        _origin: &ClusterOriginV1,
        _data_bytes: u64,
    ) -> Result<u64> {
        Err(Error::new(
            "CLI runner does not provide finalized rent-exemption quote",
        ))
    }

    fn authenticate_buffer_upload(
        &mut self,
        _query: &BufferUploadQueryV1<'_>,
    ) -> Result<BufferUploadEvidenceV1> {
        Err(Error::new(
            "CLI runner does not provide bounded Buffer upload attribution",
        ))
    }

    fn start_buffer_writer(
        &mut self,
        _query: &BufferWriterQueryV1<'_>,
    ) -> Result<BufferWriterLeaseV1> {
        Err(Error::new(
            "CLI runner does not provide a leased Buffer writer",
        ))
    }

    fn buffer_writer_status(
        &mut self,
        _lease: &BufferWriterLeaseV1,
    ) -> Result<BufferWriterStatusV1> {
        Err(Error::new(
            "CLI runner does not provide Buffer writer liveness",
        ))
    }

    fn continue_buffer_writer(
        &mut self,
        _query: &BufferWriterQueryV1<'_>,
        _lease: &BufferWriterLeaseV1,
    ) -> Result<CliOutput> {
        Err(Error::new(
            "CLI runner does not provide leased Buffer continuation",
        ))
    }
}

struct SystemCliRunner {
    executable: PathBuf,
    buffer_child: Option<Child>,
}

fn load_buffer_writer_lease(query: &BufferWriterQueryV1<'_>) -> Result<BufferWriterLeaseV1> {
    let bytes = read_regular_reference(query.lease_path, "Buffer writer lease")?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("Buffer writer lease is not UTF-8"))?
        .trim_end();
    let fields = text.split('\t').collect::<Vec<_>>();
    if fields.len() != 8
        || fields[0] != BUFFER_WRITER_LEASE_SCHEMA
        || fields[1] != query.operation_id
        || fields[2] != query.attempt_ordinal.to_string()
        || fields[7] != query.command_sha256
    {
        return Err(Error::new(
            "Buffer writer lease differs from the exact operation, attempt, or command",
        ));
    }
    let pid = fields[3]
        .parse::<u32>()
        .map_err(|_| Error::new("Buffer writer lease PID is invalid"))?;
    let process_group_id = fields[4]
        .parse::<u32>()
        .map_err(|_| Error::new("Buffer writer lease process group is invalid"))?;
    if pid == 0 || process_group_id != pid || fields[5].is_empty() {
        return Err(Error::new(
            "Buffer writer lease lacks its exact private process group/start token",
        ));
    }
    require_digest(fields[6], "Buffer writer process nonce")?;
    require_digest(fields[7], "Buffer writer command SHA-256")?;
    Ok(BufferWriterLeaseV1 {
        schema: BUFFER_WRITER_LEASE_SCHEMA.into(),
        operation_id: query.operation_id.into(),
        attempt_ordinal: query.attempt_ordinal,
        pid,
        process_group_id,
        process_start_token: fields[5].into(),
        process_nonce: fields[6].into(),
        command_sha256: fields[7].into(),
        lease_path: query.lease_path.to_string_lossy().into_owned(),
        permit_path: query.permit_path.to_string_lossy().into_owned(),
    })
}

fn process_group_id(pid: u32) -> Result<Option<u32>> {
    let output = Command::new("/bin/ps")
        .args(["-o", "pgid=", "-p", &pid.to_string()])
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(text.parse::<u32>().map_err(|_| {
        Error::new("process group query returned invalid integer")
    })?))
}

fn buffer_writer_permit_bytes(lease: &BufferWriterLeaseV1) -> Vec<u8> {
    format!(
        "{BUFFER_WRITER_PERMIT_SCHEMA}\t{}\t{}\t{}\t{}\t{}\t{}",
        lease.operation_id,
        lease.attempt_ordinal,
        lease.pid,
        lease.process_start_token,
        lease.process_nonce,
        lease.command_sha256,
    )
    .into_bytes()
}

fn buffer_writer_permit_published(lease: &BufferWriterLeaseV1) -> Result<bool> {
    let path = Path::new(&lease.permit_path);
    match fs::symlink_metadata(path) {
        Ok(_) => {
            let bytes = read_regular_reference(path, "Buffer writer permit")?;
            if bytes != buffer_writer_permit_bytes(lease) {
                return Err(Error::new(
                    "Buffer writer permit does not bind the exact durable lease",
                ));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn buffer_writer_process_nonce_is_live(lease: &BufferWriterLeaseV1) -> Result<bool> {
    let output = Command::new("/bin/ps")
        .args(["eww", "-p", &lease.pid.to_string(), "-o", "command="])
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .output()?;
    if !output.status.success() {
        return Ok(false);
    }
    let marker = format!("DCLUTCH_BUFFER_WRITER_ID={}", lease.process_nonce);
    // The marker is deliberately present in both argv and the environment.
    // Linux `ps eww` exposes the latter, while macOS `ps` accepts the same
    // spelling but returns only argv for a PID-scoped query. Requiring the
    // random marker in either platform's command projection preserves the
    // exact-process check without treating a matching PID/start/group as
    // sufficient identity.
    Ok(String::from_utf8_lossy(&output.stdout).contains(&marker))
}

impl CliRunner for SystemCliRunner {
    fn enforces_fresh_deployment_set_boundary(&self) -> bool {
        true
    }

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

    fn prepare_loader_action(
        &mut self,
        query: &LoaderActionQueryV1<'_>,
    ) -> Result<UnsignedLoaderActionV1> {
        prepare_loader_action_via_rpc(query)
    }

    fn sign_loader_action(
        &mut self,
        query: &LoaderActionQueryV1<'_>,
        unsigned: &UnsignedLoaderActionV1,
    ) -> Result<SignedLoaderActionV1> {
        sign_loader_action_from_paths(query, unsigned)
    }

    fn send_loader_action(
        &mut self,
        origin: &ClusterOriginV1,
        signed: &SignedLoaderActionV1,
    ) -> Result<String> {
        send_loader_action_via_rpc(origin, signed)
    }

    fn finalized_block_height(&mut self, origin: &ClusterOriginV1) -> Result<u64> {
        let mut rpc = Rpc::connect_cluster(origin, WritePolicyV1::ReadsOnly)?;
        rpc.call(
            "getBlockHeight",
            &serde_json::json!([{"commitment":"finalized"}]),
        )?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not u64"))
    }

    fn journaled_signature_status(
        &mut self,
        origin: &ClusterOriginV1,
        signature: &str,
    ) -> Result<JournaledSignatureStatusV1> {
        journaled_signature_status_via_rpc(origin, signature)
    }

    fn read_buffer(
        &mut self,
        origin: &ClusterOriginV1,
        buffer: Pubkey,
        authority: Pubkey,
        minimum_context_slot: u64,
    ) -> Result<Option<BufferObservationV1>> {
        read_buffer_via_rpc(origin, buffer, authority, minimum_context_slot)
    }

    fn minimum_balance_for_rent_exemption(
        &mut self,
        origin: &ClusterOriginV1,
        data_bytes: u64,
    ) -> Result<u64> {
        let mut rpc = Rpc::connect_cluster(origin, WritePolicyV1::ReadsOnly)?;
        rpc.call(
            "getMinimumBalanceForRentExemption",
            &serde_json::json!([data_bytes, {"commitment":"finalized"}]),
        )?
        .as_u64()
        .ok_or_else(|| Error::new("rent-exemption quote was not u64"))
    }

    fn authenticate_buffer_upload(
        &mut self,
        query: &BufferUploadQueryV1<'_>,
    ) -> Result<BufferUploadEvidenceV1> {
        authenticate_buffer_upload_via_rpc(query)
    }

    fn start_buffer_writer(
        &mut self,
        query: &BufferWriterQueryV1<'_>,
    ) -> Result<BufferWriterLeaseV1> {
        match fs::symlink_metadata(query.lease_path) {
            Ok(_) => {
                let lease = load_buffer_writer_lease(query)?;
                match fs::symlink_metadata(query.permit_path) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return Ok(lease);
                    }
                    Ok(_) => {
                        return Err(Error::new(
                            "unbound existing Buffer writer lease already has a premature permit",
                        ));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        match fs::symlink_metadata(query.permit_path) {
            Ok(_) => {
                return Err(Error::new(
                    "fresh Buffer writer attempt found a pre-existing permit; no subprocess was started",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let parent = query
            .lease_path
            .parent()
            .ok_or_else(|| Error::new("Buffer writer lease omitted parent"))?;
        if !parent.is_dir() || query.permit_path.parent() != Some(parent) {
            return Err(Error::new(
                "Buffer writer lease and permit must share one existing evidence directory",
            ));
        }
        let script = r#"set -eu
lease=$1
operation=$2
attempt=$3
command_sha=$4
permit=$5
nonce=$6
identity_marker=$7
[ "$identity_marker" = "DCLUTCH_BUFFER_WRITER_ID=$nonce" ]
shift 7
export LC_ALL=C TZ=UTC DCLUTCH_BUFFER_WRITER_ID="$nonce"
pid=$$
pgid=$(/bin/ps -o pgid= -p "$pid" | tr -d ' ')
started=$(/bin/ps -o lstart= -p "$pid" | sed 's/^ *//;s/ *$//')
umask 077
set -C
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' 'dclutch-buffer-writer-lease-v2' "$operation" "$attempt" "$pid" "$pgid" "$started" "$nonce" "$command_sha" > "$lease"
/bin/sync
while [ ! -f "$permit" ]; do sleep 0.05; done
[ ! -L "$permit" ]
actual=$(cat "$permit")
expected=$(printf '%s\t%s\t%s\t%s\t%s\t%s\t%s' 'dclutch-buffer-writer-permit-v1' "$operation" "$attempt" "$pid" "$started" "$nonce" "$command_sha")
[ "$actual" = "$expected" ]
exec "$@"
"#;
        let process_nonce = digest(&Keypair::new().to_bytes());
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(script)
            .arg("dclutch-buffer-writer")
            .arg(query.lease_path)
            .arg(query.operation_id)
            .arg(query.attempt_ordinal.to_string())
            .arg(query.command_sha256)
            .arg(query.permit_path)
            .arg(&process_nonce)
            .arg(format!("DCLUTCH_BUFFER_WRITER_ID={process_nonce}"))
            .arg(&self.executable)
            .args(query.command_arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("LC_ALL", "C")
            .env("TZ", "UTC")
            .env("DCLUTCH_BUFFER_WRITER_ID", &process_nonce)
            .process_group(0);
        let child = command.spawn()?;
        self.buffer_child = Some(child);
        for _ in 0..500 {
            if query.lease_path.exists() {
                let lease = load_buffer_writer_lease(query)?;
                sync_parent_directory(parent)?;
                return Ok(lease);
            }
            if self
                .buffer_child
                .as_mut()
                .expect("stored Buffer child")
                .try_wait()?
                .is_some()
            {
                return Err(Error::new(
                    "Buffer writer supervisor exited before publishing its durable lease",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err(Error::new(
            "Buffer writer supervisor did not publish its durable lease within five seconds",
        ))
    }

    fn buffer_writer_status(
        &mut self,
        lease: &BufferWriterLeaseV1,
    ) -> Result<BufferWriterStatusV1> {
        if process_start_token(lease.pid)?.as_deref() != Some(lease.process_start_token.as_str()) {
            return Ok(BufferWriterStatusV1::Exited);
        }
        if process_group_id(lease.pid)? != Some(lease.process_group_id) {
            return Err(Error::new(
                "Buffer writer PID still exists under a substituted process group; it will not be signaled",
            ));
        }
        if !buffer_writer_process_nonce_is_live(lease)? {
            return Err(Error::new(
                "Buffer writer PID/start/group exists without its exact random process identity; it will not be attached or signaled",
            ));
        }
        Ok(if buffer_writer_permit_published(lease)? {
            BufferWriterStatusV1::AliveRunning
        } else {
            BufferWriterStatusV1::AliveStopped
        })
    }

    fn continue_buffer_writer(
        &mut self,
        query: &BufferWriterQueryV1<'_>,
        lease: &BufferWriterLeaseV1,
    ) -> Result<CliOutput> {
        if self.buffer_writer_status(lease)? == BufferWriterStatusV1::Exited {
            return Err(Error::new(
                "leased Buffer writer exited before continuation",
            ));
        }
        if !buffer_writer_permit_published(lease)? {
            let mut permit = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(query.permit_path)?;
            permit.write_all(&buffer_writer_permit_bytes(lease))?;
            permit.sync_all()?;
            sync_parent_directory(
                query
                    .permit_path
                    .parent()
                    .ok_or_else(|| Error::new("Buffer writer permit omitted parent"))?,
            )?;
        }
        let Some(child) = self.buffer_child.take() else {
            return Err(Error::new(
                "exact leased Buffer writer is alive after operator restart; wait for that PID/process-group/start-token to exit before attaching or re-arming",
            ));
        };
        let output = child.wait_with_output()?;
        Ok(CliOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Parse and execute the versioned command with the real Solana CLI.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let args = parse_args(arguments)?;
    let mut runner = SystemCliRunner {
        executable: args.solana_cli.clone(),
        buffer_child: None,
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
        canonical_role_order: CHECKED_ROLE_ORDER_V1
            .iter()
            .map(|role| (*role).into())
            .collect(),
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
        buffer_child: None,
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
        buffer_child: None,
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
     --deployment-set-journal ABSOLUTE_JSON \\
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
     --buffer-pubkey PUBKEY --buffer-keypair ABSOLUTE_JSON \\
     --deployment-set-journal ABSOLUTE_JSON \\
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
     authority and persistent Buffer identity. The generated checked-release gate binds the exact source \
     commit/tree, all thirteen fresh compile logs, all thirteen zero frame reports, and every \
     release ELF. The selected ELF must be the gate's canonical regular file. The command refuses \
     handwritten acceptance, path escape, symlinks, missing links, and changed evidence. It pads \
     the checked raw ELF with zeros only to the separately captured baseline width. Execute first \
     uploads the exact ELF to the journaled Buffer with one CLI signing attempt, then authenticates \
     every bounded Buffer-history transaction, fee, payer delta, rent movement, offset, and byte \
     range. It constructs and journals the exact Upgrade message and signed packet before one \
     `sendTransaction` call with `maxRetries=0`. Restarts poll that signature; only a finalized \
     expiry with unchanged prestate can prepare a different blockhash and packet. The command \
     requires the deployment slot to advance, resolves the exact journaled finalized transaction, \
     proves the Buffer-rent refund, final fee, unchanged ProgramData rent, and operation-wide wallet \
     bridge. The legacy `unattributed_cli_net_cost_lamports` receipt field contains the exactly \
     attributed Buffer-upload fees. It dumps the deployed bytes, verifies \
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
            buffer_pubkey: Pubkey::default(),
            buffer_keypair: PathBuf::from("/dclutch-set-journal-never-reads-buffer"),
            deployment_set_journal_path: args.journal_path.clone(),
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
        let loaded_receipt = load_receipt(&role_args.receipt_path)?;
        let receipt_phase = loaded_receipt.as_ref().map(|receipt| receipt.phase.clone());
        if role.receipt.sha256.is_some() && loaded_receipt.is_none() {
            return Err(Error::new("pinned deployment-set receipt disappeared"));
        }
        if role.receipt.sha256.is_none()
            && loaded_receipt
                .as_ref()
                .is_some_and(|receipt| receipt.phase == ReceiptPhaseV1::Complete)
        {
            return Err(Error::new(format!(
                "complete deployment-set role {} must pin its exact receipt digest before the set can advance",
                role.role
            )));
        }
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
                Some(
                    ReceiptPhaseV1::BufferWriteArmed
                    | ReceiptPhaseV1::BufferReady
                    | ReceiptPhaseV1::MessagePrepared
                    | ReceiptPhaseV1::SignedNotSubmitted
                    | ReceiptPhaseV1::Submitted,
                ) => SetRoleStatusV1::Submitted,
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
                Some(
                    ReceiptPhaseV1::Prepared
                    | ReceiptPhaseV1::BufferWriteArmed
                    | ReceiptPhaseV1::BufferReady
                    | ReceiptPhaseV1::MessagePrepared
                    | ReceiptPhaseV1::SignedNotSubmitted
                    | ReceiptPhaseV1::Submitted,
                ) => "resume_exact_one_role_upgrade",
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
    let _ = unique_deployment_set_paths(&journal, &journal_path)?;
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
                if role.receipt.sha256.is_some() {
                    read_optional_reference(&role.receipt, &format!("{} receipt", role.role))?;
                } else {
                    let path = exact_reference_path(
                        &role.receipt.canonical_path,
                        &format!("{} in-flight receipt", role.role),
                    )?;
                    if read_existing_regular_receipt(
                        &path,
                        &format!("{} in-flight receipt", role.role),
                    )?
                    .is_none()
                    {
                        let parent = path
                            .parent()
                            .ok_or_else(|| Error::new("in-flight receipt path omitted a parent"))?;
                        if fs::canonicalize(parent)? != parent {
                            return Err(Error::new(
                                "in-flight receipt parent is not an exact canonical directory",
                            ));
                        }
                    }
                }
            }
        }
        read_optional_reference(&role.dump, &format!("{} dump", role.role))?;
    }
    Ok((journal, journal_sha256))
}

fn unique_deployment_set_paths(
    journal: &UpgradeSetJournalV1,
    journal_path: &Path,
) -> Result<BTreeSet<PathBuf>> {
    let mut paths = BTreeSet::new();
    if !paths.insert(journal_path.to_path_buf()) {
        return Err(Error::new("deployment-set journal path aliases itself"));
    }
    let mut insert = |path: &str, label: &str| -> Result<()> {
        let path = exact_reference_path(path, label)?;
        if !paths.insert(path) {
            return Err(Error::new(format!(
                "deployment-set evidence/output paths alias at {label}"
            )));
        }
        Ok(())
    };
    insert(
        &journal.checked_release_gate.canonical_path,
        "checked-release gate",
    )?;
    insert(
        &journal.infrastructure_carry_forward.canonical_path,
        "carry-forward snapshot",
    )?;
    for role in &journal.roles {
        if let Some(baseline) = &role.baseline {
            insert(&baseline.canonical_path, &format!("{} baseline", role.role))?;
        }
        insert(
            &role.receipt.canonical_path,
            &format!("{} receipt", role.role),
        )?;
        insert(&role.dump.canonical_path, &format!("{} dump", role.role))?;
    }
    Ok(paths)
}

fn deployment_set_plan_sha256(journal: &UpgradeSetJournalV1) -> Result<String> {
    let mut plan = journal.clone();
    for role in &mut plan.roles {
        if role.disposition == CheckedDeploymentDispositionV1::Upgrade {
            role.receipt.sha256 = None;
            role.dump.sha256 = None;
        }
    }
    Ok(digest(&serde_json::to_vec(&plan)?))
}

fn require_mutation_permit(
    args: &UpgradeArgsV1,
    expected_journal_sha256: Option<&str>,
    require_upgrade_receipt_path: bool,
    freshly_audited_raw_journal_sha256: Option<&str>,
) -> Result<String> {
    if matches!(args.role.as_str(), "registry" | "rent") {
        return Err(Error::new(
            "Registry and Rent are CarryForward and can never enter an Upgrade/Extend mutation path",
        ));
    }
    let path = exact_reference_path(
        args.deployment_set_journal_path
            .to_str()
            .ok_or_else(|| Error::new("deployment-set journal path is not UTF-8"))?,
        "deployment-set mutation journal",
    )?;
    let bytes = read_regular_reference(&path, "deployment-set mutation journal")?;
    if freshly_audited_raw_journal_sha256.is_some_and(|expected| digest(&bytes) != expected) {
        return Err(Error::new(
            "deployment-set journal changed after its fresh full mutation-boundary audit",
        ));
    }
    let journal: UpgradeSetJournalV1 = serde_json::from_slice(&bytes).map_err(|error| {
        Error::new(format!(
            "deployment-set mutation journal is not canonical {SET_JOURNAL_SCHEMA} JSON: {error}"
        ))
    })?;
    let plan_paths = unique_deployment_set_paths(&journal, &path)?;
    if !require_upgrade_receipt_path && plan_paths.contains(&args.receipt_path) {
        return Err(Error::new(
            "extension receipt path aliases deployment-set evidence or an Upgrade output path",
        ));
    }
    let journal_sha256 = deployment_set_plan_sha256(&journal)?;
    if expected_journal_sha256.is_some_and(|expected| expected != journal_sha256) {
        return Err(Error::new(
            "deployment-set immutable plan changed after the mutation receipt was fsynced",
        ));
    }
    if journal.schema != SET_JOURNAL_SCHEMA
        || journal.devnet_genesis_hash != DEVNET_GENESIS_HASH
        || (require_upgrade_receipt_path
            && (journal.source_revision != args.expected_source_revision
                || journal.source_tree_sha256 != args.expected_source_tree_sha256
                || journal.checked_release_gate.sha256
                    != args.expected_checked_release_gate_sha256))
        || journal.solana_cli_version.trim().is_empty()
        || journal.retained_upgrade_authority != args.expected_upgrade_authority.to_string()
        || journal.fee_payer != args.fee_payer.to_string()
        || journal.roles.len() != PERMANENT_DEVNET_UPGRADE_TARGETS_V1.len()
    {
        return Err(Error::new(
            "deployment-set mutation journal differs from the exact devnet source, gate, authority, payer, or seven-role closure",
        ));
    }
    let mut first_incomplete = None;
    for (index, (role, (expected_role, expected_program, expected_programdata))) in journal
        .roles
        .iter()
        .zip(PERMANENT_DEVNET_UPGRADE_TARGETS_V1)
        .enumerate()
    {
        let expected_disposition = if index < 2 {
            CheckedDeploymentDispositionV1::CarryForward
        } else {
            CheckedDeploymentDispositionV1::Upgrade
        };
        if role.role != *expected_role
            || role.program_id != *expected_program
            || role.programdata_id != *expected_programdata
            || role.disposition != expected_disposition
        {
            return Err(Error::new(format!(
                "deployment-set mutation row {index} is not exact permanent {expected_role} with its canonical disposition"
            )));
        }
        if index < 2 {
            if role.receipt.sha256.is_some() || role.baseline.is_some() {
                return Err(Error::new(
                    "Registry and Rent are CarryForward and can never enter an Upgrade/Extend mutation path",
                ));
            }
            continue;
        }
        match &role.receipt.sha256 {
            Some(expected) if first_incomplete.is_none() => {
                let receipt_bytes = read_optional_reference(
                    &role.receipt,
                    &format!("{} prior Upgrade receipt", role.role),
                )?
                .ok_or_else(|| Error::new("pinned prior Upgrade receipt disappeared"))?;
                if digest(&receipt_bytes) != *expected {
                    return Err(Error::new("prior Upgrade receipt digest drifted"));
                }
                let receipt: UpgradeReceiptV1 = serde_json::from_slice(&receipt_bytes)?;
                if receipt.phase != ReceiptPhaseV1::Complete {
                    first_incomplete = Some(index);
                }
            }
            Some(_) => {
                return Err(Error::new(
                    "deployment-set journal pins a later Upgrade after an incomplete role",
                ));
            }
            None => {
                if first_incomplete.is_none() {
                    first_incomplete = Some(index);
                }
            }
        }
    }
    let index = first_incomplete.ok_or_else(|| {
        Error::new("deployment-set is already complete; no mutation is permitted")
    })?;
    let target = &journal.roles[index];
    if target.role != args.role
        || target.program_id != args.program_id.to_string()
        || target.programdata_id != args.programdata_id.to_string()
        || target.disposition != CheckedDeploymentDispositionV1::Upgrade
        || target
            .baseline
            .as_ref()
            .is_none_or(|baseline| Path::new(&baseline.canonical_path) != args.baseline_path)
        || (require_upgrade_receipt_path
            && Path::new(&target.receipt.canonical_path) != args.receipt_path)
        || (require_upgrade_receipt_path
            && Path::new(&target.dump.canonical_path) != args.dump_path)
    {
        return Err(Error::new(format!(
            "deployment-set next role is {} at ordinal {index}, not exact requested role/path {}:{}:{}; an extension receipt must not replace the fixed Upgrade receipt",
            target.role, args.role, args.program_id, args.programdata_id
        )));
    }
    Ok(journal_sha256)
}

fn authenticate_mutation_boundary(
    args: &UpgradeArgsV1,
    runner: &mut impl CliRunner,
) -> Result<Option<String>> {
    if !runner.enforces_fresh_deployment_set_boundary() {
        return Ok(None);
    }
    let report = audit_set_journal_with_runner(
        &UpgradeSetArgsV1 {
            origin: args.origin.clone(),
            journal_path: args.deployment_set_journal_path.clone(),
            solana_cli: args.solana_cli.clone(),
        },
        runner,
    )?;
    let next = report
        .next_role
        .ok_or_else(|| Error::new("deployment set is complete; no mutation is permitted"))?;
    if next.role != args.role
        || next.program_id != args.program_id.to_string()
        || next.programdata_id != args.programdata_id.to_string()
    {
        return Err(Error::new(
            "fresh deployment-set audit does not name this exact permanent row as the next mutation",
        ));
    }
    Ok(Some(report.journal_sha256))
}

fn authenticate_phase_mutation_boundary(
    args: &UpgradeArgsV1,
    runner: &mut impl CliRunner,
    phase: ReceiptPhaseV1,
) -> Result<Option<String>> {
    if matches!(phase, ReceiptPhaseV1::Submitted | ReceiptPhaseV1::Complete) {
        // Submitted is already past the sole send boundary. A fresh set audit
        // may legitimately see either its old prestate or its new poststate;
        // exact packet/status/account recovery owns that ambiguity. If a null
        // expired packet is archived, the continuation loop audits again
        // before it can prepare or send a replacement.
        Ok(None)
    } else {
        authenticate_mutation_boundary(args, runner)
    }
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

pub(crate) fn checked_semantic_release_id(role: &str, source_revision: &str) -> Result<String> {
    let fixed = match role {
        "trading" => Some(COMPILED_DIRECT_RELEASE_ID_V1),
        "resolution" => Some(RESOLUTION_CONTROLLER_RELEASE_ID_V7),
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
            buffer_pubkey: Pubkey::default(),
            buffer_keypair: PathBuf::from("/offline-buffer-not-read"),
            deployment_set_journal_path: journal_path.to_path_buf(),
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
        buffer_child: None,
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
                | BUFFER_PUBKEY_FLAG
                | BUFFER_KEYPAIR_FLAG
                | DEPLOYMENT_SET_JOURNAL_FLAG
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
    let buffer_pubkey = parse_pubkey(&take(BUFFER_PUBKEY_FLAG)?, BUFFER_PUBKEY_FLAG)?;
    if buffer_pubkey == Pubkey::default() {
        return Err(Error::new(
            "--buffer-pubkey must name a non-default persistent Buffer account",
        ));
    }
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
        buffer_pubkey,
        buffer_keypair: absolute(&take(BUFFER_KEYPAIR_FLAG)?, BUFFER_KEYPAIR_FLAG)?,
        deployment_set_journal_path: absolute(
            &take(DEPLOYMENT_SET_JOURNAL_FLAG)?,
            DEPLOYMENT_SET_JOURNAL_FLAG,
        )?,
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
                | DEPLOYMENT_SET_JOURNAL_FLAG
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
        deployment_set_journal_path: absolute(
            &take(DEPLOYMENT_SET_JOURNAL_FLAG)?,
            DEPLOYMENT_SET_JOURNAL_FLAG,
        )?,
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
        buffer_pubkey: Pubkey::default(),
        buffer_keypair: PathBuf::from("/extension-has-no-buffer"),
        deployment_set_journal_path: args.deployment_set_journal_path.clone(),
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
    match existing {
        None => {
            let audited_journal = authenticate_mutation_boundary(args, runner)?;
            let deployment_set_journal_sha256 =
                require_mutation_permit(args, None, true, audited_journal.as_deref())?;
            let before = read_snapshot(runner, args, baseline.context_slot)?;
            require_baseline_prestate(baseline, &before.loader)?;
            require_candidate_fits(&before, candidate_live)?;
            if before.loader.live_elf_sha256 == *live_elf_sha256 {
                return Err(Error::new(
                    "the current live payload already equals the candidate but no receipt binds \
                     an Upgrade; refusing replay ambiguity",
                ));
            }
            if runner
                .read_buffer(
                    &args.origin,
                    args.buffer_pubkey,
                    args.expected_upgrade_authority,
                    before.context_slot,
                )?
                .is_some()
            {
                return Err(Error::new(
                    "persistent Buffer already exists without this operation receipt; provenance is ambiguous",
                ));
            }
            let operation_id = operation_id(args, &gate.gate_sha256, &before);
            let buffer_data_bytes = u64::try_from(BUFFER_METADATA_BYTES + gate.raw_elf.len())
                .map_err(|_| Error::new("Buffer width does not fit u64"))?;
            let buffer_rent_exempt_lamports =
                runner.minimum_balance_for_rent_exemption(&args.origin, buffer_data_bytes)?;
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
                deployment_set_journal_sha256,
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
                buffer_pubkey: args.buffer_pubkey.to_string(),
                buffer_keypair_path: path_argument(&args.buffer_keypair, BUFFER_KEYPAIR_FLAG)?,
                buffer_data_bytes,
                buffer_rent_exempt_lamports,
                buffer_write_armed_block_height: None,
                buffer_write_expiry_block_height: None,
                buffer_write_cli_output: None,
                buffer_ready_context_slot: None,
                buffer_ready_account_sha256: None,
                buffer_ready_lamports: None,
                buffer_upload_fee_lamports: None,
                buffer_upload_transactions: None,
                buffer_upload_transactions_sha256: None,
                buffer_upload_wallet_after_lamports: None,
                buffer_write_attempts: Vec::new(),
                expired_packets: Vec::new(),
                unsigned_message_base64: None,
                unsigned_message_sha256: None,
                recent_blockhash: None,
                last_valid_block_height: None,
                signed_packet_base64: None,
                signed_packet_sha256: None,
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
            continue_upgrade(runner, args, gate, candidate_live, &mut receipt)
        }
        Some(mut receipt)
            if matches!(
                receipt.phase,
                ReceiptPhaseV1::Prepared
                    | ReceiptPhaseV1::BufferWriteArmed
                    | ReceiptPhaseV1::BufferReady
                    | ReceiptPhaseV1::MessagePrepared
                    | ReceiptPhaseV1::SignedNotSubmitted
                    | ReceiptPhaseV1::Submitted
            ) =>
        {
            let audited_journal =
                authenticate_phase_mutation_boundary(args, runner, receipt.phase.clone())?;
            require_mutation_permit(
                args,
                Some(&receipt.deployment_set_journal_sha256),
                true,
                audited_journal.as_deref(),
            )?;
            let current = read_snapshot(runner, args, receipt.before_context_slot)?;
            if receipt.phase != ReceiptPhaseV1::Submitted && current.loader != receipt.before {
                return Err(Error::new(
                    "Upgrade action has not been submitted but the exact Loader prestate moved",
                ));
            }
            if matches!(
                receipt.phase,
                ReceiptPhaseV1::Prepared | ReceiptPhaseV1::BufferWriteArmed
            ) && current.wallet_lamports > receipt.wallet_before_lamports
            {
                return Err(Error::new(
                    "Upgrade payer wallet increased during Buffer upload",
                ));
            }
            continue_upgrade(runner, args, gate, candidate_live, &mut receipt)
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
        Some(receipt) if receipt.phase == ReceiptPhaseV1::BufferWriteArmed => {
            if observation.loader != receipt.before
                || observation.wallet_lamports > receipt.wallet_before_lamports
            {
                return Err(Error::new(
                    "preflight found drift from the armed Buffer receipt prestate",
                ));
            }
            let lease = &receipt
                .buffer_write_attempts
                .last()
                .ok_or_else(|| Error::new("armed receipt omitted Buffer writer lease"))?
                .lease;
            let _ = runner.buffer_writer_status(lease)?;
        }
        Some(receipt)
            if matches!(
                receipt.phase,
                ReceiptPhaseV1::BufferReady
                    | ReceiptPhaseV1::MessagePrepared
                    | ReceiptPhaseV1::SignedNotSubmitted
            ) =>
        {
            if observation.loader != receipt.before
                || observation.wallet_lamports
                    != receipt
                        .buffer_upload_wallet_after_lamports
                        .ok_or_else(|| Error::new("Buffer-ready receipt omitted payer balance"))?
            {
                return Err(Error::new(
                    "preflight found drift from the exact Buffer-ready Loader/payer prestate",
                ));
            }
            let buffer = runner
                .read_buffer(
                    &args.origin,
                    parse_pubkey(&receipt.buffer_pubkey, "receipt Buffer")?,
                    args.expected_upgrade_authority,
                    receipt.before_context_slot,
                )?
                .ok_or_else(|| Error::new("preflight found the exact ready Buffer missing"))?;
            if buffer.account_sha256
                != receipt
                    .buffer_ready_account_sha256
                    .as_deref()
                    .unwrap_or_default()
                || buffer.payload_sha256 != receipt.raw_elf_sha256
            {
                return Err(Error::new("preflight found the ready Buffer body drifted"));
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
        buffer: parse_pubkey(&receipt.buffer_pubkey, "receipt Buffer")?,
        buffer_lamports: receipt.buffer_rent_exempt_lamports,
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

    match existing {
        None => {
            let audited_journal = authenticate_mutation_boundary(&shadow, runner)?;
            let deployment_set_journal_sha256 =
                require_mutation_permit(&shadow, None, false, audited_journal.as_deref())?;
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
                deployment_set_journal_sha256,
                baseline_sha256: baseline.baseline_sha256.clone(),
                baseline_context_slot: baseline.context_slot,
                solana_cli_version: args.expected_solana_cli_version.clone(),
                extension_additional_bytes: baseline.extension_additional_bytes,
                target_rent_exempt_minimum_lamports: baseline.target_rent_exempt_minimum_lamports,
                expected_rent_top_up_lamports: baseline.extension_lamport_top_up,
                before_context_slot: before.context_slot,
                before: before.loader,
                wallet_before_lamports: before.wallet_lamports,
                expired_packets: Vec::new(),
                unsigned_message_base64: None,
                unsigned_message_sha256: None,
                recent_blockhash: None,
                last_valid_block_height: None,
                signed_packet_base64: None,
                signed_packet_sha256: None,
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
            continue_extension(runner, args, &shadow, &baseline, &mut receipt)
        }
        Some(mut receipt)
            if matches!(
                receipt.phase,
                ReceiptPhaseV1::Prepared
                    | ReceiptPhaseV1::MessagePrepared
                    | ReceiptPhaseV1::SignedNotSubmitted
                    | ReceiptPhaseV1::Submitted
            ) =>
        {
            let audited_journal =
                authenticate_phase_mutation_boundary(&shadow, runner, receipt.phase.clone())?;
            require_mutation_permit(
                &shadow,
                Some(&receipt.deployment_set_journal_sha256),
                false,
                audited_journal.as_deref(),
            )?;
            let current = read_snapshot(runner, &shadow, receipt.before_context_slot)?;
            if receipt.phase != ReceiptPhaseV1::Submitted {
                require_baseline_prestate(&baseline, &current.loader)?;
            }
            if receipt.phase != ReceiptPhaseV1::Submitted
                && current.wallet_lamports != receipt.wallet_before_lamports
            {
                return Err(Error::new(
                    "extension plan exists before submission but its payer balance moved; the \
                     exact journaled action is refused",
                ));
            }
            continue_extension(runner, args, &shadow, &baseline, &mut receipt)
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
        signature: receipt
            .transaction_signature
            .as_deref()
            .ok_or_else(|| Error::new("complete extension omitted signature"))?,
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

fn continue_extension(
    runner: &mut impl CliRunner,
    args: &ExtensionArgsV1,
    shadow: &UpgradeArgsV1,
    baseline: &UpgradeBaselineV1,
    receipt: &mut ExtensionReceiptV1,
) -> Result<ExtensionReceiptV1> {
    let additional_bytes = u32::try_from(baseline.extension_additional_bytes)
        .map_err(|_| Error::new("extension byte count exceeds Loader u32"))?;
    let query = LoaderActionQueryV1 {
        origin: &args.origin,
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        authority_keypair: &args.authority_keypair,
        payer_keypair: &args.fee_payer_keypair,
        action: LoaderActionV1::Extend { additional_bytes },
    };
    loop {
        let audited_journal =
            authenticate_phase_mutation_boundary(shadow, runner, receipt.phase.clone())?;
        require_mutation_permit(
            shadow,
            Some(&receipt.deployment_set_journal_sha256),
            false,
            audited_journal.as_deref(),
        )?;
        match receipt.phase {
            ReceiptPhaseV1::Prepared => {
                let unsigned = runner.prepare_loader_action(&query)?;
                authenticate_unsigned_loader_action(&query, &unsigned)?;
                receipt.phase = ReceiptPhaseV1::MessagePrepared;
                receipt.unsigned_message_base64 = Some(unsigned.message_base64);
                receipt.unsigned_message_sha256 = Some(unsigned.message_sha256);
                receipt.recent_blockhash = Some(unsigned.recent_blockhash);
                receipt.last_valid_block_height = Some(unsigned.last_valid_block_height);
                write_extension_receipt(&args.receipt_path, receipt)?;
            }
            ReceiptPhaseV1::MessagePrepared => {
                let unsigned = extension_unsigned(receipt)?;
                let signed = runner.sign_loader_action(&query, &unsigned)?;
                authenticate_signed_loader_action(&query, &unsigned, &signed)?;
                receipt.phase = ReceiptPhaseV1::SignedNotSubmitted;
                receipt.signed_packet_base64 = Some(signed.packet_base64);
                receipt.signed_packet_sha256 = Some(signed.packet_sha256);
                receipt.transaction_signature = Some(signed.signature);
                write_extension_receipt(&args.receipt_path, receipt)?;
            }
            ReceiptPhaseV1::SignedNotSubmitted => {
                let unsigned = extension_unsigned(receipt)?;
                let signed = extension_signed(receipt)?;
                authenticate_signed_loader_action(&query, &unsigned, &signed)?;
                // Submitted is durable before the one allowed send. A crash
                // before or after send therefore always resumes poll-only.
                receipt.phase = ReceiptPhaseV1::Submitted;
                receipt.solana_cli_output = Some(serde_json::json!({
                    "transport": "sendTransaction",
                    "maxRetries": 0,
                    "signature": signed.signature,
                }));
                write_extension_receipt(&args.receipt_path, receipt)?;
                let returned = runner.send_loader_action(&args.origin, &signed)?;
                if returned != signed.signature {
                    return Err(Error::new(
                        "extension send returned a signature different from the fsynced packet",
                    ));
                }
            }
            ReceiptPhaseV1::Submitted => {
                return finish_submitted_extension(runner, args, shadow, baseline, receipt);
            }
            ReceiptPhaseV1::Complete => return Ok(receipt.clone()),
            ReceiptPhaseV1::BufferWriteArmed | ReceiptPhaseV1::BufferReady => {
                return Err(Error::new(
                    "extension receipt entered an Upgrade-only phase",
                ));
            }
        }
    }
}

fn extension_unsigned(receipt: &ExtensionReceiptV1) -> Result<UnsignedLoaderActionV1> {
    Ok(UnsignedLoaderActionV1 {
        message_base64: receipt
            .unsigned_message_base64
            .clone()
            .ok_or_else(|| Error::new("extension receipt omitted unsigned message"))?,
        message_sha256: receipt
            .unsigned_message_sha256
            .clone()
            .ok_or_else(|| Error::new("extension receipt omitted unsigned message digest"))?,
        recent_blockhash: receipt
            .recent_blockhash
            .clone()
            .ok_or_else(|| Error::new("extension receipt omitted recent blockhash"))?,
        last_valid_block_height: receipt
            .last_valid_block_height
            .ok_or_else(|| Error::new("extension receipt omitted expiry height"))?,
    })
}

fn extension_signed(receipt: &ExtensionReceiptV1) -> Result<SignedLoaderActionV1> {
    Ok(SignedLoaderActionV1 {
        packet_base64: receipt
            .signed_packet_base64
            .clone()
            .ok_or_else(|| Error::new("extension receipt omitted signed packet"))?,
        packet_sha256: receipt
            .signed_packet_sha256
            .clone()
            .ok_or_else(|| Error::new("extension receipt omitted signed packet digest"))?,
        signature: receipt
            .transaction_signature
            .clone()
            .ok_or_else(|| Error::new("extension receipt omitted packet signature"))?,
    })
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
        if after.loader != receipt.before || after.wallet_lamports != receipt.wallet_before_lamports
        {
            return Err(Error::new(
                "extension deployment slot did not advance and its exact prestate moved; recovery is ambiguous",
            ));
        }
        let signature = receipt
            .transaction_signature
            .as_deref()
            .ok_or_else(|| Error::new("submitted extension omitted signature"))?;
        match runner.journaled_signature_status(&args.origin, signature)? {
            JournaledSignatureStatusV1::FinalizedFailure(error) => {
                return Err(Error::new(format!(
                    "journaled extension finalized with failure {error}; its fee/prestate boundary must be attributed before any replacement"
                )));
            }
            JournaledSignatureStatusV1::FinalizedSuccess => {
                return Err(Error::new(
                    "journaled extension is finalized successful but the exact finalized Loader snapshot has not caught up",
                ));
            }
            JournaledSignatureStatusV1::Pending => {
                return Err(Error::new(
                    "journaled extension signature is still present and pending; recovery is poll-only even after blockhash expiry",
                ));
            }
            JournaledSignatureStatusV1::NotFound => {}
        }
        let finalized_height = runner.finalized_block_height(&args.origin)?;
        let last_valid = receipt
            .last_valid_block_height
            .ok_or_else(|| Error::new("submitted extension omitted expiry height"))?;
        if finalized_height <= last_valid {
            return Err(Error::new(format!(
                "journaled extension signature remains pending through finalized block height {finalized_height}; poll-only recovery refuses a second send before expiry {last_valid}"
            )));
        }
        archive_expired_extension_packet(receipt, finalized_height)?;
        write_extension_receipt(&args.receipt_path, receipt)?;
        return continue_extension(runner, args, shadow, _baseline, receipt);
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
        signature: receipt
            .transaction_signature
            .as_deref()
            .ok_or_else(|| Error::new("submitted extension omitted signature"))?,
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

fn archive_expired_extension_packet(
    receipt: &mut ExtensionReceiptV1,
    finalized_height: u64,
) -> Result<()> {
    let unsigned = extension_unsigned(receipt)?;
    let signed = extension_signed(receipt)?;
    if finalized_height <= unsigned.last_valid_block_height {
        return Err(Error::new("cannot archive an unexpired extension packet"));
    }
    receipt.expired_packets.push(ExpiredLoaderPacketV1 {
        unsigned_message_base64: unsigned.message_base64,
        unsigned_message_sha256: unsigned.message_sha256,
        recent_blockhash: unsigned.recent_blockhash,
        last_valid_block_height: unsigned.last_valid_block_height,
        signed_packet_base64: signed.packet_base64,
        signed_packet_sha256: signed.packet_sha256,
        transaction_signature: signed.signature,
        expiry_observed_finalized_block_height: finalized_height,
    });
    receipt.phase = ReceiptPhaseV1::Prepared;
    receipt.unsigned_message_base64 = None;
    receipt.unsigned_message_sha256 = None;
    receipt.recent_blockhash = None;
    receipt.last_valid_block_height = None;
    receipt.signed_packet_base64 = None;
    receipt.signed_packet_sha256 = None;
    receipt.transaction_signature = None;
    receipt.solana_cli_output = None;
    Ok(())
}

fn buffer_write_arguments(
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
) -> Result<Vec<String>> {
    Ok(vec![
        "program".into(),
        "write-buffer".into(),
        path_argument(&args.elf_path, "--elf")?,
        "--url".into(),
        args.origin.url().into(),
        "--buffer".into(),
        path_argument(&args.buffer_keypair, BUFFER_KEYPAIR_FLAG)?,
        "--buffer-authority".into(),
        path_argument(&args.authority_keypair, "--buffer-authority")?,
        "--fee-payer".into(),
        path_argument(&args.fee_payer_keypair, "--fee-payer")?,
        "--max-len".into(),
        gate.raw_elf.len().to_string(),
        "--max-sign-attempts".into(),
        "1".into(),
        "--output".into(),
        "json".into(),
    ])
}

fn buffer_write_command_sha256(
    args: &UpgradeArgsV1,
    command_arguments: &[String],
) -> Result<String> {
    Ok(digest(&serde_json::to_vec(&serde_json::json!({
        "solana_cli": path_argument(&args.solana_cli, "--solana-cli")?,
        "arguments": command_arguments,
    }))?))
}

fn buffer_writer_paths(
    args: &UpgradeArgsV1,
    operation_id: &str,
    attempt_ordinal: u64,
) -> Result<(PathBuf, PathBuf)> {
    let parent = args
        .receipt_path
        .parent()
        .ok_or_else(|| Error::new("Upgrade receipt omitted parent"))?;
    let name = args
        .receipt_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new("Upgrade receipt file name is not UTF-8"))?;
    Ok((
        parent.join(format!(
            ".{name}.{operation_id}.{attempt_ordinal}.buffer-writer-lease"
        )),
        parent.join(format!(
            ".{name}.{operation_id}.{attempt_ordinal}.buffer-writer-permit"
        )),
    ))
}

fn buffer_writer_query<'a>(
    args: &UpgradeArgsV1,
    receipt: &'a UpgradeReceiptV1,
    command_arguments: &'a [String],
    command_sha256: &'a str,
    attempt_ordinal: u64,
    lease_path: &'a Path,
    permit_path: &'a Path,
) -> BufferWriterQueryV1<'a> {
    let _ = args;
    BufferWriterQueryV1 {
        operation_id: &receipt.operation_id,
        attempt_ordinal,
        command_arguments,
        command_sha256,
        lease_path,
        permit_path,
    }
}

fn start_leased_buffer_attempt(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    receipt: &mut UpgradeReceiptV1,
    armed_height: u64,
) -> Result<()> {
    let attempt_ordinal = u64::try_from(receipt.buffer_write_attempts.len())
        .map_err(|_| Error::new("Buffer writer attempt ordinal overflow"))?;
    let command_arguments = buffer_write_arguments(args, gate)?;
    let command_sha256 = buffer_write_command_sha256(args, &command_arguments)?;
    let (lease_path, permit_path) =
        buffer_writer_paths(args, &receipt.operation_id, attempt_ordinal)?;
    let query = buffer_writer_query(
        args,
        receipt,
        &command_arguments,
        &command_sha256,
        attempt_ordinal,
        &lease_path,
        &permit_path,
    );
    let lease = runner.start_buffer_writer(&query)?;
    let expiry = armed_height
        .checked_add(BUFFER_WRITE_EXPIRY_BLOCKS)
        .ok_or_else(|| Error::new("Buffer writer expiry overflow"))?;
    receipt.phase = ReceiptPhaseV1::BufferWriteArmed;
    receipt.buffer_write_armed_block_height = Some(armed_height);
    receipt.buffer_write_expiry_block_height = Some(expiry);
    receipt.buffer_write_cli_output = None;
    receipt.buffer_write_attempts.push(BufferWriteAttemptV1 {
        lease,
        armed_finalized_block_height: armed_height,
        expiry_finalized_block_height: expiry,
        exit_observed_finalized_block_height: None,
        exit_disposition: None,
    });
    write_receipt(&args.receipt_path, receipt)
}

fn continue_upgrade(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    candidate_live: &[u8],
    receipt: &mut UpgradeReceiptV1,
) -> Result<UpgradeReceiptV1> {
    let query = LoaderActionQueryV1 {
        origin: &args.origin,
        program_id: args.program_id,
        programdata_id: args.programdata_id,
        authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        authority_keypair: &args.authority_keypair,
        payer_keypair: &args.fee_payer_keypair,
        action: LoaderActionV1::Upgrade {
            buffer: args.buffer_pubkey,
        },
    };
    loop {
        let audited_journal =
            authenticate_phase_mutation_boundary(args, runner, receipt.phase.clone())?;
        require_mutation_permit(
            args,
            Some(&receipt.deployment_set_journal_sha256),
            true,
            audited_journal.as_deref(),
        )?;
        match receipt.phase {
            ReceiptPhaseV1::Prepared => {
                if runner
                    .read_buffer(
                        &args.origin,
                        args.buffer_pubkey,
                        args.expected_upgrade_authority,
                        receipt.before_context_slot,
                    )?
                    .is_some()
                {
                    return Err(Error::new(
                        "Buffer appeared before its upload boundary was armed",
                    ));
                }
                let armed_height = runner.finalized_block_height(&args.origin)?;
                start_leased_buffer_attempt(runner, args, gate, receipt, armed_height)?;
            }
            ReceiptPhaseV1::BufferWriteArmed => {
                let attempt_index = receipt
                    .buffer_write_attempts
                    .len()
                    .checked_sub(1)
                    .ok_or_else(|| Error::new("armed Buffer receipt omitted writer attempt"))?;
                let lease = receipt.buffer_write_attempts[attempt_index].lease.clone();
                let status = runner.buffer_writer_status(&lease)?;
                if status == BufferWriterStatusV1::Exited {
                    if receipt.buffer_write_attempts[attempt_index]
                        .exit_observed_finalized_block_height
                        .is_none()
                    {
                        let height = runner.finalized_block_height(&args.origin)?;
                        receipt.buffer_write_attempts[attempt_index]
                            .exit_observed_finalized_block_height = Some(height);
                        receipt.buffer_write_attempts[attempt_index].exit_disposition =
                            Some("lost_after_operator_crash".into());
                        write_receipt(&args.receipt_path, receipt)?;
                    }
                    if authenticate_ready_buffer(runner, args, gate, receipt)? {
                        continue;
                    }
                    let height = runner.finalized_block_height(&args.origin)?;
                    let expiry =
                        receipt.buffer_write_attempts[attempt_index].expiry_finalized_block_height;
                    if height <= expiry {
                        return Err(Error::new(format!(
                            "leased Buffer writer exited without an exact finalized Buffer; no second writer is allowed before conservative expiry {expiry} (current {height})"
                        )));
                    }
                    start_leased_buffer_attempt(runner, args, gate, receipt, height)?;
                    continue;
                }

                // The exact Buffer identity, ELF, rent, authority, payer,
                // PID, start token, private process group and retry horizon
                // are fsynced before any of these key reads or the permit.
                authenticate_keypair(
                    runner,
                    args,
                    &args.buffer_keypair,
                    args.buffer_pubkey,
                    "persistent Buffer",
                )?;
                authenticate_keypair(
                    runner,
                    args,
                    &args.authority_keypair,
                    args.expected_upgrade_authority,
                    "Buffer authority",
                )?;
                authenticate_keypair(
                    runner,
                    args,
                    &args.fee_payer_keypair,
                    args.fee_payer,
                    "Buffer fee payer",
                )?;
                let command_arguments = buffer_write_arguments(args, gate)?;
                let command_sha256 = buffer_write_command_sha256(args, &command_arguments)?;
                let query = BufferWriterQueryV1 {
                    operation_id: &receipt.operation_id,
                    attempt_ordinal: lease.attempt_ordinal,
                    command_arguments: &command_arguments,
                    command_sha256: &command_sha256,
                    lease_path: Path::new(&lease.lease_path),
                    permit_path: Path::new(&lease.permit_path),
                };
                let output = runner.continue_buffer_writer(&query, &lease)?;
                if !output.success {
                    let height = runner.finalized_block_height(&args.origin)?;
                    receipt.buffer_write_attempts[attempt_index]
                        .exit_observed_finalized_block_height = Some(height);
                    receipt.buffer_write_attempts[attempt_index].exit_disposition =
                        Some("returned_failure".into());
                    write_receipt(&args.receipt_path, receipt)?;
                    return Err(Error::new(format!(
                        "leased write-buffer failed after one attempt: {}",
                        redact(&output.stderr, args)
                    )));
                }
                let parsed: Value = serde_json::from_str(output.stdout.trim()).map_err(|error| {
                    Error::new(format!(
                        "write-buffer output was lost or invalid after its one attempt: {error}; receipt remains armed and recovery is read-only"
                    ))
                })?;
                let returned = parsed
                    .get("buffer")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Error::new("write-buffer output omitted exact buffer"))?;
                if returned != args.buffer_pubkey.to_string() {
                    return Err(Error::new(
                        "write-buffer output substituted Buffer identity",
                    ));
                }
                receipt.buffer_write_cli_output = Some(parsed);
                let height = runner.finalized_block_height(&args.origin)?;
                receipt.buffer_write_attempts[attempt_index].exit_observed_finalized_block_height =
                    Some(height);
                receipt.buffer_write_attempts[attempt_index].exit_disposition =
                    Some("returned_success".into());
                write_receipt(&args.receipt_path, receipt)?;
                if !authenticate_ready_buffer(runner, args, gate, receipt)? {
                    return Err(Error::new(
                        "one-attempt Buffer writer returned before exact payload finalized; recovery is poll-only until expiry",
                    ));
                }
            }
            ReceiptPhaseV1::BufferReady => {
                let unsigned = runner.prepare_loader_action(&query)?;
                authenticate_unsigned_loader_action(&query, &unsigned)?;
                receipt.phase = ReceiptPhaseV1::MessagePrepared;
                receipt.unsigned_message_base64 = Some(unsigned.message_base64);
                receipt.unsigned_message_sha256 = Some(unsigned.message_sha256);
                receipt.recent_blockhash = Some(unsigned.recent_blockhash);
                receipt.last_valid_block_height = Some(unsigned.last_valid_block_height);
                write_receipt(&args.receipt_path, receipt)?;
            }
            ReceiptPhaseV1::MessagePrepared => {
                let unsigned = upgrade_unsigned(receipt)?;
                let signed = runner.sign_loader_action(&query, &unsigned)?;
                authenticate_signed_loader_action(&query, &unsigned, &signed)?;
                receipt.phase = ReceiptPhaseV1::SignedNotSubmitted;
                receipt.signed_packet_base64 = Some(signed.packet_base64);
                receipt.signed_packet_sha256 = Some(signed.packet_sha256);
                receipt.transaction_signature = Some(signed.signature);
                write_receipt(&args.receipt_path, receipt)?;
            }
            ReceiptPhaseV1::SignedNotSubmitted => {
                let unsigned = upgrade_unsigned(receipt)?;
                let signed = upgrade_signed(receipt)?;
                authenticate_signed_loader_action(&query, &unsigned, &signed)?;
                receipt.phase = ReceiptPhaseV1::Submitted;
                receipt.solana_cli_output = Some(serde_json::json!({
                    "transport": "sendTransaction",
                    "maxRetries": 0,
                    "signature": signed.signature,
                    "buffer": args.buffer_pubkey.to_string(),
                    "spill": args.fee_payer.to_string(),
                }));
                write_receipt(&args.receipt_path, receipt)?;
                let returned = runner.send_loader_action(&args.origin, &signed)?;
                if returned != signed.signature {
                    return Err(Error::new(
                        "Upgrade send returned a signature different from the fsynced packet",
                    ));
                }
            }
            ReceiptPhaseV1::Submitted => {
                return finish_submitted(runner, args, gate, candidate_live, receipt);
            }
            ReceiptPhaseV1::Complete => return Ok(receipt.clone()),
        }
    }
}

fn authenticate_ready_buffer(
    runner: &mut impl CliRunner,
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    receipt: &mut UpgradeReceiptV1,
) -> Result<bool> {
    let Some(buffer) = runner.read_buffer(
        &args.origin,
        args.buffer_pubkey,
        args.expected_upgrade_authority,
        receipt.before_context_slot,
    )?
    else {
        return Ok(false);
    };
    if buffer.data_bytes != receipt.buffer_data_bytes
        || buffer.lamports != receipt.buffer_rent_exempt_lamports
        || buffer.authority != args.expected_upgrade_authority.to_string()
    {
        return Err(Error::new(
            "persistent Buffer width, rent, or authority differs from the fsynced plan",
        ));
    }
    if buffer.payload_sha256 != gate.raw_elf_sha256 {
        return Ok(false);
    }
    let current = read_snapshot(runner, args, receipt.before_context_slot)?;
    if current.loader != receipt.before {
        return Err(Error::new(
            "Program/ProgramData moved before final Upgrade packet preparation",
        ));
    }
    let upload = runner.authenticate_buffer_upload(&BufferUploadQueryV1 {
        origin: &args.origin,
        buffer: parse_pubkey(&receipt.buffer_pubkey, "receipt Buffer")?,
        authority: args.expected_upgrade_authority,
        payer: args.fee_payer,
        raw_elf: &gate.raw_elf,
        minimum_slot: receipt.before_context_slot,
        expected_rent_lamports: receipt.buffer_rent_exempt_lamports,
        wallet_before_lamports: receipt.wallet_before_lamports,
        wallet_after_lamports: current.wallet_lamports,
    })?;
    require_digest(
        &upload.transactions_sha256,
        "Buffer upload transaction evidence SHA-256",
    )?;
    if upload.transactions_sha256 != digest(&serde_json::to_vec(&upload.transactions)?) {
        return Err(Error::new(
            "Buffer upload transaction evidence digest drifted",
        ));
    }
    receipt.phase = ReceiptPhaseV1::BufferReady;
    receipt.buffer_ready_context_slot = Some(buffer.context_slot);
    receipt.buffer_ready_account_sha256 = Some(buffer.account_sha256);
    receipt.buffer_ready_lamports = Some(buffer.lamports);
    receipt.buffer_upload_fee_lamports = Some(upload.fee_lamports);
    receipt.buffer_upload_transactions = Some(upload.transactions);
    receipt.buffer_upload_transactions_sha256 = Some(upload.transactions_sha256);
    receipt.buffer_upload_wallet_after_lamports = Some(current.wallet_lamports);
    write_receipt(&args.receipt_path, receipt)?;
    Ok(true)
}

fn upgrade_unsigned(receipt: &UpgradeReceiptV1) -> Result<UnsignedLoaderActionV1> {
    Ok(UnsignedLoaderActionV1 {
        message_base64: receipt
            .unsigned_message_base64
            .clone()
            .ok_or_else(|| Error::new("Upgrade receipt omitted unsigned message"))?,
        message_sha256: receipt
            .unsigned_message_sha256
            .clone()
            .ok_or_else(|| Error::new("Upgrade receipt omitted unsigned message digest"))?,
        recent_blockhash: receipt
            .recent_blockhash
            .clone()
            .ok_or_else(|| Error::new("Upgrade receipt omitted blockhash"))?,
        last_valid_block_height: receipt
            .last_valid_block_height
            .ok_or_else(|| Error::new("Upgrade receipt omitted expiry height"))?,
    })
}

fn upgrade_signed(receipt: &UpgradeReceiptV1) -> Result<SignedLoaderActionV1> {
    Ok(SignedLoaderActionV1 {
        packet_base64: receipt
            .signed_packet_base64
            .clone()
            .ok_or_else(|| Error::new("Upgrade receipt omitted signed packet"))?,
        packet_sha256: receipt
            .signed_packet_sha256
            .clone()
            .ok_or_else(|| Error::new("Upgrade receipt omitted signed packet digest"))?,
        signature: receipt
            .transaction_signature
            .clone()
            .ok_or_else(|| Error::new("Upgrade receipt omitted packet signature"))?,
    })
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
        let buffer = runner.read_buffer(
            &args.origin,
            args.buffer_pubkey,
            args.expected_upgrade_authority,
            receipt.before_context_slot,
        )?;
        if after.loader != receipt.before
            || buffer.as_ref().is_none_or(|buffer| {
                buffer.payload_sha256 != receipt.raw_elf_sha256
                    || buffer.account_sha256
                        != receipt
                            .buffer_ready_account_sha256
                            .as_deref()
                            .unwrap_or_default()
            })
        {
            return Err(Error::new(
                "Upgrade deployment slot has no slot advance and its ProgramData or authenticated Buffer moved; recovery is ambiguous",
            ));
        }
        let expected_payer = receipt
            .buffer_upload_wallet_after_lamports
            .ok_or_else(|| Error::new("submitted Upgrade omitted payer post-upload balance"))?;
        if after.wallet_lamports != expected_payer {
            return Err(Error::new(format!(
                "submitted Upgrade payer moved from journaled post-upload balance {expected_payer} to {}; a failed or external transaction fee must be attributed before any replacement",
                after.wallet_lamports
            )));
        }
        let signature = receipt
            .transaction_signature
            .as_deref()
            .ok_or_else(|| Error::new("submitted Upgrade omitted signature"))?;
        match runner.journaled_signature_status(&args.origin, signature)? {
            JournaledSignatureStatusV1::FinalizedFailure(error) => {
                return Err(Error::new(format!(
                    "journaled Upgrade finalized with failure {error}; its charged fee must be attributed before any replacement"
                )));
            }
            JournaledSignatureStatusV1::FinalizedSuccess => {
                return Err(Error::new(
                    "journaled Upgrade is finalized successful but the exact finalized Loader snapshot has not caught up",
                ));
            }
            JournaledSignatureStatusV1::Pending => {
                return Err(Error::new(
                    "journaled Upgrade signature is still present and pending; recovery is poll-only even after blockhash expiry",
                ));
            }
            JournaledSignatureStatusV1::NotFound => {}
        }
        let finalized_height = runner.finalized_block_height(&args.origin)?;
        let last_valid = receipt
            .last_valid_block_height
            .ok_or_else(|| Error::new("submitted Upgrade omitted expiry height"))?;
        if finalized_height <= last_valid {
            return Err(Error::new(format!(
                "journaled Upgrade signature remains pending through finalized block height {finalized_height}; poll-only recovery refuses a second send before expiry {last_valid}"
            )));
        }
        archive_expired_upgrade_packet(receipt, finalized_height)?;
        write_receipt(&args.receipt_path, receipt)?;
        return continue_upgrade(runner, args, gate, candidate_live, receipt);
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
        buffer: parse_pubkey(&receipt.buffer_pubkey, "receipt Buffer")?,
        buffer_lamports: receipt.buffer_rent_exempt_lamports,
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
    if transaction.payer_pre_lamports
        != receipt
            .buffer_upload_wallet_after_lamports
            .ok_or_else(|| Error::new("Buffer-ready receipt omitted payer post-upload balance"))?
        || transaction.payer_post_lamports != after.wallet_lamports
        || unattributed_cli_net_cost_lamports
            != receipt
                .buffer_upload_fee_lamports
                .ok_or_else(|| Error::new("Buffer-ready receipt omitted upload fees"))?
    {
        return Err(Error::new(
            "final Upgrade transaction does not bridge the exact Buffer-upload and operation wallet balances",
        ));
    }

    let (dump_sha256, dump_shape) = verify_dump(
        runner,
        args,
        &receipt.operation_id,
        &gate.raw_elf,
        candidate_live,
    )?;
    receipt.phase = ReceiptPhaseV1::Complete;
    receipt.finalized_transaction = Some(transaction.transaction);
    receipt.finalized_transaction_sha256 = Some(transaction.transaction_sha256);
    receipt.after_context_slot = Some(after.context_slot);
    receipt.after = Some(after.loader.clone());
    receipt.arithmetic = Some(UpgradeArithmeticV1 {
        transaction_payer_pre_lamports: transaction.payer_pre_lamports,
        transaction_payer_post_lamports: transaction.payer_post_lamports,
        transaction_fee_lamports: transaction.fee_lamports,
        payer_fee_delta_lamports: transaction.fee_lamports,
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

fn archive_expired_upgrade_packet(
    receipt: &mut UpgradeReceiptV1,
    finalized_height: u64,
) -> Result<()> {
    let unsigned = upgrade_unsigned(receipt)?;
    let signed = upgrade_signed(receipt)?;
    if finalized_height <= unsigned.last_valid_block_height {
        return Err(Error::new("cannot archive an unexpired Upgrade packet"));
    }
    receipt.expired_packets.push(ExpiredLoaderPacketV1 {
        unsigned_message_base64: unsigned.message_base64,
        unsigned_message_sha256: unsigned.message_sha256,
        recent_blockhash: unsigned.recent_blockhash,
        last_valid_block_height: unsigned.last_valid_block_height,
        signed_packet_base64: signed.packet_base64,
        signed_packet_sha256: signed.packet_sha256,
        transaction_signature: signed.signature,
        expiry_observed_finalized_block_height: finalized_height,
    });
    receipt.phase = ReceiptPhaseV1::BufferReady;
    receipt.unsigned_message_base64 = None;
    receipt.unsigned_message_sha256 = None;
    receipt.recent_blockhash = None;
    receipt.last_valid_block_height = None;
    receipt.signed_packet_base64 = None;
    receipt.signed_packet_sha256 = None;
    receipt.transaction_signature = None;
    receipt.solana_cli_output = None;
    Ok(())
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
        buffer: parse_pubkey(&receipt.buffer_pubkey, "receipt Buffer")?,
        buffer_lamports: receipt.buffer_rent_exempt_lamports,
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
    validate_checked_release_gate_selection(CheckedReleaseGateSelectionV1 {
        checked_release_gate_path: &args.checked_release_gate_path,
        expected_checked_release_gate_sha256: &args.expected_checked_release_gate_sha256,
        expected_source_revision: &args.expected_source_revision,
        expected_source_tree_sha256: &args.expected_source_tree_sha256,
        role: &args.role,
        elf_path: &args.elf_path,
    })
}

/// Re-authenticate one role of the exact thirteen-link checked-release gate for
/// a localhost-only mutable substrate.  This is deliberately a projection of
/// the Upgrade validator above: local evidence and devnet Upgrade evidence
/// share one gate authority, while retaining different deployment journals.
pub(crate) fn authenticate_checked_release_gate_role_for_local_v1(
    checked_release_gate_path: &Path,
    expected_checked_release_gate_sha256: &str,
    expected_source_revision: &str,
    expected_source_tree_sha256: &str,
    role: &str,
    elf_path: &Path,
) -> Result<CheckedLocalGateRoleV1> {
    let validated = validate_checked_release_gate_selection(CheckedReleaseGateSelectionV1 {
        checked_release_gate_path,
        expected_checked_release_gate_sha256,
        expected_source_revision,
        expected_source_tree_sha256,
        role,
        elf_path,
    })?;
    Ok(CheckedLocalGateRoleV1 {
        gate_sha256: validated.gate_sha256,
        source_revision: validated.source_revision,
        source_tree_sha256: validated.source_tree_sha256,
        solana_cli_version: validated.solana_cli_version,
        raw_elf_sha256: validated.raw_elf_sha256,
        checked_build_manifest_path: validated.checked_build_manifest_path,
        checked_build_manifest_sha256: validated.checked_build_manifest_sha256,
        checked_build_manifest: validated.checked_build_manifest,
    })
}

fn validate_checked_release_gate_selection(
    args: CheckedReleaseGateSelectionV1<'_>,
) -> Result<ValidatedUpgradeGateV1> {
    require_digest(
        args.expected_checked_release_gate_sha256,
        "expected checked-release gate SHA-256",
    )?;
    require_lower_hex(
        args.expected_source_revision,
        "expected source revision",
        40,
        40,
    )?;
    require_digest(
        args.expected_source_tree_sha256,
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
    let mut selected_manifest = None;
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
            let (canonical, bytes) =
                verify_gate_file(&root, manifest, &format!("{} checked manifest", link.label))?;
            if link.label == args.role {
                selected_manifest = Some((canonical, manifest.sha256.clone(), bytes));
            }
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
    let (checked_build_manifest_path, checked_build_manifest_sha256, checked_build_manifest) =
        selected_manifest.ok_or_else(|| {
            Error::new(format!(
                "checked-release gate carries no checked build manifest for selected role {}",
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
        checked_build_manifest_path,
        checked_build_manifest_sha256,
        checked_build_manifest,
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
    let expected_order = CHECKED_ROLE_ORDER_V1
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
    operation_id: &str,
    raw_elf: &[u8],
    candidate_live: &[u8],
) -> Result<(String, String)> {
    require_lower_hex(operation_id, "Upgrade operation id", 64, 64)?;
    if args.dump_path.exists() {
        let dump = read_dump_regular(&args.dump_path)?;
        return classify_dump(&dump, raw_elf, candidate_live);
    }
    let parent = args
        .dump_path
        .parent()
        .ok_or_else(|| Error::new("dump path omitted a parent"))?;
    if !parent.is_dir() {
        return Err(Error::new("dump parent does not exist"));
    }
    let file_name = args
        .dump_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new("dump file name is not UTF-8"))?;
    let temporary = parent.join(format!(".{file_name}.{operation_id}.dump-pending"));

    let mut recovered = false;
    if temporary.exists() {
        let bytes = read_dump_regular(&temporary)?;
        if classify_dump(&bytes, raw_elf, candidate_live).is_ok() {
            recovered = true;
        } else {
            // A CLI crash may leave a truncated operation-owned temporary.
            // It is never interpreted as the final dump and the read-only
            // command may safely recreate it on the next attempt.
            fs::remove_file(&temporary)?;
            sync_parent_directory(parent)?;
        }
    }
    if !recovered {
        invoke(
            runner,
            args,
            &[
                "program".into(),
                "dump".into(),
                args.program_id.to_string(),
                path_argument(&temporary, "--dump-temporary")?,
                "--url".into(),
                args.origin.url().into(),
            ],
        )?;
    }
    let dump = read_dump_regular(&temporary).map_err(|error| {
        Error::new(format!(
            "Solana CLI dump temporary {} could not be read: {error}",
            temporary.display()
        ))
    })?;
    let classified = classify_dump(&dump, raw_elf, candidate_live)?;
    fs::File::open(&temporary)?.sync_all()?;
    fs::hard_link(&temporary, &args.dump_path).map_err(|error| {
        Error::new(format!(
            "no-clobber dump publish {} failed: {error}",
            args.dump_path.display()
        ))
    })?;
    sync_parent_directory(parent)?;
    fs::remove_file(&temporary)?;
    sync_parent_directory(parent)?;
    Ok(classified)
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

fn loader_action_message(query: &LoaderActionQueryV1<'_>, blockhash: Hash) -> Result<Message> {
    let instruction = match query.action {
        LoaderActionV1::Extend { additional_bytes } => extend_program_checked(
            &query.program_id,
            &query.authority,
            Some(&query.payer),
            additional_bytes,
        ),
        LoaderActionV1::Upgrade { buffer } => {
            upgrade(&query.program_id, &buffer, &query.authority, &query.payer)
        }
    };
    if instruction
        .accounts
        .first()
        .is_none_or(|meta| meta.pubkey != query.programdata_id)
    {
        return Err(Error::new(
            "canonical Loader instruction derived a different ProgramData account",
        ));
    }
    Ok(Message::new_with_blockhash(
        &[instruction],
        Some(&query.payer),
        &blockhash,
    ))
}

fn prepare_loader_action_via_rpc(
    query: &LoaderActionQueryV1<'_>,
) -> Result<UnsignedLoaderActionV1> {
    let mut rpc = Rpc::connect_cluster(query.origin, WritePolicyV1::ReadsOnly)?;
    let value = rpc.call(
        "getLatestBlockhash",
        &serde_json::json!([{"commitment":"finalized"}]),
    )?;
    let inner = value
        .get("value")
        .ok_or_else(|| Error::new("getLatestBlockhash omitted value"))?;
    let blockhash_text = inner
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?;
    let blockhash = blockhash_text
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("recent blockhash: {error}")))?;
    let last_valid_block_height = inner
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted lastValidBlockHeight"))?;
    let message = loader_action_message(query, blockhash)?;
    let message_bytes = bincode::serialize(&message)
        .map_err(|error| Error::new(format!("serialize Loader message: {error}")))?;
    Ok(UnsignedLoaderActionV1 {
        message_base64: BASE64.encode(&message_bytes),
        message_sha256: digest(&message_bytes),
        recent_blockhash: blockhash_text.into(),
        last_valid_block_height,
    })
}

fn authenticate_unsigned_loader_action(
    query: &LoaderActionQueryV1<'_>,
    unsigned: &UnsignedLoaderActionV1,
) -> Result<Message> {
    require_digest(&unsigned.message_sha256, "unsigned Loader message SHA-256")?;
    let bytes = BASE64
        .decode(&unsigned.message_base64)
        .map_err(|_| Error::new("unsigned Loader message is not canonical base64"))?;
    if digest(&bytes) != unsigned.message_sha256 {
        return Err(Error::new(
            "unsigned Loader message bytes differ from their SHA-256",
        ));
    }
    let message: Message = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("decode unsigned Loader message: {error}")))?;
    let blockhash = unsigned
        .recent_blockhash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("journaled blockhash: {error}")))?;
    if message != loader_action_message(query, blockhash)?
        || bincode::serialize(&message)
            .map_err(|error| Error::new(format!("re-serialize Loader message: {error}")))?
            != bytes
    {
        return Err(Error::new(
            "unsigned Loader message is not the canonical exact action",
        ));
    }
    Ok(message)
}

fn read_expected_keypair(path: &Path, expected: Pubkey, label: &str) -> Result<Keypair> {
    let keypair = read_keypair_file(path)
        .map_err(|error| Error::new(format!("read {label} keypair: {error}")))?;
    if keypair.pubkey() != expected {
        return Err(Error::new(format!(
            "{label} keypair path resolves to {}, expected {expected}",
            keypair.pubkey()
        )));
    }
    Ok(keypair)
}

fn sign_loader_action_from_paths(
    query: &LoaderActionQueryV1<'_>,
    unsigned: &UnsignedLoaderActionV1,
) -> Result<SignedLoaderActionV1> {
    let message = authenticate_unsigned_loader_action(query, unsigned)?;
    // The receipt containing `unsigned` is fsynced before this function is
    // reachable. These are the first private-key reads in the canonical path.
    let payer = read_expected_keypair(query.payer_keypair, query.payer, "fee payer")?;
    let authority = read_expected_keypair(
        query.authority_keypair,
        query.authority,
        "upgrade authority",
    )?;
    let mut transaction = Transaction::new_unsigned(message);
    transaction
        .try_sign(&[&payer, &authority], transaction.message.recent_blockhash)
        .map_err(|error| Error::new(format!("sign canonical Loader transaction: {error}")))?;
    let signature = transaction
        .signatures
        .first()
        .copied()
        .ok_or_else(|| Error::new("signed Loader transaction omitted payer signature"))?;
    let packet = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize signed Loader packet: {error}")))?;
    Ok(SignedLoaderActionV1 {
        packet_base64: BASE64.encode(&packet),
        packet_sha256: digest(&packet),
        signature: signature.to_string(),
    })
}

fn authenticate_signed_loader_action(
    query: &LoaderActionQueryV1<'_>,
    unsigned: &UnsignedLoaderActionV1,
    signed: &SignedLoaderActionV1,
) -> Result<Transaction> {
    require_digest(&signed.packet_sha256, "signed Loader packet SHA-256")?;
    let packet = BASE64
        .decode(&signed.packet_base64)
        .map_err(|_| Error::new("signed Loader packet is not canonical base64"))?;
    if digest(&packet) != signed.packet_sha256 {
        return Err(Error::new(
            "signed Loader packet bytes differ from their SHA-256",
        ));
    }
    let transaction: Transaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("decode signed Loader packet: {error}")))?;
    if transaction.message != authenticate_unsigned_loader_action(query, unsigned)?
        || bincode::serialize(&transaction)
            .map_err(|error| Error::new(format!("re-serialize Loader packet: {error}")))?
            != packet
        || transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
            != Some(signed.signature.as_str())
    {
        return Err(Error::new(
            "signed Loader packet does not bind its exact unsigned message and signature",
        ));
    }
    transaction
        .verify()
        .map_err(|error| Error::new(format!("verify signed Loader packet: {error}")))?;
    Ok(transaction)
}

fn send_loader_action_via_rpc(
    origin: &ClusterOriginV1,
    signed: &SignedLoaderActionV1,
) -> Result<String> {
    let _ = Signature::from_str(&signed.signature)
        .map_err(|_| Error::new("journaled Loader signature is invalid"))?;
    let mut rpc = Rpc::connect_cluster(origin, WritePolicyV1::Writes)?;
    let returned = rpc
        .call(
            "sendTransaction",
            &serde_json::json!([signed.packet_base64, {
                "encoding":"base64",
                "skipPreflight":false,
                "preflightCommitment":"confirmed",
                "maxRetries":0
            }]),
        )?
        .as_str()
        .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
        .to_owned();
    if returned != signed.signature {
        return Err(Error::new(
            "sendTransaction returned a signature different from the journaled packet",
        ));
    }
    Ok(returned)
}

fn journaled_signature_status_via_rpc(
    origin: &ClusterOriginV1,
    signature: &str,
) -> Result<JournaledSignatureStatusV1> {
    let _ = Signature::from_str(signature)
        .map_err(|_| Error::new("journaled Loader signature is invalid"))?;
    let mut rpc = Rpc::connect_cluster(origin, WritePolicyV1::ReadsOnly)?;
    let value = rpc.call(
        "getSignatureStatuses",
        &serde_json::json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let rows = value
        .get("value")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("getSignatureStatuses omitted value array"))?;
    if rows.len() != 1 {
        return Err(Error::new(
            "getSignatureStatuses did not return exactly one journaled signature row",
        ));
    }
    let Some(row) = rows[0].as_object() else {
        if rows[0].is_null() {
            return Ok(JournaledSignatureStatusV1::NotFound);
        }
        return Err(Error::new(
            "journaled signature status row is not an object",
        ));
    };
    let confirmation = row
        .get("confirmationStatus")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("journaled signature status omitted confirmationStatus"))?;
    if confirmation != "finalized" {
        return Ok(JournaledSignatureStatusV1::Pending);
    }
    match row.get("err") {
        Some(error) if !error.is_null() => Ok(JournaledSignatureStatusV1::FinalizedFailure(
            serde_json::to_string(error)?,
        )),
        Some(_) => Ok(JournaledSignatureStatusV1::FinalizedSuccess),
        None => Err(Error::new(
            "finalized journaled signature status omitted err",
        )),
    }
}

fn read_buffer_via_rpc(
    origin: &ClusterOriginV1,
    buffer: Pubkey,
    authority: Pubkey,
    minimum_context_slot: u64,
) -> Result<Option<BufferObservationV1>> {
    let mut rpc = Rpc::connect_cluster(origin, WritePolicyV1::ReadsOnly)?;
    let (context_slot, accounts) = rpc.finalized_accounts(&[buffer], minimum_context_slot)?;
    let Some(account) = accounts.into_iter().next().flatten() else {
        return Ok(None);
    };
    let account = RpcAccountV1::from(account);
    if account.owner != bpf_loader_upgradeable::ID || account.executable {
        return Err(Error::new(
            "persistent Buffer is not a non-executable Loader-v3 account",
        ));
    }
    if account.data.len() < BUFFER_METADATA_BYTES
        || account.data.get(..4) != Some(&1_u32.to_le_bytes())
        || account.data.get(4) != Some(&1)
        || account.data.get(5..BUFFER_METADATA_BYTES) != Some(authority.as_ref())
    {
        return Err(Error::new(
            "persistent Buffer metadata or authority is not canonical",
        ));
    }
    Ok(Some(BufferObservationV1 {
        context_slot,
        lamports: account.lamports,
        data_bytes: u64::try_from(account.data.len())
            .map_err(|_| Error::new("Buffer width does not fit u64"))?,
        account_sha256: digest(&account.data),
        authority: authority.to_string(),
        payload_sha256: digest(&account.data[BUFFER_METADATA_BYTES..]),
    }))
}

fn authenticate_buffer_upload_via_rpc(
    query: &BufferUploadQueryV1<'_>,
) -> Result<BufferUploadEvidenceV1> {
    let mut rpc = Rpc::connect_cluster(query.origin, WritePolicyV1::ReadsOnly)?;
    let mut before = None::<String>;
    let mut previous_slot = None::<u64>;
    let mut crossed_prestate = false;
    let mut transactions = Vec::new();
    let mut total_fee = 0_u64;
    let mut successful_creates = 0_u64;
    let mut seen = BTreeSet::new();
    for _ in 0..SIGNATURE_HISTORY_MAX_PAGES {
        let mut options = serde_json::Map::new();
        options.insert("commitment".into(), Value::String("finalized".into()));
        options.insert(
            "limit".into(),
            Value::Number(SIGNATURE_HISTORY_PAGE_ROWS.into()),
        );
        if let Some(cursor) = &before {
            options.insert("before".into(), Value::String(cursor.clone()));
        }
        let rows = rpc.call(
            "getSignaturesForAddress",
            &Value::Array(vec![
                Value::String(query.buffer.to_string()),
                Value::Object(options),
            ]),
        )?;
        let rows = rows
            .as_array()
            .ok_or_else(|| Error::new("Buffer signature history was not an array"))?;
        if rows.is_empty() {
            crossed_prestate = true;
            break;
        }
        for row in rows {
            let slot = row
                .get("slot")
                .and_then(Value::as_u64)
                .ok_or_else(|| Error::new("Buffer signature row omitted slot"))?;
            if previous_slot.is_some_and(|previous| slot > previous) {
                return Err(Error::new(
                    "Buffer signature history is not newest-to-oldest",
                ));
            }
            previous_slot = Some(slot);
            if slot < query.minimum_slot {
                crossed_prestate = true;
                continue;
            }
            let signature = row
                .get("signature")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::new("Buffer signature row omitted signature"))?;
            Signature::from_str(signature)
                .map_err(|_| Error::new("Buffer history signature is invalid"))?;
            if !seen.insert(signature.to_owned()) {
                return Err(Error::new("Buffer signature history repeated a row"));
            }
            let transaction = rpc.call(
                "getTransaction",
                &serde_json::json!([signature, {
                    "encoding":"base64",
                    "commitment":"finalized",
                    "maxSupportedTransactionVersion":0
                }]),
            )?;
            let (fee, successful_create) =
                validate_buffer_upload_transaction(query, signature, slot, &transaction)?;
            total_fee = total_fee
                .checked_add(fee)
                .ok_or_else(|| Error::new("Buffer upload fee total overflow"))?;
            successful_creates = successful_creates
                .checked_add(u64::from(successful_create))
                .ok_or_else(|| Error::new("Buffer create count overflow"))?;
            transactions.push(serde_json::json!({
                "signature_row": row,
                "transaction": transaction,
            }));
        }
        before = rows
            .last()
            .and_then(|row| row.get("signature"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if crossed_prestate {
            break;
        }
    }
    if !crossed_prestate {
        return Err(Error::new(format!(
            "Buffer upload history did not cross prestate slot {} within the bounded history window",
            query.minimum_slot
        )));
    }
    if successful_creates != 1 || transactions.is_empty() {
        return Err(Error::new(format!(
            "Buffer upload history contains {successful_creates} successful exact creates; expected one"
        )));
    }
    let observed_spend = query
        .wallet_before_lamports
        .checked_sub(query.wallet_after_lamports)
        .ok_or_else(|| Error::new("Buffer upload payer wallet increased"))?;
    let expected_spend = query
        .expected_rent_lamports
        .checked_add(total_fee)
        .ok_or_else(|| Error::new("Buffer rent plus fee total overflow"))?;
    if observed_spend != expected_spend {
        return Err(Error::new(format!(
            "Buffer upload wallet delta is {observed_spend}; exact rent plus all attributable fees is {expected_spend}"
        )));
    }
    let transactions_sha256 = digest(&serde_json::to_vec(&transactions)?);
    Ok(BufferUploadEvidenceV1 {
        transactions,
        transactions_sha256,
        fee_lamports: total_fee,
    })
}

fn validate_buffer_upload_transaction(
    query: &BufferUploadQueryV1<'_>,
    signature: &str,
    slot: u64,
    evidence: &Value,
) -> Result<(u64, bool)> {
    if evidence.get("slot").and_then(Value::as_u64) != Some(slot) {
        return Err(Error::new(
            "Buffer transaction slot differs from history row",
        ));
    }
    let transaction_field = evidence
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("base64 Buffer transaction omitted transaction tuple"))?;
    if transaction_field.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(Error::new("Buffer transaction is not canonical base64"));
    }
    let packet = BASE64
        .decode(
            transaction_field
                .first()
                .and_then(Value::as_str)
                .ok_or_else(|| Error::new("Buffer transaction omitted base64 packet"))?,
        )
        .map_err(|_| Error::new("Buffer transaction packet is invalid base64"))?;
    let transaction: Transaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("decode Buffer transaction: {error}")))?;
    transaction
        .verify()
        .map_err(|error| Error::new(format!("verify Buffer transaction: {error}")))?;
    if transaction
        .signatures
        .first()
        .map(ToString::to_string)
        .as_deref()
        != Some(signature)
        || transaction.message.account_keys.first() != Some(&query.payer)
    {
        return Err(Error::new(
            "Buffer transaction signature or fee payer differs from the journal",
        ));
    }
    let keys = &transaction.message.account_keys;
    let unique_index = |wanted: Pubkey| -> Result<u8> {
        let matches = keys
            .iter()
            .enumerate()
            .filter(|(_, key)| **key == wanted)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::new(format!(
                "Buffer transaction does not carry one exact account {wanted}"
            )));
        }
        u8::try_from(matches[0]).map_err(|_| Error::new("account index exceeds u8"))
    };
    let payer_index = unique_index(query.payer)?;
    let buffer_index = unique_index(query.buffer)?;
    let authority_index = unique_index(query.authority)?;
    if payer_index != 0 || !transaction.message.is_signer(usize::from(payer_index)) {
        return Err(Error::new(
            "Buffer transaction payer is not the exact signer at key zero",
        ));
    }
    let mut create = false;
    let mut initialize = false;
    let mut writes = 0_u64;
    for instruction in &transaction.message.instructions {
        let program = keys
            .get(usize::from(instruction.program_id_index))
            .ok_or_else(|| Error::new("Buffer instruction program index is out of range"))?;
        if *program == solana_sdk_ids::system_program::ID {
            let decoded: SystemInstruction = bincode::deserialize(&instruction.data)
                .map_err(|_| Error::new("Buffer system instruction is not canonical bincode"))?;
            if decoded
                != (SystemInstruction::CreateAccount {
                    lamports: query.expected_rent_lamports,
                    space: u64::try_from(BUFFER_METADATA_BYTES + query.raw_elf.len())
                        .map_err(|_| Error::new("Buffer width does not fit u64"))?,
                    owner: bpf_loader_upgradeable::ID,
                })
                || instruction.accounts.as_slice() != [payer_index, buffer_index]
                || !transaction.message.is_signer(usize::from(buffer_index))
            {
                return Err(Error::new(
                    "Buffer transaction contains a substituted system instruction",
                ));
            }
            create = true;
        } else if *program == bpf_loader_upgradeable::ID {
            let decoded: UpgradeableLoaderInstruction = bincode::deserialize(&instruction.data)
                .map_err(|_| Error::new("Buffer Loader instruction is not canonical bincode"))?;
            match decoded {
                UpgradeableLoaderInstruction::InitializeBuffer => {
                    if instruction.accounts.as_slice() != [buffer_index, authority_index] {
                        return Err(Error::new(
                            "InitializeBuffer substituted buffer or authority",
                        ));
                    }
                    initialize = true;
                }
                UpgradeableLoaderInstruction::Write { offset, bytes } => {
                    if instruction.accounts.as_slice() != [buffer_index, authority_index] {
                        return Err(Error::new("Buffer Write substituted its accounts"));
                    }
                    if !transaction.message.is_signer(usize::from(authority_index)) {
                        return Err(Error::new("Buffer Write authority is not a signer"));
                    }
                    let start = usize::try_from(offset)
                        .map_err(|_| Error::new("Buffer Write offset does not fit host"))?;
                    let end = start
                        .checked_add(bytes.len())
                        .ok_or_else(|| Error::new("Buffer Write range overflow"))?;
                    if query.raw_elf.get(start..end) != Some(bytes.as_slice()) {
                        return Err(Error::new(
                            "Buffer Write bytes are not an idempotent slice of the exact ELF",
                        ));
                    }
                    writes = writes
                        .checked_add(1)
                        .ok_or_else(|| Error::new("Buffer Write count overflow"))?;
                }
                _ => {
                    return Err(Error::new(
                        "Buffer upload transaction contains a non-upload Loader instruction",
                    ));
                }
            }
        } else {
            return Err(Error::new(
                "Buffer upload transaction invokes an unrelated program",
            ));
        }
    }
    if create != initialize || (!create && writes == 0) {
        return Err(Error::new(
            "Buffer transaction is neither exact create+initialize nor one-or-more exact writes",
        ));
    }
    let meta = evidence
        .get("meta")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("Buffer transaction omitted meta"))?;
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("Buffer transaction fee is not u64"))?;
    if fee == 0 {
        return Err(Error::new("Buffer transaction recorded a zero fee"));
    }
    let successful = meta.get("err").is_some_and(Value::is_null);
    let balances = |name: &str| -> Result<&Vec<Value>> {
        meta.get(name)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("Buffer transaction omitted {name}")))
    };
    let pre = balances("preBalances")?;
    let post = balances("postBalances")?;
    if pre.len() != keys.len() || post.len() != keys.len() {
        return Err(Error::new(
            "Buffer transaction balance vector width drifted",
        ));
    }
    let payer_delta = pre[usize::from(payer_index)]
        .as_u64()
        .and_then(|before| {
            post[usize::from(payer_index)]
                .as_u64()
                .and_then(|after| before.checked_sub(after))
        })
        .ok_or_else(|| Error::new("Buffer transaction payer balance delta is invalid"))?;
    let expected_delta = if successful && create {
        fee.checked_add(query.expected_rent_lamports)
            .ok_or_else(|| Error::new("Buffer create payer delta overflow"))?
    } else {
        fee
    };
    if payer_delta != expected_delta {
        return Err(Error::new(
            "Buffer transaction payer delta is not exact fee plus successful create rent",
        ));
    }
    let buffer_pre = pre[usize::from(buffer_index)]
        .as_u64()
        .ok_or_else(|| Error::new("Buffer prebalance is not u64"))?;
    let buffer_post = post[usize::from(buffer_index)]
        .as_u64()
        .ok_or_else(|| Error::new("Buffer postbalance is not u64"))?;
    if successful && create {
        if buffer_pre != 0 || buffer_post != query.expected_rent_lamports {
            return Err(Error::new("Buffer create did not land exact rent"));
        }
    } else if buffer_pre != buffer_post {
        return Err(Error::new(
            "non-create Buffer upload transaction changed Buffer lamports",
        ));
    }
    Ok((fee, successful && create))
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
            "journaled Upgrade signature is not the transaction fee-payer signature",
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
    if info.get("bufferAccount").and_then(Value::as_str) != Some(query.buffer.to_string().as_str())
        || info.get("spillAccount").and_then(Value::as_str)
            != Some(query.payer.to_string().as_str())
    {
        return Err(Error::new(
            "Loader-v3 Upgrade did not consume the authenticated Buffer into the exact payer spill account",
        ));
    }
    let buffer_index = key_index(query.buffer, false, true)?;

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
    let buffer_pre = balance(pre, buffer_index, "pre Buffer")?;
    let buffer_post = balance(post, buffer_index, "post Buffer")?;
    if programdata_pre != query.programdata_before_lamports
        || programdata_post != query.programdata_after_lamports
        || programdata_pre != programdata_post
    {
        return Err(Error::new(
            "Upgrade transaction did not preserve and bridge exact ProgramData rent",
        ));
    }
    if buffer_pre != query.buffer_lamports || buffer_post != 0 {
        return Err(Error::new(
            "Upgrade transaction did not consume the authenticated Buffer's exact rent",
        ));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .filter(|fee| *fee != 0)
        .ok_or_else(|| Error::new("Upgrade transaction omitted a nonzero fee"))?;
    let expected_payer_post = payer_pre
        .checked_add(buffer_pre)
        .and_then(|with_refund| with_refund.checked_sub(fee))
        .ok_or_else(|| Error::new("Upgrade payer refund arithmetic overflow"))?;
    if payer_post != expected_payer_post {
        return Err(Error::new(
            "Upgrade payer postbalance is not exact prebalance plus Buffer refund minus finalized fee",
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
    Signature::from_str(query.signature)
        .map_err(|_| Error::new("journaled extension signature is invalid"))?;
    let transaction = rpc.call(
        "getTransaction",
        &serde_json::json!([query.signature, {
            "encoding": "jsonParsed",
            "commitment": "finalized",
            "maxSupportedTransactionVersion": 0
        }]),
    )?;
    validate_extension_transaction(query, query.signature, transaction)
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

fn validate_buffer_write_attempts(
    args: &UpgradeArgsV1,
    gate: &ValidatedUpgradeGateV1,
    receipt: &UpgradeReceiptV1,
) -> Result<()> {
    let command_sha256 = (args.buffer_pubkey != Pubkey::default())
        .then(|| buffer_write_arguments(args, gate))
        .transpose()?
        .map(|arguments| buffer_write_command_sha256(args, &arguments))
        .transpose()?;
    let mut identities = BTreeSet::new();
    for (index, attempt) in receipt.buffer_write_attempts.iter().enumerate() {
        let ordinal = u64::try_from(index).expect("attempt index fits u64");
        let (lease_path, permit_path) = buffer_writer_paths(args, &receipt.operation_id, ordinal)?;
        if attempt.lease.schema != BUFFER_WRITER_LEASE_SCHEMA
            || attempt.lease.operation_id != receipt.operation_id
            || attempt.lease.attempt_ordinal != ordinal
            || attempt.lease.pid == 0
            || attempt.lease.process_group_id != attempt.lease.pid
            || attempt.lease.process_start_token.is_empty()
            || require_digest(&attempt.lease.process_nonce, "Buffer process nonce").is_err()
            || command_sha256
                .as_ref()
                .is_some_and(|expected| attempt.lease.command_sha256 != *expected)
            || require_digest(&attempt.lease.command_sha256, "Buffer command SHA-256").is_err()
            || Path::new(&attempt.lease.lease_path) != lease_path
            || Path::new(&attempt.lease.permit_path) != permit_path
            || attempt.expiry_finalized_block_height
                != attempt
                    .armed_finalized_block_height
                    .checked_add(BUFFER_WRITE_EXPIRY_BLOCKS)
                    .ok_or_else(|| Error::new("Buffer attempt expiry overflow"))?
            || attempt.exit_observed_finalized_block_height.is_some()
                != attempt.exit_disposition.is_some()
            || !identities.insert((
                attempt.lease.pid,
                attempt.lease.process_start_token.as_str(),
                attempt.lease.process_nonce.as_str(),
            ))
        {
            return Err(Error::new(
                "Buffer writer attempt history has a substituted lease, command, window, process identity, or exit boundary",
            ));
        }
        if attempt.exit_disposition.as_deref().is_some_and(|value| {
            !matches!(
                value,
                "returned_success" | "returned_failure" | "lost_after_operator_crash"
            )
        }) {
            return Err(Error::new("Buffer writer exit disposition is invalid"));
        }
        if index + 1 < receipt.buffer_write_attempts.len() {
            let exit_height = attempt.exit_observed_finalized_block_height.ok_or_else(|| {
                Error::new("every superseded Buffer writer attempt must retain its exact exit boundary")
            })?;
            let successor = &receipt.buffer_write_attempts[index + 1];
            if successor.armed_finalized_block_height <= attempt.expiry_finalized_block_height
                || successor.armed_finalized_block_height < exit_height
            {
                return Err(Error::new(
                    "Buffer writer successor was armed before prior expiry and exact exit observation",
                ));
            }
        }
    }
    if let Some(last) = receipt.buffer_write_attempts.last()
        && (receipt.buffer_write_armed_block_height != Some(last.armed_finalized_block_height)
            || receipt.buffer_write_expiry_block_height != Some(last.expiry_finalized_block_height))
    {
        return Err(Error::new(
            "Buffer writer current arm fields differ from the durable attempt history",
        ));
    }
    Ok(())
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
        || require_digest(
            &receipt.deployment_set_journal_sha256,
            "Upgrade deployment-set journal SHA-256",
        )
        .is_err()
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
        || (args.buffer_pubkey != Pubkey::default()
            && (receipt.buffer_pubkey != args.buffer_pubkey.to_string()
                || receipt.buffer_keypair_path
                    != path_argument(&args.buffer_keypair, BUFFER_KEYPAIR_FLAG)?))
        || receipt.buffer_data_bytes
            != u64::try_from(BUFFER_METADATA_BYTES + gate.raw_elf.len())
                .map_err(|_| Error::new("Buffer width does not fit u64"))?
        || receipt.buffer_rent_exempt_lamports == 0
        || receipt.before_context_slot < baseline.context_slot
        || receipt.before != baseline.observation
    {
        return Err(Error::new(
            "existing Upgrade receipt does not bind this exact role, deployment, authority, \
             payer, cluster, source, evidence, or payload; substitution and ambiguous replay are \
             refused",
        ));
    }
    validate_expired_packets(&receipt.expired_packets)?;
    let expected_operation = operation_id_from_receipt(receipt);
    if receipt.operation_id != expected_operation {
        return Err(Error::new("Upgrade receipt operation_id is not canonical"));
    }
    validate_buffer_write_attempts(args, gate, receipt)?;
    let no_buffer_ready = receipt.buffer_ready_context_slot.is_none()
        && receipt.buffer_ready_account_sha256.is_none()
        && receipt.buffer_ready_lamports.is_none()
        && receipt.buffer_upload_fee_lamports.is_none()
        && receipt.buffer_upload_transactions.is_none()
        && receipt.buffer_upload_transactions_sha256.is_none()
        && receipt.buffer_upload_wallet_after_lamports.is_none();
    let has_buffer_ready = receipt.buffer_ready_context_slot.is_some()
        && receipt.buffer_ready_account_sha256.is_some()
        && receipt.buffer_ready_lamports == Some(receipt.buffer_rent_exempt_lamports)
        && receipt.buffer_upload_fee_lamports.is_some()
        && receipt.buffer_upload_transactions.is_some()
        && receipt.buffer_upload_transactions_sha256.is_some()
        && receipt.buffer_upload_wallet_after_lamports.is_some();
    if has_buffer_ready {
        let transactions = receipt
            .buffer_upload_transactions
            .as_ref()
            .expect("checked Buffer transaction evidence");
        if receipt.buffer_upload_transactions_sha256.as_deref()
            != Some(digest(&serde_json::to_vec(transactions)?).as_str())
            || receipt
                .buffer_ready_account_sha256
                .as_deref()
                .is_none_or(|value| require_digest(value, "Buffer account SHA-256").is_err())
        {
            return Err(Error::new(
                "Buffer-ready transaction or account digest drifted",
            ));
        }
    }
    let has_unsigned = receipt.unsigned_message_base64.is_some()
        && receipt.unsigned_message_sha256.is_some()
        && receipt.recent_blockhash.is_some()
        && receipt.last_valid_block_height.is_some();
    let no_unsigned = receipt.unsigned_message_base64.is_none()
        && receipt.unsigned_message_sha256.is_none()
        && receipt.recent_blockhash.is_none()
        && receipt.last_valid_block_height.is_none();
    let has_signed = receipt.signed_packet_base64.is_some()
        && receipt.signed_packet_sha256.is_some()
        && receipt.transaction_signature.is_some();
    let no_signed = receipt.signed_packet_base64.is_none()
        && receipt.signed_packet_sha256.is_none()
        && receipt.transaction_signature.is_none();
    match receipt.phase {
        ReceiptPhaseV1::Prepared => {
            if !receipt.buffer_write_attempts.is_empty()
                || receipt.buffer_write_armed_block_height.is_some()
                || receipt.buffer_write_expiry_block_height.is_some()
                || receipt.buffer_write_cli_output.is_some()
                || !no_buffer_ready
                || !no_unsigned
                || !no_signed
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
        ReceiptPhaseV1::BufferWriteArmed => {
            if receipt.buffer_write_attempts.is_empty()
                || receipt.buffer_write_armed_block_height.is_none()
                || receipt.buffer_write_expiry_block_height.is_none()
                || !no_buffer_ready
                || !no_unsigned
                || !no_signed
                || receipt.solana_cli_output.is_some()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
                || receipt.dump_sha256.is_some()
                || receipt.dump_shape.is_some()
            {
                return Err(Error::new("armed Buffer receipt is incomplete"));
            }
        }
        ReceiptPhaseV1::BufferReady => {
            if receipt.buffer_write_attempts.is_empty()
                || receipt
                    .buffer_write_attempts
                    .last()
                    .is_none_or(|attempt| attempt.exit_observed_finalized_block_height.is_none())
                || !has_buffer_ready
                || !no_unsigned
                || !no_signed
                || receipt.solana_cli_output.is_some()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
                || receipt.dump_sha256.is_some()
                || receipt.dump_shape.is_some()
            {
                return Err(Error::new("Buffer-ready Upgrade receipt is incomplete"));
            }
        }
        ReceiptPhaseV1::MessagePrepared => {
            if !has_buffer_ready
                || !has_unsigned
                || !no_signed
                || receipt.solana_cli_output.is_some()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
                || receipt.dump_sha256.is_some()
                || receipt.dump_shape.is_some()
            {
                return Err(Error::new("message-prepared Upgrade receipt is incomplete"));
            }
        }
        ReceiptPhaseV1::SignedNotSubmitted => {
            if !has_buffer_ready
                || !has_unsigned
                || !has_signed
                || receipt.solana_cli_output.is_some()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
                || receipt.dump_sha256.is_some()
                || receipt.dump_shape.is_some()
            {
                return Err(Error::new("signed Upgrade receipt is incomplete"));
            }
        }
        ReceiptPhaseV1::Submitted => {
            if !has_buffer_ready
                || !has_unsigned
                || !has_signed
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
            if !has_buffer_ready
                || !has_unsigned
                || !has_signed
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
    let buffer_fee = receipt
        .buffer_upload_fee_lamports
        .ok_or_else(|| Error::new("complete Upgrade receipt omitted Buffer upload fee"))?;
    let upload_wallet_after = receipt
        .buffer_upload_wallet_after_lamports
        .ok_or_else(|| Error::new("complete Upgrade receipt omitted upload wallet balance"))?;
    let operation_spend = arithmetic
        .operation_wallet_before_lamports
        .checked_sub(arithmetic.operation_wallet_after_lamports)
        .ok_or_else(|| Error::new("complete Upgrade operation wallet increased"))?;
    let unattributed = operation_spend
        .checked_sub(arithmetic.transaction_fee_lamports)
        .ok_or_else(|| {
            Error::new("complete Upgrade operation spend is smaller than final transaction fee")
        })?;
    let expected_upload_spend = receipt
        .buffer_rent_exempt_lamports
        .checked_add(buffer_fee)
        .ok_or_else(|| Error::new("complete Buffer upload spend overflow"))?;
    let upload_spend = receipt
        .wallet_before_lamports
        .checked_sub(upload_wallet_after)
        .ok_or_else(|| Error::new("complete Buffer upload wallet increased"))?;
    let expected_transaction_post = arithmetic
        .transaction_payer_pre_lamports
        .checked_add(receipt.buffer_rent_exempt_lamports)
        .and_then(|with_refund| with_refund.checked_sub(arithmetic.transaction_fee_lamports))
        .ok_or_else(|| Error::new("complete Upgrade refund arithmetic overflow"))?;
    if after.deployment_slot <= receipt.before.deployment_slot
        || after.upgrade_authority != receipt.retained_upgrade_authority
        || after.live_elf_sha256 != receipt.live_elf_sha256
        || after.programdata_lamports != receipt.before.programdata_lamports
        || arithmetic.transaction_fee_lamports == 0
        || arithmetic.payer_fee_delta_lamports != arithmetic.transaction_fee_lamports
        || arithmetic.transaction_payer_pre_lamports != upload_wallet_after
        || arithmetic.transaction_payer_post_lamports != expected_transaction_post
        || arithmetic.transaction_payer_post_lamports != arithmetic.operation_wallet_after_lamports
        || arithmetic.programdata_before_lamports != receipt.before.programdata_lamports
        || arithmetic.programdata_after_lamports != after.programdata_lamports
        || arithmetic.programdata_delta_lamports != 0
        || arithmetic.operation_wallet_before_lamports != receipt.wallet_before_lamports
        || upload_spend != expected_upload_spend
        || arithmetic.operation_observed_net_spend_lamports != operation_spend
        || arithmetic.unattributed_cli_net_cost_lamports != unattributed
        || unattributed != buffer_fee
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
    if output.len() != 5
        || output.get("transport").and_then(Value::as_str) != Some("sendTransaction")
        || output.get("maxRetries").and_then(Value::as_u64) != Some(0)
        || output.get("signature").and_then(Value::as_str) != Some(signature)
        || output.get("buffer").and_then(Value::as_str) != Some(receipt.buffer_pubkey.as_str())
        || output.get("spill").and_then(Value::as_str) != Some(receipt.fee_payer.as_str())
    {
        return Err(Error::new(
            "Upgrade receipt send evidence does not bind its exact packet, Buffer, spill, and maxRetries=0",
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
        || require_digest(
            &receipt.deployment_set_journal_sha256,
            "extension deployment-set journal SHA-256",
        )
        .is_err()
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
    validate_expired_packets(&receipt.expired_packets)?;
    let has_unsigned = receipt.unsigned_message_base64.is_some()
        && receipt.unsigned_message_sha256.is_some()
        && receipt.recent_blockhash.is_some()
        && receipt.last_valid_block_height.is_some();
    let no_unsigned = receipt.unsigned_message_base64.is_none()
        && receipt.unsigned_message_sha256.is_none()
        && receipt.recent_blockhash.is_none()
        && receipt.last_valid_block_height.is_none();
    let has_signed = receipt.signed_packet_base64.is_some()
        && receipt.signed_packet_sha256.is_some()
        && receipt.transaction_signature.is_some();
    let no_signed = receipt.signed_packet_base64.is_none()
        && receipt.signed_packet_sha256.is_none()
        && receipt.transaction_signature.is_none();
    match receipt.phase {
        ReceiptPhaseV1::BufferWriteArmed | ReceiptPhaseV1::BufferReady => {
            return Err(Error::new(
                "extension receipt entered an Upgrade-only phase",
            ));
        }
        ReceiptPhaseV1::Prepared => {
            if !no_unsigned
                || !no_signed
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
        ReceiptPhaseV1::MessagePrepared => {
            if !has_unsigned
                || !no_signed
                || receipt.solana_cli_output.is_some()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
            {
                return Err(Error::new(
                    "message-prepared extension receipt is incomplete",
                ));
            }
        }
        ReceiptPhaseV1::SignedNotSubmitted => {
            if !has_unsigned
                || !has_signed
                || receipt.solana_cli_output.is_some()
                || receipt.finalized_transaction.is_some()
                || receipt.finalized_transaction_sha256.is_some()
                || receipt.after_context_slot.is_some()
                || receipt.after.is_some()
                || receipt.arithmetic.is_some()
            {
                return Err(Error::new("signed extension receipt is incomplete"));
            }
        }
        ReceiptPhaseV1::Submitted => {
            if !has_unsigned
                || !has_signed
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
            if !has_unsigned
                || !has_signed
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

fn validate_expired_packets(packets: &[ExpiredLoaderPacketV1]) -> Result<()> {
    let mut signatures = BTreeSet::new();
    for packet in packets {
        require_digest(
            &packet.unsigned_message_sha256,
            "expired unsigned message SHA-256",
        )?;
        require_digest(
            &packet.signed_packet_sha256,
            "expired signed packet SHA-256",
        )?;
        let message = BASE64
            .decode(&packet.unsigned_message_base64)
            .map_err(|_| Error::new("expired unsigned message is not base64"))?;
        let signed = BASE64
            .decode(&packet.signed_packet_base64)
            .map_err(|_| Error::new("expired signed packet is not base64"))?;
        Signature::from_str(&packet.transaction_signature)
            .map_err(|_| Error::new("expired packet signature is invalid"))?;
        packet
            .recent_blockhash
            .parse::<Hash>()
            .map_err(|_| Error::new("expired packet blockhash is invalid"))?;
        if digest(&message) != packet.unsigned_message_sha256
            || digest(&signed) != packet.signed_packet_sha256
            || packet.expiry_observed_finalized_block_height <= packet.last_valid_block_height
            || !signatures.insert(packet.transaction_signature.as_str())
        {
            return Err(Error::new(
                "expired packet history has substituted bytes, duplicate signatures, or a non-expired height",
            ));
        }
    }
    Ok(())
}

fn load_receipt(path: &Path) -> Result<Option<UpgradeReceiptV1>> {
    match read_existing_regular_receipt(path, "Upgrade receipt")? {
        Some(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|error| {
            Error::new(format!(
                "existing Upgrade receipt {} is invalid: {error}",
                path.display()
            ))
        })?)),
        None => Ok(None),
    }
}

fn load_extension_receipt(path: &Path) -> Result<Option<ExtensionReceiptV1>> {
    match read_existing_regular_receipt(path, "extension receipt")? {
        Some(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|error| {
            Error::new(format!(
                "existing extension receipt {} is invalid: {error}",
                path.display()
            ))
        })?)),
        None => Ok(None),
    }
}

fn write_receipt(path: &Path, receipt: &mut UpgradeReceiptV1) -> Result<()> {
    let expected = (!receipt.receipt_sha256.is_empty()).then(|| receipt.receipt_sha256.clone());
    receipt.receipt_sha256.clear();
    receipt.receipt_sha256 = digest(&serde_json::to_vec(receipt)?);
    write_json_atomic_receipt_cas(path, receipt, expected.as_deref())
}

fn write_extension_receipt(path: &Path, receipt: &mut ExtensionReceiptV1) -> Result<()> {
    let expected = (!receipt.receipt_sha256.is_empty()).then(|| receipt.receipt_sha256.clone());
    receipt.receipt_sha256.clear();
    receipt.receipt_sha256 = digest(&serde_json::to_vec(receipt)?);
    write_json_atomic_receipt_cas(path, receipt, expected.as_deref())
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
    sync_parent_directory(parent)?;
    Ok(())
}

const RECEIPT_TRANSITION_LOCK_SCHEMA: &str = "dclutch-receipt-transition-lock-v1";

fn read_existing_regular_receipt(path: &Path, label: &str) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
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
            Ok(Some(fs::read(path)?))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn canonical_embedded_receipt_sha256(bytes: &[u8], label: &str) -> Result<String> {
    let mut value: Value = serde_json::from_slice(bytes)
        .map_err(|error| Error::new(format!("{label} is not JSON: {error}")))?;
    let embedded = value
        .get("receipt_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{label} omitted receipt_sha256")))?
        .to_owned();
    require_digest(&embedded, &format!("{label} embedded receipt SHA-256"))?;
    let canonical = match value.get("schema").and_then(Value::as_str) {
        Some(SCHEMA) => {
            let mut receipt: UpgradeReceiptV1 = serde_json::from_value(value.clone())?;
            receipt.receipt_sha256.clear();
            serde_json::to_vec(&receipt)?
        }
        Some(EXTENSION_SCHEMA) => {
            let mut receipt: ExtensionReceiptV1 = serde_json::from_value(value.clone())?;
            receipt.receipt_sha256.clear();
            serde_json::to_vec(&receipt)?
        }
        _ => {
            value
                .as_object_mut()
                .expect("receipt digest field proves object")
                .insert("receipt_sha256".into(), Value::String(String::new()));
            serde_json::to_vec(&value)?
        }
    };
    if digest(&canonical) != embedded {
        return Err(Error::new(format!(
            "{label} body differs from its embedded canonical receipt SHA-256"
        )));
    }
    Ok(embedded)
}

fn process_start_token(pid: u32) -> Result<Option<String>> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(Error::new(
            "process identity PID is outside the host-safe range",
        ));
    }
    let output = Command::new("/bin/ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .output()?;
    if !output.status.success() {
        let alive = Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?
            .success();
        if alive {
            return Err(Error::new(
                "process exists but its exact operating-system start token cannot be read",
            ));
        }
        return Ok(None);
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok((!token.is_empty()).then_some(token))
}

fn publish_receipt_transition(path: &Path, pending: &Path, expected: Option<&str>) -> Result<()> {
    match expected {
        None => fs::hard_link(pending, path).map_err(|error| {
            Error::new(format!(
                "no-clobber receipt publish {} failed: {error}",
                path.display()
            ))
        })?,
        Some(expected) => {
            let current = read_existing_regular_receipt(path, "receipt CAS publish target")?
                .ok_or_else(|| Error::new("receipt CAS publish target disappeared"))?;
            if canonical_embedded_receipt_sha256(&current, "receipt CAS publish target")?
                != expected
            {
                return Err(Error::new(
                    "receipt CAS publish target changed before atomic replacement",
                ));
            }
            fs::rename(pending, path)?;
        }
    }
    sync_parent_directory(
        path.parent()
            .ok_or_else(|| Error::new("receipt path omitted parent"))?,
    )
}

fn recover_receipt_transition(path: &Path, lock_path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("receipt path omitted parent"))?;
    let lock_bytes = read_regular_reference(lock_path, "receipt transition lock")?;
    let lock: ReceiptTransitionLockV1 = serde_json::from_slice(&lock_bytes)?;
    if lock.schema != RECEIPT_TRANSITION_LOCK_SCHEMA
        || lock.pending_file_name.contains('/')
        || lock.pending_file_name.contains("..")
    {
        return Err(Error::new("receipt transition lock identity is invalid"));
    }
    require_digest(
        &lock.target_receipt_sha256,
        "receipt transition target SHA-256",
    )?;
    if let Some(expected) = &lock.expected_receipt_sha256 {
        require_digest(expected, "receipt transition prior SHA-256")?;
    }
    let live_owner = process_start_token(lock.owner_pid)?;
    let current_pid = std::process::id();
    let current_start = process_start_token(current_pid)?;
    if live_owner.as_deref() == Some(lock.owner_start_token.as_str())
        && (lock.owner_pid != current_pid
            || current_start.as_deref() != Some(lock.owner_start_token.as_str()))
    {
        return Err(Error::new(format!(
            "receipt transition is still owned by exact live process {} ({})",
            lock.owner_pid, lock.owner_start_token
        )));
    }
    let pending = parent.join(&lock.pending_file_name);
    let current = match read_existing_regular_receipt(path, "published receipt during recovery")? {
        Some(bytes) => Some(canonical_embedded_receipt_sha256(
            &bytes,
            "published receipt during recovery",
        )?),
        None => None,
    };
    if current.as_deref() != Some(lock.target_receipt_sha256.as_str()) {
        if current.as_deref() != lock.expected_receipt_sha256.as_deref() {
            return Err(Error::new(
                "stale receipt transition found a published receipt other than its exact prior or target; no bytes were overwritten",
            ));
        }
        let pending_bytes = read_regular_reference(&pending, "receipt transition pending file")?;
        if canonical_embedded_receipt_sha256(&pending_bytes, "pending receipt during recovery")?
            != lock.target_receipt_sha256
        {
            return Err(Error::new(
                "stale receipt transition pending file differs from its durable target",
            ));
        }
        publish_receipt_transition(path, &pending, lock.expected_receipt_sha256.as_deref())?;
    }
    if pending.exists() {
        fs::remove_file(&pending)?;
    }
    fs::remove_file(lock_path)?;
    sync_parent_directory(parent)
}

/// Publishes one receipt transition under a crash-recoverable exact-content CAS.
fn write_json_atomic_receipt_cas<T: Serialize>(
    path: &Path,
    value: &T,
    expected_receipt_sha256: Option<&str>,
) -> Result<()> {
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
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let target = canonical_embedded_receipt_sha256(&bytes, "new receipt transition")?;
    let lock_path = parent.join(format!(".{file_name}.lock"));
    if lock_path.exists() {
        recover_receipt_transition(path, &lock_path)?;
    }
    if let Some(expected) = expected_receipt_sha256 {
        require_digest(expected, "prior receipt SHA-256")?;
    }
    let current = match read_existing_regular_receipt(path, "prior receipt during CAS")? {
        Some(bytes) => Some(canonical_embedded_receipt_sha256(
            &bytes,
            "prior receipt during CAS",
        )?),
        None => None,
    };
    if current.as_deref() == Some(target.as_str()) {
        return Ok(());
    }
    if current.as_deref() != expected_receipt_sha256 {
        return Err(Error::new(
            "receipt changed since this transition loaded it; exact-content stale writer refused",
        ));
    }
    let pending_file_name = format!(".{file_name}.{target}.receipt-pending");
    let pending = parent.join(&pending_file_name);
    match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&pending)
    {
        Ok(mut file) => {
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if read_regular_reference(&pending, "existing receipt pending file")? != bytes {
                return Err(Error::new(
                    "existing receipt pending file has substituted bytes",
                ));
            }
        }
        Err(error) => return Err(error.into()),
    }
    sync_parent_directory(parent)?;
    let owner_pid = std::process::id();
    let owner_start_token = process_start_token(owner_pid)?
        .ok_or_else(|| Error::new("cannot bind receipt transition to current process start"))?;
    let lock = ReceiptTransitionLockV1 {
        schema: RECEIPT_TRANSITION_LOCK_SCHEMA.into(),
        owner_pid,
        owner_start_token,
        expected_receipt_sha256: expected_receipt_sha256.map(str::to_owned),
        target_receipt_sha256: target,
        pending_file_name,
    };
    let candidate = parent.join(format!(
        ".{file_name}.{owner_pid}.{}.lock-candidate",
        lock.target_receipt_sha256
    ));
    let candidate_bytes = serde_json::to_vec_pretty(&lock)?;
    match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&candidate)
    {
        Ok(mut file) => {
            file.write_all(&candidate_bytes)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut expected = candidate_bytes;
            expected.push(b'\n');
            if read_regular_reference(&candidate, "orphan receipt lock candidate")? != expected {
                return Err(Error::new(
                    "orphan receipt lock candidate has substituted bytes",
                ));
            }
        }
        Err(error) => return Err(error.into()),
    }
    sync_parent_directory(parent)?;
    fs::hard_link(&candidate, &lock_path).map_err(|error| {
        Error::new(format!(
            "receipt transition lock acquisition failed: {error}"
        ))
    })?;
    sync_parent_directory(parent)?;
    fs::remove_file(&candidate)?;
    publish_receipt_transition(path, &pending, expected_receipt_sha256)?;
    if pending.exists() {
        fs::remove_file(&pending)?;
    }
    fs::remove_file(&lock_path)?;
    sync_parent_directory(parent)
}

fn sync_parent_directory(parent: &Path) -> Result<()> {
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn require_role(role: &str) -> Result<()> {
    if !CHECKED_ROLE_ORDER_V1.contains(&role) {
        return Err(Error::new(format!(
            "unknown Upgrade role {role:?}; one run names exactly one of {}",
            CHECKED_ROLE_ORDER_V1.join(", ")
        )));
    }
    Ok(())
}

fn role_ordinal(role: &str) -> Result<u8> {
    CHECKED_ROLE_ORDER_V1
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

    use dclutch_release_tool::{BuildMetadataV1, ReleaseEvidenceV1, build_checked_release};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let requested = std::env::temp_dir().join(format!(
                "dclutch-devnet-upgrade-v1-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&requested).expect("create test directory");
            // macOS commonly exposes its temporary root through `/var`,
            // while `canonicalize` resolves that path beneath `/private/var`.
            // Exercise the production exact-path checks with the real
            // canonical coordinate instead of accidentally testing the host
            // alias before the intended interruption or refusal boundary.
            let path = fs::canonicalize(&requested).expect("canonical test directory");
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
        reported_authority: Pubkey,
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
        send_success: bool,
        send_lands_then_errors: bool,
        sign_success: bool,
        deploy_program: Pubkey,
        dump: Vec<u8>,
        upgrade_fee_lamports: u64,
        upgrade_transaction_override: Option<Value>,
        preserve_upgrade_override_signature: bool,
        extension_fee_lamports: u64,
        extension_instruction_bytes: Option<u64>,
        buffer_pubkey: Pubkey,
        buffer_written: bool,
        buffer_payload: Vec<u8>,
        buffer_rent_lamports: u64,
        buffer_upload_fee_lamports: u64,
        finalized_height: u64,
        authority_signer: Keypair,
        payer_signer: Keypair,
        send_count: u64,
        sign_count: u64,
        buffer_writer_alive: bool,
        crash_after_buffer_lease: bool,
        buffer_writer_restart_without_handle: bool,
        signature_status_override: Option<JournaledSignatureStatusV1>,
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
                reported_authority: fixture.authority,
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
                send_success: true,
                send_lands_then_errors: false,
                sign_success: true,
                deploy_program: fixture.args.buffer_pubkey,
                dump: fixture.raw_elf.clone(),
                upgrade_fee_lamports: 15_000,
                upgrade_transaction_override: None,
                preserve_upgrade_override_signature: false,
                extension_fee_lamports: 5_000,
                extension_instruction_bytes: None,
                buffer_pubkey: fixture.args.buffer_pubkey,
                buffer_written: false,
                buffer_payload: fixture.raw_elf.clone(),
                buffer_rent_lamports: 100_000,
                buffer_upload_fee_lamports: 0,
                finalized_height: 1_000,
                authority_signer: fixture.authority_signer.insecure_clone(),
                payer_signer: fixture.payer_signer.insecure_clone(),
                send_count: 0,
                sign_count: 0,
                buffer_writer_alive: false,
                crash_after_buffer_lease: false,
                buffer_writer_restart_without_handle: false,
                signature_status_override: None,
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
                    } else if self.buffer_written {
                        self.before_wallet
                            - self.buffer_rent_lamports
                            - self.buffer_upload_fee_lamports
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
                    let address = if path.contains("buffer") {
                        self.buffer_pubkey
                    } else if path.contains("authority") {
                        self.reported_authority
                    } else {
                        self.payer
                    };
                    Ok(success(format!("{address}\n")))
                }
                Some("program") if arguments.get(1).map(String::as_str) == Some("write-buffer") => {
                    if !self.deploy_success {
                        return Ok(CliOutput {
                            success: false,
                            stdout: String::new(),
                            stderr: "synthetic write-buffer interruption".into(),
                        });
                    }
                    self.buffer_written = true;
                    Ok(success(
                        json!({
                            "buffer": self.deploy_program.to_string()
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
            let buffer = query.buffer;
            let spill = query.payer;
            let mut transaction = self.upgrade_transaction_override.clone().unwrap_or_else(|| {
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
                            self.before_wallet
                                - self.buffer_rent_lamports
                                - self.buffer_upload_fee_lamports,
                            query.programdata_before_lamports,
                            1_140,
                            0,
                            self.buffer_rent_lamports,
                            1,
                            1
                        ],
                        "postBalances": [
                            self.after_wallet,
                            query.programdata_after_lamports,
                            1_140,
                            0,
                            0,
                            1,
                            1
                        ]
                    }
                })
            });
            if self.upgrade_transaction_override.is_some()
                && !self.preserve_upgrade_override_signature
            {
                transaction["transaction"]["signatures"][0] = json!(query.signature);
            }
            validate_upgrade_transaction(query, transaction)
        }

        fn resolve_extension_transaction(
            &mut self,
            query: &ExtensionTransactionQueryV1<'_>,
        ) -> Result<ExtensionTransactionEvidenceV1> {
            let signature = query.signature.to_owned();
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

        fn prepare_loader_action(
            &mut self,
            query: &LoaderActionQueryV1<'_>,
        ) -> Result<UnsignedLoaderActionV1> {
            let mut blockhash_bytes = [44; 32];
            blockhash_bytes[0] = u8::try_from(self.finalized_height % 251).expect("height byte");
            let blockhash = Hash::new_from_array(blockhash_bytes);
            let message = loader_action_message(query, blockhash)?;
            let bytes = bincode::serialize(&message)
                .map_err(|error| Error::new(format!("fake message: {error}")))?;
            Ok(UnsignedLoaderActionV1 {
                message_base64: BASE64.encode(&bytes),
                message_sha256: digest(&bytes),
                recent_blockhash: blockhash.to_string(),
                last_valid_block_height: self.finalized_height + 150,
            })
        }

        fn sign_loader_action(
            &mut self,
            query: &LoaderActionQueryV1<'_>,
            unsigned: &UnsignedLoaderActionV1,
        ) -> Result<SignedLoaderActionV1> {
            if !self.sign_success {
                return Err(Error::new("synthetic crash before packet signing"));
            }
            self.sign_count += 1;
            let message = authenticate_unsigned_loader_action(query, unsigned)?;
            let mut transaction = Transaction::new_unsigned(message);
            transaction
                .try_sign(
                    &[&self.payer_signer, &self.authority_signer],
                    transaction.message.recent_blockhash,
                )
                .map_err(|error| Error::new(format!("fake sign: {error}")))?;
            let packet = bincode::serialize(&transaction)
                .map_err(|error| Error::new(format!("fake packet: {error}")))?;
            Ok(SignedLoaderActionV1 {
                packet_base64: BASE64.encode(&packet),
                packet_sha256: digest(&packet),
                signature: transaction.signatures[0].to_string(),
            })
        }

        fn send_loader_action(
            &mut self,
            _origin: &ClusterOriginV1,
            signed: &SignedLoaderActionV1,
        ) -> Result<String> {
            if !self.send_success {
                return Err(Error::new("synthetic packet send interruption"));
            }
            self.send_count += 1;
            self.deployed = true;
            if self.send_lands_then_errors {
                return Err(Error::new("synthetic lost send response after landing"));
            }
            Ok(signed.signature.clone())
        }

        fn finalized_block_height(&mut self, _origin: &ClusterOriginV1) -> Result<u64> {
            Ok(self.finalized_height)
        }

        fn journaled_signature_status(
            &mut self,
            _origin: &ClusterOriginV1,
            _signature: &str,
        ) -> Result<JournaledSignatureStatusV1> {
            if let Some(status) = &self.signature_status_override {
                return Ok(status.clone());
            }
            Ok(if self.deployed {
                JournaledSignatureStatusV1::FinalizedSuccess
            } else {
                JournaledSignatureStatusV1::NotFound
            })
        }

        fn read_buffer(
            &mut self,
            _origin: &ClusterOriginV1,
            buffer: Pubkey,
            authority: Pubkey,
            _minimum_context_slot: u64,
        ) -> Result<Option<BufferObservationV1>> {
            assert_eq!(buffer, self.buffer_pubkey);
            assert_eq!(authority, self.authority);
            if !self.buffer_written {
                return Ok(None);
            }
            let mut data = vec![0_u8; BUFFER_METADATA_BYTES];
            data[..4].copy_from_slice(&1_u32.to_le_bytes());
            data[4] = 1;
            data[5..BUFFER_METADATA_BYTES].copy_from_slice(authority.as_ref());
            data.extend_from_slice(&self.buffer_payload);
            Ok(Some(BufferObservationV1 {
                context_slot: self.before_context_slot + 1,
                lamports: self.buffer_rent_lamports,
                data_bytes: u64::try_from(data.len()).expect("fake Buffer width"),
                account_sha256: digest(&data),
                authority: authority.to_string(),
                payload_sha256: digest(&data[BUFFER_METADATA_BYTES..]),
            }))
        }

        fn minimum_balance_for_rent_exemption(
            &mut self,
            _origin: &ClusterOriginV1,
            _data_bytes: u64,
        ) -> Result<u64> {
            Ok(self.buffer_rent_lamports)
        }

        fn authenticate_buffer_upload(
            &mut self,
            query: &BufferUploadQueryV1<'_>,
        ) -> Result<BufferUploadEvidenceV1> {
            assert_eq!(query.buffer, self.buffer_pubkey);
            assert_eq!(query.authority, self.authority);
            assert_eq!(query.payer, self.payer);
            assert_eq!(query.raw_elf, self.buffer_payload.as_slice());
            assert_eq!(query.expected_rent_lamports, self.buffer_rent_lamports);
            let transactions = vec![json!({"fixture": "exact-buffer-upload"})];
            Ok(BufferUploadEvidenceV1 {
                transactions_sha256: digest(
                    &serde_json::to_vec(&transactions).expect("fake upload JSON"),
                ),
                transactions,
                fee_lamports: self.buffer_upload_fee_lamports,
            })
        }

        fn start_buffer_writer(
            &mut self,
            query: &BufferWriterQueryV1<'_>,
        ) -> Result<BufferWriterLeaseV1> {
            self.buffer_writer_alive = true;
            Ok(BufferWriterLeaseV1 {
                schema: BUFFER_WRITER_LEASE_SCHEMA.into(),
                operation_id: query.operation_id.into(),
                attempt_ordinal: query.attempt_ordinal,
                pid: 41,
                process_group_id: 41,
                process_start_token: "synthetic-start-token".into(),
                process_nonce: "41".repeat(32),
                command_sha256: query.command_sha256.into(),
                lease_path: query.lease_path.to_string_lossy().into_owned(),
                permit_path: query.permit_path.to_string_lossy().into_owned(),
            })
        }

        fn buffer_writer_status(
            &mut self,
            _lease: &BufferWriterLeaseV1,
        ) -> Result<BufferWriterStatusV1> {
            Ok(if self.buffer_writer_alive {
                BufferWriterStatusV1::AliveStopped
            } else {
                BufferWriterStatusV1::Exited
            })
        }

        fn continue_buffer_writer(
            &mut self,
            query: &BufferWriterQueryV1<'_>,
            _lease: &BufferWriterLeaseV1,
        ) -> Result<CliOutput> {
            if self.crash_after_buffer_lease {
                return Err(Error::new(
                    "synthetic crash after durable Buffer writer lease",
                ));
            }
            if self.buffer_writer_restart_without_handle {
                return Err(Error::new(
                    "exact leased Buffer writer is alive after operator restart",
                ));
            }
            let output = self.run(query.command_arguments)?;
            self.buffer_writer_alive = false;
            Ok(output)
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
        authority_signer: Keypair,
        payer_signer: Keypair,
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

    fn synthetic_sbf_elf(seed: u8) -> Vec<u8> {
        let mut elf = vec![0_u8; 64];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[6] = 1;
        elf[16..18].copy_from_slice(&3_u16.to_le_bytes());
        elf[18..20].copy_from_slice(&263_u16.to_le_bytes());
        elf[20..24].copy_from_slice(&1_u32.to_le_bytes());
        elf[52..54].copy_from_slice(&64_u16.to_le_bytes());
        elf[63] = seed;
        elf
    }

    fn checked_build_manifest(
        role: &str,
        package: &str,
        ordinal: usize,
        elf: &[u8],
        source_revision: &str,
        source_tree_sha256: &str,
        solana_cli_version: &str,
    ) -> Vec<u8> {
        let seed = u8::try_from(ordinal).expect("fixture link ordinal") + 1;
        let program_id = [seed; 32];
        let programdata_id = [seed + 32; 32];
        let loader = bpf_loader_upgradeable::ID.to_bytes();
        let mut program = [0_u8; 36];
        program[..4].copy_from_slice(&2_u32.to_le_bytes());
        program[4..].copy_from_slice(&programdata_id);
        let mut programdata = vec![0_u8; 45];
        programdata[..4].copy_from_slice(&3_u32.to_le_bytes());
        programdata.extend_from_slice(elf);
        let semantic_preimage = format!(
            "dclutch/checked-release-candidate/unowned-semantic-release/v1\nrole={role}\npackage={package}\nsource_revision={source_revision}\n"
        );
        let metadata = BuildMetadataV1::parse(&format!(
            concat!(
                "dclutch-release-metadata-v1\n",
                "semantic_kind=unowned\n",
                "program_id={}\n",
                "programdata_id={}\n",
                "loader_program_id={}\n",
                "program_owner={}\n",
                "program_executable=true\n",
                "programdata_owner={}\n",
                "programdata_executable=false\n",
                "source_digest={}\n",
                "cargo_lock_digest={}\n",
                "source_revision={}\n",
                "rustc_version=rustc 1.90.0 (fixture)\n",
                "solana_version={}\n",
                "cargo_build_sbf_version=cargo-build-sbf 4.0.2 (fixture)\n",
                "target_triple=sbpf-solana-solana\n",
                "build_command=cargo build-sbf --manifest-path programs/{}/Cargo.toml -- --locked\n",
                "assumption=synthetic checked-build evidence is scoped to this hostile unit test\n",
            ),
            hex(&program_id),
            hex(&programdata_id),
            hex(&loader),
            hex(&loader),
            hex(&loader),
            source_tree_sha256,
            "ef".repeat(32),
            source_revision,
            solana_cli_version,
            package,
        ))
        .expect("canonical fixture build metadata");
        build_checked_release(ReleaseEvidenceV1 {
            elf,
            semantic_preimage: semantic_preimage.as_bytes(),
            program_account_data: &program,
            programdata_account_data: &programdata,
            metadata: &metadata,
        })
        .expect("fixture checked build release")
        .encode()
        .expect("encode fixture checked build release")
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
            .enumerate()
            .map(|(ordinal, (label, package, produces_artifact))| {
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
                    let bytes = if *label == "custody" {
                        raw_elf.to_vec()
                    } else {
                        synthetic_sbf_elf(
                            u8::try_from(ordinal).expect("fixture link ordinal") + 1,
                        )
                    };
                    let checked = checked_build_manifest(
                        label,
                        package,
                        ordinal,
                        &bytes,
                        "0123456789abcdef0123456789abcdef01234567",
                        &source_tree.sha256,
                        solana_cli_version,
                    );
                    (
                        Some(write_gate_evidence(
                            root,
                            &format!("elf/{label}.so"),
                            &bytes,
                        )),
                        Some(write_gate_evidence(
                            root,
                            &format!("evidence/{label}/checked.bin"),
                            &checked,
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
            let program = parse_pubkey(
                PERMANENT_DEVNET_UPGRADE_TARGETS_V1[2].1,
                "fixture Custody Program",
            )
            .expect("Custody Program");
            let programdata = parse_pubkey(
                PERMANENT_DEVNET_UPGRADE_TARGETS_V1[2].2,
                "fixture Custody ProgramData",
            )
            .expect("Custody ProgramData");
            let authority_signer = Keypair::new();
            let payer_signer = Keypair::new();
            let authority = authority_signer.pubkey();
            let payer = payer_signer.pubkey();
            let raw_elf = synthetic_sbf_elf(3);
            let mut candidate_live = raw_elf.clone();
            candidate_live.extend_from_slice(&[0; 4]);
            let mut before_live = synthetic_sbf_elf(4);
            before_live.extend_from_slice(&[0; 4]);
            let solana_cli_version = "solana-cli 4.0.2 (test fixture)".to_owned();
            let gate = checked_gate(&directory.0, &raw_elf, &solana_cli_version);
            let elf_path = directory.0.join("elf/custody.so");
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
                canonical_role_order: CHECKED_ROLE_ORDER_V1
                    .iter()
                    .map(|role| String::from(*role))
                    .collect(),
                role_ordinal: role_ordinal("custody").expect("role"),
                role: "custody".into(),
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
            let mut args = UpgradeArgsV1 {
                origin: ClusterOriginV1::parse(
                    "https://api.devnet.solana.com",
                    Some(DEVNET_GENESIS_HASH),
                )
                .expect("devnet origin"),
                role: "custody".into(),
                program_id: program,
                programdata_id: programdata,
                expected_upgrade_authority: authority,
                // These files deliberately do not exist. The implementation
                // may pass their paths to the fake CLI; it may never read them.
                authority_keypair: directory.0.join("missing-authority-keypair.json"),
                fee_payer: payer,
                fee_payer_keypair: directory.0.join("missing-payer-keypair.json"),
                buffer_pubkey: Pubkey::new_from_array([11; 32]),
                buffer_keypair: directory.0.join("missing-buffer-keypair.json"),
                deployment_set_journal_path: directory.0.join("deployment-set.json"),
                elf_path,
                checked_release_gate_path: gate_path,
                expected_checked_release_gate_sha256: gate_sha256,
                expected_source_revision: gate.source_revision.clone(),
                expected_source_tree_sha256: gate.source_tree_sha256.clone(),
                baseline_path,
                receipt_path: directory.0.join("receipt.json"),
                dump_path: directory.0.join("dump.so"),
                solana_cli: directory.0.join("missing-solana-cli"),
                target_acknowledgment: format!("custody:{program}"),
                exclusive_payer_window_acknowledgment: Some(format!("custody:{program}:{payer}")),
                execute: true,
                preflight: false,
            };
            let carry_path = directory.0.join("carry-forward.json");
            fs::write(&carry_path, b"{}\n").expect("write carry placeholder");
            let roles = PERMANENT_DEVNET_UPGRADE_TARGETS_V1
                .iter()
                .enumerate()
                .map(|(index, (role, program_id, programdata_id))| {
                    let future_baseline_path =
                        directory.0.join(format!("future-{role}-baseline.json"));
                    let role_receipt_path = if index == 2 {
                        args.receipt_path.clone()
                    } else {
                        directory.0.join(format!("{role}-receipt.json"))
                    };
                    let role_dump_path = if index == 2 {
                        args.dump_path.clone()
                    } else {
                        directory.0.join(format!("{role}-dump.so"))
                    };
                    let baseline = (index >= 2).then(|| {
                        if index == 2 {
                            SetPinnedFileV1 {
                                canonical_path: path_argument(&args.baseline_path, "baseline")
                                    .expect("baseline path"),
                                sha256: baseline.baseline_sha256.clone(),
                            }
                        } else {
                            SetPinnedFileV1 {
                                canonical_path: path_argument(
                                    &future_baseline_path,
                                    "future baseline",
                                )
                                .expect("future baseline path"),
                                sha256: digest(format!("future-{role}").as_bytes()),
                            }
                        }
                    });
                    UpgradeSetRoleV1 {
                        role: (*role).into(),
                        disposition: if index < 2 {
                            CheckedDeploymentDispositionV1::CarryForward
                        } else {
                            CheckedDeploymentDispositionV1::Upgrade
                        },
                        program_id: (*program_id).into(),
                        programdata_id: (*programdata_id).into(),
                        baseline,
                        receipt: SetOptionalFileV1 {
                            canonical_path: path_argument(&role_receipt_path, "role receipt")
                                .expect("receipt path"),
                            sha256: None,
                        },
                        dump: SetOptionalFileV1 {
                            canonical_path: path_argument(&role_dump_path, "role dump")
                                .expect("dump path"),
                            sha256: None,
                        },
                    }
                })
                .collect();
            let journal = UpgradeSetJournalV1 {
                schema: SET_JOURNAL_SCHEMA.into(),
                checked_release_gate: SetPinnedFileV1 {
                    canonical_path: path_argument(&args.checked_release_gate_path, "gate")
                        .expect("gate path"),
                    sha256: args.expected_checked_release_gate_sha256.clone(),
                },
                source_revision: args.expected_source_revision.clone(),
                source_tree_sha256: args.expected_source_tree_sha256.clone(),
                devnet_genesis_hash: DEVNET_GENESIS_HASH.into(),
                solana_cli_version: solana_cli_version.clone(),
                retained_upgrade_authority: authority.to_string(),
                fee_payer: payer.to_string(),
                infrastructure_carry_forward: SetPinnedFileV1 {
                    canonical_path: path_argument(&carry_path, "carry").expect("carry path"),
                    sha256: digest(&fs::read(&carry_path).expect("carry bytes")),
                },
                roles,
            };
            fs::write(
                &args.deployment_set_journal_path,
                serde_json::to_vec_pretty(&journal).expect("journal JSON"),
            )
            .expect("write journal");
            args.deployment_set_journal_path =
                fs::canonicalize(&args.deployment_set_journal_path).expect("canonical journal");
            Self {
                _directory: directory,
                args,
                gate,
                solana_cli_version,
                program,
                programdata,
                authority,
                payer,
                authority_signer,
                payer_signer,
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
                deployment_set_journal_path: self.args.deployment_set_journal_path.clone(),
                baseline_path: self.args.baseline_path.clone(),
                receipt_path: self._directory.0.join("extension-receipt.json"),
                solana_cli: self.args.solana_cli.clone(),
                expected_solana_cli_version: self.solana_cli_version.clone(),
                target_acknowledgment: format!("custody:{}:+{additional_bytes}", self.program),
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
        buffer: Pubkey,
        slot: u64,
        programdata_lamports: u64,
        fee: u64,
    ) -> Value {
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
                                "spillAccount": payer.to_string(),
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
                    999_999, programdata_lamports, 1_140, 0, 1, 1, 1
                ],
                "postBalances": [
                    1_000_000 - fee, programdata_lamports, 1_140, 0, 0, 1, 1
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
        fn enforces_fresh_deployment_set_boundary(&self) -> bool {
            true
        }

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
                    canonical_role_order: CHECKED_ROLE_ORDER_V1
                        .iter()
                        .map(|role| String::from(*role))
                        .collect(),
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
                    let receipt_buffer = Pubkey::new_from_array([90 + byte; 32]);
                    let transaction = set_upgrade_transaction(
                        &signature,
                        program,
                        programdata,
                        authority,
                        payer,
                        receipt_buffer,
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
                        deployment_set_journal_sha256: digest(b"mixed-set-journal"),
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
                        buffer_pubkey: receipt_buffer.to_string(),
                        buffer_keypair_path: format!("mixed-set/{role}-buffer-keypair.json"),
                        buffer_data_bytes: u64::try_from(BUFFER_METADATA_BYTES + raw_elf.len())
                            .expect("buffer width"),
                        buffer_rent_exempt_lamports: 1,
                        buffer_write_armed_block_height: Some(context_slot),
                        buffer_write_expiry_block_height: Some(context_slot + 512),
                        buffer_write_cli_output: Some(json!({"status": "fixture"})),
                        buffer_ready_context_slot: Some(context_slot),
                        buffer_ready_account_sha256: Some(digest(&raw_elf)),
                        buffer_ready_lamports: Some(1),
                        buffer_upload_fee_lamports: Some(0),
                        buffer_upload_transactions: Some(Vec::new()),
                        buffer_upload_transactions_sha256: Some(digest(b"[]")),
                        buffer_upload_wallet_after_lamports: Some(999_999),
                        buffer_write_attempts: Vec::new(),
                        expired_packets: Vec::new(),
                        unsigned_message_base64: Some(BASE64.encode(b"fixture-message")),
                        unsigned_message_sha256: Some(digest(b"fixture-message")),
                        recent_blockhash: Some(Hash::new_from_array([91 + byte; 32]).to_string()),
                        last_valid_block_height: Some(context_slot + 150),
                        signed_packet_base64: Some(BASE64.encode(b"fixture-packet")),
                        signed_packet_sha256: Some(digest(b"fixture-packet")),
                        transaction_signature: Some(signature.clone()),
                        solana_cli_output: Some(json!({
                            "transport": "sendTransaction",
                            "maxRetries": 0,
                            "signature": signature,
                            "buffer": receipt_buffer.to_string(),
                            "spill": payer.to_string(),
                        })),
                        finalized_transaction: Some(transaction.clone()),
                        finalized_transaction_sha256: Some(digest(
                            &serde_json::to_vec(&transaction).expect("transaction JSON"),
                        )),
                        after_context_slot: Some(context_slot + 1),
                        after: Some(after.clone()),
                        arithmetic: Some(UpgradeArithmeticV1 {
                            transaction_payer_pre_lamports: 999_999,
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
                    let (lease_path, permit_path) = buffer_writer_paths(
                        &UpgradeArgsV1 {
                            receipt_path: receipt_path.clone(),
                            ..fixture.args.clone()
                        },
                        &receipt.operation_id,
                        0,
                    )
                    .expect("fixture Buffer writer paths");
                    receipt.buffer_write_attempts.push(BufferWriteAttemptV1 {
                        lease: BufferWriterLeaseV1 {
                            schema: BUFFER_WRITER_LEASE_SCHEMA.into(),
                            operation_id: receipt.operation_id.clone(),
                            attempt_ordinal: 0,
                            pid: 41,
                            process_group_id: 41,
                            process_start_token: format!("fixture-{role}"),
                            process_nonce: digest(role.as_bytes()),
                            command_sha256: digest(b"fixture-buffer-command"),
                            lease_path: lease_path.to_string_lossy().into_owned(),
                            permit_path: permit_path.to_string_lossy().into_owned(),
                        },
                        armed_finalized_block_height: context_slot,
                        expiry_finalized_block_height: context_slot + BUFFER_WRITE_EXPIRY_BLOCKS,
                        exit_observed_finalized_block_height: Some(context_slot),
                        exit_disposition: Some("returned_success".into()),
                    });
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
            hex(&RESOLUTION_CONTROLLER_RELEASE_ID_V7)
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
        let in_flight = MixedSetFixture::new(0);
        let in_flight_path = PathBuf::from(&in_flight.journal.roles[2].receipt.canonical_path);
        let outside = in_flight
            ._fixture
            ._directory
            .0
            .join("outside-in-flight-receipt.json");
        fs::write(&outside, b"{}\n").expect("outside in-flight receipt");
        std::os::unix::fs::symlink(&outside, &in_flight_path).expect("in-flight receipt symlink");
        assert!(
            load_set_journal_path(&in_flight.args.journal_path)
                .expect_err("unpinned in-flight receipt symlink")
                .to_string()
                .contains("regular non-symlink")
        );

        let mut aliases = MixedSetFixture::new(0);
        aliases.journal.roles[3].receipt.canonical_path =
            aliases.journal.roles[2].dump.canonical_path.clone();
        aliases.rewrite_journal();
        assert!(
            load_set_journal_path(&aliases.args.journal_path)
                .expect_err("future receipt cannot alias another role's future dump")
                .to_string()
                .contains("paths alias")
        );

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

        let mut substituted_prior = MixedSetFixture::new(2);
        substituted_prior.journal.roles.swap(2, 3);
        let custody_identity = PERMANENT_DEVNET_UPGRADE_TARGETS_V1[2];
        let resolution_identity = PERMANENT_DEVNET_UPGRADE_TARGETS_V1[3];
        substituted_prior.journal.roles[2].role = custody_identity.0.into();
        substituted_prior.journal.roles[2].program_id = custody_identity.1.into();
        substituted_prior.journal.roles[2].programdata_id = custody_identity.2.into();
        substituted_prior.journal.roles[3].role = resolution_identity.0.into();
        substituted_prior.journal.roles[3].program_id = resolution_identity.1.into();
        substituted_prior.journal.roles[3].programdata_id = resolution_identity.2.into();
        substituted_prior.rewrite_journal();
        let error =
            audit_set_journal_with_runner(&substituted_prior.args, &mut substituted_prior.runner)
                .expect_err(
                    "prior Complete receipt cannot be substituted under another permanent row",
                );
        assert!(
            error.to_string().contains("existing Upgrade receipt")
                || error.to_string().contains("exact role")
                || error.to_string().contains("baseline")
        );

        let mut baseline_drift = MixedSetFixture::new(0);
        let baseline_path = PathBuf::from(
            &baseline_drift.journal.roles[2]
                .baseline
                .as_ref()
                .expect("Custody baseline")
                .canonical_path,
        );
        OpenOptions::new()
            .append(true)
            .open(&baseline_path)
            .expect("baseline")
            .write_all(b"\n")
            .expect("baseline drift");
        let error = audit_set_journal_with_runner(&baseline_drift.args, &mut baseline_drift.runner)
            .expect_err("target baseline bytes must remain pinned");
        assert!(error.to_string().contains("baseline SHA-256"));
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
        assert_eq!(receipt.role, "custody");
        assert_eq!(receipt.program_id, fixture.program.to_string());
        assert_eq!(
            receipt.checked_release_gate_sha256,
            fixture.args.expected_checked_release_gate_sha256
        );
        assert_eq!(
            receipt.source_tree_sha256,
            fixture.args.expected_source_tree_sha256
        );
        Signature::from_str(
            receipt
                .transaction_signature
                .as_deref()
                .expect("journaled Upgrade signature"),
        )
        .expect("valid Upgrade signature");
        assert_eq!(receipt.after.as_ref().expect("after").deployment_slot, 92);
        assert_eq!(
            receipt.arithmetic,
            Some(UpgradeArithmeticV1 {
                transaction_payer_pre_lamports: 900_000,
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
                && call.get(1).map(String::as_str) == Some("write-buffer")
                && call.contains(&"--max-sign-attempts".to_owned())
                && call.contains(&"1".to_owned())
        }));
        assert!(
            runner
                .snapshot_minimum_slots
                .iter()
                .all(|slot| *slot >= 489_212_834)
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
    fn operation_accounting_attributes_legacy_net_cost_field_to_buffer_fees() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.after_wallet = 980_000;
        runner.buffer_upload_fee_lamports = 5_000;
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
                buffer: fixture.args.buffer_pubkey,
                buffer_lamports: runner.buffer_rent_lamports,
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
        assert!(
            error
                .to_string()
                .contains("Buffer refund minus finalized fee"),
            "{error}"
        );

        let signature = Fixture::new();
        let mut signature_runner = FakeRunner::new(&signature);
        let mut signature_transaction = make_transaction(&signature, &mut signature_runner);
        signature_transaction["transaction"]["signatures"][0] =
            json!(Signature::from([9_u8; 64]).to_string());
        signature_runner.upgrade_transaction_override = Some(signature_transaction);
        signature_runner.preserve_upgrade_override_signature = true;
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
        let gate = validate_checked_release_gate(&fixture.args).expect("canonical gate");
        let mut hostile_history = accepted.clone();
        let prior = hostile_history
            .buffer_write_attempts
            .first_mut()
            .expect("first Buffer attempt");
        prior.exit_observed_finalized_block_height = None;
        prior.exit_disposition = None;
        let mut successor = prior.clone();
        successor.lease.attempt_ordinal = 1;
        successor.lease.pid = 42;
        successor.lease.process_group_id = 42;
        successor.lease.process_start_token = "synthetic-second-start".into();
        successor.lease.process_nonce = "42".repeat(32);
        let (lease_path, permit_path) = buffer_writer_paths(
            &fixture.args,
            &hostile_history.operation_id,
            successor.lease.attempt_ordinal,
        )
        .expect("second attempt paths");
        successor.lease.lease_path = lease_path.to_string_lossy().into_owned();
        successor.lease.permit_path = permit_path.to_string_lossy().into_owned();
        successor.armed_finalized_block_height = prior.expiry_finalized_block_height + 1;
        successor.expiry_finalized_block_height =
            successor.armed_finalized_block_height + BUFFER_WRITE_EXPIRY_BLOCKS;
        successor.exit_observed_finalized_block_height =
            Some(successor.armed_finalized_block_height);
        successor.exit_disposition = Some("returned_success".into());
        hostile_history.buffer_write_armed_block_height =
            Some(successor.armed_finalized_block_height);
        hostile_history.buffer_write_expiry_block_height =
            Some(successor.expiry_finalized_block_height);
        hostile_history.buffer_write_attempts.push(successor);
        assert!(
            validate_buffer_write_attempts(&fixture.args, &gate, &hostile_history)
                .expect_err("superseded attempt without exit boundary")
                .to_string()
                .contains("superseded Buffer writer attempt")
        );

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
        assert!(
            error.to_string().contains("armed Buffer receipt"),
            "{error}"
        );
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
        assert!(error.to_string().contains("exact target custody:"));
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
    fn localhost_projection_is_the_exact_upgrade_gate_validator() {
        let fixture = Fixture::new();
        let upgrade = validate_checked_release_gate(&fixture.args).expect("canonical gate");
        let local = authenticate_checked_release_gate_role_for_local_v1(
            &fixture.args.checked_release_gate_path,
            &fixture.args.expected_checked_release_gate_sha256,
            &fixture.args.expected_source_revision,
            &fixture.args.expected_source_tree_sha256,
            &fixture.args.role,
            &fixture.args.elf_path,
        )
        .expect("localhost projection");
        assert_eq!(local.gate_sha256, upgrade.gate_sha256);
        assert_eq!(local.source_revision, upgrade.source_revision);
        assert_eq!(local.source_tree_sha256, upgrade.source_tree_sha256);
        assert_eq!(local.solana_cli_version, upgrade.solana_cli_version);
        assert_eq!(local.raw_elf_sha256, digest(&fixture.raw_elf));

        let error = authenticate_checked_release_gate_role_for_local_v1(
            &fixture.args.checked_release_gate_path,
            &fixture.args.expected_checked_release_gate_sha256,
            &fixture.args.expected_source_revision,
            &fixture.args.expected_source_tree_sha256,
            "trading",
            &fixture.args.elf_path,
        )
        .expect_err("role substitution must refuse");
        assert!(
            error.to_string().contains("selected trading ELF path"),
            "{error}"
        );

        let orphan_source_elf = fixture._directory.0.join("dclutch_sbf.so");
        fs::write(&orphan_source_elf, b"\x7fELFhostile-source-role")
            .expect("write hostile orphan Source ELF");
        let error = authenticate_checked_release_gate_role_for_local_v1(
            &fixture.args.checked_release_gate_path,
            &fixture.args.expected_checked_release_gate_sha256,
            &fixture.args.expected_source_revision,
            &fixture.args.expected_source_tree_sha256,
            "resolution",
            &orphan_source_elf,
        )
        .expect_err("generic dclutch_sbf.so must not substitute for Resolution");
        assert!(
            error.to_string().contains(
                "selected resolution ELF path is not the gate's exact canonical role ELF"
            ),
            "{error}"
        );
    }

    #[test]
    fn checked_gate_materializes_seven_exact_mutable_local_loader_pairs() {
        let fixture = Fixture::new();
        let work = fixture._directory.0.join("checked-local-mutable");
        let plan_path = work.join("plan.json");
        let report = crate::local_mutable::prepare_local_mutable_v1(vec![
            "--work".into(),
            work.display().to_string(),
            "--output".into(),
            plan_path.display().to_string(),
            "--checked-release-gate".into(),
            fixture.args.checked_release_gate_path.display().to_string(),
            "--expected-checked-release-gate-sha256".into(),
            fixture.args.expected_checked_release_gate_sha256.clone(),
            "--expected-source-revision".into(),
            fixture.args.expected_source_revision.clone(),
            "--expected-source-tree-sha256".into(),
            fixture.args.expected_source_tree_sha256.clone(),
            "--seed".into(),
            "51".repeat(32),
        ])
        .expect("prepare checked mutable localhost substrate");
        let plan: crate::model::SuccessorPlan = serde_json::from_slice(
            &fs::read(&plan_path).expect("checked local mutable plan bytes"),
        )
        .expect("checked local mutable plan JSON");
        crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)
            .expect("reauthenticate checked local mutable plan");
        let loopback = crate::cluster::ClusterOriginV1::parse("http://127.0.0.1:20890/", None)
            .expect("owned loopback origin");
        crate::campaign::authenticate_checked_campaign_plan(&plan, &loopback)
            .expect("all seven ArtifactRelease rows and the profile rejoin the checked set");
        let checked = plan
            .checked_local_mutable_set
            .as_ref()
            .expect("checked local mutable set");
        assert_eq!(report.schema, "dclutch-local-mutable-prepare-report-v1");
        assert_eq!(checked.roles.len(), 7);
        assert_eq!(report.programs.len(), 7);
        assert_eq!(report.checked_local_mutable_set_sha256, checked.set_sha256);
        assert_eq!(
            fs::read_dir(&report.account_dir)
                .expect("local account directory")
                .count(),
            18
        );
        for (ordinal, role) in checked.roles.iter().enumerate() {
            assert_eq!(role.role, CHECKED_ROLE_ORDER_V1[ordinal]);
            assert_eq!(role.deployment_slot, ordinal as u64 + 1);
            assert!(plan.genesis_accounts.values().any(|account| {
                account.address == role.programdata_id
                    && account.data_sha256 == role.programdata_account_sha256
            }));
        }
        assert!(report.keypairs.len() >= crate::campaign::KEYPAIR_ROLES.len());

        let mut substituted_record = plan.clone();
        let rent_record = substituted_record
            .records
            .get("rent_artifact_release")
            .expect("rent ArtifactRelease")
            .clone();
        substituted_record.registry.artifact_release_id = rent_record.content_sha256.clone();
        substituted_record
            .records
            .insert("registry_artifact_release".into(), rent_record);
        let error =
            crate::campaign::authenticate_checked_campaign_plan(&substituted_record, &loopback)
                .expect_err("coherently substituted role record must refuse");
        assert!(
            error.to_string().contains("checked mutable slot pin"),
            "{error}"
        );

        let binding = |pin: &crate::model::ProgramPin| {
            dclutch_release_set_contract::ExecutionRoleBindingV1::new(
                dclutch_release_set_contract::ProgramIdentityV1::new(
                    crate::plan::pubkey(&pin.program_id)
                        .expect("program")
                        .to_bytes(),
                )
                .expect("program identity"),
                dclutch_release_set_contract::ArtifactReleaseIdV1::new(
                    crate::plan::hex32(&pin.artifact_release_id).expect("artifact ID"),
                )
                .expect("artifact identity"),
            )
        };
        let mut substituted_profile = plan.clone();
        let profile = dclutch_release_set_contract::ProtocolInfrastructureProfileV1::new(
            binding(&substituted_profile.core),
            binding(&substituted_profile.rent_credit),
        )
        .expect("coherent but substituted profile");
        let profile_bytes = profile.to_bytes();
        substituted_profile.infrastructure_profile.body_hex = crate::plan::hex(&profile_bytes);
        substituted_profile.infrastructure_profile.body_sha256 = digest(&profile_bytes);
        substituted_profile
            .infrastructure_profile
            .registry_artifact_release_id = substituted_profile.core.artifact_release_id.clone();
        let error =
            crate::campaign::authenticate_checked_campaign_plan(&substituted_profile, &loopback)
                .expect_err("coherently substituted infrastructure profile must refuse");
        assert!(
            error
                .to_string()
                .contains("substituted a Registry or Rent binding"),
            "{error}"
        );

        let mut substituted_schema = plan.clone();
        let registry =
            crate::plan::pubkey(&substituted_schema.registry.program_id).expect("Registry program");
        let pair = substituted_schema
            .records
            .get_mut("registry_artifact_release")
            .expect("Registry ArtifactRelease");
        let substituted_schema_id = [0x5a; 32];
        let content = crate::plan::hex32(&pair.content_sha256).expect("record content digest");
        pair.schema_id = crate::plan::hex(&substituted_schema_id);
        pair.raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &substituted_schema_id, &content],
            &registry,
        )
        .0
        .to_string();
        pair.staging = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &substituted_schema_id, &content],
            &registry,
        )
        .0
        .to_string();
        let error =
            crate::campaign::authenticate_checked_campaign_plan(&substituted_schema, &loopback)
                .expect_err("coherent ArtifactRelease schema/address substitution must refuse");
        assert!(
            error.to_string().contains("ArtifactRelease schema"),
            "{error}"
        );

        let registry_program_pin = plan
            .genesis_accounts
            .get("loader.registry.program")
            .expect("Registry Program account pin");
        let registry_program_path =
            PathBuf::from(&plan.account_dir).join(format!("{}.json", registry_program_pin.address));
        let original_registry_program =
            fs::read(&registry_program_path).expect("Registry Program account JSON");
        let mut changed_registry_program = original_registry_program.clone();
        changed_registry_program.push(b'\n');
        fs::write(&registry_program_path, changed_registry_program)
            .expect("change Registry Program account JSON");
        let error = crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)
            .expect_err("changed account JSON must refuse");
        assert!(error.to_string().contains("file digest"), "{error}");
        fs::write(&registry_program_path, &original_registry_program)
            .expect("restore Registry Program account JSON");

        let original_registry_account = crate::plan::authenticate_cli_account_file_v1(
            &registry_program_path,
            registry_program_pin,
        )
        .expect("authenticate original Registry Program account JSON");
        let mut surplus_plan = plan.clone();
        let surplus_pin = surplus_plan
            .genesis_accounts
            .get_mut("loader.registry.program")
            .expect("surplus Registry Program account pin");
        let surplus_lamports = surplus_pin
            .lamports
            .checked_add(1)
            .expect("one surplus lamport");
        let mut surplus_value: serde_json::Value =
            serde_json::from_slice(&original_registry_program).expect("account JSON value");
        surplus_value["account"]["lamports"] = serde_json::json!(surplus_lamports);
        let mut surplus_bytes =
            serde_json::to_vec_pretty(&surplus_value).expect("surplus account JSON");
        surplus_bytes.push(b'\n');
        surplus_pin.lamports = surplus_lamports;
        surplus_pin.account_sha256 = crate::plan::account_sha256_v1(
            original_registry_account.owner,
            surplus_lamports,
            original_registry_account.executable,
            original_registry_account.rent_epoch,
            &original_registry_account.data,
        )
        .expect("surplus account digest");
        surplus_pin.json_file_sha256 = digest(&surplus_bytes);
        fs::write(&registry_program_path, surplus_bytes)
            .expect("write coherent surplus Registry Program account JSON");
        let error = crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&surplus_plan)
            .expect_err("coherent rent surplus must refuse");
        assert!(error.to_string().contains("Loader pair"), "{error}");
        fs::write(&registry_program_path, &original_registry_program)
            .expect("restore exact-rent Registry Program account JSON");

        let extra_path = PathBuf::from(&plan.account_dir).join("extra.json");
        fs::write(&extra_path, b"{}\n").expect("write extra account JSON");
        let error = crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)
            .expect_err("extra account JSON must refuse");
        assert!(error.to_string().contains("missing, extra"), "{error}");
        fs::remove_file(&extra_path).expect("remove extra account JSON");

        let held_path = work.join("held-registry-program.json");
        fs::rename(&registry_program_path, &held_path).expect("hold Registry Program account JSON");
        let error = crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)
            .expect_err("missing account JSON must refuse");
        assert!(error.to_string().contains("missing, extra"), "{error}");
        fs::rename(&held_path, &registry_program_path)
            .expect("restore missing Registry Program account JSON");

        let mut aliased_set = checked.clone();
        aliased_set.roles[1].program_id = aliased_set.roles[0].programdata_id.clone();
        let authority = crate::plan::pubkey(&aliased_set.retained_upgrade_authority)
            .expect("retained authority");
        let error = crate::local_mutable::authenticate_exact_local_account_dir_v1(
            &plan,
            &aliased_set,
            authority,
        )
        .expect_err("Program/ProgramData alias must refuse");
        assert!(error.to_string().contains("aliased"), "{error}");
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
            .find(|link| link.label == "custody")
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
            .find(|link| link.label == "custody")
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
        runner.reported_authority = Pubkey::new_unique();
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
    fn lost_or_substituted_buffer_output_recovers_exact_buffer_without_second_writer() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.deploy_program = Pubkey::new_unique();
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("substituted deploy output must refuse");
        assert!(
            error
                .to_string()
                .contains("write-buffer output substituted Buffer identity")
        );
        let receipt = load_receipt(&fixture.args.receipt_path)
            .expect("receipt read")
            .expect("armed receipt");
        assert_eq!(receipt.phase, ReceiptPhaseV1::BufferWriteArmed);
        assert!(runner.buffer_written);

        let mut resume = FakeRunner::new(&fixture);
        resume.buffer_written = true;
        let complete = execute_with_runner(&fixture.args, &mut resume)
            .expect("exact persistent Buffer recovers without CLI output");
        assert_eq!(complete.phase, ReceiptPhaseV1::Complete);
        assert!(
            resume
                .calls
                .iter()
                .all(|call| { call.get(1).map(String::as_str) != Some("write-buffer") })
        );
    }

    #[test]
    fn poststate_requires_slot_advancement_and_never_replays() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.after_slot = runner.before_slot;
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("stationary slot must refuse");
        assert!(error.to_string().contains("no slot advance"));
        assert_eq!(runner.send_count, 1);

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
        assert!(
            first
                .to_string()
                .contains("synthetic write-buffer interruption")
        );
        let receipt = load_receipt(&fixture.args.receipt_path)
            .expect("receipt read")
            .expect("prepared receipt");
        assert_eq!(receipt.phase, ReceiptPhaseV1::BufferWriteArmed);

        let mut ambiguous = FakeRunner::new(&fixture);
        ambiguous.deployed = true;
        let second = execute_with_runner(&fixture.args, &mut ambiguous)
            .expect_err("moved chain behind prepared receipt must stop");
        assert!(second.to_string().contains("Loader prestate moved"));
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
        Signature::from_str(
            receipt
                .transaction_signature
                .as_deref()
                .expect("journaled extension signature"),
        )
        .expect("valid extension signature");
        assert!(receipt.finalized_transaction.is_some());
        let programdata_before_bytes = u64::try_from(fixture.before_live.len())
            .expect("fixture live width")
            .checked_add(45)
            .expect("fixture ProgramData width");
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
                programdata_before_bytes,
                programdata_after_bytes: programdata_before_bytes + 128,
                extension_additional_bytes: 128,
            })
        );
        assert!(runner.calls.iter().all(|call| {
            !(call.first().map(String::as_str) == Some("program")
                && call.get(1).map(String::as_str) == Some("extend"))
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
    fn submitted_upgrade_restart_is_poll_only_then_reprepares_after_exact_expiry() {
        let fixture = Fixture::new();
        let mut crashed = FakeRunner::new(&fixture);
        crashed.send_success = false;
        let error = execute_with_runner(&fixture.args, &mut crashed)
            .expect_err("synthetic crash at the send boundary");
        assert!(
            error
                .to_string()
                .contains("synthetic packet send interruption")
        );
        let submitted = load_receipt(&fixture.args.receipt_path)
            .expect("submitted receipt read")
            .expect("submitted receipt");
        assert_eq!(submitted.phase, ReceiptPhaseV1::Submitted);
        assert!(submitted.transaction_signature.is_some());
        assert_eq!(crashed.send_count, 0);

        let mut pending = FakeRunner::new(&fixture);
        pending.buffer_written = true;
        let error = execute_with_runner(&fixture.args, &mut pending)
            .expect_err("unexpired exact packet must remain poll-only");
        assert!(error.to_string().contains("poll-only recovery"));
        assert_eq!(pending.send_count, 0);
        assert!(pending.calls.iter().all(|call| {
            call.get(1).map(String::as_str) != Some("write-buffer")
                && call.get(1).map(String::as_str) != Some("deploy")
        }));

        let mut expired = FakeRunner::new(&fixture);
        expired.buffer_written = true;
        expired.finalized_height = submitted.last_valid_block_height.expect("submitted expiry") + 1;
        let complete = execute_with_runner(&fixture.args, &mut expired)
            .expect("expired packet safely reprepares once");
        assert_eq!(complete.phase, ReceiptPhaseV1::Complete);
        assert_eq!(complete.expired_packets.len(), 1);
        assert_eq!(expired.send_count, 1);
        assert_ne!(
            complete.expired_packets[0].transaction_signature,
            complete
                .transaction_signature
                .expect("replacement signature")
        );
    }

    #[test]
    fn submitted_restart_never_enters_fresh_set_audit_before_signature_recovery() {
        struct FreshAuditTrap;

        impl CliRunner for FreshAuditTrap {
            fn enforces_fresh_deployment_set_boundary(&self) -> bool {
                panic!("Submitted recovery must not ask for a fresh set audit");
            }

            fn run(&mut self, _arguments: &[String]) -> Result<CliOutput> {
                panic!("Submitted recovery helper must remain side-effect free");
            }
        }

        let fixture = Fixture::new();
        let mut runner = FreshAuditTrap;
        assert!(
            authenticate_phase_mutation_boundary(
                &fixture.args,
                &mut runner,
                ReceiptPhaseV1::Submitted,
            )
            .expect("Submitted phase bypasses contradictory fresh poststate audit")
            .is_none()
        );
    }

    #[test]
    fn live_orphan_buffer_writer_blocks_attach_and_second_writer_until_exact_exit() {
        let fixture = Fixture::new();
        let mut crashed = FakeRunner::new(&fixture);
        crashed.crash_after_buffer_lease = true;
        let error = execute_with_runner(&fixture.args, &mut crashed)
            .expect_err("synthetic crash after durable writer lease");
        assert!(error.to_string().contains("durable Buffer writer lease"));
        let armed = load_receipt(&fixture.args.receipt_path)
            .expect("armed receipt read")
            .expect("armed receipt");
        assert_eq!(armed.phase, ReceiptPhaseV1::BufferWriteArmed);
        assert_eq!(armed.buffer_write_attempts.len(), 1);
        assert!(
            armed.buffer_write_attempts[0]
                .exit_observed_finalized_block_height
                .is_none()
        );

        let mut orphan_alive = FakeRunner::new(&fixture);
        orphan_alive.buffer_writer_alive = true;
        orphan_alive.buffer_writer_restart_without_handle = true;
        orphan_alive.buffer_written = true;
        let error = execute_with_runner(&fixture.args, &mut orphan_alive)
            .expect_err("live exact orphan must remain the sole writer");
        assert!(error.to_string().contains("alive after operator restart"));
        assert!(
            orphan_alive
                .calls
                .iter()
                .all(|call| { call.get(1).map(String::as_str) != Some("write-buffer") })
        );

        let mut exited = FakeRunner::new(&fixture);
        exited.buffer_written = true;
        let complete = execute_with_runner(&fixture.args, &mut exited)
            .expect("exact exited writer permits authenticated Buffer attach");
        assert_eq!(complete.phase, ReceiptPhaseV1::Complete);
        assert_eq!(complete.buffer_write_attempts.len(), 1);
        assert_eq!(
            complete.buffer_write_attempts[0]
                .exit_disposition
                .as_deref(),
            Some("lost_after_operator_crash")
        );
        assert!(
            exited
                .calls
                .iter()
                .all(|call| { call.get(1).map(String::as_str) != Some("write-buffer") })
        );
    }

    #[test]
    fn submitted_upgrade_expiry_requires_null_signature_and_exact_payer() {
        let fixture = Fixture::new();
        let mut crashed = FakeRunner::new(&fixture);
        crashed.send_success = false;
        execute_with_runner(&fixture.args, &mut crashed).expect_err("synthetic send interruption");
        let submitted = load_receipt(&fixture.args.receipt_path)
            .expect("submitted receipt read")
            .expect("submitted receipt");
        let expired_height = submitted.last_valid_block_height.expect("expiry") + 1;

        let mut charged = FakeRunner::new(&fixture);
        charged.buffer_written = true;
        charged.buffer_upload_fee_lamports = 1;
        charged.finalized_height = expired_height;
        let error = execute_with_runner(&fixture.args, &mut charged)
            .expect_err("charged failed packet cannot be replaced");
        assert!(error.to_string().contains("payer moved"));
        assert_eq!(charged.send_count, 0);

        let mut pending = FakeRunner::new(&fixture);
        pending.buffer_written = true;
        pending.finalized_height = expired_height;
        pending.signature_status_override = Some(JournaledSignatureStatusV1::Pending);
        let error = execute_with_runner(&fixture.args, &mut pending)
            .expect_err("present signature remains poll-only after expiry");
        assert!(error.to_string().contains("still present and pending"));
        assert_eq!(pending.send_count, 0);

        let mut failed = FakeRunner::new(&fixture);
        failed.buffer_written = true;
        failed.finalized_height = expired_height;
        failed.signature_status_override = Some(JournaledSignatureStatusV1::FinalizedFailure(
            "InstructionError".into(),
        ));
        let error = execute_with_runner(&fixture.args, &mut failed)
            .expect_err("finalized failure requires attribution");
        assert!(error.to_string().contains("charged fee must be attributed"));
        assert_eq!(failed.send_count, 0);
    }

    #[test]
    fn crash_after_send_before_response_attaches_exact_upgrade_without_resend() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        runner.send_lands_then_errors = true;
        let error = execute_with_runner(&fixture.args, &mut runner)
            .expect_err("send response is lost after landing");
        assert!(
            error
                .to_string()
                .contains("lost send response after landing")
        );
        assert_eq!(runner.send_count, 1);
        assert_eq!(
            load_receipt(&fixture.args.receipt_path)
                .expect("receipt read")
                .expect("receipt")
                .phase,
            ReceiptPhaseV1::Submitted
        );

        runner.send_lands_then_errors = false;
        let complete = execute_with_runner(&fixture.args, &mut runner)
            .expect("exact landed signature attaches from poststate");
        assert_eq!(complete.phase, ReceiptPhaseV1::Complete);
        assert_eq!(runner.send_count, 1);
    }

    #[test]
    fn message_and_signed_boundaries_resume_exactly_and_journal_drift_refuses_keys() {
        let fixture = Fixture::new();
        let mut before_sign = FakeRunner::new(&fixture);
        before_sign.sign_success = false;
        let error = execute_with_runner(&fixture.args, &mut before_sign)
            .expect_err("synthetic crash after message fsync");
        assert!(error.to_string().contains("before packet signing"));
        let mut receipt = load_receipt(&fixture.args.receipt_path)
            .expect("message receipt read")
            .expect("message receipt");
        assert_eq!(receipt.phase, ReceiptPhaseV1::MessagePrepared);

        let mut signer = FakeRunner::new(&fixture);
        signer.buffer_written = true;
        let query = LoaderActionQueryV1 {
            origin: &fixture.args.origin,
            program_id: fixture.program,
            programdata_id: fixture.programdata,
            authority: fixture.authority,
            payer: fixture.payer,
            authority_keypair: &fixture.args.authority_keypair,
            payer_keypair: &fixture.args.fee_payer_keypair,
            action: LoaderActionV1::Upgrade {
                buffer: fixture.args.buffer_pubkey,
            },
        };
        let unsigned = upgrade_unsigned(&receipt).expect("fsynced message");
        let signed = signer
            .sign_loader_action(&query, &unsigned)
            .expect("synthetic exact signature");
        receipt.phase = ReceiptPhaseV1::SignedNotSubmitted;
        receipt.signed_packet_base64 = Some(signed.packet_base64);
        receipt.signed_packet_sha256 = Some(signed.packet_sha256);
        receipt.transaction_signature = Some(signed.signature);
        write_receipt(&fixture.args.receipt_path, &mut receipt)
            .expect("signed-not-submitted boundary");

        let mut resume = FakeRunner::new(&fixture);
        resume.buffer_written = true;
        let complete = execute_with_runner(&fixture.args, &mut resume)
            .expect("signed packet resumes without resigning");
        assert_eq!(complete.phase, ReceiptPhaseV1::Complete);
        assert_eq!(resume.sign_count, 0);
        assert_eq!(resume.send_count, 1);

        let drift = Fixture::new();
        let mut before_sign = FakeRunner::new(&drift);
        before_sign.sign_success = false;
        let _ = execute_with_runner(&drift.args, &mut before_sign)
            .expect_err("message boundary for journal drift");
        let mut journal: UpgradeSetJournalV1 = serde_json::from_slice(
            &fs::read(&drift.args.deployment_set_journal_path).expect("journal bytes"),
        )
        .expect("journal");
        journal.source_tree_sha256 = "ab".repeat(32);
        fs::write(
            &drift.args.deployment_set_journal_path,
            serde_json::to_vec_pretty(&journal).expect("drifted journal"),
        )
        .expect("journal drift");
        let mut refused = FakeRunner::new(&drift);
        refused.buffer_written = true;
        let error = execute_with_runner(&drift.args, &mut refused)
            .expect_err("journal digest drift must refuse before signing");
        assert!(error.to_string().contains("immutable plan changed"));
        assert_eq!(refused.sign_count, 0);
        assert_eq!(refused.send_count, 0);
    }

    #[test]
    fn submitted_extension_restart_never_applies_double_extend() {
        let fixture = Fixture::new();
        let args = fixture.extension_args(128);
        let mut crashed = FakeRunner::new(&fixture);
        configure_extension_runner(&fixture, &mut crashed, 128);
        crashed.send_success = false;
        let error = execute_extension_with_runner(&args, &mut crashed)
            .expect_err("synthetic extension send crash");
        assert!(
            error
                .to_string()
                .contains("synthetic packet send interruption")
        );
        let submitted = load_extension_receipt(&args.receipt_path)
            .expect("extension receipt read")
            .expect("extension receipt");
        assert_eq!(submitted.phase, ReceiptPhaseV1::Submitted);

        let mut pending = FakeRunner::new(&fixture);
        configure_extension_runner(&fixture, &mut pending, 128);
        let error = execute_extension_with_runner(&args, &mut pending)
            .expect_err("unexpired extension remains pending");
        assert!(error.to_string().contains("poll-only recovery"));
        assert_eq!(pending.send_count, 0);

        let mut expired = FakeRunner::new(&fixture);
        configure_extension_runner(&fixture, &mut expired, 128);
        expired.finalized_height = submitted.last_valid_block_height.expect("extension expiry") + 1;
        let complete = execute_extension_with_runner(&args, &mut expired)
            .expect("expired extension packet safely reprepares");
        assert_eq!(complete.phase, ReceiptPhaseV1::Complete);
        assert_eq!(complete.expired_packets.len(), 1);
        assert_eq!(expired.send_count, 1);
        assert_eq!(
            complete
                .after
                .expect("extension poststate")
                .programdata_data_bytes,
            submitted.before.programdata_data_bytes + 128
        );
    }

    #[test]
    fn receipt_publish_is_no_clobber_and_cas_refuses_concurrent_writer() {
        let directory = TestDirectory::new();
        let path = directory.0.join("cas-receipt.json");
        let receipt_value = |writer: u64| {
            let mut value = json!({"receipt_sha256": "", "writer": writer});
            let sha256 = digest(&serde_json::to_vec(&value).expect("CAS value"));
            value["receipt_sha256"] = json!(sha256);
            value
        };
        let initial = receipt_value(0);
        let initial_digest = initial["receipt_sha256"]
            .as_str()
            .expect("initial digest")
            .to_owned();
        write_json_atomic_receipt_cas(&path, &initial, None).expect("initial no-clobber receipt");
        assert!(
            write_json_atomic_receipt_cas(&path, &receipt_value(9), None)
                .expect_err("second creator must not clobber")
                .to_string()
                .contains("exact-content stale writer")
        );

        let second = receipt_value(1);
        write_json_atomic_receipt_cas(&path, &second, Some(&initial_digest))
            .expect("first loaded writer wins CAS");
        let error = write_json_atomic_receipt_cas(&path, &receipt_value(2), Some(&initial_digest))
            .expect_err("stale concurrent writer must refuse");
        assert!(error.to_string().contains("exact-content stale writer"));
        let value: Value = serde_json::from_slice(&fs::read(&path).expect("CAS receipt bytes"))
            .expect("CAS receipt JSON");
        assert_eq!(value.get("writer").and_then(Value::as_u64), Some(1));

        let mut hostile = value.clone();
        hostile["writer"] = json!(77);
        fs::write(
            &path,
            serde_json::to_vec_pretty(&hostile).expect("hostile receipt bytes"),
        )
        .expect("hostile same-inner-digest body");
        let second_digest = second["receipt_sha256"].as_str().expect("second digest");
        let error = write_json_atomic_receipt_cas(&path, &receipt_value(2), Some(second_digest))
            .expect_err("changed body with retained embedded digest must refuse");
        assert!(error.to_string().contains("body differs"));
    }

    #[test]
    fn system_buffer_writer_lease_round_trips_and_preexisting_permit_refuses() {
        let directory = TestDirectory::new();
        let lease_path = directory.0.join("writer.lease");
        let permit_path = directory.0.join("writer.permit");
        let arguments = vec!["leased-buffer-smoke".into()];
        let command_sha256 = digest(b"harmless-echo-command");
        let operation_id = "ab".repeat(32);
        let query = BufferWriterQueryV1 {
            operation_id: &operation_id,
            attempt_ordinal: 0,
            command_arguments: &arguments,
            command_sha256: &command_sha256,
            lease_path: &lease_path,
            permit_path: &permit_path,
        };
        let mut runner = SystemCliRunner {
            executable: PathBuf::from("/bin/echo"),
            buffer_child: None,
        };
        let lease = runner
            .start_buffer_writer(&query)
            .expect("real supervisor publishes parseable eight-field lease");
        assert_eq!(lease.schema, BUFFER_WRITER_LEASE_SCHEMA);
        assert_eq!(lease.process_group_id, lease.pid);
        require_digest(&lease.process_nonce, "real process nonce").expect("strong nonce");
        assert_eq!(
            runner
                .buffer_writer_status(&lease)
                .expect("exact stopped writer status"),
            BufferWriterStatusV1::AliveStopped
        );
        assert!(!permit_path.exists());
        let output = runner
            .continue_buffer_writer(&query, &lease)
            .expect("durable exact permit releases real supervisor");
        assert!(output.success);
        assert_eq!(output.stdout.trim(), "leased-buffer-smoke");
        assert_eq!(
            runner
                .buffer_writer_status(&lease)
                .expect("exited writer status"),
            BufferWriterStatusV1::Exited
        );
        assert_eq!(
            read_regular_reference(&permit_path, "test permit").expect("permit bytes"),
            buffer_writer_permit_bytes(&lease)
        );
        assert!(
            runner
                .start_buffer_writer(&query)
                .expect_err("existing exact lease cannot attach across a premature permit")
                .to_string()
                .contains("premature permit")
        );

        let refused_lease = directory.0.join("refused.lease");
        let refused_permit = directory.0.join("refused.permit");
        fs::write(&refused_permit, b"stale").expect("pre-existing permit");
        let refused_operation_id = "cd".repeat(32);
        let refused_query = BufferWriterQueryV1 {
            operation_id: &refused_operation_id,
            attempt_ordinal: 0,
            command_arguments: &arguments,
            command_sha256: &command_sha256,
            lease_path: &refused_lease,
            permit_path: &refused_permit,
        };
        let error = runner
            .start_buffer_writer(&refused_query)
            .expect_err("pre-existing permit cannot release a fresh writer");
        assert!(error.to_string().contains("pre-existing permit"));
        assert!(!refused_lease.exists());

        let symlink_lease = directory.0.join("symlink.lease");
        let symlink_permit = directory.0.join("symlink.permit");
        std::os::unix::fs::symlink(&permit_path, &symlink_permit)
            .expect("pre-existing permit symlink");
        let symlink_operation_id = "ef".repeat(32);
        let symlink_query = BufferWriterQueryV1 {
            operation_id: &symlink_operation_id,
            attempt_ordinal: 0,
            command_arguments: &arguments,
            command_sha256: &command_sha256,
            lease_path: &symlink_lease,
            permit_path: &symlink_permit,
        };
        assert!(
            runner
                .start_buffer_writer(&symlink_query)
                .expect_err("pre-existing permit symlink cannot release a fresh writer")
                .to_string()
                .contains("pre-existing permit")
        );
        assert!(!symlink_lease.exists());
    }

    #[test]
    fn stale_receipt_transition_lock_finishes_exact_pending_target() {
        let directory = TestDirectory::new();
        let path = directory.0.join("recovery-receipt.json");
        let receipt_value = |writer: u64| {
            let mut value = json!({"receipt_sha256": "", "writer": writer});
            let sha256 = digest(&serde_json::to_vec(&value).expect("CAS value"));
            value["receipt_sha256"] = json!(sha256);
            value
        };
        let initial = receipt_value(0);
        write_json_atomic_receipt_cas(&path, &initial, None).expect("initial receipt");
        let initial_digest = initial["receipt_sha256"]
            .as_str()
            .expect("initial digest")
            .to_owned();
        let target = receipt_value(1);
        let target_digest = target["receipt_sha256"]
            .as_str()
            .expect("target digest")
            .to_owned();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name");
        let pending_file_name = format!(".{file_name}.{target_digest}.receipt-pending");
        let pending = directory.0.join(&pending_file_name);
        let mut target_bytes = serde_json::to_vec_pretty(&target).expect("target bytes");
        target_bytes.push(b'\n');
        fs::write(&pending, target_bytes).expect("crashed pending target");
        let lock_path = directory.0.join(format!(".{file_name}.lock"));
        let mut exited = Command::new("/usr/bin/true")
            .spawn()
            .expect("short-lived lock owner");
        let exited_pid = exited.id();
        exited.wait().expect("short-lived owner exit");
        let stale_lock = ReceiptTransitionLockV1 {
            schema: RECEIPT_TRANSITION_LOCK_SCHEMA.into(),
            owner_pid: exited_pid,
            owner_start_token: "gone-process".into(),
            expected_receipt_sha256: Some(initial_digest.clone()),
            target_receipt_sha256: target_digest,
            pending_file_name,
        };
        fs::write(
            &lock_path,
            serde_json::to_vec_pretty(&stale_lock).expect("stale lock"),
        )
        .expect("crashed lock");
        sync_parent_directory(&directory.0).expect("crash boundary fsync");

        write_json_atomic_receipt_cas(&path, &target, Some(&initial_digest))
            .expect("stale transition recovery");
        let recovered: Value = serde_json::from_slice(&fs::read(&path).expect("recovered bytes"))
            .expect("recovered JSON");
        assert_eq!(recovered["writer"], json!(1));
        assert!(!lock_path.exists());
        assert!(!pending.exists());
    }

    #[test]
    fn receipt_cas_refuses_a_symlink_publish_target() {
        let directory = TestDirectory::new();
        let path = directory.0.join("receipt-link.json");
        let outside = directory.0.join("outside-receipt.json");
        let receipt_value = |writer: u64| {
            let mut value = json!({"receipt_sha256": "", "writer": writer});
            let sha256 = digest(&serde_json::to_vec(&value).expect("CAS value"));
            value["receipt_sha256"] = json!(sha256);
            value
        };
        let initial = receipt_value(0);
        fs::write(
            &outside,
            serde_json::to_vec_pretty(&initial).expect("outside receipt"),
        )
        .expect("outside receipt bytes");
        std::os::unix::fs::symlink(&outside, &path).expect("receipt symlink");
        let initial_digest = initial["receipt_sha256"].as_str().expect("initial digest");
        let error = write_json_atomic_receipt_cas(&path, &receipt_value(1), Some(initial_digest))
            .expect_err("CAS must not follow or clobber a receipt symlink");
        assert!(error.to_string().contains("regular non-symlink"));
        let unchanged: Value =
            serde_json::from_slice(&fs::read(&outside).expect("outside receipt unchanged"))
                .expect("outside receipt JSON");
        assert_eq!(unchanged["writer"], json!(0));
        assert!(path.is_symlink());
    }

    #[test]
    fn receipt_cas_refuses_a_symlink_pending_candidate() {
        let directory = TestDirectory::new();
        let path = directory.0.join("receipt.json");
        let mut target = json!({"receipt_sha256": "", "writer": 1});
        let target_digest = digest(&serde_json::to_vec(&target).expect("target value"));
        target["receipt_sha256"] = json!(target_digest);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name");
        let pending = directory
            .0
            .join(format!(".{file_name}.{target_digest}.receipt-pending"));
        let outside = directory.0.join("outside-pending.json");
        let mut bytes = serde_json::to_vec_pretty(&target).expect("target bytes");
        bytes.push(b'\n');
        fs::write(&outside, bytes).expect("outside exact target");
        std::os::unix::fs::symlink(&outside, &pending).expect("pending symlink");
        let error = write_json_atomic_receipt_cas(&path, &target, None)
            .expect_err("CAS must not publish a symlink pending candidate");
        assert!(error.to_string().contains("regular non-symlink"));
        assert!(!path.exists());
        assert!(pending.is_symlink());
    }

    #[test]
    fn partial_dump_temporary_is_never_published_and_is_recovered() {
        let fixture = Fixture::new();
        let mut runner = FakeRunner::new(&fixture);
        let operation_id = "a".repeat(64);
        let file_name = fixture
            .args
            .dump_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("dump name");
        let temporary = fixture
            .args
            .dump_path
            .parent()
            .expect("dump parent")
            .join(format!(".{file_name}.{operation_id}.dump-pending"));
        fs::write(&temporary, b"partial").expect("partial dump temporary");
        let (sha256, shape) = verify_dump(
            &mut runner,
            &fixture.args,
            &operation_id,
            &fixture.raw_elf,
            &fixture.candidate_live,
        )
        .expect("partial temporary recovered");
        assert_eq!(sha256, digest(&fixture.raw_elf));
        assert_eq!(shape, "raw-elf");
        assert_eq!(
            fs::read(&fixture.args.dump_path).expect("published dump"),
            fixture.raw_elf
        );
        assert!(!temporary.exists());
    }

    #[test]
    fn mutation_permit_refuses_carry_forward_and_out_of_order_roles() {
        let fixture = Fixture::new();
        let error = require_mutation_permit(&fixture.args, None, false, None)
            .expect_err("Extension receipt must not replace the journal's Upgrade receipt");
        assert!(error.to_string().contains("extension receipt"));
        let journal: UpgradeSetJournalV1 = serde_json::from_slice(
            &fs::read(&fixture.args.deployment_set_journal_path).expect("journal bytes"),
        )
        .expect("journal");
        let mut later_row_collision = fixture.args.clone();
        later_row_collision.receipt_path = PathBuf::from(&journal.roles[3].receipt.canonical_path);
        let error = require_mutation_permit(&later_row_collision, None, false, None)
            .expect_err("Extension receipt must not consume a later Upgrade output path");
        assert!(error.to_string().contains("aliases deployment-set"));

        let mut registry = fixture.args.clone();
        registry.role = "registry".into();
        registry.program_id =
            parse_pubkey(PERMANENT_DEVNET_UPGRADE_TARGETS_V1[0].1, "Registry").expect("Registry");
        registry.programdata_id = parse_pubkey(
            PERMANENT_DEVNET_UPGRADE_TARGETS_V1[0].2,
            "Registry ProgramData",
        )
        .expect("Registry ProgramData");
        let error = require_mutation_permit(&registry, None, true, None)
            .expect_err("CarryForward Registry mutation must refuse");
        assert!(error.to_string().contains("CarryForward"));

        let mut core = fixture.args.clone();
        core.role = "core".into();
        core.program_id =
            parse_pubkey(PERMANENT_DEVNET_UPGRADE_TARGETS_V1[6].1, "Core").expect("Core");
        core.programdata_id =
            parse_pubkey(PERMANENT_DEVNET_UPGRADE_TARGETS_V1[6].2, "Core ProgramData")
                .expect("Core ProgramData");
        let error = require_mutation_permit(&core, None, true, None)
            .expect_err("Core before Custody must refuse");
        assert!(error.to_string().contains("next role is custody"));

        let wrong_dump = Fixture::new();
        let mut journal: UpgradeSetJournalV1 = serde_json::from_slice(
            &fs::read(&wrong_dump.args.deployment_set_journal_path).expect("journal bytes"),
        )
        .expect("journal");
        journal.roles[2].dump.canonical_path = wrong_dump
            ._directory
            .0
            .join("substituted-dump.so")
            .to_string_lossy()
            .into_owned();
        fs::write(
            &wrong_dump.args.deployment_set_journal_path,
            serde_json::to_vec_pretty(&journal).expect("journal JSON"),
        )
        .expect("wrong dump journal");
        let error = require_mutation_permit(&wrong_dump.args, None, true, None)
            .expect_err("target dump path substitution must refuse");
        assert!(error.to_string().contains("next role is custody"));
    }

    fn signed_buffer_transaction(
        instructions: &[solana_program::instruction::Instruction],
        payer: &Keypair,
        second_signer: &Keypair,
    ) -> Transaction {
        let message = Message::new(instructions, Some(&payer.pubkey()));
        let mut transaction = Transaction::new_unsigned(message);
        transaction
            .try_sign(&[payer, second_signer], Hash::new_from_array([73_u8; 32]))
            .expect("sign Buffer transaction");
        transaction
    }

    fn buffer_transaction_evidence(
        transaction: &Transaction,
        slot: u64,
        fee: u64,
        payer_before: u64,
        payer_after: u64,
        buffer: Pubkey,
        buffer_before: u64,
        buffer_after: u64,
        successful: bool,
    ) -> (String, Value) {
        let mut pre = vec![0_u64; transaction.message.account_keys.len()];
        let mut post = vec![0_u64; transaction.message.account_keys.len()];
        let payer_index = transaction
            .message
            .account_keys
            .iter()
            .position(|key| *key == transaction.message.account_keys[0])
            .expect("payer key index");
        let buffer_index = transaction
            .message
            .account_keys
            .iter()
            .position(|key| *key == buffer)
            .expect("Buffer key index");
        pre[payer_index] = payer_before;
        post[payer_index] = payer_after;
        pre[buffer_index] = buffer_before;
        post[buffer_index] = buffer_after;
        let packet = bincode::serialize(transaction).expect("serialize Buffer transaction");
        let signature = transaction.signatures[0].to_string();
        (
            signature,
            json!({
                "slot": slot,
                "transaction": [BASE64.encode(packet), "base64"],
                "meta": {
                    "err": if successful { Value::Null } else { json!({"InstructionError":[0,"Synthetic"]}) },
                    "fee": fee,
                    "preBalances": pre,
                    "postBalances": post
                }
            }),
        )
    }

    #[test]
    fn buffer_history_accepts_only_exact_idempotent_upload_transactions() {
        let fixture = Fixture::new();
        let payer = Keypair::new();
        let buffer = Keypair::new();
        let authority = Keypair::new();
        let raw_elf = b"\x7fELFexact-buffer-payload".to_vec();
        let rent = 123_456_u64;
        let fee = 5_000_u64;
        let slot = 777_u64;
        let query = BufferUploadQueryV1 {
            origin: &fixture.args.origin,
            buffer: buffer.pubkey(),
            authority: authority.pubkey(),
            payer: payer.pubkey(),
            raw_elf: &raw_elf,
            minimum_slot: slot,
            expected_rent_lamports: rent,
            wallet_before_lamports: 1_000_000,
            wallet_after_lamports: 1_000_000 - rent - fee,
        };

        let create_instructions = solana_loader_v3_interface::instruction::create_buffer(
            &payer.pubkey(),
            &buffer.pubkey(),
            &authority.pubkey(),
            rent,
            raw_elf.len(),
        )
        .expect("canonical Buffer create");
        let create = signed_buffer_transaction(&create_instructions, &payer, &buffer);
        let (create_signature, create_evidence) = buffer_transaction_evidence(
            &create,
            slot,
            fee,
            1_000_000,
            1_000_000 - rent - fee,
            buffer.pubkey(),
            0,
            rent,
            true,
        );
        assert_eq!(
            validate_buffer_upload_transaction(&query, &create_signature, slot, &create_evidence,)
                .expect("exact create+initialize"),
            (fee, true)
        );

        let write_instruction = solana_loader_v3_interface::instruction::write(
            &buffer.pubkey(),
            &authority.pubkey(),
            0,
            raw_elf[..8].to_vec(),
        );
        let write = signed_buffer_transaction(&[write_instruction], &payer, &authority);
        let (write_signature, write_evidence) = buffer_transaction_evidence(
            &write,
            slot + 1,
            fee,
            1_000_000,
            1_000_000 - fee,
            buffer.pubkey(),
            rent,
            rent,
            true,
        );
        assert_eq!(
            validate_buffer_upload_transaction(
                &query,
                &write_signature,
                slot + 1,
                &write_evidence,
            )
            .expect("exact idempotent write"),
            (fee, false)
        );
        assert_eq!(
            validate_buffer_upload_transaction(
                &query,
                &write_signature,
                slot + 1,
                &write_evidence,
            )
            .expect("an exact repeated write remains byte-idempotent"),
            (fee, false)
        );

        let wrong_offset_instruction = solana_loader_v3_interface::instruction::write(
            &buffer.pubkey(),
            &authority.pubkey(),
            1,
            raw_elf[..8].to_vec(),
        );
        let wrong_offset =
            signed_buffer_transaction(&[wrong_offset_instruction], &payer, &authority);
        let (signature, evidence) = buffer_transaction_evidence(
            &wrong_offset,
            slot + 2,
            fee,
            1_000_000,
            1_000_000 - fee,
            buffer.pubkey(),
            rent,
            rent,
            true,
        );
        assert!(
            validate_buffer_upload_transaction(&query, &signature, slot + 2, &evidence)
                .expect_err("wrong offset bytes must refuse")
                .to_string()
                .contains("idempotent slice")
        );

        let unrelated = solana_program::instruction::Instruction {
            program_id: Pubkey::new_unique(),
            accounts: Vec::new(),
            data: vec![1],
        };
        let unrelated_write = signed_buffer_transaction(
            &[
                solana_loader_v3_interface::instruction::write(
                    &buffer.pubkey(),
                    &authority.pubkey(),
                    0,
                    raw_elf[..8].to_vec(),
                ),
                unrelated,
            ],
            &payer,
            &authority,
        );
        let (signature, evidence) = buffer_transaction_evidence(
            &unrelated_write,
            slot + 3,
            fee,
            1_000_000,
            1_000_000 - fee,
            buffer.pubkey(),
            rent,
            rent,
            true,
        );
        assert!(
            validate_buffer_upload_transaction(&query, &signature, slot + 3, &evidence)
                .expect_err("unrelated program must refuse")
                .to_string()
                .contains("unrelated program")
        );

        let wrong_payer_query = BufferUploadQueryV1 {
            payer: Pubkey::new_unique(),
            ..query
        };
        assert!(
            validate_buffer_upload_transaction(
                &wrong_payer_query,
                &write_signature,
                slot + 1,
                &write_evidence,
            )
            .expect_err("wrong payer must refuse")
            .to_string()
            .contains("fee payer")
        );
        let wrong_authority_query = BufferUploadQueryV1 {
            authority: Pubkey::new_unique(),
            ..query
        };
        assert!(
            validate_buffer_upload_transaction(
                &wrong_authority_query,
                &write_signature,
                slot + 1,
                &write_evidence,
            )
            .expect_err("wrong authority must refuse")
            .to_string()
            .contains("one exact account")
        );
        let wrong_rent_query = BufferUploadQueryV1 {
            expected_rent_lamports: rent + 1,
            ..query
        };
        assert!(
            validate_buffer_upload_transaction(
                &wrong_rent_query,
                &create_signature,
                slot,
                &create_evidence,
            )
            .expect_err("wrong rent must refuse")
            .to_string()
            .contains("substituted system instruction")
        );

        let (failed_signature, failed_evidence) = buffer_transaction_evidence(
            &create,
            slot,
            fee,
            1_000_000,
            1_000_000 - fee,
            buffer.pubkey(),
            0,
            0,
            false,
        );
        assert_eq!(
            validate_buffer_upload_transaction(&query, &failed_signature, slot, &failed_evidence,)
                .expect("failed exact create records only its fee"),
            (fee, false)
        );
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
