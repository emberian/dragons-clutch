//! Pure durable journal semantics for Direct's two pre-Hot setup transactions.
//!
//! This module deliberately owns no filesystem, RPC, key, signing, or
//! submission behavior.  It makes the exterior's crash boundary explicit:
//! replay setup is finalized before token setup can be planned; a signed or
//! submitted packet is poll-only; and every state advance preserves the exact
//! message and expected account effects admitted at `Planned`.

use std::{collections::BTreeSet, str::FromStr as _};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_program::pubkey::Pubkey;
use solana_sdk::{
    message::VersionedMessage, signature::Signature, transaction::VersionedTransaction,
};

use crate::{Error, Result};

pub(crate) const DIRECT_SETUP_JOURNAL_SCHEMA_V1: &str = "dclutch-direct-trade-setup-journal-v1";

/// The total setup order.  Neither member is an economic Direct event.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DirectSetupStageV1 {
    ReplaySetup,
    TokenSetup,
}

/// Durable phases. `Prepared` owns the exact signed packet before any dispatch,
/// `Dispatching` permits only an identical-packet resend, and `Submitted` is
/// poll-only after an RPC has accepted delivery.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DirectSetupJournalPhaseV1 {
    Planned,
    Prepared,
    Dispatching,
    Submitted,
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectSetupRecoveryActionV1 {
    SignAndPrepare,
    BeginDispatch,
    DispatchIdenticalPacket,
    PollOnly,
    Complete,
}

/// Manifest identities supplied independently by the exterior on every load.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectSetupManifestBindingV1 {
    pub(crate) public_manifest_sha256: String,
    pub(crate) private_session_sha256: String,
}

impl DirectSetupManifestBindingV1 {
    pub(crate) fn new(
        public_manifest_sha256: impl Into<String>,
        private_session_sha256: impl Into<String>,
    ) -> Result<Self> {
        let binding = Self {
            public_manifest_sha256: public_manifest_sha256.into(),
            private_session_sha256: private_session_sha256.into(),
        };
        authenticate_binding_v1(&binding)?;
        Ok(binding)
    }
}

/// Complete account state, including the bytes rather than only their digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectSetupAccountPoststateV1 {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data_base64: String,
    pub(crate) data_sha256: String,
}

impl DirectSetupAccountPoststateV1 {
    pub(crate) fn new(
        address: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: &[u8],
    ) -> Self {
        Self {
            address: address.to_string(),
            owner: owner.to_string(),
            lamports,
            executable,
            data_base64: BASE64.encode(data),
            data_sha256: sha256_hex(data),
        }
    }

    pub(crate) fn data(&self) -> Result<Vec<u8>> {
        decode_canonical_base64_v1(&self.data_base64, "Direct setup account data")
    }
}

/// Exact return-data producer and body.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectSetupReturnDataV1 {
    pub(crate) producer: String,
    pub(crate) body_base64: String,
    pub(crate) body_sha256: String,
}

impl DirectSetupReturnDataV1 {
    pub(crate) fn new(producer: Pubkey, body: &[u8]) -> Result<Self> {
        if body.is_empty() {
            return Err(refusal("Direct setup return body was empty"));
        }
        Ok(Self {
            producer: producer.to_string(),
            body_base64: BASE64.encode(body),
            body_sha256: sha256_hex(body),
        })
    }

    pub(crate) fn body(&self) -> Result<Vec<u8>> {
        decode_canonical_base64_v1(&self.body_base64, "Direct setup return body")
    }
}

/// Inputs admitted once while constructing a `Planned` journal.
#[derive(Clone, Debug)]
pub(crate) struct DirectSetupJournalPlanV1 {
    pub(crate) message: VersionedMessage,
    pub(crate) last_valid_block_height: u64,
    pub(crate) exact_fee_lamports: u64,
    pub(crate) expected_signer: Pubkey,
    pub(crate) expected_return_data: Option<DirectSetupReturnDataV1>,
    pub(crate) expected_poststates: Vec<DirectSetupAccountPoststateV1>,
}

/// Finalized transaction observation supplied by the exterior after polling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectSetupFinalizationV1 {
    pub(crate) finalized_slot: u64,
    /// SHA-256 of the exact finalized transaction wire bytes.
    pub(crate) transaction_sha256: String,
    pub(crate) fee_lamports: u64,
    pub(crate) compute_units_consumed: u64,
    pub(crate) return_data: Option<DirectSetupReturnDataV1>,
    pub(crate) poststates: Vec<DirectSetupAccountPoststateV1>,
}

/// One self-authenticating durable setup action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DirectSetupJournalV1 {
    pub(crate) schema: String,
    pub(crate) public_manifest_sha256: String,
    pub(crate) private_session_sha256: String,
    pub(crate) stage: DirectSetupStageV1,
    pub(crate) phase: DirectSetupJournalPhaseV1,
    /// Absent only for replay setup.  Token setup names the finalized replay
    /// journal's exact self digest.
    pub(crate) predecessor_state_sha256: Option<String>,
    pub(crate) message_base64: String,
    pub(crate) message_sha256: String,
    pub(crate) last_valid_block_height: u64,
    pub(crate) exact_fee_lamports: u64,
    pub(crate) expected_wire_bytes: usize,
    pub(crate) unique_message_account_count: usize,
    pub(crate) expected_signer: String,
    pub(crate) expected_return_data: Option<DirectSetupReturnDataV1>,
    pub(crate) expected_poststates: Vec<DirectSetupAccountPoststateV1>,
    pub(crate) signed_packet_base64: Option<String>,
    pub(crate) signed_packet_sha256: Option<String>,
    pub(crate) expected_signature: Option<String>,
    pub(crate) finalized_slot: Option<u64>,
    pub(crate) transaction_sha256: Option<String>,
    pub(crate) fee_lamports: Option<u64>,
    pub(crate) compute_units_consumed: Option<u64>,
    pub(crate) return_data: Option<DirectSetupReturnDataV1>,
    pub(crate) finalized_poststates: Vec<DirectSetupAccountPoststateV1>,
    pub(crate) state_sha256: String,
}

pub(crate) fn plan_direct_replay_setup_journal_v1(
    binding: &DirectSetupManifestBindingV1,
    plan: DirectSetupJournalPlanV1,
) -> Result<DirectSetupJournalV1> {
    if plan.expected_return_data.is_none() {
        return Err(refusal(
            "Direct replay setup omitted its exact return producer/body",
        ));
    }
    build_planned_journal_v1(binding, DirectSetupStageV1::ReplaySetup, None, plan)
}

pub(crate) fn plan_direct_token_setup_journal_v1(
    binding: &DirectSetupManifestBindingV1,
    finalized_replay: &DirectSetupJournalV1,
    plan: DirectSetupJournalPlanV1,
) -> Result<DirectSetupJournalV1> {
    authenticate_direct_setup_journal_v1(binding, finalized_replay, None)?;
    if finalized_replay.stage != DirectSetupStageV1::ReplaySetup
        || finalized_replay.phase != DirectSetupJournalPhaseV1::Finalized
    {
        return Err(refusal(
            "Direct token setup predecessor was not finalized replay setup",
        ));
    }
    if plan.expected_return_data.is_none() {
        return Err(refusal(
            "Direct token setup omitted its exact return producer/body",
        ));
    }
    build_planned_journal_v1(
        binding,
        DirectSetupStageV1::TokenSetup,
        Some(finalized_replay.state_sha256.clone()),
        plan,
    )
}

fn build_planned_journal_v1(
    binding: &DirectSetupManifestBindingV1,
    stage: DirectSetupStageV1,
    predecessor_state_sha256: Option<String>,
    plan: DirectSetupJournalPlanV1,
) -> Result<DirectSetupJournalV1> {
    authenticate_binding_v1(binding)?;
    if plan.last_valid_block_height == 0 {
        return Err(refusal("Direct setup last-valid block height was zero"));
    }
    if plan.expected_poststates.is_empty() {
        return Err(refusal("Direct setup omitted complete expected poststates"));
    }
    authenticate_poststates_v1(&plan.expected_poststates, "expected")?;
    if let Some(expected) = &plan.expected_return_data {
        authenticate_return_data_v1(expected, "expected")?;
    }

    let message_bytes = plan.message.serialize();
    let (unique_message_account_count, required_signatures) =
        authenticate_message_v1(&plan.message, plan.expected_signer)?;
    if required_signatures != 1 {
        return Err(refusal("Direct setup message did not have one signer"));
    }
    let expected_wire_bytes = bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default()],
        message: plan.message,
    })
    .map_err(|error| Error::new(format!("Direct setup packet geometry: {error}")))?
    .len();
    let mut journal = DirectSetupJournalV1 {
        schema: DIRECT_SETUP_JOURNAL_SCHEMA_V1.into(),
        public_manifest_sha256: binding.public_manifest_sha256.clone(),
        private_session_sha256: binding.private_session_sha256.clone(),
        stage,
        phase: DirectSetupJournalPhaseV1::Planned,
        predecessor_state_sha256,
        message_base64: BASE64.encode(&message_bytes),
        message_sha256: sha256_hex(&message_bytes),
        last_valid_block_height: plan.last_valid_block_height,
        exact_fee_lamports: plan.exact_fee_lamports,
        expected_wire_bytes,
        unique_message_account_count,
        expected_signer: plan.expected_signer.to_string(),
        expected_return_data: plan.expected_return_data,
        expected_poststates: plan.expected_poststates,
        signed_packet_base64: None,
        signed_packet_sha256: None,
        expected_signature: None,
        finalized_slot: None,
        transaction_sha256: None,
        fee_lamports: None,
        compute_units_consumed: None,
        return_data: None,
        finalized_poststates: Vec::new(),
        state_sha256: String::new(),
    };
    refresh_direct_setup_journal_digest_v1(&mut journal)?;
    authenticate_direct_setup_journal_self_v1(binding, &journal)?;
    Ok(journal)
}

/// Authenticate a journal against independently reconstructed manifest roots
/// and, for token setup, its exact finalized replay predecessor.
pub(crate) fn authenticate_direct_setup_journal_v1(
    binding: &DirectSetupManifestBindingV1,
    journal: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
) -> Result<()> {
    authenticate_direct_setup_journal_self_v1(binding, journal)?;
    match (journal.stage, predecessor) {
        (DirectSetupStageV1::ReplaySetup, None) => {
            if journal.predecessor_state_sha256.is_some() {
                return Err(refusal("Direct replay setup had a predecessor"));
            }
        }
        (DirectSetupStageV1::ReplaySetup, Some(_)) => {
            return Err(refusal(
                "Direct replay setup was reordered after another stage",
            ));
        }
        (DirectSetupStageV1::TokenSetup, None) => {
            return Err(refusal("Direct token setup omitted its replay predecessor"));
        }
        (DirectSetupStageV1::TokenSetup, Some(previous)) => {
            authenticate_direct_setup_journal_self_v1(binding, previous)?;
            if previous.stage != DirectSetupStageV1::ReplaySetup
                || previous.phase != DirectSetupJournalPhaseV1::Finalized
                || journal.predecessor_state_sha256.as_deref()
                    != Some(previous.state_sha256.as_str())
            {
                return Err(refusal(
                    "Direct token setup predecessor order or digest changed",
                ));
            }
        }
    }
    Ok(())
}

/// Compare a loaded journal to the independently reconstructed `Planned`
/// intent.  Progress fields may advance, but no admitted byte may change.
pub(crate) fn authenticate_direct_setup_against_plan_v1(
    binding: &DirectSetupManifestBindingV1,
    journal: &DirectSetupJournalV1,
    reconstructed_plan: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
) -> Result<()> {
    authenticate_direct_setup_journal_v1(binding, journal, predecessor)?;
    authenticate_direct_setup_journal_v1(binding, reconstructed_plan, predecessor)?;
    if reconstructed_plan.phase != DirectSetupJournalPhaseV1::Planned
        || !same_durable_intent_v1(journal, reconstructed_plan)
    {
        return Err(refusal(
            "Direct setup journal differed from the reconstructed durable intent",
        ));
    }
    Ok(())
}

pub(crate) fn authenticate_direct_setup_chain_v1(
    binding: &DirectSetupManifestBindingV1,
    replay: &DirectSetupJournalV1,
    token: &DirectSetupJournalV1,
) -> Result<()> {
    authenticate_direct_setup_journal_v1(binding, replay, None)?;
    authenticate_direct_setup_journal_v1(binding, token, Some(replay))
}

pub(crate) fn direct_setup_recovery_action_v1(
    binding: &DirectSetupManifestBindingV1,
    journal: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
) -> Result<DirectSetupRecoveryActionV1> {
    authenticate_direct_setup_journal_v1(binding, journal, predecessor)?;
    Ok(match journal.phase {
        DirectSetupJournalPhaseV1::Planned => DirectSetupRecoveryActionV1::SignAndPrepare,
        DirectSetupJournalPhaseV1::Prepared => DirectSetupRecoveryActionV1::BeginDispatch,
        DirectSetupJournalPhaseV1::Dispatching => {
            DirectSetupRecoveryActionV1::DispatchIdenticalPacket
        }
        DirectSetupJournalPhaseV1::Submitted => DirectSetupRecoveryActionV1::PollOnly,
        DirectSetupJournalPhaseV1::Finalized => DirectSetupRecoveryActionV1::Complete,
    })
}

/// Admit an externally signed packet without reading or opening any key.
pub(crate) fn advance_direct_setup_signed_v1(
    binding: &DirectSetupManifestBindingV1,
    current: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
    signed_packet: &[u8],
) -> Result<DirectSetupJournalV1> {
    authenticate_direct_setup_journal_v1(binding, current, predecessor)?;
    if current.phase != DirectSetupJournalPhaseV1::Planned {
        return Err(refusal("Direct setup could only be signed from Planned"));
    }
    let transaction = authenticate_signed_packet_bytes_v1(current, signed_packet)?;
    let mut next = current.clone();
    next.phase = DirectSetupJournalPhaseV1::Prepared;
    next.signed_packet_base64 = Some(BASE64.encode(signed_packet));
    next.signed_packet_sha256 = Some(sha256_hex(signed_packet));
    next.expected_signature = transaction.signatures.first().map(ToString::to_string);
    refresh_direct_setup_journal_digest_v1(&mut next)?;
    authenticate_direct_setup_transition_v1(binding, current, &next, predecessor)?;
    Ok(next)
}

/// Arm the exact prepared packet for identical-byte dispatch recovery.
pub(crate) fn advance_direct_setup_dispatching_v1(
    binding: &DirectSetupManifestBindingV1,
    current: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
) -> Result<DirectSetupJournalV1> {
    authenticate_direct_setup_journal_v1(binding, current, predecessor)?;
    if current.phase != DirectSetupJournalPhaseV1::Prepared {
        return Err(refusal(
            "Direct setup could only arm Dispatching after Prepared",
        ));
    }
    let mut next = current.clone();
    next.phase = DirectSetupJournalPhaseV1::Dispatching;
    refresh_direct_setup_journal_digest_v1(&mut next)?;
    authenticate_direct_setup_transition_v1(binding, current, &next, predecessor)?;
    Ok(next)
}

/// Record RPC acceptance after dispatch of the exact armed packet.
pub(crate) fn advance_direct_setup_submitted_v1(
    binding: &DirectSetupManifestBindingV1,
    current: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
) -> Result<DirectSetupJournalV1> {
    authenticate_direct_setup_journal_v1(binding, current, predecessor)?;
    if current.phase != DirectSetupJournalPhaseV1::Dispatching {
        return Err(refusal(
            "Direct setup could only record submission after Dispatching",
        ));
    }
    let mut next = current.clone();
    next.phase = DirectSetupJournalPhaseV1::Submitted;
    refresh_direct_setup_journal_digest_v1(&mut next)?;
    authenticate_direct_setup_transition_v1(binding, current, &next, predecessor)?;
    Ok(next)
}

pub(crate) fn advance_direct_setup_finalized_v1(
    binding: &DirectSetupManifestBindingV1,
    current: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
    finalized: DirectSetupFinalizationV1,
) -> Result<DirectSetupJournalV1> {
    authenticate_direct_setup_journal_v1(binding, current, predecessor)?;
    if current.phase != DirectSetupJournalPhaseV1::Submitted {
        return Err(refusal(
            "Direct setup could only finalize the durable Submitted phase",
        ));
    }
    if finalized.finalized_slot == 0 {
        return Err(refusal("Direct setup finalized slot was zero"));
    }
    require_hex32_v1(
        &finalized.transaction_sha256,
        "Direct setup finalized transaction digest",
    )?;
    if finalized.transaction_sha256 != current.signed_packet_sha256.as_deref().unwrap_or_default()
        || finalized.fee_lamports != current.exact_fee_lamports
        || finalized.return_data != current.expected_return_data
        || finalized.poststates != current.expected_poststates
    {
        return Err(refusal(
            "Direct setup finalized fee, transaction, return data, or poststates changed",
        ));
    }
    if let Some(return_data) = &finalized.return_data {
        authenticate_return_data_v1(return_data, "finalized")?;
    }
    authenticate_poststates_v1(&finalized.poststates, "finalized")?;

    let mut next = current.clone();
    next.phase = DirectSetupJournalPhaseV1::Finalized;
    next.finalized_slot = Some(finalized.finalized_slot);
    next.transaction_sha256 = Some(finalized.transaction_sha256);
    next.fee_lamports = Some(finalized.fee_lamports);
    next.compute_units_consumed = Some(finalized.compute_units_consumed);
    next.return_data = finalized.return_data;
    next.finalized_poststates = finalized.poststates;
    refresh_direct_setup_journal_digest_v1(&mut next)?;
    authenticate_direct_setup_transition_v1(binding, current, &next, predecessor)?;
    Ok(next)
}

/// Authenticate exactly one adjacent state advance.  Phase omission and any
/// plan-byte substitution are refused even when the attacker recomputes the
/// journal's self digest.
pub(crate) fn authenticate_direct_setup_transition_v1(
    binding: &DirectSetupManifestBindingV1,
    previous: &DirectSetupJournalV1,
    next: &DirectSetupJournalV1,
    predecessor: Option<&DirectSetupJournalV1>,
) -> Result<()> {
    authenticate_direct_setup_journal_v1(binding, previous, predecessor)?;
    authenticate_direct_setup_journal_v1(binding, next, predecessor)?;
    if !same_durable_intent_v1(previous, next) {
        return Err(refusal(
            "Direct setup durable intent changed across a transition",
        ));
    }
    match (previous.phase, next.phase) {
        (DirectSetupJournalPhaseV1::Planned, DirectSetupJournalPhaseV1::Prepared) => {
            require_no_finalized_evidence_v1(next)?;
            authenticate_signed_packet_v1(next)?;
        }
        (DirectSetupJournalPhaseV1::Prepared, DirectSetupJournalPhaseV1::Dispatching)
        | (DirectSetupJournalPhaseV1::Dispatching, DirectSetupJournalPhaseV1::Submitted) => {
            if signed_evidence_v1(previous) != signed_evidence_v1(next) {
                return Err(refusal("Direct setup signed bytes changed at submission"));
            }
            require_no_finalized_evidence_v1(next)?;
        }
        (DirectSetupJournalPhaseV1::Submitted, DirectSetupJournalPhaseV1::Finalized) => {
            if signed_evidence_v1(previous) != signed_evidence_v1(next) {
                return Err(refusal("Direct setup signed bytes changed at finalization"));
            }
        }
        _ => {
            return Err(refusal(
                "Direct setup transition skipped, repeated, or reversed a durable phase",
            ));
        }
    }
    Ok(())
}

fn authenticate_direct_setup_journal_self_v1(
    binding: &DirectSetupManifestBindingV1,
    journal: &DirectSetupJournalV1,
) -> Result<()> {
    authenticate_binding_v1(binding)?;
    if journal.schema != DIRECT_SETUP_JOURNAL_SCHEMA_V1
        || journal.public_manifest_sha256 != binding.public_manifest_sha256
        || journal.private_session_sha256 != binding.private_session_sha256
        || journal.state_sha256 != direct_setup_journal_digest_v1(journal)?
        || journal.last_valid_block_height == 0
        || journal.expected_poststates.is_empty()
    {
        return Err(refusal(
            "Direct setup journal identity or self digest changed",
        ));
    }
    match journal.stage {
        DirectSetupStageV1::ReplaySetup => {
            if journal.predecessor_state_sha256.is_some() || journal.expected_return_data.is_none()
            {
                return Err(refusal("Direct replay setup durable intent shape changed"));
            }
        }
        DirectSetupStageV1::TokenSetup => {
            let predecessor = journal
                .predecessor_state_sha256
                .as_deref()
                .ok_or_else(|| refusal("Direct token setup omitted predecessor digest"))?;
            require_hex32_v1(predecessor, "Direct token predecessor digest")?;
            if journal.expected_return_data.is_none() {
                return Err(refusal("Direct token setup omitted return data"));
            }
        }
    }
    if let Some(expected) = &journal.expected_return_data {
        authenticate_return_data_v1(expected, "expected")?;
    }
    authenticate_poststates_v1(&journal.expected_poststates, "expected")?;

    let message_bytes =
        decode_canonical_base64_v1(&journal.message_base64, "Direct setup versioned message")?;
    if sha256_hex(&message_bytes) != journal.message_sha256 {
        return Err(refusal("Direct setup message digest changed"));
    }
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("Direct setup versioned message: {error}")))?;
    if message.serialize() != message_bytes {
        return Err(refusal("Direct setup versioned message was not canonical"));
    }
    let expected_signer = parse_pubkey_v1(&journal.expected_signer, "Direct setup signer")?;
    let (unique, required_signatures) = authenticate_message_v1(&message, expected_signer)?;
    let expected_wire = bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default(); required_signatures],
        message,
    })
    .map_err(|error| Error::new(format!("Direct setup packet geometry: {error}")))?
    .len();
    if required_signatures != 1
        || unique != journal.unique_message_account_count
        || expected_wire != journal.expected_wire_bytes
    {
        return Err(refusal(
            "Direct setup signer, wire width, or unique account geometry changed",
        ));
    }

    match journal.phase {
        DirectSetupJournalPhaseV1::Planned => {
            if signed_evidence_v1(journal).iter().any(Option::is_some) {
                return Err(refusal("Planned Direct setup carried signed evidence"));
            }
            require_no_finalized_evidence_v1(journal)?;
        }
        DirectSetupJournalPhaseV1::Prepared
        | DirectSetupJournalPhaseV1::Dispatching
        | DirectSetupJournalPhaseV1::Submitted => {
            authenticate_signed_packet_v1(journal)?;
            require_no_finalized_evidence_v1(journal)?;
        }
        DirectSetupJournalPhaseV1::Finalized => {
            authenticate_signed_packet_v1(journal)?;
            if journal.finalized_slot.is_none_or(|slot| slot == 0)
                || journal.transaction_sha256 != journal.signed_packet_sha256
                || journal.fee_lamports != Some(journal.exact_fee_lamports)
                || journal.compute_units_consumed.is_none()
                || journal.return_data != journal.expected_return_data
                || journal.finalized_poststates != journal.expected_poststates
            {
                return Err(refusal("Finalized Direct setup evidence changed"));
            }
            if let Some(return_data) = &journal.return_data {
                authenticate_return_data_v1(return_data, "finalized")?;
            }
            authenticate_poststates_v1(&journal.finalized_poststates, "finalized")?;
        }
    }
    Ok(())
}

fn authenticate_message_v1(
    message: &VersionedMessage,
    expected_signer: Pubkey,
) -> Result<(usize, usize)> {
    let (header, keys, has_lookups) = match message {
        VersionedMessage::Legacy(message) => {
            (&message.header, message.account_keys.as_slice(), false)
        }
        VersionedMessage::V0(message) => (
            &message.header,
            message.account_keys.as_slice(),
            !message.address_table_lookups.is_empty(),
        ),
    };
    if has_lookups {
        return Err(refusal(
            "Direct setup message unexpectedly depended on a lookup table",
        ));
    }
    let required_signatures = usize::from(header.num_required_signatures);
    if required_signatures != 1 || keys.first() != Some(&expected_signer) {
        return Err(refusal("Direct setup message signer closure changed"));
    }
    let unique = keys.iter().copied().collect::<BTreeSet<_>>().len();
    if unique != keys.len() || unique == 0 || unique > 64 {
        return Err(refusal(
            "Direct setup message keys were duplicated, empty, or above the lock limit",
        ));
    }
    Ok((unique, required_signatures))
}

fn authenticate_signed_packet_v1(journal: &DirectSetupJournalV1) -> Result<()> {
    let encoded = journal
        .signed_packet_base64
        .as_deref()
        .ok_or_else(|| refusal("Signed Direct setup omitted packet bytes"))?;
    let packet = decode_canonical_base64_v1(encoded, "Direct setup signed packet")?;
    if journal.signed_packet_sha256.as_deref() != Some(sha256_hex(&packet).as_str()) {
        return Err(refusal("Direct setup signed packet digest changed"));
    }
    let transaction = authenticate_signed_packet_bytes_v1(journal, &packet)?;
    if journal.expected_signature.as_deref()
        != transaction
            .signatures
            .first()
            .map(ToString::to_string)
            .as_deref()
    {
        return Err(refusal("Direct setup expected signature changed"));
    }
    Ok(())
}

fn authenticate_signed_packet_bytes_v1(
    journal: &DirectSetupJournalV1,
    packet: &[u8],
) -> Result<VersionedTransaction> {
    if packet.len() != journal.expected_wire_bytes {
        return Err(refusal("Direct setup signed packet wire width changed"));
    }
    let transaction: VersionedTransaction = bincode::deserialize(packet)
        .map_err(|error| Error::new(format!("Direct setup signed packet: {error}")))?;
    if bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("Direct setup signed packet reencode: {error}")))?
        != packet
    {
        return Err(refusal("Direct setup signed packet was not canonical"));
    }
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("Direct setup signed packet signatures: {error}")))?;
    let message_bytes = transaction.message.serialize();
    if BASE64.encode(&message_bytes) != journal.message_base64
        || sha256_hex(&message_bytes) != journal.message_sha256
        || transaction.signatures.len() != 1
    {
        return Err(refusal(
            "Direct setup signed packet message or signer count changed",
        ));
    }
    Ok(transaction)
}

fn authenticate_poststates_v1(states: &[DirectSetupAccountPoststateV1], label: &str) -> Result<()> {
    if states.is_empty() {
        return Err(refusal(format!(
            "Direct setup {label} poststates were empty"
        )));
    }
    let mut addresses = BTreeSet::new();
    for state in states {
        let address = parse_pubkey_v1(&state.address, "Direct setup poststate address")?;
        parse_pubkey_v1(&state.owner, "Direct setup poststate owner")?;
        if !addresses.insert(address) {
            return Err(refusal(format!(
                "Direct setup {label} poststates duplicated an address"
            )));
        }
        let data = state.data()?;
        if state.data_sha256 != sha256_hex(&data) {
            return Err(refusal(format!(
                "Direct setup {label} poststate data digest changed"
            )));
        }
    }
    Ok(())
}

fn authenticate_return_data_v1(value: &DirectSetupReturnDataV1, label: &str) -> Result<()> {
    parse_pubkey_v1(&value.producer, "Direct setup return producer")?;
    let body = value.body()?;
    if body.is_empty() || value.body_sha256 != sha256_hex(&body) {
        return Err(refusal(format!(
            "Direct setup {label} return body or digest changed"
        )));
    }
    Ok(())
}

fn authenticate_binding_v1(binding: &DirectSetupManifestBindingV1) -> Result<()> {
    require_hex32_v1(
        &binding.public_manifest_sha256,
        "Direct setup public manifest digest",
    )?;
    require_hex32_v1(
        &binding.private_session_sha256,
        "Direct setup private session digest",
    )?;
    if binding.public_manifest_sha256 == binding.private_session_sha256 {
        return Err(refusal(
            "Direct setup public and private manifest identities collided",
        ));
    }
    Ok(())
}

fn same_durable_intent_v1(left: &DirectSetupJournalV1, right: &DirectSetupJournalV1) -> bool {
    left.schema == right.schema
        && left.public_manifest_sha256 == right.public_manifest_sha256
        && left.private_session_sha256 == right.private_session_sha256
        && left.stage == right.stage
        && left.predecessor_state_sha256 == right.predecessor_state_sha256
        && left.message_base64 == right.message_base64
        && left.message_sha256 == right.message_sha256
        && left.last_valid_block_height == right.last_valid_block_height
        && left.exact_fee_lamports == right.exact_fee_lamports
        && left.expected_wire_bytes == right.expected_wire_bytes
        && left.unique_message_account_count == right.unique_message_account_count
        && left.expected_signer == right.expected_signer
        && left.expected_return_data == right.expected_return_data
        && left.expected_poststates == right.expected_poststates
}

fn signed_evidence_v1(journal: &DirectSetupJournalV1) -> [Option<&str>; 3] {
    [
        journal.signed_packet_base64.as_deref(),
        journal.signed_packet_sha256.as_deref(),
        journal.expected_signature.as_deref(),
    ]
}

fn require_no_finalized_evidence_v1(journal: &DirectSetupJournalV1) -> Result<()> {
    if journal.finalized_slot.is_some()
        || journal.transaction_sha256.is_some()
        || journal.fee_lamports.is_some()
        || journal.compute_units_consumed.is_some()
        || journal.return_data.is_some()
        || !journal.finalized_poststates.is_empty()
    {
        return Err(refusal(
            "Non-finalized Direct setup carried finalized-only evidence",
        ));
    }
    Ok(())
}

fn refresh_direct_setup_journal_digest_v1(journal: &mut DirectSetupJournalV1) -> Result<()> {
    journal.state_sha256.clear();
    journal.state_sha256 = direct_setup_journal_digest_v1(journal)?;
    Ok(())
}

fn direct_setup_journal_digest_v1(journal: &DirectSetupJournalV1) -> Result<String> {
    let mut canonical = journal.clone();
    canonical.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn decode_canonical_base64_v1(value: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label} base64: {error}")))?;
    if BASE64.encode(&bytes) != value {
        return Err(refusal(format!("{label} was not canonical base64")));
    }
    Ok(bytes)
}

fn parse_pubkey_v1(value: &str, label: &str) -> Result<Pubkey> {
    let key = Pubkey::from_str(value).map_err(|error| Error::new(format!("{label}: {error}")))?;
    if key.to_string() != value {
        return Err(refusal(format!("{label} was not canonical")));
    }
    Ok(key)
}

fn require_hex32_v1(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(refusal(format!(
            "{label} was not canonical lowercase hex32"
        )));
    }
    Ok(())
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

fn refusal(reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: {}", reason.as_ref()))
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use solana_hash::Hash;
    use solana_program::pubkey::Pubkey;
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::{Message, VersionedMessage},
        signature::{Keypair, Signature, Signer as _},
        transaction::VersionedTransaction,
    };
    use solana_sdk_ids::system_program;

    use super::{
        DirectSetupAccountPoststateV1, DirectSetupFinalizationV1, DirectSetupJournalPhaseV1,
        DirectSetupJournalPlanV1, DirectSetupJournalV1, DirectSetupManifestBindingV1,
        DirectSetupRecoveryActionV1, DirectSetupReturnDataV1, DirectSetupStageV1,
        advance_direct_setup_dispatching_v1, advance_direct_setup_finalized_v1,
        advance_direct_setup_signed_v1, advance_direct_setup_submitted_v1,
        authenticate_direct_setup_against_plan_v1, authenticate_direct_setup_chain_v1,
        authenticate_direct_setup_journal_v1, authenticate_direct_setup_transition_v1,
        direct_setup_recovery_action_v1, plan_direct_replay_setup_journal_v1,
        plan_direct_token_setup_journal_v1, refresh_direct_setup_journal_digest_v1, sha256_hex,
    };

    fn binding() -> DirectSetupManifestBindingV1 {
        DirectSetupManifestBindingV1::new("11".repeat(32), "22".repeat(32)).unwrap()
    }

    fn message(payer: &Keypair, discriminator: u8) -> VersionedMessage {
        VersionedMessage::Legacy(Message::new_with_blockhash(
            &[Instruction {
                program_id: system_program::id(),
                accounts: vec![AccountMeta::new(payer.pubkey(), true)],
                data: vec![discriminator],
            }],
            Some(&payer.pubkey()),
            &Hash::new_unique(),
        ))
    }

    fn state(tag: u8) -> DirectSetupAccountPoststateV1 {
        DirectSetupAccountPoststateV1::new(
            Pubkey::new_from_array([tag; 32]),
            system_program::id(),
            u64::from(tag) + 1,
            false,
            &[tag, tag.wrapping_add(1)],
        )
    }

    fn replay_plan(payer: &Keypair) -> (DirectSetupJournalV1, VersionedMessage) {
        let message = message(payer, 1);
        let plan = plan_direct_replay_setup_journal_v1(
            &binding(),
            DirectSetupJournalPlanV1 {
                message: message.clone(),
                last_valid_block_height: 99,
                exact_fee_lamports: 5_000,
                expected_signer: payer.pubkey(),
                expected_return_data: Some(
                    DirectSetupReturnDataV1::new(Pubkey::new_from_array([9; 32]), &[7; 16])
                        .unwrap(),
                ),
                expected_poststates: vec![state(3), state(4)],
            },
        )
        .unwrap();
        (plan, message)
    }

    fn signed(
        planned: &DirectSetupJournalV1,
        message: VersionedMessage,
        payer: &Keypair,
        predecessor: Option<&DirectSetupJournalV1>,
    ) -> DirectSetupJournalV1 {
        let packet = bincode::serialize(
            &VersionedTransaction::try_new(message, &[payer]).expect("sign fixture"),
        )
        .unwrap();
        advance_direct_setup_signed_v1(&binding(), planned, predecessor, &packet).unwrap()
    }

    fn dispatching(
        prepared: &DirectSetupJournalV1,
        predecessor: Option<&DirectSetupJournalV1>,
    ) -> DirectSetupJournalV1 {
        advance_direct_setup_dispatching_v1(&binding(), prepared, predecessor).unwrap()
    }

    fn submitted(
        dispatching: &DirectSetupJournalV1,
        predecessor: Option<&DirectSetupJournalV1>,
    ) -> DirectSetupJournalV1 {
        advance_direct_setup_submitted_v1(&binding(), dispatching, predecessor).unwrap()
    }

    fn finalized_replay(payer: &Keypair) -> DirectSetupJournalV1 {
        let (planned, message) = replay_plan(payer);
        let signed = signed(&planned, message, payer, None);
        let dispatching = dispatching(&signed, None);
        let submitted = submitted(&dispatching, None);
        advance_direct_setup_finalized_v1(
            &binding(),
            &submitted,
            None,
            DirectSetupFinalizationV1 {
                finalized_slot: 55,
                transaction_sha256: submitted.signed_packet_sha256.clone().unwrap(),
                fee_lamports: submitted.exact_fee_lamports,
                compute_units_consumed: 321,
                return_data: submitted.expected_return_data.clone(),
                poststates: submitted.expected_poststates.clone(),
            },
        )
        .unwrap()
    }

    fn token_plan(
        payer: &Keypair,
        replay: &DirectSetupJournalV1,
    ) -> (DirectSetupJournalV1, VersionedMessage) {
        let message = message(payer, 2);
        let plan = plan_direct_token_setup_journal_v1(
            &binding(),
            replay,
            DirectSetupJournalPlanV1 {
                message: message.clone(),
                last_valid_block_height: 199,
                exact_fee_lamports: 5_000,
                expected_signer: payer.pubkey(),
                expected_return_data: Some(
                    DirectSetupReturnDataV1::new(Pubkey::new_from_array([10; 32]), &[8; 16])
                        .unwrap(),
                ),
                expected_poststates: vec![state(5), state(6), state(7)],
            },
        )
        .unwrap();
        (plan, message)
    }

    #[test]
    fn exact_two_stage_lifecycle_has_identical_resend_then_poll_only_recovery() {
        let payer = Keypair::new();
        let (planned, message) = replay_plan(&payer);
        assert_eq!(
            direct_setup_recovery_action_v1(&binding(), &planned, None).unwrap(),
            DirectSetupRecoveryActionV1::SignAndPrepare
        );
        let signed = signed(&planned, message, &payer, None);
        assert_eq!(
            direct_setup_recovery_action_v1(&binding(), &signed, None).unwrap(),
            DirectSetupRecoveryActionV1::BeginDispatch
        );
        let dispatching = dispatching(&signed, None);
        assert_eq!(
            direct_setup_recovery_action_v1(&binding(), &dispatching, None).unwrap(),
            DirectSetupRecoveryActionV1::DispatchIdenticalPacket
        );
        let submitted = submitted(&dispatching, None);
        assert_eq!(
            direct_setup_recovery_action_v1(&binding(), &submitted, None).unwrap(),
            DirectSetupRecoveryActionV1::PollOnly
        );
        let replay = advance_direct_setup_finalized_v1(
            &binding(),
            &submitted,
            None,
            DirectSetupFinalizationV1 {
                finalized_slot: 55,
                transaction_sha256: submitted.signed_packet_sha256.clone().unwrap(),
                fee_lamports: 5_000,
                compute_units_consumed: 321,
                return_data: submitted.expected_return_data.clone(),
                poststates: submitted.expected_poststates.clone(),
            },
        )
        .unwrap();
        assert_eq!(
            direct_setup_recovery_action_v1(&binding(), &replay, None).unwrap(),
            DirectSetupRecoveryActionV1::Complete
        );
        let (token, _) = token_plan(&payer, &replay);
        authenticate_direct_setup_chain_v1(&binding(), &replay, &token).unwrap();
    }

    #[test]
    fn manifest_message_signer_return_and_poststate_substitutions_are_refused() {
        let payer = Keypair::new();
        let (planned, _) = replay_plan(&payer);

        let hostile_binding =
            DirectSetupManifestBindingV1::new("33".repeat(32), "22".repeat(32)).unwrap();
        assert!(authenticate_direct_setup_journal_v1(&hostile_binding, &planned, None).is_err());

        for case in 0..5 {
            let mut hostile = planned.clone();
            match case {
                0 => hostile.message_base64.push('A'),
                1 => hostile.expected_signer = Pubkey::new_unique().to_string(),
                2 => hostile.expected_return_data.as_mut().unwrap().body_sha256 = "aa".repeat(32),
                3 => hostile.expected_poststates[0].owner = Pubkey::new_unique().to_string(),
                _ => hostile.expected_poststates.swap(0, 1),
            }
            refresh_direct_setup_journal_digest_v1(&mut hostile).unwrap();
            assert!(
                authenticate_direct_setup_against_plan_v1(&binding(), &hostile, &planned, None)
                    .is_err(),
                "hostile case {case}"
            );
        }
    }

    #[test]
    fn signed_packet_and_signature_substitution_are_refused() {
        let payer = Keypair::new();
        let (planned, message) = replay_plan(&payer);
        let signed = signed(&planned, message, &payer, None);
        for case in 0..3 {
            let mut hostile = signed.clone();
            match case {
                0 => hostile.signed_packet_sha256 = Some("aa".repeat(32)),
                1 => hostile.expected_signature = Some(Signature::default().to_string()),
                _ => hostile.signed_packet_base64.as_mut().unwrap().push('A'),
            }
            refresh_direct_setup_journal_digest_v1(&mut hostile).unwrap();
            assert!(
                authenticate_direct_setup_journal_v1(&binding(), &hostile, None).is_err(),
                "hostile case {case}"
            );
        }
    }

    #[test]
    fn phase_omission_repeat_reverse_and_blind_resign_are_refused() {
        let payer = Keypair::new();
        let (planned, message) = replay_plan(&payer);
        let signed = signed(&planned, message, &payer, None);
        let dispatching = dispatching(&signed, None);
        let submitted = submitted(&dispatching, None);
        let finalized = finalized_replay(&payer);

        assert!(
            authenticate_direct_setup_transition_v1(&binding(), &planned, &dispatching, None)
                .is_err()
        );
        assert!(
            authenticate_direct_setup_transition_v1(&binding(), &submitted, &finalized, None)
                .is_err()
        );
        assert!(
            authenticate_direct_setup_transition_v1(&binding(), &signed, &signed, None).is_err()
        );
        assert!(
            authenticate_direct_setup_transition_v1(&binding(), &submitted, &dispatching, None)
                .is_err()
        );
        let packet = BASE64
            .decode(signed.signed_packet_base64.as_ref().unwrap())
            .unwrap();
        assert!(advance_direct_setup_signed_v1(&binding(), &signed, None, &packet).is_err());
    }

    #[test]
    fn replay_token_reordering_and_predecessor_substitution_are_refused() {
        let payer = Keypair::new();
        let replay = finalized_replay(&payer);
        let (token, _) = token_plan(&payer, &replay);
        assert!(authenticate_direct_setup_journal_v1(&binding(), &token, None).is_err());
        assert!(authenticate_direct_setup_journal_v1(&binding(), &replay, Some(&token)).is_err());

        let mut hostile = token.clone();
        hostile.predecessor_state_sha256 = Some("aa".repeat(32));
        refresh_direct_setup_journal_digest_v1(&mut hostile).unwrap();
        assert!(authenticate_direct_setup_journal_v1(&binding(), &hostile, Some(&replay)).is_err());

        let mut unfinished = replay.clone();
        unfinished.phase = DirectSetupJournalPhaseV1::Submitted;
        unfinished.finalized_slot = None;
        unfinished.transaction_sha256 = None;
        unfinished.fee_lamports = None;
        unfinished.compute_units_consumed = None;
        unfinished.return_data = None;
        unfinished.finalized_poststates.clear();
        refresh_direct_setup_journal_digest_v1(&mut unfinished).unwrap();
        assert!(
            plan_direct_token_setup_journal_v1(
                &binding(),
                &unfinished,
                DirectSetupJournalPlanV1 {
                    message: message(&payer, 2),
                    last_valid_block_height: 199,
                    exact_fee_lamports: 5_000,
                    expected_signer: payer.pubkey(),
                    expected_return_data: None,
                    expected_poststates: vec![state(5)],
                }
            )
            .is_err()
        );
    }

    #[test]
    fn finalized_transaction_fee_cu_return_and_each_complete_poststate_are_exact() {
        let payer = Keypair::new();
        let (planned, message) = replay_plan(&payer);
        let signed = signed(&planned, message, &payer, None);
        let dispatching = dispatching(&signed, None);
        let submitted = submitted(&dispatching, None);
        let exact = DirectSetupFinalizationV1 {
            finalized_slot: 55,
            transaction_sha256: submitted.signed_packet_sha256.clone().unwrap(),
            fee_lamports: submitted.exact_fee_lamports,
            compute_units_consumed: 321,
            return_data: submitted.expected_return_data.clone(),
            poststates: submitted.expected_poststates.clone(),
        };
        assert!(
            advance_direct_setup_finalized_v1(&binding(), &submitted, None, exact.clone()).is_ok()
        );

        for case in 0..8 {
            let mut hostile = exact.clone();
            match case {
                0 => hostile.finalized_slot = 0,
                1 => hostile.transaction_sha256 = "aa".repeat(32),
                2 => hostile.fee_lamports += 1,
                3 => {
                    hostile.return_data.as_mut().unwrap().producer =
                        Pubkey::new_unique().to_string()
                }
                4 => hostile.return_data.as_mut().unwrap().body_base64 = BASE64.encode([1; 16]),
                5 => hostile.poststates[0].lamports += 1,
                6 => hostile.poststates[0].executable = !hostile.poststates[0].executable,
                _ => hostile.poststates.swap(0, 1),
            }
            assert!(
                advance_direct_setup_finalized_v1(&binding(), &submitted, None, hostile).is_err(),
                "hostile case {case}"
            );
        }

        // CU is evidence rather than a precommitted economic fact, but it may
        // not be omitted in the finalized journal shape.
        let finalized =
            advance_direct_setup_finalized_v1(&binding(), &submitted, None, exact).unwrap();
        let mut omitted_cu = finalized;
        omitted_cu.compute_units_consumed = None;
        refresh_direct_setup_journal_digest_v1(&mut omitted_cu).unwrap();
        assert!(authenticate_direct_setup_journal_v1(&binding(), &omitted_cu, None).is_err());
    }

    #[test]
    fn noncanonical_hash_base64_duplicate_accounts_and_lookup_messages_are_refused() {
        assert!(DirectSetupManifestBindingV1::new("AA".repeat(32), "22".repeat(32)).is_err());
        let payer = Keypair::new();
        let (planned, _) = replay_plan(&payer);
        let mut hostile = planned.clone();
        hostile
            .expected_poststates
            .push(hostile.expected_poststates[0].clone());
        refresh_direct_setup_journal_digest_v1(&mut hostile).unwrap();
        assert!(authenticate_direct_setup_journal_v1(&binding(), &hostile, None).is_err());

        let mut hostile = planned;
        hostile.expected_poststates[0].data_base64.push('A');
        refresh_direct_setup_journal_digest_v1(&mut hostile).unwrap();
        assert!(authenticate_direct_setup_journal_v1(&binding(), &hostile, None).is_err());
    }

    #[test]
    fn stage_and_phase_serde_are_stable() {
        assert_eq!(
            serde_json::to_string(&DirectSetupStageV1::ReplaySetup).unwrap(),
            "\"replay-setup\""
        );
        assert_eq!(
            serde_json::to_string(&DirectSetupJournalPhaseV1::Prepared).unwrap(),
            "\"prepared\""
        );
        assert_eq!(
            serde_json::to_string(&DirectSetupJournalPhaseV1::Dispatching).unwrap(),
            "\"dispatching\""
        );
        assert_ne!(sha256_hex(b"replay"), sha256_hex(b"token"));
    }
}
