//! Durable exterior state for the four packet AggregateRetirement lifecycle.
//!
//! This module owns no RPC transport and no protocol codec. It binds the exact
//! operator-produced instructions, classifies the live onchain checkpoint, and
//! makes every crash boundary explicit. The executable exterior supplies chain
//! observations and signed packets; PRIVATE consumes the serialized campaign,
//! journals, and terminal conservation receipt.

use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_market_core_codec::{
    AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1, AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1,
    AGGREGATE_RETIREMENT_FINISH_MAGIC_V1, AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1,
    AggregateRetirementCheckpointV1, AggregateRetirementPhaseV1,
    AggregateRetirementSuffixRequestV1,
};
use dclutch_market_retirement_v1_operator::{
    CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1, CHECKPOINT_RETIREMENT_FINISH_BYTES_V1,
    CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1, CheckpointMarketRetirementReportV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_compute_budget_interface::ID as COMPUTE_BUDGET_PROGRAM_ID;
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk::{message::VersionedMessage, transaction::VersionedTransaction};
use solana_sdk_ids::system_program;

use crate::{Error, Result, rpc::SignedVersionedPacketV1};

pub(crate) const AGGREGATE_RETIREMENT_CAMPAIGN_SCHEMA_V1: &str =
    "dclutch-owned-loopback-aggregate-retirement-campaign-v1";
pub(crate) const AGGREGATE_RETIREMENT_JOURNAL_SCHEMA_V1: &str =
    "dclutch-owned-loopback-aggregate-retirement-journal-v1";
pub(crate) const AGGREGATE_RETIREMENT_COMPLETION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-aggregate-retirement-completion-v1";
pub(crate) const EXACT_RETIREMENT_PROTOCOL_AND_PAYER_KEYS_V1: usize = 36;
pub(crate) const EXACT_RETIREMENT_RESOLVED_KEYS_WITH_COMPUTE_BUDGET_V1: usize = 37;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AggregateRetirementOperationV1 {
    Prepare,
    CloseVault,
    CloseReplay,
    Finish,
}

impl AggregateRetirementOperationV1 {
    pub(crate) const ORDERED: [Self; 4] = [
        Self::Prepare,
        Self::CloseVault,
        Self::CloseReplay,
        Self::Finish,
    ];

    pub(crate) const fn ordinal(self) -> usize {
        match self {
            Self::Prepare => 0,
            Self::CloseVault => 1,
            Self::CloseReplay => 2,
            Self::Finish => 3,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Prepare => "aggregate-retirement-prepare",
            Self::CloseVault => "aggregate-retirement-close-vault",
            Self::CloseReplay => "aggregate-retirement-close-replay",
            Self::Finish => "aggregate-retirement-finish",
        }
    }

    pub(crate) const fn expected_wire_bytes(self) -> usize {
        match self {
            Self::Prepare => 1_135,
            Self::CloseVault | Self::CloseReplay => 1_191,
            Self::Finish => 1_071,
        }
    }

    pub(crate) const fn expected_data_bytes(self) -> usize {
        match self {
            Self::Prepare => CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1,
            Self::CloseVault | Self::CloseReplay => CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1,
            Self::Finish => CHECKPOINT_RETIREMENT_FINISH_BYTES_V1,
        }
    }

    pub(crate) const fn predecessor(self) -> AggregateRetirementChainPhaseV1 {
        match self {
            Self::Prepare => AggregateRetirementChainPhaseV1::Ready,
            Self::CloseVault => AggregateRetirementChainPhaseV1::ClaimsClosed,
            Self::CloseReplay => AggregateRetirementChainPhaseV1::HoardVaultClosed,
            Self::Finish => AggregateRetirementChainPhaseV1::CustodyReplayClosed,
        }
    }

    pub(crate) const fn successor(self) -> AggregateRetirementChainPhaseV1 {
        match self {
            Self::Prepare => AggregateRetirementChainPhaseV1::ClaimsClosed,
            Self::CloseVault => AggregateRetirementChainPhaseV1::HoardVaultClosed,
            Self::CloseReplay => AggregateRetirementChainPhaseV1::CustodyReplayClosed,
            Self::Finish => AggregateRetirementChainPhaseV1::Complete,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AggregateRetirementChainPhaseV1 {
    Ready,
    ClaimsClosed,
    HoardVaultClosed,
    CustodyReplayClosed,
    Complete,
}

impl AggregateRetirementChainPhaseV1 {
    const fn ordinal(self) -> usize {
        match self {
            Self::Ready => 0,
            Self::ClaimsClosed => 1,
            Self::HoardVaultClosed => 2,
            Self::CustodyReplayClosed => 3,
            Self::Complete => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AggregateRetirementJournalPhaseV1 {
    Planned,
    Prepared,
    Dispatching,
    Submitted,
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AggregateRetirementRecoveryV1 {
    PersistPrepared,
    SignOnceAndPersistDispatching,
    PollThenResendIdentical,
    PollOnly,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AggregateRetirementRouteV1 {
    Plan(AggregateRetirementOperationV1),
    Recover(
        AggregateRetirementOperationV1,
        AggregateRetirementRecoveryV1,
    ),
    Complete,
}

#[derive(Clone, Debug)]
pub(crate) struct AggregateRetirementInitialAccountV1 {
    pub(crate) key: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct AggregateRetirementCampaignInputV1 {
    pub(crate) genesis_hash: String,
    pub(crate) rpc_url: String,
    pub(crate) plan_sha256: String,
    pub(crate) evidence_sha256: String,
    pub(crate) payer: Pubkey,
    pub(crate) lookup_table: Pubkey,
    pub(crate) lookup_table_sha256: String,
    pub(crate) core_program: Pubkey,
    pub(crate) claims_program: Pubkey,
    pub(crate) market: AggregateRetirementInitialAccountV1,
    pub(crate) rent_credit: AggregateRetirementInitialAccountV1,
    pub(crate) checkpoint: AggregateRetirementInitialAccountV1,
    pub(crate) custody_replay: AggregateRetirementInitialAccountV1,
    pub(crate) hoard_vault: AggregateRetirementInitialAccountV1,
    pub(crate) source_receipt: AggregateRetirementInitialAccountV1,
    pub(crate) refund_wallet: AggregateRetirementInitialAccountV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableRetirementAccountV1 {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data_len: usize,
    pub(crate) data_sha256: String,
    pub(crate) account_sha256: String,
}

impl DurableRetirementAccountV1 {
    fn from_initial(value: &AggregateRetirementInitialAccountV1) -> Self {
        let mut result = Self {
            address: value.key.to_string(),
            owner: value.owner.to_string(),
            lamports: value.lamports,
            executable: value.executable,
            data_len: value.data.len(),
            data_sha256: sha256_hex(&value.data),
            account_sha256: String::new(),
        };
        result.account_sha256 = account_digest_v1(&result);
        result
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableRetirementInstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableRetirementOperationV1 {
    pub(crate) operation: AggregateRetirementOperationV1,
    pub(crate) program_id: String,
    accounts: Vec<DurableRetirementInstructionAccountV1>,
    data_base64: String,
    data_sha256: String,
    pub(crate) expected_wire_bytes: usize,
    pub(crate) exact_protocol_and_payer_keys: usize,
}

impl DurableRetirementOperationV1 {
    pub(crate) fn instruction(&self) -> Result<Instruction> {
        let program_id = self
            .program_id
            .parse::<Pubkey>()
            .map_err(|error| Error::new(format!("retirement program: {error}")))?;
        let accounts = self
            .accounts
            .iter()
            .map(|meta| {
                let pubkey = meta
                    .address
                    .parse::<Pubkey>()
                    .map_err(|error| Error::new(format!("retirement account: {error}")))?;
                Ok(if meta.writable {
                    solana_program::instruction::AccountMeta::new(pubkey, meta.signer)
                } else {
                    solana_program::instruction::AccountMeta::new_readonly(pubkey, meta.signer)
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Instruction {
            program_id,
            accounts,
            data: decode_base64(&self.data_base64, "retirement instruction")?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AggregateRetirementClassifiedLamportsV1 {
    pub(crate) market: u64,
    pub(crate) rent_credit: u64,
    pub(crate) claims_refund: u64,
    pub(crate) custody_replay: u64,
    pub(crate) hoard_vault: u64,
    pub(crate) expected_refund_delta: u64,
    pub(crate) refund_wallet_before: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AggregateRetirementCampaignV1 {
    pub(crate) schema: String,
    pub(crate) cluster: String,
    pub(crate) genesis_hash: String,
    pub(crate) rpc_url: String,
    pub(crate) plan_sha256: String,
    pub(crate) evidence_sha256: String,
    pub(crate) payer: String,
    pub(crate) lookup_table: String,
    pub(crate) lookup_table_sha256: String,
    pub(crate) core_program: String,
    pub(crate) claims_program: String,
    pub(crate) market: DurableRetirementAccountV1,
    pub(crate) rent_credit: DurableRetirementAccountV1,
    pub(crate) checkpoint: DurableRetirementAccountV1,
    pub(crate) custody_replay: DurableRetirementAccountV1,
    pub(crate) hoard_vault: DurableRetirementAccountV1,
    pub(crate) source_receipt: DurableRetirementAccountV1,
    pub(crate) refund_wallet: DurableRetirementAccountV1,
    pub(crate) classified_lamports: AggregateRetirementClassifiedLamportsV1,
    pub(crate) operations: Vec<DurableRetirementOperationV1>,
    pub(crate) campaign_sha256: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AggregateRetirementChainAccountV1 {
    pub(crate) key: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AggregateRetirementChainProjectionV1 {
    pub(crate) phase: AggregateRetirementChainPhaseV1,
    pub(crate) finalized_slot: u64,
    pub(crate) checkpoint_history_sha256: Option<String>,
    pub(crate) accounts: BTreeMap<String, Option<DurableRetirementAccountV1>>,
    pub(crate) state_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AggregateRetirementPacketBindingV1 {
    pub(crate) signed: SignedVersionedPacketV1,
    pub(crate) message_sha256: String,
    pub(crate) resolved_account_keys: Vec<String>,
    pub(crate) resolved_account_keys_sha256: String,
    pub(crate) exact_key_set_sha256: String,
    pub(crate) lookup_table_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AggregateRetirementFinalizationV1 {
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) packet_sha256: String,
    pub(crate) fee_lamports: u64,
    pub(crate) compute_units_consumed: u64,
    pub(crate) poststate_sha256: String,
    pub(crate) checkpoint_history_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AggregateRetirementJournalV1 {
    pub(crate) schema: String,
    pub(crate) campaign_sha256: String,
    pub(crate) operation: AggregateRetirementOperationV1,
    pub(crate) phase: AggregateRetirementJournalPhaseV1,
    pub(crate) predecessor: AggregateRetirementChainPhaseV1,
    pub(crate) successor: AggregateRetirementChainPhaseV1,
    pub(crate) planned_prestate_sha256: String,
    pub(crate) intent_sha256: String,
    pub(crate) packet: Option<AggregateRetirementPacketBindingV1>,
    pub(crate) finalization: Option<AggregateRetirementFinalizationV1>,
    pub(crate) state_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AggregateRetirementCompletionJournalV1 {
    pub(crate) operation: AggregateRetirementOperationV1,
    pub(crate) journal_sha256: String,
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) fee_lamports: u64,
    pub(crate) compute_units_consumed: u64,
    pub(crate) packet_sha256: String,
    pub(crate) poststate_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AggregateRetirementConservationReceiptV1 {
    pub(crate) schema: String,
    pub(crate) status: String,
    pub(crate) campaign_sha256: String,
    pub(crate) market: String,
    pub(crate) checkpoint: String,
    pub(crate) rent_credit: String,
    pub(crate) refund_wallet: String,
    pub(crate) payer: String,
    pub(crate) classified_lamports: AggregateRetirementClassifiedLamportsV1,
    pub(crate) total_transaction_fees_lamports: u64,
    pub(crate) terminal_refund_wallet_lamports: u64,
    pub(crate) journals: Vec<AggregateRetirementCompletionJournalV1>,
    pub(crate) receipt_sha256: String,
}

pub(crate) fn build_aggregate_retirement_campaign_v1(
    input: AggregateRetirementCampaignInputV1,
    report: &CheckpointMarketRetirementReportV1,
) -> Result<AggregateRetirementCampaignV1> {
    for value in [
        &input.plan_sha256,
        &input.evidence_sha256,
        &input.lookup_table_sha256,
    ] {
        require_sha256(value, "campaign input digest")?;
    }
    if input.genesis_hash.is_empty()
        || input.rpc_url.is_empty()
        || input.payer == Pubkey::default()
        || input.lookup_table == Pubkey::default()
        || input.core_program == Pubkey::default()
        || input.claims_program == Pubkey::default()
        || input.refund_wallet.owner != system_program::ID
        || input.refund_wallet.executable
        || !input.refund_wallet.data.is_empty()
    {
        return Err(refusal(
            "campaign binding or immutable refund wallet was invalid",
        ));
    }
    let expected_refund_delta = input
        .market
        .lamports
        .checked_add(input.rent_credit.lamports)
        .and_then(|value| value.checked_add(input.checkpoint.lamports))
        .and_then(|value| value.checked_add(input.custody_replay.lamports))
        .and_then(|value| value.checked_add(input.hoard_vault.lamports))
        .ok_or_else(|| refusal("classified retirement lamports overflowed"))?;
    if expected_refund_delta != report.expected_refund_delta {
        return Err(refusal(
            "operator refund delta differed from exact classified account lamports",
        ));
    }
    let instructions = [
        (AggregateRetirementOperationV1::Prepare, &report.prepare),
        (
            AggregateRetirementOperationV1::CloseVault,
            &report.close_vault,
        ),
        (
            AggregateRetirementOperationV1::CloseReplay,
            &report.close_replay,
        ),
        (AggregateRetirementOperationV1::Finish, &report.finish),
    ];
    let operations = instructions
        .into_iter()
        .map(|(operation, instruction)| durable_operation_v1(operation, input.payer, instruction))
        .collect::<Result<Vec<_>>>()?;
    if operations
        .windows(2)
        .any(|pair| pair[0].accounts != pair[1].accounts)
    {
        return Err(refusal(
            "checkpoint retirement operations changed their exact account frame",
        ));
    }
    let classified_lamports = AggregateRetirementClassifiedLamportsV1 {
        market: input.market.lamports,
        rent_credit: input.rent_credit.lamports,
        claims_refund: input.checkpoint.lamports,
        custody_replay: input.custody_replay.lamports,
        hoard_vault: input.hoard_vault.lamports,
        expected_refund_delta,
        refund_wallet_before: input.refund_wallet.lamports,
    };
    let mut campaign = AggregateRetirementCampaignV1 {
        schema: AGGREGATE_RETIREMENT_CAMPAIGN_SCHEMA_V1.into(),
        cluster: "owned-loopback".into(),
        genesis_hash: input.genesis_hash,
        rpc_url: input.rpc_url,
        plan_sha256: input.plan_sha256,
        evidence_sha256: input.evidence_sha256,
        payer: input.payer.to_string(),
        lookup_table: input.lookup_table.to_string(),
        lookup_table_sha256: input.lookup_table_sha256,
        core_program: input.core_program.to_string(),
        claims_program: input.claims_program.to_string(),
        market: DurableRetirementAccountV1::from_initial(&input.market),
        rent_credit: DurableRetirementAccountV1::from_initial(&input.rent_credit),
        checkpoint: DurableRetirementAccountV1::from_initial(&input.checkpoint),
        custody_replay: DurableRetirementAccountV1::from_initial(&input.custody_replay),
        hoard_vault: DurableRetirementAccountV1::from_initial(&input.hoard_vault),
        source_receipt: DurableRetirementAccountV1::from_initial(&input.source_receipt),
        refund_wallet: DurableRetirementAccountV1::from_initial(&input.refund_wallet),
        classified_lamports,
        operations,
        campaign_sha256: String::new(),
    };
    campaign.campaign_sha256 = campaign_digest_v1(&campaign)?;
    authenticate_aggregate_retirement_campaign_v1(&campaign)?;
    Ok(campaign)
}

pub(crate) fn authenticate_aggregate_retirement_campaign_v1(
    campaign: &AggregateRetirementCampaignV1,
) -> Result<()> {
    if campaign.schema != AGGREGATE_RETIREMENT_CAMPAIGN_SCHEMA_V1
        || campaign.cluster != "owned-loopback"
        || campaign.genesis_hash.is_empty()
        || campaign.rpc_url.is_empty()
        || campaign.campaign_sha256 != campaign_digest_v1(campaign)?
        || campaign.operations.len() != 4
        || campaign
            .operations
            .iter()
            .map(|operation| operation.operation)
            .ne(AggregateRetirementOperationV1::ORDERED)
    {
        return Err(refusal(
            "retirement campaign identity, digest, or operation order changed",
        ));
    }
    for value in [
        &campaign.plan_sha256,
        &campaign.evidence_sha256,
        &campaign.lookup_table_sha256,
        &campaign.campaign_sha256,
    ] {
        require_sha256(value, "campaign digest")?;
    }
    for account in campaign_accounts(campaign) {
        authenticate_durable_account(account)?;
    }
    let payer = parse_pubkey(&campaign.payer, "campaign payer")?;
    let core = parse_pubkey(&campaign.core_program, "campaign Core")?;
    let claims = parse_pubkey(&campaign.claims_program, "campaign Claims")?;
    if core == claims || payer == Pubkey::default() {
        return Err(refusal("campaign program or payer identities aliased"));
    }
    let sum = campaign
        .classified_lamports
        .market
        .checked_add(campaign.classified_lamports.rent_credit)
        .and_then(|value| value.checked_add(campaign.classified_lamports.claims_refund))
        .and_then(|value| value.checked_add(campaign.classified_lamports.custody_replay))
        .and_then(|value| value.checked_add(campaign.classified_lamports.hoard_vault))
        .ok_or_else(|| refusal("campaign classified lamports overflowed"))?;
    if sum != campaign.classified_lamports.expected_refund_delta
        || campaign.classified_lamports.refund_wallet_before != campaign.refund_wallet.lamports
    {
        return Err(refusal("campaign conservation classification changed"));
    }
    for operation in &campaign.operations {
        authenticate_operation_v1(operation, payer, core)?;
    }
    if campaign
        .operations
        .windows(2)
        .any(|pair| pair[0].accounts != pair[1].accounts)
    {
        return Err(refusal("campaign operation account frames diverged"));
    }
    Ok(())
}

pub(crate) fn classify_aggregate_retirement_chain_v1(
    campaign: &AggregateRetirementCampaignV1,
    finalized_slot: u64,
    accounts: &BTreeMap<Pubkey, Option<AggregateRetirementChainAccountV1>>,
    finalized_fees_lamports: u64,
    allow_one_unreconciled_fee: bool,
) -> Result<AggregateRetirementChainProjectionV1> {
    authenticate_aggregate_retirement_campaign_v1(campaign)?;
    if finalized_slot == 0 {
        return Err(refusal("retirement chain projection used slot zero"));
    }
    let expected_keys = campaign_account_keys(campaign)?;
    if accounts.keys().copied().collect::<BTreeSet<_>>() != expected_keys {
        return Err(refusal(
            "retirement chain observation changed its exact account set",
        ));
    }
    let get = |account: &DurableRetirementAccountV1| -> Result<_> {
        let key = parse_pubkey(&account.address, "campaign account")?;
        accounts
            .get(&key)
            .ok_or_else(|| refusal("retirement account disappeared from exact observation"))
    };
    let source = get(&campaign.source_receipt)?
        .as_ref()
        .ok_or_else(|| refusal("immutable Resolution closure receipt was missing"))?;
    authenticate_live_initial_account(source, &campaign.source_receipt, true)?;
    let wallet = get(&campaign.refund_wallet)?
        .as_ref()
        .ok_or_else(|| refusal("immutable retirement refund wallet was missing"))?;
    if wallet.owner != system_program::ID || wallet.executable || !wallet.data.is_empty() {
        return Err(refusal(
            "retirement refund wallet changed its immutable shape",
        ));
    }
    let market = get(&campaign.market)?;
    let rent_credit = get(&campaign.rent_credit)?;
    let checkpoint = get(&campaign.checkpoint)?;
    let replay = get(&campaign.custody_replay)?;
    let vault = get(&campaign.hoard_vault)?;
    let core = parse_pubkey(&campaign.core_program, "campaign Core")?;
    let claims = parse_pubkey(&campaign.claims_program, "campaign Claims")?;
    let mut checkpoint_history_sha256 = None;
    let phase = match (market, rent_credit, checkpoint, replay, vault) {
        (Some(market), Some(rent), Some(checkpoint), Some(replay), Some(vault))
            if checkpoint.owner == claims =>
        {
            authenticate_live_initial_account(market, &campaign.market, true)?;
            authenticate_live_initial_account(rent, &campaign.rent_credit, true)?;
            authenticate_live_initial_account(checkpoint, &campaign.checkpoint, true)?;
            authenticate_live_initial_account(replay, &campaign.custody_replay, true)?;
            authenticate_live_initial_account(vault, &campaign.hoard_vault, true)?;
            AggregateRetirementChainPhaseV1::Ready
        }
        (Some(market), Some(rent), Some(checkpoint), replay, vault) if checkpoint.owner == core => {
            authenticate_live_initial_account(market, &campaign.market, true)?;
            let decoded = AggregateRetirementCheckpointV1::decode(&checkpoint.data)
                .map_err(|_| refusal("Core-owned retirement checkpoint was noncanonical"))?;
            if checkpoint.lamports != campaign.classified_lamports.claims_refund {
                return Err(refusal("checkpoint changed the retained Claims refund"));
            }
            let bundle_digest = campaign_bundle_digest_v1(campaign)?;
            if decoded.bundle_digest() != bundle_digest {
                return Err(refusal(
                    "checkpoint changed the immutable retirement bundle",
                ));
            }
            checkpoint_history_sha256 = Some(sha256_hex(&decoded.history_digest()));
            match decoded.phase() {
                AggregateRetirementPhaseV1::ClaimsClosed => {
                    let replay = replay.as_ref().ok_or_else(|| {
                        refusal("ClaimsClosed checkpoint omitted live Custody replay")
                    })?;
                    let vault = vault.as_ref().ok_or_else(|| {
                        refusal("ClaimsClosed checkpoint omitted live Hoard vault")
                    })?;
                    authenticate_live_initial_account(replay, &campaign.custody_replay, true)?;
                    authenticate_live_initial_account(vault, &campaign.hoard_vault, true)?;
                    require_rent_account(
                        rent,
                        &campaign.rent_credit,
                        campaign.classified_lamports.rent_credit,
                    )?;
                    AggregateRetirementChainPhaseV1::ClaimsClosed
                }
                AggregateRetirementPhaseV1::HoardVaultClosed => {
                    if vault.is_some() {
                        return Err(refusal("HoardVaultClosed checkpoint left the vault live"));
                    }
                    let replay = replay.as_ref().ok_or_else(|| {
                        refusal("HoardVaultClosed checkpoint omitted live Custody replay")
                    })?;
                    authenticate_live_initial_account(replay, &campaign.custody_replay, true)?;
                    require_rent_account(
                        rent,
                        &campaign.rent_credit,
                        campaign
                            .classified_lamports
                            .rent_credit
                            .checked_add(campaign.classified_lamports.hoard_vault)
                            .ok_or_else(|| refusal("vault refund overflowed"))?,
                    )?;
                    AggregateRetirementChainPhaseV1::HoardVaultClosed
                }
                AggregateRetirementPhaseV1::CustodyReplayClosed => {
                    if vault.is_some() || replay.is_some() {
                        return Err(refusal(
                            "CustodyReplayClosed checkpoint left a Custody account live",
                        ));
                    }
                    require_rent_account(
                        rent,
                        &campaign.rent_credit,
                        campaign
                            .classified_lamports
                            .rent_credit
                            .checked_add(campaign.classified_lamports.hoard_vault)
                            .and_then(|value| {
                                value.checked_add(campaign.classified_lamports.custody_replay)
                            })
                            .ok_or_else(|| refusal("Custody refund overflowed"))?,
                    )?;
                    AggregateRetirementChainPhaseV1::CustodyReplayClosed
                }
            }
        }
        (None, None, None, None, None) => AggregateRetirementChainPhaseV1::Complete,
        _ => {
            return Err(refusal(
                "retirement accounts did not form one exhaustive ordered chain phase",
            ));
        }
    };
    let expected_wallet = campaign
        .classified_lamports
        .refund_wallet_before
        .checked_add(if phase == AggregateRetirementChainPhaseV1::Complete {
            campaign.classified_lamports.expected_refund_delta
        } else {
            0
        })
        .and_then(|value| {
            if campaign.payer == campaign.refund_wallet.address {
                value.checked_sub(finalized_fees_lamports)
            } else {
                Some(value)
            }
        })
        .ok_or_else(|| refusal("retirement refund-wallet fee arithmetic underflowed"))?;
    if (!allow_one_unreconciled_fee && wallet.lamports != expected_wallet)
        || (allow_one_unreconciled_fee && wallet.lamports > expected_wallet)
    {
        return Err(refusal(
            "retirement refund wallet differed beyond exact finalized/unreconciled fee accounting",
        ));
    }
    let durable_accounts = accounts
        .iter()
        .map(|(key, value)| {
            Ok((
                key.to_string(),
                value.as_ref().map(durable_chain_account_v1),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut projection = AggregateRetirementChainProjectionV1 {
        phase,
        finalized_slot,
        checkpoint_history_sha256,
        accounts: durable_accounts,
        state_sha256: String::new(),
    };
    projection.state_sha256 = projection_digest_v1(&projection)?;
    authenticate_chain_projection_v1(&projection)?;
    Ok(projection)
}

pub(crate) fn plan_aggregate_retirement_journal_v1(
    campaign: &AggregateRetirementCampaignV1,
    operation: AggregateRetirementOperationV1,
    projection: &AggregateRetirementChainProjectionV1,
) -> Result<AggregateRetirementJournalV1> {
    authenticate_aggregate_retirement_campaign_v1(campaign)?;
    authenticate_chain_projection_v1(projection)?;
    if projection.phase != operation.predecessor() {
        return Err(refusal(
            "retirement journal predecessor differed from chain",
        ));
    }
    let mut journal = AggregateRetirementJournalV1 {
        schema: AGGREGATE_RETIREMENT_JOURNAL_SCHEMA_V1.into(),
        campaign_sha256: campaign.campaign_sha256.clone(),
        operation,
        phase: AggregateRetirementJournalPhaseV1::Planned,
        predecessor: operation.predecessor(),
        successor: operation.successor(),
        planned_prestate_sha256: projection.state_sha256.clone(),
        intent_sha256: String::new(),
        packet: None,
        finalization: None,
        state_sha256: String::new(),
    };
    journal.intent_sha256 = journal_intent_digest_v1(&journal)?;
    refresh_journal_digest_v1(&mut journal)?;
    authenticate_aggregate_retirement_journal_v1(campaign, &journal)?;
    Ok(journal)
}

pub(crate) fn prepare_aggregate_retirement_journal_v1(
    campaign: &AggregateRetirementCampaignV1,
    current: &AggregateRetirementJournalV1,
    projection: &AggregateRetirementChainProjectionV1,
) -> Result<AggregateRetirementJournalV1> {
    authenticate_aggregate_retirement_journal_v1(campaign, current)?;
    authenticate_chain_projection_v1(projection)?;
    if current.phase != AggregateRetirementJournalPhaseV1::Planned
        || projection.phase != current.predecessor
        || projection.state_sha256 != current.planned_prestate_sha256
    {
        return Err(refusal(
            "retirement prepare changed its exact planned prestate",
        ));
    }
    transition_journal_v1(
        current,
        AggregateRetirementJournalPhaseV1::Prepared,
        None,
        None,
    )
    .and_then(|next| authenticate_transition_v1(campaign, current, &next).map(|_| next))
}

pub(crate) fn dispatch_aggregate_retirement_journal_v1(
    campaign: &AggregateRetirementCampaignV1,
    current: &AggregateRetirementJournalV1,
    packet: AggregateRetirementPacketBindingV1,
) -> Result<AggregateRetirementJournalV1> {
    authenticate_aggregate_retirement_journal_v1(campaign, current)?;
    if current.phase != AggregateRetirementJournalPhaseV1::Prepared {
        return Err(refusal("retirement dispatch requires durable Prepared"));
    }
    authenticate_packet_binding_v1(campaign, current.operation, &packet)?;
    transition_journal_v1(
        current,
        AggregateRetirementJournalPhaseV1::Dispatching,
        Some(packet),
        None,
    )
    .and_then(|next| authenticate_transition_v1(campaign, current, &next).map(|_| next))
}

pub(crate) fn build_aggregate_retirement_packet_binding_v1(
    campaign: &AggregateRetirementCampaignV1,
    operation: AggregateRetirementOperationV1,
    signed: SignedVersionedPacketV1,
    resolved_account_keys: Vec<Pubkey>,
) -> Result<AggregateRetirementPacketBindingV1> {
    let bytes = decode_base64(&signed.packet_base64, "retirement signed packet")?;
    let transaction: VersionedTransaction = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("retirement signed packet: {error}")))?;
    let observed_set = resolved_account_keys
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let binding = AggregateRetirementPacketBindingV1 {
        signed,
        message_sha256: sha256_hex(&transaction.message.serialize()),
        resolved_account_keys: resolved_account_keys
            .iter()
            .map(ToString::to_string)
            .collect(),
        resolved_account_keys_sha256: sha256_hex(&pubkey_bytes(&resolved_account_keys)),
        exact_key_set_sha256: sha256_hex(&pubkey_bytes(
            &observed_set.iter().copied().collect::<Vec<_>>(),
        )),
        lookup_table_sha256: campaign.lookup_table_sha256.clone(),
    };
    authenticate_packet_binding_v1(campaign, operation, &binding)?;
    Ok(binding)
}

pub(crate) fn submit_aggregate_retirement_journal_v1(
    campaign: &AggregateRetirementCampaignV1,
    current: &AggregateRetirementJournalV1,
    returned_signature: &str,
) -> Result<AggregateRetirementJournalV1> {
    authenticate_aggregate_retirement_journal_v1(campaign, current)?;
    let expected = current
        .packet
        .as_ref()
        .map(|packet| packet.signed.signature.as_str());
    if current.phase != AggregateRetirementJournalPhaseV1::Dispatching
        || expected != Some(returned_signature)
    {
        return Err(refusal("retirement submission changed its exact signature"));
    }
    transition_journal_v1(
        current,
        AggregateRetirementJournalPhaseV1::Submitted,
        None,
        None,
    )
    .and_then(|next| authenticate_transition_v1(campaign, current, &next).map(|_| next))
}

pub(crate) fn finalize_aggregate_retirement_journal_v1(
    campaign: &AggregateRetirementCampaignV1,
    current: &AggregateRetirementJournalV1,
    projection: &AggregateRetirementChainProjectionV1,
    finalization: AggregateRetirementFinalizationV1,
) -> Result<AggregateRetirementJournalV1> {
    authenticate_aggregate_retirement_journal_v1(campaign, current)?;
    authenticate_chain_projection_v1(projection)?;
    let packet = current
        .packet
        .as_ref()
        .ok_or_else(|| refusal("Submitted retirement journal omitted packet"))?;
    if current.phase != AggregateRetirementJournalPhaseV1::Submitted
        || projection.phase != current.successor
        || projection.state_sha256 != finalization.poststate_sha256
        || projection.checkpoint_history_sha256 != finalization.checkpoint_history_sha256
        || finalization.signature != packet.signed.signature
        || finalization.packet_sha256 != packet.signed.packet_sha256
        || finalization.finalized_slot == 0
        || finalization.compute_units_consumed == 0
    {
        return Err(refusal(
            "retirement finalization changed signature, packet, slot, phase, receipt, or poststate",
        ));
    }
    transition_journal_v1(
        current,
        AggregateRetirementJournalPhaseV1::Finalized,
        None,
        Some(finalization),
    )
    .and_then(|next| authenticate_transition_v1(campaign, current, &next).map(|_| next))
}

pub(crate) fn aggregate_retirement_recovery_v1(
    campaign: &AggregateRetirementCampaignV1,
    journal: &AggregateRetirementJournalV1,
) -> Result<AggregateRetirementRecoveryV1> {
    authenticate_aggregate_retirement_journal_v1(campaign, journal)?;
    Ok(match journal.phase {
        AggregateRetirementJournalPhaseV1::Planned => {
            AggregateRetirementRecoveryV1::PersistPrepared
        }
        AggregateRetirementJournalPhaseV1::Prepared => {
            AggregateRetirementRecoveryV1::SignOnceAndPersistDispatching
        }
        AggregateRetirementJournalPhaseV1::Dispatching => {
            AggregateRetirementRecoveryV1::PollThenResendIdentical
        }
        AggregateRetirementJournalPhaseV1::Submitted => AggregateRetirementRecoveryV1::PollOnly,
        AggregateRetirementJournalPhaseV1::Finalized => AggregateRetirementRecoveryV1::Complete,
    })
}

pub(crate) fn route_aggregate_retirement_v1(
    campaign: &AggregateRetirementCampaignV1,
    journals: &[AggregateRetirementJournalV1],
    projection: &AggregateRetirementChainProjectionV1,
) -> Result<AggregateRetirementRouteV1> {
    authenticate_aggregate_retirement_campaign_v1(campaign)?;
    authenticate_chain_projection_v1(projection)?;
    if journals.len() > 4 {
        return Err(refusal(
            "retirement journal sequence exceeded four mutations",
        ));
    }
    let mut finalized = 0usize;
    let mut active = None;
    for (index, journal) in journals.iter().enumerate() {
        authenticate_aggregate_retirement_journal_v1(campaign, journal)?;
        if journal.operation != AggregateRetirementOperationV1::ORDERED[index] {
            return Err(refusal("retirement journals were not in predecessor order"));
        }
        if journal.phase == AggregateRetirementJournalPhaseV1::Finalized {
            if active.is_some() {
                return Err(refusal(
                    "retirement finalized a journal after an active predecessor",
                ));
            }
            finalized += 1;
        } else if active.replace(journal).is_some() || index + 1 != journals.len() {
            return Err(refusal(
                "retirement journals contained a gap or multiple active mutations",
            ));
        }
    }
    let observed = projection.phase.ordinal();
    if let Some(journal) = active {
        if observed == finalized {
            return Ok(AggregateRetirementRouteV1::Recover(
                journal.operation,
                aggregate_retirement_recovery_v1(campaign, journal)?,
            ));
        }
        if observed == finalized + 1
            && matches!(
                journal.phase,
                AggregateRetirementJournalPhaseV1::Dispatching
                    | AggregateRetirementJournalPhaseV1::Submitted
            )
        {
            return Ok(AggregateRetirementRouteV1::Recover(
                journal.operation,
                aggregate_retirement_recovery_v1(campaign, journal)?,
            ));
        }
        return Err(refusal(
            "onchain retirement phase advanced without one reconcilable signed journal",
        ));
    }
    if observed != finalized {
        return Err(refusal(
            "onchain retirement phase did not equal the exact finalized journal prefix",
        ));
    }
    if finalized == 4 {
        return Ok(AggregateRetirementRouteV1::Complete);
    }
    Ok(AggregateRetirementRouteV1::Plan(
        AggregateRetirementOperationV1::ORDERED[finalized],
    ))
}

pub(crate) fn build_aggregate_retirement_conservation_receipt_v1(
    campaign: &AggregateRetirementCampaignV1,
    journals: &[AggregateRetirementJournalV1],
    projection: &AggregateRetirementChainProjectionV1,
) -> Result<AggregateRetirementConservationReceiptV1> {
    if route_aggregate_retirement_v1(campaign, journals, projection)?
        != AggregateRetirementRouteV1::Complete
        || projection.phase != AggregateRetirementChainPhaseV1::Complete
    {
        return Err(refusal(
            "terminal retirement receipt requires complete chain and journals",
        ));
    }
    let wallet = projection
        .accounts
        .get(&campaign.refund_wallet.address)
        .and_then(Option::as_ref)
        .ok_or_else(|| refusal("terminal retirement receipt omitted refund wallet"))?;
    let mut total_fees = 0u64;
    let mut completion = Vec::with_capacity(4);
    for journal in journals {
        let finalization = journal
            .finalization
            .as_ref()
            .ok_or_else(|| refusal("terminal retirement journal omitted finalization"))?;
        total_fees = total_fees
            .checked_add(finalization.fee_lamports)
            .ok_or_else(|| refusal("terminal retirement fee total overflowed"))?;
        completion.push(AggregateRetirementCompletionJournalV1 {
            operation: journal.operation,
            journal_sha256: journal.state_sha256.clone(),
            signature: finalization.signature.clone(),
            finalized_slot: finalization.finalized_slot,
            fee_lamports: finalization.fee_lamports,
            compute_units_consumed: finalization.compute_units_consumed,
            packet_sha256: finalization.packet_sha256.clone(),
            poststate_sha256: finalization.poststate_sha256.clone(),
        });
    }
    let expected = campaign
        .classified_lamports
        .refund_wallet_before
        .checked_add(campaign.classified_lamports.expected_refund_delta)
        .and_then(|value| {
            if campaign.payer == campaign.refund_wallet.address {
                value.checked_sub(total_fees)
            } else {
                Some(value)
            }
        })
        .ok_or_else(|| refusal("terminal retirement conservation arithmetic underflowed"))?;
    if wallet.lamports != expected {
        return Err(refusal(
            "terminal retirement wallet did not conserve exact rent/refund classes",
        ));
    }
    let mut receipt = AggregateRetirementConservationReceiptV1 {
        schema: AGGREGATE_RETIREMENT_COMPLETION_SCHEMA_V1.into(),
        status: "finalized".into(),
        campaign_sha256: campaign.campaign_sha256.clone(),
        market: campaign.market.address.clone(),
        checkpoint: campaign.checkpoint.address.clone(),
        rent_credit: campaign.rent_credit.address.clone(),
        refund_wallet: campaign.refund_wallet.address.clone(),
        payer: campaign.payer.clone(),
        classified_lamports: campaign.classified_lamports.clone(),
        total_transaction_fees_lamports: total_fees,
        terminal_refund_wallet_lamports: wallet.lamports,
        journals: completion,
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = conservation_receipt_digest_v1(&receipt)?;
    Ok(receipt)
}

pub(crate) fn authenticate_aggregate_retirement_journal_v1(
    campaign: &AggregateRetirementCampaignV1,
    journal: &AggregateRetirementJournalV1,
) -> Result<()> {
    authenticate_aggregate_retirement_campaign_v1(campaign)?;
    if journal.schema != AGGREGATE_RETIREMENT_JOURNAL_SCHEMA_V1
        || journal.campaign_sha256 != campaign.campaign_sha256
        || journal.predecessor != journal.operation.predecessor()
        || journal.successor != journal.operation.successor()
        || journal.intent_sha256 != journal_intent_digest_v1(journal)?
        || journal.state_sha256 != journal_state_digest_v1(journal)?
    {
        return Err(refusal("retirement journal identity or digest changed"));
    }
    require_sha256(&journal.planned_prestate_sha256, "journal prestate")?;
    require_sha256(&journal.intent_sha256, "journal intent")?;
    require_sha256(&journal.state_sha256, "journal state")?;
    match journal.phase {
        AggregateRetirementJournalPhaseV1::Planned
        | AggregateRetirementJournalPhaseV1::Prepared => {
            if journal.packet.is_some() || journal.finalization.is_some() {
                return Err(refusal(
                    "unsigned retirement journal carried packet/finalization",
                ));
            }
        }
        AggregateRetirementJournalPhaseV1::Dispatching
        | AggregateRetirementJournalPhaseV1::Submitted => {
            authenticate_packet_binding_v1(
                campaign,
                journal.operation,
                journal
                    .packet
                    .as_ref()
                    .ok_or_else(|| refusal("signed retirement journal omitted packet"))?,
            )?;
            if journal.finalization.is_some() {
                return Err(refusal("nonfinal retirement journal carried finalization"));
            }
        }
        AggregateRetirementJournalPhaseV1::Finalized => {
            let packet = journal
                .packet
                .as_ref()
                .ok_or_else(|| refusal("finalized retirement journal omitted packet"))?;
            authenticate_packet_binding_v1(campaign, journal.operation, packet)?;
            let finalized = journal
                .finalization
                .as_ref()
                .ok_or_else(|| refusal("finalized retirement journal omitted evidence"))?;
            if finalized.signature != packet.signed.signature
                || finalized.packet_sha256 != packet.signed.packet_sha256
                || finalized.finalized_slot == 0
                || finalized.compute_units_consumed == 0
            {
                return Err(refusal("finalized retirement evidence changed"));
            }
            require_sha256(&finalized.poststate_sha256, "finalized poststate")?;
        }
    }
    Ok(())
}

fn durable_operation_v1(
    operation: AggregateRetirementOperationV1,
    payer: Pubkey,
    instruction: &Instruction,
) -> Result<DurableRetirementOperationV1> {
    if instruction.accounts.len() != 35 || instruction.data.len() != operation.expected_data_bytes()
    {
        return Err(refusal("retirement operator changed account or data width"));
    }
    if operation != AggregateRetirementOperationV1::Prepare {
        let suffix = AggregateRetirementSuffixRequestV1::decode(
            instruction
                .data
                .get(..AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1)
                .ok_or_else(|| refusal("retirement suffix prefix was missing"))?,
        )
        .map_err(|_| refusal("retirement suffix request was noncanonical"))?;
        let expected_magic = match operation {
            AggregateRetirementOperationV1::CloseVault => AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1,
            AggregateRetirementOperationV1::CloseReplay => {
                AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1
            }
            AggregateRetirementOperationV1::Finish => AGGREGATE_RETIREMENT_FINISH_MAGIC_V1,
            AggregateRetirementOperationV1::Prepare => unreachable!(),
        };
        if suffix.magic() != expected_magic
            || suffix.expected_phase_revision != operation.ordinal() as u64
        {
            return Err(refusal(
                "retirement suffix action or predecessor revision changed",
            ));
        }
    }
    let exact = std::iter::once(payer)
        .chain(std::iter::once(instruction.program_id))
        .chain(instruction.accounts.iter().map(|meta| meta.pubkey))
        .collect::<BTreeSet<_>>()
        .len();
    if exact != EXACT_RETIREMENT_PROTOCOL_AND_PAYER_KEYS_V1 {
        return Err(refusal(format!(
            "retirement instruction resolved {exact} protocol/payer keys, expected {EXACT_RETIREMENT_PROTOCOL_AND_PAYER_KEYS_V1}",
        )));
    }
    Ok(DurableRetirementOperationV1 {
        operation,
        program_id: instruction.program_id.to_string(),
        accounts: instruction
            .accounts
            .iter()
            .map(|meta| DurableRetirementInstructionAccountV1 {
                address: meta.pubkey.to_string(),
                signer: meta.is_signer,
                writable: meta.is_writable,
            })
            .collect(),
        data_base64: BASE64.encode(&instruction.data),
        data_sha256: sha256_hex(&instruction.data),
        expected_wire_bytes: operation.expected_wire_bytes(),
        exact_protocol_and_payer_keys: exact,
    })
}

fn authenticate_operation_v1(
    operation: &DurableRetirementOperationV1,
    payer: Pubkey,
    core: Pubkey,
) -> Result<()> {
    let instruction = operation.instruction()?;
    if instruction.program_id != core
        || instruction.accounts.len() != 35
        || instruction.data.len() != operation.operation.expected_data_bytes()
        || operation.data_sha256 != sha256_hex(&instruction.data)
        || operation.expected_wire_bytes != operation.operation.expected_wire_bytes()
        || operation.exact_protocol_and_payer_keys != EXACT_RETIREMENT_PROTOCOL_AND_PAYER_KEYS_V1
    {
        return Err(refusal("durable retirement operation changed"));
    }
    let rebuilt = durable_operation_v1(operation.operation, payer, &instruction)?;
    if &rebuilt != operation {
        return Err(refusal("durable retirement operation was noncanonical"));
    }
    Ok(())
}

fn authenticate_packet_binding_v1(
    campaign: &AggregateRetirementCampaignV1,
    operation: AggregateRetirementOperationV1,
    packet: &AggregateRetirementPacketBindingV1,
) -> Result<()> {
    require_sha256(&packet.message_sha256, "packet message")?;
    require_sha256(&packet.resolved_account_keys_sha256, "packet resolved keys")?;
    require_sha256(&packet.exact_key_set_sha256, "packet key set")?;
    if packet.lookup_table_sha256 != campaign.lookup_table_sha256
        || packet.resolved_account_keys.len()
            != EXACT_RETIREMENT_RESOLVED_KEYS_WITH_COMPUTE_BUDGET_V1
    {
        return Err(refusal(
            "retirement packet changed ALT or resolved-key width",
        ));
    }
    let bytes = decode_base64(&packet.signed.packet_base64, "retirement signed packet")?;
    if bytes.len() != operation.expected_wire_bytes()
        || packet.signed.packet_sha256 != sha256_hex(&bytes)
    {
        return Err(refusal("retirement packet width or digest changed"));
    }
    let transaction: VersionedTransaction = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("retirement signed packet: {error}")))?;
    if bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("retirement packet reencode: {error}")))?
        != bytes
    {
        return Err(refusal("retirement signed packet was noncanonical"));
    }
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("retirement packet signature: {error}")))?;
    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| refusal("retirement packet omitted signature"))?;
    let message_bytes = transaction.message.serialize();
    if transaction.signatures.len() != 1
        || signature.to_string() != packet.signed.signature
        || sha256_hex(&message_bytes) != packet.message_sha256
        || packet.signed.last_valid_block_height == 0
    {
        return Err(refusal(
            "retirement packet signature, message, or validity changed",
        ));
    }
    let resolved = packet
        .resolved_account_keys
        .iter()
        .map(|value| parse_pubkey(value, "resolved retirement key"))
        .collect::<Result<Vec<_>>>()?;
    if sha256_hex(&pubkey_bytes(&resolved)) != packet.resolved_account_keys_sha256
        || resolved.iter().copied().collect::<BTreeSet<_>>().len() != resolved.len()
    {
        return Err(refusal(
            "retirement resolved keys were duplicated or changed",
        ));
    }
    let expected_operation = &campaign.operations[operation.ordinal()];
    let instruction = expected_operation.instruction()?;
    let expected_set = std::iter::once(parse_pubkey(&campaign.payer, "campaign payer")?)
        .chain(std::iter::once(COMPUTE_BUDGET_PROGRAM_ID))
        .chain(std::iter::once(instruction.program_id))
        .chain(instruction.accounts.iter().map(|meta| meta.pubkey))
        .collect::<BTreeSet<_>>();
    let observed_set = resolved.iter().copied().collect::<BTreeSet<_>>();
    if expected_set.len() != EXACT_RETIREMENT_RESOLVED_KEYS_WITH_COMPUTE_BUDGET_V1
        || observed_set != expected_set
        || sha256_hex(&pubkey_bytes(
            &observed_set.iter().copied().collect::<Vec<_>>(),
        )) != packet.exact_key_set_sha256
    {
        return Err(refusal(
            "retirement packet did not bind the exact 36-key route plus ComputeBudget",
        ));
    }
    let VersionedMessage::V0(message) = &transaction.message else {
        return Err(refusal("retirement packet was not v0"));
    };
    if message.instructions.len() != 3
        || resolved.get(usize::from(message.instructions[0].program_id_index))
            != Some(&COMPUTE_BUDGET_PROGRAM_ID)
        || resolved.get(usize::from(message.instructions[1].program_id_index))
            != Some(&COMPUTE_BUDGET_PROGRAM_ID)
        || message.instructions[0].data.first() != Some(&2)
        || message.instructions[1].data.first() != Some(&3)
    {
        return Err(refusal(
            "retirement packet changed its two exact ComputeBudget prefixes",
        ));
    }
    let compiled = &message.instructions[2];
    if resolved.get(usize::from(compiled.program_id_index)) != Some(&instruction.program_id)
        || compiled.data != instruction.data
        || compiled.accounts.len() != instruction.accounts.len()
        || compiled
            .accounts
            .iter()
            .zip(&instruction.accounts)
            .any(|(index, meta)| resolved.get(usize::from(*index)) != Some(&meta.pubkey))
    {
        return Err(refusal(
            "retirement packet changed the authenticated Core instruction",
        ));
    }
    Ok(())
}

fn transition_journal_v1(
    current: &AggregateRetirementJournalV1,
    phase: AggregateRetirementJournalPhaseV1,
    packet: Option<AggregateRetirementPacketBindingV1>,
    finalization: Option<AggregateRetirementFinalizationV1>,
) -> Result<AggregateRetirementJournalV1> {
    let mut next = current.clone();
    next.phase = phase;
    if packet.is_some() {
        next.packet = packet;
    }
    if finalization.is_some() {
        next.finalization = finalization;
    }
    refresh_journal_digest_v1(&mut next)?;
    Ok(next)
}

fn authenticate_transition_v1(
    campaign: &AggregateRetirementCampaignV1,
    previous: &AggregateRetirementJournalV1,
    next: &AggregateRetirementJournalV1,
) -> Result<()> {
    authenticate_aggregate_retirement_journal_v1(campaign, previous)?;
    authenticate_aggregate_retirement_journal_v1(campaign, next)?;
    if previous.intent_sha256 != next.intent_sha256
        || previous.operation != next.operation
        || previous.planned_prestate_sha256 != next.planned_prestate_sha256
    {
        return Err(refusal(
            "retirement journal intent changed across transition",
        ));
    }
    let legal = matches!(
        (previous.phase, next.phase),
        (
            AggregateRetirementJournalPhaseV1::Planned,
            AggregateRetirementJournalPhaseV1::Prepared
        ) | (
            AggregateRetirementJournalPhaseV1::Prepared,
            AggregateRetirementJournalPhaseV1::Dispatching
        ) | (
            AggregateRetirementJournalPhaseV1::Dispatching,
            AggregateRetirementJournalPhaseV1::Submitted
        ) | (
            AggregateRetirementJournalPhaseV1::Submitted,
            AggregateRetirementJournalPhaseV1::Finalized
        )
    );
    if !legal
        || (previous.packet.is_some() && previous.packet != next.packet)
        || (previous.finalization.is_some() && previous.finalization != next.finalization)
    {
        return Err(refusal(
            "retirement journal phase skipped, reversed, or changed durable bytes",
        ));
    }
    Ok(())
}

fn authenticate_chain_projection_v1(
    projection: &AggregateRetirementChainProjectionV1,
) -> Result<()> {
    if projection.finalized_slot == 0
        || projection.state_sha256 != projection_digest_v1(projection)?
        || projection
            .checkpoint_history_sha256
            .as_ref()
            .is_some_and(|value| require_sha256(value, "checkpoint history").is_err())
    {
        return Err(refusal(
            "retirement chain projection digest or slot changed",
        ));
    }
    for account in projection.accounts.values().flatten() {
        authenticate_durable_account(account)?;
    }
    Ok(())
}

fn authenticate_live_initial_account(
    live: &AggregateRetirementChainAccountV1,
    initial: &DurableRetirementAccountV1,
    exact_lamports: bool,
) -> Result<()> {
    if live.key.to_string() != initial.address
        || live.owner.to_string() != initial.owner
        || live.executable != initial.executable
        || live.data.len() != initial.data_len
        || sha256_hex(&live.data) != initial.data_sha256
        || (exact_lamports && live.lamports != initial.lamports)
    {
        return Err(refusal(
            "live retirement account differed from its exact initial fact",
        ));
    }
    Ok(())
}

fn require_rent_account(
    live: &AggregateRetirementChainAccountV1,
    initial: &DurableRetirementAccountV1,
    expected_lamports: u64,
) -> Result<()> {
    if live.key.to_string() != initial.address
        || live.owner.to_string() != initial.owner
        || live.lamports != expected_lamports
        || live.executable != initial.executable
        || live.data.len() != initial.data_len
        || sha256_hex(&live.data) != initial.data_sha256
    {
        return Err(refusal(
            "RentCredit changed beyond the canonical refund prefix",
        ));
    }
    Ok(())
}

fn durable_chain_account_v1(
    account: &AggregateRetirementChainAccountV1,
) -> DurableRetirementAccountV1 {
    DurableRetirementAccountV1::from_initial(&AggregateRetirementInitialAccountV1 {
        key: account.key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data.clone(),
    })
}

fn campaign_accounts(campaign: &AggregateRetirementCampaignV1) -> [&DurableRetirementAccountV1; 7] {
    [
        &campaign.market,
        &campaign.rent_credit,
        &campaign.checkpoint,
        &campaign.custody_replay,
        &campaign.hoard_vault,
        &campaign.source_receipt,
        &campaign.refund_wallet,
    ]
}

fn campaign_account_keys(campaign: &AggregateRetirementCampaignV1) -> Result<BTreeSet<Pubkey>> {
    let keys = campaign_accounts(campaign)
        .into_iter()
        .map(|account| parse_pubkey(&account.address, "campaign account"))
        .collect::<Result<BTreeSet<_>>>()?;
    if keys.len() != 7 {
        return Err(refusal("retirement campaign account identities aliased"));
    }
    Ok(keys)
}

fn campaign_bundle_digest_v1(campaign: &AggregateRetirementCampaignV1) -> Result<[u8; 32]> {
    let operation = &campaign.operations[AggregateRetirementOperationV1::CloseVault.ordinal()];
    let instruction = operation.instruction()?;
    let suffix = AggregateRetirementSuffixRequestV1::decode(
        instruction
            .data
            .get(..AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1)
            .ok_or_else(|| refusal("campaign vault suffix was missing"))?,
    )
    .map_err(|_| refusal("campaign vault suffix was invalid"))?;
    Ok(suffix.bundle_digest)
}

fn authenticate_durable_account(account: &DurableRetirementAccountV1) -> Result<()> {
    parse_pubkey(&account.address, "durable account address")?;
    parse_pubkey(&account.owner, "durable account owner")?;
    require_sha256(&account.data_sha256, "durable account data")?;
    require_sha256(&account.account_sha256, "durable account")?;
    if account.account_sha256 != account_digest_v1(account) {
        return Err(refusal("durable retirement account digest changed"));
    }
    Ok(())
}

fn account_digest_v1(account: &DurableRetirementAccountV1) -> String {
    let mut copy = account.clone();
    copy.account_sha256.clear();
    sha256_hex(&serde_json::to_vec(&copy).expect("serializable durable retirement account"))
}

fn campaign_digest_v1(campaign: &AggregateRetirementCampaignV1) -> Result<String> {
    let mut copy = campaign.clone();
    copy.campaign_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn projection_digest_v1(projection: &AggregateRetirementChainProjectionV1) -> Result<String> {
    let mut copy = projection.clone();
    copy.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn journal_intent_digest_v1(journal: &AggregateRetirementJournalV1) -> Result<String> {
    let mut copy = journal.clone();
    copy.phase = AggregateRetirementJournalPhaseV1::Planned;
    copy.intent_sha256.clear();
    copy.packet = None;
    copy.finalization = None;
    copy.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn journal_state_digest_v1(journal: &AggregateRetirementJournalV1) -> Result<String> {
    let mut copy = journal.clone();
    copy.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn conservation_receipt_digest_v1(
    receipt: &AggregateRetirementConservationReceiptV1,
) -> Result<String> {
    let mut copy = receipt.clone();
    copy.receipt_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&copy)?))
}

fn refresh_journal_digest_v1(journal: &mut AggregateRetirementJournalV1) -> Result<()> {
    journal.state_sha256 = journal_state_digest_v1(journal)?;
    Ok(())
}

fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn require_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(refusal(format!("{label} was not lowercase SHA-256")));
    }
    Ok(())
}

fn decode_base64(value: &str, label: &str) -> Result<Vec<u8>> {
    let decoded = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    if BASE64.encode(&decoded) != value {
        return Err(refusal(format!("{label} was not canonical base64")));
    }
    Ok(decoded)
}

fn pubkey_bytes(values: &[Pubkey]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_bytes()).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(format!("REFUSED aggregate retirement: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::instruction::AccountMeta;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn account(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
    ) -> AggregateRetirementInitialAccountV1 {
        AggregateRetirementInitialAccountV1 {
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    fn report(
        core: Pubkey,
        market: Pubkey,
        checkpoint: Pubkey,
    ) -> CheckpointMarketRetirementReportV1 {
        let mut accounts = (1..=35)
            .map(|byte| AccountMeta::new_readonly(key(byte), false))
            .collect::<Vec<_>>();
        accounts[0] = AccountMeta::new(market, false);
        accounts[4] = AccountMeta::new_readonly(core, false);
        accounts[14] = AccountMeta::new(checkpoint, false);
        let bundle = [7; 32];
        let source = [8; 32];
        let suffix = |magic, phase, custody| {
            AggregateRetirementSuffixRequestV1::new(
                magic,
                market.to_bytes(),
                checkpoint.to_bytes(),
                bundle,
                source,
                if magic == AGGREGATE_RETIREMENT_FINISH_MAGIC_V1 {
                    [0; 32]
                } else {
                    [9; 32]
                },
                phase,
                custody,
            )
            .expect("suffix")
            .to_bytes()
        };
        let instruction = |data: Vec<u8>| Instruction {
            program_id: core,
            accounts: accounts.clone(),
            data,
        };
        let mut vault = suffix(AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1, 1, 2).to_vec();
        vault.resize(CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1, 0x41);
        let mut replay = suffix(AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1, 2, 3).to_vec();
        replay.resize(CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1, 0x42);
        let mut finish = suffix(AGGREGATE_RETIREMENT_FINISH_MAGIC_V1, 3, 4).to_vec();
        finish.resize(CHECKPOINT_RETIREMENT_FINISH_BYTES_V1, 0x43);
        CheckpointMarketRetirementReportV1 {
            prepare: instruction(vec![0x40; CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1]),
            close_vault: instruction(vault),
            close_replay: instruction(replay),
            finish: instruction(finish),
            observation: dclutch_market_retirement_v1_operator::Observation {
                slot: 9,
                unix_timestamp: 10,
                finality: dclutch_market_retirement_v1_operator::Finality::Finalized,
            },
            expected_refund_delta: 150,
        }
    }

    fn campaign() -> AggregateRetirementCampaignV1 {
        let core = key(5);
        let claims = key(6);
        let market = key(40);
        let checkpoint = key(41);
        let input = AggregateRetirementCampaignInputV1 {
            genesis_hash: key(90).to_string(),
            rpc_url: "http://127.0.0.1:43210/".into(),
            plan_sha256: "11".repeat(32),
            evidence_sha256: "22".repeat(32),
            payer: key(80),
            lookup_table: key(81),
            lookup_table_sha256: "33".repeat(32),
            core_program: core,
            claims_program: claims,
            market: account(market, core, 10, vec![1]),
            rent_credit: account(key(42), key(7), 20, vec![2]),
            checkpoint: account(checkpoint, claims, 30, vec![3]),
            custody_replay: account(key(43), key(8), 40, vec![4]),
            hoard_vault: account(key(44), key(8), 50, vec![5]),
            source_receipt: account(key(45), key(9), 1, vec![6]),
            refund_wallet: account(key(46), system_program::ID, 1_000, Vec::new()),
        };
        build_aggregate_retirement_campaign_v1(input, &report(core, market, checkpoint))
            .expect("campaign")
    }

    fn projection(phase: AggregateRetirementChainPhaseV1) -> AggregateRetirementChainProjectionV1 {
        let mut value = AggregateRetirementChainProjectionV1 {
            phase,
            finalized_slot: 50,
            checkpoint_history_sha256: (phase != AggregateRetirementChainPhaseV1::Ready
                && phase != AggregateRetirementChainPhaseV1::Complete)
                .then(|| "44".repeat(32)),
            accounts: BTreeMap::new(),
            state_sha256: String::new(),
        };
        value.state_sha256 = projection_digest_v1(&value).expect("projection digest");
        value
    }

    #[test]
    fn campaign_binds_four_exact_packet_and_key_shapes() {
        let campaign = campaign();
        authenticate_aggregate_retirement_campaign_v1(&campaign).expect("campaign");
        assert_eq!(campaign.operations.len(), 4);
        assert_eq!(
            campaign
                .operations
                .iter()
                .map(|operation| operation.expected_wire_bytes)
                .collect::<Vec<_>>(),
            [1_135, 1_191, 1_191, 1_071]
        );
        assert!(
            campaign
                .operations
                .iter()
                .all(|operation| operation.exact_protocol_and_payer_keys == 36)
        );
    }

    #[test]
    fn route_is_chain_derived_and_refuses_unjournaled_or_skipped_progress() {
        let campaign = campaign();
        let ready = projection(AggregateRetirementChainPhaseV1::Ready);
        assert_eq!(
            route_aggregate_retirement_v1(&campaign, &[], &ready).expect("ready route"),
            AggregateRetirementRouteV1::Plan(AggregateRetirementOperationV1::Prepare)
        );
        let journal = plan_aggregate_retirement_journal_v1(
            &campaign,
            AggregateRetirementOperationV1::Prepare,
            &ready,
        )
        .expect("planned");
        assert!(
            route_aggregate_retirement_v1(
                &campaign,
                std::slice::from_ref(&journal),
                &projection(AggregateRetirementChainPhaseV1::ClaimsClosed)
            )
            .is_err()
        );
        assert!(
            route_aggregate_retirement_v1(
                &campaign,
                &[],
                &projection(AggregateRetirementChainPhaseV1::ClaimsClosed)
            )
            .is_err()
        );
    }

    #[test]
    fn recovery_distinguishes_identical_resend_from_poll_only() {
        let campaign = campaign();
        let ready = projection(AggregateRetirementChainPhaseV1::Ready);
        let planned = plan_aggregate_retirement_journal_v1(
            &campaign,
            AggregateRetirementOperationV1::Prepare,
            &ready,
        )
        .expect("planned");
        let prepared =
            prepare_aggregate_retirement_journal_v1(&campaign, &planned, &ready).expect("prepared");
        assert_eq!(
            aggregate_retirement_recovery_v1(&campaign, &planned).expect("planned route"),
            AggregateRetirementRecoveryV1::PersistPrepared
        );
        assert_eq!(
            aggregate_retirement_recovery_v1(&campaign, &prepared).expect("prepared route"),
            AggregateRetirementRecoveryV1::SignOnceAndPersistDispatching
        );
    }

    #[test]
    fn journal_digest_refuses_phase_theater_and_intent_mutation() {
        let campaign = campaign();
        let ready = projection(AggregateRetirementChainPhaseV1::Ready);
        let journal = plan_aggregate_retirement_journal_v1(
            &campaign,
            AggregateRetirementOperationV1::Prepare,
            &ready,
        )
        .expect("planned");
        let mut skipped = journal.clone();
        skipped.phase = AggregateRetirementJournalPhaseV1::Submitted;
        assert!(authenticate_aggregate_retirement_journal_v1(&campaign, &skipped).is_err());
        let mut retargeted = journal;
        retargeted.operation = AggregateRetirementOperationV1::Finish;
        assert!(authenticate_aggregate_retirement_journal_v1(&campaign, &retargeted).is_err());
    }
}
