//! Lock-bounded Trading checkpoint lifecycle for Dealer scenario evaluation.
//!
//! This module executes the durable mechanics which do not need the original
//! 121-lock selector-9 frame: a Dealer-authorized PDA creation, six ordered
//! readonly transcript pages, producer-bound evaluation sealing, exact locked
//! Custody proofs, atomic Claims/obligation commit, and permissionless
//! post-expiry cleanup or delivery.
//!
//! The evaluation receipt's owner and PDA authenticate one producer for that
//! receipt. The caller which invokes this route must additionally authenticate
//! that producer through the release-selected admitted-accelerator artifacts
//! and persist those artifacts under Trading ownership. Until that producer
//! route lands, the commit executor is real but Dealer acceptance is not a
//! complete caller-backed capability.

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

use dclutch_claims_svm::{
    CallerRole,
    frame_spec_v1::{ClaimsFrameRoleV1, SignedDeltaFrameSpecV3},
    signed_delta_v3::{SignedDeltaPlanV3, SignedDeltaReceiptV3},
};
use dclutch_core_contract::ContentId;
use dclutch_dealer_codec::{
    scenario_admission_v1::{
        DEALER_SCENARIO_ACTIVE_RESERVATION_ADMISSIBLE_STATES_V1,
        DEALER_SCENARIO_COMMIT_CHECKPOINT_ADMISSIBLE_STATES_V1,
    },
    scenario_checkpoint_v1::{
        DEALER_SCENARIO_CHECKPOINT_BYTES_V1, DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1,
        DEALER_SCENARIO_CLAIMS_PRESTATE_DOMAIN_V1, DEALER_SCENARIO_CUSTODY_PRESTATE_DOMAIN_V1,
        DEALER_SCENARIO_PAGE_BUMPS_BYTES_V1, DEALER_SCENARIO_PAGE_RECEIPT_DOMAIN_V1,
        DEALER_SCENARIO_PREPARATION_PAGES_V1, DealerScenarioCheckpointInputV1,
        DealerScenarioCheckpointV1, DealerScenarioCommitEvidenceV1, DealerScenarioEvaluationV1,
        DealerScenarioPageBumpsV1,
    },
    scenario_custody_reservation_v1::{
        DEALER_SCENARIO_CUSTODY_EFFECT_BYTES_V1, DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1,
        DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1,
        DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
        DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1, DealerScenarioCustodyEffectManifestV1,
        DealerScenarioCustodyEffectV1, DealerScenarioReservationBatchStatusV1,
        DealerScenarioReservationBatchV1, DealerScenarioReservationStateV1,
    },
    scenario_evaluation_receipt_v1::{
        DEALER_SCENARIO_EVALUATION_RECEIPT_PDA_DOMAIN_V1, DealerScenarioEvaluationReceiptV1,
    },
    scenario_membership_manifest_v1::{
        DEALER_SCENARIO_MEMBERSHIP_MANIFEST_PDA_DOMAIN_V1,
        DEALER_SCENARIO_MEMBERSHIP_PAGE_DOMAIN_V1, DealerScenarioMembershipManifestV1,
    },
    scenario_reservation_receipt_v1::{
        DEALER_SCENARIO_MAX_RESERVATIONS_V1, DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
        DealerScenarioReservationActionV1, DealerScenarioReservationReceiptV1,
    },
};
use dclutch_registry_activation_auth_v1::authenticate_activated_role_v1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{SysvarSerialize, clock::Clock},
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    TradingSbfError, child_refused_v1,
    claims_composition_v3::signed_delta_post_resource_digest,
    dealer::{
        v3_obligation::{DealerObligationProjectionV3, stage_scenario_obligation_replacement_v3},
        v3_trade::DealerScenarioTradeRequestV3,
    },
};

/// Create one request-scoped checkpoint PDA.
pub const DEALER_SCENARIO_CHECKPOINT_CREATE_MAGIC_V1: [u8; 8] = *b"DCLTDCP1";
/// Append the next canonical readonly transcript page.
pub const DEALER_SCENARIO_CHECKPOINT_PAGE_MAGIC_V1: [u8; 8] = *b"DCLTDPG1";
/// Seal one producer-bound evaluation after all pages exist.
pub const DEALER_SCENARIO_CHECKPOINT_EVALUATE_MAGIC_V1: [u8; 8] = *b"DCLTDEV1";
/// Close an expired checkpoint to its immutable beneficiary.
pub const DEALER_SCENARIO_CHECKPOINT_CLEANUP_MAGIC_V1: [u8; 8] = *b"DCLTDCL1";
/// Append one Custody reservation receipt.
///
/// `DCLTDRV1`, not `DCLTDRS1`. It carried `DCLTDRS1` until 2026-09-01, which is
/// also `dclutch_direct_codec::replay_setup_v1::DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1`
/// -- and BOTH are top-level selectors of this same Trading ELF. Nothing
/// separated them but instruction length and dispatch order: this arm is
/// `data == MAGIC`, exactly 8 bytes (`is_dealer_scenario_checkpoint_reserve_v1`),
/// the Direct arm is `len == 120 && data[..8] == MAGIC`, and this one is tried
/// first (`lib.rs:546` before `:601`). A mis-sized Direct replay-setup request
/// therefore did not refuse -- it routed into the Dealer family -- and any
/// future widening of this bare instruction would have collided silently.
///
/// Re-lettered rather than disambiguated by length, because a length test is a
/// guard whose two sides move together: it stays correct only while nobody
/// changes either width, and nothing would have failed if somebody did. The
/// census gate now refuses a magic claimed by two constants
/// (`tools/gauntlet/census/src/magics.rs`), so this cannot silently return.
pub const DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1: [u8; 8] = *b"DCLTDRV1";
/// Append one reverse-order Custody rollback receipt.
pub const DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1: [u8; 8] = *b"DCLTDRB1";
/// Atomically commit Claims and the exact candidate obligation against locked value.
pub const DEALER_SCENARIO_CHECKPOINT_COMMIT_MAGIC_V1: [u8; 8] = *b"DCLTDCM1";

/// Exact create instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_CREATE_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact page instruction width: magic, the canonical page ordinal, and the
/// producer's two mined PDA bumps.
pub const DEALER_SCENARIO_CHECKPOINT_PAGE_INSTRUCTION_BYTES_V1: usize =
    9 + DEALER_SCENARIO_PAGE_BUMPS_BYTES_V1;
/// Offset of the mined bump tail inside the page instruction.
const PAGE_BUMPS_OFFSET: usize = 9;
/// Exact evaluate instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_EVALUATE_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact cleanup instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_CLEANUP_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact reservation-ingest instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_RESERVE_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact rollback-ingest instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_ROLLBACK_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact final commit instruction width.
pub const DEALER_SCENARIO_CHECKPOINT_COMMIT_INSTRUCTION_BYTES_V1: usize = 8;

/// Exact create account count.
pub const DEALER_SCENARIO_CHECKPOINT_CREATE_ACCOUNT_COUNT_V1: usize = 12;
/// Fixed account prefix before one page's readonly observations.
pub const DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1: usize = 3;
/// Maximum readonly observations carried by one page transaction.
pub const DEALER_SCENARIO_CHECKPOINT_PAGE_MAX_OBSERVATIONS_V1: usize = 48;
/// Exact evaluation-seal account count.
pub const DEALER_SCENARIO_CHECKPOINT_EVALUATE_ACCOUNT_COUNT_V1: usize = 8;
/// Exact permissionless-cleanup account count.
pub const DEALER_SCENARIO_CHECKPOINT_CLEANUP_ACCOUNT_COUNT_V1: usize = 3;
/// Exact release-authenticated reservation or rollback receipt-ingest count.
pub const DEALER_SCENARIO_CHECKPOINT_RESERVATION_ACCOUNT_COUNT_V1: usize = 11;
/// Fixed final-commit prefix before the exact Claims child frame.
pub const DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1: usize = 13;
/// Two readonly Custody proofs follow the Claims frame for each effect.
pub const DEALER_SCENARIO_CHECKPOINT_COMMIT_EFFECT_ACCOUNTS_V1: usize = 2;

const CREATE_PAYER: usize = 0;
const CREATE_DEALER_AUTHORITY: usize = 1;
const CREATE_REFUND_BENEFICIARY: usize = 2;
const CREATE_CHECKPOINT: usize = 3;
const CREATE_REQUEST: usize = 4;
const CREATE_ROOT: usize = 5;
const CREATE_OBLIGATION: usize = 6;
const CREATE_CLOCK: usize = 7;
const CREATE_RENT: usize = 8;
const CREATE_SYSTEM: usize = 9;
const CREATE_MANIFEST_PRODUCER: usize = 10;
const CREATE_MANIFEST: usize = 11;

const PAGE_CHECKPOINT: usize = 0;
const PAGE_CLOCK: usize = 1;
const PAGE_MANIFEST: usize = 2;

const EVALUATE_CHECKPOINT: usize = 0;
const EVALUATE_CLOCK: usize = 1;
const EVALUATE_PRODUCER: usize = 2;
const EVALUATE_RECEIPT: usize = 3;
const EVALUATE_CANDIDATE_BANK: usize = 4;
const EVALUATE_CANDIDATE_OBLIGATION: usize = 5;
const EVALUATE_CLAIMS_DELTA: usize = 6;
const EVALUATE_EFFECTS: usize = 7;

const CLEANUP_CHECKPOINT: usize = 0;
const CLEANUP_BENEFICIARY: usize = 1;
const CLEANUP_CLOCK: usize = 2;

const RESERVATION_CHECKPOINT: usize = 0;
const RESERVATION_CLOCK: usize = 1;
const RESERVATION_PRODUCER: usize = 2;
const RESERVATION_PRODUCER_PROGRAMDATA: usize = 3;
const RESERVATION_ACTIVATION_CACHE: usize = 4;
const RESERVATION_REGISTRY: usize = 5;
const RESERVATION_RECEIPT: usize = 6;
const RESERVATION_STATE: usize = 7;
const RESERVATION_EFFECT_PRODUCER: usize = 8;
const RESERVATION_EFFECT_MANIFEST: usize = 9;
const RESERVATION_EFFECT_BODY: usize = 10;

const COMMIT_CHECKPOINT: usize = 0;
const COMMIT_CLOCK: usize = 1;
const COMMIT_REQUEST: usize = 2;
const COMMIT_EVALUATION_RECEIPT: usize = 3;
const COMMIT_CANDIDATE_BANK: usize = 4;
const COMMIT_CANDIDATE_OBLIGATION: usize = 5;
const COMMIT_CLAIMS_DELTA: usize = 6;
const COMMIT_EFFECTS: usize = 7;
const COMMIT_ROOT: usize = 8;
const COMMIT_OBLIGATION: usize = 9;
const COMMIT_CUSTODY_PROGRAM: usize = 10;
const COMMIT_CUSTODY_PROGRAMDATA: usize = 11;
const COMMIT_BATCH: usize = 12;

struct AuthenticatedDealerScenarioEvaluationV1 {
    claims_prestate_digest: [u8; 32],
    custody_prestate_digest: [u8; 32],
    receipt_digest: [u8; 32],
    evaluation: DealerScenarioEvaluationV1,
}

/// Return whether bytes select checkpoint creation.
#[must_use]
pub fn is_dealer_scenario_checkpoint_create_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_CREATE_MAGIC_V1
}

/// Return whether bytes select one ordered page append.
#[must_use]
pub fn is_dealer_scenario_checkpoint_page_v1(data: &[u8]) -> bool {
    data.len() == DEALER_SCENARIO_CHECKPOINT_PAGE_INSTRUCTION_BYTES_V1
        && data.get(..8) == Some(DEALER_SCENARIO_CHECKPOINT_PAGE_MAGIC_V1.as_slice())
}

/// Return whether bytes select evaluation sealing.
#[must_use]
pub fn is_dealer_scenario_checkpoint_evaluate_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_EVALUATE_MAGIC_V1
}

/// Return whether bytes select permissionless expiry cleanup.
#[must_use]
pub fn is_dealer_scenario_checkpoint_cleanup_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_CLEANUP_MAGIC_V1
}

/// Return whether bytes select reservation receipt ingestion.
#[must_use]
pub fn is_dealer_scenario_checkpoint_reserve_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1
}

/// Return whether bytes select rollback receipt ingestion.
#[must_use]
pub fn is_dealer_scenario_checkpoint_rollback_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_ROLLBACK_MAGIC_V1
}

/// Return whether bytes select the atomic Claims/obligation commit.
#[must_use]
pub fn is_dealer_scenario_checkpoint_commit_v1(data: &[u8]) -> bool {
    data == DEALER_SCENARIO_CHECKPOINT_COMMIT_MAGIC_V1
}

/// Create one Dealer-authorized request-scoped checkpoint PDA.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_create_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_create_v1(instruction_data)
        || accounts.len() != DEALER_SCENARIO_CHECKPOINT_CREATE_ACCOUNT_COUNT_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let payer = account(accounts, CREATE_PAYER)?;
    let dealer = account(accounts, CREATE_DEALER_AUTHORITY)?;
    let beneficiary = account(accounts, CREATE_REFUND_BENEFICIARY)?;
    let checkpoint_account = account(accounts, CREATE_CHECKPOINT)?;
    let request_account = account(accounts, CREATE_REQUEST)?;
    let root = account(accounts, CREATE_ROOT)?;
    let obligation = account(accounts, CREATE_OBLIGATION)?;
    let clock_account = account(accounts, CREATE_CLOCK)?;
    let rent_account = account(accounts, CREATE_RENT)?;
    let system = account(accounts, CREATE_SYSTEM)?;
    let manifest_producer = account(accounts, CREATE_MANIFEST_PRODUCER)?;
    let manifest_account = account(accounts, CREATE_MANIFEST)?;
    if !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || !dealer.is_signer
        || dealer.is_writable
        || dealer.executable
        || beneficiary.is_signer
        || beneficiary.is_writable
        || beneficiary.executable
        || !checkpoint_account.is_writable
        || checkpoint_account.is_signer
        || checkpoint_account.executable
        || checkpoint_account.owner != &system_program::ID
        || checkpoint_account.data_len() != 0
        || request_account.is_signer
        || request_account.is_writable
        || request_account.executable
        || root.is_signer
        || root.is_writable
        || root.executable
        || obligation.is_signer
        || obligation.is_writable
        || obligation.executable
        || system.key != &system_program::ID
        || !system.executable
        || !manifest_producer.executable
        || manifest_producer.is_signer
        || manifest_producer.is_writable
        || manifest_account.is_signer
        || manifest_account.is_writable
        || manifest_account.executable
        || manifest_account.owner != manifest_producer.key
        || has_duplicate_keys(accounts)
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_data = request_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let request = DealerScenarioTradeRequestV3::decode(&request_data)
        .map_err(|_| TradingSbfError::Content)?;
    if dealer.key.to_bytes() != request.dealer_owner
        || root.key.to_bytes() != request.child_root
        || root.owner != program_id
        || obligation.key.to_bytes() != request.obligation
        || obligation.owner != program_id
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_digest = hash(&request_data).to_bytes();
    let (expected_checkpoint, bump) = Pubkey::find_program_address(
        &[DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1, &request_digest],
        program_id,
    );
    if checkpoint_account.key != &expected_checkpoint {
        return Err(TradingSbfError::Content.into());
    }
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let manifest = DealerScenarioMembershipManifestV1::decode(&manifest_data)
        .map_err(|_| TradingSbfError::Content)?;
    let expected_manifest = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_MEMBERSHIP_MANIFEST_PDA_DOMAIN_V1,
            checkpoint_account.key.as_ref(),
            &request_digest,
        ],
        manifest_producer.key,
    )
    .0;
    if manifest_account.key != &expected_manifest
        || manifest.producer_program != manifest_producer.key.to_bytes()
        || manifest.checkpoint != checkpoint_account.key.to_bytes()
        || manifest.request_digest != request_digest
    {
        return Err(TradingSbfError::Content.into());
    }
    let membership_manifest_digest = hash(&manifest_data).to_bytes();
    let root_data = root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let obligation_data = obligation
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let obligation_digest = hash(&obligation_data).to_bytes();
    if obligation_digest != request.current_obligation_digest {
        return Err(TradingSbfError::Content.into());
    }
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let rent = Rent::from_account_info(rent_account).map_err(|_| TradingSbfError::Content)?;
    let checkpoint = DealerScenarioCheckpointV1::new(DealerScenarioCheckpointInputV1 {
        release_set: request.release_set,
        market: request.market,
        child_root: request.child_root,
        obligation: request.obligation,
        refund_beneficiary: beneficiary.key.to_bytes(),
        request_digest,
        membership_manifest_digest,
        root_prestate_digest: hash(&root_data).to_bytes(),
        claims_prestate_digest: [0; 32],
        obligation_prestate_digest: obligation_digest,
        custody_prestate_digest: [0; 32],
        generation: request.generation,
        created_slot: clock.slot,
        expires_at: request.expires_at,
    })
    .map_err(|_| TradingSbfError::Content)?;
    drop(obligation_data);
    drop(root_data);
    drop(request_data);
    drop(manifest_data);

    let minimum = rent.minimum_balance(DEALER_SCENARIO_CHECKPOINT_BYTES_V1);
    let deficit = minimum.saturating_sub(checkpoint_account.lamports());
    if deficit > 0 {
        invoke(
            &transfer(payer.key, checkpoint_account.key, deficit),
            &[payer.clone(), checkpoint_account.clone(), system.clone()],
        )
        .map_err(|_| TradingSbfError::Commit)?;
    }
    let bump_seed = [bump];
    let signer = [
        DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1,
        request_digest.as_slice(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &allocate(
            checkpoint_account.key,
            u64::try_from(DEALER_SCENARIO_CHECKPOINT_BYTES_V1)
                .map_err(|_| TradingSbfError::Commit)?,
        ),
        &[checkpoint_account.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Commit)?;
    invoke_signed(
        &assign(checkpoint_account.key, program_id),
        &[checkpoint_account.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Commit)?;
    write_new_checkpoint(program_id, checkpoint_account, checkpoint)?;
    set_return_data(&request_digest);
    Ok(())
}

/// Append one canonical transcript page, deriving its receipt from observations.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_page_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_page_v1(instruction_data)
        || accounts.len() <= DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1
        || accounts.len()
            > DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1
                + DEALER_SCENARIO_CHECKPOINT_PAGE_MAX_OBSERVATIONS_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let page_index = *instruction_data
        .get(8)
        .ok_or(TradingSbfError::UnsupportedContent)?;
    // Reading a hint must not be able to refuse: a short or absent tail yields
    // the absent bank and both derivations search exactly as they used to.
    let bumps = DealerScenarioPageBumpsV1::from_tail(
        instruction_data
            .get(PAGE_BUMPS_OFFSET..)
            .unwrap_or_default(),
    );
    let checkpoint_account = account(accounts, PAGE_CHECKPOINT)?;
    let clock_account = account(accounts, PAGE_CLOCK)?;
    let manifest_account = account(accounts, PAGE_MANIFEST)?;
    let observations = accounts
        .get(DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1..)
        .ok_or(TradingSbfError::Content)?;
    let (checkpoint, prestate_digest) = read_checkpoint(program_id, checkpoint_account)?;
    require_checkpoint_pda_hinted(program_id, checkpoint_account, checkpoint, bumps.checkpoint)?;
    let (receipt_digest, last_membership_key) = authenticate_membership_page_v1(
        checkpoint_account,
        clock_account,
        manifest_account,
        observations,
        &checkpoint,
        prestate_digest,
        page_index,
        bumps.membership_manifest,
    )?;
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let next = checkpoint
        .append_page(
            clock.slot,
            page_index,
            prestate_digest,
            receipt_digest,
            last_membership_key,
        )
        .map_err(|_| TradingSbfError::Transition)?;
    write_checkpoint_last(program_id, checkpoint_account, checkpoint, next)?;
    set_return_data(&receipt_digest);
    Ok(())
}

#[inline(never)]
fn authenticate_membership_page_v1(
    checkpoint_account: &AccountInfo<'_>,
    clock_account: &AccountInfo<'_>,
    manifest_account: &AccountInfo<'_>,
    observations: &[AccountInfo<'_>],
    checkpoint: &DealerScenarioCheckpointV1,
    prestate_digest: [u8; 32],
    page_index: u8,
    manifest_bump: u8,
) -> Result<([u8; 32], [u8; 32]), ProgramError> {
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let manifest = DealerScenarioMembershipManifestV1::decode(&manifest_data)
        .map_err(|_| TradingSbfError::Content)?;
    let input = checkpoint.input();
    let manifest_producer = Pubkey::new_from_array(manifest.producer_program);
    let manifest_seeds: [&[u8]; 3] = [
        DEALER_SCENARIO_MEMBERSHIP_MANIFEST_PDA_DOMAIN_V1,
        checkpoint_account.key.as_ref(),
        input.request_digest.as_slice(),
    ];
    // The same derivation at a mined bump. The producer owns this account and
    // derived its address before it could name it in a manifest, so it holds
    // the bump already; the equality below is unchanged and still the check.
    let expected_manifest = if manifest_bump == 0 {
        Pubkey::find_program_address(&manifest_seeds, &manifest_producer).0
    } else {
        let bump = [manifest_bump];
        Pubkey::create_program_address(
            &[
                manifest_seeds[0],
                manifest_seeds[1],
                manifest_seeds[2],
                bump.as_slice(),
            ],
            &manifest_producer,
        )
        .map_err(|_| TradingSbfError::Content)?
    };
    if manifest_account.is_signer
        || manifest_account.is_writable
        || manifest_account.executable
        || manifest_account.owner.to_bytes() != manifest.producer_program
        || manifest_account.key != &expected_manifest
        || hash(&manifest_data).to_bytes() != input.membership_manifest_digest
        || manifest.checkpoint != checkpoint_account.key.to_bytes()
        || manifest.request_digest != input.request_digest
        || manifest
            .page_account_counts
            .get(usize::from(page_index))
            .copied()
            != u8::try_from(observations.len()).ok()
        || !strictly_increasing_membership(checkpoint.last_membership_key(), observations)
        || manifest
            .page_membership_digests
            .get(usize::from(page_index))
            .copied()
            != Some(membership_page_digest(
                checkpoint_account.key,
                input.request_digest,
                page_index,
                observations,
            )?)
    {
        return Err(TradingSbfError::Content.into());
    }
    if observations.iter().any(|current| {
        current.is_signer
            || current.is_writable
            || current.key == checkpoint_account.key
            || current.key == clock_account.key
    }) || has_duplicate_keys(observations)
    {
        return Err(TradingSbfError::Content.into());
    }
    let receipt_digest = page_receipt_digest(
        checkpoint_account.key,
        *checkpoint,
        prestate_digest,
        page_index,
        observations,
    )?;
    let last_membership_key = observations
        .last()
        .ok_or(TradingSbfError::Content)?
        .key
        .to_bytes();
    drop(manifest_data);
    Ok((receipt_digest, last_membership_key))
}

/// Seal one producer-owned evaluation receipt after all six pages exist.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_evaluate_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_evaluate_v1(instruction_data)
        || accounts.len() != DEALER_SCENARIO_CHECKPOINT_EVALUATE_ACCOUNT_COUNT_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let checkpoint_account = account(accounts, EVALUATE_CHECKPOINT)?;
    let clock_account = account(accounts, EVALUATE_CLOCK)?;
    let producer = account(accounts, EVALUATE_PRODUCER)?;
    let receipt_account = account(accounts, EVALUATE_RECEIPT)?;
    let candidate_bank = account(accounts, EVALUATE_CANDIDATE_BANK)?;
    let candidate_obligation = account(accounts, EVALUATE_CANDIDATE_OBLIGATION)?;
    let claims_delta = account(accounts, EVALUATE_CLAIMS_DELTA)?;
    let effects = account(accounts, EVALUATE_EFFECTS)?;
    if !producer.executable
        || producer.is_signer
        || producer.is_writable
        || [
            receipt_account,
            candidate_bank,
            candidate_obligation,
            claims_delta,
            effects,
        ]
        .iter()
        .any(|current| {
            current.is_signer
                || current.is_writable
                || current.executable
                || current.owner != producer.key
        })
    {
        return Err(TradingSbfError::Content.into());
    }
    let (checkpoint, prestate_digest) = read_checkpoint(program_id, checkpoint_account)?;
    require_checkpoint_pda(program_id, checkpoint_account, checkpoint)?;
    if has_duplicate_keys(accounts) {
        return Err(TradingSbfError::Content.into());
    }
    let authenticated = authenticate_dealer_scenario_evaluation_v1(
        checkpoint_account,
        producer,
        receipt_account,
        candidate_bank,
        candidate_obligation,
        claims_delta,
        effects,
        &checkpoint,
        prestate_digest,
    )?;
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let next = checkpoint
        .finish_evaluation(
            clock.slot,
            prestate_digest,
            authenticated.claims_prestate_digest,
            authenticated.custody_prestate_digest,
            authenticated.evaluation,
        )
        .map_err(|_| TradingSbfError::Transition)?;
    write_checkpoint_last(program_id, checkpoint_account, checkpoint, next)?;
    set_return_data(&authenticated.receipt_digest);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_dealer_scenario_evaluation_v1(
    checkpoint_account: &AccountInfo<'_>,
    producer: &AccountInfo<'_>,
    receipt_account: &AccountInfo<'_>,
    candidate_bank: &AccountInfo<'_>,
    candidate_obligation: &AccountInfo<'_>,
    claims_delta: &AccountInfo<'_>,
    effects: &AccountInfo<'_>,
    checkpoint: &DealerScenarioCheckpointV1,
    prestate_digest: [u8; 32],
) -> Result<AuthenticatedDealerScenarioEvaluationV1, ProgramError> {
    let input = checkpoint.input();
    let expected_receipt = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_EVALUATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint_account.key.as_ref(),
            &input.request_digest,
        ],
        producer.key,
    )
    .0;
    if receipt_account.key != &expected_receipt {
        return Err(TradingSbfError::Content.into());
    }
    // Both domain-separated prestates commit the complete canonical membership
    // transcript. Once keys are globally sorted for cross-page uniqueness,
    // physical page ordinals no longer imply Claims-versus-Custody ownership.
    let claims_prestate_digest = joined_page_digest(
        DEALER_SCENARIO_CLAIMS_PRESTATE_DOMAIN_V1,
        *checkpoint,
        0,
        DEALER_SCENARIO_PREPARATION_PAGES_V1,
    )?;
    let custody_prestate_digest = joined_page_digest(
        DEALER_SCENARIO_CUSTODY_PRESTATE_DOMAIN_V1,
        *checkpoint,
        0,
        DEALER_SCENARIO_PREPARATION_PAGES_V1,
    )?;
    let receipt_data = receipt_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let receipt = DealerScenarioEvaluationReceiptV1::decode(&receipt_data)
        .map_err(|_| TradingSbfError::Content)?;
    let candidate_bank_digest = account_data_digest(candidate_bank)?;
    let candidate_obligation_digest = account_data_digest(candidate_obligation)?;
    let claims_delta_digest = account_data_digest(claims_delta)?;
    let effects_digest = account_data_digest(effects)?;
    if effects.data_len() != DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1 {
        return Err(TradingSbfError::Content.into());
    }
    let effects_data = effects
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let effects_manifest = DealerScenarioCustodyEffectManifestV1::decode(&effects_data)
        .map_err(|_| TradingSbfError::Content)?;
    drop(effects_data);
    if receipt.producer_program != producer.key.to_bytes()
        || receipt.checkpoint != checkpoint_account.key.to_bytes()
        || receipt.checkpoint_prestate_digest != prestate_digest
        || receipt.request_digest != input.request_digest
        || receipt.claims_prestate_digest != claims_prestate_digest
        || receipt.custody_prestate_digest != custody_prestate_digest
        || receipt.candidate_bank_digest != candidate_bank_digest
        || receipt.candidate_obligation_digest != candidate_obligation_digest
        || receipt.claims_delta_digest != claims_delta_digest
        || receipt.effects_digest != effects_digest
        || effects_manifest.producer_program != producer.key.to_bytes()
        || effects_manifest.checkpoint != checkpoint_account.key.to_bytes()
        || effects_manifest.request_digest != input.request_digest
        || effects_manifest.effect_count != receipt.custody_effect_count
    {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt_digest = hash(&receipt_data).to_bytes();
    drop(receipt_data);
    Ok(AuthenticatedDealerScenarioEvaluationV1 {
        claims_prestate_digest,
        custody_prestate_digest,
        receipt_digest,
        evaluation: DealerScenarioEvaluationV1 {
            custody_effect_count: receipt.custody_effect_count,
            evaluation_receipt_digest: receipt_digest,
            candidate_bank_digest,
            candidate_obligation_digest,
            claims_delta_digest,
            effects_digest,
        },
    })
}

/// Ingest one producer-owned Custody reservation receipt.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_reserve_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    process_dealer_scenario_checkpoint_reservation_receipt_v1(
        program_id,
        accounts,
        instruction_data,
        DealerScenarioReservationActionV1::Reserve,
    )
}

/// Ingest one reverse-order Custody rollback receipt after expiry.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_rollback_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    process_dealer_scenario_checkpoint_reservation_receipt_v1(
        program_id,
        accounts,
        instruction_data,
        DealerScenarioReservationActionV1::Rollback,
    )
}

fn process_dealer_scenario_checkpoint_reservation_receipt_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    expected_action: DealerScenarioReservationActionV1,
) -> Result<(), ProgramError> {
    let selector_matches = match expected_action {
        DealerScenarioReservationActionV1::Reserve => {
            is_dealer_scenario_checkpoint_reserve_v1(instruction_data)
        }
        DealerScenarioReservationActionV1::Rollback => {
            is_dealer_scenario_checkpoint_rollback_v1(instruction_data)
        }
    };
    if !selector_matches
        || accounts.len() != DEALER_SCENARIO_CHECKPOINT_RESERVATION_ACCOUNT_COUNT_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let checkpoint_account = account(accounts, RESERVATION_CHECKPOINT)?;
    let clock_account = account(accounts, RESERVATION_CLOCK)?;
    let producer = account(accounts, RESERVATION_PRODUCER)?;
    let producer_programdata = account(accounts, RESERVATION_PRODUCER_PROGRAMDATA)?;
    let activation_cache = account(accounts, RESERVATION_ACTIVATION_CACHE)?;
    let registry = account(accounts, RESERVATION_REGISTRY)?;
    let receipt_account = account(accounts, RESERVATION_RECEIPT)?;
    let reservation_state = account(accounts, RESERVATION_STATE)?;
    let effect_producer = account(accounts, RESERVATION_EFFECT_PRODUCER)?;
    let effect_manifest = account(accounts, RESERVATION_EFFECT_MANIFEST)?;
    let effect_body = account(accounts, RESERVATION_EFFECT_BODY)?;
    if !producer.executable
        || producer.is_signer
        || producer.is_writable
        || producer_programdata.is_signer
        || producer_programdata.is_writable
        || producer_programdata.executable
        || activation_cache.is_signer
        || activation_cache.is_writable
        || activation_cache.executable
        || registry.is_signer
        || registry.is_writable
        || !registry.executable
        || receipt_account.is_signer
        || receipt_account.executable
        || receipt_account.owner != producer.key
        || reservation_state.is_signer
        || reservation_state.executable
        || reservation_state.owner != producer.key
        || !effect_producer.executable
        || effect_producer.is_signer
        || effect_producer.is_writable
        || effect_manifest.is_signer
        || effect_manifest.is_writable
        || effect_manifest.executable
        || effect_manifest.owner != effect_producer.key
        || effect_manifest.data_len() != DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1
        || effect_body.is_signer
        || effect_body.is_writable
        || effect_body.executable
        || effect_body.owner != effect_producer.key
        || effect_body.data_len() != DEALER_SCENARIO_CUSTODY_EFFECT_BYTES_V1
        || has_duplicate_keys(accounts)
    {
        return Err(TradingSbfError::Content.into());
    }
    // `receipt_account` and `reservation_state` are deliberately privilege-
    // tolerant here.  This route only reads them, but the canonical atomic
    // bundle runs immediately after Custody creates both accounts in the same
    // transaction.  SVM merges privileges transaction-wide, so the AccountInfo
    // values observed here are writable even though this instruction's metas
    // are readonly.  Their Custody owner, exact PDA/body joins, and non-signer/
    // non-executable properties below remain the authority boundary; requiring
    // readonly would make the producer-plus-ingest transaction unreachable.
    let (checkpoint, checkpoint_prestate_digest) = read_checkpoint(program_id, checkpoint_account)?;
    require_checkpoint_pda(program_id, checkpoint_account, checkpoint)?;
    let input = checkpoint.input();
    let release_receipt = authenticate_activated_role_v1(
        registry,
        activation_cache,
        &input.release_set,
        ExecutionRoleV1::Custody,
        producer,
        producer_programdata,
    )
    .map_err(|_| TradingSbfError::Release)?;
    if release_receipt.program().as_bytes() != producer.key.as_ref() {
        return Err(TradingSbfError::Release.into());
    }
    let receipt_data = receipt_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let receipt = DealerScenarioReservationReceiptV1::decode(&receipt_data)
        .map_err(|_| TradingSbfError::Content)?;
    let action_seed = [expected_action as u8];
    let ordinal_seed = [receipt.effect_ordinal];
    let expected_receipt = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint_account.key.as_ref(),
            &input.request_digest,
            action_seed.as_slice(),
            ordinal_seed.as_slice(),
        ],
        producer.key,
    )
    .0;
    authenticate_reservation_effect_bank_v1(
        effect_producer,
        effect_manifest,
        effect_body,
        checkpoint_account,
        &checkpoint,
        &receipt,
    )?;
    if receipt.action != expected_action
        || receipt_account.key != &expected_receipt
        || receipt.producer_program != producer.key.to_bytes()
        || receipt.checkpoint != checkpoint_account.key.to_bytes()
        || receipt.checkpoint_prestate_digest != checkpoint_prestate_digest
        || receipt.request_digest != input.request_digest
        || receipt.effects_digest != checkpoint.evaluation().effects_digest
        || receipt.effect_count != checkpoint.evaluation().custody_effect_count
        || receipt.reservation != reservation_state.key.to_bytes()
        || account_data_digest(reservation_state)? != receipt.reservation_poststate_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt_digest = hash(&receipt_data).to_bytes();
    drop(receipt_data);
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let next = match expected_action {
        DealerScenarioReservationActionV1::Reserve => checkpoint.append_reservation(
            clock.slot,
            receipt.effect_ordinal,
            checkpoint_prestate_digest,
            receipt_digest,
        ),
        DealerScenarioReservationActionV1::Rollback => checkpoint.append_rollback(
            clock.slot,
            receipt.effect_ordinal,
            checkpoint_prestate_digest,
            receipt.prior_receipt_digest,
            receipt_digest,
        ),
    }
    .map_err(|_| TradingSbfError::Transition)?;
    write_checkpoint_last(program_id, checkpoint_account, checkpoint, next)?;
    set_return_data(&receipt_digest);
    Ok(())
}

#[inline(never)]
fn authenticate_reservation_effect_bank_v1(
    producer: &AccountInfo<'_>,
    manifest_account: &AccountInfo<'_>,
    body_account: &AccountInfo<'_>,
    checkpoint_account: &AccountInfo<'_>,
    checkpoint: &DealerScenarioCheckpointV1,
    receipt: &DealerScenarioReservationReceiptV1,
) -> Result<(), ProgramError> {
    let input = checkpoint.input();
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let manifest_digest = hash(&manifest_data).to_bytes();
    let manifest = DealerScenarioCustodyEffectManifestV1::decode(&manifest_data)
        .map_err(|_| TradingSbfError::Content)?;
    drop(manifest_data);
    let body_data = body_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let body_digest = hash(&body_data).to_bytes();
    let body =
        DealerScenarioCustodyEffectV1::decode(&body_data).map_err(|_| TradingSbfError::Content)?;
    let effect_index = usize::from(receipt.effect_ordinal);
    if manifest_digest != checkpoint.evaluation().effects_digest
        || manifest.producer_program != producer.key.to_bytes()
        || manifest.checkpoint != checkpoint_account.key.to_bytes()
        || manifest.request_digest != input.request_digest
        || manifest.effect_count != receipt.effect_count
        || manifest.effect_accounts.get(effect_index).copied() != Some(body_account.key.to_bytes())
        || manifest.effect_digests.get(effect_index).copied() != Some(body_digest)
        || body.producer_program != producer.key.to_bytes()
        || body.checkpoint != checkpoint_account.key.to_bytes()
        || body.request_digest != input.request_digest
        || body.ordinal != receipt.effect_ordinal
        || body.effect_count != receipt.effect_count
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

struct PreparedDealerScenarioCommitV1 {
    checkpoint_prestate_digest: [u8; 32],
    next_checkpoint: Vec<u8>,
    candidate_obligation: Vec<u8>,
    claims_wire: Vec<u8>,
    authority_seeds: CallerAuthoritySeedsV1,
    authority_bump: u8,
    claims_account_count: usize,
    claims_program_index: usize,
}

#[derive(Default)]
struct LockedBatchCommitContextV1 {
    checkpoint: [u8; 32],
    batch: [u8; 32],
    release_set: [u8; 32],
    market: [u8; 32],
    request_digest: [u8; 32],
    effects_digest: [u8; 32],
    refund_beneficiary: [u8; 32],
    generation: u64,
    expires_at: u64,
    effect_count: u8,
    receipt_digests: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    checkpoint_receipt_digests: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    reservation_states: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
    effect_digests: [[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1],
}

/// Atomically commit the authenticated Claims delta and exact obligation body.
///
/// Custody value has already moved into checkpoint-scoped escrows. This route
/// proves that complete locked batch, invokes the current Claims release, then
/// writes the candidate obligation and `Committed` checkpoint last. Delivery
/// remains permissionless and resumable in the separate Custody batch route.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_commit_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_commit_v1(instruction_data) {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let prepared = prepare_dealer_scenario_commit_v1(program_id, accounts)?;
    execute_dealer_scenario_claims_v1(accounts, &prepared)?;
    let obligation = account(accounts, COMMIT_OBLIGATION)?;
    let checkpoint_account = account(accounts, COMMIT_CHECKPOINT)?;
    {
        let mut destination = obligation
            .try_borrow_mut_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if destination.len() != prepared.candidate_obligation.len() {
            return Err(TradingSbfError::Commit.into());
        }
        destination.copy_from_slice(&prepared.candidate_obligation);
    }
    write_prepared_checkpoint_v1(
        program_id,
        checkpoint_account,
        prepared.checkpoint_prestate_digest,
        &prepared.next_checkpoint,
    )?;
    set_return_data(&hash(&prepared.next_checkpoint).to_bytes());
    Ok(())
}

#[inline(never)]
fn prepare_dealer_scenario_commit_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<Box<PreparedDealerScenarioCommitV1>, ProgramError> {
    let (spec, claims_account_count, effect_count) = commit_geometry_v1(program_id, accounts)?;
    let expected_accounts = DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1
        .checked_add(claims_account_count)
        .and_then(|value| {
            value.checked_add(
                effect_count.checked_mul(DEALER_SCENARIO_CHECKPOINT_COMMIT_EFFECT_ACCOUNTS_V1)?,
            )
        })
        .ok_or(TradingSbfError::Content)?;
    if effect_count == 0 || accounts.len() != expected_accounts || has_duplicate_keys(accounts) {
        return Err(TradingSbfError::Content.into());
    }
    require_commit_fixed_privileges(program_id, accounts)?;
    let claims_end = DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1
        .checked_add(claims_account_count)
        .ok_or(TradingSbfError::Content)?;
    require_commit_claims_privileges_v1(accounts, spec, claims_end)?;
    authenticate_commit_releases_v1(program_id, accounts, spec, claims_end)?;
    authenticate_commit_artifacts_v1(program_id, accounts)?;
    let candidate_obligation = prepare_candidate_obligation_v1(program_id, accounts)?;
    let custody_program = account(accounts, COMMIT_CUSTODY_PROGRAM)?;
    let reservation_receipts =
        authenticate_locked_batch_v1(program_id, accounts, claims_end, custody_program)?;
    let (checkpoint_prestate_digest, next_checkpoint) =
        prepare_committed_checkpoint_v1(program_id, accounts, reservation_receipts)?;
    let (claims_wire, authority_seeds, authority_bump) =
        prepare_claims_authority_v1(program_id, accounts, claims_account_count)?;
    let claims_program_index = claims_role_index(spec, ClaimsFrameRoleV1::ClaimsProgram)?;
    Ok(Box::new(PreparedDealerScenarioCommitV1 {
        checkpoint_prestate_digest,
        next_checkpoint,
        candidate_obligation,
        claims_wire,
        authority_seeds,
        authority_bump,
        claims_account_count,
        claims_program_index,
    }))
}

fn commit_geometry_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(SignedDeltaFrameSpecV3, usize, usize), ProgramError> {
    if accounts.len() <= DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let checkpoint_account = account(accounts, COMMIT_CHECKPOINT)?;
    let checkpoint = read_checkpoint(program_id, checkpoint_account)?.0;
    require_checkpoint_pda(program_id, checkpoint_account, checkpoint)?;
    if !DEALER_SCENARIO_COMMIT_CHECKPOINT_ADMISSIBLE_STATES_V1.admits(checkpoint.phase()) {
        return Err(TradingSbfError::Transition.into());
    }
    let request_data = account(accounts, COMMIT_REQUEST)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let request = DealerScenarioTradeRequestV3::decode(&request_data)
        .map_err(|_| TradingSbfError::Content)?;
    let plan = request
        .claims_plan()
        .map_err(|_| TradingSbfError::Content)?;
    let spec =
        SignedDeltaFrameSpecV3::new(plan.position_count()).map_err(|_| TradingSbfError::Content)?;
    Ok((
        spec,
        usize::from(spec.account_count().map_err(|_| TradingSbfError::Content)?),
        usize::from(checkpoint.evaluation().custody_effect_count),
    ))
}

fn authenticate_commit_releases_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    spec: SignedDeltaFrameSpecV3,
    claims_end: usize,
) -> Result<(), ProgramError> {
    let claims_accounts = accounts
        .get(DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1..claims_end)
        .ok_or(TradingSbfError::Content)?;
    let trading_program = account(
        claims_accounts,
        claims_role_index(spec, ClaimsFrameRoleV1::CallerProgram)?,
    )?;
    let trading_programdata = account(
        claims_accounts,
        claims_role_index(spec, ClaimsFrameRoleV1::CallerProgramData)?,
    )?;
    let activation_cache = account(
        claims_accounts,
        claims_role_index(spec, ClaimsFrameRoleV1::ActivationCache)?,
    )?;
    let registry = account(
        claims_accounts,
        claims_role_index(spec, ClaimsFrameRoleV1::RegistryProgram)?,
    )?;
    let checkpoint = read_checkpoint(program_id, account(accounts, COMMIT_CHECKPOINT)?)?.0;
    let release_set = checkpoint.input().release_set;
    if trading_program.key != program_id {
        return Err(TradingSbfError::Release.into());
    }
    let trading_receipt = authenticate_activated_role_v1(
        registry,
        activation_cache,
        &release_set,
        ExecutionRoleV1::Trading,
        trading_program,
        trading_programdata,
    )
    .map_err(|_| TradingSbfError::Release)?;
    if trading_receipt.program().as_bytes() != program_id.as_ref() {
        return Err(TradingSbfError::Release.into());
    }
    let custody_program = account(accounts, COMMIT_CUSTODY_PROGRAM)?;
    let custody_receipt = authenticate_activated_role_v1(
        registry,
        activation_cache,
        &release_set,
        ExecutionRoleV1::Custody,
        custody_program,
        account(accounts, COMMIT_CUSTODY_PROGRAMDATA)?,
    )
    .map_err(|_| TradingSbfError::Release)?;
    if custody_receipt.program().as_bytes() != custody_program.key.as_ref() {
        return Err(TradingSbfError::Release.into());
    }
    Ok(())
}

fn prepare_candidate_obligation_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<Vec<u8>, ProgramError> {
    let checkpoint = read_checkpoint(program_id, account(accounts, COMMIT_CHECKPOINT)?)?.0;
    let input = checkpoint.input();
    let request_data = account(accounts, COMMIT_REQUEST)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let request = DealerScenarioTradeRequestV3::decode(&request_data)
        .map_err(|_| TradingSbfError::Content)?;
    let obligation = account(accounts, COMMIT_OBLIGATION)?;
    let obligation_data = obligation
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if obligation.owner != program_id
        || obligation.key.to_bytes() != input.obligation
        || input.obligation != request.obligation
        || hash(&obligation_data).to_bytes() != input.obligation_prestate_digest
        || input.obligation_prestate_digest != request.current_obligation_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    let current = DealerObligationProjectionV3::decode(&obligation_data)
        .map_err(|_| TradingSbfError::Content)?;
    if current.width() != request.width || current.revision() != request.current_obligation_revision
    {
        return Err(TradingSbfError::Transition.into());
    }
    let mut candidate = vec![0_u8; obligation_data.len()];
    let mut values =
        vec![0_u64; usize::try_from(request.width).map_err(|_| TradingSbfError::Content)?];
    request
        .decode_candidate_obligations(&mut values)
        .map_err(|_| TradingSbfError::Content)?;
    stage_scenario_obligation_replacement_v3(current, &values, &mut candidate)
        .map_err(|_| TradingSbfError::Transition)?;
    drop(obligation_data);
    let candidate_data = account(accounts, COMMIT_CANDIDATE_OBLIGATION)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if candidate_data.as_ref() != candidate.as_slice()
        || hash(&candidate_data).to_bytes() != request.candidate_obligation_digest
        || request.candidate_obligation_digest
            != checkpoint.evaluation().candidate_obligation_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(candidate)
}

fn prepare_committed_checkpoint_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    reservation_receipts: [[u8; 32]; dclutch_dealer_codec::scenario_reservation_receipt_v1::DEALER_SCENARIO_MAX_RESERVATIONS_V1],
) -> Result<([u8; 32], Vec<u8>), ProgramError> {
    let checkpoint_account = account(accounts, COMMIT_CHECKPOINT)?;
    let (checkpoint, prestate_digest) = read_checkpoint(program_id, checkpoint_account)?;
    let input = checkpoint.input();
    let request_data = account(accounts, COMMIT_REQUEST)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let request = DealerScenarioTradeRequestV3::decode(&request_data)
        .map_err(|_| TradingSbfError::Content)?;
    let root_data = account(accounts, COMMIT_ROOT)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let evidence = DealerScenarioCommitEvidenceV1 {
        request_digest: hash(&request_data).to_bytes(),
        root_prestate_digest: hash(&root_data).to_bytes(),
        claims_prestate_digest: input.claims_prestate_digest,
        obligation_prestate_digest: input.obligation_prestate_digest,
        custody_prestate_digest: input.custody_prestate_digest,
        evaluation_receipt_digest: account_data_digest(account(
            accounts,
            COMMIT_EVALUATION_RECEIPT,
        )?)?,
        candidate_bank_digest: account_data_digest(account(accounts, COMMIT_CANDIDATE_BANK)?)?,
        candidate_obligation_digest: request.candidate_obligation_digest,
        claims_delta_digest: account_data_digest(account(accounts, COMMIT_CLAIMS_DELTA)?)?,
        effects_digest: account_data_digest(account(accounts, COMMIT_EFFECTS)?)?,
        reservation_receipt_digests: reservation_receipts,
    };
    let clock = Clock::from_account_info(account(accounts, COMMIT_CLOCK)?)
        .map_err(|_| TradingSbfError::Content)?;
    let mut bytes = vec![0_u8; DEALER_SCENARIO_CHECKPOINT_BYTES_V1];
    checkpoint
        .commit_into(clock.slot, prestate_digest, evidence, &mut bytes)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok((prestate_digest, bytes))
}

fn prepare_claims_authority_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    claims_account_count: usize,
) -> Result<(Vec<u8>, CallerAuthoritySeedsV1, u8), ProgramError> {
    let request_data = account(accounts, COMMIT_REQUEST)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let request = DealerScenarioTradeRequestV3::decode(&request_data)
        .map_err(|_| TradingSbfError::Content)?;
    let plan = request
        .claims_plan()
        .map_err(|_| TradingSbfError::Content)?;
    if plan.caller_role() != CallerRole::Trading {
        return Err(TradingSbfError::Release.into());
    }
    let claims_wire = request.claims_packet().to_vec();
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(plan.release_set()).map_err(|_| TradingSbfError::Content)?,
        plan.market(),
        ExecutionRoleV1::Trading,
        plan.request_id(),
        hash(&claims_wire).to_bytes(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (authority, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    let child = accounts
        .get(
            DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1
                ..DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1
                    .checked_add(claims_account_count)
                    .ok_or(TradingSbfError::Content)?,
        )
        .ok_or(TradingSbfError::Content)?;
    if account(child, 0)?.key != &authority {
        return Err(TradingSbfError::Release.into());
    }
    Ok((claims_wire, seeds, bump))
}

fn write_prepared_checkpoint_v1(
    program_id: &Pubkey,
    checkpoint: &AccountInfo<'_>,
    expected_prestate_digest: [u8; 32],
    next: &[u8],
) -> Result<(), ProgramError> {
    if checkpoint.owner != program_id
        || next.len() != DEALER_SCENARIO_CHECKPOINT_BYTES_V1
        || DealerScenarioCheckpointV1::decode(next).is_err()
    {
        return Err(TradingSbfError::Commit.into());
    }
    let current = checkpoint
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if hash(&current).to_bytes() != expected_prestate_digest {
        return Err(TradingSbfError::Commit.into());
    }
    drop(current);
    let mut destination = checkpoint
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    destination.copy_from_slice(next);
    Ok(())
}

fn require_commit_fixed_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let checkpoint = account(accounts, COMMIT_CHECKPOINT)?;
    let obligation = account(accounts, COMMIT_OBLIGATION)?;
    let custody_program = account(accounts, COMMIT_CUSTODY_PROGRAM)?;
    let custody_programdata = account(accounts, COMMIT_CUSTODY_PROGRAMDATA)?;
    let readonly = [
        COMMIT_CLOCK,
        COMMIT_REQUEST,
        COMMIT_EVALUATION_RECEIPT,
        COMMIT_CANDIDATE_BANK,
        COMMIT_CANDIDATE_OBLIGATION,
        COMMIT_CLAIMS_DELTA,
        COMMIT_EFFECTS,
        COMMIT_ROOT,
        COMMIT_CUSTODY_PROGRAMDATA,
        COMMIT_BATCH,
    ];
    if !checkpoint.is_writable
        || checkpoint.is_signer
        || checkpoint.executable
        || checkpoint.owner != program_id
        || !obligation.is_writable
        || obligation.is_signer
        || obligation.executable
        || obligation.owner != program_id
        || !custody_program.executable
        || custody_program.is_signer
        || custody_program.is_writable
        || custody_programdata.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    for index in readonly {
        let current = account(accounts, index)?;
        if current.is_signer || current.is_writable || current.executable {
            return Err(TradingSbfError::Content.into());
        }
    }
    Ok(())
}

fn require_commit_claims_privileges_v1(
    accounts: &[AccountInfo<'_>],
    spec: SignedDeltaFrameSpecV3,
    claims_end: usize,
) -> Result<(), ProgramError> {
    let claims_accounts = accounts
        .get(DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1..claims_end)
        .ok_or(TradingSbfError::Content)?;
    for (index, current) in claims_accounts.iter().enumerate() {
        let expected = spec
            .account(u16::try_from(index).map_err(|_| TradingSbfError::Content)?)
            .map_err(|_| TradingSbfError::Content)?;
        if current.is_signer || current.is_writable != expected.privileges().writable() {
            return Err(TradingSbfError::Content.into());
        }
    }
    Ok(())
}

fn authenticate_commit_artifacts_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let checkpoint_account = account(accounts, COMMIT_CHECKPOINT)?;
    let checkpoint = read_checkpoint(program_id, checkpoint_account)?.0;
    let request_data = account(accounts, COMMIT_REQUEST)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let request = DealerScenarioTradeRequestV3::decode(&request_data)
        .map_err(|_| TradingSbfError::Content)?;
    let input = checkpoint.input();
    if request.release_set != input.release_set
        || request.market != input.market
        || request.child_root != input.child_root
        || request.generation != input.generation
        || hash(request.bytes()).to_bytes() != input.request_digest
        || account(accounts, COMMIT_ROOT)?.key.to_bytes() != input.child_root
    {
        return Err(TradingSbfError::Transition.into());
    }
    for index in [
        COMMIT_EVALUATION_RECEIPT,
        COMMIT_CANDIDATE_BANK,
        COMMIT_CANDIDATE_OBLIGATION,
        COMMIT_CLAIMS_DELTA,
        COMMIT_EFFECTS,
    ] {
        let current = account(accounts, index)?;
        if current.owner != program_id || current.executable {
            return Err(TradingSbfError::Release.into());
        }
    }
    let evaluation_account = account(accounts, COMMIT_EVALUATION_RECEIPT)?;
    let evaluation_data = evaluation_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let evaluation = DealerScenarioEvaluationReceiptV1::decode(&evaluation_data)
        .map_err(|_| TradingSbfError::Content)?;
    let expected_evaluation = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_EVALUATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint_account.key.as_ref(),
            &input.request_digest,
        ],
        program_id,
    )
    .0;
    if evaluation_account.key != &expected_evaluation
        || evaluation.producer_program != program_id.to_bytes()
        || evaluation.checkpoint != checkpoint_account.key.to_bytes()
        || evaluation.request_digest != input.request_digest
        || hash(&evaluation_data).to_bytes() != checkpoint.evaluation().evaluation_receipt_digest
        || evaluation.candidate_bank_digest
            != account_data_digest(account(accounts, COMMIT_CANDIDATE_BANK)?)?
        || evaluation.candidate_obligation_digest
            != account_data_digest(account(accounts, COMMIT_CANDIDATE_OBLIGATION)?)?
        || evaluation.claims_delta_digest
            != account_data_digest(account(accounts, COMMIT_CLAIMS_DELTA)?)?
        || evaluation.effects_digest != account_data_digest(account(accounts, COMMIT_EFFECTS)?)?
        || evaluation.custody_effect_count != checkpoint.evaluation().custody_effect_count
    {
        return Err(TradingSbfError::Transition.into());
    }
    drop(evaluation_data);
    let claims_delta = account(accounts, COMMIT_CLAIMS_DELTA)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if claims_delta.as_ref() != request.claims_packet() {
        return Err(TradingSbfError::Transition.into());
    }
    drop(claims_delta);
    let effects_data = account(accounts, COMMIT_EFFECTS)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let effects = DealerScenarioCustodyEffectManifestV1::decode(&effects_data)
        .map_err(|_| TradingSbfError::Content)?;
    if effects.producer_program != program_id.to_bytes()
        || effects.checkpoint != checkpoint_account.key.to_bytes()
        || effects.request_digest != input.request_digest
        || effects.effect_count != checkpoint.evaluation().custody_effect_count
        || hash(&effects_data).to_bytes() != checkpoint.evaluation().effects_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

fn authenticate_locked_batch_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    effect_accounts_start: usize,
    custody_program: &AccountInfo<'_>,
) -> Result<[[u8; 32]; DEALER_SCENARIO_MAX_RESERVATIONS_V1], ProgramError> {
    let mut context = Box::<LockedBatchCommitContextV1>::default();
    authenticate_locked_checkpoint_v1(program_id, accounts, &mut context)?;
    authenticate_locked_batch_header_v1(program_id, accounts, custody_program, &mut context)?;
    authenticate_locked_effect_manifest_v1(program_id, accounts, &mut context)?;
    for ordinal in 0..usize::from(context.effect_count) {
        let offset = effect_accounts_start
            .checked_add(
                ordinal
                    .checked_mul(DEALER_SCENARIO_CHECKPOINT_COMMIT_EFFECT_ACCOUNTS_V1)
                    .ok_or(TradingSbfError::Content)?,
            )
            .ok_or(TradingSbfError::Content)?;
        authenticate_locked_effect_v1(
            accounts,
            offset,
            u8::try_from(ordinal).map_err(|_| TradingSbfError::Content)?,
            custody_program,
            &context,
        )?;
    }
    Ok(context.receipt_digests)
}

#[inline(never)]
fn authenticate_locked_checkpoint_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    context: &mut LockedBatchCommitContextV1,
) -> Result<(), ProgramError> {
    let checkpoint_account = account(accounts, COMMIT_CHECKPOINT)?;
    let checkpoint = read_checkpoint(program_id, checkpoint_account)?.0;
    let input = checkpoint.input();
    let evaluation = checkpoint.evaluation();
    context.checkpoint = checkpoint_account.key.to_bytes();
    context.release_set = input.release_set;
    context.market = input.market;
    context.request_digest = input.request_digest;
    context.effects_digest = evaluation.effects_digest;
    context.refund_beneficiary = input.refund_beneficiary;
    context.generation = input.generation;
    context.expires_at = input.expires_at;
    context.effect_count = evaluation.custody_effect_count;
    for ordinal in 0..DEALER_SCENARIO_MAX_RESERVATIONS_V1 {
        context.checkpoint_receipt_digests[ordinal] = checkpoint
            .reservation_receipt_digest(
                u8::try_from(ordinal).map_err(|_| TradingSbfError::Content)?,
            )
            .map_err(|_| TradingSbfError::Content)?;
    }
    Ok(())
}

#[inline(never)]
fn authenticate_locked_batch_header_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    custody_program: &AccountInfo<'_>,
    context: &mut LockedBatchCommitContextV1,
) -> Result<(), ProgramError> {
    let checkpoint_account = account(accounts, COMMIT_CHECKPOINT)?;
    let batch_account = account(accounts, COMMIT_BATCH)?;
    if batch_account.owner != custody_program.key
        || batch_account.data_len() != DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1
    {
        return Err(TradingSbfError::Release.into());
    }
    let batch_data = batch_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let batch = DealerScenarioReservationBatchV1::decode(&batch_data)
        .map_err(|_| TradingSbfError::Content)?;
    // Custody signs this account into existence under exactly two seeds, and
    // the checkpoint is itself derived from the request digest, so a third
    // request-digest seed here named an address Custody can never create. The
    // request digest is still bound -- by the decoded body, immediately below.
    let expected_batch = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
            checkpoint_account.key.as_ref(),
        ],
        custody_program.key,
    )
    .0;
    if batch_account.key != &expected_batch
        || batch.status != DealerScenarioReservationBatchStatusV1::Reserved
        || batch.effect_count != context.effect_count
        || batch.reserved_count != batch.effect_count
        || batch.rollback_count != 0
        || batch.release_set != context.release_set
        || batch.market != context.market
        || batch.trading_program != program_id.to_bytes()
        || batch.checkpoint != context.checkpoint
        || batch.request_digest != context.request_digest
        || batch.effects_digest != context.effects_digest
        || batch.refund_beneficiary != context.refund_beneficiary
        || batch.expires_at != context.expires_at
        || batch.generation != context.generation
    {
        return Err(TradingSbfError::Transition.into());
    }
    context.batch = batch_account.key.to_bytes();
    context.receipt_digests = batch.receipt_digests;
    context.reservation_states = batch.reservation_states;
    Ok(())
}

#[inline(never)]
fn authenticate_locked_effect_manifest_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    context: &mut LockedBatchCommitContextV1,
) -> Result<(), ProgramError> {
    let effects_data = account(accounts, COMMIT_EFFECTS)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let effects = DealerScenarioCustodyEffectManifestV1::decode(&effects_data)
        .map_err(|_| TradingSbfError::Content)?;
    if effects.producer_program != program_id.to_bytes()
        || effects.checkpoint != context.checkpoint
        || effects.request_digest != context.request_digest
        || effects.effect_count != context.effect_count
        || hash(&effects_data).to_bytes() != context.effects_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    context.effect_digests = effects.effect_digests;
    Ok(())
}

#[inline(never)]
fn authenticate_locked_effect_v1(
    accounts: &[AccountInfo<'_>],
    offset: usize,
    ordinal: u8,
    custody_program: &AccountInfo<'_>,
    context: &LockedBatchCommitContextV1,
) -> Result<(), ProgramError> {
    let receipt_account = account(accounts, offset)?;
    let state_account = account(accounts, offset + 1)?;
    if receipt_account.owner != custody_program.key
        || state_account.owner != custody_program.key
        || receipt_account.is_writable
        || state_account.is_writable
        || state_account.data_len() != DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1
    {
        return Err(TradingSbfError::Release.into());
    }
    let receipt_data = receipt_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let receipt = DealerScenarioReservationReceiptV1::decode(&receipt_data)
        .map_err(|_| TradingSbfError::Content)?;
    let state_data = state_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let state = DealerScenarioReservationStateV1::decode(&state_data)
        .map_err(|_| TradingSbfError::Content)?;
    let index = usize::from(ordinal);
    let receipt_digest = hash(&receipt_data).to_bytes();
    let batch_receipt = context
        .receipt_digests
        .get(index)
        .copied()
        .ok_or(TradingSbfError::Content)?;
    let batch_state = context
        .reservation_states
        .get(index)
        .copied()
        .ok_or(TradingSbfError::Content)?;
    let effect_digest = context
        .effect_digests
        .get(index)
        .copied()
        .ok_or(TradingSbfError::Content)?;
    if receipt.action != DealerScenarioReservationActionV1::Reserve
        || receipt.effect_ordinal != ordinal
        || receipt.effect_count != context.effect_count
        || receipt.producer_program != custody_program.key.to_bytes()
        || receipt.checkpoint != context.checkpoint
        || receipt.request_digest != context.request_digest
        || receipt.effects_digest != context.effects_digest
        || receipt.reservation != state_account.key.to_bytes()
        || receipt.reservation_poststate_digest != hash(&state_data).to_bytes()
        || receipt_digest != batch_receipt
        || receipt_digest
            != context
                .checkpoint_receipt_digests
                .get(index)
                .copied()
                .ok_or(TradingSbfError::Content)?
        || !DEALER_SCENARIO_ACTIVE_RESERVATION_ADMISSIBLE_STATES_V1.admits(state.status)
        || state.ordinal != ordinal
        || state.effect_count != context.effect_count
        || state.batch != context.batch
        || state.checkpoint != context.checkpoint
        || state.request_digest != context.request_digest
        || state.effects_digest != context.effects_digest
        || state.effect_digest != effect_digest
        || state_account.key.to_bytes() != batch_state
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

#[inline(never)]
fn execute_dealer_scenario_claims_v1(
    accounts: &[AccountInfo<'_>],
    prepared: &PreparedDealerScenarioCommitV1,
) -> Result<(), ProgramError> {
    let start = DEALER_SCENARIO_CHECKPOINT_COMMIT_FIXED_ACCOUNTS_V1;
    let end = start
        .checked_add(prepared.claims_account_count)
        .ok_or(TradingSbfError::Content)?;
    let child = accounts.get(start..end).ok_or(TradingSbfError::Content)?;
    let claims_program = account(child, prepared.claims_program_index)?;
    let mut metas = Vec::new();
    metas
        .try_reserve_exact(child.len())
        .map_err(|_| TradingSbfError::Content)?;
    for (index, current) in child.iter().enumerate() {
        let signer = index == 0 || current.is_signer;
        metas.push(if current.is_writable {
            AccountMeta::new(*current.key, signer)
        } else {
            AccountMeta::new_readonly(*current.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: prepared.claims_wire.clone(),
    };
    let mut infos = Vec::new();
    infos
        .try_reserve_exact(child.len().saturating_add(1))
        .map_err(|_| TradingSbfError::Content)?;
    infos.extend(child.iter().cloned());
    infos.push(claims_program.clone());
    let bump = [prepared.authority_bump];
    let [domain, release, market, role, context, digest] = prepared.authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump]],
    )
    .map_err(child_refused_v1)?;
    let (producer, receipt_data) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *claims_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    let plan =
        SignedDeltaPlanV3::decode(&prepared.claims_wire).map_err(|_| TradingSbfError::Content)?;
    let receipt =
        SignedDeltaReceiptV3::decode(&receipt_data).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_plan(plan)
        .map_err(|_| TradingSbfError::Transition)?;
    let (positions, aggregates, deltas) = plan.table_bytes();
    let table_digest = hashv(&[
        b"dclutch/claims/signed-delta-table/v3",
        positions,
        aggregates,
        deltas,
    ])
    .to_bytes();
    let post_resource_digest = signed_delta_post_resource_digest(&infos, plan.position_count())?;
    if receipt.packet_digest() != hash(&prepared.claims_wire).to_bytes()
        || receipt.table_digest() != table_digest
        || receipt.claims_program() != claims_program.key.to_bytes()
        || receipt.post_resource_digest() != post_resource_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

fn claims_role_index(
    spec: SignedDeltaFrameSpecV3,
    role: ClaimsFrameRoleV1,
) -> Result<usize, ProgramError> {
    for index in 0..spec.account_count().map_err(|_| TradingSbfError::Content)? {
        if spec
            .account(index)
            .map_err(|_| TradingSbfError::Content)?
            .role()
            == role
        {
            return Ok(usize::from(index));
        }
    }
    Err(TradingSbfError::Content.into())
}

/// Close an expired checkpoint to its immutable refund beneficiary.
#[inline(never)]
pub fn process_dealer_scenario_checkpoint_cleanup_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_dealer_scenario_checkpoint_cleanup_v1(instruction_data)
        || accounts.len() != DEALER_SCENARIO_CHECKPOINT_CLEANUP_ACCOUNT_COUNT_V1
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let checkpoint_account = account(accounts, CLEANUP_CHECKPOINT)?;
    let beneficiary = account(accounts, CLEANUP_BENEFICIARY)?;
    let clock_account = account(accounts, CLEANUP_CLOCK)?;
    let (checkpoint, _) = read_checkpoint(program_id, checkpoint_account)?;
    require_checkpoint_pda(program_id, checkpoint_account, checkpoint)?;
    let clock = Clock::from_account_info(clock_account).map_err(|_| TradingSbfError::Content)?;
    let expected = checkpoint
        .cleanup_beneficiary(clock.slot)
        .map_err(|_| TradingSbfError::Transition)?;
    if beneficiary.key.to_bytes() != expected
        || !checkpoint_account.is_writable
        || !beneficiary.is_writable
        || beneficiary.is_signer
        || beneficiary.executable
        || beneficiary.key == checkpoint_account.key
    {
        return Err(TradingSbfError::Commit.into());
    }
    let amount = checkpoint_account.lamports();
    let beneficiary_post = beneficiary
        .lamports()
        .checked_add(amount)
        .ok_or(TradingSbfError::Commit)?;
    **beneficiary
        .try_borrow_mut_lamports()
        .map_err(|_| TradingSbfError::Commit)? = beneficiary_post;
    **checkpoint_account
        .try_borrow_mut_lamports()
        .map_err(|_| TradingSbfError::Commit)? = 0;
    checkpoint_account
        .resize(0)
        .map_err(|_| TradingSbfError::Commit)?;
    checkpoint_account.assign(&system_program::ID);
    Ok(())
}

fn read_checkpoint(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Result<(DealerScenarioCheckpointV1, [u8; 32]), ProgramError> {
    if account.owner != program_id
        || !account.is_writable
        || account.is_signer
        || account.executable
        || account.data_len() != DEALER_SCENARIO_CHECKPOINT_BYTES_V1
    {
        return Err(TradingSbfError::Content.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let digest = hash(&data).to_bytes();
    let checkpoint =
        DealerScenarioCheckpointV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
    Ok((checkpoint, digest))
}

fn require_checkpoint_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    checkpoint: DealerScenarioCheckpointV1,
) -> Result<(), ProgramError> {
    require_checkpoint_pda_hinted(program_id, account, checkpoint, 0)
}

/// The same join, reproducing the checkpoint address at a mined bump where the
/// producer supplied one.
///
/// The derivation IS the check: the bump is fed to `create_program_address`
/// over seeds this function builds for itself -- the checkpoint PDA domain and
/// the request digest read out of the account's own body -- and the result is
/// compared with the account the frame supplied, by the equality that was
/// always here. A wrong bump derives a different address, or none, and
/// refuses. Canonicality is enforced where the account is MADE:
/// `process_dealer_scenario_checkpoint_create_v1` allocates only at the
/// address `find_program_address` returns, so a non-canonical hint names an
/// address at which no Trading-owned checkpoint of this width exists.
fn require_checkpoint_pda_hinted(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    checkpoint: DealerScenarioCheckpointV1,
    hint: u8,
) -> Result<(), ProgramError> {
    let digest = checkpoint.input().request_digest;
    let seeds: [&[u8]; 2] = [DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1, digest.as_slice()];
    let expected = if hint == 0 {
        Pubkey::find_program_address(&seeds, program_id).0
    } else {
        let bump = [hint];
        Pubkey::create_program_address(&[seeds[0], seeds[1], bump.as_slice()], program_id)
            .map_err(|_| TradingSbfError::Content)?
    };
    if account.key == &expected {
        Ok(())
    } else {
        Err(TradingSbfError::Content.into())
    }
}

fn write_new_checkpoint(
    program_id: &Pubkey,
    target: &AccountInfo<'_>,
    checkpoint: DealerScenarioCheckpointV1,
) -> Result<(), ProgramError> {
    let bytes = checkpoint.to_bytes().map_err(|_| TradingSbfError::Commit)?;
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if target.owner != program_id || data.len() != bytes.len() || data.iter().any(|byte| *byte != 0)
    {
        return Err(TradingSbfError::Commit.into());
    }
    data.copy_from_slice(&bytes);
    Ok(())
}

fn write_checkpoint_last(
    program_id: &Pubkey,
    target: &AccountInfo<'_>,
    expected: DealerScenarioCheckpointV1,
    next: DealerScenarioCheckpointV1,
) -> Result<(), ProgramError> {
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if target.owner != program_id
        || data.len() != DEALER_SCENARIO_CHECKPOINT_BYTES_V1
        || DealerScenarioCheckpointV1::decode(&data).map_err(|_| TradingSbfError::Commit)?
            != expected
    {
        return Err(TradingSbfError::Commit.into());
    }
    let next_bytes = next.to_bytes().map_err(|_| TradingSbfError::Commit)?;
    data.copy_from_slice(&next_bytes);
    Ok(())
}

fn page_receipt_digest(
    checkpoint_key: &Pubkey,
    checkpoint: DealerScenarioCheckpointV1,
    checkpoint_prestate_digest: [u8; 32],
    page_index: u8,
    observations: &[AccountInfo<'_>],
) -> Result<[u8; 32], ProgramError> {
    let mut digests = Vec::with_capacity(observations.len());
    for current in observations {
        let data = current
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Content)?;
        let lamports = current.lamports().to_le_bytes();
        let data_len = u64::try_from(data.len())
            .map_err(|_| TradingSbfError::Content)?
            .to_le_bytes();
        let executable = [u8::from(current.executable)];
        digests.push(
            hashv(&[
                current.key.as_ref(),
                current.owner.as_ref(),
                &lamports,
                &data_len,
                &executable,
                &data,
            ])
            .to_bytes(),
        );
    }
    let page = [page_index];
    let input = checkpoint.input();
    let mut parts = vec![
        DEALER_SCENARIO_PAGE_RECEIPT_DOMAIN_V1,
        checkpoint_key.as_ref(),
        checkpoint_prestate_digest.as_slice(),
        page.as_slice(),
        input.request_digest.as_slice(),
    ];
    parts.extend(digests.iter().map(<[u8; 32]>::as_slice));
    Ok(hashv(&parts).to_bytes())
}

fn membership_page_digest(
    checkpoint_key: &Pubkey,
    request_digest: [u8; 32],
    page_index: u8,
    observations: &[AccountInfo<'_>],
) -> Result<[u8; 32], ProgramError> {
    let page = [page_index];
    let count = [u8::try_from(observations.len()).map_err(|_| TradingSbfError::Content)?];
    let mut parts = vec![
        DEALER_SCENARIO_MEMBERSHIP_PAGE_DOMAIN_V1,
        checkpoint_key.as_ref(),
        request_digest.as_slice(),
        page.as_slice(),
        count.as_slice(),
    ];
    parts.extend(observations.iter().map(|account| account.key.as_ref()));
    Ok(hashv(&parts).to_bytes())
}

fn strictly_increasing_membership(prior_key: [u8; 32], observations: &[AccountInfo<'_>]) -> bool {
    let mut previous = prior_key;
    for current in observations {
        let key = current.key.to_bytes();
        if key <= previous {
            return false;
        }
        previous = key;
    }
    true
}

fn joined_page_digest(
    domain: &[u8],
    checkpoint: DealerScenarioCheckpointV1,
    start: usize,
    end: usize,
) -> Result<[u8; 32], ProgramError> {
    let mut digests = Vec::new();
    for page in start..end {
        digests.push(
            checkpoint
                .page_receipt_digest(u8::try_from(page).map_err(|_| TradingSbfError::Transition)?)
                .map_err(|_| TradingSbfError::Transition)?,
        );
    }
    let input = checkpoint.input();
    let mut parts = vec![domain, input.request_digest.as_slice()];
    parts.extend(digests.iter().map(<[u8; 32]>::as_slice));
    Ok(hashv(&parts).to_bytes())
}

fn account_data_digest(account: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.is_empty() {
        return Err(TradingSbfError::Content.into());
    }
    Ok(hash(&data).to_bytes())
}

fn has_duplicate_keys(accounts: &[AccountInfo<'_>]) -> bool {
    accounts.iter().enumerate().any(|(index, current)| {
        accounts
            .get(index.saturating_add(1)..)
            .unwrap_or(&[])
            .iter()
            .any(|other| current.key == other.key)
    })
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}

const _: () = assert!(DEALER_SCENARIO_PREPARATION_PAGES_V1 == 6);
const _: () = assert!(DEALER_SCENARIO_CHECKPOINT_CREATE_ACCOUNT_COUNT_V1 + 1 < 64);
const _: () = assert!(
    DEALER_SCENARIO_CHECKPOINT_PAGE_FIXED_ACCOUNTS_V1
        + DEALER_SCENARIO_CHECKPOINT_PAGE_MAX_OBSERVATIONS_V1
        + 1
        < 64
);
const _: () = assert!(DEALER_SCENARIO_CHECKPOINT_EVALUATE_ACCOUNT_COUNT_V1 + 1 < 64);
const _: () = assert!(DEALER_SCENARIO_CHECKPOINT_RESERVATION_ACCOUNT_COUNT_V1 + 1 < 64);
const _: () = assert!(DEALER_SCENARIO_CHECKPOINT_CLEANUP_ACCOUNT_COUNT_V1 + 1 < 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_are_exact_and_page_ordinal_is_not_a_selector() {
        assert!(is_dealer_scenario_checkpoint_create_v1(b"DCLTDCP1"));
        assert!(is_dealer_scenario_checkpoint_page_v1(b"DCLTDPG1\x05"));
        assert!(is_dealer_scenario_checkpoint_evaluate_v1(b"DCLTDEV1"));
        assert!(is_dealer_scenario_checkpoint_reserve_v1(b"DCLTDRV1"));
        assert!(is_dealer_scenario_checkpoint_rollback_v1(b"DCLTDRB1"));
        assert!(is_dealer_scenario_checkpoint_commit_v1(b"DCLTDCM1"));
        assert!(is_dealer_scenario_checkpoint_cleanup_v1(b"DCLTDCL1"));
        assert!(!is_dealer_scenario_checkpoint_page_v1(b"DCLTDPG1"));
        assert!(!is_dealer_scenario_checkpoint_create_v1(b"DCLTDCP1\x00"));
        assert!(!is_dealer_scenario_checkpoint_reserve_v1(b"DCLTDRV1\x00"));
        assert!(!is_dealer_scenario_checkpoint_rollback_v1(b"DCLTDRB2"));
        assert!(!is_dealer_scenario_checkpoint_commit_v1(b"DCLTDCM1\x00"));
    }

    /// The reserve arm must not answer to Direct's replay-setup magic.
    ///
    /// Until 2026-09-01 it did: both were `DCLTDRS1`, both are top-level
    /// selectors of this same ELF, and this arm is tried first, so a
    /// mis-sized Direct replay-setup request routed HERE instead of refusing.
    /// The bare 8-byte shape is what made it reachable -- `data == MAGIC`
    /// admits exactly the prefix Direct's 120-byte request begins with.
    #[test]
    fn the_reserve_arm_does_not_answer_to_directs_replay_setup_magic() {
        let direct = dclutch_direct_codec::replay_setup_v1::DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1;
        assert_ne!(
            DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1, direct,
            "two top-level selectors of one ELF may not share a discriminant"
        );
        assert!(
            !is_dealer_scenario_checkpoint_reserve_v1(&direct),
            "a bare Direct replay-setup prefix must not select the Dealer reserve arm"
        );
        // And the historical value must select nothing here any more.
        assert!(!is_dealer_scenario_checkpoint_reserve_v1(b"DCLTDRS1"));
    }
}
