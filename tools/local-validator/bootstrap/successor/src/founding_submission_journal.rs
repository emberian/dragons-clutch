//! Durable submission semantics for the split founding ladder.
//!
//! The campaign report is the filesystem owner. This module owns the smaller
//! semantic fact embedded in it: one exact founding packet moves through
//! `Planned -> Prepared -> Dispatching -> Submitted -> Finalized`, and no recovery path may
//! turn an ambiguous packet into a freshly signed transaction.

use std::{collections::BTreeSet, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    hash::Hash, message::VersionedMessage, pubkey::Pubkey, signature::Signature,
    transaction::VersionedTransaction,
};

use crate::{
    Error, Result,
    cluster::{DEVNET_GENESIS_HASH, MAINNET_BETA_GENESIS_HASH},
    model::AccountEvidence,
};

pub(crate) const FOUNDING_SUBMISSION_JOURNAL_SCHEMA_V1: &str =
    "dclutch-public-founding-submission-journal-v1";
pub(crate) const FOUNDING_PRE_SEND_PROJECTION_SCHEMA_V1: &str =
    "dclutch-public-founding-pre-send-projection-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FoundingSubmissionOperationV1 {
    Dcltcfq1,
    Dcltpcb2,
    Dcltgmf2,
    CoreFundingCreateV1,
    ResolutionFundingActivateV1,
    CoreFundingAcceptV1,
}

impl FoundingSubmissionOperationV1 {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Dcltcfq1 => "DCLTCFQ1",
            Self::Dcltpcb2 => "DCLTPCB2",
            Self::Dcltgmf2 => "DCLTGMF2",
            Self::CoreFundingCreateV1 => "core-funding-create-v1",
            Self::ResolutionFundingActivateV1 => "resolution-funding-activate-v1",
            Self::CoreFundingAcceptV1 => "core-funding-accept-v1",
        }
    }

    pub(crate) const fn exact_unique_accounts(self) -> usize {
        match self {
            Self::Dcltcfq1 => 51,
            Self::Dcltpcb2 | Self::Dcltgmf2 => 60,
            // Canonical V7 frames are pairwise distinct and carry their own
            // program key as a frame account. The bounded inline v0 packet
            // adds exactly the disposable payer and ComputeBudget program.
            Self::CoreFundingCreateV1 => 20,
            Self::ResolutionFundingActivateV1 | Self::CoreFundingAcceptV1 => 22,
        }
    }

    pub(crate) const fn exact_required_signatures(self) -> usize {
        match self {
            Self::Dcltcfq1 | Self::Dcltpcb2 => 2,
            Self::Dcltgmf2
            | Self::CoreFundingCreateV1
            | Self::ResolutionFundingActivateV1
            | Self::CoreFundingAcceptV1 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FoundingSubmissionPhaseV1 {
    Planned,
    Prepared,
    Dispatching,
    Submitted,
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FoundingSubmissionRecoveryV1 {
    SignOnce,
    BeginDispatch,
    ResendIdenticalPacket,
    PollOnly,
    Complete,
}

/// Exact native projection emitted after Dispatching has been durably persisted
/// and immediately before its packet is sent. A kill-boundary harness can
/// stop at this hook and prove recovery sees the same intent, packet, and
/// signature rather than allowing a new signing path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FoundingPreSendProjectionV1 {
    pub(crate) schema: String,
    pub(crate) evidence_path: String,
    pub(crate) operation: FoundingSubmissionOperationV1,
    pub(crate) phase: FoundingSubmissionPhaseV1,
    pub(crate) intent_sha256: String,
    pub(crate) signed_packet_sha256: String,
    pub(crate) signature: String,
    pub(crate) dispatching_state_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FoundingSubmissionBindingV1 {
    pub(crate) cluster: String,
    pub(crate) genesis_hash: String,
    pub(crate) evidence_path: String,
    pub(crate) rpc_url: String,
    pub(crate) plan_sha256: String,
    pub(crate) market_sha256: String,
    pub(crate) payer: Pubkey,
}

impl FoundingSubmissionBindingV1 {
    pub(crate) fn new(
        cluster: impl Into<String>,
        genesis_hash: impl Into<String>,
        evidence_path: &Path,
        rpc_url: impl Into<String>,
        plan_sha256: impl Into<String>,
        market_sha256: impl Into<String>,
        payer: Pubkey,
    ) -> Result<Self> {
        if !evidence_path.is_absolute() {
            return Err(refusal("founding journal evidence path must be absolute"));
        }
        let binding = Self {
            cluster: cluster.into(),
            genesis_hash: genesis_hash.into(),
            evidence_path: evidence_path
                .to_str()
                .ok_or_else(|| refusal("founding journal evidence path must be UTF-8"))?
                .to_owned(),
            rpc_url: rpc_url.into(),
            plan_sha256: plan_sha256.into(),
            market_sha256: market_sha256.into(),
            payer,
        };
        authenticate_binding_v1(&binding)?;
        Ok(binding)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FoundingSubmissionPlanV1 {
    pub(crate) operation: FoundingSubmissionOperationV1,
    pub(crate) message: VersionedMessage,
    pub(crate) last_valid_block_height: u64,
    pub(crate) exact_fee_lamports: u64,
    pub(crate) expected_signers: Vec<Pubkey>,
    pub(crate) resolved_accounts_sha256: String,
    pub(crate) prestate_accounts: Vec<Pubkey>,
    pub(crate) prestate_sha256: String,
    pub(crate) completion_accounts: Vec<Pubkey>,
    pub(crate) completion_contract_sha256: String,
    pub(crate) recovery_payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FoundingFinalizationV1 {
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) transaction_sha256: String,
    pub(crate) fee_lamports: u64,
    pub(crate) compute_units_consumed: u64,
    pub(crate) completion_contract_sha256: String,
    pub(crate) poststates: Vec<AccountEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FoundingSubmissionJournalV1 {
    pub(crate) schema: String,
    pub(crate) cluster: String,
    pub(crate) genesis_hash: String,
    pub(crate) evidence_path: String,
    pub(crate) rpc_url: String,
    pub(crate) plan_sha256: String,
    pub(crate) market_sha256: String,
    pub(crate) payer: String,
    pub(crate) operation: FoundingSubmissionOperationV1,
    pub(crate) phase: FoundingSubmissionPhaseV1,
    pub(crate) message_base64: String,
    pub(crate) message_sha256: String,
    pub(crate) last_valid_block_height: u64,
    pub(crate) exact_fee_lamports: u64,
    pub(crate) exact_unique_message_accounts: usize,
    pub(crate) expected_signers: Vec<String>,
    pub(crate) resolved_accounts_sha256: String,
    pub(crate) prestate_accounts: Vec<String>,
    pub(crate) prestate_sha256: String,
    pub(crate) completion_accounts: Vec<String>,
    pub(crate) completion_contract_sha256: String,
    pub(crate) recovery_payload_base64: String,
    pub(crate) recovery_payload_sha256: String,
    pub(crate) expected_wire_bytes: usize,
    /// Stable digest of every immutable, key-free Planned field. This remains
    /// byte-identical while the enclosing state digest advances by phase.
    pub(crate) intent_sha256: String,
    pub(crate) signed_packet_base64: Option<String>,
    pub(crate) signed_packet_sha256: Option<String>,
    pub(crate) expected_signature: Option<String>,
    pub(crate) finalized_slot: Option<u64>,
    pub(crate) transaction_sha256: Option<String>,
    pub(crate) fee_lamports: Option<u64>,
    pub(crate) compute_units_consumed: Option<u64>,
    pub(crate) finalized_poststates: Vec<AccountEvidence>,
    pub(crate) finalized_poststates_sha256: Option<String>,
    pub(crate) state_sha256: String,
}

pub(crate) fn plan_founding_submission_v1(
    binding: &FoundingSubmissionBindingV1,
    plan: FoundingSubmissionPlanV1,
) -> Result<FoundingSubmissionJournalV1> {
    authenticate_binding_v1(binding)?;
    if plan.last_valid_block_height == 0 || plan.exact_fee_lamports == 0 {
        return Err(refusal(
            "founding journal requires positive validity and exact fee",
        ));
    }
    for (value, label) in [
        (&plan.resolved_accounts_sha256, "resolved-account digest"),
        (&plan.prestate_sha256, "prestate digest"),
        (
            &plan.completion_contract_sha256,
            "completion-contract digest",
        ),
    ] {
        require_sha256_v1(value, label)?;
    }
    authenticate_account_list_v1(&plan.prestate_accounts, "prestate")?;
    authenticate_account_list_v1(&plan.completion_accounts, "completion")?;
    if plan.recovery_payload.is_empty() {
        return Err(refusal("founding recovery payload was empty"));
    }
    let (unique, required_signatures) = message_geometry_v1(&plan.message, &plan.expected_signers)?;
    if unique != plan.operation.exact_unique_accounts()
        || required_signatures != plan.operation.exact_required_signatures()
        || plan.expected_signers.first() != Some(&binding.payer)
    {
        return Err(refusal(format!(
            "{} planned geometry changed: expected {} unique accounts and {} signers, observed {unique} and {required_signatures}",
            plan.operation.label(),
            plan.operation.exact_unique_accounts(),
            plan.operation.exact_required_signatures(),
        )));
    }
    let message_bytes = plan.message.serialize();
    let expected_wire_bytes = bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default(); required_signatures],
        message: plan.message,
    })
    .map_err(|error| Error::new(format!("founding packet geometry: {error}")))?
    .len();
    if expected_wire_bytes > 1_232 {
        return Err(refusal("founding planned packet exceeds 1,232 bytes"));
    }
    let mut journal = FoundingSubmissionJournalV1 {
        schema: FOUNDING_SUBMISSION_JOURNAL_SCHEMA_V1.into(),
        cluster: binding.cluster.clone(),
        genesis_hash: binding.genesis_hash.clone(),
        evidence_path: binding.evidence_path.clone(),
        rpc_url: binding.rpc_url.clone(),
        plan_sha256: binding.plan_sha256.clone(),
        market_sha256: binding.market_sha256.clone(),
        payer: binding.payer.to_string(),
        operation: plan.operation,
        phase: FoundingSubmissionPhaseV1::Planned,
        message_base64: BASE64.encode(&message_bytes),
        message_sha256: sha256_hex(&message_bytes),
        last_valid_block_height: plan.last_valid_block_height,
        exact_fee_lamports: plan.exact_fee_lamports,
        exact_unique_message_accounts: unique,
        expected_signers: plan
            .expected_signers
            .iter()
            .map(ToString::to_string)
            .collect(),
        resolved_accounts_sha256: plan.resolved_accounts_sha256,
        prestate_accounts: plan
            .prestate_accounts
            .iter()
            .map(ToString::to_string)
            .collect(),
        prestate_sha256: plan.prestate_sha256,
        completion_accounts: plan
            .completion_accounts
            .iter()
            .map(ToString::to_string)
            .collect(),
        completion_contract_sha256: plan.completion_contract_sha256,
        recovery_payload_base64: BASE64.encode(&plan.recovery_payload),
        recovery_payload_sha256: sha256_hex(&plan.recovery_payload),
        expected_wire_bytes,
        intent_sha256: String::new(),
        signed_packet_base64: None,
        signed_packet_sha256: None,
        expected_signature: None,
        finalized_slot: None,
        transaction_sha256: None,
        fee_lamports: None,
        compute_units_consumed: None,
        finalized_poststates: Vec::new(),
        finalized_poststates_sha256: None,
        state_sha256: String::new(),
    };
    journal.intent_sha256 = intent_digest_v1(&journal)?;
    refresh_state_digest_v1(&mut journal)?;
    authenticate_founding_submission_v1(binding, &journal)?;
    Ok(journal)
}

pub(crate) fn prepare_founding_submission_v1(
    binding: &FoundingSubmissionBindingV1,
    current: &FoundingSubmissionJournalV1,
    packet: &[u8],
) -> Result<FoundingSubmissionJournalV1> {
    authenticate_founding_submission_v1(binding, current)?;
    if current.phase != FoundingSubmissionPhaseV1::Planned {
        return Err(refusal("founding packet may only be prepared from Planned"));
    }
    let transaction = authenticate_packet_bytes_v1(current, packet)?;
    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| refusal("prepared founding packet omitted its signature"))?;
    let mut next = current.clone();
    next.phase = FoundingSubmissionPhaseV1::Prepared;
    next.signed_packet_base64 = Some(BASE64.encode(packet));
    next.signed_packet_sha256 = Some(sha256_hex(packet));
    next.expected_signature = Some(signature.to_string());
    refresh_state_digest_v1(&mut next)?;
    authenticate_transition_v1(binding, current, &next)?;
    Ok(next)
}

pub(crate) fn submit_founding_submission_v1(
    binding: &FoundingSubmissionBindingV1,
    current: &FoundingSubmissionJournalV1,
    returned_signature: &str,
) -> Result<FoundingSubmissionJournalV1> {
    authenticate_founding_submission_v1(binding, current)?;
    if current.phase != FoundingSubmissionPhaseV1::Dispatching
        || current.expected_signature.as_deref() != Some(returned_signature)
    {
        return Err(refusal(
            "founding submission did not return the exact prepared signature",
        ));
    }
    let mut next = current.clone();
    next.phase = FoundingSubmissionPhaseV1::Submitted;
    refresh_state_digest_v1(&mut next)?;
    authenticate_transition_v1(binding, current, &next)?;
    Ok(next)
}

pub(crate) fn dispatch_founding_submission_v1(
    binding: &FoundingSubmissionBindingV1,
    current: &FoundingSubmissionJournalV1,
) -> Result<FoundingSubmissionJournalV1> {
    authenticate_founding_submission_v1(binding, current)?;
    if current.phase != FoundingSubmissionPhaseV1::Prepared {
        return Err(refusal(
            "founding dispatch may only begin from durable Prepared",
        ));
    }
    let mut next = current.clone();
    next.phase = FoundingSubmissionPhaseV1::Dispatching;
    refresh_state_digest_v1(&mut next)?;
    authenticate_transition_v1(binding, current, &next)?;
    Ok(next)
}

pub(crate) fn finalize_founding_submission_v1(
    binding: &FoundingSubmissionBindingV1,
    current: &FoundingSubmissionJournalV1,
    finalization: FoundingFinalizationV1,
) -> Result<FoundingSubmissionJournalV1> {
    authenticate_founding_submission_v1(binding, current)?;
    if current.phase != FoundingSubmissionPhaseV1::Submitted {
        return Err(refusal("founding submission may only finalize Submitted"));
    }
    if finalization.signature != current.expected_signature.as_deref().unwrap_or_default()
        || finalization.finalized_slot == 0
        || finalization.transaction_sha256
            != current.signed_packet_sha256.as_deref().unwrap_or_default()
        || finalization.fee_lamports != current.exact_fee_lamports
        || finalization.completion_contract_sha256 != current.completion_contract_sha256
    {
        return Err(refusal(
            "founding finalization changed signature, packet, fee, slot, or completion contract",
        ));
    }
    authenticate_finalized_poststates_v1(current, &finalization.poststates)?;
    let mut next = current.clone();
    next.phase = FoundingSubmissionPhaseV1::Finalized;
    next.finalized_slot = Some(finalization.finalized_slot);
    next.transaction_sha256 = Some(finalization.transaction_sha256);
    next.fee_lamports = Some(finalization.fee_lamports);
    next.compute_units_consumed = Some(finalization.compute_units_consumed);
    next.finalized_poststates_sha256 = Some(poststates_digest_v1(&finalization.poststates)?);
    next.finalized_poststates = finalization.poststates;
    refresh_state_digest_v1(&mut next)?;
    authenticate_transition_v1(binding, current, &next)?;
    Ok(next)
}

pub(crate) fn founding_submission_recovery_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<FoundingSubmissionRecoveryV1> {
    authenticate_founding_submission_v1(binding, journal)?;
    Ok(match journal.phase {
        FoundingSubmissionPhaseV1::Planned => FoundingSubmissionRecoveryV1::SignOnce,
        FoundingSubmissionPhaseV1::Prepared => FoundingSubmissionRecoveryV1::BeginDispatch,
        FoundingSubmissionPhaseV1::Dispatching => {
            FoundingSubmissionRecoveryV1::ResendIdenticalPacket
        }
        FoundingSubmissionPhaseV1::Submitted => FoundingSubmissionRecoveryV1::PollOnly,
        FoundingSubmissionPhaseV1::Finalized => FoundingSubmissionRecoveryV1::Complete,
    })
}

pub(crate) fn authenticate_founding_packet_fresh_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
    current_block_height: u64,
) -> Result<()> {
    authenticate_founding_submission_v1(binding, journal)?;
    if !matches!(
        journal.phase,
        FoundingSubmissionPhaseV1::Planned
            | FoundingSubmissionPhaseV1::Prepared
            | FoundingSubmissionPhaseV1::Dispatching
    ) {
        return Err(refusal(
            "Submitted/Finalized founding packet has no signing or send path",
        ));
    }
    if current_block_height > journal.last_valid_block_height {
        return Err(refusal(format!(
            "{} packet expired; preserve this evidence path and create a new authorized path, never re-sign in place",
            journal.operation.label()
        )));
    }
    Ok(())
}

pub(crate) fn founding_submission_packet_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<Vec<u8>> {
    authenticate_founding_submission_v1(binding, journal)?;
    if !matches!(
        journal.phase,
        FoundingSubmissionPhaseV1::Prepared
            | FoundingSubmissionPhaseV1::Dispatching
            | FoundingSubmissionPhaseV1::Submitted
            | FoundingSubmissionPhaseV1::Finalized
    ) {
        return Err(refusal("Planned founding journal has no signed packet"));
    }
    decode_canonical_base64_v1(
        journal
            .signed_packet_base64
            .as_deref()
            .ok_or_else(|| refusal("founding journal omitted signed packet"))?,
        "founding signed packet",
    )
}

pub(crate) fn founding_pre_send_projection_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<FoundingPreSendProjectionV1> {
    authenticate_founding_submission_v1(binding, journal)?;
    if journal.phase != FoundingSubmissionPhaseV1::Dispatching {
        return Err(refusal(
            "founding pre-send hook requires Dispatching journal",
        ));
    }
    Ok(FoundingPreSendProjectionV1 {
        schema: FOUNDING_PRE_SEND_PROJECTION_SCHEMA_V1.into(),
        evidence_path: journal.evidence_path.clone(),
        operation: journal.operation,
        phase: journal.phase,
        intent_sha256: journal.intent_sha256.clone(),
        signed_packet_sha256: journal
            .signed_packet_sha256
            .clone()
            .ok_or_else(|| refusal("Dispatching founding journal omitted packet digest"))?,
        signature: journal
            .expected_signature
            .clone()
            .ok_or_else(|| refusal("Dispatching founding journal omitted signature"))?,
        dispatching_state_sha256: journal.state_sha256.clone(),
    })
}

pub(crate) fn visit_founding_pre_send_boundary_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
    hook: &mut dyn FnMut(&FoundingPreSendProjectionV1) -> Result<()>,
) -> Result<FoundingPreSendProjectionV1> {
    let projection = founding_pre_send_projection_v1(binding, journal)?;
    hook(&projection)?;
    Ok(projection)
}

pub(crate) fn founding_submission_message_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<VersionedMessage> {
    authenticate_founding_submission_v1(binding, journal)?;
    let bytes = decode_canonical_base64_v1(&journal.message_base64, "founding message")?;
    bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("founding versioned message: {error}")))
}

pub(crate) fn founding_submission_recovery_payload_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<Vec<u8>> {
    authenticate_founding_submission_v1(binding, journal)?;
    decode_canonical_base64_v1(
        &journal.recovery_payload_base64,
        "founding recovery payload",
    )
}

pub(crate) fn founding_submission_finalized_poststates_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<Vec<AccountEvidence>> {
    authenticate_founding_submission_v1(binding, journal)?;
    if journal.phase != FoundingSubmissionPhaseV1::Finalized {
        return Err(refusal("founding poststates require Finalized journal"));
    }
    Ok(journal.finalized_poststates.clone())
}

pub(crate) fn authenticate_founding_submission_v1(
    binding: &FoundingSubmissionBindingV1,
    journal: &FoundingSubmissionJournalV1,
) -> Result<()> {
    authenticate_binding_v1(binding)?;
    if journal.schema != FOUNDING_SUBMISSION_JOURNAL_SCHEMA_V1
        || journal.cluster != binding.cluster
        || journal.genesis_hash != binding.genesis_hash
        || journal.evidence_path != binding.evidence_path
        || journal.rpc_url != binding.rpc_url
        || journal.plan_sha256 != binding.plan_sha256
        || journal.market_sha256 != binding.market_sha256
        || journal.payer != binding.payer.to_string()
        || journal.intent_sha256 != intent_digest_v1(journal)?
        || journal.state_sha256 != state_digest_v1(journal)?
        || journal.last_valid_block_height == 0
        || journal.exact_fee_lamports == 0
        || journal.exact_unique_message_accounts != journal.operation.exact_unique_accounts()
        || journal.expected_signers.len() != journal.operation.exact_required_signatures()
    {
        return Err(refusal("founding journal identity or state digest changed"));
    }
    for (value, label) in [
        (&journal.message_sha256, "message digest"),
        (&journal.resolved_accounts_sha256, "resolved-account digest"),
        (&journal.prestate_sha256, "prestate digest"),
        (
            &journal.completion_contract_sha256,
            "completion-contract digest",
        ),
        (&journal.recovery_payload_sha256, "recovery-payload digest"),
        (&journal.intent_sha256, "intent digest"),
    ] {
        require_sha256_v1(value, label)?;
    }
    let prestate_accounts = parse_account_list_v1(&journal.prestate_accounts, "prestate")?;
    let completion_accounts = parse_account_list_v1(&journal.completion_accounts, "completion")?;
    authenticate_account_list_v1(&prestate_accounts, "prestate")?;
    authenticate_account_list_v1(&completion_accounts, "completion")?;
    let recovery_payload = decode_canonical_base64_v1(
        &journal.recovery_payload_base64,
        "founding recovery payload",
    )?;
    if recovery_payload.is_empty()
        || sha256_hex(&recovery_payload) != journal.recovery_payload_sha256
    {
        return Err(refusal("founding recovery payload or digest changed"));
    }
    let message_bytes = decode_canonical_base64_v1(&journal.message_base64, "founding message")?;
    if sha256_hex(&message_bytes) != journal.message_sha256 {
        return Err(refusal("founding message digest changed"));
    }
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("founding versioned message: {error}")))?;
    if message.serialize() != message_bytes {
        return Err(refusal("founding versioned message was not canonical"));
    }
    let expected_signers = journal
        .expected_signers
        .iter()
        .map(|value| {
            value
                .parse::<Pubkey>()
                .map_err(|error| Error::new(format!("founding expected signer: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if expected_signers.first() != Some(&binding.payer) {
        return Err(refusal("founding payer signer changed"));
    }
    let (unique, signatures) = message_geometry_v1(&message, &expected_signers)?;
    let expected_wire = bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default(); signatures],
        message,
    })
    .map_err(|error| Error::new(format!("founding packet geometry: {error}")))?
    .len();
    if unique != journal.exact_unique_message_accounts
        || signatures != journal.operation.exact_required_signatures()
        || expected_wire != journal.expected_wire_bytes
        || expected_wire > 1_232
    {
        return Err(refusal("founding message geometry changed"));
    }
    match journal.phase {
        FoundingSubmissionPhaseV1::Planned => {
            require_no_signed_v1(journal)?;
            require_no_finalized_v1(journal)?;
        }
        FoundingSubmissionPhaseV1::Prepared
        | FoundingSubmissionPhaseV1::Dispatching
        | FoundingSubmissionPhaseV1::Submitted => {
            authenticate_signed_packet_v1(journal)?;
            require_no_finalized_v1(journal)?;
        }
        FoundingSubmissionPhaseV1::Finalized => {
            authenticate_signed_packet_v1(journal)?;
            if journal.finalized_slot.is_none_or(|slot| slot == 0)
                || journal.transaction_sha256 != journal.signed_packet_sha256
                || journal.fee_lamports != Some(journal.exact_fee_lamports)
                || journal.compute_units_consumed.is_none()
            {
                return Err(refusal("finalized founding evidence changed"));
            }
            authenticate_finalized_poststates_v1(journal, &journal.finalized_poststates)?;
            if journal.finalized_poststates_sha256.as_deref()
                != Some(poststates_digest_v1(&journal.finalized_poststates)?.as_str())
            {
                return Err(refusal("finalized founding poststate digest changed"));
            }
        }
    }
    Ok(())
}

fn authenticate_transition_v1(
    binding: &FoundingSubmissionBindingV1,
    previous: &FoundingSubmissionJournalV1,
    next: &FoundingSubmissionJournalV1,
) -> Result<()> {
    authenticate_founding_submission_v1(binding, previous)?;
    authenticate_founding_submission_v1(binding, next)?;
    if !same_intent_v1(previous, next) {
        return Err(refusal("founding durable intent changed across transition"));
    }
    match (previous.phase, next.phase) {
        (FoundingSubmissionPhaseV1::Planned, FoundingSubmissionPhaseV1::Prepared) => {
            authenticate_signed_packet_v1(next)?;
            require_no_finalized_v1(next)?;
        }
        (FoundingSubmissionPhaseV1::Prepared, FoundingSubmissionPhaseV1::Dispatching) => {
            if signed_fields_v1(previous) != signed_fields_v1(next) {
                return Err(refusal("founding signed packet changed at dispatch"));
            }
        }
        (FoundingSubmissionPhaseV1::Dispatching, FoundingSubmissionPhaseV1::Submitted) => {
            if signed_fields_v1(previous) != signed_fields_v1(next) {
                return Err(refusal("founding signed packet changed at submission"));
            }
        }
        (FoundingSubmissionPhaseV1::Submitted, FoundingSubmissionPhaseV1::Finalized) => {
            if signed_fields_v1(previous) != signed_fields_v1(next) {
                return Err(refusal("founding signed packet changed at finalization"));
            }
        }
        _ => return Err(refusal("founding phase skipped, repeated, or reversed")),
    }
    Ok(())
}

fn same_intent_v1(left: &FoundingSubmissionJournalV1, right: &FoundingSubmissionJournalV1) -> bool {
    left.schema == right.schema
        && left.cluster == right.cluster
        && left.genesis_hash == right.genesis_hash
        && left.evidence_path == right.evidence_path
        && left.rpc_url == right.rpc_url
        && left.plan_sha256 == right.plan_sha256
        && left.market_sha256 == right.market_sha256
        && left.payer == right.payer
        && left.operation == right.operation
        && left.message_base64 == right.message_base64
        && left.message_sha256 == right.message_sha256
        && left.last_valid_block_height == right.last_valid_block_height
        && left.exact_fee_lamports == right.exact_fee_lamports
        && left.exact_unique_message_accounts == right.exact_unique_message_accounts
        && left.expected_signers == right.expected_signers
        && left.resolved_accounts_sha256 == right.resolved_accounts_sha256
        && left.prestate_accounts == right.prestate_accounts
        && left.prestate_sha256 == right.prestate_sha256
        && left.completion_accounts == right.completion_accounts
        && left.completion_contract_sha256 == right.completion_contract_sha256
        && left.recovery_payload_base64 == right.recovery_payload_base64
        && left.recovery_payload_sha256 == right.recovery_payload_sha256
        && left.expected_wire_bytes == right.expected_wire_bytes
        && left.intent_sha256 == right.intent_sha256
}

fn message_geometry_v1(
    message: &VersionedMessage,
    expected_signers: &[Pubkey],
) -> Result<(usize, usize)> {
    let (header, static_keys, loaded) = match message {
        VersionedMessage::Legacy(message) => (&message.header, message.account_keys.as_slice(), 0),
        VersionedMessage::V0(message) => (
            &message.header,
            message.account_keys.as_slice(),
            message
                .address_table_lookups
                .iter()
                .map(|lookup| lookup.writable_indexes.len() + lookup.readonly_indexes.len())
                .sum(),
        ),
    };
    let required = usize::from(header.num_required_signatures);
    if required == 0
        || static_keys.get(..required) != Some(expected_signers)
        || expected_signers
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != expected_signers.len()
    {
        return Err(refusal("founding signer closure changed"));
    }
    if static_keys.iter().copied().collect::<BTreeSet<_>>().len() != static_keys.len() {
        return Err(refusal("founding static message keys were duplicated"));
    }
    let unique = static_keys
        .len()
        .checked_add(loaded)
        .ok_or_else(|| refusal("founding unique-account count overflow"))?;
    if unique > 64 {
        return Err(refusal(
            "founding message exceeds devnet's 64-account lock limit",
        ));
    }
    Ok((unique, required))
}

fn authenticate_signed_packet_v1(journal: &FoundingSubmissionJournalV1) -> Result<()> {
    let packet = decode_canonical_base64_v1(
        journal
            .signed_packet_base64
            .as_deref()
            .ok_or_else(|| refusal("prepared founding journal omitted packet"))?,
        "founding signed packet",
    )?;
    if journal.signed_packet_sha256.as_deref() != Some(sha256_hex(&packet).as_str()) {
        return Err(refusal("founding signed packet digest changed"));
    }
    let transaction = authenticate_packet_bytes_v1(journal, &packet)?;
    if journal.expected_signature.as_deref()
        != transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
    {
        return Err(refusal("founding expected signature changed"));
    }
    Ok(())
}

fn authenticate_packet_bytes_v1(
    journal: &FoundingSubmissionJournalV1,
    packet: &[u8],
) -> Result<VersionedTransaction> {
    if packet.len() != journal.expected_wire_bytes || packet.len() > 1_232 {
        return Err(refusal("founding signed packet width changed"));
    }
    let transaction: VersionedTransaction = bincode::deserialize(packet)
        .map_err(|error| Error::new(format!("founding signed packet: {error}")))?;
    if bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("founding packet reencode: {error}")))?
        != packet
    {
        return Err(refusal("founding signed packet was not canonical"));
    }
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("founding packet signature: {error}")))?;
    let message = transaction.message.serialize();
    if BASE64.encode(&message) != journal.message_base64
        || sha256_hex(&message) != journal.message_sha256
        || transaction.signatures.len() != journal.operation.exact_required_signatures()
    {
        return Err(refusal("founding packet message or signer count changed"));
    }
    Ok(transaction)
}

fn require_no_signed_v1(journal: &FoundingSubmissionJournalV1) -> Result<()> {
    if signed_fields_v1(journal).iter().any(Option::is_some) {
        return Err(refusal("Planned founding journal carried signed evidence"));
    }
    Ok(())
}

fn require_no_finalized_v1(journal: &FoundingSubmissionJournalV1) -> Result<()> {
    if journal.finalized_slot.is_some()
        || journal.transaction_sha256.is_some()
        || journal.fee_lamports.is_some()
        || journal.compute_units_consumed.is_some()
        || !journal.finalized_poststates.is_empty()
        || journal.finalized_poststates_sha256.is_some()
    {
        return Err(refusal(
            "non-final founding journal carried finalized evidence",
        ));
    }
    Ok(())
}

fn authenticate_finalized_poststates_v1(
    journal: &FoundingSubmissionJournalV1,
    poststates: &[AccountEvidence],
) -> Result<()> {
    let expected = journal
        .completion_accounts
        .iter()
        .map(|value| {
            value
                .parse::<Pubkey>()
                .map_err(|error| Error::new(format!("founding completion account: {error}")))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let mut prior = None;
    let mut observed = BTreeSet::new();
    for account in poststates {
        let address = account
            .address
            .parse::<Pubkey>()
            .map_err(|error| Error::new(format!("founding poststate address: {error}")))?;
        account
            .owner
            .parse::<Pubkey>()
            .map_err(|error| Error::new(format!("founding poststate owner: {error}")))?;
        require_sha256_v1(&account.data_sha256, "poststate data digest")?;
        require_sha256_v1(&account.account_sha256, "poststate account digest")?;
        if prior.is_some_and(|value| value >= address) || !observed.insert(address) {
            return Err(refusal(
                "founding finalized poststates were duplicated or not canonically ordered",
            ));
        }
        prior = Some(address);
    }
    if observed != expected {
        return Err(refusal(
            "founding finalized poststates changed the completion account set",
        ));
    }
    Ok(())
}

fn signed_fields_v1(journal: &FoundingSubmissionJournalV1) -> [Option<&str>; 3] {
    [
        journal.signed_packet_base64.as_deref(),
        journal.signed_packet_sha256.as_deref(),
        journal.expected_signature.as_deref(),
    ]
}

fn authenticate_binding_v1(binding: &FoundingSubmissionBindingV1) -> Result<()> {
    if !Path::new(&binding.evidence_path).is_absolute()
        || binding.rpc_url.is_empty()
        || binding.payer == Pubkey::default()
    {
        return Err(refusal("founding journal binding was incomplete"));
    }
    let genesis = binding
        .genesis_hash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("founding genesis hash: {error}")))?;
    if genesis == Hash::default()
        || binding.genesis_hash == MAINNET_BETA_GENESIS_HASH
        || !matches!(binding.cluster.as_str(), "devnet" | "loopback")
        || (binding.cluster == "devnet" && binding.genesis_hash != DEVNET_GENESIS_HASH)
    {
        return Err(refusal(
            "founding journal cluster/genesis identity was not an admitted devnet or loopback chain",
        ));
    }
    let rpc_url = binding.rpc_url.to_ascii_lowercase();
    if rpc_url.contains("mainnet") || rpc_url.contains("testnet") {
        return Err(refusal(
            "founding journal is devnet-only and refuses mainnet/testnet RPC identities",
        ));
    }
    require_sha256_v1(&binding.plan_sha256, "plan digest")?;
    require_sha256_v1(&binding.market_sha256, "Market digest")?;
    Ok(())
}

fn parse_account_list_v1(values: &[String], label: &str) -> Result<Vec<Pubkey>> {
    values
        .iter()
        .map(|value| {
            value
                .parse::<Pubkey>()
                .map_err(|error| Error::new(format!("founding {label} account: {error}")))
        })
        .collect()
}

fn authenticate_account_list_v1(values: &[Pubkey], label: &str) -> Result<()> {
    if values.is_empty()
        || values.iter().any(|value| *value == Pubkey::default())
        || values.iter().copied().collect::<BTreeSet<_>>().len() != values.len()
    {
        return Err(refusal(format!(
            "founding {label} accounts were empty, zero, or duplicated"
        )));
    }
    Ok(())
}

fn require_sha256_v1(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(refusal(format!(
            "founding {label} was not lowercase SHA-256"
        )));
    }
    Ok(())
}

fn decode_canonical_base64_v1(value: &str, label: &str) -> Result<Vec<u8>> {
    let decoded = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    if BASE64.encode(&decoded) != value {
        return Err(refusal(format!("{label} was not canonical base64")));
    }
    Ok(decoded)
}

fn refresh_state_digest_v1(journal: &mut FoundingSubmissionJournalV1) -> Result<()> {
    journal.state_sha256.clear();
    journal.state_sha256 = state_digest_v1(journal)?;
    Ok(())
}

fn intent_digest_v1(journal: &FoundingSubmissionJournalV1) -> Result<String> {
    let mut copy = journal.clone();
    copy.phase = FoundingSubmissionPhaseV1::Planned;
    copy.intent_sha256.clear();
    copy.signed_packet_base64 = None;
    copy.signed_packet_sha256 = None;
    copy.expected_signature = None;
    copy.finalized_slot = None;
    copy.transaction_sha256 = None;
    copy.fee_lamports = None;
    copy.compute_units_consumed = None;
    copy.finalized_poststates.clear();
    copy.finalized_poststates_sha256 = None;
    copy.state_sha256.clear();
    serde_json::to_vec(&copy)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(Into::into)
}

fn poststates_digest_v1(poststates: &[AccountEvidence]) -> Result<String> {
    serde_json::to_vec(poststates)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(Into::into)
}

fn state_digest_v1(journal: &FoundingSubmissionJournalV1) -> Result<String> {
    let mut copy = journal.clone();
    copy.state_sha256.clear();
    serde_json::to_vec(&copy)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(Into::into)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(format!("REFUSED: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{hash::Hash, message::v0, signature::Keypair, signer::Signer as _};

    fn digest(byte: u8) -> String {
        format!("{byte:02x}").repeat(32)
    }

    fn binding(payer: Pubkey) -> FoundingSubmissionBindingV1 {
        FoundingSubmissionBindingV1::new(
            "devnet",
            DEVNET_GENESIS_HASH,
            Path::new("/tmp/founding-evidence.json"),
            "https://api.devnet.solana.com/",
            digest(1),
            digest(2),
            payer,
        )
        .expect("binding")
    }

    fn loopback_binding(payer: Pubkey) -> FoundingSubmissionBindingV1 {
        FoundingSubmissionBindingV1::new(
            "loopback",
            Hash::new_unique().to_string(),
            Path::new("/tmp/loopback-founding-evidence.json"),
            "http://127.0.0.1:43210/",
            digest(1),
            digest(2),
            payer,
        )
        .expect("loopback binding")
    }

    fn message(signers: &[Pubkey], operation: FoundingSubmissionOperationV1) -> VersionedMessage {
        let total = operation.exact_unique_accounts();
        let mut static_keys = signers.to_vec();
        static_keys.push(Pubkey::new_unique());
        let loaded = total - static_keys.len();
        VersionedMessage::V0(v0::Message {
            header: solana_sdk::message::MessageHeader {
                num_required_signatures: u8::try_from(signers.len()).expect("signer width"),
                num_readonly_signed_accounts: 0,
                num_readonly_unsigned_accounts: 1,
            },
            account_keys: static_keys,
            recent_blockhash: Hash::new_unique(),
            instructions: Vec::new(),
            address_table_lookups: vec![solana_sdk::message::v0::MessageAddressTableLookup {
                account_key: Pubkey::new_unique(),
                writable_indexes: (0..u8::try_from(loaded).expect("loaded width")).collect(),
                readonly_indexes: Vec::new(),
            }],
        })
    }

    fn planned(
        signers: &[&Keypair],
        operation: FoundingSubmissionOperationV1,
    ) -> (FoundingSubmissionBindingV1, FoundingSubmissionJournalV1) {
        let payer = signers.first().expect("payer");
        let signer_keys = signers
            .iter()
            .map(|signer| signer.pubkey())
            .collect::<Vec<_>>();
        let binding = binding(payer.pubkey());
        let journal = plan_founding_submission_v1(
            &binding,
            FoundingSubmissionPlanV1 {
                operation,
                message: message(&signer_keys, operation),
                last_valid_block_height: 900,
                exact_fee_lamports: 5_000,
                expected_signers: signer_keys,
                resolved_accounts_sha256: digest(3),
                prestate_accounts: vec![Pubkey::new_unique()],
                prestate_sha256: digest(4),
                completion_accounts: vec![Pubkey::new_unique()],
                completion_contract_sha256: digest(5),
                recovery_payload: b"recovery".to_vec(),
            },
        )
        .expect("planned");
        (binding, journal)
    }

    fn prepared(
        signers: &[&Keypair],
        operation: FoundingSubmissionOperationV1,
    ) -> (FoundingSubmissionBindingV1, FoundingSubmissionJournalV1) {
        let (binding, planned) = planned(signers, operation);
        let message_bytes = BASE64.decode(&planned.message_base64).expect("message");
        let message: VersionedMessage = bincode::deserialize(&message_bytes).expect("decode");
        let transaction = VersionedTransaction::try_new(message, signers).expect("sign");
        let packet = bincode::serialize(&transaction).expect("packet");
        let prepared =
            prepare_founding_submission_v1(&binding, &planned, &packet).expect("prepared");
        (binding, prepared)
    }

    fn poststates(journal: &FoundingSubmissionJournalV1) -> Vec<AccountEvidence> {
        let owner = Pubkey::new_unique();
        let mut accounts = journal
            .completion_accounts
            .iter()
            .map(|address| AccountEvidence {
                address: address.clone(),
                owner: owner.to_string(),
                lamports: 1,
                executable: false,
                data_len: 1,
                data_sha256: digest(6),
                account_sha256: digest(7),
            })
            .collect::<Vec<_>>();
        accounts.sort_by(|left, right| {
            left.address
                .parse::<Pubkey>()
                .expect("left")
                .cmp(&right.address.parse::<Pubkey>().expect("right"))
        });
        accounts
    }

    #[test]
    fn exact_split_founding_geometry_progresses_through_all_crash_boundaries() {
        for operation in [
            FoundingSubmissionOperationV1::Dcltcfq1,
            FoundingSubmissionOperationV1::Dcltpcb2,
            FoundingSubmissionOperationV1::Dcltgmf2,
            FoundingSubmissionOperationV1::CoreFundingCreateV1,
            FoundingSubmissionOperationV1::ResolutionFundingActivateV1,
            FoundingSubmissionOperationV1::CoreFundingAcceptV1,
        ] {
            let payer = Keypair::new();
            let beneficiary = Keypair::new();
            let signers = match operation {
                FoundingSubmissionOperationV1::Dcltcfq1
                | FoundingSubmissionOperationV1::Dcltpcb2 => {
                    vec![&payer, &beneficiary]
                }
                FoundingSubmissionOperationV1::Dcltgmf2
                | FoundingSubmissionOperationV1::CoreFundingCreateV1
                | FoundingSubmissionOperationV1::ResolutionFundingActivateV1
                | FoundingSubmissionOperationV1::CoreFundingAcceptV1 => vec![&payer],
            };
            let (binding, prepared) = prepared(&signers, operation);
            assert_eq!(
                founding_submission_recovery_v1(&binding, &prepared).expect("recovery"),
                FoundingSubmissionRecoveryV1::BeginDispatch
            );
            assert!(founding_pre_send_projection_v1(&binding, &prepared).is_err());
            let dispatching =
                dispatch_founding_submission_v1(&binding, &prepared).expect("dispatching");
            assert_eq!(dispatching.intent_sha256, prepared.intent_sha256);
            let mut killed_projection = None;
            let simulated_kill =
                visit_founding_pre_send_boundary_v1(&binding, &dispatching, &mut |projection| {
                    killed_projection = Some(projection.clone());
                    Err(Error::new("simulated kill before send"))
                });
            assert!(simulated_kill.is_err());
            assert_eq!(
                founding_submission_recovery_v1(&binding, &dispatching)
                    .expect("kill-before-send recovery"),
                FoundingSubmissionRecoveryV1::ResendIdenticalPacket
            );
            let projection =
                visit_founding_pre_send_boundary_v1(&binding, &dispatching, &mut |_| Ok(()))
                    .expect("pre-send hook");
            assert_eq!(killed_projection.as_ref(), Some(&projection));
            assert_eq!(projection.schema, FOUNDING_PRE_SEND_PROJECTION_SCHEMA_V1);
            assert_eq!(projection.evidence_path, binding.evidence_path);
            assert_eq!(projection.phase, FoundingSubmissionPhaseV1::Dispatching);
            assert_eq!(projection.intent_sha256, prepared.intent_sha256);
            assert_eq!(
                founding_submission_recovery_v1(&binding, &dispatching).expect("recovery"),
                FoundingSubmissionRecoveryV1::ResendIdenticalPacket
            );
            let signature = dispatching.expected_signature.clone().expect("signature");
            let submitted = submit_founding_submission_v1(&binding, &dispatching, &signature)
                .expect("submitted");
            assert_eq!(submitted.intent_sha256, prepared.intent_sha256);
            assert_eq!(
                founding_submission_recovery_v1(&binding, &submitted).expect("recovery"),
                FoundingSubmissionRecoveryV1::PollOnly
            );
            let finalized = finalize_founding_submission_v1(
                &binding,
                &submitted,
                FoundingFinalizationV1 {
                    signature,
                    finalized_slot: 77,
                    transaction_sha256: submitted
                        .signed_packet_sha256
                        .clone()
                        .expect("packet digest"),
                    fee_lamports: 5_000,
                    compute_units_consumed: 700_000,
                    completion_contract_sha256: digest(5),
                    poststates: poststates(&submitted),
                },
            )
            .expect("finalized");
            assert_eq!(
                founding_submission_recovery_v1(&binding, &finalized).expect("recovery"),
                FoundingSubmissionRecoveryV1::Complete
            );
            assert_eq!(finalized.intent_sha256, prepared.intent_sha256);
            let mut substituted_poststate = finalized.clone();
            substituted_poststate.finalized_poststates[0].address =
                Pubkey::new_unique().to_string();
            substituted_poststate.finalized_poststates_sha256 = Some(
                poststates_digest_v1(&substituted_poststate.finalized_poststates)
                    .expect("poststate digest"),
            );
            refresh_state_digest_v1(&mut substituted_poststate).expect("state digest");
            assert!(authenticate_founding_submission_v1(&binding, &substituted_poststate).is_err());
        }
    }

    #[test]
    fn path_packet_signature_prestate_and_completion_substitutions_refuse() {
        let payer = Keypair::new();
        let beneficiary = Keypair::new();
        let (binding, prepared) = prepared(
            &[&payer, &beneficiary],
            FoundingSubmissionOperationV1::Dcltpcb2,
        );
        let mut moved_path = prepared.clone();
        moved_path.evidence_path = "/tmp/other.json".into();
        refresh_state_digest_v1(&mut moved_path).expect("rehash");
        assert!(authenticate_founding_submission_v1(&binding, &moved_path).is_err());

        let mut packet = prepared.clone();
        packet
            .signed_packet_base64
            .as_mut()
            .expect("packet")
            .push('A');
        refresh_state_digest_v1(&mut packet).expect("rehash");
        assert!(authenticate_founding_submission_v1(&binding, &packet).is_err());

        let mut signature = prepared.clone();
        signature.expected_signature = Some(Signature::new_unique().to_string());
        refresh_state_digest_v1(&mut signature).expect("rehash");
        assert!(authenticate_founding_submission_v1(&binding, &signature).is_err());

        for field in ["prestate", "completion"] {
            let mut changed = prepared.clone();
            if field == "prestate" {
                changed.prestate_sha256 = digest(8);
            } else {
                changed.completion_contract_sha256 = digest(9);
            }
            refresh_state_digest_v1(&mut changed).expect("rehash");
            assert!(
                authenticate_founding_submission_v1(&binding, &changed).is_err(),
                "{field} changed immutable intent"
            );
            assert!(
                authenticate_transition_v1(&binding, &prepared, &changed).is_err(),
                "{field} substitution"
            );
        }

        let mut recovery_payload = prepared.clone();
        recovery_payload.recovery_payload_base64 = BASE64.encode(b"replacement");
        recovery_payload.recovery_payload_sha256 = sha256_hex(b"replacement");
        refresh_state_digest_v1(&mut recovery_payload).expect("rehash");
        assert!(authenticate_founding_submission_v1(&binding, &recovery_payload).is_err());
    }

    #[test]
    fn expired_or_ambiguous_packets_have_no_blind_resign_transition() {
        let payer = Keypair::new();
        let (binding, prepared) = prepared(&[&payer], FoundingSubmissionOperationV1::Dcltgmf2);
        assert_eq!(
            founding_submission_recovery_v1(&binding, &prepared).expect("prepared recovery"),
            FoundingSubmissionRecoveryV1::BeginDispatch
        );
        authenticate_founding_packet_fresh_v1(&binding, &prepared, 900)
            .expect("last valid height remains fresh");
        assert!(authenticate_founding_packet_fresh_v1(&binding, &prepared, 901).is_err());
        assert!(prepare_founding_submission_v1(&binding, &prepared, &[]).is_err());
        let dispatching =
            dispatch_founding_submission_v1(&binding, &prepared).expect("dispatching");
        assert_eq!(
            founding_submission_recovery_v1(&binding, &dispatching).expect("dispatch recovery"),
            FoundingSubmissionRecoveryV1::ResendIdenticalPacket
        );
        assert!(dispatch_founding_submission_v1(&binding, &dispatching).is_err());
        let signature = dispatching.expected_signature.clone().expect("signature");
        let submitted =
            submit_founding_submission_v1(&binding, &dispatching, &signature).expect("submitted");
        assert!(prepare_founding_submission_v1(&binding, &submitted, &[]).is_err());
        assert_eq!(
            founding_submission_recovery_v1(&binding, &submitted).expect("submitted recovery"),
            FoundingSubmissionRecoveryV1::PollOnly
        );
        assert!(authenticate_founding_packet_fresh_v1(&binding, &submitted, 1).is_err());
    }

    #[test]
    fn operation_geometry_and_mainnet_or_relative_path_bindings_refuse() {
        let payer = Keypair::new();
        let binding = binding(payer.pubkey());
        let local_binding = loopback_binding(payer.pubkey());
        let local_operation = FoundingSubmissionOperationV1::CoreFundingCreateV1;
        let local_plan = plan_founding_submission_v1(
            &local_binding,
            FoundingSubmissionPlanV1 {
                operation: local_operation,
                message: message(&[payer.pubkey()], local_operation),
                last_valid_block_height: 900,
                exact_fee_lamports: 5_000,
                expected_signers: vec![payer.pubkey()],
                resolved_accounts_sha256: digest(3),
                prestate_accounts: vec![Pubkey::new_unique()],
                prestate_sha256: digest(4),
                completion_accounts: vec![Pubkey::new_unique()],
                completion_contract_sha256: digest(5),
                recovery_payload: b"recovery".to_vec(),
            },
        )
        .expect("loopback plan");
        assert_eq!(local_plan.cluster, "loopback");
        assert_eq!(local_plan.genesis_hash, local_binding.genesis_hash);
        assert!(authenticate_founding_submission_v1(&local_binding, &local_plan).is_ok());
        let wrong = plan_founding_submission_v1(
            &binding,
            FoundingSubmissionPlanV1 {
                operation: FoundingSubmissionOperationV1::Dcltgmf2,
                message: message(
                    &[payer.pubkey(), Pubkey::new_unique()],
                    FoundingSubmissionOperationV1::Dcltpcb2,
                ),
                last_valid_block_height: 1,
                exact_fee_lamports: 1,
                expected_signers: vec![payer.pubkey()],
                resolved_accounts_sha256: digest(1),
                prestate_accounts: vec![Pubkey::new_unique()],
                prestate_sha256: digest(2),
                completion_accounts: vec![Pubkey::new_unique()],
                completion_contract_sha256: digest(3),
                recovery_payload: b"recovery".to_vec(),
            },
        );
        assert!(wrong.is_err());
        assert!(
            FoundingSubmissionBindingV1::new(
                "devnet",
                DEVNET_GENESIS_HASH,
                Path::new("relative.json"),
                "https://api.mainnet-beta.solana.com/",
                digest(1),
                digest(2),
                payer.pubkey(),
            )
            .is_err()
        );
        assert!(
            FoundingSubmissionBindingV1::new(
                "devnet",
                DEVNET_GENESIS_HASH,
                Path::new("/tmp/mainnet-refusal.json"),
                "https://api.mainnet-beta.solana.com/",
                digest(1),
                digest(2),
                payer.pubkey(),
            )
            .is_err()
        );
        assert!(
            FoundingSubmissionBindingV1::new(
                "devnet",
                Hash::new_unique().to_string(),
                Path::new("/tmp/wrong-devnet-genesis.json"),
                "https://api.devnet.solana.com/",
                digest(1),
                digest(2),
                payer.pubkey(),
            )
            .is_err()
        );
    }
}
