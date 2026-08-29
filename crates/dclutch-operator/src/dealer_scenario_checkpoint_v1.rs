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
use dclutch_claims_svm::frame_spec_v1::{ClaimsFrameRoleV1, SignedDeltaFrameSpecV3};
use dclutch_dealer_codec::scenario_checkpoint_v1::{
    DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1, DEALER_SCENARIO_CLAIMS_PRESTATE_DOMAIN_V1,
    DEALER_SCENARIO_CUSTODY_PRESTATE_DOMAIN_V1, DEALER_SCENARIO_PREPARATION_PAGES_V1,
    DealerScenarioCheckpointV1,
};
use dclutch_dealer_codec::scenario_evaluation_receipt_v1::{
    DEALER_SCENARIO_EVALUATION_RECEIPT_PDA_DOMAIN_V1, DealerScenarioEvaluationReceiptV1,
};
use dclutch_dealer_codec::scenario_custody_reservation_v1::{
    DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1, DealerScenarioCustodyEffectManifestV1,
    DealerScenarioCustodyEffectV1, DealerScenarioCustodyRequestKindV1,
    encode_dealer_scenario_activation_instruction_v1,
    encode_dealer_scenario_reservation_instruction_v1,
};
use dclutch_dealer_codec::scenario_membership_manifest_v1::{
    DEALER_SCENARIO_MEMBERSHIP_MANIFEST_PDA_DOMAIN_V1, DEALER_SCENARIO_MEMBERSHIP_PAGE_DOMAIN_V1,
    DEALER_SCENARIO_MEMBERSHIP_PAGES_V1, DealerScenarioMembershipManifestV1,
};
use dclutch_dealer_codec::scenario_custody_reservation_v1::DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1;
use dclutch_dealer_codec::scenario_reservation_receipt_v1::{
    DEALER_SCENARIO_MAX_RESERVATIONS_V1, DealerScenarioReservationActionV1,
};
use dclutch_trading_sbf::dealer::v3_trade_profile::{
    DEALER_SCENARIO_PROFILE_SPANS_V4, dealer_scenario_logical_frame_v4,
};
use dclutch_trading_sbf::dealer::{
    v3_composer::ScenarioCustodyEffectV3, v3_multi_lp::MultiLpCustodyRequestV3,
};
use dclutch_trading_sbf::dealer_scenario_checkpoint_v1::{
    DEALER_SCENARIO_CHECKPOINT_CLEANUP_MAGIC_V1, DEALER_SCENARIO_CHECKPOINT_COMMIT_MAGIC_V1,
    DEALER_SCENARIO_CHECKPOINT_CREATE_MAGIC_V1, DEALER_SCENARIO_CHECKPOINT_EVALUATE_MAGIC_V1,
    DEALER_SCENARIO_CHECKPOINT_PAGE_MAGIC_V1, DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1,
    DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1,
};
use solana_hash::Hash;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::dealer_scenario_hot_v4::{
    DealerScenarioHotMetaStateV4, DealerScenarioTransactionLockCensusV1,
    SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1, census_dealer_scenario_transaction_locks_v1,
    require_dealer_scenario_devnet_lock_limit_v1,
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
    /// Ingest one Custody reservation receipt.
    Reserve(u8),
    /// Ingest one reverse-order Custody rollback receipt.
    Rollback(u8),
    /// Atomically commit Claims and the candidate obligation against locked value.
    Commit,
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

/// Complete account bank for one real Custody reservation producer call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioReservationAccountsV1 {
    /// Release-selected Custody program.
    pub custody_program: Pubkey,
    /// Core Market.
    pub market: Pubkey,
    /// Registry activation cache.
    pub activation_cache: Pubkey,
    /// Registry executable.
    pub registry_program: Pubkey,
    /// Release-selected Trading executable.
    pub trading_program: Pubkey,
    /// Trading ProgramData pinned by the release.
    pub trading_programdata: Pubkey,
    /// Immutable Realm.
    pub realm: Pubkey,
    /// Vacant Realm staging cursor.
    pub realm_staging: Pubkey,
    /// Standard Custody replay cursor.
    pub custody_replay: Pubkey,
    /// Trading checkpoint.
    pub checkpoint: Pubkey,
    /// Evaluator executable owning the effect bank.
    pub effect_producer: Pubkey,
    /// Complete effect manifest.
    pub effect_manifest: Pubkey,
    /// Ordinal-selected effect body.
    pub effect_body: Pubkey,
    /// Custody reservation batch PDA.
    pub batch: Pubkey,
    /// Custody per-effect state PDA.
    pub reservation_state: Pubkey,
    /// Custody typed receipt PDA for the selected action.
    pub reservation_receipt: Pubkey,
    /// Original source token account.
    pub source: Pubkey,
    /// Original final destination token account.
    pub destination: Pubkey,
    /// Custody-owned temporary RecoveryReserve vault.
    pub escrow: Pubkey,
    /// Realm-selected collateral Mint.
    pub mint: Pubkey,
    /// Custody transfer authority.
    pub custody_authority: Pubkey,
    /// Realm-selected Token or Token-2022 program.
    pub token_program: Pubkey,
    /// Rent payer and transaction signer.
    pub payer: Pubkey,
    /// Immutable rollback/rent beneficiary.
    pub refund_beneficiary: Pubkey,
    /// Clock sysvar.
    pub clock: Pubkey,
    /// Rent sysvar.
    pub rent: Pubkey,
    /// System program.
    pub system_program: Pubkey,
    /// Custody ProgramData pinned by the release and checked by Trading ingest.
    pub custody_programdata: Pubkey,
}

/// Atomic producer-then-ingest message with its real escrow-inclusive geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioReservationBundlePacketV1 {
    /// Reserve or reverse rollback.
    pub action: DealerScenarioReservationActionV1,
    /// Effect ordinal.
    pub effect_ordinal: u8,
    /// Custody producer followed immediately by Trading receipt ingestion.
    pub instructions: [Instruction; 2],
    /// Unsigned v0 message.
    pub message: VersionedMessage,
    /// Exact fully signed transaction width.
    pub wire_bytes: usize,
    /// Number of addresses loaded from lookup tables.
    pub loaded_addresses: usize,
    /// Exact ordered wallet signer set.
    pub required_signers: Vec<Pubkey>,
    /// Resolved payer/meta/program lock census.
    pub lock_census: DealerScenarioTransactionLockCensusV1,
}

/// Exact per-effect account quartet consumed by Custody batch activation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DealerScenarioActivationEffectAccountsV1 {
    /// Evaluator-owned exact effect body.
    pub effect_body: Pubkey,
    /// Custody-owned active reservation state.
    pub reservation_state: Pubkey,
    /// Custody RecoveryReserve token escrow.
    pub escrow: Pubkey,
    /// Exact original destination token account.
    pub destination: Pubkey,
}

/// Complete common and ordered effect account bank for Custody activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioActivationAccountsV1 {
    /// Release-selected Custody program.
    pub custody_program: Pubkey,
    /// Core Market.
    pub market: Pubkey,
    /// Registry activation cache.
    pub activation_cache: Pubkey,
    /// Registry executable.
    pub registry_program: Pubkey,
    /// Release-selected Trading executable.
    pub trading_program: Pubkey,
    /// Trading ProgramData pinned by the release.
    pub trading_programdata: Pubkey,
    /// Immutable Realm.
    pub realm: Pubkey,
    /// Vacant Realm staging cursor.
    pub realm_staging: Pubkey,
    /// Writable standard Custody replay cursor.
    pub custody_replay: Pubkey,
    /// Reserved Trading checkpoint.
    pub checkpoint: Pubkey,
    /// Evaluator executable owning the effect bank.
    pub effect_producer: Pubkey,
    /// Complete evaluator-owned effect manifest.
    pub effect_manifest: Pubkey,
    /// Custody reservation batch PDA.
    pub batch: Pubkey,
    /// Vacant Custody activation receipt PDA.
    pub activation_receipt: Pubkey,
    /// Realm-selected collateral Mint.
    pub mint: Pubkey,
    /// Custody transfer authority.
    pub custody_authority: Pubkey,
    /// Realm-selected Token or Token-2022 program.
    pub token_program: Pubkey,
    /// Rent payer and transaction signer.
    pub payer: Pubkey,
    /// Immutable escrow-rent beneficiary.
    pub refund_beneficiary: Pubkey,
    /// Rent sysvar.
    pub rent: Pubkey,
    /// System program.
    pub system_program: Pubkey,
    /// Active ordered prefix; inactive suffix must be all-default.
    pub effects: [DealerScenarioActivationEffectAccountsV1; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    /// Exact active effect count.
    pub effect_count: u8,
}

/// One real escrow-inclusive Custody activation packet and census.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioActivationPacketV1 {
    /// Exact Custody activation instruction.
    pub instruction: Instruction,
    /// Unsigned v0 message.
    pub message: VersionedMessage,
    /// Exact fully-signed transaction width.
    pub wire_bytes: usize,
    /// Number of addresses loaded from lookup tables.
    pub loaded_addresses: usize,
    /// Exact ordered signer set.
    pub required_signers: Vec<Pubkey>,
    /// Resolved payer/meta/program lock census.
    pub lock_census: DealerScenarioTransactionLockCensusV1,
}

/// One ordered readonly Custody proof consumed by the final commit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DealerScenarioCommitEffectAccountsV1 {
    /// Custody-owned typed Reserve receipt.
    pub reservation_receipt: Pubkey,
    /// Custody-owned active reservation state.
    pub reservation_state: Pubkey,
}

/// Exact final Claims/obligation commit account bank.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioCommitAccountsV1 {
    /// Current Trading program.
    pub trading_program: Pubkey,
    /// Transaction fee payer; no protocol authority is inferred from it.
    pub payer: Pubkey,
    /// Trading-owned checkpoint.
    pub checkpoint: Pubkey,
    /// Clock sysvar used only for pre-expiry commit admission.
    pub clock: Pubkey,
    /// Exact immutable Dealer request.
    pub request: Pubkey,
    /// Trading-owned persisted evaluation receipt.
    pub evaluation_receipt: Pubkey,
    /// Trading-owned exact candidate bank.
    pub candidate_bank: Pubkey,
    /// Trading-owned exact candidate obligation body.
    pub candidate_obligation: Pubkey,
    /// Trading-owned exact SignedDelta body.
    pub claims_delta: Pubkey,
    /// Trading-owned Custody-effect manifest.
    pub effects: Pubkey,
    /// Immutable Dealer child root.
    pub root: Pubkey,
    /// Writable canonical obligation.
    pub obligation: Pubkey,
    /// Current Custody program.
    pub custody_program: Pubkey,
    /// Current Custody ProgramData.
    pub custody_programdata: Pubkey,
    /// Custody-owned complete locked batch.
    pub batch: Pubkey,
    /// Exact Claims SignedDelta frame, excluding the Claims callee duplicate.
    pub claims_accounts: Vec<AccountMeta>,
    /// Active ordered proof prefix.
    pub effect_accounts:
        [DealerScenarioCommitEffectAccountsV1; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    /// Exact active effect count.
    pub effect_count: u8,
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

/// Reservation accounts replacing the repeated Custody transfer frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioReservedFinalAccountsV1 {
    /// Common final-commit evidence accounts.
    pub fixed: DealerScenarioFinalCommitFixedAccountsV1,
    /// Release-selected Custody program activating every reservation in one CPI.
    pub custody_program: Pubkey,
    /// Exact ordered producer receipt accounts.
    pub reservation_receipts: [Pubkey; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    /// Exact ordered Custody reservation accounts.
    pub reservation_states: [Pubkey; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    /// Active prefix length selected by the admitted evaluator.
    pub effect_count: u8,
}

/// Canonical six-page account partition and its producer-owned manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioCanonicalMembershipPagesV1 {
    /// Typed manifest body the selected evaluator must publish.
    pub manifest: DealerScenarioMembershipManifestV1,
    /// Strictly increasing, deduplicated keys in six balanced pages.
    pub pages: [Vec<Pubkey>; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1],
}

/// Exact evaluator-owned effect manifest and bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioCustodyEffectArtifactsV1 {
    /// Manifest body hashed by the evaluation receipt.
    pub manifest: DealerScenarioCustodyEffectManifestV1,
    /// Active canonical prefix; inactive suffix is absent.
    pub effect_bodies: [Option<DealerScenarioCustodyEffectV1>; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
}

/// Encode the semantic projection's one canonical Custody effect bank.
pub fn encode_dealer_scenario_custody_effect_artifacts_v1(
    producer_program: Pubkey,
    checkpoint: Pubkey,
    request_digest: [u8; 32],
    effect_accounts: [Pubkey; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    effects: &[Option<ScenarioCustodyEffectV3>; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    effect_count: u8,
) -> Result<DealerScenarioCustodyEffectArtifactsV1, DealerScenarioCheckpointOperatorErrorV1> {
    let active = usize::from(effect_count);
    if producer_program == Pubkey::default()
        || checkpoint == Pubkey::default()
        || request_digest == [0; 32]
        || active == 0
        || active > DEALER_SCENARIO_MAX_RESERVATIONS_V1
        || effects.iter().skip(active).any(Option::is_some)
        || effect_accounts
            .iter()
            .take(active)
            .any(|account| *account == Pubkey::default())
        || effect_accounts
            .iter()
            .skip(active)
            .any(|account| *account != Pubkey::default())
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let mut effect_bodies = [None; DEALER_SCENARIO_MAX_RESERVATIONS_V1];
    let mut effect_digests = [[0_u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1];
    for ordinal in 0..active {
        let effect = effects
            .get(ordinal)
            .copied()
            .flatten()
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
        let kind = match effect.request {
            MultiLpCustodyRequestV3::Canonical(_) => DealerScenarioCustodyRequestKindV1::Canonical,
            MultiLpCustodyRequestV3::Delegated(_) => {
                // A staged rollback cannot restore delegated allowance. The
                // caller must first deposit external value into an internal
                // TradingPrincipal vault and evaluate canonical effects.
                return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
            }
        };
        let mut request_payload = [0_u8; DEALER_SCENARIO_DELEGATED_CUSTODY_REQUEST_BYTES_V1];
        effect
            .request
            .encode_into(
                request_payload
                    .get_mut(..effect.request.encoded_len())
                    .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
            )
            .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
        let body = DealerScenarioCustodyEffectV1 {
            kind,
            ordinal: u8::try_from(ordinal)
                .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
            effect_count,
            producer_program: producer_program.to_bytes(),
            checkpoint: checkpoint.to_bytes(),
            request_digest,
            source_after: effect.source_after,
            destination_after: effect.destination_after,
            request_payload,
        };
        let body_bytes = body
            .encode()
            .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
        *effect_digests
            .get_mut(ordinal)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)? =
            hash(&body_bytes).to_bytes();
        *effect_bodies
            .get_mut(ordinal)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)? = Some(body);
    }
    let manifest = DealerScenarioCustodyEffectManifestV1 {
        effect_count,
        producer_program: producer_program.to_bytes(),
        checkpoint: checkpoint.to_bytes(),
        request_digest,
        effect_accounts: effect_accounts.map(|account| account.to_bytes()),
        effect_digests,
    };
    manifest
        .encode()
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    Ok(DealerScenarioCustodyEffectArtifactsV1 {
        manifest,
        effect_bodies,
    })
}

/// Project the smallest one-transaction Claims/Custody effect topology.
///
/// This is legacy account-topology evidence for the original monolithic
/// proposal. The executable split now locks value first, commits Claims plus
/// the obligation atomically, and delivers Custody escrows afterward.
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

/// Project the bounded final topology after every Custody effect is reserved.
///
/// This remains legacy topology evidence, not the executable final route. The
/// real commit builder below authenticates the locked batch and moves the
/// checkpoint to `Committed`; Custody delivery is then permissionless and
/// resumable in a later transaction.
pub fn project_dealer_scenario_reserved_final_topology_v1(
    state: DealerScenarioHotMetaStateV4<'_>,
    tail_count: u32,
    spans: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    accounts: DealerScenarioReservedFinalAccountsV1,
) -> Result<DealerScenarioFinalCommitTopologyV1, DealerScenarioCheckpointOperatorErrorV1> {
    let effect_count = usize::from(accounts.effect_count);
    if effect_count > DEALER_SCENARIO_MAX_RESERVATIONS_V1
        || accounts.custody_program == Pubkey::default()
        || accounts
            .reservation_receipts
            .iter()
            .take(effect_count)
            .chain(accounts.reservation_states.iter().take(effect_count))
            .any(|key| *key == Pubkey::default())
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let profile_account = state
        .fixed_accounts
        .get(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)
        .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let profile = AccountProfileV2::decode(&profile_account.account.data)
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let frame = dealer_scenario_logical_frame_v4(spans)
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let fixed = accounts.fixed;
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
    push_logical_meta(
        &mut metas,
        state,
        profile,
        tail_count,
        &spans,
        frame.obligation,
    )?;
    push_meta(
        &mut metas,
        AccountMeta::new_readonly(accounts.custody_program, false),
    );
    for (receipt, reservation) in accounts
        .reservation_receipts
        .iter()
        .zip(accounts.reservation_states.iter())
        .take(effect_count)
    {
        push_meta(&mut metas, AccountMeta::new_readonly(*receipt, false));
        push_meta(&mut metas, AccountMeta::new(*reservation, false));
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

/// Derive the one canonical six-page membership partition.
///
/// The set is the complete physical Dealer frame after alias de-duplication.
/// Sorting once across the whole set makes every page boundary strict, so a
/// key cannot be repeated in another page. The balanced split is unique for a
/// given set and keeps the largest page at or below 48 accounts.
pub fn project_dealer_scenario_canonical_membership_pages_v1(
    state: DealerScenarioHotMetaStateV4<'_>,
    producer_program: Pubkey,
    checkpoint: Pubkey,
    request_digest: [u8; 32],
) -> Result<DealerScenarioCanonicalMembershipPagesV1, DealerScenarioCheckpointOperatorErrorV1> {
    if producer_program == Pubkey::default()
        || checkpoint == Pubkey::default()
        || request_digest == [0; 32]
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let mut keys = state
        .fixed_accounts
        .iter()
        .chain(state.strategy_accounts.iter())
        .chain(state.runtime_suffix_accounts.iter())
        .map(|meta| meta.account.key)
        .collect::<Vec<_>>();
    keys.sort_unstable_by_key(Pubkey::to_bytes);
    keys.dedup();
    let maximum = DEALER_SCENARIO_MEMBERSHIP_PAGES_V1
        .checked_mul(usize::from(
            dclutch_dealer_codec::scenario_membership_manifest_v1::DEALER_SCENARIO_MEMBERSHIP_PAGE_MAX_ACCOUNTS_V1,
        ))
        .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
    if keys.len() < DEALER_SCENARIO_MEMBERSHIP_PAGES_V1 || keys.len() > maximum {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let base = keys.len() / DEALER_SCENARIO_MEMBERSHIP_PAGES_V1;
    let extra = keys.len() % DEALER_SCENARIO_MEMBERSHIP_PAGES_V1;
    let mut cursor = 0_usize;
    let mut pages: [Vec<Pubkey>; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1] =
        core::array::from_fn(|_| Vec::new());
    for (page_index, page) in pages.iter_mut().enumerate() {
        let count = base + usize::from(page_index < extra);
        let end = cursor
            .checked_add(count)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
        page.extend_from_slice(
            keys.get(cursor..end)
                .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
        );
        cursor = end;
    }
    if cursor != keys.len() {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let mut page_account_counts = [0_u8; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1];
    let mut page_membership_digests = [[0_u8; 32]; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1];
    for (page_index, page) in pages.iter().enumerate() {
        let page_index_u8 = u8::try_from(page_index)
            .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
        let count = u8::try_from(page.len())
            .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
        *page_account_counts
            .get_mut(page_index)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)? = count;
        *page_membership_digests
            .get_mut(page_index)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)? =
            dealer_scenario_membership_page_digest_v1(
                checkpoint,
                request_digest,
                page_index_u8,
                page,
            )?;
    }
    Ok(DealerScenarioCanonicalMembershipPagesV1 {
        manifest: DealerScenarioMembershipManifestV1 {
            producer_program: producer_program.to_bytes(),
            checkpoint: checkpoint.to_bytes(),
            request_digest,
            total_account_count: u16::try_from(keys.len())
                .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
            page_account_counts,
            page_membership_digests,
        },
        pages,
    })
}

/// Producer-owned manifest PDA for one checkpoint and request.
#[must_use]
pub fn dealer_scenario_membership_manifest_address_v1(
    producer_program: Pubkey,
    checkpoint: Pubkey,
    request_digest: [u8; 32],
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_MEMBERSHIP_MANIFEST_PDA_DOMAIN_V1,
            checkpoint.as_ref(),
            &request_digest,
        ],
        &producer_program,
    )
    .0
}

fn dealer_scenario_membership_page_digest_v1(
    checkpoint: Pubkey,
    request_digest: [u8; 32],
    page_index: u8,
    keys: &[Pubkey],
) -> Result<[u8; 32], DealerScenarioCheckpointOperatorErrorV1> {
    let page = [page_index];
    let count = [u8::try_from(keys.len())
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?];
    let mut parts = vec![
        DEALER_SCENARIO_MEMBERSHIP_PAGE_DOMAIN_V1,
        checkpoint.as_ref(),
        request_digest.as_slice(),
        page.as_slice(),
        count.as_slice(),
    ];
    parts.extend(keys.iter().map(Pubkey::as_ref));
    Ok(hashv(&parts).to_bytes())
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
    manifest_producer: Pubkey,
    membership_manifest: Pubkey,
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
            AccountMeta::new_readonly(manifest_producer, false),
            AccountMeta::new_readonly(membership_manifest, false),
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
#[allow(clippy::too_many_arguments)]
pub fn build_dealer_scenario_checkpoint_page_v1(
    trading_program: Pubkey,
    payer: Pubkey,
    checkpoint: Pubkey,
    clock: Pubkey,
    membership_manifest: Pubkey,
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
        || observations.contains(&membership_manifest)
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let mut accounts = Vec::with_capacity(3 + observations.len());
    accounts.push(AccountMeta::new(checkpoint, false));
    accounts.push(AccountMeta::new_readonly(clock, false));
    accounts.push(AccountMeta::new_readonly(membership_manifest, false));
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

/// Build and compile one Custody reservation-receipt ingestion transaction.
#[allow(clippy::too_many_arguments)]
pub fn build_dealer_scenario_checkpoint_reserve_v1(
    trading_program: Pubkey,
    payer: Pubkey,
    checkpoint: Pubkey,
    clock: Pubkey,
    custody_program: Pubkey,
    custody_programdata: Pubkey,
    activation_cache: Pubkey,
    registry_program: Pubkey,
    reservation_receipt: Pubkey,
    reservation_state: Pubkey,
    effect_producer: Pubkey,
    effect_manifest: Pubkey,
    effect_body: Pubkey,
    effect_ordinal: u8,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    build_dealer_scenario_checkpoint_reservation_route_v1(
        DealerScenarioCheckpointRouteV1::Reserve(effect_ordinal),
        DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1,
        trading_program,
        payer,
        checkpoint,
        clock,
        custody_program,
        custody_programdata,
        activation_cache,
        registry_program,
        reservation_receipt,
        reservation_state,
        effect_producer,
        effect_manifest,
        effect_body,
        recent_blockhash,
        lookup_tables,
    )
}

/// Build and compile one reverse-order rollback-receipt ingestion transaction.
#[allow(clippy::too_many_arguments)]
pub fn build_dealer_scenario_checkpoint_rollback_v1(
    trading_program: Pubkey,
    payer: Pubkey,
    checkpoint: Pubkey,
    clock: Pubkey,
    custody_program: Pubkey,
    custody_programdata: Pubkey,
    activation_cache: Pubkey,
    registry_program: Pubkey,
    rollback_receipt: Pubkey,
    reservation_state: Pubkey,
    effect_producer: Pubkey,
    effect_manifest: Pubkey,
    effect_body: Pubkey,
    effect_ordinal: u8,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    build_dealer_scenario_checkpoint_reservation_route_v1(
        DealerScenarioCheckpointRouteV1::Rollback(effect_ordinal),
        DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1,
        trading_program,
        payer,
        checkpoint,
        clock,
        custody_program,
        custody_programdata,
        activation_cache,
        registry_program,
        rollback_receipt,
        reservation_state,
        effect_producer,
        effect_manifest,
        effect_body,
        recent_blockhash,
        lookup_tables,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_dealer_scenario_checkpoint_reservation_route_v1(
    route: DealerScenarioCheckpointRouteV1,
    magic: [u8; 8],
    trading_program: Pubkey,
    payer: Pubkey,
    checkpoint: Pubkey,
    clock: Pubkey,
    custody_program: Pubkey,
    custody_programdata: Pubkey,
    activation_cache: Pubkey,
    registry_program: Pubkey,
    receipt: Pubkey,
    reservation_state: Pubkey,
    effect_producer: Pubkey,
    effect_manifest: Pubkey,
    effect_body: Pubkey,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    compile_checkpoint_packet(
        route,
        payer,
        Instruction {
            program_id: trading_program,
            accounts: vec![
                AccountMeta::new(checkpoint, false),
                AccountMeta::new_readonly(clock, false),
                AccountMeta::new_readonly(custody_program, false),
                AccountMeta::new_readonly(custody_programdata, false),
                AccountMeta::new_readonly(activation_cache, false),
                AccountMeta::new_readonly(registry_program, false),
                AccountMeta::new_readonly(receipt, false),
                AccountMeta::new_readonly(reservation_state, false),
                AccountMeta::new_readonly(effect_producer, false),
                AccountMeta::new_readonly(effect_manifest, false),
                AccountMeta::new_readonly(effect_body, false),
            ],
            data: magic.to_vec(),
        },
        recent_blockhash,
        lookup_tables,
    )
}

/// Build one atomic Custody producer plus release-authenticated Trading ingest.
///
/// The first instruction moves real token value (or reverses it after expiry)
/// and writes the typed receipt. The second instruction must immediately join
/// that receipt to the evaluator-owned effect bank and checkpoint. Transaction
/// rollback makes the pair atomic, while either durable producer output also
/// remains independently ingestible after an RPC-response loss.
pub fn build_dealer_scenario_reservation_bundle_v1(
    action: DealerScenarioReservationActionV1,
    effect_ordinal: u8,
    accounts: DealerScenarioReservationAccountsV1,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioReservationBundlePacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    let producer_data = encode_dealer_scenario_reservation_instruction_v1(action, effect_ordinal)
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let producer = Instruction {
        program_id: accounts.custody_program,
        accounts: vec![
            AccountMeta::new_readonly(accounts.market, false),
            AccountMeta::new_readonly(accounts.activation_cache, false),
            AccountMeta::new_readonly(accounts.registry_program, false),
            AccountMeta::new_readonly(accounts.trading_program, false),
            AccountMeta::new_readonly(accounts.trading_programdata, false),
            AccountMeta::new_readonly(accounts.realm, false),
            AccountMeta::new_readonly(accounts.realm_staging, false),
            AccountMeta::new_readonly(accounts.custody_replay, false),
            AccountMeta::new_readonly(accounts.checkpoint, false),
            AccountMeta::new_readonly(accounts.effect_producer, false),
            AccountMeta::new_readonly(accounts.effect_manifest, false),
            AccountMeta::new_readonly(accounts.effect_body, false),
            AccountMeta::new(accounts.batch, false),
            AccountMeta::new(accounts.reservation_state, false),
            AccountMeta::new(accounts.reservation_receipt, false),
            AccountMeta::new(accounts.source, false),
            AccountMeta::new_readonly(accounts.destination, false),
            AccountMeta::new(accounts.escrow, false),
            AccountMeta::new_readonly(accounts.mint, false),
            AccountMeta::new_readonly(accounts.custody_authority, false),
            AccountMeta::new_readonly(accounts.token_program, false),
            AccountMeta::new(accounts.payer, true),
            if action == DealerScenarioReservationActionV1::Rollback {
                AccountMeta::new(accounts.refund_beneficiary, false)
            } else {
                AccountMeta::new_readonly(accounts.refund_beneficiary, false)
            },
            AccountMeta::new_readonly(accounts.clock, false),
            AccountMeta::new_readonly(accounts.rent, false),
            AccountMeta::new_readonly(accounts.system_program, false),
        ],
        data: producer_data.to_vec(),
    };
    let ingest_magic = match action {
        DealerScenarioReservationActionV1::Reserve => DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1,
        DealerScenarioReservationActionV1::Rollback => DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1,
    };
    let ingest = Instruction {
        program_id: accounts.trading_program,
        accounts: vec![
            AccountMeta::new(accounts.checkpoint, false),
            AccountMeta::new_readonly(accounts.clock, false),
            AccountMeta::new_readonly(accounts.custody_program, false),
            AccountMeta::new_readonly(accounts.custody_programdata, false),
            AccountMeta::new_readonly(accounts.activation_cache, false),
            AccountMeta::new_readonly(accounts.registry_program, false),
            AccountMeta::new_readonly(accounts.reservation_receipt, false),
            AccountMeta::new_readonly(accounts.reservation_state, false),
            AccountMeta::new_readonly(accounts.effect_producer, false),
            AccountMeta::new_readonly(accounts.effect_manifest, false),
            AccountMeta::new_readonly(accounts.effect_body, false),
        ],
        data: ingest_magic.to_vec(),
    };
    let instructions = [producer, ingest];
    let census = census_dealer_scenario_transaction_locks_v1(accounts.payer, &instructions);
    require_dealer_scenario_devnet_lock_limit_v1(accounts.payer, &instructions)
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::LockLimit)?;
    let required_signers = signer_set_many(accounts.payer, &instructions);
    let message = v0::Message::try_compile(
        &accounts.payer,
        &instructions,
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
    Ok(DealerScenarioReservationBundlePacketV1 {
        action,
        effect_ordinal,
        instructions,
        message: versioned,
        wire_bytes,
        loaded_addresses,
        required_signers,
        lock_census: census,
    })
}

/// Build the bounded transaction which commits Claims and the obligation.
///
/// The embedded Claims caller authority is a Trading PDA and is therefore not
/// a top-level signer. Trading signs only the child CPI after every locked
/// Custody proof and candidate artifact has rejoined the checkpoint.
pub fn build_dealer_scenario_commit_v1(
    accounts: DealerScenarioCommitAccountsV1,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioCheckpointPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    let active = usize::from(accounts.effect_count);
    if active == 0 || active > DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let position_count = accounts
        .claims_accounts
        .len()
        .checked_sub(20)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| matches!(value, 1 | 2))
        .ok_or(DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let spec = SignedDeltaFrameSpecV3::new(position_count)
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    if usize::from(
        spec.account_count()
            .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
    ) != accounts.claims_accounts.len()
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    for (index, meta) in accounts.claims_accounts.iter().enumerate() {
        let expected = spec
            .account(
                u16::try_from(index)
                    .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?,
            )
            .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
        let privileges = expected.privileges();
        if meta.is_signer
            || meta.is_writable != privileges.writable()
            || (matches!(expected.role(), ClaimsFrameRoleV1::CallerProgram)
                && meta.pubkey != accounts.trading_program)
            || matches!(expected.role(), ClaimsFrameRoleV1::CallerAuthority)
                && (meta.is_signer || meta.is_writable)
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
        }
    }
    let default_effect = DealerScenarioCommitEffectAccountsV1::default();
    if accounts.effect_accounts.iter().take(active).any(|proof| {
        proof.reservation_receipt == Pubkey::default()
            || proof.reservation_state == Pubkey::default()
    }) || accounts
        .effect_accounts
        .iter()
        .skip(active)
        .any(|proof| *proof != default_effect)
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let mut metas = vec![
        AccountMeta::new(accounts.checkpoint, false),
        AccountMeta::new_readonly(accounts.clock, false),
        AccountMeta::new_readonly(accounts.request, false),
        AccountMeta::new_readonly(accounts.evaluation_receipt, false),
        AccountMeta::new_readonly(accounts.candidate_bank, false),
        AccountMeta::new_readonly(accounts.candidate_obligation, false),
        AccountMeta::new_readonly(accounts.claims_delta, false),
        AccountMeta::new_readonly(accounts.effects, false),
        AccountMeta::new_readonly(accounts.root, false),
        AccountMeta::new(accounts.obligation, false),
        AccountMeta::new_readonly(accounts.custody_program, false),
        AccountMeta::new_readonly(accounts.custody_programdata, false),
        AccountMeta::new_readonly(accounts.batch, false),
    ];
    metas.extend(accounts.claims_accounts);
    for proof in accounts.effect_accounts.iter().take(active) {
        metas.extend([
            AccountMeta::new_readonly(proof.reservation_receipt, false),
            AccountMeta::new_readonly(proof.reservation_state, false),
        ]);
    }
    let keys = metas.iter().map(|meta| meta.pubkey).collect::<Vec<_>>();
    if keys.contains(&Pubkey::default()) || has_duplicate_keys(&keys) {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    compile_checkpoint_packet(
        DealerScenarioCheckpointRouteV1::Commit,
        accounts.payer,
        Instruction {
            program_id: accounts.trading_program,
            accounts: metas,
            data: DEALER_SCENARIO_CHECKPOINT_COMMIT_MAGIC_V1.to_vec(),
        },
        recent_blockhash,
        lookup_tables,
    )
}

/// Build the one bounded transaction that delivers every reserved escrow.
///
/// The caller normally supplies an ALT because four effects resolve 38 locks
/// and do not fit the 1,232-byte packet ceiling as static addresses. The lock
/// census always counts the fully resolved accounts, never compressed bytes.
pub fn build_dealer_scenario_activation_v1(
    accounts: DealerScenarioActivationAccountsV1,
    recent_blockhash: Hash,
    lookup_tables: &[AddressLookupTableAccount],
) -> Result<DealerScenarioActivationPacketV1, DealerScenarioCheckpointOperatorErrorV1> {
    let active = usize::from(accounts.effect_count);
    if active == 0 || active > DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let default_effect = DealerScenarioActivationEffectAccountsV1::default();
    if accounts.effects.iter().take(active).any(|effect| {
        [
            effect.effect_body,
            effect.reservation_state,
            effect.escrow,
            effect.destination,
        ]
        .contains(&Pubkey::default())
    }) || accounts
        .effects
        .iter()
        .skip(active)
        .any(|effect| *effect != default_effect)
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let mut metas = vec![
        AccountMeta::new_readonly(accounts.market, false),
        AccountMeta::new_readonly(accounts.activation_cache, false),
        AccountMeta::new_readonly(accounts.registry_program, false),
        AccountMeta::new_readonly(accounts.trading_program, false),
        AccountMeta::new_readonly(accounts.trading_programdata, false),
        AccountMeta::new_readonly(accounts.realm, false),
        AccountMeta::new_readonly(accounts.realm_staging, false),
        AccountMeta::new(accounts.custody_replay, false),
        AccountMeta::new_readonly(accounts.checkpoint, false),
        AccountMeta::new_readonly(accounts.effect_producer, false),
        AccountMeta::new_readonly(accounts.effect_manifest, false),
        AccountMeta::new(accounts.batch, false),
        AccountMeta::new(accounts.activation_receipt, false),
        AccountMeta::new_readonly(accounts.mint, false),
        AccountMeta::new_readonly(accounts.custody_authority, false),
        AccountMeta::new_readonly(accounts.token_program, false),
        AccountMeta::new(accounts.payer, true),
        AccountMeta::new(accounts.refund_beneficiary, false),
        AccountMeta::new_readonly(accounts.rent, false),
        AccountMeta::new_readonly(accounts.system_program, false),
    ];
    for effect in accounts.effects.iter().take(active) {
        metas.extend([
            AccountMeta::new_readonly(effect.effect_body, false),
            AccountMeta::new(effect.reservation_state, false),
            AccountMeta::new(effect.escrow, false),
            AccountMeta::new(effect.destination, false),
        ]);
    }
    let keys = metas.iter().map(|meta| meta.pubkey).collect::<Vec<_>>();
    if accounts.custody_program == Pubkey::default()
        || keys.contains(&Pubkey::default())
        || has_duplicate_keys(&keys)
        || keys.contains(&accounts.custody_program)
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let instruction = Instruction {
        program_id: accounts.custody_program,
        accounts: metas,
        data: encode_dealer_scenario_activation_instruction_v1(accounts.effect_count)
            .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?
            .to_vec(),
    };
    let census = census_dealer_scenario_transaction_locks_v1(
        accounts.payer,
        core::slice::from_ref(&instruction),
    );
    require_dealer_scenario_devnet_lock_limit_v1(
        accounts.payer,
        core::slice::from_ref(&instruction),
    )
    .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::LockLimit)?;
    let required_signers = signer_set(accounts.payer, &instruction);
    let message = v0::Message::try_compile(
        &accounts.payer,
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
    Ok(DealerScenarioActivationPacketV1 {
        instruction,
        message: versioned,
        wire_bytes,
        loaded_addresses,
        required_signers,
        lock_census: census,
    })
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
    signer_set_many(payer, core::slice::from_ref(instruction))
}

fn signer_set_many(payer: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut signers = vec![payer];
    for signer in instructions
        .iter()
        .flat_map(|instruction| instruction.accounts.iter())
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
    /// Evaluator-selected Custody effect count.
    pub custody_effect_count: u8,
    /// Reservation receipt digests, overwritten by rollback receipts in reverse order.
    pub reservation_receipts: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    /// Number of finalized reservation ingestions.
    pub reservation_count: u8,
    /// Number of finalized reverse-order rollback ingestions.
    pub rollback_count: u8,
    /// Whether the atomic Claims/obligation transaction reached finalized `Committed`.
    pub committed: bool,
    /// Custody batch activation receipt digest, zero before activation.
    pub activation_receipt: [u8; 32],
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
            custody_effect_count: 0,
            reservation_receipts: [[0; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
            reservation_count: 0,
            rollback_count: 0,
            committed: false,
            activation_receipt: [0; 32],
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
        custody_effect_count: u8,
        checkpoint_poststate_digest: [u8; 32],
    ) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if usize::from(self.next_page) != DEALER_SCENARIO_PREPARATION_PAGES_V1
            || evaluation_receipt == [0; 32]
            || checkpoint_poststate_digest == [0; 32]
            || self.evaluation_receipt != [0; 32]
            || custody_effect_count == 0
            || usize::from(custody_effect_count) > DEALER_SCENARIO_MAX_RESERVATIONS_V1
            || self.cleaned
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        self.evaluation_receipt = evaluation_receipt;
        self.custody_effect_count = custody_effect_count;
        self.checkpoint_digest = checkpoint_poststate_digest;
        Ok(())
    }

    /// Record one finalized ordered Custody reservation.
    pub fn record_reservation(
        &mut self,
        effect_ordinal: u8,
        receipt_digest: [u8; 32],
        checkpoint_poststate_digest: [u8; 32],
    ) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if self.evaluation_receipt == [0; 32]
            || effect_ordinal != self.reservation_count
            || effect_ordinal >= self.custody_effect_count
            || receipt_digest == [0; 32]
            || checkpoint_poststate_digest == [0; 32]
            || self.rollback_count != 0
            || self.committed
            || self.activation_receipt != [0; 32]
            || self.cleaned
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        let destination = self
            .reservation_receipts
            .get_mut(usize::from(effect_ordinal))
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Journal)?;
        if *destination != [0; 32] {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        *destination = receipt_digest;
        self.reservation_count = self
            .reservation_count
            .checked_add(1)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
        self.checkpoint_digest = checkpoint_poststate_digest;
        Ok(())
    }

    /// Record one finalized reverse-order Custody rollback.
    pub fn record_rollback(
        &mut self,
        effect_ordinal: u8,
        prior_receipt_digest: [u8; 32],
        rollback_receipt_digest: [u8; 32],
        checkpoint_poststate_digest: [u8; 32],
    ) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        let expected = self
            .reservation_count
            .checked_sub(self.rollback_count)
            .and_then(|value| value.checked_sub(1))
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Journal)?;
        if effect_ordinal != expected
            || prior_receipt_digest == [0; 32]
            || rollback_receipt_digest == [0; 32]
            || checkpoint_poststate_digest == [0; 32]
            || self.committed
            || self.activation_receipt != [0; 32]
            || self.cleaned
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        let destination = self
            .reservation_receipts
            .get_mut(usize::from(effect_ordinal))
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Journal)?;
        if *destination != prior_receipt_digest {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        *destination = rollback_receipt_digest;
        self.rollback_count = self
            .rollback_count
            .checked_add(1)
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?;
        self.checkpoint_digest = checkpoint_poststate_digest;
        Ok(())
    }

    /// Record the finalized atomic Claims/obligation checkpoint transition.
    pub fn record_committed(
        &mut self,
        checkpoint_poststate_digest: [u8; 32],
    ) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if self.custody_effect_count == 0
            || self.reservation_count != self.custody_effect_count
            || self.rollback_count != 0
            || self.committed
            || self.activation_receipt != [0; 32]
            || checkpoint_poststate_digest == [0; 32]
            || self.cleaned
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        self.committed = true;
        self.checkpoint_digest = checkpoint_poststate_digest;
        Ok(())
    }

    /// Record finalized Custody activation only after liabilities committed.
    pub fn record_activated(
        &mut self,
        activation_receipt_digest: [u8; 32],
    ) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if self.custody_effect_count == 0
            || self.reservation_count != self.custody_effect_count
            || self.rollback_count != 0
            || !self.committed
            || activation_receipt_digest == [0; 32]
            || self.activation_receipt != [0; 32]
            || self.cleaned
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        self.activation_receipt = activation_receipt_digest;
        Ok(())
    }

    /// Record finalized expiry cleanup only after create occurred.
    pub fn record_cleaned(&mut self) -> Result<(), DealerScenarioCheckpointOperatorErrorV1> {
        if self.checkpoint_digest == [0; 32]
            || self.reservation_count != self.rollback_count
            || self.committed
            || self.activation_receipt != [0; 32]
            || self.cleaned
        {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Journal);
        }
        self.cleaned = true;
        Ok(())
    }
}

/// Producer-owned bodies one evaluation seals, as observed on chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioEvaluationBodiesV1<'a> {
    /// Exact selected scalar-then-identity candidate bank body.
    pub candidate_bank: &'a [u8],
    /// Exact candidate obligation body.
    pub candidate_obligation: &'a [u8],
    /// Exact expected Claims delta body.
    pub claims_delta: &'a [u8],
    /// Exact ordered Custody-effects manifest body.
    pub effects: &'a [u8],
}

/// The one canonical Custody-owned reservation batch address for a checkpoint.
///
/// Custody signs this account into existence under exactly these seeds, and
/// Trading authenticates it at exactly this address. Keeping the derivation in
/// one supported place is the point: the two programs disagreed here once,
/// Trading having named a third request-digest seed that Custody never signs,
/// which made every real batch unauthenticatable at commit. The checkpoint is
/// itself derived from the request digest, so the request is bound already.
#[must_use]
pub fn dealer_scenario_reservation_batch_address_v1(
    custody_program: Pubkey,
    checkpoint: Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
            checkpoint.as_ref(),
        ],
        &custody_program,
    )
    .0
}

/// Producer-owned evaluation receipt PDA for one checkpoint and request.
#[must_use]
pub fn dealer_scenario_evaluation_receipt_address_v1(
    producer_program: Pubkey,
    checkpoint: Pubkey,
    request_digest: [u8; 32],
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_EVALUATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint.as_ref(),
            &request_digest,
        ],
        &producer_program,
    )
    .0
}

/// Derive the one evaluation receipt Trading will accept for this checkpoint.
///
/// An evaluator can only seal a checkpoint it actually read: both prestate
/// digests fold the complete ordered page transcript the checkpoint already
/// carries, so a receipt cannot be written before the pages exist, and a
/// receipt written against a different transcript cannot pass. The four body
/// digests are taken from the bodies themselves, which is what stops a producer
/// from promising one candidate and publishing another.
///
/// The checkpoint body is the exact account data as observed on chain; its
/// SHA-256 is the prestate the receipt commits to, so any later mutation of the
/// checkpoint invalidates the receipt rather than silently re-binding it.
pub fn derive_dealer_scenario_evaluation_receipt_v1(
    producer_program: Pubkey,
    checkpoint: Pubkey,
    checkpoint_body: &[u8],
    bodies: DealerScenarioEvaluationBodiesV1<'_>,
    custody_effect_count: u8,
) -> Result<DealerScenarioEvaluationReceiptV1, DealerScenarioCheckpointOperatorErrorV1> {
    let active = usize::from(custody_effect_count);
    if producer_program == Pubkey::default()
        || checkpoint == Pubkey::default()
        || active == 0
        || active > DEALER_SCENARIO_MAX_RESERVATIONS_V1
        || bodies.candidate_bank.is_empty()
        || bodies.candidate_obligation.is_empty()
        || bodies.claims_delta.is_empty()
        || bodies.effects.is_empty()
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let state = DealerScenarioCheckpointV1::decode(checkpoint_body)
        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Geometry)?;
    let input = state.input();
    Ok(DealerScenarioEvaluationReceiptV1 {
        custody_effect_count,
        producer_program: producer_program.to_bytes(),
        checkpoint: checkpoint.to_bytes(),
        checkpoint_prestate_digest: hash(checkpoint_body).to_bytes(),
        request_digest: input.request_digest,
        claims_prestate_digest: joined_transcript_digest_v1(
            DEALER_SCENARIO_CLAIMS_PRESTATE_DOMAIN_V1,
            state,
        )?,
        custody_prestate_digest: joined_transcript_digest_v1(
            DEALER_SCENARIO_CUSTODY_PRESTATE_DOMAIN_V1,
            state,
        )?,
        candidate_bank_digest: hash(bodies.candidate_bank).to_bytes(),
        candidate_obligation_digest: hash(bodies.candidate_obligation).to_bytes(),
        claims_delta_digest: hash(bodies.claims_delta).to_bytes(),
        effects_digest: hash(bodies.effects).to_bytes(),
    })
}

/// Fold the complete ordered page transcript under one separation domain.
fn joined_transcript_digest_v1(
    domain: &[u8],
    state: DealerScenarioCheckpointV1,
) -> Result<[u8; 32], DealerScenarioCheckpointOperatorErrorV1> {
    let mut digests = Vec::with_capacity(DEALER_SCENARIO_PREPARATION_PAGES_V1);
    for page in 0..DEALER_SCENARIO_PREPARATION_PAGES_V1 {
        digests.push(
            state
                .page_receipt_digest(
                    u8::try_from(page)
                        .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
                )
                .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Journal)?,
        );
    }
    let input = state.input();
    let mut parts = vec![domain, input.request_digest.as_slice()];
    parts.extend(digests.iter().map(<[u8; 32]>::as_slice));
    Ok(hashv(&parts).to_bytes())
}

/// Producer-owned evidence one sealed evaluation names.
///
/// Every account here is readonly and owned by the evaluator program. Trading
/// re-derives the receipt PDA and rejects any body whose digest differs from
/// the one the receipt committed, so a caller cannot substitute an evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerAcceptedEvaluationAccountsV4 {
    /// Evaluator executable that owns the receipt and its evidence.
    pub producer_program: Pubkey,
    /// Producer-owned receipt PDA for this checkpoint and request.
    pub evaluation_receipt: Pubkey,
    /// Exact candidate register bank.
    pub candidate_bank: Pubkey,
    /// Exact candidate obligation body.
    pub candidate_obligation: Pubkey,
    /// Exact borrowed SignedDelta body.
    pub claims_delta: Pubkey,
    /// Complete Custody-effect manifest.
    pub effects: Pubkey,
}

/// One Custody reservation ingestion in canonical effect order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerAcceptedReservationAccountsV4 {
    /// Release-selected Custody program.
    pub custody_program: Pubkey,
    /// Custody ProgramData pinned by the release.
    pub custody_programdata: Pubkey,
    /// Registry activation cache.
    pub activation_cache: Pubkey,
    /// Registry executable.
    pub registry_program: Pubkey,
    /// Custody-owned reservation receipt.
    pub reservation_receipt: Pubkey,
    /// Custody-owned reservation state.
    pub reservation_state: Pubkey,
    /// Evaluator executable owning the effect bank.
    pub effect_producer: Pubkey,
    /// Complete effect manifest.
    pub effect_manifest: Pubkey,
    /// Exact single effect body.
    pub effect_body: Pubkey,
}

/// Complete unsigned input for one accepted Dealer scenario transition.
///
/// This is a scenario accepted-transition transcript. It selects no price,
/// quotes nothing, and holds no inventory: it is the ordered set of lock-bounded
/// transactions a durable caller submits to carry one already-authenticated
/// Dealer scenario request from checkpoint creation to a committed obligation.
#[derive(Clone, Copy, Debug)]
pub struct DealerAcceptedTranscriptInputV4<'a> {
    /// Release-selected Trading program that owns every route.
    pub trading_program: Pubkey,
    /// Transaction fee payer; no protocol authority is inferred from it.
    pub payer: Pubkey,
    /// Dealer Claims Position owner, the one wallet signer creation requires.
    pub dealer_authority: Pubkey,
    /// Immutable rent beneficiary named at creation.
    pub refund_beneficiary: Pubkey,
    /// Exact immutable Dealer request account.
    pub request: Pubkey,
    /// Digest of the exact request body, which seeds the checkpoint PDA.
    pub request_digest: [u8; 32],
    /// Immutable Trading-owned Dealer child root.
    pub root: Pubkey,
    /// Canonical Trading-owned obligation PDA.
    pub obligation: Pubkey,
    /// Clock sysvar.
    pub clock: Pubkey,
    /// Rent sysvar.
    pub rent: Pubkey,
    /// System program.
    pub system_program: Pubkey,
    /// Producer that owns the canonical membership manifest.
    pub manifest_producer: Pubkey,
    /// Producer-owned membership manifest PDA.
    pub membership_manifest: Pubkey,
    /// The one canonical membership partition, in page order.
    pub pages: &'a [Vec<Pubkey>; DEALER_SCENARIO_MEMBERSHIP_PAGES_V1],
    /// Producer-owned evidence the evaluation seals.
    pub evaluation: DealerAcceptedEvaluationAccountsV4,
    /// Ordered Custody reservations, one per evaluated effect.
    pub reservations: &'a [DealerAcceptedReservationAccountsV4],
    /// Complete final commit bank.
    pub commit: &'a DealerScenarioCommitAccountsV1,
    /// Blockhash used only to measure the compiled packet width.
    pub recent_blockhash: Hash,
    /// Address tables a caller resolves the wide routes through.
    pub lookup_tables: &'a [AddressLookupTableAccount],
}

/// One ordered, lock-bounded, packet-safe accepted-transition transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerAcceptedTranscriptV4 {
    /// Trading-owned checkpoint the whole transcript advances.
    pub checkpoint: Pubkey,
    /// Exact request digest every route re-derives.
    pub request_digest: [u8; 32],
    /// Planned durable journal, positioned before the first submission.
    pub journal: DealerScenarioCheckpointJournalV1,
    /// Canonical route order: create, six pages, evaluate, reservations, commit.
    pub packets: Vec<DealerScenarioCheckpointPacketV1>,
}

impl DealerAcceptedTranscriptV4 {
    /// Borrow the exact route order this transcript admits.
    #[must_use]
    pub fn routes(&self) -> Vec<DealerScenarioCheckpointRouteV1> {
        self.packets.iter().map(|packet| packet.route).collect()
    }

    /// Largest resolved lock count any single transaction in the transcript takes.
    #[must_use]
    pub fn peak_account_lock_count(&self) -> usize {
        self.packets
            .iter()
            .map(|packet| packet.lock_census.unique_account_lock_count)
            .max()
            .unwrap_or(0)
    }
}

/// Build the one canonical accepted Dealer scenario transition transcript.
///
/// The unsplit admitted Hot instruction for the same scenario resolves 121
/// account locks against a 64-lock runtime ceiling, which no address lookup
/// table changes. This builder is the submittable form of the same transition:
/// it emits the exact ordered checkpoint routes, and refuses the whole
/// transcript unless every constituent transaction is independently
/// lock-bounded and packet-safe.
///
/// It signs nothing and submits nothing. The returned journal is positioned
/// before creation so a durable caller can only advance it in route order.
pub fn build_dealer_accepted_transcript_v4(
    input: DealerAcceptedTranscriptInputV4<'_>,
) -> Result<DealerAcceptedTranscriptV4, DealerScenarioCheckpointOperatorErrorV1> {
    let reservation_count = input.reservations.len();
    if reservation_count == 0
        || reservation_count > DEALER_SCENARIO_MAX_RESERVATIONS_V1
        || usize::from(input.commit.effect_count) != reservation_count
        || input.commit.trading_program != input.trading_program
        || input.commit.checkpoint
            != dealer_scenario_checkpoint_address_v1(input.trading_program, input.request_digest)
        || input.commit.request != input.request
        || input.commit.root != input.root
        || input.commit.obligation != input.obligation
        || input.commit.evaluation_receipt != input.evaluation.evaluation_receipt
        || input.commit.candidate_bank != input.evaluation.candidate_bank
        || input.commit.candidate_obligation != input.evaluation.candidate_obligation
        || input.commit.claims_delta != input.evaluation.claims_delta
        || input.commit.effects != input.evaluation.effects
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let journal =
        DealerScenarioCheckpointJournalV1::planned(input.trading_program, input.request_digest)?;
    let checkpoint = journal.checkpoint;
    if input.membership_manifest
        != dealer_scenario_membership_manifest_address_v1(
            input.manifest_producer,
            checkpoint,
            input.request_digest,
        )
    {
        return Err(DealerScenarioCheckpointOperatorErrorV1::Geometry);
    }
    let mut packets = Vec::with_capacity(
        DEALER_SCENARIO_MEMBERSHIP_PAGES_V1
            .checked_add(reservation_count)
            .and_then(|value| value.checked_add(3))
            .ok_or(DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
    );
    packets.push(build_dealer_scenario_checkpoint_create_v1(
        input.trading_program,
        input.payer,
        input.dealer_authority,
        input.refund_beneficiary,
        checkpoint,
        input.request,
        input.root,
        input.obligation,
        input.clock,
        input.rent,
        input.system_program,
        input.manifest_producer,
        input.membership_manifest,
        input.recent_blockhash,
        input.lookup_tables,
    )?);
    for (page_index, page) in input.pages.iter().enumerate() {
        packets.push(build_dealer_scenario_checkpoint_page_v1(
            input.trading_program,
            input.payer,
            checkpoint,
            input.clock,
            input.membership_manifest,
            u8::try_from(page_index)
                .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
            page,
            input.recent_blockhash,
            input.lookup_tables,
        )?);
    }
    packets.push(build_dealer_scenario_checkpoint_evaluate_v1(
        input.trading_program,
        input.payer,
        checkpoint,
        input.clock,
        input.evaluation.producer_program,
        input.evaluation.evaluation_receipt,
        input.evaluation.candidate_bank,
        input.evaluation.candidate_obligation,
        input.evaluation.claims_delta,
        input.evaluation.effects,
        input.recent_blockhash,
        input.lookup_tables,
    )?);
    for (ordinal, reservation) in input.reservations.iter().enumerate() {
        packets.push(build_dealer_scenario_checkpoint_reserve_v1(
            input.trading_program,
            input.payer,
            checkpoint,
            input.clock,
            reservation.custody_program,
            reservation.custody_programdata,
            reservation.activation_cache,
            reservation.registry_program,
            reservation.reservation_receipt,
            reservation.reservation_state,
            reservation.effect_producer,
            reservation.effect_manifest,
            reservation.effect_body,
            u8::try_from(ordinal)
                .map_err(|_| DealerScenarioCheckpointOperatorErrorV1::Arithmetic)?,
            input.recent_blockhash,
            input.lookup_tables,
        )?);
    }
    packets.push(build_dealer_scenario_commit_v1(
        input.commit.clone(),
        input.recent_blockhash,
        input.lookup_tables,
    )?);
    for packet in &packets {
        if packet.lock_census.unique_account_lock_count > SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1 {
            return Err(DealerScenarioCheckpointOperatorErrorV1::LockLimit);
        }
        if packet.wire_bytes > DEALER_SCENARIO_PACKET_BYTES_V1 {
            return Err(DealerScenarioCheckpointOperatorErrorV1::Packet);
        }
    }
    Ok(DealerAcceptedTranscriptV4 {
        checkpoint,
        request_digest: input.request_digest,
        journal,
        packets,
    })
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
            key(12),
            key(13),
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
        let reserve = build_dealer_scenario_checkpoint_reserve_v1(
            key(1),
            key(2),
            key(5),
            key(9),
            key(18),
            key(19),
            key(20),
            key(21),
            key(22),
            key(23),
            key(24),
            key(25),
            key(26),
            0,
            blockhash(),
            &[],
        )
        .expect("reserve");
        let rollback = build_dealer_scenario_checkpoint_rollback_v1(
            key(1),
            key(2),
            key(5),
            key(9),
            key(18),
            key(19),
            key(20),
            key(21),
            key(27),
            key(23),
            key(24),
            key(25),
            key(26),
            0,
            blockhash(),
            &[],
        )
        .expect("rollback");
        assert_eq!(
            (
                create.wire_bytes,
                create.lock_census.unique_account_lock_count
            ),
            (607, 13)
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
        assert_eq!(
            (
                reserve.wire_bytes,
                reserve.lock_census.unique_account_lock_count
            ),
            (542, 13)
        );
        assert_eq!(
            (
                rollback.wire_bytes,
                rollback.lock_census.unique_account_lock_count
            ),
            (542, 13)
        );
        for packet in [create, evaluate, reserve, rollback, cleanup] {
            assert!(packet.wire_bytes <= DEALER_SCENARIO_PACKET_BYTES_V1);
            assert!(packet.lock_census.unique_account_lock_count <= 64);
            assert_eq!(packet.loaded_addresses, 0);
        }
    }

    #[test]
    fn real_reservation_bundle_includes_escrow_and_fits_devnet() {
        let accounts = DealerScenarioReservationAccountsV1 {
            custody_program: key(30),
            market: key(31),
            activation_cache: key(32),
            registry_program: key(33),
            trading_program: key(34),
            trading_programdata: key(35),
            realm: key(36),
            realm_staging: key(37),
            custody_replay: key(38),
            checkpoint: key(39),
            effect_producer: key(40),
            effect_manifest: key(41),
            effect_body: key(42),
            batch: key(43),
            reservation_state: key(44),
            reservation_receipt: key(45),
            source: key(46),
            destination: key(47),
            escrow: key(48),
            mint: key(49),
            custody_authority: key(50),
            token_program: key(51),
            payer: key(52),
            refund_beneficiary: key(53),
            clock: key(54),
            rent: key(55),
            system_program: key(56),
            custody_programdata: key(57),
        };
        let reserve = build_dealer_scenario_reservation_bundle_v1(
            DealerScenarioReservationActionV1::Reserve,
            0,
            accounts,
            blockhash(),
            &[],
        )
        .expect("reserve bundle");
        let rollback = build_dealer_scenario_reservation_bundle_v1(
            DealerScenarioReservationActionV1::Rollback,
            0,
            accounts,
            blockhash(),
            &[],
        )
        .expect("rollback bundle");
        assert_eq!(reserve.lock_census.unique_account_lock_count, 28);
        assert_eq!(rollback.lock_census.unique_account_lock_count, 28);
        assert_eq!(reserve.wire_bytes, 1_060);
        assert_eq!(rollback.wire_bytes, 1_060);
        assert!(!reserve.instructions[0].accounts[22].is_writable);
        assert!(rollback.instructions[0].accounts[22].is_writable);
    }

    #[test]
    fn four_effect_activation_uses_real_escrows_and_fits_with_alt() {
        let effects = [
            DealerScenarioActivationEffectAccountsV1 {
                effect_body: key(60),
                reservation_state: key(61),
                escrow: key(62),
                destination: key(63),
            },
            DealerScenarioActivationEffectAccountsV1 {
                effect_body: key(64),
                reservation_state: key(65),
                escrow: key(66),
                destination: key(67),
            },
            DealerScenarioActivationEffectAccountsV1 {
                effect_body: key(68),
                reservation_state: key(69),
                escrow: key(70),
                destination: key(71),
            },
            DealerScenarioActivationEffectAccountsV1 {
                effect_body: key(72),
                reservation_state: key(73),
                escrow: key(74),
                destination: key(75),
            },
        ];
        let accounts = DealerScenarioActivationAccountsV1 {
            custody_program: key(30),
            market: key(31),
            activation_cache: key(32),
            registry_program: key(33),
            trading_program: key(34),
            trading_programdata: key(35),
            realm: key(36),
            realm_staging: key(37),
            custody_replay: key(38),
            checkpoint: key(39),
            effect_producer: key(40),
            effect_manifest: key(41),
            batch: key(42),
            activation_receipt: key(43),
            mint: key(44),
            custody_authority: key(45),
            token_program: key(46),
            payer: key(47),
            refund_beneficiary: key(48),
            rent: key(50),
            system_program: key(51),
            effects,
            effect_count: 4,
        };
        assert_eq!(
            build_dealer_scenario_activation_v1(accounts, blockhash(), &[]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Packet)
        );
        let table = AddressLookupTableAccount {
            key: key(29),
            addresses: (31_u8..=75).map(key).collect(),
        };
        let packet = build_dealer_scenario_activation_v1(
            accounts,
            blockhash(),
            core::slice::from_ref(&table),
        )
        .expect("activation");
        assert_eq!(packet.lock_census.unique_account_lock_count, 37);
        assert_eq!(packet.wire_bytes, 285);
        assert_eq!(packet.loaded_addresses, 35);
        assert_eq!(packet.instruction.accounts.len(), 36);
        assert_eq!(packet.instruction.accounts[22].pubkey, key(62));
        assert!(packet.instruction.accounts[22].is_writable);
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
            key(5),
            0,
            &observations,
            blockhash(),
            core::slice::from_ref(&table),
        )
        .expect("paged packet");
        assert_eq!(packet.lock_census.unique_account_lock_count, 53);
        assert_eq!(packet.loaded_addresses, 48);
        assert_eq!(packet.wire_bytes, 409);
        assert!(packet.wire_bytes <= DEALER_SCENARIO_PACKET_BYTES_V1);

        let mut too_many = observations;
        too_many.push(key(68));
        assert_eq!(
            build_dealer_scenario_checkpoint_page_v1(
                key(1),
                key(2),
                key(3),
                key(4),
                key(5),
                0,
                &too_many,
                blockhash(),
                core::slice::from_ref(&table),
            ),
            Err(DealerScenarioCheckpointOperatorErrorV1::Geometry)
        );
    }

    #[test]
    fn committed_claims_and_obligation_fit_with_exact_custody_proofs() {
        let spec = SignedDeltaFrameSpecV3::new(2).expect("spec");
        let mut claims_accounts = Vec::new();
        for index in 0..spec.account_count().expect("count") {
            let account = spec.account(index).expect("account");
            let role = account.role();
            let pubkey = if role == ClaimsFrameRoleV1::CallerProgram {
                key(3)
            } else {
                key(60_u8
                    .checked_add(u8::try_from(index).expect("index"))
                    .expect("key"))
            };
            claims_accounts.push(if account.privileges().writable() {
                AccountMeta::new(pubkey, false)
            } else {
                AccountMeta::new_readonly(pubkey, false)
            });
        }
        let proofs = [
            DealerScenarioCommitEffectAccountsV1 {
                reservation_receipt: key(90),
                reservation_state: key(91),
            },
            DealerScenarioCommitEffectAccountsV1 {
                reservation_receipt: key(92),
                reservation_state: key(93),
            },
            DealerScenarioCommitEffectAccountsV1 {
                reservation_receipt: key(94),
                reservation_state: key(95),
            },
            DealerScenarioCommitEffectAccountsV1 {
                reservation_receipt: key(96),
                reservation_state: key(97),
            },
        ];
        let commit = DealerScenarioCommitAccountsV1 {
            trading_program: key(3),
            payer: key(2),
            checkpoint: key(4),
            clock: key(5),
            request: key(6),
            evaluation_receipt: key(7),
            candidate_bank: key(8),
            candidate_obligation: key(9),
            claims_delta: key(10),
            effects: key(11),
            root: key(12),
            obligation: key(13),
            custody_program: key(14),
            custody_programdata: key(15),
            batch: key(16),
            claims_accounts,
            effect_accounts: proofs,
            effect_count: 4,
        };
        let mut signer_hostile = commit.clone();
        signer_hostile.claims_accounts[0].is_signer = true;
        assert_eq!(
            build_dealer_scenario_commit_v1(signer_hostile, blockhash(), &[]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Geometry)
        );
        let caller_program_index = (0..spec.account_count().expect("count"))
            .find(|index| {
                spec.account(*index).expect("account").role() == ClaimsFrameRoleV1::CallerProgram
            })
            .expect("caller program index");
        let mut caller_hostile = commit.clone();
        caller_hostile.claims_accounts[usize::from(caller_program_index)].pubkey = key(250);
        assert_eq!(
            build_dealer_scenario_commit_v1(caller_hostile, blockhash(), &[]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Geometry)
        );
        assert_eq!(
            build_dealer_scenario_commit_v1(commit.clone(), blockhash(), &[]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Packet)
        );
        let table = AddressLookupTableAccount {
            key: key(1),
            addresses: (4_u8..=97).map(key).collect(),
        };
        let packet =
            build_dealer_scenario_commit_v1(commit, blockhash(), core::slice::from_ref(&table))
                .expect("commit");
        assert_eq!(packet.route, DealerScenarioCheckpointRouteV1::Commit);
        assert_eq!(packet.instruction.accounts.len(), 43);
        assert_eq!(packet.lock_census.unique_account_lock_count, 44);
        assert!(packet.wire_bytes <= DEALER_SCENARIO_PACKET_BYTES_V1);
        assert!(packet.loaded_addresses >= 40);
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
    fn the_reservation_batch_address_is_the_one_custody_signs() {
        // Custody creates this account with invoke_signed over exactly two
        // seeds. Trading once authenticated a three-seed address, so no batch
        // Custody could actually create was ever authenticatable at commit.
        // Pinning the seeds here is what stops the two drifting apart again.
        let custody = key(11);
        let checkpoint = key(12);
        assert_eq!(
            dealer_scenario_reservation_batch_address_v1(custody, checkpoint),
            Pubkey::find_program_address(
                &[
                    DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
                    checkpoint.as_ref(),
                ],
                &custody,
            )
            .0
        );
        assert_ne!(
            dealer_scenario_reservation_batch_address_v1(custody, checkpoint),
            Pubkey::find_program_address(
                &[
                    DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
                    checkpoint.as_ref(),
                    &[7_u8; 32],
                ],
                &custody,
            )
            .0,
            "a request-digest seed names an address Custody never signs"
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
            .record_evaluated([50; 32], 2, [51; 32])
            .expect("evaluation");
        journal
            .record_reservation(0, [60; 32], [61; 32])
            .expect("reservation zero");
        journal
            .record_reservation(1, [62; 32], [63; 32])
            .expect("reservation one");
        assert_eq!(
            journal.record_activated([64; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        let mut committed = journal.clone();
        committed.record_committed([64; 32]).expect("commit");
        assert_eq!(
            committed.record_rollback(1, [62; 32], [65; 32], [66; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        assert_eq!(
            committed.record_cleaned(),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        let mut activated = journal.clone();
        activated.record_committed([64; 32]).expect("commit");
        activated.record_activated([65; 32]).expect("activation");
        assert_eq!(
            activated.record_rollback(1, [62; 32], [65; 32], [66; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        assert_eq!(
            activated.record_cleaned(),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        assert_eq!(
            journal.record_cleaned(),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        assert_eq!(
            journal.record_rollback(0, [60; 32], [70; 32], [71; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
        journal
            .record_rollback(1, [62; 32], [72; 32], [73; 32])
            .expect("rollback one");
        journal
            .record_rollback(0, [60; 32], [74; 32], [75; 32])
            .expect("rollback zero");
        journal.record_cleaned().expect("cleanup");
        assert_eq!(
            journal.record_evaluated([52; 32], 2, [53; 32]),
            Err(DealerScenarioCheckpointOperatorErrorV1::Journal)
        );
    }
}
