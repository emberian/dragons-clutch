//! Finalized, devnet-only exterior driver for one Pyth-resolved flagship Market.
//!
//! This module owns orchestration, durable stage receipts, and hostile resume
//! classification. It deliberately does not own any protocol wire: provider
//! submit/execute/reclaim and Core terminal admission are constructed by the
//! corresponding `dclutch-operator` builders.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_market_core_codec::{CoreState, Phase as CorePhase, Readiness};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    provider_transport_v3::{
        ProviderExecuteDeploymentV3, ProviderExecuteIntentV3, ProviderExecuteSnapshotV3,
        ProviderReclaimDeploymentV3, ProviderSubmitDeploymentV3, ProviderSubmitIntentV3,
        ProviderSubmitSnapshotV3, ProviderTransportReportV3, build_provider_execute_v3,
        build_provider_reclaim_v3, build_provider_submit_v3, compile_provider_execute_v0,
        compile_provider_reclaim_v0, compile_provider_submit_v0,
    },
};
use dclutch_pyth_svm::{
    FullPriceUpdateV2, GuardianSetV1, PostUpdateParamsView, ProgramDataV3View, ProgramV3View,
    PythReleaseV1, ReceiverConfigV2View, VerifiedEncodedVaaV1, devnet_release_v1,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3, RESOLUTION_CERTIFICATE_BYTES_V2,
    ResolutionCertificateV2,
};
use dclutch_source_contract::{
    PythAdapterConfigV1, SourceResolutionPhaseV1, SourceResolutionStateV2, WindowSpecV1,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk::{signature::Keypair, signer::Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_system_interface::instruction::transfer;

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1, bounded_instructions},
    wallet_terminal::authenticate_role,
};

const INPUT_FORMAT: &str = "dclutch-flagship-resolution-input-v1";
const CHECKPOINT_FORMAT: &str = "dclutch-flagship-resolution-checkpoint-v1";
const GEOMETRY_BLOCKHASH: [u8; 32] = [0x6d; 32];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LookupTablesV1 {
    submit: String,
    execute: String,
    reclaim: String,
}

/// Routing hints only. Every field is rejoined to a finalized Market, record,
/// PDA, activation cache, or provider release before it may reach a message.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountSelectorsV1 {
    market: String,
    source_state: String,
    source_material: String,
    source_spec: String,
    source_provider_release: String,
    adapter_config: String,
    window: String,
    statistic: String,
    pyth_release: String,
    product: String,
    result_domain: String,
    portfolio: String,
    certificate: String,
    activation_cache: String,
    infrastructure: String,
    registry_program: String,
    registry_programdata: String,
    registry_artifact: String,
    registry_artifact_staging: String,
    core_program: String,
    core_programdata: String,
    trading_program: String,
    trading_programdata: String,
    resolution_program: String,
    resolution_programdata: String,
    receiver_program: String,
    receiver_programdata: String,
    receiver_config: String,
    router_program: String,
    router_programdata: String,
    guardian_set: String,
    encoded_vaa: String,
    update_account: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanInputV1 {
    format: String,
    generation: u64,
    release_set: String,
    submitter: String,
    resolver: String,
    refund_recipient: String,
    terminal_sequence: u64,
    reclaim_after_unix_seconds: i64,
    post_update_body_base64: String,
    accounts: AccountSelectorsV1,
    lookup_tables: LookupTablesV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StageV1 {
    Submit,
    Execute,
    Reclaim,
    Complete,
}

impl StageV1 {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "submit" => Ok(Self::Submit),
            "execute" => Ok(Self::Execute),
            "reclaim" => Ok(Self::Reclaim),
            "complete" => Ok(Self::Complete),
            _ => Err(Error::new(
                "--through must be submit, execute, reclaim, or complete",
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::Execute => "execute",
            Self::Reclaim => "reclaim",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Debug)]
struct SelectedInputV1 {
    generation: u64,
    release_set: [u8; 32],
    submitter: Pubkey,
    resolver: Pubkey,
    refund_recipient: Pubkey,
    terminal_sequence: u64,
    reclaim_after_unix_seconds: i64,
    post_update_body: Vec<u8>,
    accounts: BTreeMap<&'static str, Pubkey>,
    lookup_tables: BTreeMap<StageV1, Pubkey>,
}

impl SelectedInputV1 {
    fn parse(input: &PlanInputV1) -> Result<Self> {
        if input.format != INPUT_FORMAT {
            return Err(Error::new(format!("input format must be {INPUT_FORMAT}")));
        }
        if input.generation == 0 || input.terminal_sequence == 0 {
            return Err(Error::new(
                "generation and terminalSequence must be positive",
            ));
        }
        let release_set = hex32(&input.release_set)?;
        if release_set == [0; 32] {
            return Err(Error::new("releaseSet must be nonzero"));
        }
        let submitter = nonzero_pubkey(&input.submitter, "submitter")?;
        let resolver = nonzero_pubkey(&input.resolver, "resolver")?;
        let refund_recipient = nonzero_pubkey(&input.refund_recipient, "refundRecipient")?;
        let post_update_body = BASE64
            .decode(&input.post_update_body_base64)
            .map_err(|error| Error::new(format!("postUpdateBodyBase64: {error}")))?;
        PostUpdateParamsView::parse(&post_update_body)
            .map_err(|error| Error::new(format!("postUpdateBodyBase64: {error:?}")))?;
        let mut accounts = BTreeMap::new();
        macro_rules! account {
            ($label:literal, $field:ident) => {{
                let value = nonzero_pubkey(&input.accounts.$field, $label)?;
                if accounts.insert($label, value).is_some() {
                    return Err(Error::new(format!("duplicate selector label {}", $label)));
                }
            }};
        }
        account!("market", market);
        account!("source_state", source_state);
        account!("source_material", source_material);
        account!("source_spec", source_spec);
        account!("source_provider_release", source_provider_release);
        account!("adapter_config", adapter_config);
        account!("window", window);
        account!("statistic", statistic);
        account!("pyth_release", pyth_release);
        account!("product", product);
        account!("result_domain", result_domain);
        account!("portfolio", portfolio);
        account!("certificate", certificate);
        account!("activation_cache", activation_cache);
        account!("infrastructure", infrastructure);
        account!("registry_program", registry_program);
        account!("registry_programdata", registry_programdata);
        account!("registry_artifact", registry_artifact);
        account!("registry_artifact_staging", registry_artifact_staging);
        account!("core_program", core_program);
        account!("core_programdata", core_programdata);
        account!("trading_program", trading_program);
        account!("trading_programdata", trading_programdata);
        account!("resolution_program", resolution_program);
        account!("resolution_programdata", resolution_programdata);
        account!("receiver_program", receiver_program);
        account!("receiver_programdata", receiver_programdata);
        account!("receiver_config", receiver_config);
        account!("router_program", router_program);
        account!("router_programdata", router_programdata);
        account!("guardian_set", guardian_set);
        account!("encoded_vaa", encoded_vaa);
        account!("update_account", update_account);
        let mut lookup_tables = BTreeMap::new();
        for (stage, value, label) in [
            (
                StageV1::Submit,
                &input.lookup_tables.submit,
                "lookupTables.submit",
            ),
            (
                StageV1::Execute,
                &input.lookup_tables.execute,
                "lookupTables.execute",
            ),
            (
                StageV1::Reclaim,
                &input.lookup_tables.reclaim,
                "lookupTables.reclaim",
            ),
        ] {
            lookup_tables.insert(stage, nonzero_pubkey(value, label)?);
        }
        let selected = Self {
            generation: input.generation,
            release_set,
            submitter,
            resolver,
            refund_recipient,
            terminal_sequence: input.terminal_sequence,
            reclaim_after_unix_seconds: input.reclaim_after_unix_seconds,
            post_update_body,
            accounts,
            lookup_tables,
        };
        selected.require_distinct()?;
        Ok(selected)
    }

    fn account(&self, label: &'static str) -> Result<Pubkey> {
        self.accounts
            .get(label)
            .copied()
            .ok_or_else(|| Error::new(format!("internal missing selector {label}")))
    }

    fn table(&self, stage: StageV1) -> Result<Pubkey> {
        self.lookup_tables
            .get(&stage)
            .copied()
            .ok_or_else(|| Error::new(format!("internal missing {} lookup table", stage.label())))
    }

    fn require_distinct(&self) -> Result<()> {
        let mut seen = BTreeMap::<Pubkey, &'static str>::new();
        for (&label, &key) in &self.accounts {
            if let Some(other) = seen.insert(key, label) {
                return Err(Error::new(format!(
                    "address-book substitution: {label} and {other} both name {key}"
                )));
            }
        }
        if self.submitter == self.accounts["update_account"]
            || self.resolver == self.accounts["update_account"]
        {
            return Err(Error::new(
                "the vacant Receiver update signer must be distinct from submitter and resolver",
            ));
        }
        Ok(())
    }
}

fn nonzero_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    let key = pubkey(value)?;
    if key == Pubkey::default() {
        return Err(Error::new(format!("{label} must be nonzero")));
    }
    Ok(key)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotKindV1 {
    Vacant,
    Submitted,
    Consumed,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChainFactsV1 {
    market_phase: CorePhase,
    market_readiness: Readiness,
    source_phase: SourceResolutionPhaseV1,
    lifecycle: SlotKindV1,
    update: SlotKindV1,
    certificate: SlotKindV1,
}

fn classify(facts: ChainFactsV1) -> Result<StageV1> {
    use SlotKindV1::{Consumed, Other, Submitted, Vacant};
    match (
        facts.market_phase,
        facts.market_readiness,
        facts.source_phase,
        facts.lifecycle,
        facts.update,
        facts.certificate,
    ) {
        (
            CorePhase::Open,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Primary,
            Vacant,
            Vacant,
            Vacant,
        ) => Ok(StageV1::Submit),
        (
            CorePhase::Open,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Primary,
            Submitted,
            Submitted,
            Vacant,
        ) => Ok(StageV1::Execute),
        (
            CorePhase::Terminal,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted,
            Consumed,
            Submitted,
            Submitted,
        ) => Ok(StageV1::Reclaim),
        (
            CorePhase::Terminal,
            Readiness::Consumed,
            SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted,
            Vacant,
            Vacant,
            Submitted,
        ) => Ok(StageV1::Complete),
        (_, _, _, Other, _, _) | (_, _, _, _, Other, _) | (_, _, _, _, _, Other) => Err(
            Error::new("ambiguous submitted state: an output account has an unknown owner or body"),
        ),
        _ => Err(Error::new(
            "ambiguous submitted state: Market, Source, lifecycle, update, and certificate do not form one canonical stage",
        )),
    }
}

#[derive(Clone)]
struct FinalizedSnapshotV1 {
    observation: Observation,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

impl FinalizedSnapshotV1 {
    fn account(&self, key: Pubkey, label: &str) -> Result<&RpcAccount> {
        self.accounts
            .get(&key)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("finalized snapshot is missing {label} {key}")))
    }

    fn optional(&self, key: Pubkey) -> Option<&RpcAccount> {
        self.accounts.get(&key).and_then(Option::as_ref)
    }

    fn observed(&self, key: Pubkey, label: &str) -> Result<ObservedAccount> {
        let account = self.account(key, label)?;
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

fn observe(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    stage: StageV1,
    minimum_slot: u64,
) -> Result<FinalizedSnapshotV1> {
    let mut keys = BTreeSet::new();
    keys.extend(selected.accounts.values().copied());
    keys.insert(lifecycle_address(selected)?);
    keys.insert(selected.table(stage)?);
    if keys.len() > 100 {
        return Err(Error::new(
            "flagship finalized snapshot exceeds the 100-account RPC bound",
        ));
    }
    let ordered = keys.into_iter().collect::<Vec<_>>();
    let (slot, values) = rpc.finalized_accounts(&ordered, minimum_slot)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    Ok(FinalizedSnapshotV1 {
        observation,
        accounts: ordered.into_iter().zip(values).collect(),
    })
}

fn chain_facts(selected: &SelectedInputV1, snapshot: &FinalizedSnapshotV1) -> Result<ChainFactsV1> {
    let market_key = selected.account("market")?;
    let core = selected.account("core_program")?;
    let market_account = snapshot.account(market_key, "Market")?;
    let market = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Market: {error:?}")))?;
    if market_account.owner != core
        || market_account.executable
        || market.identity.market_id.to_bytes() != market_key.to_bytes()
        || market.identity.generation != selected.generation
        || market.identity.selected_release_set.to_bytes() != selected.release_set
        || market.identity.registry_program.to_bytes()
            != selected.account("registry_program")?.to_bytes()
        || market.identity.resolution_policy.to_bytes()
            != hash(
                &snapshot
                    .account(selected.account("source_material")?, "SourceMaterial")?
                    .data,
            )
            .to_bytes()
    {
        return Err(Error::new(
            "wrong release, Market, generation, or source material",
        ));
    }
    let source_key = selected.account("source_state")?;
    let resolution = selected.account("resolution_program")?;
    let source_account = snapshot.account(source_key, "Source state")?;
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("Source state: {error:?}")))?;
    if source_account.owner != resolution
        || source_account.executable
        || source.market() != market_key.to_bytes()
        || source.generation() != selected.generation
        || source.material_id().to_bytes() != market.identity.resolution_policy.to_bytes()
    {
        return Err(Error::new(
            "Source state does not belong to this Market generation",
        ));
    }
    let lifecycle = lifecycle_kind(selected, snapshot, market_key, source_key)?;
    let update = update_kind(selected, snapshot)?;
    let certificate = certificate_kind(selected, snapshot, market_key)?;
    Ok(ChainFactsV1 {
        market_phase: market.phase,
        market_readiness: market.readiness,
        source_phase: source.phase(),
        lifecycle,
        update,
        certificate,
    })
}

fn is_vacant(account: Option<&RpcAccount>) -> bool {
    match account {
        None => true,
        Some(account) => {
            account.owner == system_program::ID && !account.executable && account.data.is_empty()
        }
    }
}

fn lifecycle_kind(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    market: Pubkey,
    source_state: Pubkey,
) -> Result<SlotKindV1> {
    let update = selected.account("update_account")?;
    let resolution = selected.account("resolution_program")?;
    let lifecycle = lifecycle_address(selected)?;
    let account = snapshot.optional(lifecycle);
    if is_vacant(account) {
        return Ok(SlotKindV1::Vacant);
    }
    let Some(account) = account else {
        return Ok(SlotKindV1::Vacant);
    };
    if account.owner != resolution || account.executable {
        return Ok(SlotKindV1::Other);
    }
    let lifecycle = match ProviderUpdateLifecycleV3::decode(&account.data) {
        Ok(value) => value,
        Err(_) => return Ok(SlotKindV1::Other),
    };
    if lifecycle.market != market.to_bytes()
        || lifecycle.source_state != source_state.to_bytes()
        || lifecycle.generation != selected.generation
        || lifecycle.release_set != selected.release_set
        || lifecycle.update_account != update.to_bytes()
        || lifecycle.provider_submitter != selected.submitter.to_bytes()
        || lifecycle.refund_recipient != selected.refund_recipient.to_bytes()
        || lifecycle.post_body_digest != hash(&selected.post_update_body).to_bytes()
    {
        return Ok(SlotKindV1::Other);
    }
    Ok(match lifecycle.status {
        ProviderUpdateStatusV3::Submitted => SlotKindV1::Submitted,
        ProviderUpdateStatusV3::Consumed => SlotKindV1::Consumed,
    })
}

fn lifecycle_address(selected: &SelectedInputV1) -> Result<Pubkey> {
    Ok(Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            selected.account("update_account")?.as_ref(),
        ],
        &selected.account("resolution_program")?,
    )
    .0)
}

fn update_kind(selected: &SelectedInputV1, snapshot: &FinalizedSnapshotV1) -> Result<SlotKindV1> {
    let key = selected.account("update_account")?;
    let account = snapshot.optional(key);
    if is_vacant(account) {
        return Ok(SlotKindV1::Vacant);
    }
    let Some(account) = account else {
        return Ok(SlotKindV1::Vacant);
    };
    if account.owner != selected.account("receiver_program")?
        || account.executable
        || FullPriceUpdateV2::parse(&account.data).is_err()
    {
        return Ok(SlotKindV1::Other);
    }
    Ok(SlotKindV1::Submitted)
}

fn certificate_kind(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    market: Pubkey,
) -> Result<SlotKindV1> {
    let key = selected.account("certificate")?;
    let account = snapshot.optional(key);
    if is_vacant(account) {
        return Ok(SlotKindV1::Vacant);
    }
    let Some(account) = account else {
        return Ok(SlotKindV1::Vacant);
    };
    if account.owner != selected.account("resolution_program")? || account.executable {
        return Ok(SlotKindV1::Other);
    }
    let certificate = match ResolutionCertificateV2::decode(&account.data) {
        Ok(value) => value,
        Err(_) => return Ok(SlotKindV1::Other),
    };
    if certificate.market != market.to_bytes()
        || certificate.generation != selected.generation
        || certificate.receipt_account != key.to_bytes()
    {
        return Ok(SlotKindV1::Other);
    }
    Ok(SlotKindV1::Submitted)
}

fn authenticate_current_deployments(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<()> {
    let registry = snapshot.observed(selected.account("registry_program")?, "Registry program")?;
    let activation =
        snapshot.observed(selected.account("activation_cache")?, "activation cache")?;
    for (role, program_label, programdata_label) in [
        (ExecutionRoleV1::Core, "core_program", "core_programdata"),
        (
            ExecutionRoleV1::Trading,
            "trading_program",
            "trading_programdata",
        ),
        (
            ExecutionRoleV1::Resolution,
            "resolution_program",
            "resolution_programdata",
        ),
    ] {
        authenticate_role(
            &registry,
            &activation,
            selected.release_set,
            role,
            &snapshot.observed(selected.account(program_label)?, program_label)?,
            &snapshot.observed(selected.account(programdata_label)?, programdata_label)?,
        )?;
    }
    Ok(())
}

fn authenticate_devnet_pyth(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    require_provider_observation: bool,
) -> Result<PythReleaseV1> {
    let release = PythReleaseV1::decode(
        &snapshot
            .account(selected.account("pyth_release")?, "Pyth release record")?
            .data,
    )
    .map_err(|error| Error::new(format!("Pyth release: {error:?}")))?;
    let expected = devnet_release_v1()
        .map_err(|error| Error::new(format!("compiled devnet Pyth release: {error:?}")))?;
    if release.to_bytes() != expected.to_bytes() {
        return Err(Error::new(
            "Pyth release record is not the exact devnet production row",
        ));
    }
    for (program_label, programdata_label, expected_slot, expected_program, expected_data) in [
        (
            "receiver_program",
            "receiver_programdata",
            release.receiver_deployment_slot(),
            release.receiver_program(),
            release.receiver_programdata(),
        ),
        (
            "router_program",
            "router_programdata",
            release.router_deployment_slot(),
            release.router_program(),
            release.router_programdata(),
        ),
    ] {
        let program_key = selected.account(program_label)?;
        let data_key = selected.account(programdata_label)?;
        let program = snapshot.account(program_key, program_label)?;
        let programdata = snapshot.account(data_key, programdata_label)?;
        let view = ProgramV3View::parse(&program.data)
            .map_err(|error| Error::new(format!("{program_label}: {error:?}")))?;
        let data_view = ProgramDataV3View::parse(&programdata.data)
            .map_err(|error| Error::new(format!("{programdata_label}: {error:?}")))?;
        if program_key.to_bytes() != expected_program
            || data_key.to_bytes() != expected_data
            || program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || view.programdata() != data_key.to_bytes()
            || programdata.owner != bpf_loader_upgradeable::ID
            || programdata.executable
            || data_view.deployment_slot() != expected_slot
        {
            return Err(Error::new(format!(
                "current {program_label} Program/ProgramData link, slot, owner, or executable bit refused"
            )));
        }
    }
    if selected.account("receiver_config")?.to_bytes() != release.receiver_config() {
        return Err(Error::new("Receiver Config address substitution refused"));
    }
    let config = snapshot.account(selected.account("receiver_config")?, "Receiver Config")?;
    let config_view = ReceiverConfigV2View::parse(&config.data)
        .map_err(|error| Error::new(format!("Receiver Config body: {error:?}")))?;
    if config.owner.to_bytes() != release.receiver_program()
        || config.executable
        || hash(&config.data).to_bytes() != release.config_digest()
        || config_view.router_program() != release.router_program()
        || config_view.minimum_signatures() != release.required_guardian_count()
    {
        return Err(Error::new(
            "Receiver Config owner, body, router, or threshold refused",
        ));
    }
    if !require_provider_observation {
        return Ok(release);
    }
    let encoded = snapshot.account(selected.account("encoded_vaa")?, "verified EncodedVaa")?;
    let encoded_view = VerifiedEncodedVaaV1::parse(&encoded.data)
        .map_err(|error| Error::new(format!("verified EncodedVaa: {error:?}")))?;
    let guardian = snapshot.account(selected.account("guardian_set")?, "GuardianSet")?;
    let guardian_view = GuardianSetV1::parse(&guardian.data)
        .map_err(|error| Error::new(format!("GuardianSet: {error:?}")))?;
    let expected_guardian = Pubkey::find_program_address(
        &[
            b"GuardianSet",
            &encoded_view.guardian_set_index().to_be_bytes(),
        ],
        &selected.account("router_program")?,
    )
    .0;
    if encoded.owner != selected.account("router_program")?
        || encoded.executable
        || encoded_view.write_authority() != selected.submitter.to_bytes()
        || guardian.owner != selected.account("router_program")?
        || guardian.executable
        || selected.account("guardian_set")? != expected_guardian
        || guardian_view
            .authenticate(
                encoded_view,
                release.guardian_set_count(),
                release.required_guardian_count(),
            )
            .is_err()
    {
        return Err(Error::new(
            "EncodedVaa/GuardianSet account, authority, PDA, or signature threshold refused",
        ));
    }
    Ok(release)
}

fn preflight_posted_observation(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<()> {
    let update = FullPriceUpdateV2::parse(
        &snapshot
            .account(selected.account("update_account")?, "Receiver update")?
            .data,
    )
    .map_err(|error| Error::new(format!("Receiver update: {error:?}")))?;
    let window = WindowSpecV1::decode(
        &snapshot
            .account(selected.account("window")?, "WindowSpec")?
            .data,
    )
    .map_err(|error| Error::new(format!("WindowSpec: {error:?}")))?;
    let adapter = PythAdapterConfigV1::decode(
        &snapshot
            .account(selected.account("adapter_config")?, "PythAdapterConfig")?
            .data,
    )
    .map_err(|error| Error::new(format!("PythAdapterConfig: {error:?}")))?;
    validate_observation_fields(
        update.publish_time(),
        update.feed_id(),
        update.price(),
        update.confidence(),
        update.exponent(),
        snapshot.observation.unix_timestamp,
        window,
        adapter,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_observation_fields(
    publication: i64,
    feed_id: [u8; 32],
    price: i64,
    confidence: u64,
    exponent: i32,
    finalized_now: i64,
    window: WindowSpecV1,
    adapter: PythAdapterConfigV1,
) -> Result<()> {
    let oldest = finalized_now
        .checked_sub(i64::from(window.max_age_seconds()))
        .ok_or_else(|| Error::new("Pyth freshness lower bound overflow"))?;
    let newest = finalized_now
        .checked_add(i64::from(window.max_future_skew_seconds()))
        .ok_or_else(|| Error::new("Pyth freshness upper bound overflow"))?;
    let in_schedule = window
        .contains_observation(publication)
        .map_err(|error| Error::new(format!("Pyth observation schedule: {error:?}")))?;
    if !in_schedule || publication < oldest || publication > newest {
        return Err(Error::new(format!(
            "stale or wrong-period Pyth observation: publication {publication}, Market window [{}, {}], finalized freshness band [{oldest}, {newest}]",
            window.start_unix_seconds(),
            window.end_unix_seconds()
        )));
    }
    let confidence_limit = u128::from(price.unsigned_abs())
        .checked_mul(u128::from(adapter.max_confidence_bps()))
        .ok_or_else(|| Error::new("Pyth confidence limit overflow"))?;
    let observed_confidence = u128::from(confidence)
        .checked_mul(10_000)
        .ok_or_else(|| Error::new("Pyth observed confidence overflow"))?;
    if feed_id != adapter.provider_feed_id()
        || exponent != adapter.expected_exponent()
        || observed_confidence > confidence_limit
    {
        return Err(Error::new(
            "Pyth update feed, exponent, or confidence differs from the finalized adapter record",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountMetaPlanV1 {
    pubkey: String,
    signer: bool,
    writable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstructionPlanV1 {
    program_id: String,
    accounts: Vec<AccountMetaPlanV1>,
    data_base64: String,
    sha256: String,
}

impl InstructionPlanV1 {
    fn from_instruction(instruction: &Instruction) -> Result<Self> {
        let serialized = bincode::serialize(instruction)
            .map_err(|error| Error::new(format!("serialize stage instruction: {error}")))?;
        Ok(Self {
            program_id: instruction.program_id.to_string(),
            accounts: instruction
                .accounts
                .iter()
                .map(|meta| AccountMetaPlanV1 {
                    pubkey: meta.pubkey.to_string(),
                    signer: meta.is_signer,
                    writable: meta.is_writable,
                })
                .collect(),
            data_base64: BASE64.encode(&instruction.data),
            sha256: hex(&Sha256::digest(serialized)),
        })
    }

    fn instruction(&self) -> Result<Instruction> {
        let instruction = Instruction {
            program_id: pubkey(&self.program_id)?,
            accounts: self
                .accounts
                .iter()
                .map(|meta| {
                    Ok(if meta.writable {
                        AccountMeta::new(pubkey(&meta.pubkey)?, meta.signer)
                    } else {
                        AccountMeta::new_readonly(pubkey(&meta.pubkey)?, meta.signer)
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            data: BASE64
                .decode(&self.data_base64)
                .map_err(|error| Error::new(format!("checkpoint instruction body: {error}")))?,
        };
        if InstructionPlanV1::from_instruction(&instruction)?.sha256 != self.sha256 {
            return Err(Error::new("checkpoint instruction digest mismatch"));
        }
        Ok(instruction)
    }
}

impl StagePlanV1 {
    fn validate(&self) -> Result<()> {
        if self.stage == StageV1::Complete
            || self.observation_slot == 0
            || self.required_signers.is_empty()
            || self.compiled_wire_bytes > 1_232
        {
            return Err(Error::new("durable stage header is not executable"));
        }
        let payer = pubkey(
            self.required_signers
                .first()
                .ok_or_else(|| Error::new("durable stage omitted payer"))?,
        )?;
        let action = self.action.instruction()?;
        let mut unbounded = Vec::with_capacity(self.transfers.len() + 1);
        for top_up in &self.transfers {
            unbounded.push(transfer(
                &payer,
                &pubkey(&top_up.destination)?,
                top_up.lamports,
            ));
        }
        unbounded.push(action);
        let expected = bounded_instructions(&unbounded, None)?
            .iter()
            .map(InstructionPlanV1::from_instruction)
            .collect::<Result<Vec<_>>>()?;
        if expected != self.transaction_instructions {
            return Err(Error::new(
                "durable transaction instructions differ from transfers, action, or compute policy",
            ));
        }
        pubkey(&self.lookup_table)?;
        pubkey(&self.mutation_account)?;
        for signer in &self.required_signers {
            pubkey(signer)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransferPlanV1 {
    destination: String,
    lamports: u64,
    purpose: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArithmeticPlanV1 {
    lifecycle_rent_lamports: u64,
    update_rent_lamports: u64,
    certificate_rent_lamports: u64,
    provider_fee_lamports: u64,
    expected_reclaim_update_lamports: u64,
    expected_reclaim_lifecycle_lamports: u64,
    expected_reclaim_total_lamports: u64,
}

impl Default for ArithmeticPlanV1 {
    fn default() -> Self {
        Self {
            lifecycle_rent_lamports: 0,
            update_rent_lamports: 0,
            certificate_rent_lamports: 0,
            provider_fee_lamports: 0,
            expected_reclaim_update_lamports: 0,
            expected_reclaim_lifecycle_lamports: 0,
            expected_reclaim_total_lamports: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StagePlanV1 {
    stage: StageV1,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    action: InstructionPlanV1,
    transaction_instructions: Vec<InstructionPlanV1>,
    lookup_table: String,
    lookup_table_account_sha256: String,
    compiled_wire_bytes: usize,
    compiled_loaded_addresses: usize,
    required_signers: Vec<String>,
    transfers: Vec<TransferPlanV1>,
    arithmetic: ArithmeticPlanV1,
    mutation_account: String,
    submission_armed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResumeActionV1 {
    RecoverFinalized,
    ReprepareUnsigned,
}

fn resume_action(current: StageV1, prior: &StagePlanV1) -> Result<ResumeActionV1> {
    if current > prior.stage {
        return Ok(ResumeActionV1::RecoverFinalized);
    }
    if current < prior.stage {
        return Err(Error::new(format!(
            "chain stage {} precedes durable stage {}; replay or address-book substitution refused",
            current.label(),
            prior.stage.label()
        )));
    }
    if prior.submission_armed {
        return Err(Error::new(format!(
            "durable {} submission was armed but the finalized chain has not advanced; ambiguous submitted state refuses another signature",
            current.label()
        )));
    }
    Ok(ResumeActionV1::ReprepareUnsigned)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StageReceiptV1 {
    stage: StageV1,
    signature: String,
    slot: u64,
    fee_lamports: u64,
    transfer_fee_lamports: u64,
    arithmetic: ArithmeticPlanV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckpointV1 {
    format: String,
    input_sha256: String,
    stage_plan: Option<StagePlanV1>,
    receipts: Vec<StageReceiptV1>,
    verified_terminal: bool,
}

struct PreparedStageV1 {
    plan: StagePlanV1,
    instructions: Vec<Instruction>,
    table: ObservedAccount,
}

fn provider_submit_report(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<ProviderTransportReportV3> {
    let report = build_provider_submit_v3(
        &ProviderSubmitSnapshotV3 {
            market: snapshot.observed(selected.account("market")?, "Market")?,
            source_state: snapshot.observed(selected.account("source_state")?, "Source state")?,
            source_material: snapshot
                .observed(selected.account("source_material")?, "SourceMaterial")?,
            source_spec: snapshot.observed(selected.account("source_spec")?, "SourceSpec")?,
            source_provider_release: snapshot.observed(
                selected.account("source_provider_release")?,
                "ProviderRelease",
            )?,
            pyth_release: snapshot.observed(selected.account("pyth_release")?, "Pyth release")?,
            window: snapshot.observed(selected.account("window")?, "WindowSpec")?,
            encoded_vaa: snapshot
                .observed(selected.account("encoded_vaa")?, "verified EncodedVaa")?,
        },
        ProviderSubmitDeploymentV3 {
            infrastructure: selected.account("infrastructure")?,
            registry_programdata: selected.account("registry_programdata")?,
            registry_artifact: selected.account("registry_artifact")?,
            registry_artifact_staging: selected.account("registry_artifact_staging")?,
            core_programdata: selected.account("core_programdata")?,
            resolution_program: selected.account("resolution_program")?,
            resolution_programdata: selected.account("resolution_programdata")?,
            receiver_config: selected.account("receiver_config")?,
            guardian_set: selected.account("guardian_set")?,
        },
        &ProviderSubmitIntentV3 {
            submitter: selected.submitter,
            refund_recipient: selected.refund_recipient,
            update_account: selected.account("update_account")?,
            reclaim_after_unix_seconds: selected.reclaim_after_unix_seconds,
            post_update_body: selected.post_update_body.clone(),
        },
    )
    .map_err(|error| Error::new(format!("provider submit builder: {error:?}")))?;
    if report.lifecycle != lifecycle_address(selected)? {
        return Err(Error::new(
            "provider submit derived an unexpected lifecycle PDA",
        ));
    }
    Ok(report)
}

fn provider_execute_report(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<ProviderTransportReportV3> {
    let report = build_provider_execute_v3(
        &ProviderExecuteSnapshotV3 {
            market: snapshot.observed(selected.account("market")?, "Market")?,
            source_state: snapshot.observed(selected.account("source_state")?, "Source state")?,
            lifecycle: snapshot.observed(lifecycle_address(selected)?, "provider lifecycle")?,
            update: snapshot.observed(selected.account("update_account")?, "Receiver update")?,
            source_material: snapshot
                .observed(selected.account("source_material")?, "SourceMaterial")?,
            source_spec: snapshot.observed(selected.account("source_spec")?, "SourceSpec")?,
            source_provider_release: snapshot.observed(
                selected.account("source_provider_release")?,
                "ProviderRelease",
            )?,
            adapter_config: snapshot
                .observed(selected.account("adapter_config")?, "PythAdapterConfig")?,
            window: snapshot.observed(selected.account("window")?, "WindowSpec")?,
            statistic: snapshot.observed(selected.account("statistic")?, "StatisticSpec")?,
            pyth_release: snapshot.observed(selected.account("pyth_release")?, "Pyth release")?,
            product: snapshot.observed(selected.account("product")?, "Product")?,
            result_domain: snapshot.observed(selected.account("result_domain")?, "ResultDomain")?,
            portfolio: snapshot.observed(selected.account("portfolio")?, "Portfolio")?,
        },
        ProviderExecuteDeploymentV3 {
            registry_programdata: selected.account("registry_programdata")?,
            registry_artifact: selected.account("registry_artifact")?,
            registry_artifact_staging: selected.account("registry_artifact_staging")?,
            core_programdata: selected.account("core_programdata")?,
            trading_program: selected.account("trading_program")?,
            trading_programdata: selected.account("trading_programdata")?,
            resolution_program: selected.account("resolution_program")?,
            resolution_programdata: selected.account("resolution_programdata")?,
            receiver_config: selected.account("receiver_config")?,
        },
        &ProviderExecuteIntentV3 {
            resolver: selected.resolver,
            terminal_sequence: selected.terminal_sequence,
            post_update_body: selected.post_update_body.clone(),
        },
    )
    .map_err(|error| Error::new(format!("provider execute builder: {error:?}")))?;
    let certificate = report
        .instruction
        .accounts
        .get(3)
        .ok_or_else(|| Error::new("provider execute report lost certificate account"))?
        .pubkey;
    if certificate != selected.account("certificate")? {
        return Err(Error::new("certificate address substitution refused"));
    }
    Ok(report)
}

fn provider_reclaim_report(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<ProviderTransportReportV3> {
    build_provider_reclaim_v3(
        &snapshot.observed(lifecycle_address(selected)?, "provider lifecycle")?,
        &snapshot.observed(selected.account("pyth_release")?, "Pyth release")?,
        ProviderReclaimDeploymentV3 {
            resolver: selected.resolver,
            registry_programdata: selected.account("registry_programdata")?,
            resolution_program: selected.account("resolution_program")?,
            resolution_programdata: selected.account("resolution_programdata")?,
        },
    )
    .map_err(|error| Error::new(format!("provider reclaim builder: {error:?}")))
}

fn vacant_top_up(
    snapshot: &FinalizedSnapshotV1,
    destination: Pubkey,
    rent_minimum: u64,
    purpose: &str,
) -> Result<Option<TransferPlanV1>> {
    let current = snapshot.optional(destination);
    if !is_vacant(current) {
        return Err(Error::new(format!("{purpose} destination is not vacant")));
    }
    let lamports = current.map_or(0, |account| account.lamports);
    if lamports > rent_minimum {
        return Err(Error::new(format!(
            "{purpose} vacant account holds {lamports} lamports, above exact rent {rent_minimum}"
        )));
    }
    let missing = rent_minimum
        .checked_sub(lamports)
        .ok_or_else(|| Error::new(format!("{purpose} rent subtraction overflow")))?;
    Ok((missing != 0).then(|| TransferPlanV1 {
        destination: destination.to_string(),
        lamports: missing,
        purpose: purpose.to_owned(),
    }))
}

fn table_account_digest(account: &ObservedAccount) -> String {
    let mut hasher = Sha256::new();
    hasher.update(account.key.as_ref());
    hasher.update(account.owner.as_ref());
    hasher.update(account.lamports.to_le_bytes());
    hasher.update([u8::from(account.executable)]);
    hasher.update(&account.data);
    hex(&hasher.finalize())
}

fn prepare_stage(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    stage: StageV1,
) -> Result<PreparedStageV1> {
    authenticate_current_deployments(selected, snapshot)?;
    match stage {
        StageV1::Submit | StageV1::Execute => {
            authenticate_devnet_pyth(selected, snapshot, true)?;
        }
        StageV1::Reclaim => {
            authenticate_devnet_pyth(selected, snapshot, false)?;
        }
        StageV1::Complete => {}
    }
    let observed_stage = classify(chain_facts(selected, snapshot)?)?;
    if observed_stage != stage {
        return Err(Error::new(format!(
            "stage changed across finalized observations: selected {}, now {}",
            stage.label(),
            observed_stage.label()
        )));
    }
    let table = snapshot.observed(selected.table(stage)?, "stage lookup table")?;
    let mut arithmetic = ArithmeticPlanV1::default();
    let mut transfers = Vec::new();
    let (
        action,
        required_signers,
        _builder_wire_bytes,
        _builder_loaded_addresses,
        mutation_account,
    ) = match stage {
        StageV1::Submit => {
            let report = provider_submit_report(selected, snapshot)?;
            let lifecycle_rent = rpc.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3)?;
            let update_rent = rpc.minimum_balance(dclutch_pyth_svm::FULL_PRICE_UPDATE_V2_LEN)?;
            let config = ReceiverConfigV2View::parse(
                &snapshot
                    .account(selected.account("receiver_config")?, "Receiver Config")?
                    .data,
            )
            .map_err(|error| Error::new(format!("Receiver Config: {error:?}")))?;
            arithmetic.lifecycle_rent_lamports = lifecycle_rent;
            arithmetic.update_rent_lamports = update_rent;
            arithmetic.provider_fee_lamports = config.fee();
            if let Some(transfer) = vacant_top_up(
                snapshot,
                lifecycle_address(selected)?,
                lifecycle_rent,
                "provider lifecycle",
            )? {
                transfers.push(transfer);
            }
            let compiled = compile_provider_submit_v0(
                &report,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                std::slice::from_ref(&table),
            )
            .map_err(|error| Error::new(format!("provider submit v0 geometry: {error:?}")))?;
            (
                report.instruction,
                compiled.required_signers,
                compiled.message.wire_bytes,
                compiled.message.loaded_addresses,
                selected.account("update_account")?,
            )
        }
        StageV1::Execute => {
            preflight_posted_observation(selected, snapshot)?;
            let report = provider_execute_report(selected, snapshot)?;
            let certificate_rent = rpc.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2)?;
            arithmetic.certificate_rent_lamports = certificate_rent;
            let lifecycle = ProviderUpdateLifecycleV3::decode(
                &snapshot
                    .account(lifecycle_address(selected)?, "provider lifecycle")?
                    .data,
            )
            .map_err(|error| Error::new(format!("provider lifecycle: {error:?}")))?;
            arithmetic.update_rent_lamports = lifecycle.update_rent_lamports;
            arithmetic.provider_fee_lamports = lifecycle.provider_fee_lamports;
            if let Some(transfer) = vacant_top_up(
                snapshot,
                selected.account("certificate")?,
                certificate_rent,
                "terminal certificate",
            )? {
                transfers.push(transfer);
            }
            let compiled = compile_provider_execute_v0(
                &report,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                std::slice::from_ref(&table),
            )
            .map_err(|error| Error::new(format!("provider execute v0 geometry: {error:?}")))?;
            (
                report.instruction,
                compiled.required_signers,
                compiled.message.wire_bytes,
                compiled.message.loaded_addresses,
                selected.account("source_state")?,
            )
        }
        StageV1::Reclaim => {
            let report = provider_reclaim_report(selected, snapshot)?;
            let lifecycle_account =
                snapshot.account(lifecycle_address(selected)?, "provider lifecycle")?;
            let lifecycle = ProviderUpdateLifecycleV3::decode(&lifecycle_account.data)
                .map_err(|error| Error::new(format!("provider lifecycle: {error:?}")))?;
            if snapshot.observation.unix_timestamp < lifecycle.reclaim_after_unix_seconds {
                return Err(Error::new(format!(
                    "reclaim is premature: finalized clock {} is before {}",
                    snapshot.observation.unix_timestamp, lifecycle.reclaim_after_unix_seconds
                )));
            }
            arithmetic.expected_reclaim_update_lamports = lifecycle.update_rent_lamports;
            arithmetic.expected_reclaim_lifecycle_lamports = lifecycle_account.lamports;
            arithmetic.expected_reclaim_total_lamports = lifecycle
                .update_rent_lamports
                .checked_add(lifecycle_account.lamports)
                .ok_or_else(|| Error::new("reclaim credit overflow"))?;
            let compiled = compile_provider_reclaim_v0(
                &report,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                std::slice::from_ref(&table),
            )
            .map_err(|error| Error::new(format!("provider reclaim v0 geometry: {error:?}")))?;
            (
                report.instruction,
                compiled.required_signers,
                compiled.message.wire_bytes,
                compiled.message.loaded_addresses,
                selected.account("update_account")?,
            )
        }
        StageV1::Complete => return Err(Error::new("complete has no transaction plan")),
    };
    let expected_signers = match stage {
        StageV1::Submit => vec![selected.submitter, selected.account("update_account")?],
        StageV1::Execute | StageV1::Reclaim => vec![selected.resolver],
        StageV1::Complete => Vec::new(),
    };
    if required_signers != expected_signers {
        return Err(Error::new(format!(
            "{} compiler signer boundary changed",
            stage.label()
        )));
    }
    let mut instructions = Vec::with_capacity(transfers.len() + 1);
    for top_up in &transfers {
        instructions.push(transfer(
            required_signers
                .first()
                .ok_or_else(|| Error::new("stage has no fee payer"))?,
            &pubkey(&top_up.destination)?,
            top_up.lamports,
        ));
    }
    instructions.push(action.clone());
    let bounded = bounded_instructions(&instructions, None)?;
    let routed = dclutch_versioned_message_operator::compile_v0_message(
        *required_signers
            .first()
            .ok_or_else(|| Error::new("stage has no fee payer"))?,
        &bounded,
        Hash::new_from_array(GEOMETRY_BLOCKHASH),
        snapshot.observation,
        std::slice::from_ref(&table),
    )
    .map_err(|error| {
        Error::new(format!(
            "{} atomic stage geometry: {error:?}",
            stage.label()
        ))
    })?;
    if usize::from(routed.required_signatures) != required_signers.len() {
        return Err(Error::new(format!(
            "{} atomic prepay changed the signer boundary",
            stage.label()
        )));
    }
    let action_plan = InstructionPlanV1::from_instruction(&action)?;
    let transaction_instructions = bounded
        .iter()
        .map(InstructionPlanV1::from_instruction)
        .collect::<Result<Vec<_>>>()?;
    let plan = StagePlanV1 {
        stage,
        observation_slot: snapshot.observation.slot,
        observation_unix_timestamp: snapshot.observation.unix_timestamp,
        action: action_plan,
        transaction_instructions,
        lookup_table: table.key.to_string(),
        lookup_table_account_sha256: table_account_digest(&table),
        compiled_wire_bytes: routed.wire_bytes,
        compiled_loaded_addresses: routed.loaded_addresses,
        required_signers: required_signers.iter().map(ToString::to_string).collect(),
        transfers,
        arithmetic,
        mutation_account: mutation_account.to_string(),
        submission_armed: false,
    };
    // Persistence must round-trip the instruction before a secret can be read.
    if plan.action.instruction()? != action {
        return Err(Error::new(
            "durable stage instruction round-trip changed bytes or metas",
        ));
    }
    Ok(PreparedStageV1 {
        plan,
        instructions,
        table,
    })
}

#[derive(Default)]
struct CommandArgumentsV1 {
    rpc_url: Option<String>,
    acknowledgment: Option<String>,
    input: Option<PathBuf>,
    checkpoint: Option<PathBuf>,
    submitter_keypair: Option<PathBuf>,
    resolver_keypair: Option<PathBuf>,
    update_keypair: Option<PathBuf>,
    through: Option<StageV1>,
    execute: bool,
}

impl CommandArgumentsV1 {
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
                "--checkpoint" => {
                    set_once(&mut parsed.checkpoint, PathBuf::from(value), "--checkpoint")?
                }
                "--submitter-keypair" => set_once(
                    &mut parsed.submitter_keypair,
                    PathBuf::from(value),
                    "--submitter-keypair",
                )?,
                "--resolver-keypair" => set_once(
                    &mut parsed.resolver_keypair,
                    PathBuf::from(value),
                    "--resolver-keypair",
                )?,
                "--update-keypair" => set_once(
                    &mut parsed.update_keypair,
                    PathBuf::from(value),
                    "--update-keypair",
                )?,
                "--through" => {
                    if parsed.through.replace(StageV1::parse(&value)?).is_some() {
                        return Err(Error::new("--through may be supplied only once"));
                    }
                }
                _ => return Err(Error::new(format!("unknown flagship argument: {argument}"))),
            }
        }
        if !parsed.execute
            && (parsed.submitter_keypair.is_some()
                || parsed.resolver_keypair.is_some()
                || parsed.update_keypair.is_some())
        {
            return Err(Error::new(
                "keypair paths are refused in read-only preflight; add them only with --execute",
            ));
        }
        Ok(parsed)
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T, label: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(format!("{label} may be supplied only once")));
    }
    Ok(())
}

fn absolute(path: Option<PathBuf>, label: &str) -> Result<PathBuf> {
    let path = path.ok_or_else(|| Error::new(format!("{label} is required")))?;
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap flagship-resolution-v1 --rpc-url URL \
     --i-mean-devnet DEVNET_GENESIS --input ABSOLUTE_JSON \
     --checkpoint ABSOLUTE_JSON [--through submit|execute|reclaim|complete] \
     [--execute --submitter-keypair ABSOLUTE_JSON --resolver-keypair ABSOLUTE_JSON \
     --update-keypair ABSOLUTE_JSON]\n\nThe default is key-free, finalized, devnet-only preflight. \
     With --execute, each next chain-derived stage is durably written before the minimum \
     necessary key file is opened; no signer bytes or key paths enter the checkpoint."
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = CommandArgumentsV1::parse(arguments)?;
    let input_path = absolute(arguments.input.clone(), "--input")?;
    let checkpoint_path = absolute(arguments.checkpoint.clone(), "--checkpoint")?;
    let input_bytes = fs::read(&input_path).map_err(|error| {
        Error::new(format!(
            "read flagship input {}: {error}",
            input_path.display()
        ))
    })?;
    let input: PlanInputV1 = serde_json::from_slice(&input_bytes)?;
    let selected = SelectedInputV1::parse(&input)?;
    let input_sha256 = hex(&Sha256::digest(&input_bytes));
    let origin = ClusterOriginV1::parse(
        arguments
            .rpc_url
            .as_deref()
            .ok_or_else(|| Error::new("--rpc-url is required"))?,
        arguments.acknowledgment.as_deref(),
    )?;
    if origin.loopback_port().is_some() {
        return Err(Error::new(
            "flagship-resolution-v1 is the devnet exterior and refuses loopback origins",
        ));
    }
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&origin, policy)?;
    let mut checkpoint = load_checkpoint(&checkpoint_path, &input_sha256)?;
    let through = arguments.through.unwrap_or(StageV1::Complete);
    let mut minimum_slot = 0_u64;
    loop {
        let guessed = checkpoint
            .stage_plan
            .as_ref()
            .map_or(StageV1::Submit, |plan| plan.stage);
        let initial = observe(&mut rpc, &selected, guessed, minimum_slot)?;
        let initial_stage = classify(chain_facts(&selected, &initial)?)?;
        let snapshot = if initial_stage == guessed || initial_stage == StageV1::Complete {
            initial
        } else {
            observe(&mut rpc, &selected, initial_stage, initial.observation.slot)?
        };
        let stage = classify(chain_facts(&selected, &snapshot)?)?;
        if let Some(prior) = checkpoint.stage_plan.as_ref() {
            match resume_action(stage, prior)? {
                ResumeActionV1::RecoverFinalized => {
                    let recovered = recover_receipt(&mut rpc, prior)?;
                    checkpoint.receipts.push(recovered);
                    checkpoint.stage_plan = None;
                    write_checkpoint(&checkpoint_path, &checkpoint)?;
                }
                ResumeActionV1::ReprepareUnsigned => {}
            }
        }
        if stage == StageV1::Complete {
            verify_terminal(&selected, &snapshot)?;
            checkpoint.verified_terminal = true;
            checkpoint.stage_plan = None;
            write_checkpoint(&checkpoint_path, &checkpoint)?;
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
        if stage > through {
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
        let prepared = prepare_stage(&mut rpc, &selected, &snapshot, stage)?;
        checkpoint.stage_plan = Some(prepared.plan.clone());
        // This is the durable-before-secret boundary. No key file has been opened above.
        write_checkpoint(&checkpoint_path, &checkpoint)?;
        if !arguments.execute {
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
        let (payer, update) = load_stage_signers(&selected, stage, &arguments)?;
        checkpoint
            .stage_plan
            .as_mut()
            .ok_or_else(|| Error::new("durable stage disappeared before arming"))?
            .submission_armed = true;
        // A restart after this write must never sign again until chain state proves
        // that the atomic action finalized. No signer bytes or paths are persisted.
        write_checkpoint(&checkpoint_path, &checkpoint)?;
        let receipt = execute_stage(&mut rpc, &selected, &prepared, &payer, update.as_ref())?;
        minimum_slot = receipt.slot;
        checkpoint.receipts.push(receipt);
        checkpoint.stage_plan = None;
        write_checkpoint(&checkpoint_path, &checkpoint)?;
        if stage >= through {
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
            return Ok(());
        }
    }
}

fn load_checkpoint(path: &Path, input_sha256: &str) -> Result<CheckpointV1> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CheckpointV1 {
                format: CHECKPOINT_FORMAT.to_owned(),
                input_sha256: input_sha256.to_owned(),
                stage_plan: None,
                receipts: Vec::new(),
                verified_terminal: false,
            });
        }
        Err(error) => {
            return Err(Error::new(format!(
                "read checkpoint {}: {error}",
                path.display()
            )));
        }
    };
    let checkpoint: CheckpointV1 = serde_json::from_slice(&bytes)?;
    if checkpoint.format != CHECKPOINT_FORMAT || checkpoint.input_sha256 != input_sha256 {
        return Err(Error::new(
            "checkpoint format or input digest differs; cross-market resume refused",
        ));
    }
    if let Some(plan) = &checkpoint.stage_plan {
        plan.validate()?;
    }
    if checkpoint
        .receipts
        .windows(2)
        .any(|pair| pair[0].stage >= pair[1].stage || pair[0].slot > pair[1].slot)
    {
        return Err(Error::new(
            "checkpoint receipts are duplicated, out of order, or cross-run substituted",
        ));
    }
    Ok(checkpoint)
}

fn write_checkpoint(path: &Path, checkpoint: &CheckpointV1) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("checkpoint path has no parent"))?;
    if !parent.is_dir() {
        return Err(Error::new("checkpoint parent directory does not exist"));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("checkpoint filename is not UTF-8"))?;
    let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(checkpoint)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(&temporary).map_err(|error| {
        Error::new(format!(
            "create checkpoint temporary {}: {error}",
            temporary.display()
        ))
    })?;
    use std::io::Write as _;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path).map_err(|error| {
        Error::new(format!(
            "atomically install checkpoint {}: {error}",
            path.display()
        ))
    })?;
    Ok(())
}

fn load_keypair(path: Option<&PathBuf>, label: &str, expected: Pubkey) -> Result<Keypair> {
    let path = path.ok_or_else(|| Error::new(format!("--{label}-keypair is required")))?;
    let seed = read_keypair_file(path, label)?;
    let keypair = Keypair::new_from_array(seed);
    if keypair.pubkey() != expected {
        return Err(Error::new(format!(
            "{label} keypair public key {} differs from authenticated input {expected}",
            keypair.pubkey()
        )));
    }
    Ok(keypair)
}

fn lamports(rpc: &mut Rpc, key: Pubkey, label: &str) -> Result<u64> {
    Ok(rpc.required_account(key, label)?.lamports)
}

fn load_stage_signers(
    selected: &SelectedInputV1,
    stage: StageV1,
    arguments: &CommandArgumentsV1,
) -> Result<(Keypair, Option<Keypair>)> {
    match stage {
        StageV1::Submit => Ok((
            load_keypair(
                arguments.submitter_keypair.as_ref(),
                "submitter",
                selected.submitter,
            )?,
            Some(load_keypair(
                arguments.update_keypair.as_ref(),
                "update",
                selected.account("update_account")?,
            )?),
        )),
        StageV1::Execute | StageV1::Reclaim => Ok((
            load_keypair(
                arguments.resolver_keypair.as_ref(),
                "resolver",
                selected.resolver,
            )?,
            None,
        )),
        StageV1::Complete => Err(Error::new("complete has no executable stage")),
    }
}

fn execute_stage(
    rpc: &mut Rpc,
    selected: &SelectedInputV1,
    prepared: &PreparedStageV1,
    payer: &Keypair,
    update: Option<&Keypair>,
) -> Result<StageReceiptV1> {
    let payer_before = lamports(rpc, payer.pubkey(), "stage payer")?;
    let refund_before = lamports(rpc, selected.refund_recipient, "refund recipient")?;
    let additional = update.into_iter().collect::<Vec<_>>();
    let evidence = rpc.send_v0_with_signers(
        &format!("flagship resolution {}", prepared.plan.stage.label()),
        &prepared.instructions,
        payer,
        &additional,
        prepared.table.observation,
        std::slice::from_ref(&prepared.table),
    )?;
    if evidence.error.is_some() {
        return Err(Error::new(format!(
            "{} reached finalized history with an error",
            prepared.plan.stage.label()
        )));
    }
    let fee = evidence
        .fee_lamports
        .ok_or_else(|| Error::new("finalized transaction omitted exact fee metadata"))?;
    let payer_after = lamports(rpc, payer.pubkey(), "stage payer poststate")?;
    let refund_after = lamports(rpc, selected.refund_recipient, "refund recipient poststate")?;
    let top_ups = prepared
        .plan
        .transfers
        .iter()
        .try_fold(0_u64, |sum, transfer| {
            sum.checked_add(transfer.lamports)
                .ok_or_else(|| Error::new("rent top-up sum overflow"))
        })?;
    let non_fee_debit = match prepared.plan.stage {
        StageV1::Submit => top_ups
            .checked_add(prepared.plan.arithmetic.update_rent_lamports)
            .and_then(|value| value.checked_add(prepared.plan.arithmetic.provider_fee_lamports))
            .ok_or_else(|| Error::new("submit arithmetic overflow"))?,
        StageV1::Execute => top_ups,
        StageV1::Reclaim => 0,
        StageV1::Complete => 0,
    };
    if prepared.plan.stage == StageV1::Reclaim {
        let refund = prepared.plan.arithmetic.expected_reclaim_total_lamports;
        if payer.pubkey() == selected.refund_recipient {
            if payer_after.checked_add(fee) != payer_before.checked_add(refund) {
                return Err(Error::new(
                    "reclaim payer/refund balance does not equal prestate + exact refund - fee",
                ));
            }
        } else if payer_after.checked_add(fee) != Some(payer_before)
            || refund_after
                != refund_before
                    .checked_add(refund)
                    .ok_or_else(|| Error::new("refund balance overflow"))?
        {
            return Err(Error::new(
                "reclaim fee or beneficiary credit differs from exact lifecycle + update rent",
            ));
        }
    } else {
        let total = non_fee_debit
            .checked_add(fee)
            .ok_or_else(|| Error::new("payer debit overflow"))?;
        if payer_after.checked_add(total) != Some(payer_before) {
            return Err(Error::new(format!(
                "{} payer delta differs from exact fee/rent/provider charge arithmetic",
                prepared.plan.stage.label()
            )));
        }
        if refund_after != refund_before {
            return Err(Error::new(
                "refund recipient changed before reclaim; unsolicited mutation refused",
            ));
        }
    }
    let post = observe(rpc, selected, prepared.plan.stage, evidence.slot)?;
    let post_stage = classify(chain_facts(selected, &post)?)?;
    let expected = match prepared.plan.stage {
        StageV1::Submit => StageV1::Execute,
        StageV1::Execute => StageV1::Reclaim,
        StageV1::Reclaim => StageV1::Complete,
        StageV1::Complete => StageV1::Complete,
    };
    if post_stage != expected {
        return Err(Error::new(format!(
            "{} finalized but detector reads {}, expected {}",
            prepared.plan.stage.label(),
            post_stage.label(),
            expected.label()
        )));
    }
    if matches!(prepared.plan.stage, StageV1::Execute | StageV1::Reclaim) {
        verify_terminal(selected, &post)?;
    }
    Ok(StageReceiptV1 {
        stage: prepared.plan.stage,
        signature: evidence.signature,
        slot: evidence.slot,
        fee_lamports: fee,
        transfer_fee_lamports: 0,
        arithmetic: prepared.plan.arithmetic.clone(),
    })
}

fn verify_terminal(selected: &SelectedInputV1, snapshot: &FinalizedSnapshotV1) -> Result<()> {
    let market_key = selected.account("market")?;
    let market = CoreState::decode(&snapshot.account(market_key, "Terminal Market")?.data)
        .map_err(|error| Error::new(format!("Terminal Market: {error:?}")))?;
    let certificate_key = selected.account("certificate")?;
    let certificate = ResolutionCertificateV2::decode(
        &snapshot
            .account(certificate_key, "terminal certificate")?
            .data,
    )
    .map_err(|error| Error::new(format!("terminal certificate: {error:?}")))?;
    let source = SourceResolutionStateV2::decode(
        &snapshot
            .account(selected.account("source_state")?, "terminal Source state")?
            .data,
    )
    .map_err(|error| Error::new(format!("terminal Source state: {error:?}")))?;
    if market.phase != CorePhase::Terminal
        || market.readiness != Readiness::Consumed
        || market.identity.market_id.to_bytes() != market_key.to_bytes()
        || market.identity.generation != selected.generation
        || market.identity.selected_release_set.to_bytes() != selected.release_set
        || market.terminal_receipt.map(|value| value.to_bytes()) != Some(certificate_key.to_bytes())
        || certificate.market != market_key.to_bytes()
        || certificate.generation != selected.generation
        || certificate.receipt_account != certificate_key.to_bytes()
        || certificate.selector != market.terminal_winner
        || !matches!(
            source.phase(),
            SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
        )
        || source.market() != market_key.to_bytes()
        || source.generation() != selected.generation
    {
        return Err(Error::new(
            "finalized Core Terminal, receipt, winner, Source, Market, generation, or release join refused",
        ));
    }
    Ok(())
}

fn recover_receipt(rpc: &mut Rpc, plan: &StagePlanV1) -> Result<StageReceiptV1> {
    let rows = rpc.call(
        "getSignaturesForAddress",
        &json!([plan.mutation_account, {
            "commitment":"finalized",
            "limit":64
        }]),
    )?;
    let rows = rows
        .as_array()
        .ok_or_else(|| Error::new("getSignaturesForAddress result was not an array"))?;
    let mut matches = Vec::new();
    for row in rows {
        let slot = row
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("signature history row omitted slot"))?;
        if slot < plan.observation_slot
            || !row.get("err").is_some_and(Value::is_null)
            || row.get("confirmationStatus").and_then(Value::as_str) != Some("finalized")
        {
            continue;
        }
        let signature = row
            .get("signature")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("signature history row omitted signature"))?;
        let transaction = rpc.call(
            "getTransaction",
            &json!([signature, {
                "commitment":"finalized",
                "encoding":"jsonParsed",
                "maxSupportedTransactionVersion":0
            }]),
        )?;
        if transaction_matches_plan(&transaction, plan)? {
            let fee = transaction
                .get("meta")
                .and_then(|meta| meta.get("fee"))
                .and_then(Value::as_u64)
                .ok_or_else(|| Error::new("matching finalized transaction omitted fee"))?;
            matches.push(StageReceiptV1 {
                stage: plan.stage,
                signature: signature.to_owned(),
                slot,
                fee_lamports: fee,
                transfer_fee_lamports: 0,
                arithmetic: plan.arithmetic.clone(),
            });
        }
    }
    match matches.len() {
        1 => matches
            .pop()
            .ok_or_else(|| Error::new("matching receipt disappeared")),
        0 => Err(Error::new(
            "chain advanced past a durable stage but no exact finalized mutation transaction was found; ambiguous submitted state",
        )),
        count => Err(Error::new(format!(
            "chain advanced past a durable stage with {count} exact finalized mutation transactions; ambiguous replay refused"
        ))),
    }
}

fn transaction_matches_plan(transaction: &Value, plan: &StagePlanV1) -> Result<bool> {
    if !transaction
        .get("meta")
        .and_then(|meta| meta.get("err"))
        .is_some_and(Value::is_null)
    {
        return Ok(false);
    }
    let instructions = transaction
        .get("transaction")
        .and_then(|transaction| transaction.get("message"))
        .and_then(|message| message.get("instructions"))
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("getTransaction omitted parsed instructions"))?;
    let expected_accounts = plan
        .action
        .accounts
        .iter()
        .map(|account| account.pubkey.as_str())
        .collect::<Vec<_>>();
    let expected_data = base58_encode(
        &BASE64
            .decode(&plan.action.data_base64)
            .map_err(|error| Error::new(format!("checkpoint action data: {error}")))?,
    )?;
    let mut exact = 0_usize;
    for instruction in instructions {
        let Some(accounts) = instruction.get("accounts").and_then(Value::as_array) else {
            continue;
        };
        let actual_accounts = accounts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        if instruction.get("programId").and_then(Value::as_str)
            == Some(plan.action.program_id.as_str())
            && instruction.get("data").and_then(Value::as_str) == Some(expected_data.as_str())
            && actual_accounts == expected_accounts
        {
            exact = exact
                .checked_add(1)
                .ok_or_else(|| Error::new("instruction match count overflow"))?;
        }
    }
    Ok(exact == 1)
}

fn base58_encode(bytes: &[u8]) -> Result<String> {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    if bytes.is_empty() {
        return Ok(String::new());
    }
    let zeroes = bytes.iter().take_while(|byte| **byte == 0).count();
    let capacity = bytes
        .len()
        .checked_mul(138)
        .and_then(|value| value.checked_div(100))
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| Error::new("base58 capacity overflow"))?;
    let mut digits = vec![0_u8; capacity];
    let mut length = 0_usize;
    for &byte in bytes {
        let mut carry = u32::from(byte);
        for digit in digits.iter_mut().take(length).rev() {
            let value = u32::from(*digit).saturating_mul(256).saturating_add(carry);
            *digit = u8::try_from(value % 58)
                .map_err(|_| Error::new("base58 digit conversion refused"))?;
            carry = value / 58;
        }
        while carry != 0 {
            if let Some(digit) = digits.get_mut(length) {
                *digit = u8::try_from(carry % 58)
                    .map_err(|_| Error::new("base58 carry conversion refused"))?;
            }
            length = length
                .checked_add(1)
                .ok_or_else(|| Error::new("base58 length overflow"))?;
            carry /= 58;
        }
    }
    let mut output = String::with_capacity(
        zeroes
            .checked_add(length)
            .ok_or_else(|| Error::new("base58 output length overflow"))?,
    );
    output.extend(std::iter::repeat_n('1', zeroes));
    for digit in digits.iter().take(length).rev() {
        output.push(char::from(ALPHABET[usize::from(*digit)]));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(
        market_phase: CorePhase,
        source_phase: SourceResolutionPhaseV1,
        lifecycle: SlotKindV1,
        update: SlotKindV1,
        certificate: SlotKindV1,
    ) -> ChainFactsV1 {
        ChainFactsV1 {
            market_phase,
            market_readiness: Readiness::Consumed,
            source_phase,
            lifecycle,
            update,
            certificate,
        }
    }

    #[test]
    fn fake_rpc_canonical_stage_ladder_is_exhaustive() {
        use SlotKindV1::{Consumed, Submitted, Vacant};
        assert_eq!(
            classify(facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Primary,
                Vacant,
                Vacant,
                Vacant,
            ))
            .expect("submit"),
            StageV1::Submit
        );
        assert_eq!(
            classify(facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Primary,
                Submitted,
                Submitted,
                Vacant,
            ))
            .expect("execute"),
            StageV1::Execute
        );
        assert!(
            classify(facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Resolved,
                Consumed,
                Submitted,
                Submitted,
            ))
            .is_err(),
            "provider execution has no finalized pre-admission intermediate"
        );
        assert_eq!(
            classify(facts(
                CorePhase::Terminal,
                SourceResolutionPhaseV1::Resolved,
                Consumed,
                Submitted,
                Submitted,
            ))
            .expect("reclaim"),
            StageV1::Reclaim
        );
        assert_eq!(
            classify(facts(
                CorePhase::Terminal,
                SourceResolutionPhaseV1::Resolved,
                Vacant,
                Vacant,
                Submitted,
            ))
            .expect("complete"),
            StageV1::Complete
        );
    }

    #[test]
    fn fake_rpc_partial_and_ambiguous_states_refuse() {
        use SlotKindV1::{Consumed, Other, Submitted, Vacant};
        for hostile in [
            facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Primary,
                Submitted,
                Vacant,
                Vacant,
            ),
            facts(
                CorePhase::Open,
                SourceResolutionPhaseV1::Resolved,
                Consumed,
                Submitted,
                Vacant,
            ),
            facts(
                CorePhase::Terminal,
                SourceResolutionPhaseV1::Resolved,
                Other,
                Submitted,
                Submitted,
            ),
            facts(
                CorePhase::Terminal,
                SourceResolutionPhaseV1::Resolved,
                Vacant,
                Submitted,
                Submitted,
            ),
        ] {
            assert!(classify(hostile).is_err());
        }
    }

    #[test]
    fn cli_refuses_keys_during_read_only_preflight_and_duplicate_flags() {
        assert!(
            CommandArgumentsV1::parse(vec!["--submitter-keypair".into(), "/tmp/key.json".into(),])
                .is_err()
        );
        assert!(CommandArgumentsV1::parse(vec!["--execute".into(), "--execute".into(),]).is_err());
    }

    #[test]
    fn fake_rpc_stale_wrong_feed_and_wide_confidence_refuse_before_execute() {
        let source = dclutch_source_contract::ContentId::new([1; 32]).expect("source identity");
        let schedule = dclutch_source_contract::ContentId::new([2; 32]).expect("schedule identity");
        let window = WindowSpecV1::new(
            source,
            dclutch_source_contract::WindowKind::Terminal,
            90,
            110,
            20,
            5,
            schedule,
        )
        .expect("window");
        let adapter = PythAdapterConfigV1::new([3; 32], -8, 100).expect("adapter");
        assert!(
            validate_observation_fields(100, [3; 32], 10_000, 50, -8, 105, window, adapter).is_ok()
        );
        assert!(
            validate_observation_fields(80, [3; 32], 10_000, 50, -8, 105, window, adapter).is_err()
        );
        assert!(
            validate_observation_fields(100, [4; 32], 10_000, 50, -8, 105, window, adapter)
                .is_err()
        );
        assert!(
            validate_observation_fields(100, [3; 32], 10_000, 101, -8, 105, window, adapter)
                .is_err()
        );
    }

    #[test]
    fn durable_plan_refuses_action_or_compute_policy_substitution() {
        let payer = Pubkey::new_from_array([5; 32]);
        let instruction = Instruction {
            program_id: Pubkey::new_from_array([7; 32]),
            accounts: vec![AccountMeta::new_readonly(payer, true)],
            data: vec![1, 2, 3],
        };
        let transaction_instructions =
            bounded_instructions(std::slice::from_ref(&instruction), None)
                .expect("bounded transaction")
                .iter()
                .map(InstructionPlanV1::from_instruction)
                .collect::<Result<Vec<_>>>()
                .expect("instruction plans");
        let mut plan = StagePlanV1 {
            stage: StageV1::Execute,
            observation_slot: 1,
            observation_unix_timestamp: 2,
            action: InstructionPlanV1::from_instruction(&instruction).expect("action"),
            transaction_instructions,
            lookup_table: Pubkey::new_from_array([8; 32]).to_string(),
            lookup_table_account_sha256: "11".repeat(32),
            compiled_wire_bytes: 300,
            compiled_loaded_addresses: 1,
            required_signers: vec![payer.to_string()],
            transfers: vec![],
            arithmetic: ArithmeticPlanV1::default(),
            mutation_account: Pubkey::new_from_array([9; 32]).to_string(),
            submission_armed: false,
        };
        assert!(plan.validate().is_ok());
        assert_eq!(
            resume_action(StageV1::Execute, &plan).expect("unsigned reprepare"),
            ResumeActionV1::ReprepareUnsigned
        );
        plan.submission_armed = true;
        assert!(resume_action(StageV1::Execute, &plan).is_err());
        assert_eq!(
            resume_action(StageV1::Reclaim, &plan).expect("finalized advance"),
            ResumeActionV1::RecoverFinalized
        );
        assert!(resume_action(StageV1::Submit, &plan).is_err());
        plan.submission_armed = false;
        plan.action.data_base64 = BASE64.encode([9, 9, 9]);
        assert!(plan.validate().is_err());
    }

    #[test]
    fn recovered_transaction_requires_exact_program_accounts_and_data() {
        let instruction = Instruction {
            program_id: Pubkey::new_from_array([7; 32]),
            accounts: vec![AccountMeta::new(Pubkey::new_from_array([8; 32]), true)],
            data: vec![0, 1, 2, 255],
        };
        let action = InstructionPlanV1::from_instruction(&instruction).expect("instruction plan");
        let plan = StagePlanV1 {
            stage: StageV1::Submit,
            observation_slot: 9,
            observation_unix_timestamp: 10,
            action,
            transaction_instructions: vec![],
            lookup_table: Pubkey::new_from_array([9; 32]).to_string(),
            lookup_table_account_sha256: "00".repeat(32),
            compiled_wire_bytes: 400,
            compiled_loaded_addresses: 1,
            required_signers: vec![],
            transfers: vec![],
            arithmetic: ArithmeticPlanV1::default(),
            mutation_account: Pubkey::new_from_array([10; 32]).to_string(),
            submission_armed: false,
        };
        let exact = json!({
            "meta":{"err":null,"fee":5000},
            "transaction":{"message":{"instructions":[{
                "programId":instruction.program_id.to_string(),
                "accounts":[instruction.accounts[0].pubkey.to_string()],
                "data":base58_encode(&instruction.data).expect("base58")
            }]}}
        });
        assert!(transaction_matches_plan(&exact, &plan).expect("match"));
        let mut substituted = exact;
        substituted["transaction"]["message"]["instructions"][0]["accounts"][0] =
            Value::String(Pubkey::new_from_array([11; 32]).to_string());
        assert!(!transaction_matches_plan(&substituted, &plan).expect("refuse"));
    }

    #[test]
    fn base58_matches_known_system_program_spelling() {
        assert_eq!(
            base58_encode(&[0; 32]).expect("base58"),
            "11111111111111111111111111111111"
        );
        assert_eq!(base58_encode(&[0, 1]).expect("base58"), "12");
    }
}
