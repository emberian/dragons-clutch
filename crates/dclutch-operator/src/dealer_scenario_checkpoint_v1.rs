//! Unsigned lock-bounded Dealer checkpoint callers and resumable journal.
//!
//! These builders perform no RPC, signing, or submission. They construct the
//! exact Trading instruction, compile a packet-sized v0 message, and report the
//! resolved transaction-lock census before a caller can persist or sign it.

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_capability_program_contract::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
};
use dclutch_dealer_codec::scenario_checkpoint_v1::{
    DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1, DEALER_SCENARIO_PREPARATION_PAGES_V1,
};
use dclutch_trading_sbf::dealer::v3_trade_profile::{
    DEALER_SCENARIO_PROFILE_SPANS_V4, dealer_scenario_logical_frame_v4,
};
use dclutch_trading_sbf::dealer_scenario_checkpoint_v1::{
    DEALER_SCENARIO_CHECKPOINT_CLEANUP_MAGIC_V1, DEALER_SCENARIO_CHECKPOINT_CREATE_MAGIC_V1,
    DEALER_SCENARIO_CHECKPOINT_EVALUATE_MAGIC_V1, DEALER_SCENARIO_CHECKPOINT_PAGE_MAGIC_V1,
};
use solana_hash::Hash;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::dealer_scenario_hot_v4::{
    DealerScenarioHotMetaStateV4, DealerScenarioTransactionLockCensusV1,
    census_dealer_scenario_transaction_locks_v1, require_dealer_scenario_devnet_lock_limit_v1,
};

/// Current serialized transaction packet ceiling.
pub const DEALER_SCENARIO_PACKET_BYTES_V1: usize = 1_232;

/// One transaction in the checkpoint lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioCheckpointRouteV1 {
    /// Create the request-scoped Trading PDA.
    Create,
    /// Append one canonical page ordinal.
    Page(u8),
    /// Seal the producer-bound evaluation receipt.
    Evaluate,
    /// Close after expiry to the immutable beneficiary.
    Cleanup,
}

/// Packet-safe unsigned route and its exact lock/signer geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioCheckpointPacketV1 {
    /// Semantic lifecycle route represented by this message.
    pub route: DealerScenarioCheckpointRouteV1,
    /// Exact top-level Trading instruction.
    pub instruction: Instruction,
    /// Unsigned v0 message.
    pub message: VersionedMessage,
    /// Exact fully-signed transaction width.
    pub wire_bytes: usize,
    /// Number of addresses loaded from lookup tables.
    pub loaded_addresses: usize,
    /// Exact ordered wallet signer set.
    pub required_signers: Vec<Pubkey>,
    /// Resolved payer/meta/program lock census.
    pub lock_census: DealerScenarioTransactionLockCensusV1,
}

/// Stable builder or journal refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioCheckpointOperatorErrorV1 {
    /// A page ordinal, account set, or signer set was not canonical.
    Geometry,
    /// The transaction would exceed the cluster's 64-lock ceiling.
    LockLimit,
    /// Versioned-message construction failed.
    Message,
    /// The fully signed transaction exceeded the packet ceiling.
    Packet,
    /// A journal transition was skipped, replayed, or substituted.
    Journal,
    /// Checked arithmetic overflowed.
    Arithmetic,
}

/// Non-effect accounts the final commit must carry in addition to its child frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioFinalCommitFixedAccountsV1 {
    /// Transaction fee payer.
    pub payer: Pubkey,
    /// Trading program invoked by the transaction.
    pub trading_program: Pubkey,
    /// Evaluated checkpoint, closed last.
    pub checkpoint: Pubkey,
    /// Clock sysvar used for expiry admission.
    pub clock: Pubkey,
    /// Exact immutable request body.
    pub request: Pubkey,
    /// Producer-owned evaluation receipt.
    pub evaluation_receipt: Pubkey,
    /// Producer-owned candidate bank.
    pub candidate_bank: Pubkey,
    /// Producer-owned candidate obligation body.
    pub candidate_obligation: Pubkey,
    /// Producer-owned expected Claims delta.
    pub claims_delta: Pubkey,
    /// Producer-owned ordered Custody effects.
    pub effects: Pubkey,
}

/// Exact account-only final-commit topology before an executor exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioFinalCommitTopologyV1 {
    /// Deduplicated effect accounts with unioned privileges.
    pub effect_accounts: Vec<AccountMeta>,
    /// Distinct resolved locks including payer and Trading program.
    pub unique_account_lock_count: usize,
    /// Whether one transaction fits devnet's current lock limit.
    pub fits_devnet_lock_limit: bool,
}

/// Project the smallest one-transaction Claims/Custody effect topology.
///
/// This is account topology only. It deliberately does not claim a final
/// executor: the future handler must still reauthenticate the release-selected
/// producer, immediate Claims/Custody receipts, candidate obligation bytes,
/// and close the checkpoint atomically.
pub fn project_dealer_scenario_final_commit_topology_v1(
    state: DealerScenarioHotMetaStateV4<'_>,
    tail_count: u32,
    spans: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    fixed: DealerScenarioFinalCommitFixedAccountsV1,
) -> Result<DealerScenarioFinalCommitTopologyV1, DealerScenarioCheckpointOperatorErrorV1> {
    let profile_account = state
        .fixed_accounts
        .get(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)
        .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let profile = AccountProfileV2::decode(&profile_account.account.data)
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let frame = dealer_scenario_logical_frame_v4(spans)
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let mut metas = Vec::new();
    push_meta(&mut metas, AccountMeta::new(fixed.checkpoint, false));
    for key in [
        fixed.clock,
        fixed.request,
        fixed.evaluation_receipt,
        fixed.candidate_bank,
        fixed.candidate_obligation,
        fixed.claims_delta,
        fixed.effects,
    ] {
        push_meta(&mut metas, AccountMeta::new_readonly(key, false));
    }
    push_logical_meta(&mut metas, state, profile, tail_count, &spans, 0)?;
    let custody_counts = [
        *spans
            .first()
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
        *spans
            .get(1)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
        *spans
            .get(2)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
        *spans
            .get(3)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
        *spans
            .get(5)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
        *spans
            .get(6)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
    ];
    for (start, count) in frame.custody_starts.into_iter().zip(custody_counts) {
        for logical in start
            ..start
                .checked_add(count)
                .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?
        {
            push_logical_meta(&mut metas, state, profile, tail_count, &spans, logical)?;
        }
    }
    let claims_end = frame
        .claims_positions_start
        .checked_add(
            *spans
                .get(4)
                .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
        )
        .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
    for logical in frame.claims_fixed_start..claims_end {
        push_logical_meta(&mut metas, state, profile, tail_count, &spans, logical)?;
    }
    for logical in [frame.obligation, frame.custody_program] {
        push_logical_meta(&mut metas, state, profile, tail_count, &spans, logical)?;
    }
    let instruction = Instruction {
        program_id: fixed.trading_program,
        accounts: metas.clone(),
        data: Vec::new(),
    };
    let census = census_dealer_scenario_transaction_locks_v1(
        fixed.payer,
        core::slice::from_ref(&instruction),
    );
    Ok(DealerScenarioFinalCommitTopologyV1 {
        effect_accounts: metas,
        unique_account_lock_count: census.unique_account_lock_count,
        fits_devnet_lock_limit: census.unique_account_lock_count <= 64,
    })
}

fn push_logical_meta(
    metas: &mut Vec<AccountMeta>,
    state: DealerScenarioHotMetaStateV4<'_>,
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    spans: &[u32],
    logical_coordinate: u32,
) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
    let physical_ordinal = profile
        .physical_account_ordinal_with_dynamic_spans(
            tail_count,
            spans,
            usize::try_from(logical_coordinate)
                .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
        )
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let account = if let Some(fixed_index) = [
        HOT_ROOT_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    ]
    .get(physical_ordinal)
    {
        state
            .fixed_accounts
            .get(*fixed_index)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?
    } else {
        state
            .runtime_suffix_accounts
            .get(
                physical_ordinal
                    .checked_sub(5)
                    .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
            )
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?
    };
    push_meta(
        metas,
        AccountMeta {
            pubkey: account.account.key,
            is_signer: account.is_signer,
            is_writable: account.is_writable,
        },
    );
    Ok(())
}

fn push_meta(metas: &mut Vec<AccountMeta>, incoming: AccountMeta) {
    if let Some(existing) = metas
        .iter_mut()
        .find(|existing| existing.pubkey == incoming.pubkey)
    {
        existing.is_signer |= incoming.is_signer;
        existing.is_writable |= incoming.is_writable;
    } else {
        metas.push(incoming);
    }
}

/// Exact checkpoint PDA for one complete Dealer request body.
#[must_use]
pub fn dealer_scenario_checkpoint_address_v1(
    trading_program: Pubkey,
    request_digest: [u8; 32],
) -> Pubkey {
    Pubkey::find_program_address(
        &[DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1, &request_digest],
        &trading_program,
    )
    .0
}

/// Build and compile one checkpoint-create transaction.
#[allow(clippy::too_many_arguments)]
pub fn build_dealer_scenario_checkpoint_create_v1(
    trading_program: Pubkey,
    payer: Pubkey,
    dealer_authority: Pubkey,
    refund_beneficiary: Pubkey,
    checkpoint: Pubkey,
    request: Pubkey,
    root: Pubkey,
    obligation: Pubkey,
    clock: Pubkey,
    rent: Pubkey,
    system_program: Pubkey,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    let instruction = Instruction {
        program_id: trading_program,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(dealer_authority, true),
            AccountMeta::new_readonly(refund_beneficiary, false),
            AccountMeta::new(checkpoint, false),
            AccountMeta::new_readonly(request, false),
            AccountMeta::new_readonly(root, false),
            AccountMeta::new_readonly(obligation, false),
            AccountMeta::new_readonly(clock, false),
            AccountMeta::new_readonly(rent, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: DEALER_SCENARIO_CHECKPOINT_CREATE_MAGIC_V1.to_vec(),
    };
    compile_checkpoint_packet(
        DealerScenarioCheckpointRouteV1::Create,
        payer,
        instruction,
        recent_blockhash,
        lookup_tables,
    )
}

/// Build and compile one ordered readonly page transaction.
pub fn build_dealer_scenario_checkpoint_page_v1(
    trading_program: Pubkey,
    payer: Pubkey,
    checkpoint: Pubkey,
    clock: Pubkey,
    page_index: u8,
    observations: &[Pubkey],
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    if usize::from(page_index) >= DEALER_SCENARIO_PREPARATION_PAGES_V1
        || observations.is_empty()
        || observations.len()
            > dclutch_trading_sbf::dealer_scenario_checkpoint_v1::DEALER_SCENARIO_CHECKPOINT_PAGE_MAX_OBSERVATIONS_V1
        || has_duplicate_keys(observations)
        || observations.contains(&checkpoint)
        || observations.contains(&clock)
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let mut accounts = Vec::with_capacity(2 + observations.len());
    accounts.push(AccountMeta::new(checkpoint, false));
    accounts.push(AccountMeta::new_readonly(clock, false));
    accounts.extend(
        observations
            .iter()
            .copied()
            .map(|key| AccountMeta::new_readonly(key, false)),
    );
    let mut data = DEALER_SCENARIO_CHECKPOINT_PAGE_MAGIC_V1.to_vec();
    data.push(page_index);
    compile_checkpoint_packet(
        DealerScenarioCheckpointRouteV1::Page(page_index),
        payer,
        Instruction {
            program_id: trading_program,
            accounts,
            data,
        },
        recent_blockhash,
        lookup_tables,
    )
}

/// Build and compile one evaluation-seal transaction.
#[allow(clippy::too_many_arguments)]
pub fn build_dealer_scenario_checkpoint_evaluate_v1(
    trading_program: Pubkey,
    payer: Pubkey,
    checkpoint: Pubkey,
    clock: Pubkey,
    producer_program: Pubkey,
    evaluation_receipt: Pubkey,
    candidate_bank: Pubkey,
    candidate_obligation: Pubkey,
    claims_delta: Pubkey,
    effects: Pubkey,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    let instruction = Instruction {
        program_id: trading_program,
        accounts: vec![
            AccountMeta::new(checkpoint, false),
            AccountMeta::new_readonly(clock, false),
            AccountMeta::new_readonly(producer_program, false),
            AccountMeta::new_readonly(evaluation_receipt, false),
            AccountMeta::new_readonly(candidate_bank, false),
            AccountMeta::new_readonly(candidate_obligation, false),
            AccountMeta::new_readonly(claims_delta, false),
            AccountMeta::new_readonly(effects, false),
        ],
        data: DEALER_SCENARIO_CHECKPOINT_EVALUATE_MAGIC_V1.to_vec(),
    };
    compile_checkpoint_packet(
        DealerScenarioCheckpointRouteV1::Evaluate,
        payer,
        instruction,
        recent_blockhash,
        lookup_tables,
    )
}

/// Build and compile one permissionless expiry cleanup transaction.
pub fn build_dealer_scenario_checkpoint_cleanup_v1(
    trading_program: Pubkey,
    payer: Pubkey,
    checkpoint: Pubkey,
    beneficiary: Pubkey,
    clock: Pubkey,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    let instruction = Instruction {
        program_id: trading_program,
        accounts: vec![
            AccountMeta::new(checkpoint, false),
            AccountMeta::new(beneficiary, false),
            AccountMeta::new_readonly(clock, false),
        ],
        data: DEALER_SCENARIO_CHECKPOINT_CLEANUP_MAGIC_V1.to_vec(),
    };
    compile_checkpoint_packet(
        DealerScenarioCheckpointRouteV1::Cleanup,
        payer,
        instruction,
        recent_blockhash,
        lookup_tables,
    )
}

fn compile_checkpoint_packet(
    route: DealerScenarioCheckpointRouteV1,
    payer: Pubkey,
    instruction: Instruction,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    let census =
        census_dealer_scenario_transaction_locks_v1(payer, core::slice::from_ref(&instruction));
    require_dealer_scenario_devnet_lock_limit_v1(payer, core::slice::from_ref(&instruction))
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::LockLimit)?;
    let required_signers = signer_set(payer, &instruction);
    let message = v0::Message::try_compile(
        &payer,
        core::slice::from_ref(&instruction),
        lookup_tables,
        recent_blockhash,
    )
    .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Message)?;
    if usize::from(message.header.num_required_signatures) != required_signers.len() {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let loaded_addresses = message
        .address_table_lookups
        .iter()
        .try_fold(0_usize, |total, lookup| {
            total
                .checked_add(lookup.writable_indexes.len())
                .and_then(|value| value.checked_add(lookup.readonly_indexes.len()))
        })
        .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
    let versioned = VersionedMessage::V0(message);
    let signature_count = required_signers.len();
    let wire_bytes = short_vec_prefix_bytes(signature_count)
        .checked_add(
            signature_count
                .checked_mul(64)
                .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
        )
        .and_then(|value| value.checked_add(versioned.serialize().len()))
        .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
    if wire_bytes > DEALER_SCENARIO_PACKET_BYTES_V1 {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Packet);
    }
    Ok(DealerScenarioCheckpointPacketV1 {
        route,
        instruction,
        message: versioned,
        wire_bytes,
        loaded_addresses,
        required_signers,
        lock_census: census,
    })
}

fn signer_set(payer: Pubkey, instruction: &Instruction) -> Vec<Pubkey> {
    let mut signers = vec![payer];
    for signer in instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_signer)
        .map(|meta| meta.pubkey)
    {
        if !signers.contains(&signer) {
            signers.push(signer);
        }
    }
    signers
}

fn has_duplicate_keys(keys: &[Pubkey]) -> bool {
    keys.iter().enumerate().any(|(index, current)| {
        keys.get(index.saturating_add(1)..)
            .unwrap_or(&[])
            .contains(current)
    })
}

fn short_vec_prefix_bytes(value: usize) -> usize {
    if value < 128 {
        1
    } else if value < 16_384 {
        2
    } else {
        3
    }
}

/// Durable local submission journal for crash-safe checkpoint progression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioCheckpointJournalV1 {
    /// Trading-owned checkpoint address.
    pub checkpoint: Pubkey,
    /// Exact Dealer request digest.
    pub request_digest: [u8; 32],
    /// Last chain-observed checkpoint digest, zero until create confirms.
    pub checkpoint_digest: [u8; 32],
    /// Next page ordinal which may be journaled.
    pub next_page: u8,
    /// Exact ordered page receipt digests.
    pub page_receipts: [[u8; 32]; DEALER_SCENARIO_PREPARATION_PAGES_V1],
    /// Sealed evaluation receipt digest, zero before evaluation.
    pub evaluation_receipt: [u8; 32],
    /// Whether finalized cleanup observed the checkpoint vacant.
    pub cleaned: bool,
}

impl DealerScenarioCheckpointJournalV1 {
    /// Create a planned journal before signing checkpoint creation.
    pub fn planned(
        trading_program: Pubkey,
        request_digest: [u8; 32],
    ) -> Result<Self, DealerScenarioCheckpointOperatorErrorV1> {
        if request_digest == [0; 32] {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        Ok(Self {
            checkpoint: dealer_scenario_checkpoint_address_v1(trading_program, request_digest),
            request_digest,
            checkpoint_digest: [0; 32],
            next_page: 0,
            page_receipts: [[0; 32]; DEALER_SCENARIO_PREPARATION_PAGES_V1],
            evaluation_receipt: [0; 32],
            cleaned: false,
        })
    }

    /// Record finalized checkpoint creation.
    pub fn record_created(
        &mut self,
        checkpoint_digest: [u8; 32],
    ) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if self.checkpoint_digest != [0; 32] || checkpoint_digest == [0; 32] || self.cleaned {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        self.checkpoint_digest = checkpoint_digest;
        Ok(())
    }

    /// Record one finalized ordered page and its checkpoint poststate.
    pub fn record_page(
        &mut self,
        page: u8,
        receipt_digest: [u8; 32],
        checkpoint_poststate_digest: [u8; 32],
    ) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if self.checkpoint_digest == [0; 32]
            || page != self.next_page
            || usize::from(page) >= DEALER_SCENARIO_PREPARATION_PAGES_V1
            || receipt_digest == [0; 32]
            || checkpoint_poststate_digest == [0; 32]
            || self.evaluation_receipt != [0; 32]
            || self.cleaned
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        let destination = self
            .page_receipts
            .get_mut(usize::from(page))
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Journal)?;
        if *destination != [0; 32] {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        *destination = receipt_digest;
        self.next_page = self
            .next_page
            .checked_add(1)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
        self.checkpoint_digest = checkpoint_poststate_digest;
        Ok(())
    }

    /// Record finalized evaluation sealing and its checkpoint poststate.
    pub fn record_evaluated(
        &mut self,
        evaluation_receipt: [u8; 32],
        checkpoint_poststate_digest: [u8; 32],
    ) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if usize::from(self.next_page) != DEALER_SCENARIO_PREPARATION_PAGES_V1
            || evaluation_receipt == [0; 32]
            || checkpoint_poststate_digest == [0; 32]
            || self.evaluation_receipt != [0; 32]
            || self.cleaned
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        self.evaluation_receipt = evaluation_receipt;
        self.checkpoint_digest = checkpoint_poststate_digest;
        Ok(())
    }

    /// Record finalized expiry cleanup only after create occurred.
    pub fn record_cleaned(&mut self) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if self.checkpoint_digest == [0; 32] || self.cleaned {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        self.cleaned = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn blockhash() -> Hash {
        Hash::new_from_array([99; 32])
    }

    #[test]
    fn every_lifecycle_packet_is_small_and_lock_bounded() {
        let create = build_dealer_scenario_checkpoint_create_v1(
            key(1),
            key(2),
            key(3),
            key(4),
            key(5),
            key(6),
            key(7),
            key(8),
            key(9),
            key(10),
            key(11),
            blockhash(),
            &[],
        )
        .expect("create");
        let evaluate = build_dealer_scenario_checkpoint_evaluate_v1(
            key(1),
            key(2),
            key(5),
            key(9),
            key(12),
            key(13),
            key(14),
            key(15),
            key(16),
            key(17),
            blockhash(),
            &[],
        )
        .expect("evaluate");
        let cleanup = build_dealer_scenario_checkpoint_cleanup_v1(
            key(1),
            key(2),
            key(5),
            key(4),
            key(9),
            blockhash(),
            &[],
        )
        .expect("cleanup");
        assert_eq!(
            (
                create.wire_bytes,
                create.lock_census.unique_account_lock_count
            ),
            (541, 11)
        );
        assert_eq!(
            (
                evaluate.wire_bytes,
                evaluate.lock_census.unique_account_lock_count
            ),
            (443, 10)
        );
        assert_eq!(
            (
                cleanup.wire_bytes,
                cleanup.lock_census.unique_account_lock_count
            ),
            (278, 5)
        );
        for packet in [create, evaluate, cleanup] {
            assert!(packet.wire_bytes <= DEALER_SCENARIO_PACKET_BYTES_V1);
            assert!(packet.lock_census.unique_account_lock_count <= 64);
            assert_eq!(packet.loaded_addresses, 0);
        }
    }

    #[test]
    fn maximum_page_uses_a_table_and_stays_below_64_locks() {
        let observations = (20_u8..68).map(key).collect::<Vec<_>>();
        let table = AddressLookupTableAccount {
            key: key(19),
            addresses: observations.clone(),
        };
        let packet = build_dealer_scenario_checkpoint_page_v1(
            key(1),
            key(2),
            key(3),
            key(4),
            0,
            &observations,
            blockhash(),
            core::slice::from_ref(&table),
        )
        .expect("paged packet");
        assert_eq!(packet.lock_census.unique_account_lock_count, 52);
        assert_eq!(packet.loaded_addresses, 48);
        assert_eq!(packet.wire_bytes, 376);
        assert!(packet.wire_bytes <= DEALER_SCENARIO_PACKET_BYTES_V1);

        let mut too_many = observations;
        too_many.push(key(68));
        assert_eq!(
            build_dealer_scenario_checkpoint_page_v1(
                key(1),
                key(2),
                key(3),
                key(4),
                0,
                &too_many,
                blockhash(),
                core::slice::from_ref(&table),
            ),
            Err(DealerScenarioCheckpointOperatorErrorV1::Geometry)
        );
    }

    #[test]
    fn resolved_transaction_census_admits_64_and_refuses_65() {
        let payer = key(2);
        let program = key(1);
        let at_64 = Instruction {
            program_id: program,
            accounts: (3_u8..65)
                .map(|byte| AccountMeta::new_readonly(key(byte), false))
                .collect(),
            data: Vec::new(),
        };
        assert_eq!(
            census_dealer_scenario_transaction_locks_v1(payer, core::slice::from_ref(&at_64))
                .unique_account_lock_count,
            64
        );
        assert!(
            require_dealer_scenario_devnet_lock_limit_v1(payer, core::slice::from_ref(&at_64))
                .is_ok()
        );
        let mut at_65 = at_64;
        at_65
            .accounts
            .push(AccountMeta::new_readonly(key(65), false));
        assert_eq!(
            require_dealer_scenario_devnet_lock_limit_v1(payer, core::slice::from_ref(&at_65)),
            Err(crate::dealer_scenario_hot_v4::DealerScenarioLockLimitErrorV1::LockLimit)
        );
    }

    #[test]
    fn journal_refuses_skip_replay_and_post_terminal_progress() {
        let mut journal =
            DealerScenarioCheckpointJournalV1::planned(key(1), [2; 32]).expect("planned");
        assert_eq!(
            journal.record_page(0, [3; 32], [4; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        journal.record_created([5; 32]).expect("created");
        assert_eq!(
            journal.record_page(1, [6; 32], [7; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        for page in 0_u8..6 {
            journal
                .record_page(page, [10 + page; 32], [20 + page; 32])
                .expect("ordered page");
        }
        assert_eq!(
            journal.record_page(5, [40; 32], [41; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        journal
            .record_evaluated([50; 32], [51; 32])
            .expect("evaluation");
        journal.record_cleaned().expect("cleanup");
        assert_eq!(
            journal.record_evaluated([52; 32], [53; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
    }
}
