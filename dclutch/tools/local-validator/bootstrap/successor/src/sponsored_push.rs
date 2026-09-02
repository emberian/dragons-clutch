//! Exterior construction and execution for the sponsored Pyth push lifecycle.
//!
//! The mutable upstream price account is never treated as durable evidence.
//! This caller reads one finalized snapshot, derives the immutable candidate
//! and canonical head from the authenticated release and price body, binds the
//! executing Resolution deployment through the active release set, and only
//! then constructs an unsigned action. Execution is optional and requires a
//! separately named signer keypair.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{ErrorKind, Write as _},
    os::unix::fs::MetadataExt as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_market_core_codec::CoreState;
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_pyth_svm::{
    FULL_PRICE_UPDATE_V2_LEN, FullPriceUpdateV2, PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1,
    PythSponsoredPushReleaseV1, devnet_sponsored_sol_usd_release_v1,
};
use dclutch_registry_contract::ACTIVATION_PDA_DOMAIN_V1;
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1, SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
    SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1, SponsoredPushActionV1, SponsoredPushCandidateV1,
    SponsoredPushHeadV1, SponsoredPushInstructionV1,
};
use dclutch_resolution_core_v3_operator::{
    ResolutionAdmitTerminalSnapshotV3, build_resolution_admit_terminal_v3,
};
use dclutch_source_contract::{
    PROVIDER_RELEASE_SCHEMA_ID_V1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, ProviderReleaseV1,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
    SourceResolutionPhaseV1, SourceResolutionStateV2, SourceSpecV1, WINDOW_SPEC_SCHEMA_ID_V1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::{
    Error, Result, absolute,
    campaign::{parse_campaign_terminal_evidence_with_expected_cluster_v1, read_keypair_file},
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1},
    model::{AccountEvidence, SuccessorPlan, TransactionEvidence},
    plan::{hex, pubkey},
    rpc::{Rpc, RpcAccount, SignedVersionedPacketV1, WritePolicyV1, account_evidence},
    terminal_lifecycle::{authenticate_plan_source, required_account, routed_record},
    wallet_terminal::authenticate_role,
};

const INPUT_FORMAT: &str = "dclutch-sponsored-push-exterior-input-v1";
const REPORT_FORMAT: &str = "dclutch-sponsored-push-exterior-report-v2";
const SUCCESS_CERTIFICATE_KIND: u8 = 1;
const FAILURE_CERTIFICATE_KIND: u8 = 4;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordPairInputV1 {
    raw: String,
    staging: String,
}

impl RecordPairInputV1 {
    fn keys(&self) -> Result<(Pubkey, Pubkey)> {
        Ok((pubkey(&self.raw)?, pubkey(&self.staging)?))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SponsoredAccountsInputV1 {
    registry_program: String,
    activation_cache: String,
    core_program: String,
    /// Core's ProgramData, which only the terminal admission needs.
    ///
    /// `default` is carried so an input authored for the five sponsored-push
    /// actions still deserializes; an empty value never reaches a message,
    /// because [`InputKeysV1::core_programdata`] refuses it by name.
    #[serde(default)]
    core_programdata: String,
    resolution_program: String,
    resolution_programdata: String,
    market: String,
    source_state: String,
    source_material: RecordPairInputV1,
    source_spec: RecordPairInputV1,
    provider_release: RecordPairInputV1,
    adapter_config: RecordPairInputV1,
    window: RecordPairInputV1,
    statistic: RecordPairInputV1,
    sponsored_release: RecordPairInputV1,
    product: RecordPairInputV1,
    result_domain: RecordPairInputV1,
    portfolio: RecordPairInputV1,
    capability_manifest: RecordPairInputV1,
    failure_funding: String,
    price_account: String,
    receiver_program: String,
    receiver_programdata: String,
    push_oracle_program: String,
    push_oracle_programdata: String,
    receiver_config: String,
    lookup_table: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SponsoredInputV1 {
    format: String,
    generation: u64,
    terminal_sequence: u64,
    release_set: String,
    accounts: SponsoredAccountsInputV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ExteriorActionV1 {
    Capture,
    Settle,
    CommitFailure,
    /// Project the committed Resolution certificate to Core.
    ///
    /// This is the one action whose instruction belongs to Core rather than to
    /// Resolution, and the one action with no [`SponsoredPushActionV1`] wire
    /// discriminant: Core's `AdmitTerminal` is a different program's route,
    /// built by `build_resolution_admit_terminal_v3` from the same finalized
    /// snapshot this command already authenticates.
    AdmitTerminal,
    /// Fund the certificate seat the next terminal walk will initialize.
    ///
    /// `initialize_certificate_at_kind` allocates and assigns; it does not
    /// fund. The seat must already hold `rent.minimum_balance(312)` when the
    /// walk runs, and on 2026-09-02 cohort-13's failure walk refused `0x8002`
    /// after 305,522 CU because nothing on the devnet path had put it there --
    /// the gauntlet's `prepay_certificate` is a local-only caller. This is the
    /// public arm of the same act, and it is a System transfer, not a protocol
    /// instruction.
    PrepayCertificate,
    CloseCandidate,
    CloseHead,
}

impl ExteriorActionV1 {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "capture" => Ok(Self::Capture),
            "settle" => Ok(Self::Settle),
            "commit-failure" => Ok(Self::CommitFailure),
            "admit-terminal" => Ok(Self::AdmitTerminal),
            "prepay-certificate" => Ok(Self::PrepayCertificate),
            "close-candidate" => Ok(Self::CloseCandidate),
            "close-head" => Ok(Self::CloseHead),
            _ => Err(Error::new(format!(
                "unknown sponsored-push action: {value}"
            ))),
        }
    }

    /// The Resolution wire action, absent for the Core-owned admission.
    const fn wire(self) -> Option<SponsoredPushActionV1> {
        match self {
            Self::Capture => Some(SponsoredPushActionV1::Capture),
            Self::Settle => Some(SponsoredPushActionV1::Settle),
            Self::CommitFailure => Some(SponsoredPushActionV1::CommitFailure),
            Self::AdmitTerminal | Self::PrepayCertificate => None,
            Self::CloseCandidate => Some(SponsoredPushActionV1::CloseCandidate),
            Self::CloseHead => Some(SponsoredPushActionV1::CloseHead),
        }
    }

    /// Whether the action's meaning includes a positive terminal sequence.
    ///
    /// The admission carries one for the same reason the two terminal walks do:
    /// the certificate it projects is addressed by that sequence, and Core
    /// recomputes it from the Source decision and refuses a mismatch.
    const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Settle | Self::CommitFailure | Self::AdmitTerminal | Self::PrepayCertificate
        )
    }

    /// Whether this action initializes a terminal certificate seat.
    const fn writes_a_certificate(self) -> bool {
        matches!(self, Self::Settle | Self::CommitFailure)
    }

    /// The certificate kind tag this walk's seat carries in its seeds.
    const fn certificate_kind(self) -> Option<u8> {
        match self {
            Self::Settle => Some(SUCCESS_CERTIFICATE_KIND),
            Self::CommitFailure => Some(FAILURE_CERTIFICATE_KIND),
            _ => None,
        }
    }

    /// Whether the transaction's fee payer must not appear in the instruction.
    ///
    /// These routes take no signer meta at all, so a payer that aliased one of
    /// their read-only protocol accounts would silently promote it to a
    /// writable signer in the compiled message.
    const fn payer_is_outside_the_frame(self) -> bool {
        matches!(
            self,
            Self::AdmitTerminal | Self::CloseCandidate | Self::CloseHead
        )
    }
}

#[derive(Default)]
struct ArgumentsV1 {
    rpc_url: Option<String>,
    acknowledgment: Option<String>,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    action: Option<ExteriorActionV1>,
    signer: Option<Pubkey>,
    signer_keypair: Option<PathBuf>,
    candidate: Option<Pubkey>,
    /// Which terminal walk's seat `prepay-certificate` funds.
    ///
    /// The seat address carries the certificate KIND in its seeds, and before
    /// the walk runs the Source phase cannot say which kind is coming -- it is
    /// still `Primary`. So the caller names the walk, in the same vocabulary
    /// `--action` uses, and anything but the two terminal walks refuses.
    prepay_for: Option<ExteriorActionV1>,
    execute: bool,
}

impl ArgumentsV1 {
    fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut parsed = Self::default();
        let mut iterator = arguments.into_iter();
        while let Some(argument) = iterator.next() {
            if argument == "--execute" {
                if parsed.execute {
                    return Err(Error::new("--execute may be supplied only once"));
                }
                parsed.execute = true;
                continue;
            }
            let value = iterator
                .next()
                .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
            match argument.as_str() {
                "--rpc-url" => set_once(&mut parsed.rpc_url, value, "--rpc-url")?,
                flag if flag == DEVNET_ACKNOWLEDGMENT_FLAG => set_once(
                    &mut parsed.acknowledgment,
                    value,
                    DEVNET_ACKNOWLEDGMENT_FLAG,
                )?,
                "--input" => set_once(&mut parsed.input, PathBuf::from(value), "--input")?,
                "--output" => set_once(&mut parsed.output, PathBuf::from(value), "--output")?,
                "--action" => set_once(
                    &mut parsed.action,
                    ExteriorActionV1::parse(&value)?,
                    "--action",
                )?,
                "--signer" => set_once(&mut parsed.signer, pubkey(&value)?, "--signer")?,
                "--signer-keypair" => set_once(
                    &mut parsed.signer_keypair,
                    PathBuf::from(value),
                    "--signer-keypair",
                )?,
                "--candidate" => set_once(&mut parsed.candidate, pubkey(&value)?, "--candidate")?,
                "--prepay-for" => set_once(
                    &mut parsed.prepay_for,
                    ExteriorActionV1::parse(&value)?,
                    "--prepay-for",
                )?,
                _ => {
                    return Err(Error::new(format!(
                        "unknown sponsored-push argument: {argument}"
                    )));
                }
            }
        }
        if !parsed.execute && parsed.signer_keypair.is_some() {
            return Err(Error::new(
                "--signer-keypair is refused during read-only preflight; add --execute",
            ));
        }
        if (parsed.action == Some(ExteriorActionV1::PrepayCertificate))
            != parsed.prepay_for.is_some()
        {
            return Err(Error::new(
                "--prepay-for is required exactly for prepay-certificate",
            ));
        }
        if parsed
            .prepay_for
            .is_some_and(|walk| !walk.writes_a_certificate())
        {
            return Err(Error::new(
                "--prepay-for must name settle or commit-failure: those are the two walks that initialize a certificate seat",
            ));
        }
        Ok(parsed)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlannedMetaV1 {
    address: String,
    signer: bool,
    writable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlannedInstructionV1 {
    program: String,
    data_base64: String,
    data_sha256: String,
    accounts: Vec<PlannedMetaV1>,
}

impl PlannedInstructionV1 {
    fn new(instruction: &Instruction) -> Self {
        Self {
            program: instruction.program_id.to_string(),
            data_base64: BASE64.encode(&instruction.data),
            data_sha256: hex(&Sha256::digest(&instruction.data)),
            accounts: instruction
                .accounts
                .iter()
                .map(|meta| PlannedMetaV1 {
                    address: meta.pubkey.to_string(),
                    signer: meta.is_signer,
                    writable: meta.is_writable,
                })
                .collect(),
        }
    }

    fn instruction(&self) -> Result<Instruction> {
        let data = BASE64
            .decode(&self.data_base64)
            .map_err(|error| Error::new(format!("planned instruction base64: {error}")))?;
        if hex(&Sha256::digest(&data)) != self.data_sha256 {
            return Err(Error::new("planned instruction digest changed"));
        }
        Ok(Instruction {
            program_id: pubkey(&self.program)?,
            accounts: self
                .accounts
                .iter()
                .map(|meta| {
                    let address = pubkey(&meta.address)?;
                    Ok(if meta.writable {
                        AccountMeta::new(address, meta.signer)
                    } else {
                        AccountMeta::new_readonly(address, meta.signer)
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            data,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ExteriorPhaseV1 {
    Planned,
    Prepared,
    Submitted,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExteriorReportV1 {
    format: String,
    cluster: String,
    input_sha256: String,
    action: ExteriorActionV1,
    phase: ExteriorPhaseV1,
    signer: String,
    observation_slot: u64,
    observation_unix_seconds: i64,
    sponsored_release: String,
    head: String,
    candidate: Option<String>,
    certificate: Option<String>,
    receipt: Option<String>,
    instruction: PlannedInstructionV1,
    lookup_table: RoutingTableEvidenceV1,
    prestate: BTreeMap<String, Option<AccountEvidence>>,
    signed_packet: Option<SignedVersionedPacketV1>,
    transaction: Option<TransactionEvidence>,
    poststate: BTreeMap<String, Option<AccountEvidence>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RoutingTableEvidenceV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_base64: String,
    data_sha256: String,
    account_sha256: String,
}

impl RoutingTableEvidenceV1 {
    fn new(table: &ObservedAccount) -> Self {
        Self {
            address: table.key.to_string(),
            owner: table.owner.to_string(),
            lamports: table.lamports,
            executable: table.executable,
            data_base64: BASE64.encode(&table.data),
            data_sha256: hex(&Sha256::digest(&table.data)),
            account_sha256: routing_table_digest(table),
        }
    }

    fn observed(&self, observation: Observation) -> Result<ObservedAccount> {
        let data = BASE64
            .decode(&self.data_base64)
            .map_err(|error| Error::new(format!("lookup table base64: {error}")))?;
        if BASE64.encode(&data) != self.data_base64
            || hex(&Sha256::digest(&data)) != self.data_sha256
        {
            return Err(Error::new("lookup table body digest changed"));
        }
        let table = ObservedAccount {
            observation,
            key: pubkey(&self.address)?,
            owner: pubkey(&self.owner)?,
            lamports: self.lamports,
            executable: self.executable,
            data,
        };
        if routing_table_digest(&table) != self.account_sha256 {
            return Err(Error::new("lookup table account digest changed"));
        }
        authenticate_frozen_routing_table(&table)?;
        Ok(table)
    }
}

struct SnapshotV1 {
    observation: Observation,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

impl SnapshotV1 {
    fn required(&self, key: Pubkey, label: &str) -> Result<&RpcAccount> {
        self.accounts
            .get(&key)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("finalized snapshot is missing {label} {key}")))
    }

    fn optional(&self, key: Pubkey) -> Option<&RpcAccount> {
        self.accounts.get(&key).and_then(Option::as_ref)
    }

    fn observed(&self, key: Pubkey, label: &str) -> Result<ObservedAccount> {
        let account = self.required(key, label)?;
        Ok(ObservedAccount {
            observation: self.observation,
            key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data.clone(),
        })
    }
}

struct CoordinatesV1 {
    release_id: [u8; 32],
    head: Pubkey,
    candidate: Option<Pubkey>,
    certificate: Option<Pubkey>,
    receipt: Option<Pubkey>,
}

pub(crate) fn run_devnet(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::Devnet)
}

pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::OwnedLoopback)
}

pub(crate) const fn usage() -> &'static str {
    "  dclutch-local-successor-bootstrap devnet-sponsored-push-v1 --rpc-url URL \
     --i-mean-devnet DEVNET_GENESIS --input ABSOLUTE_JSON --output ABSOLUTE_NEW_JSON \
     --action capture|settle|commit-failure|admit-terminal|prepay-certificate|close-candidate|close-head \
     --signer PUBKEY [--candidate PUBKEY] [--prepay-for settle|commit-failure] [--execute] \
     [--signer-keypair ABSOLUTE_JSON]"
}

pub(crate) const fn owned_loopback_usage() -> &'static str {
    "  dclutch-local-successor-bootstrap local-private-validator-sponsored-push-v1 \
     --rpc-url http://127.0.0.1:PORT --input ABSOLUTE_JSON --output ABSOLUTE_NEW_JSON \
     --action capture|settle|commit-failure|admit-terminal|prepay-certificate|close-candidate|close-head \
     --signer PUBKEY [--candidate PUBKEY] [--prepay-for settle|commit-failure] [--execute] \
     [--signer-keypair ABSOLUTE_JSON]"
}

fn run(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<()> {
    let args = ArgumentsV1::parse(arguments)?;
    let rpc_url = args
        .rpc_url
        .ok_or_else(|| Error::new("--rpc-url is required"))?;
    let origin = ClusterOriginV1::parse(&rpc_url, args.acknowledgment.as_deref())?;
    expected.authenticate(&origin)?;
    let input_path = absolute(args.input.map(|path| path.display().to_string()), "--input")?;
    let output_path = absolute(
        args.output.map(|path| path.display().to_string()),
        "--output",
    )?;
    let _report_lease = SponsoredReportLeaseV1::acquire(&output_path)?;
    let input_bytes = fs::read(&input_path)?;
    let input: SponsoredInputV1 = serde_json::from_slice(&input_bytes)
        .map_err(|error| Error::new(format!("sponsored input: {error}")))?;
    if input.format != INPUT_FORMAT || input.generation == 0 {
        return Err(Error::new("sponsored input format or generation refused"));
    }
    let action = args
        .action
        .ok_or_else(|| Error::new("--action is required"))?;
    if action.terminal() != (input.terminal_sequence != 0) {
        return Err(Error::new(
            "terminal sequence must be positive exactly for settle, commit-failure, or admit-terminal",
        ));
    }
    if (action == ExteriorActionV1::CloseCandidate) != args.candidate.is_some() {
        return Err(Error::new(
            "--candidate is required exactly for close-candidate",
        ));
    }
    let signer = args
        .signer
        .ok_or_else(|| Error::new("--signer is required"))?;
    let prepay_for = args.prepay_for;
    let input_sha256 = hex(&Sha256::digest(&input_bytes));
    let input_lookup_table = pubkey(&input.accounts.lookup_table)?;
    let mut rpc = Rpc::connect_cluster(
        &origin,
        if args.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    let existed = output_path.exists();
    let mut report = if existed {
        let bytes = fs::read(&output_path)?;
        let report: ExteriorReportV1 = serde_json::from_slice(&bytes)
            .map_err(|error| Error::new(format!("sponsored report: {error}")))?;
        authenticate_report(
            &report,
            expected,
            &input_sha256,
            action,
            signer,
            args.candidate,
            input_lookup_table,
        )?;
        report
    } else {
        let prepared = prepare(&mut rpc, &input, action, signer, args.candidate, prepay_for)?;
        let writable = writable_keys(&prepared.instruction);
        let report = ExteriorReportV1 {
            format: REPORT_FORMAT.to_owned(),
            cluster: expected.evidence_label().to_owned(),
            input_sha256: input_sha256.clone(),
            action,
            phase: ExteriorPhaseV1::Planned,
            signer: signer.to_string(),
            observation_slot: prepared.snapshot.observation.slot,
            observation_unix_seconds: prepared.snapshot.observation.unix_timestamp,
            sponsored_release: hex(&prepared.coordinates.release_id),
            head: prepared.coordinates.head.to_string(),
            candidate: prepared
                .coordinates
                .candidate
                .map(|value| value.to_string()),
            certificate: prepared
                .coordinates
                .certificate
                .map(|value| value.to_string()),
            receipt: prepared.coordinates.receipt.map(|value| value.to_string()),
            instruction: PlannedInstructionV1::new(&prepared.instruction),
            lookup_table: RoutingTableEvidenceV1::new(&prepared.lookup_table),
            prestate: evidence_for(&prepared.snapshot, &writable),
            signed_packet: None,
            transaction: None,
            poststate: BTreeMap::new(),
        };
        write_report_new(&output_path, &report)?;
        report
    };
    if !existed {
        println!("{}", serde_json::to_string_pretty(&report)?);
        if args.execute {
            return Err(Error::new(
                "created a key-free planned report; review it, then rerun the exact command with the same output path to sign",
            ));
        }
        return Ok(());
    }
    if !args.execute || report.phase == ExteriorPhaseV1::Finalized {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let label = format!("sponsored-push-{action:?}");
    if report.phase == ExteriorPhaseV1::Planned {
        let keypair_path = args.signer_keypair.as_deref().ok_or_else(|| {
            Error::new("a planned action requires --signer-keypair with --execute")
        })?;
        // Reauthenticate every live input immediately before signing.  The
        // reviewed preflight remains the authority: if a mutable provider body
        // moved the candidate, the caller must create and review a new report.
        let prepared = prepare(&mut rpc, &input, action, signer, args.candidate, prepay_for)?;
        let current = PlannedInstructionV1::new(&prepared.instruction);
        authenticate_reprepared_coordinates(&report, &prepared.coordinates)?;
        if current != report.instruction {
            return Err(Error::new(
                "finalized inputs changed after preflight; use a new output path and review the new action",
            ));
        }
        if routing_table_digest(&prepared.lookup_table) != report.lookup_table.account_sha256 {
            return Err(Error::new(
                "frozen sponsored routing table changed after preflight",
            ));
        }
        let keypair = Keypair::new_from_array(read_keypair_file(keypair_path, "sponsored signer")?);
        if keypair.pubkey() != signer {
            return Err(Error::new("--signer does not match --signer-keypair"));
        }
        let writable = writable_keys(&prepared.instruction);
        report.observation_slot = prepared.snapshot.observation.slot;
        report.observation_unix_seconds = prepared.snapshot.observation.unix_timestamp;
        report.prestate = evidence_for(&prepared.snapshot, &writable);
        report.signed_packet = Some(rpc.prepare_signed_v0_packet(
            &label,
            std::slice::from_ref(&prepared.instruction),
            &keypair,
            &prepared.lookup_table,
        )?);
        report.phase = ExteriorPhaseV1::Prepared;
        write_report_replace(&output_path, &report)?;
    }

    let instruction = report.instruction.instruction()?;
    let lookup_table = report.lookup_table.observed(Observation {
        slot: report.observation_slot,
        unix_timestamp: report.observation_unix_seconds,
        finality: Finality::Finalized,
    })?;
    let packet = report
        .signed_packet
        .as_ref()
        .ok_or_else(|| Error::new("prepared report omitted its signed packet"))?;
    if report.phase == ExteriorPhaseV1::Prepared {
        rpc.submit_signed_v0_packet(
            &label,
            std::slice::from_ref(&instruction),
            signer,
            &lookup_table,
            packet,
        )?;
        report.phase = ExteriorPhaseV1::Submitted;
        write_report_replace(&output_path, &report)?;
    }
    if report.phase != ExteriorPhaseV1::Submitted {
        return Err(Error::new("sponsored report phase refused"));
    }
    let packet = report
        .signed_packet
        .as_ref()
        .ok_or_else(|| Error::new("submitted report omitted its signed packet"))?;
    report.transaction = Some(rpc.confirm_signed_v0_packet(
        &label,
        std::slice::from_ref(&instruction),
        signer,
        &lookup_table,
        packet,
    )?);
    let writable = writable_keys(&instruction);
    let post = read_snapshot(&mut rpc, &writable, report.observation_slot)?;
    report.poststate = evidence_for(&post, &writable);
    report.phase = ExteriorPhaseV1::Finalized;
    authenticate_report(
        &report,
        expected,
        &input_sha256,
        action,
        signer,
        args.candidate,
        input_lookup_table,
    )?;
    write_report_replace(&output_path, &report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn authenticate_report(
    report: &ExteriorReportV1,
    expected: ExpectedClusterV1,
    input_sha256: &str,
    action: ExteriorActionV1,
    signer: Pubkey,
    candidate: Option<Pubkey>,
    input_lookup_table: Pubkey,
) -> Result<()> {
    let close_candidate_matches = action != ExteriorActionV1::CloseCandidate
        || report.candidate == candidate.map(|value| value.to_string());
    if report.format != REPORT_FORMAT
        || report.cluster != expected.evidence_label()
        || report.input_sha256 != input_sha256
        || report.action != action
        || report.signer != signer.to_string()
        || !close_candidate_matches
        || report.lookup_table.address != input_lookup_table.to_string()
    {
        return Err(Error::new(
            "existing sponsored report does not match this exact cluster/input/action/signer",
        ));
    }
    let instruction = report.instruction.instruction()?;
    if instruction.program_id.to_string() != report.instruction.program {
        return Err(Error::new("sponsored report instruction program changed"));
    }
    authenticate_report_coordinates(report, &instruction)?;
    authenticate_evidence_keys(&report.prestate, &instruction, "prestate")?;
    let table = report.lookup_table.observed(Observation {
        slot: report.observation_slot,
        unix_timestamp: report.observation_unix_seconds,
        finality: Finality::Finalized,
    })?;
    let shape_ok = match report.phase {
        ExteriorPhaseV1::Planned => {
            report.signed_packet.is_none()
                && report.transaction.is_none()
                && report.poststate.is_empty()
        }
        ExteriorPhaseV1::Prepared | ExteriorPhaseV1::Submitted => {
            report.signed_packet.is_some()
                && report.transaction.is_none()
                && report.poststate.is_empty()
        }
        ExteriorPhaseV1::Finalized => {
            report.signed_packet.is_some()
                && report.transaction.as_ref().is_some_and(|transaction| {
                    report
                        .signed_packet
                        .as_ref()
                        .is_some_and(|packet| packet.signature == transaction.signature)
                })
                && !report.poststate.is_empty()
        }
    };
    if !shape_ok {
        return Err(Error::new("existing sponsored report phase shape refused"));
    }
    if let Some(packet) = report.signed_packet.as_ref() {
        if packet.last_valid_block_height == 0 {
            return Err(Error::new(
                "existing sponsored report omitted packet expiry height",
            ));
        }
        Rpc::authenticate_signed_v0_packet(
            "sponsored report",
            std::slice::from_ref(&instruction),
            signer,
            &table,
            packet,
        )?;
    }
    if report.phase == ExteriorPhaseV1::Finalized {
        let transaction = report
            .transaction
            .as_ref()
            .ok_or_else(|| Error::new("finalized sponsored report omitted transaction"))?;
        let expected_label = format!("sponsored-push-{action:?}");
        if transaction.label != expected_label
            || transaction.signature
                != report
                    .signed_packet
                    .as_ref()
                    .map(|packet| packet.signature.as_str())
                    .unwrap_or_default()
            || transaction.slot < report.observation_slot
            || !transaction.transaction_metadata_available
            || transaction.fee_lamports.is_none()
            || transaction.compute_units_consumed.is_none()
            || transaction.error.is_some()
        {
            return Err(Error::new(
                "finalized sponsored transaction identity or success evidence changed",
            ));
        }
        authenticate_evidence_keys(&report.poststate, &instruction, "poststate")?;
    }
    Ok(())
}

fn authenticate_evidence_keys(
    evidence: &BTreeMap<String, Option<AccountEvidence>>,
    instruction: &Instruction,
    label: &str,
) -> Result<()> {
    let expected = writable_keys(instruction)
        .into_iter()
        .map(|key| key.to_string())
        .collect::<BTreeSet<_>>();
    if evidence.keys().cloned().collect::<BTreeSet<_>>() != expected
        || evidence.iter().any(|(key, value)| {
            value
                .as_ref()
                .is_some_and(|account| account.address != *key)
        })
    {
        return Err(Error::new(format!(
            "sponsored report {label} coordinate set changed"
        )));
    }
    Ok(())
}

fn authenticate_report_coordinates(
    report: &ExteriorReportV1,
    instruction: &Instruction,
) -> Result<()> {
    let account = |index: usize| {
        instruction
            .accounts
            .get(index)
            .map(|meta| meta.pubkey.to_string())
            .ok_or_else(|| Error::new("sponsored report instruction width changed"))
    };
    let required_matches =
        |index: usize, value: &str| -> Result<bool> { Ok(account(index)? == value) };
    let optional_matches = |index: usize, value: Option<&str>| -> Result<bool> {
        value
            .map(|value| required_matches(index, value))
            .transpose()
            .map(|value| value == Some(true))
    };
    let same = match report.action {
        ExteriorActionV1::Capture => {
            required_matches(4, &report.head)? && optional_matches(5, report.candidate.as_deref())?
        }
        ExteriorActionV1::Settle => {
            required_matches(4, &report.head)?
                && optional_matches(5, report.candidate.as_deref())?
                && optional_matches(7, report.certificate.as_deref())?
                && optional_matches(8, report.receipt.as_deref())?
        }
        ExteriorActionV1::CommitFailure => {
            report.candidate.is_none()
                && optional_matches(5, report.certificate.as_deref())?
                && report.receipt.is_none()
                && required_matches(22, &report.head)?
        }
        // Core's own `ADMIT_CERTIFICATE` index. The head is still recorded as a
        // derived coordinate of this market, and still pinned by
        // `authenticate_reprepared_coordinates`, but Core's admission frame does
        // not carry it: the certificate is what it authenticates.
        ExteriorActionV1::AdmitTerminal => {
            report.candidate.is_none()
                && report.receipt.is_none()
                && optional_matches(14, report.certificate.as_deref())?
        }
        // A System transfer names its destination at index 1.
        ExteriorActionV1::PrepayCertificate => {
            report.candidate.is_none()
                && report.receipt.is_none()
                && optional_matches(1, report.certificate.as_deref())?
        }
        ExteriorActionV1::CloseCandidate => {
            optional_matches(2, report.candidate.as_deref())?
                && report.certificate.is_none()
                && report.receipt.is_none()
        }
        ExteriorActionV1::CloseHead => {
            required_matches(2, &report.head)?
                && report.candidate.is_none()
                && report.certificate.is_none()
                && report.receipt.is_none()
        }
    };
    if !same {
        return Err(Error::new(
            "sponsored report coordinates differ from its authenticated instruction",
        ));
    }
    Ok(())
}

fn authenticate_reprepared_coordinates(
    report: &ExteriorReportV1,
    coordinates: &CoordinatesV1,
) -> Result<()> {
    let same = report.sponsored_release == hex(&coordinates.release_id)
        && report.head == coordinates.head.to_string()
        && report.candidate == coordinates.candidate.map(|value| value.to_string())
        && report.certificate == coordinates.certificate.map(|value| value.to_string())
        && report.receipt == coordinates.receipt.map(|value| value.to_string());
    if !same {
        return Err(Error::new(
            "derived sponsored coordinates changed after preflight; use a new output path",
        ));
    }
    Ok(())
}

fn routing_table_digest(table: &ObservedAccount) -> String {
    let mut hasher = Sha256::new();
    hasher.update(table.key.as_ref());
    hasher.update(table.owner.as_ref());
    hasher.update(table.lamports.to_le_bytes());
    hasher.update([u8::from(table.executable)]);
    hasher.update(&table.data);
    hex(&hasher.finalize())
}

fn authenticate_frozen_routing_table(table: &ObservedAccount) -> Result<()> {
    if table.owner != lookup_table_program::ID || table.executable {
        return Err(Error::new(
            "sponsored routing table is not an Address Lookup Table account",
        ));
    }
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| Error::new("sponsored routing table body refused"))?;
    if decoded.meta.authority.is_some()
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= table.observation.slot
        || decoded.addresses.is_empty()
    {
        return Err(Error::new(
            "sponsored routing table must be finalized, frozen, active, and nonempty",
        ));
    }
    Ok(())
}

struct PreparedV1 {
    snapshot: SnapshotV1,
    coordinates: CoordinatesV1,
    instruction: Instruction,
    lookup_table: ObservedAccount,
}

fn prepare(
    rpc: &mut Rpc,
    input: &SponsoredInputV1,
    action: ExteriorActionV1,
    signer: Pubkey,
    requested_candidate: Option<Pubkey>,
    prepay_for: Option<ExteriorActionV1>,
) -> Result<PreparedV1> {
    let keys = InputKeysV1::parse(input)?;
    let release_account = rpc.required_account(keys.sponsored_release.0, "sponsored release")?;
    let release = authenticate_release(input, &keys, &release_account)?;
    let release_id = hash(&release.to_bytes()).to_bytes();
    let generation = input.generation.to_le_bytes();
    let head = Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
            keys.market.as_ref(),
            &generation,
            &release_id,
        ],
        &keys.resolution_program,
    )
    .0;
    let price_account = rpc.required_account(keys.price_account, "sponsored price")?;
    let update = authenticate_price(&release, keys.price_account, &price_account)?;
    let candidate = match action {
        ExteriorActionV1::Capture => Some(candidate_address(
            keys.resolution_program,
            keys.market,
            input.generation,
            release_id,
            keys.price_account,
            update,
            &price_account.data,
        )),
        ExteriorActionV1::CloseCandidate => requested_candidate,
        ExteriorActionV1::Settle => {
            let head_account = rpc.required_account(head, "sponsored head")?;
            let decoded = SponsoredPushHeadV1::decode(&head_account.data)
                .map_err(|error| Error::new(format!("sponsored head: {error:?}")))?;
            Some(Pubkey::new_from_array(decoded.best_candidate))
        }
        ExteriorActionV1::CommitFailure
        | ExteriorActionV1::AdmitTerminal
        | ExteriorActionV1::PrepayCertificate
        | ExteriorActionV1::CloseHead => None,
    };
    let certificate = match action {
        ExteriorActionV1::Settle => Some(certificate_address(
            keys.resolution_program,
            keys.source_state,
            SUCCESS_CERTIFICATE_KIND,
            input.terminal_sequence,
        )),
        ExteriorActionV1::CommitFailure => Some(certificate_address(
            keys.resolution_program,
            keys.source_state,
            FAILURE_CERTIFICATE_KIND,
            input.terminal_sequence,
        )),
        // The admission does not choose a certificate kind: the terminal Source
        // it is projecting already did, and reading it anywhere else would let
        // a caller ask Core to accept a success certificate for a failed walk.
        ExteriorActionV1::AdmitTerminal => Some(certificate_address(
            keys.resolution_program,
            keys.source_state,
            terminal_certificate_kind(rpc, keys.source_state, keys.resolution_program)?,
            input.terminal_sequence,
        )),
        // The seat is addressed by the walk the caller named, not by a phase
        // that has not happened yet.
        ExteriorActionV1::PrepayCertificate => Some(certificate_address(
            keys.resolution_program,
            keys.source_state,
            prepay_for
                .and_then(ExteriorActionV1::certificate_kind)
                .ok_or_else(|| Error::new("prepay-certificate requires --prepay-for"))?,
            input.terminal_sequence,
        )),
        _ => None,
    };
    let receipt = (action == ExteriorActionV1::Settle).then(|| {
        Pubkey::find_program_address(
            &[
                SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1,
                keys.source_state.as_ref(),
                &input.terminal_sequence.to_le_bytes(),
            ],
            &keys.resolution_program,
        )
        .0
    });
    let mut addresses = keys.all_accounts();
    addresses.extend([
        head,
        signer,
        sysvar::clock::ID,
        sysvar::rent::ID,
        system_program::ID,
    ]);
    if let Some(value) = candidate {
        addresses.push(value);
    }
    if let Some(value) = certificate {
        addresses.push(value);
    }
    if let Some(value) = receipt {
        addresses.push(value);
    }
    let snapshot = read_snapshot(rpc, &dedupe(addresses), 0)?;
    authenticate_final_coordinates(
        input,
        action,
        &keys,
        &snapshot,
        &CoordinatesV1 {
            release_id,
            head,
            candidate,
            certificate,
            receipt,
        },
        release,
    )?;
    authenticate_resolution_role(input, &keys, &snapshot)?;
    authenticate_source_provider_join(&keys, &snapshot, release)?;
    let coordinates = CoordinatesV1 {
        release_id,
        head,
        candidate,
        certificate,
        receipt,
    };
    let instruction = build_instruction(input, action, signer, &keys, &coordinates, &snapshot)?;
    require_distinct_metas(&instruction)?;
    let lookup_table = snapshot.observed(keys.lookup_table, "sponsored routing table")?;
    authenticate_frozen_routing_table(&lookup_table)?;
    if action.payer_is_outside_the_frame()
        && instruction
            .accounts
            .iter()
            .any(|meta| meta.pubkey == signer)
    {
        return Err(Error::new(
            "cleanup transaction payer aliases a read-only protocol account; use a distinct permissionless worker",
        ));
    }
    Ok(PreparedV1 {
        snapshot,
        coordinates,
        instruction,
        lookup_table,
    })
}

#[derive(Clone)]
struct InputKeysV1 {
    registry_program: Pubkey,
    activation_cache: Pubkey,
    core_program: Pubkey,
    /// `None` when the input predates the terminal-admission arm.
    core_programdata: Option<Pubkey>,
    resolution_program: Pubkey,
    resolution_programdata: Pubkey,
    market: Pubkey,
    source_state: Pubkey,
    source_material: (Pubkey, Pubkey),
    source_spec: (Pubkey, Pubkey),
    provider_release: (Pubkey, Pubkey),
    adapter_config: (Pubkey, Pubkey),
    window: (Pubkey, Pubkey),
    statistic: (Pubkey, Pubkey),
    sponsored_release: (Pubkey, Pubkey),
    product: (Pubkey, Pubkey),
    result_domain: (Pubkey, Pubkey),
    portfolio: (Pubkey, Pubkey),
    capability_manifest: (Pubkey, Pubkey),
    failure_funding: Pubkey,
    price_account: Pubkey,
    receiver_program: Pubkey,
    receiver_programdata: Pubkey,
    push_oracle_program: Pubkey,
    push_oracle_programdata: Pubkey,
    receiver_config: Pubkey,
    lookup_table: Pubkey,
}

impl InputKeysV1 {
    fn parse(input: &SponsoredInputV1) -> Result<Self> {
        let value = &input.accounts;
        Ok(Self {
            registry_program: pubkey(&value.registry_program)?,
            activation_cache: pubkey(&value.activation_cache)?,
            core_program: pubkey(&value.core_program)?,
            core_programdata: if value.core_programdata.is_empty() {
                None
            } else {
                Some(pubkey(&value.core_programdata)?)
            },
            resolution_program: pubkey(&value.resolution_program)?,
            resolution_programdata: pubkey(&value.resolution_programdata)?,
            market: pubkey(&value.market)?,
            source_state: pubkey(&value.source_state)?,
            source_material: value.source_material.keys()?,
            source_spec: value.source_spec.keys()?,
            provider_release: value.provider_release.keys()?,
            adapter_config: value.adapter_config.keys()?,
            window: value.window.keys()?,
            statistic: value.statistic.keys()?,
            sponsored_release: value.sponsored_release.keys()?,
            product: value.product.keys()?,
            result_domain: value.result_domain.keys()?,
            portfolio: value.portfolio.keys()?,
            capability_manifest: value.capability_manifest.keys()?,
            failure_funding: pubkey(&value.failure_funding)?,
            price_account: pubkey(&value.price_account)?,
            receiver_program: pubkey(&value.receiver_program)?,
            receiver_programdata: pubkey(&value.receiver_programdata)?,
            push_oracle_program: pubkey(&value.push_oracle_program)?,
            push_oracle_programdata: pubkey(&value.push_oracle_programdata)?,
            receiver_config: pubkey(&value.receiver_config)?,
            lookup_table: pubkey(&value.lookup_table)?,
        })
    }

    /// Core's ProgramData, refused by name when the input omits it.
    fn core_programdata(&self) -> Result<Pubkey> {
        self.core_programdata.ok_or_else(|| {
            Error::new("terminal admission requires accounts.coreProgramdata; this input omits it")
        })
    }

    fn all_accounts(&self) -> Vec<Pubkey> {
        let mut out = vec![
            self.registry_program,
            self.activation_cache,
            self.core_program,
            self.resolution_program,
            self.resolution_programdata,
            self.market,
            self.source_state,
            self.failure_funding,
            self.price_account,
            self.receiver_program,
            self.receiver_programdata,
            self.push_oracle_program,
            self.push_oracle_programdata,
            self.receiver_config,
            self.lookup_table,
        ];
        out.extend(self.core_programdata);
        for pair in [
            self.source_material,
            self.source_spec,
            self.provider_release,
            self.adapter_config,
            self.window,
            self.statistic,
            self.sponsored_release,
            self.product,
            self.result_domain,
            self.portfolio,
            self.capability_manifest,
        ] {
            out.extend([pair.0, pair.1]);
        }
        out
    }
}

fn authenticate_release(
    input: &SponsoredInputV1,
    keys: &InputKeysV1,
    account: &RpcAccount,
) -> Result<PythSponsoredPushReleaseV1> {
    let release = PythSponsoredPushReleaseV1::decode(&account.data)
        .map_err(|error| Error::new(format!("sponsored release: {error:?}")))?;
    let compiled = devnet_sponsored_sol_usd_release_v1()
        .map_err(|error| Error::new(format!("compiled sponsored release: {error:?}")))?;
    if release.to_bytes() != compiled.to_bytes()
        || account.owner != keys.registry_program
        || input.release_set.len() != 64
    {
        return Err(Error::new(
            "sponsored release or release-set identity refused",
        ));
    }
    Ok(release)
}

fn authenticate_price(
    release: &PythSponsoredPushReleaseV1,
    key: Pubkey,
    account: &RpcAccount,
) -> Result<FullPriceUpdateV2> {
    if key.to_bytes() != release.price_account()
        || account.owner.to_bytes() != release.receiver_program()
        || account.executable
        || account.data.len() != FULL_PRICE_UPDATE_V2_LEN
    {
        return Err(Error::new(
            "sponsored price account identity or body refused",
        ));
    }
    let update = FullPriceUpdateV2::parse(&account.data)
        .map_err(|error| Error::new(format!("sponsored price body: {error:?}")))?;
    if update.write_authority() != key.to_bytes()
        || update.feed_id() != release.feed_id()
        || update.publish_time() <= 0
        || update.posted_slot() == 0
        || update.prev_publish_time() > update.publish_time()
    {
        return Err(Error::new("sponsored price body relations refused"));
    }
    Ok(update)
}

fn authenticate_resolution_role(
    input: &SponsoredInputV1,
    keys: &InputKeysV1,
    snapshot: &SnapshotV1,
) -> Result<()> {
    let release_set = hex32(&input.release_set)?;
    authenticate_role(
        &snapshot.observed(keys.registry_program, "Registry program")?,
        &snapshot.observed(keys.activation_cache, "activation cache")?,
        release_set,
        ExecutionRoleV1::Resolution,
        &snapshot.observed(keys.resolution_program, "Resolution program")?,
        &snapshot.observed(keys.resolution_programdata, "Resolution ProgramData")?,
    )
    .map_err(Into::into)
}

fn authenticate_source_provider_join(
    keys: &InputKeysV1,
    snapshot: &SnapshotV1,
    release: PythSponsoredPushReleaseV1,
) -> Result<()> {
    let source = SourceSpecV1::decode(&snapshot.required(keys.source_spec.0, "SourceSpec")?.data)
        .map_err(|error| Error::new(format!("SourceSpec: {error:?}")))?;
    let provider = ProviderReleaseV1::decode(
        &snapshot
            .required(keys.provider_release.0, "ProviderRelease")?
            .data,
    )
    .map_err(|error| Error::new(format!("ProviderRelease: {error:?}")))?;
    let provider_id = hash(&provider.to_bytes()).to_bytes();
    let release_id = hash(&release.to_bytes()).to_bytes();
    if source.access_profile() != SourceAccessProfile::PythSponsoredPushSnapshot
        || source.provider_release_id().to_bytes() != provider_id
        || provider.provider_deployment_release_id().to_bytes() != release_id
        || provider.provider_family_id().to_bytes() != release.provider_family_id()
        || provider.adapter_release_id().to_bytes() != release.adapter_id()
        || provider.decoding_rules_id().to_bytes() != release.price_update_codec_id()
        || provider.transport_profile_id().to_bytes() != release.transport_profile_id()
    {
        return Err(Error::new("Source/provider/sponsored release join refused"));
    }
    Ok(())
}

fn authenticate_final_coordinates(
    input: &SponsoredInputV1,
    action: ExteriorActionV1,
    keys: &InputKeysV1,
    snapshot: &SnapshotV1,
    coordinates: &CoordinatesV1,
    initial_release: PythSponsoredPushReleaseV1,
) -> Result<()> {
    let final_release = authenticate_release(
        input,
        keys,
        snapshot.required(keys.sponsored_release.0, "sponsored release")?,
    )?;
    if final_release.to_bytes() != initial_release.to_bytes()
        || hash(&final_release.to_bytes()).to_bytes() != coordinates.release_id
    {
        return Err(Error::new(
            "sponsored release changed between finalized observations",
        ));
    }
    let final_price_account = snapshot.required(keys.price_account, "sponsored price")?;
    let final_update = authenticate_price(&final_release, keys.price_account, final_price_account)?;
    match action {
        ExteriorActionV1::Capture => {
            let expected = candidate_address(
                keys.resolution_program,
                keys.market,
                input.generation,
                coordinates.release_id,
                keys.price_account,
                final_update,
                &final_price_account.data,
            );
            if coordinates.candidate != Some(expected) {
                return Err(Error::new(
                    "sponsored price changed between finalized observations; retry preflight",
                ));
            }
        }
        ExteriorActionV1::Settle => {
            let head = SponsoredPushHeadV1::decode(
                &snapshot.required(coordinates.head, "sponsored head")?.data,
            )
            .map_err(|error| Error::new(format!("sponsored head: {error:?}")))?;
            let candidate = coordinates
                .candidate
                .ok_or_else(|| Error::new("settle candidate missing"))?;
            let candidate_record = SponsoredPushCandidateV1::decode(
                &snapshot.required(candidate, "selected candidate")?.data,
            )
            .map_err(|error| Error::new(format!("selected candidate: {error:?}")))?;
            if head.best_candidate != candidate.to_bytes()
                || head.market != keys.market.to_bytes()
                || head.source_state != keys.source_state.to_bytes()
                || head.sponsored_release != coordinates.release_id
                || head.generation != input.generation
                || candidate_record.market != keys.market.to_bytes()
                || candidate_record.source_state != keys.source_state.to_bytes()
                || candidate_record.sponsored_release != coordinates.release_id
                || candidate_record.generation != input.generation
            {
                return Err(Error::new(
                    "selected head/candidate/release binding changed or was substituted",
                ));
            }
        }
        ExteriorActionV1::CommitFailure => {
            if snapshot.optional(coordinates.head).is_some_and(|head| {
                head.owner != system_program::ID || head.executable || !head.data.is_empty()
            }) {
                return Err(Error::new(
                    "commit-failure requires the canonical sponsored head to be vacant",
                ));
            }
        }
        // The certificate kind was chosen from a Source read taken before the
        // snapshot. Re-derive it from the snapshot's own Source body: if the
        // phase moved between the two finalized reads, the planned address is
        // for a certificate this Source no longer names.
        ExteriorActionV1::AdmitTerminal => {
            let source = SourceResolutionStateV2::decode(
                &snapshot
                    .required(keys.source_state, "Source resolution state")?
                    .data,
            )
            .map_err(|error| Error::new(format!("Source resolution state: {error:?}")))?;
            let kind = match source.phase() {
                SourceResolutionPhaseV1::Resolved => SUCCESS_CERTIFICATE_KIND,
                SourceResolutionPhaseV1::FailureCommitted => FAILURE_CERTIFICATE_KIND,
                other => {
                    return Err(Error::new(format!(
                        "terminal admission requires a Resolved or FailureCommitted Source; this one is {other:?}"
                    )));
                }
            };
            let expected = certificate_address(
                keys.resolution_program,
                keys.source_state,
                kind,
                input.terminal_sequence,
            );
            if coordinates.certificate != Some(expected) {
                return Err(Error::new(
                    "Source terminal phase changed between finalized observations; retry preflight",
                ));
            }
        }
        // The seat must still be exactly what a prepay is for: System-owned,
        // bodiless, and short of its rent. Each of the three is refused by its
        // own name, because "the transfer refused" and "the walk already ran"
        // are different facts and the second one is not an error.
        ExteriorActionV1::PrepayCertificate => {
            let seat = coordinates
                .certificate
                .ok_or_else(|| Error::new("prepay-certificate seat missing"))?;
            if let Some(account) = snapshot.optional(seat) {
                if account.owner != system_program::ID || account.executable {
                    return Err(Error::new(format!(
                        "certificate seat {seat} is already occupied by {}; a prepay funds a seat the walk has not taken yet",
                        account.owner
                    )));
                }
                if !account.data.is_empty() {
                    return Err(Error::new(format!(
                        "certificate seat {seat} already carries {} bytes; this is not a vacant seat",
                        account.data.len()
                    )));
                }
            }
        }
        ExteriorActionV1::CloseCandidate | ExteriorActionV1::CloseHead => {}
    }
    Ok(())
}

/// Read the terminal certificate kind off the Source state itself.
///
/// `Resolved` and `FailureCommitted` are the only two phases Core's
/// `authenticate_admit_projection` admits, and each pins one certificate kind
/// tag in the PDA seeds. Every other phase is refused here, by name, before a
/// snapshot is taken — an admission built for a Source still walking would
/// otherwise be planned in full and refused only by the chain.
fn terminal_certificate_kind(
    rpc: &mut Rpc,
    source_state: Pubkey,
    resolution_program: Pubkey,
) -> Result<u8> {
    let account = rpc.required_account(source_state, "Source resolution state")?;
    if account.owner != resolution_program || account.executable {
        return Err(Error::new(
            "Source resolution state is not owned by the selected Resolution deployment",
        ));
    }
    let source = SourceResolutionStateV2::decode(&account.data)
        .map_err(|error| Error::new(format!("Source resolution state: {error:?}")))?;
    match source.phase() {
        SourceResolutionPhaseV1::Resolved => Ok(SUCCESS_CERTIFICATE_KIND),
        SourceResolutionPhaseV1::FailureCommitted => Ok(FAILURE_CERTIFICATE_KIND),
        other => Err(Error::new(format!(
            "terminal admission requires a Resolved or FailureCommitted Source; this one is {other:?}"
        ))),
    }
}

fn candidate_address(
    program: Pubkey,
    market: Pubkey,
    generation: u64,
    release: [u8; 32],
    price: Pubkey,
    update: FullPriceUpdateV2,
    update_bytes: &[u8],
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1,
            market.as_ref(),
            &generation.to_le_bytes(),
            &release,
            price.as_ref(),
            &update.publish_time().to_le_bytes(),
            &update.posted_slot().to_le_bytes(),
            &hash(update_bytes).to_bytes(),
        ],
        &program,
    )
    .0
}

fn certificate_address(program: Pubkey, source: Pubkey, kind: u8, sequence: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source.as_ref(),
            &[kind],
            &sequence.to_le_bytes(),
        ],
        &program,
    )
    .0
}

fn build_instruction(
    input: &SponsoredInputV1,
    action: ExteriorActionV1,
    signer: Pubkey,
    keys: &InputKeysV1,
    coordinates: &CoordinatesV1,
    snapshot: &SnapshotV1,
) -> Result<Instruction> {
    if action == ExteriorActionV1::AdmitTerminal {
        return admit_terminal_instruction(input, keys, coordinates, snapshot);
    }
    if action == ExteriorActionV1::PrepayCertificate {
        return prepay_certificate_instruction(signer, coordinates, snapshot);
    }
    let data = SponsoredPushInstructionV1 {
        action: action
            .wire()
            .ok_or_else(|| Error::new("this action has no sponsored-push wire discriminant"))?,
        generation: input.generation,
        terminal_sequence: input.terminal_sequence,
    }
    .to_bytes()
    .map_err(|error| Error::new(format!("sponsored wire: {error:?}")))?
    .to_vec();
    let accounts = match action {
        ExteriorActionV1::Capture => capture_metas(signer, keys, coordinates)?,
        ExteriorActionV1::Settle => settle_metas(signer, keys, coordinates, snapshot)?,
        ExteriorActionV1::CommitFailure => failure_metas(signer, keys, coordinates)?,
        ExteriorActionV1::AdmitTerminal | ExteriorActionV1::PrepayCertificate => {
            return Err(Error::new(
                "the Core admission and the seat prepay build their own frames",
            ));
        }
        ExteriorActionV1::CloseCandidate => close_candidate_metas(keys, coordinates, snapshot)?,
        ExteriorActionV1::CloseHead => close_head_metas(keys, coordinates, snapshot)?,
    };
    Ok(Instruction {
        program_id: keys.resolution_program,
        accounts,
        data,
    })
}

/// Build Core's `AdmitTerminal` from the same finalized snapshot.
///
/// Nothing here is hand-assembled: `build_resolution_admit_terminal_v3` owns
/// the frame, the role request, the caller-authority derivation and every
/// conjunct Core will recheck, and it is the same builder the relayed ladder's
/// `accept` stage calls. The route that was missing on devnet was never the
/// capability — it was a producer that could reach the builder for a market
/// whose Source resolved from a sponsored snapshot rather than a relayed VAA.
///
/// The three guards after the call are the ones a caller can still get wrong
/// by pointing this input at another deployment: the instruction must belong to
/// the Core program this input names, it must carry no signer meta (Core's
/// admission is permissionless, and a signer meta would make the fee payer
/// authorize it), and its terminal sequence must be the one the input declares
/// rather than one the builder happened to find.
fn admit_terminal_instruction(
    input: &SponsoredInputV1,
    keys: &InputKeysV1,
    coordinates: &CoordinatesV1,
    snapshot: &SnapshotV1,
) -> Result<Instruction> {
    let certificate = coordinates
        .certificate
        .ok_or_else(|| Error::new("terminal admission certificate missing"))?;
    let report = build_resolution_admit_terminal_v3(&ResolutionAdmitTerminalSnapshotV3 {
        market: snapshot.observed(keys.market, "Market")?,
        activation_cache: snapshot.observed(keys.activation_cache, "activation cache")?,
        registry_program: snapshot.observed(keys.registry_program, "Registry program")?,
        core_program: snapshot.observed(keys.core_program, "Core program")?,
        core_programdata: snapshot.observed(keys.core_programdata()?, "Core ProgramData")?,
        resolution_program: snapshot.observed(keys.resolution_program, "Resolution program")?,
        resolution_programdata: snapshot
            .observed(keys.resolution_programdata, "Resolution ProgramData")?,
        source_material: snapshot.observed(keys.source_material.0, "SourceMaterial")?,
        source_material_staging: observed_or_vacant(snapshot, keys.source_material.1)?,
        capability_manifest: snapshot.observed(keys.capability_manifest.0, "CapabilityManifest")?,
        capability_manifest_staging: observed_or_vacant(snapshot, keys.capability_manifest.1)?,
        source_state: snapshot.observed(keys.source_state, "Source resolution state")?,
        funding_ledger: snapshot.observed(keys.failure_funding, "Resolution funding ledger")?,
        certificate: snapshot.observed(certificate, "terminal certificate")?,
        rent_sysvar: snapshot.observed(sysvar::rent::ID, "Rent sysvar")?,
        product_raw: snapshot.observed(keys.product.0, "Product")?,
        product_staging: observed_or_vacant(snapshot, keys.product.1)?,
        result_domain_raw: snapshot.observed(keys.result_domain.0, "ResultDomain")?,
        result_domain_staging: observed_or_vacant(snapshot, keys.result_domain.1)?,
        portfolio_raw: snapshot.observed(keys.portfolio.0, "Portfolio")?,
        portfolio_staging: observed_or_vacant(snapshot, keys.portfolio.1)?,
    })
    .map_err(|error| Error::new(format!("Core terminal admission builder: {error:?}")))?;
    if report.instruction.program_id != keys.core_program {
        return Err(Error::new(
            "terminal admission was built against a different Core program",
        ));
    }
    if report
        .instruction
        .accounts
        .iter()
        .any(|meta| meta.is_signer)
    {
        return Err(Error::new(
            "terminal admission must carry no signer meta; the fee payer is not an authority",
        ));
    }
    if report.terminal_sequence != input.terminal_sequence {
        return Err(Error::new(format!(
            "Source decision names terminal sequence {}, the input names {}",
            report.terminal_sequence, input.terminal_sequence
        )));
    }
    Ok(report.instruction)
}

/// Fund one terminal certificate seat to exactly its rent, and no further.
///
/// The rent comes from the Rent SYSVAR in this same finalized snapshot, not
/// from a second `getMinimumBalanceForRentExemption` round trip: the walk that
/// will consume this seat reads the same sysvar in the same way, so taking the
/// number from anywhere else is a second source of truth for one figure.
///
/// The transfer is exactly the shortfall. A seat that already holds its rent is
/// a refusal rather than a zero-lamport transfer, because "the prepay is done"
/// and "the prepay just ran" are different facts and only one of them should
/// produce a signature.
fn prepay_certificate_instruction(
    signer: Pubkey,
    coordinates: &CoordinatesV1,
    snapshot: &SnapshotV1,
) -> Result<Instruction> {
    let seat = coordinates
        .certificate
        .ok_or_else(|| Error::new("prepay-certificate seat missing"))?;
    if seat == signer {
        return Err(Error::new("the certificate seat cannot be its own funder"));
    }
    let rent: solana_sdk::rent::Rent =
        bincode::deserialize(&snapshot.required(sysvar::rent::ID, "Rent sysvar")?.data)
            .map_err(|error| Error::new(format!("Rent sysvar: {error}")))?;
    let required = rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2);
    let held = snapshot
        .optional(seat)
        .map_or(0, |account| account.lamports);
    let shortfall = required.saturating_sub(held);
    if shortfall == 0 {
        return Err(Error::new(format!(
            "certificate seat {seat} already holds {held} lamports against the {required} its {RESOLUTION_CERTIFICATE_BYTES_V2}-byte body needs; nothing to prepay"
        )));
    }
    Ok(transfer(&signer, &seat, shortfall))
}

/// A staging cursor, read as vacant only where the snapshot actually looked.
///
/// A key the snapshot never requested and a key it read back empty are two
/// different facts that would otherwise produce the same `ObservedAccount`, and
/// the builder treats a vacant staging cursor as a positive authentication of
/// a finalized record. So an unrequested key is a refusal, not a vacancy.
fn observed_or_vacant(snapshot: &SnapshotV1, key: Pubkey) -> Result<ObservedAccount> {
    let account = snapshot
        .accounts
        .get(&key)
        .ok_or_else(|| {
            Error::new(format!(
                "finalized snapshot never observed {key}; a vacant reading would be a fabrication"
            ))
        })?
        .clone()
        .unwrap_or(RpcAccount {
            lamports: 0,
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        });
    Ok(ObservedAccount {
        observation: snapshot.observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    })
}

fn push_pair(metas: &mut Vec<AccountMeta>, pair: (Pubkey, Pubkey)) {
    metas.push(AccountMeta::new_readonly(pair.0, false));
    metas.push(AccountMeta::new_readonly(pair.1, false));
}

fn source_record_metas(keys: &InputKeysV1) -> Vec<AccountMeta> {
    let mut metas = Vec::with_capacity(14);
    for pair in [
        keys.source_material,
        keys.source_spec,
        keys.provider_release,
        keys.adapter_config,
        keys.window,
        keys.statistic,
        keys.sponsored_release,
    ] {
        push_pair(&mut metas, pair);
    }
    metas
}

fn capture_metas(
    signer: Pubkey,
    keys: &InputKeysV1,
    coordinates: &CoordinatesV1,
) -> Result<Vec<AccountMeta>> {
    let candidate = coordinates
        .candidate
        .ok_or_else(|| Error::new("capture candidate missing"))?;
    let mut metas = vec![
        AccountMeta::new(signer, true),
        AccountMeta::new_readonly(keys.market, false),
        AccountMeta::new_readonly(keys.core_program, false),
        AccountMeta::new_readonly(keys.activation_cache, false),
        AccountMeta::new(coordinates.head, false),
        AccountMeta::new(candidate, false),
        AccountMeta::new_readonly(keys.source_state, false),
    ];
    metas.extend(source_record_metas(keys));
    metas.extend([
        AccountMeta::new_readonly(keys.price_account, false),
        AccountMeta::new_readonly(keys.receiver_program, false),
        AccountMeta::new_readonly(keys.receiver_programdata, false),
        AccountMeta::new_readonly(keys.push_oracle_program, false),
        AccountMeta::new_readonly(keys.push_oracle_programdata, false),
        AccountMeta::new_readonly(keys.receiver_config, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]);
    Ok(metas)
}

fn settle_metas(
    signer: Pubkey,
    keys: &InputKeysV1,
    coordinates: &CoordinatesV1,
    snapshot: &SnapshotV1,
) -> Result<Vec<AccountMeta>> {
    let candidate = coordinates
        .candidate
        .ok_or_else(|| Error::new("settle candidate missing"))?;
    let candidate_account = snapshot.required(candidate, "selected sponsored candidate")?;
    SponsoredPushCandidateV1::decode(&candidate_account.data)
        .map_err(|error| Error::new(format!("selected candidate: {error:?}")))?;
    let mut metas = vec![
        AccountMeta::new(signer, true),
        AccountMeta::new_readonly(keys.market, false),
        AccountMeta::new_readonly(keys.core_program, false),
        AccountMeta::new_readonly(keys.activation_cache, false),
        AccountMeta::new_readonly(coordinates.head, false),
        AccountMeta::new_readonly(candidate, false),
        AccountMeta::new(keys.source_state, false),
        AccountMeta::new(
            coordinates
                .certificate
                .ok_or_else(|| Error::new("certificate missing"))?,
            false,
        ),
        AccountMeta::new(
            coordinates
                .receipt
                .ok_or_else(|| Error::new("receipt missing"))?,
            false,
        ),
    ];
    metas.extend(source_record_metas(keys));
    for pair in [keys.product, keys.result_domain, keys.portfolio] {
        push_pair(&mut metas, pair);
    }
    metas.extend([
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]);
    Ok(metas)
}

fn failure_metas(
    signer: Pubkey,
    keys: &InputKeysV1,
    coordinates: &CoordinatesV1,
) -> Result<Vec<AccountMeta>> {
    let certificate = coordinates
        .certificate
        .ok_or_else(|| Error::new("failure certificate missing"))?;
    let mut metas = vec![
        AccountMeta::new(signer, true),
        AccountMeta::new_readonly(keys.market, false),
        AccountMeta::new_readonly(keys.core_program, false),
        AccountMeta::new_readonly(keys.activation_cache, false),
        AccountMeta::new(keys.source_state, false),
        AccountMeta::new(certificate, false),
    ];
    push_pair(&mut metas, keys.source_material);
    push_pair(&mut metas, keys.window);
    push_pair(&mut metas, keys.product);
    push_pair(&mut metas, keys.result_domain);
    push_pair(&mut metas, keys.portfolio);
    push_pair(&mut metas, keys.capability_manifest);
    metas.extend([
        AccountMeta::new(keys.failure_funding, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(coordinates.head, false),
    ]);
    push_pair(&mut metas, keys.source_spec);
    push_pair(&mut metas, keys.provider_release);
    push_pair(&mut metas, keys.sponsored_release);
    Ok(metas)
}

fn close_candidate_metas(
    keys: &InputKeysV1,
    coordinates: &CoordinatesV1,
    snapshot: &SnapshotV1,
) -> Result<Vec<AccountMeta>> {
    let candidate = coordinates
        .candidate
        .ok_or_else(|| Error::new("close candidate missing"))?;
    let decoded =
        SponsoredPushCandidateV1::decode(&snapshot.required(candidate, "candidate")?.data)
            .map_err(|error| Error::new(format!("candidate: {error:?}")))?;
    Ok(vec![
        AccountMeta::new_readonly(keys.market, false),
        AccountMeta::new_readonly(keys.source_state, false),
        AccountMeta::new(candidate, false),
        AccountMeta::new(Pubkey::new_from_array(decoded.refund_recipient), false),
    ])
}

fn close_head_metas(
    keys: &InputKeysV1,
    coordinates: &CoordinatesV1,
    snapshot: &SnapshotV1,
) -> Result<Vec<AccountMeta>> {
    let decoded = SponsoredPushHeadV1::decode(&snapshot.required(coordinates.head, "head")?.data)
        .map_err(|error| Error::new(format!("head: {error:?}")))?;
    Ok(vec![
        AccountMeta::new_readonly(keys.market, false),
        AccountMeta::new_readonly(keys.source_state, false),
        AccountMeta::new(coordinates.head, false),
        AccountMeta::new(Pubkey::new_from_array(decoded.head_refund_recipient), false),
    ])
}

fn read_snapshot(rpc: &mut Rpc, addresses: &[Pubkey], minimum_slot: u64) -> Result<SnapshotV1> {
    let (slot, accounts) = rpc.finalized_accounts(addresses, minimum_slot)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    Ok(SnapshotV1 {
        observation,
        accounts: addresses.iter().copied().zip(accounts).collect(),
    })
}

fn evidence_for(
    snapshot: &SnapshotV1,
    keys: &[Pubkey],
) -> BTreeMap<String, Option<AccountEvidence>> {
    keys.iter()
        .map(|key| {
            (
                key.to_string(),
                snapshot
                    .optional(*key)
                    .map(|account| account_evidence(*key, account)),
            )
        })
        .collect()
}

fn writable_keys(instruction: &Instruction) -> Vec<Pubkey> {
    instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect()
}

fn dedupe(values: Vec<Pubkey>) -> Vec<Pubkey> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(*value))
        .collect()
}

fn require_distinct_metas(instruction: &Instruction) -> Result<()> {
    let mut seen = BTreeSet::new();
    if instruction
        .accounts
        .iter()
        .any(|meta| !seen.insert(meta.pubkey))
    {
        return Err(Error::new("sponsored instruction account aliases refused"));
    }
    Ok(())
}

fn hex32(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(Error::new("identity must be 32-byte lowercase hex"));
    }
    let mut out = [0_u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        let start = index * 2;
        *slot = u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| Error::new("identity must be 32-byte lowercase hex"))?;
    }
    if hex(&out) != value {
        return Err(Error::new("identity must be lowercase canonical hex"));
    }
    Ok(out)
}

fn set_once<T>(slot: &mut Option<T>, value: T, label: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(format!("{label} may be supplied only once")));
    }
    Ok(())
}

struct SponsoredReportLeaseV1 {
    path: PathBuf,
    parent: PathBuf,
    file: fs::File,
}

impl SponsoredReportLeaseV1 {
    fn acquire(report: &Path) -> Result<Self> {
        let parent = report
            .parent()
            .ok_or_else(|| Error::new("sponsored report requires a parent directory"))?;
        fs::create_dir_all(parent)?;
        let name = report
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| Error::new("sponsored report requires a UTF-8 file name"))?;
        let path = report.with_file_name(format!(".{name}.sponsored-push.lock"));
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::new("system clock precedes the Unix epoch"))?
            .as_secs();
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(Error::new(format!(
                    "sponsored report is locked at {}; locks are never removed automatically. Confirm no live writer owns it before removing a stale lock",
                    path.display()
                )));
            }
            Err(error) => {
                return Err(Error::new(format!(
                    "create sponsored report lock {}: {error}",
                    path.display()
                )));
            }
        };
        let owner = serde_json::to_vec(&serde_json::json!({
            "format": "dclutch-sponsored-push-report-lock-v1",
            "pid": std::process::id(),
            "report": report.display().to_string(),
            "createdAtUnixSeconds": created_at,
            "stalePolicy": "never-auto-remove",
        }))?;
        if let Err(error) = file
            .write_all(&owner)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::File::open(parent)?.sync_all())
        {
            if Self::owns_link(&file, &path) {
                let _ = fs::remove_file(&path);
            }
            return Err(Error::new(format!(
                "initialize sponsored report lock {}: {error}",
                path.display()
            )));
        }
        if !Self::owns_link(&file, &path) {
            return Err(Error::new(
                "sponsored report lock changed while it was acquired",
            ));
        }
        Ok(Self {
            path,
            parent: parent.to_path_buf(),
            file,
        })
    }

    fn owns_link(file: &fs::File, path: &Path) -> bool {
        let Ok(held) = file.metadata() else {
            return false;
        };
        let Ok(linked) = fs::symlink_metadata(path) else {
            return false;
        };
        held.dev() == linked.dev() && held.ino() == linked.ino()
    }
}

impl Drop for SponsoredReportLeaseV1 {
    fn drop(&mut self) {
        if Self::owns_link(&self.file, &self.path) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::File::open(&self.parent).and_then(|directory| directory.sync_all());
        }
    }
}

fn write_report_new(path: &Path, report: &ExteriorReportV1) -> Result<()> {
    if path.exists() {
        return Err(Error::new(format!(
            "report already exists: {}",
            path.display()
        )));
    }
    write_report_replace(path, report)
}

fn write_report_replace(path: &Path, report: &ExteriorReportV1) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("report path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".sponsored-push-{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(report)?;
    fs::write(&temporary, bytes)?;
    fs::File::open(&temporary)?.sync_all()?;
    fs::rename(&temporary, path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The producer: `dclutch-sponsored-push-exterior-input-v1`, authored from chain
// ---------------------------------------------------------------------------

/// The public arm's producer command name.
pub(crate) const INPUT_COMMAND_DEVNET_V1: &str = "devnet-sponsored-push-input-v1";
/// The owned-loopback producer command name.
pub(crate) const INPUT_COMMAND_LOOPBACK_V1: &str =
    "local-private-validator-sponsored-push-input-v1";

const fn input_command(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => INPUT_COMMAND_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => INPUT_COMMAND_LOOPBACK_V1,
    }
}

pub(crate) const fn input_usage() -> &'static str {
    "  dclutch-local-successor-bootstrap devnet-sponsored-push-input-v1 --rpc-url URL \
     --i-mean-devnet DEVNET_GENESIS --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON \
     --market PUBKEY --lookup-table PUBKEY --output ABSOLUTE_NEW_JSON \
     [--terminal-sequence U64]"
}

pub(crate) const fn input_owned_loopback_usage() -> &'static str {
    "  dclutch-local-successor-bootstrap local-private-validator-sponsored-push-input-v1 \
     --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON \
     --market PUBKEY --lookup-table PUBKEY --output ABSOLUTE_NEW_JSON \
     [--terminal-sequence U64]"
}

pub(crate) fn run_devnet_input(arguments: Vec<String>) -> Result<()> {
    run_input(arguments, ExpectedClusterV1::Devnet)
}

pub(crate) fn run_owned_loopback_input(arguments: Vec<String>) -> Result<()> {
    run_input(arguments, ExpectedClusterV1::OwnedLoopback)
}

struct InputArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    lookup_table: Pubkey,
    terminal_sequence: u64,
    output: PathBuf,
}

/// Author one `dclutch-sponsored-push-exterior-input-v1` from finalized state.
///
/// # Why this exists
///
/// The consumer above was written, shipped, exercised and used to resolve a
/// devnet market, and until this function nothing in the tree WROTE its input.
/// A sweep for the format string found the consumer, a keeper that checks one
/// field of it, a README example and a doc -- the producer-missing shape, where
/// a reader, a schema and a refusal all exist with nothing that emits the thing
/// they judge. Cohort-13's document was assembled by hand from chain, one
/// address at a time, by solving each record's schema against its own digest
/// until the known address reproduced.
///
/// This is that walk, as a command.
///
/// # What it takes and what it refuses to be told
///
/// Two files and two addresses. The plan names the deployment; the sealed
/// campaign report names each record's persisted body; `--market` names the
/// Market and `--lookup-table` the frozen routing table. Everything else is
/// DERIVED and then cross-checked against a persisted fact:
///
/// - every record pair is `record_pair(registry, schema, digest)` over the
///   report's own `data_sha256`, and `routed_record` refuses when the derived
///   raw address is not the persisted one -- so each of the eleven pairs is a
///   reproduction rather than a transcription, which is exactly the property
///   the hand-authored document had;
/// - the activation cache is derived from `ACTIVATION_PDA_DOMAIN_V1` and the
///   Market's OWN selected release set, then checked against the plan's;
/// - the Source state is derived from the Market and its generation, then
///   checked against the report's row;
/// - generation and release set come off the Market, never off the plan alone;
/// - the four Pyth addresses come out of the sponsored release RECORD read from
///   chain, and the two ProgramData addresses from the Loader's own derivation.
///
/// # The last check is the strongest one
///
/// The document is handed back through `InputKeysV1::parse` before it is
/// written. The producer and the consumer share one `SponsoredInputV1`, so a
/// field this emits that the consumer would refuse cannot reach a file.
fn run_input(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<()> {
    let args = parse_input_arguments(arguments, expected)?;
    expected.authenticate(&args.origin)?;
    let mut rpc = Rpc::connect_cluster(&args.origin, WritePolicyV1::ReadsOnly)?;

    let plan_bytes = fs::read(&args.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)
        .map_err(|error| Error::new(format!("successor plan: {error}")))?;
    let evidence_bytes = fs::read(&args.evidence)?;
    let evidence =
        parse_campaign_terminal_evidence_with_expected_cluster_v1(&evidence_bytes, expected)?;
    authenticate_plan_source(&plan_bytes, &evidence.plan_sha256)?;

    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;

    // The Market is the authority for its own generation and release set. The
    // plan's release set is a cross-check, not the source.
    let market_account = rpc.required_account(args.market, "Core Market")?;
    if market_account.owner != core || market_account.executable {
        return Err(Error::new(format!(
            "Market {} is owned by {}, not the plan's Core {core}",
            args.market, market_account.owner
        )));
    }
    let state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    let release_set = state.identity.selected_release_set.to_bytes();
    if release_set != hex32(&plan.release_set_id)? {
        return Err(Error::new(
            "the Market selected a release set the plan does not name",
        ));
    }
    let generation = state.identity.generation;
    if generation == 0 {
        return Err(Error::new("a founded Market never carries generation zero"));
    }

    let activation_cache =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &registry).0;
    if activation_cache != pubkey(&plan.activation)? {
        return Err(Error::new(format!(
            "the release set's activation cache derives to {activation_cache}, and the plan names {}",
            plan.activation
        )));
    }

    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            args.market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let persisted_source =
        pubkey(&required_account(&evidence, "resolution_source_state")?.address)?;
    if source_state != persisted_source {
        return Err(Error::new(format!(
            "the Source resolution state derives to {source_state}, and the campaign recorded {persisted_source}"
        )));
    }

    let pair = |label: &str, schema: [u8; 32]| -> Result<RecordPairInputV1> {
        let routed = routed_record(&evidence, label, registry, schema)?;
        Ok(RecordPairInputV1 {
            raw: routed.raw.to_string(),
            staging: routed.staging.to_string(),
        })
    };

    // The sponsored release RECORD carries the four Pyth addresses. Reading
    // them here, from the record this Market selected, is what keeps the runbook
    // out of the document.
    let sponsored = pair(
        "pyth_sponsored_push_release_record",
        PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1,
    )?;
    let sponsored_account =
        rpc.required_account(pubkey(&sponsored.raw)?, "sponsored release record")?;
    if sponsored_account.owner != registry {
        return Err(Error::new(
            "the sponsored release record is not Registry-owned",
        ));
    }
    let release = PythSponsoredPushReleaseV1::decode(&sponsored_account.data)
        .map_err(|error| Error::new(format!("sponsored release: {error:?}")))?;
    let compiled = devnet_sponsored_sol_usd_release_v1()
        .map_err(|error| Error::new(format!("compiled sponsored release: {error:?}")))?;
    if release.to_bytes() != compiled.to_bytes() {
        return Err(Error::new(
            "the Market's sponsored release is not the compiled devnet SOL/USD release",
        ));
    }
    let receiver_program = Pubkey::new_from_array(release.receiver_program());
    let push_oracle_program = Pubkey::new_from_array(release.push_oracle_program());

    let accounts = SponsoredAccountsInputV1 {
        registry_program: registry.to_string(),
        activation_cache: activation_cache.to_string(),
        core_program: core.to_string(),
        core_programdata: pubkey(&plan.core.programdata_id)?.to_string(),
        resolution_program: resolution.to_string(),
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?.to_string(),
        market: args.market.to_string(),
        source_state: source_state.to_string(),
        source_material: pair(
            "source_material_record",
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        )?,
        source_spec: pair("source_spec_record", SOURCE_SPEC_SCHEMA_ID_V1)?,
        provider_release: pair("provider_release_record", PROVIDER_RELEASE_SCHEMA_ID_V1)?,
        adapter_config: pair(
            "pyth_adapter_config_record",
            PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
        )?,
        window: pair("window_spec_record", WINDOW_SPEC_SCHEMA_ID_V1)?,
        statistic: pair("statistic_spec_record", STATISTIC_SPEC_SCHEMA_ID_V1)?,
        sponsored_release: sponsored,
        product: pair("product_record", PRODUCT_RECORD_SCHEMA_ID_V2)?,
        result_domain: pair("result_domain_record", RESULT_DOMAIN_SCHEMA_ID_V2)?,
        portfolio: pair("portfolio_record", PORTFOLIO_SCHEMA_ID_V2)?,
        capability_manifest: pair(
            "capability_manifest_record",
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        )?,
        failure_funding: required_account(&evidence, "resolution_funding_ledger")?
            .address
            .clone(),
        price_account: Pubkey::new_from_array(release.price_account()).to_string(),
        receiver_program: receiver_program.to_string(),
        receiver_programdata: loader_programdata(receiver_program).to_string(),
        push_oracle_program: push_oracle_program.to_string(),
        push_oracle_programdata: loader_programdata(push_oracle_program).to_string(),
        receiver_config: Pubkey::new_from_array(release.receiver_config()).to_string(),
        lookup_table: args.lookup_table.to_string(),
    };
    let document = SponsoredInputV1 {
        format: INPUT_FORMAT.to_owned(),
        generation,
        terminal_sequence: args.terminal_sequence,
        release_set: hex(&release_set),
        accounts,
    };

    // The routing table is routing, never authority, and it must already be
    // what the consumer will demand of it.
    let table_slot = rpc.finalized_slot()?;
    let table = rpc.required_account(args.lookup_table, "sponsored routing table")?;
    authenticate_frozen_routing_table(&ObservedAccount {
        observation: Observation {
            slot: table_slot,
            unix_timestamp: 0,
            finality: Finality::Finalized,
        },
        key: args.lookup_table,
        owner: table.owner,
        lamports: table.lamports,
        executable: table.executable,
        data: table.data.clone(),
    })?;

    // The producer's own output, read back through the consumer's parser. One
    // struct, one format constant, one set of address rules: a document this
    // emits that the consumer would refuse cannot reach a file.
    let serialized = format!("{}\n", serde_json::to_string_pretty(&document)?);
    let reparsed: SponsoredInputV1 = serde_json::from_str(&serialized)
        .map_err(|error| Error::new(format!("the emitted document did not re-parse: {error}")))?;
    InputKeysV1::parse(&reparsed)?;

    write_new_document(&args.output, serialized.as_bytes())?;
    println!("{serialized}");
    Ok(())
}

/// The Loader's own ProgramData derivation, never a written-down address.
fn loader_programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Write a document only where none exists, and never truncate one that does.
fn write_new_document(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| match error.kind() {
            ErrorKind::AlreadyExists => Error::new(format!(
                "{} already exists; a producer never overwrites a document another run may be using",
                path.display()
            )),
            _ => Error::new(format!("{}: {error}", path.display())),
        })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn parse_input_arguments(
    arguments: Vec<String>,
    expected: ExpectedClusterV1,
) -> Result<InputArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut market = None;
    let mut lookup_table = None;
    let mut terminal_sequence = None;
    let mut output = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            flag if flag == DEVNET_ACKNOWLEDGMENT_FLAG && expected == ExpectedClusterV1::Devnet => {
                &mut acknowledgment
            }
            "--plan" => &mut plan,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--lookup-table" => &mut lookup_table,
            "--terminal-sequence" => &mut terminal_sequence,
            "--output" => &mut output,
            other => {
                return Err(Error::new(format!(
                    "unknown {} argument: {other}",
                    input_command(expected)
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
    Ok(InputArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: PathBuf::from(absolute(Some(required(plan, "--plan")?), "--plan")?),
        evidence: PathBuf::from(absolute(
            Some(required(evidence, "--evidence")?),
            "--evidence",
        )?),
        market: pubkey(&required(market, "--market")?)?,
        lookup_table: pubkey(&required(lookup_table, "--lookup-table")?)?,
        terminal_sequence: terminal_sequence
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|error| Error::new(format!("--terminal-sequence: {error}")))
            })
            .transpose()?
            .unwrap_or_default(),
        output: PathBuf::from(absolute(Some(required(output, "--output")?), "--output")?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_address_lookup_table_interface::state::LookupTableMeta;
    use std::borrow::Cow;

    fn key(tag: u8) -> Pubkey {
        Pubkey::new_from_array([tag; 32])
    }

    fn frozen_table(tag: u8, addresses: Vec<Pubkey>) -> ObservedAccount {
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: None,
                deactivation_slot: u64::MAX,
                last_extended_slot: 9,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: Observation {
                slot: 10,
                unix_timestamp: 20,
                finality: Finality::Finalized,
            },
            key: key(tag),
            owner: lookup_table_program::ID,
            lamports: 1,
            executable: false,
            data: table.serialize_for_tests().expect("table bytes"),
        }
    }

    fn signed_packet(
        payer: &Keypair,
        instruction: &Instruction,
        table: &ObservedAccount,
    ) -> SignedVersionedPacketV1 {
        let bounded = crate::rpc::bounded_instructions(std::slice::from_ref(instruction), None)
            .expect("bounded packet");
        let routed = dclutch_versioned_message_operator::compile_v0_message(
            payer.pubkey(),
            &bounded,
            solana_hash::Hash::new_unique(),
            table.observation,
            std::slice::from_ref(table),
        )
        .expect("v0 message");
        let transaction =
            solana_sdk::transaction::VersionedTransaction::try_new(routed.message, &[payer])
                .expect("signed packet");
        let bytes = bincode::serialize(&transaction).expect("packet bytes");
        SignedVersionedPacketV1 {
            signature: transaction.signatures[0].to_string(),
            packet_base64: BASE64.encode(&bytes),
            packet_sha256: hex(&Sha256::digest(&bytes)),
            last_valid_block_height: 99,
        }
    }

    fn vacant_evidence(instruction: &Instruction) -> BTreeMap<String, Option<AccountEvidence>> {
        writable_keys(instruction)
            .into_iter()
            .map(|key| (key.to_string(), None))
            .collect()
    }

    fn keys() -> InputKeysV1 {
        let mut next = 1_u8;
        fn take(next: &mut u8) -> Pubkey {
            let value = key(*next);
            *next = next.saturating_add(1);
            value
        }
        fn pair(next: &mut u8) -> (Pubkey, Pubkey) {
            (take(next), take(next))
        }
        InputKeysV1 {
            registry_program: take(&mut next),
            activation_cache: take(&mut next),
            core_program: take(&mut next),
            core_programdata: Some(take(&mut next)),
            resolution_program: take(&mut next),
            resolution_programdata: take(&mut next),
            market: take(&mut next),
            source_state: take(&mut next),
            source_material: pair(&mut next),
            source_spec: pair(&mut next),
            provider_release: pair(&mut next),
            adapter_config: pair(&mut next),
            window: pair(&mut next),
            statistic: pair(&mut next),
            sponsored_release: pair(&mut next),
            product: pair(&mut next),
            result_domain: pair(&mut next),
            portfolio: pair(&mut next),
            capability_manifest: pair(&mut next),
            failure_funding: take(&mut next),
            price_account: take(&mut next),
            receiver_program: take(&mut next),
            receiver_programdata: take(&mut next),
            push_oracle_program: take(&mut next),
            push_oracle_programdata: take(&mut next),
            receiver_config: take(&mut next),
            lookup_table: take(&mut next),
        }
    }

    #[test]
    fn action_partition_is_closed() {
        for (text, expected) in [
            ("capture", ExteriorActionV1::Capture),
            ("settle", ExteriorActionV1::Settle),
            ("commit-failure", ExteriorActionV1::CommitFailure),
            ("admit-terminal", ExteriorActionV1::AdmitTerminal),
            ("close-candidate", ExteriorActionV1::CloseCandidate),
            ("close-head", ExteriorActionV1::CloseHead),
        ] {
            assert_eq!(
                ExteriorActionV1::parse(text).expect("known action"),
                expected
            );
        }
        assert!(ExteriorActionV1::parse("pull").is_err());
    }

    /// The Core admission is the one action with no Resolution wire action,
    /// the one that must not put the payer inside its frame, and one of the
    /// three that carry a terminal sequence.
    #[test]
    fn terminal_admission_is_a_core_action_not_a_sponsored_push_one() {
        assert!(ExteriorActionV1::AdmitTerminal.wire().is_none());
        assert!(ExteriorActionV1::AdmitTerminal.terminal());
        assert!(ExteriorActionV1::AdmitTerminal.payer_is_outside_the_frame());
        for other in [
            ExteriorActionV1::Capture,
            ExteriorActionV1::Settle,
            ExteriorActionV1::CommitFailure,
            ExteriorActionV1::CloseCandidate,
            ExteriorActionV1::CloseHead,
        ] {
            assert!(
                other.wire().is_some(),
                "{other:?} lost its sponsored-push wire discriminant"
            );
        }
    }

    /// `--prepay-for` exists for exactly one action and admits exactly the two
    /// walks that initialize a seat.
    #[test]
    fn the_prepay_target_is_required_where_it_means_something_and_nowhere_else() {
        let base = |extra: &[&str]| {
            let mut out = vec![
                "--rpc-url".to_owned(),
                "http://127.0.0.1:21400".to_owned(),
                "--input".to_owned(),
                "/abs/in.json".to_owned(),
                "--output".to_owned(),
                "/abs/out.json".to_owned(),
                "--signer".to_owned(),
                key(9).to_string(),
            ];
            out.extend(extra.iter().map(|value| (*value).to_owned()));
            out
        };
        assert!(
            ArgumentsV1::parse(base(&["--action", "prepay-certificate"])).is_err(),
            "prepay-certificate without a named walk has no seat to address"
        );
        assert!(
            ArgumentsV1::parse(base(&["--action", "capture", "--prepay-for", "settle"])).is_err(),
            "a walk named for an action that funds nothing is a caller mistake"
        );
        let Err(refusal) = ArgumentsV1::parse(base(&[
            "--action",
            "prepay-certificate",
            "--prepay-for",
            "close-head",
        ])) else {
            panic!("close-head initializes no certificate seat");
        };
        assert!(
            refusal
                .0
                .contains("--prepay-for must name settle or commit-failure"),
            "expected the named-walk refusal, got: {refusal}"
        );
        for walk in ["settle", "commit-failure"] {
            assert!(
                ArgumentsV1::parse(base(&[
                    "--action",
                    "prepay-certificate",
                    "--prepay-for",
                    walk
                ]))
                .is_ok(),
                "{walk} initializes a certificate seat and must be nameable"
            );
        }
    }

    /// The transfer is the shortfall, the rent comes from the sysvar, and a
    /// seat that is already funded is a refusal rather than a zero transfer.
    #[test]
    fn a_seat_prepay_transfers_the_shortfall_and_refuses_a_funded_seat() {
        let seat = key(60);
        let signer = key(61);
        let rent = solana_sdk::rent::Rent::default();
        let required = rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2);
        let snapshot = |held: Option<u64>| SnapshotV1 {
            observation: Observation {
                slot: 5,
                unix_timestamp: 9,
                finality: Finality::Finalized,
            },
            accounts: BTreeMap::from([
                (
                    sysvar::rent::ID,
                    Some(RpcAccount {
                        lamports: 1,
                        owner: sysvar::ID,
                        executable: false,
                        rent_epoch: 0,
                        data: bincode::serialize(&rent).expect("rent bytes"),
                    }),
                ),
                (
                    seat,
                    held.map(|lamports| RpcAccount {
                        lamports,
                        owner: system_program::ID,
                        executable: false,
                        rent_epoch: 0,
                        data: Vec::new(),
                    }),
                ),
            ]),
        };
        let coordinates = CoordinatesV1 {
            release_id: [7; 32],
            head: key(62),
            candidate: None,
            certificate: Some(seat),
            receipt: None,
        };

        let vacant = prepay_certificate_instruction(signer, &coordinates, &snapshot(None))
            .expect("a vacant seat is prepayable");
        assert_eq!(vacant.program_id, system_program::ID);
        assert_eq!(
            u64::from_le_bytes(
                vacant.data[4..12]
                    .try_into()
                    .expect("a System transfer carries eight lamport bytes")
            ),
            required,
            "a vacant seat needs its whole rent"
        );

        let partial = prepay_certificate_instruction(signer, &coordinates, &snapshot(Some(11)))
            .expect("a partly funded seat is topped up");
        assert_eq!(
            u64::from_le_bytes(partial.data[4..12].try_into().expect("lamport bytes")),
            required - 11,
            "the transfer is the shortfall, never the whole rent again"
        );

        let Err(refusal) =
            prepay_certificate_instruction(signer, &coordinates, &snapshot(Some(required)))
        else {
            panic!("a seat already at its rent has nothing to prepay");
        };
        assert!(
            refusal.0.contains("nothing to prepay"),
            "expected the already-funded refusal, got: {refusal}"
        );
    }

    /// The producer and the consumer share one struct, so a document the
    /// producer emits is a document the consumer parses -- including the
    /// `coreProgramdata` an older input omits.
    #[test]
    fn the_emitted_document_is_the_document_the_consumer_reads() {
        let pair = |a: u8, b: u8| RecordPairInputV1 {
            raw: key(a).to_string(),
            staging: key(b).to_string(),
        };
        let emitted = SponsoredInputV1 {
            format: INPUT_FORMAT.to_owned(),
            generation: 2,
            terminal_sequence: 1,
            release_set: hex(&[0x5a; 32]),
            accounts: SponsoredAccountsInputV1 {
                registry_program: key(1).to_string(),
                activation_cache: key(2).to_string(),
                core_program: key(3).to_string(),
                core_programdata: key(4).to_string(),
                resolution_program: key(5).to_string(),
                resolution_programdata: key(6).to_string(),
                market: key(7).to_string(),
                source_state: key(8).to_string(),
                source_material: pair(10, 11),
                source_spec: pair(12, 13),
                provider_release: pair(14, 15),
                adapter_config: pair(16, 17),
                window: pair(18, 19),
                statistic: pair(20, 21),
                sponsored_release: pair(22, 23),
                product: pair(24, 25),
                result_domain: pair(26, 27),
                portfolio: pair(28, 29),
                capability_manifest: pair(30, 31),
                failure_funding: key(32).to_string(),
                price_account: key(33).to_string(),
                receiver_program: key(34).to_string(),
                receiver_programdata: key(35).to_string(),
                push_oracle_program: key(36).to_string(),
                push_oracle_programdata: key(37).to_string(),
                receiver_config: key(38).to_string(),
                lookup_table: key(39).to_string(),
            },
        };
        let text = serde_json::to_string(&emitted).expect("the producer's own serialization");
        let read: SponsoredInputV1 =
            serde_json::from_str(&text).expect("the consumer's own deserialization");
        let keys = InputKeysV1::parse(&read).expect("every address parses");
        assert_eq!(keys.core_programdata, Some(key(4)));
        assert_eq!(read.format, INPUT_FORMAT);
        assert_eq!(read.generation, 2);
        // The camelCase spelling is part of the contract with every input
        // already on disk, so it is asserted rather than assumed.
        assert!(
            text.contains("\"coreProgramdata\"") && text.contains("\"terminalSequence\""),
            "the emitted document must use the input's own field spellings"
        );
    }

    /// An input authored before this arm existed still deserializes, and the
    /// refusal names the field rather than reporting a zero address.
    #[test]
    fn terminal_admission_refuses_an_input_without_core_programdata() {
        let mut keys = keys();
        keys.core_programdata = None;
        let refusal = keys
            .core_programdata()
            .expect_err("an omitted ProgramData must refuse by name");
        assert!(
            refusal.0.contains("accounts.coreProgramdata"),
            "expected the omitted-field refusal, got: {refusal}"
        );
    }

    /// A snapshot that never requested a staging cursor must refuse rather
    /// than hand the builder a fabricated vacancy, because the builder reads a
    /// vacant cursor as a positive authentication of its finalized record.
    #[test]
    fn an_unobserved_staging_cursor_is_a_refusal_and_not_a_vacancy() {
        let cursor = key(200);
        let observed = SnapshotV1 {
            observation: Observation {
                slot: 7,
                unix_timestamp: 11,
                finality: Finality::Finalized,
            },
            accounts: BTreeMap::from([(cursor, None)]),
        };
        assert_eq!(
            observed_or_vacant(&observed, cursor)
                .expect("an observed vacancy is a vacancy")
                .owner,
            system_program::ID
        );
        let unobserved = SnapshotV1 {
            observation: observed.observation,
            accounts: BTreeMap::new(),
        };
        let refusal = observed_or_vacant(&unobserved, cursor)
            .expect_err("an unrequested key must not read as vacant");
        assert!(
            refusal
                .0
                .contains("a vacant reading would be a fabrication"),
            "expected the fabrication refusal, got: {refusal}"
        );
    }

    #[test]
    fn execute_without_keypair_is_reserved_for_durable_resume() {
        let parsed = ArgumentsV1::parse(vec!["--execute".into()])
            .expect("resume may poll a persisted packet without reopening a key");
        assert!(parsed.execute);
        assert!(parsed.signer_keypair.is_none());
        assert!(
            ArgumentsV1::parse(vec!["--signer-keypair".into(), "/tmp/key.json".into()]).is_err(),
            "read-only preflight unexpectedly admitted a private-key path"
        );
    }

    #[test]
    fn report_lease_excludes_a_second_writer_for_the_whole_run() {
        let directory = std::env::temp_dir().join(format!(
            "dclutch-sponsored-report-lock-{}-{}",
            std::process::id(),
            Pubkey::new_unique()
        ));
        fs::create_dir(&directory).expect("temporary directory");
        let report = directory.join("report.json");
        let first = SponsoredReportLeaseV1::acquire(&report).expect("first writer");
        assert!(
            SponsoredReportLeaseV1::acquire(&report).is_err(),
            "concurrent writer unexpectedly acquired the same report"
        );
        drop(first);
        let resumed = SponsoredReportLeaseV1::acquire(&report).expect("writer after release");
        drop(resumed);
        fs::remove_dir(&directory).expect("remove empty temporary directory");
    }

    #[test]
    fn every_durable_report_phase_authenticates_the_offline_packet() {
        let payer = Keypair::new();
        let head = key(0x81);
        let candidate = key(0x82);
        let instruction = Instruction {
            program_id: key(0x83),
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(key(0x84), false),
                AccountMeta::new_readonly(key(0x85), false),
                AccountMeta::new_readonly(key(0x86), false),
                AccountMeta::new(head, false),
                AccountMeta::new(candidate, false),
            ],
            data: vec![4, 5, 6],
        };
        let table = frozen_table(
            0x87,
            instruction
                .accounts
                .iter()
                .skip(1)
                .map(|meta| meta.pubkey)
                .collect(),
        );
        let packet = signed_packet(&payer, &instruction, &table);
        let mut report = ExteriorReportV1 {
            format: REPORT_FORMAT.into(),
            cluster: ExpectedClusterV1::OwnedLoopback.evidence_label().into(),
            input_sha256: "33".repeat(32),
            action: ExteriorActionV1::Capture,
            phase: ExteriorPhaseV1::Prepared,
            signer: payer.pubkey().to_string(),
            observation_slot: table.observation.slot,
            observation_unix_seconds: table.observation.unix_timestamp,
            sponsored_release: "44".repeat(32),
            head: head.to_string(),
            candidate: Some(candidate.to_string()),
            certificate: None,
            receipt: None,
            instruction: PlannedInstructionV1::new(&instruction),
            lookup_table: RoutingTableEvidenceV1::new(&table),
            prestate: vacant_evidence(&instruction),
            signed_packet: Some(packet.clone()),
            transaction: None,
            poststate: BTreeMap::new(),
        };
        let authenticate = |report: &ExteriorReportV1| {
            authenticate_report(
                report,
                ExpectedClusterV1::OwnedLoopback,
                &"33".repeat(32),
                ExteriorActionV1::Capture,
                payer.pubkey(),
                None,
                table.key,
            )
        };
        assert!(authenticate(&report).is_ok());
        report.signed_packet.as_mut().expect("packet").packet_sha256 = "00".repeat(32);
        assert!(authenticate(&report).is_err());
        report.signed_packet = Some(packet.clone());
        report.phase = ExteriorPhaseV1::Finalized;
        report.transaction = Some(TransactionEvidence {
            label: "sponsored-push-Capture".into(),
            signature: packet.signature.clone(),
            slot: table.observation.slot + 1,
            transaction_metadata_available: true,
            fee_lamports: Some(5_000),
            fee_only_balance_change: Some(false),
            compute_units_consumed: Some(100_000),
            error: None,
            logs: Vec::new(),
        });
        report.poststate = vacant_evidence(&instruction);
        assert!(authenticate(&report).is_ok());
        report.transaction.as_mut().expect("transaction").label = "substituted".into();
        assert!(authenticate(&report).is_err());
    }

    #[test]
    fn report_phase_and_coordinates_are_exact() {
        let signer = key(0x60);
        let head = key(0x64);
        let candidate = key(0x65);
        let instruction = Instruction {
            program_id: key(0x70),
            accounts: vec![
                AccountMeta::new(signer, true),
                AccountMeta::new_readonly(key(0x61), false),
                AccountMeta::new_readonly(key(0x62), false),
                AccountMeta::new_readonly(key(0x63), false),
                AccountMeta::new(head, false),
                AccountMeta::new(candidate, false),
            ],
            data: vec![1, 2, 3],
        };
        let mut report = ExteriorReportV1 {
            format: REPORT_FORMAT.into(),
            cluster: ExpectedClusterV1::OwnedLoopback.evidence_label().into(),
            input_sha256: "11".repeat(32),
            action: ExteriorActionV1::Capture,
            phase: ExteriorPhaseV1::Planned,
            signer: signer.to_string(),
            observation_slot: 10,
            observation_unix_seconds: 20,
            sponsored_release: "22".repeat(32),
            head: head.to_string(),
            candidate: Some(candidate.to_string()),
            certificate: None,
            receipt: None,
            instruction: PlannedInstructionV1::new(&instruction),
            lookup_table: RoutingTableEvidenceV1::new(&frozen_table(
                0x71,
                instruction
                    .accounts
                    .iter()
                    .skip(1)
                    .map(|meta| meta.pubkey)
                    .collect(),
            )),
            prestate: vacant_evidence(&instruction),
            signed_packet: None,
            transaction: None,
            poststate: BTreeMap::new(),
        };
        assert!(
            authenticate_report(
                &report,
                ExpectedClusterV1::OwnedLoopback,
                &"11".repeat(32),
                ExteriorActionV1::Capture,
                signer,
                None,
                key(0x71),
            )
            .is_ok()
        );
        report.head = key(0x66).to_string();
        assert!(
            authenticate_report(
                &report,
                ExpectedClusterV1::OwnedLoopback,
                &"11".repeat(32),
                ExteriorActionV1::Capture,
                signer,
                None,
                key(0x71),
            )
            .is_err()
        );
        report.head = head.to_string();
        report.phase = ExteriorPhaseV1::Prepared;
        assert!(
            authenticate_report(
                &report,
                ExpectedClusterV1::OwnedLoopback,
                &"11".repeat(32),
                ExteriorActionV1::Capture,
                signer,
                None,
                key(0x71),
            )
            .is_err(),
            "prepared phase unexpectedly omitted its signed packet"
        );
    }

    #[test]
    fn sponsored_actions_fit_the_routed_packet_ceiling() {
        const PACKET_DATA_SIZE: usize = 1_232;
        for width in [30_usize, 32, 29, 4] {
            let payer = Keypair::new();
            let mut accounts = Vec::with_capacity(width);
            accounts.push(AccountMeta::new(payer.pubkey(), true));
            for _ in 1..width {
                accounts.push(AccountMeta::new_readonly(Pubkey::new_unique(), false));
            }
            let instruction = Instruction {
                program_id: Pubkey::new_unique(),
                accounts,
                data: vec![0; 32],
            };
            let bounded =
                crate::rpc::bounded_instructions(std::slice::from_ref(&instruction), None)
                    .expect("bounded action");
            let table = frozen_table(
                u8::try_from(width).expect("small width"),
                instruction
                    .accounts
                    .iter()
                    .skip(1)
                    .map(|meta| meta.pubkey)
                    .collect(),
            );
            let routed = dclutch_versioned_message_operator::compile_v0_message(
                payer.pubkey(),
                &bounded,
                solana_hash::Hash::new_unique(),
                table.observation,
                std::slice::from_ref(&table),
            )
            .expect("routed v0 message");
            let transaction =
                solana_sdk::transaction::VersionedTransaction::try_new(routed.message, &[&payer])
                    .expect("signed v0 packet");
            let bytes = bincode::serialize(&transaction).expect("packet bytes");
            eprintln!(
                "sponsored width={width} routed packet bytes={}",
                bytes.len()
            );
            assert!(
                bytes.len() <= PACKET_DATA_SIZE,
                "width {width} is not packet-safe"
            );
        }
    }

    #[test]
    fn capture_and_failure_build_exact_frames() {
        let keys = keys();
        let coordinates = CoordinatesV1 {
            release_id: [0x70; 32],
            head: key(0x71),
            candidate: Some(key(0x72)),
            certificate: Some(key(0x73)),
            receipt: None,
        };
        let capture = capture_metas(key(0x74), &keys, &coordinates).expect("capture frame");
        assert_eq!(capture.len(), 30);
        assert!(
            capture
                .first()
                .is_some_and(|meta| meta.is_signer && meta.is_writable)
        );
        assert_eq!(capture.iter().filter(|meta| meta.is_writable).count(), 3);
        let failure = failure_metas(key(0x75), &keys, &coordinates).expect("failure frame");
        assert_eq!(failure.len(), 29);
        assert!(
            failure
                .first()
                .is_some_and(|meta| meta.is_signer && meta.is_writable)
        );
        assert_eq!(failure.iter().filter(|meta| meta.is_writable).count(), 4);
    }

    #[test]
    fn canonical_identity_parser_refuses_noncanonical_text() {
        let lower = "ab".repeat(32);
        assert_eq!(hex32(&lower).expect("canonical identity"), [0xab; 32]);
        assert!(hex32(&lower.to_uppercase()).is_err());
        assert!(hex32("11").is_err());
    }
}
